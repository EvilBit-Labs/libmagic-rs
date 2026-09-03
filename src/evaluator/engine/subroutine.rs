// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `MetaType::Use` subroutine dispatch: the RAII anchor/base-offset/endian-flip
//! scope guard and the subroutine evaluation entry point.
//!
//! Extracted from `engine/mod.rs` as a pure code-motion split (issue #391
//! item 1, Unit U3). See GOTCHAS S3.10 and S16 for the subroutine
//! base-offset and `\^` endian-flip semantics this module implements.

use super::evaluate_rules;
use crate::LibmagicError;
use crate::evaluator::{EvaluationContext, RecursionGuard, RuleMatch, offset};
use crate::parser::ast::MagicRule;
use log::{debug, warn};
use std::sync::atomic::{AtomicBool, Ordering};

/// RAII guard for `MetaType::Use` subroutine dispatch.
///
/// Saves `last_match_end` and `base_offset` on entry, seeds the context
/// with the use-site offset (for both fields so that a subroutine's
/// `&0` relative offset resolves to the use-site and its positive
/// absolute offsets bias against the use-site per magic(5)), and
/// restores both on drop.
///
/// This is the safety net for early-return paths inside
/// `evaluate_use_rule`: a `RecursionGuard::enter` failure or a
/// `Timeout`/`RecursionLimitExceeded` inside the subroutine body would
/// otherwise leave the caller's context with corrupted anchor and
/// base-offset state. The guard's `Drop` impl restores both fields on
/// every exit path, error or success.
struct SubroutineScope<'a> {
    context: &'a mut EvaluationContext,
    saved_anchor: usize,
    saved_base: usize,
    saved_flip: bool,
}

impl<'a> SubroutineScope<'a> {
    /// Enter a subroutine body. `flip_use` is the `\^` prefix on the
    /// invoking `use` site; the effective flip inside the body is the
    /// caller's flip state XOR `flip_use` -- matching libmagic's `flip =
    /// !flip` toggle, which nests: a `\^use` inside an already-flipped
    /// subroutine un-flips. Restored on drop along with anchor and base.
    fn enter(context: &'a mut EvaluationContext, use_site: usize, flip_use: bool) -> Self {
        let saved_anchor = context.last_match_end();
        let saved_base = context.base_offset();
        let saved_flip = context.flip_endian();
        context.set_last_match_end(use_site);
        context.set_base_offset(use_site);
        context.set_flip_endian(saved_flip ^ flip_use);
        Self {
            context,
            saved_anchor,
            saved_base,
            saved_flip,
        }
    }

    fn context(&mut self) -> &mut EvaluationContext {
        self.context
    }
}

impl Drop for SubroutineScope<'_> {
    fn drop(&mut self) {
        self.context.set_last_match_end(self.saved_anchor);
        self.context.set_base_offset(self.saved_base);
        self.context.set_flip_endian(self.saved_flip);
    }
}

/// Process-local once guard for the "use directive without rule environment"
/// warning. Ensures we surface the misconfiguration exactly once per process
/// so low-level programmatic consumers of [`evaluate_rules`] (tests, fuzz
/// harnesses) that intentionally run without a `MagicDatabase`-attached
/// environment do not flood the log on every `Use` rule they encounter.
static USE_WITHOUT_RULE_ENV_WARNED: AtomicBool = AtomicBool::new(false);

/// Rebase an `Indirect` offset's dereferenced result into the caller's frame.
///
/// A `use` site continues the caller's frame, so the pointer it reads holds a
/// position *within* that frame, not an absolute file offset. jpeg's
/// `>>(2.S+2) use jpeg_segment` reads a segment length: invoked at 20 with a
/// length of 128 at byte 22, the next segment is `20 + 128 + 2 = 150`.
///
/// An `indirect` rule is the opposite -- it re-enters at a fresh absolute
/// position, which is why mach-o's `>(8.L) indirect x` must keep its
/// `arch[i].offset` value unbiased. That rule never reaches this function; the
/// engine dispatches it separately. See GOTCHAS S3.10.
///
/// Excluded: a non-`Indirect` offset (already in the right frame), a zero base
/// (top level), and `&(N.X)` (`result_relative`), whose anchor addition
/// already carries the use-site.
fn apply_use_result_base(
    spec: &crate::parser::ast::OffsetSpec,
    resolved: usize,
    context: &EvaluationContext,
    buffer: &[u8],
) -> Result<usize, LibmagicError> {
    use crate::parser::ast::OffsetSpec;

    let base = context.base_offset();
    if base == 0
        || !matches!(
            spec,
            OffsetSpec::Indirect {
                result_relative: false,
                ..
            }
        )
    {
        return Ok(resolved);
    }
    let rebased = resolved.checked_add(base).ok_or_else(|| {
        LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
            offset: i64::try_from(resolved).unwrap_or(i64::MAX),
        })
    })?;
    if rebased > buffer.len() {
        return Err(LibmagicError::EvaluationError(
            crate::error::EvaluationError::BufferOverrun { offset: rebased },
        ));
    }
    Ok(rebased)
}

/// Evaluate a `TypeKind::Meta(MetaType::Use { name, .. })` rule inline.
///
/// Looks up `name` in the context's rule environment, temporarily sets the
/// GNU `file` previous-match anchor to the resolved offset, and recursively
/// evaluates the subroutine's rules against `buffer`. Any matches produced
/// by the subroutine are returned in document order and are intended to be
/// pushed into the caller's match vector *before* the synthetic `Use` match
/// itself (matching GNU `file` behavior where a `use` site is replaced by
/// its expansion in the output).
///
/// Returns `Ok((Some(terminal_anchor), matches))` on a successful resolution
/// (even if the subroutine produced no matches), or `Ok((None, vec![]))`
/// when:
/// - the context has no rule environment attached (programmatic consumers
///   bypassing `MagicDatabase`)
/// - the referenced name is not in the table (logged at warn level)
///
/// Recursion-limit propagation is handled via [`RecursionGuard`] so that a
/// subroutine calling `use` on itself triggers `RecursionLimitExceeded`
/// instead of a stack overflow.
pub(crate) fn evaluate_use_rule(
    rule: &MagicRule,
    name: &str,
    flip_endian: bool,
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<(Option<usize>, Vec<RuleMatch>), LibmagicError> {
    let Some(env) = context.rule_env() else {
        // Surface the misconfiguration once per process at warn! level so
        // it is visible in default logging, then gate subsequent hits so a
        // magic file with many `use` directives does not flood the log.
        // Use `Ordering::Relaxed`: the flag is an idempotent diagnostic
        // latch, not a synchronization primitive guarding other state.
        if USE_WITHOUT_RULE_ENV_WARNED.swap(true, Ordering::Relaxed) {
            debug!("use directive '{name}' evaluated without a rule environment; no-op");
        } else {
            warn!(
                "use directive '{name}' evaluated without a rule environment; treating as no-op (subsequent occurrences suppressed)"
            );
        }
        return Ok((None, Vec::new()));
    };

    let Some(subroutine_rules) = env.name_table.get(name) else {
        warn!("use directive references unknown name '{name}'");
        return Ok((None, Vec::new()));
    };
    // The `name` line can carry its own description (e.g. Mach-O universal
    // `0 name mach-o \b [`, `0 name matlab4 Matlab v4 mat-file`). GNU `file`
    // emits it ahead of the subroutine body, attached with no separating
    // space. Capture it here while the env borrow is live; the owned `String`
    // lets us drop that borrow before mutating the context below. `None` for
    // a bare `name <id>`.
    let name_message = env.name_table.name_message(name);
    // `NameTable::get` returns an `Arc<[MagicRule]>`, so this clone is a
    // reference-count increment rather than a deep copy of the rule tree.
    // The Arc is cloned here to release the immutable borrow of `context`
    // (via `env`) before we mutably borrow the context below.

    // Resolve the use-site offset under the *caller's* base, not the
    // subroutine's -- the use rule itself is in the caller's scope.
    let resolved = offset::resolve_offset_with_base(
        &rule.offset,
        buffer,
        context.last_match_end(),
        context.base_offset(),
    )?;
    let absolute_offset = apply_use_result_base(&rule.offset, resolved, context, buffer)?;

    // `SubroutineScope` seeds `last_match_end` and `base_offset` with
    // the use-site offset and restores both on drop. This is the
    // safety net for early-return paths below -- if
    // `RecursionGuard::enter` or the inner `evaluate_rules` returns
    // `Err(Timeout)` / `Err(RecursionLimitExceeded)`, the `?` unwinds
    // through the guard's `Drop` impl and the caller's context
    // returns to its pre-use state. Without the RAII wrapper a manual
    // save/restore pair would be bypassed on every error path.
    // Capture both the subroutine's matches AND the terminal anchor
    // where the subroutine left `last_match_end`. The terminal anchor
    // is what GNU `file`-compatible inlining semantics require: sibling
    // rules after the `use` site must resolve `&N` against the position
    // the subroutine reached, not the use-site offset. Reading the
    // anchor INSIDE the scope (before Drop restores the caller's value)
    // preserves it for the caller.
    let (subroutine_matches, terminal_anchor) = {
        let mut scope = SubroutineScope::enter(context, absolute_offset, flip_endian);
        let mut guard = RecursionGuard::enter(scope.context())?;
        let matches = evaluate_rules(&subroutine_rules, buffer, guard.context())?;
        let terminal = guard.context().last_match_end();
        (matches, terminal)
    };

    // Prepend the `name` line's own description (if any) ahead of the body's
    // matches, matching GNU `file`: `use mach-o` emits the mach-o subroutine's
    // `\b [` before the per-arch body. The name line reads no bytes, so this
    // synthetic match carries a dummy value and does NOT touch the anchor
    // (`terminal_anchor` still comes from the body's evaluation). To reproduce
    // `file`'s no-separator attachment (`ParentSUBMSG`, not `Parent SUBMSG`),
    // ensure the message begins with the `\b` no-separator marker
    // (`concatenate_messages` strips a leading literal `\b` / U+0008); a name
    // message that already starts with one (mach-o's `\b [`) is left as-is so
    // it is not double-marked.
    // A `use` site's own message is dropped (GOTCHAS S14.4), except for a
    // leading no-separator marker, which is a formatting control rather than
    // description text: `>0 use mach-o-cpu \b` must attach the subroutine's
    // first output with no separating space. Recorded here, applied *after*
    // the name message is prepended below -- the subroutine's first output is
    // the name line when it has one, not the first body match.
    let use_site_suppresses_separator =
        crate::evaluator::strip_no_separator_marker(&rule.message).is_some();

    let matches = match name_message {
        Some(msg) if !msg.is_empty() => {
            let attached = if crate::evaluator::strip_no_separator_marker(&msg).is_some() {
                msg
            } else {
                format!("\\b{msg}")
            };
            let name_match = RuleMatch::new(
                attached,
                absolute_offset,
                rule.level,
                crate::parser::ast::Value::Uint(0),
                rule.typ.clone(),
                RuleMatch::calculate_confidence(rule.level),
            );
            let mut combined = Vec::with_capacity(subroutine_matches.len() + 1);
            combined.push(name_match);
            combined.extend(subroutine_matches);
            combined
        }
        _ => subroutine_matches,
    };

    // Applied to the assembled vector so it lands on the first *rendered*
    // match. With a name message that match is the name line, which already
    // carries its own marker, so this is a no-op there and the body's first
    // fragment keeps its ordinary separating space. Marking the body directly
    // instead glued it to the name message.
    let matches = if use_site_suppresses_separator {
        super::output::attach_no_separator_to_first(matches)
    } else {
        matches
    };

    Ok((Some(terminal_anchor), matches))
}

#[cfg(test)]
mod tests {
    //! Branch coverage for [`apply_use_result_base`] (issue #471).
    //!
    //! `apply_use_result_base` decides whether a dereferenced indirect offset is
    //! rebased by the subroutine base. GOTCHAS S3.10 states the contract: a `use`
    //! rule's `Indirect` result is base-relative, an `indirect` rule's stays
    //! absolute, and three cases are excluded -- a non-`Indirect` spec, a zero
    //! base, and `&(N.X)` (`result_relative`), whose anchor addition already
    //! carries the use-site.
    //!
    //! These tests live here rather than in `engine/tests/` because
    //! `apply_use_result_base` has no visibility modifier at all: a private item
    //! is reachable only from its defining module and that module's descendants,
    //! so the sibling `engine::tests` cannot see it. Widening it to reach a test
    //! would follow the same principle
    //! `docs/solutions/developer-experience/rust-test-visibility-boundary.md`
    //! argues against, though that document's own case is the `tests/`-versus-`src/`
    //! crate boundary rather than this one.
    //!
    //! Every expected value below is derived from the arithmetic GOTCHAS S3.10
    //! states, never recorded from a run of the current code. A table written from
    //! observed output would pass by construction and agree with a latent bug as
    //! readily as with correct behavior.

    use super::apply_use_result_base;
    use crate::LibmagicError;
    use crate::error::EvaluationError;
    use crate::evaluator::{EvaluationConfig, EvaluationContext};
    use crate::parser::ast::{Endianness, IndirectAdjustmentOp, OffsetSpec, TypeKind};

    /// The measured JPEG case from GOTCHAS S3.10: `jpeg_segment` invoked at
    /// use-site 20 reads a segment length of 128 at byte 22, and the next segment
    /// sits at `20 + (128 + 2) = 150`.
    const JPEG_USE_SITE: usize = 20;
    const JPEG_RESOLVED: usize = 130;
    const JPEG_REBASED: usize = 150;

    /// Build a plain `use`-style indirect spec: `(2.S+2)`, no anchor relativity.
    fn indirect_spec() -> OffsetSpec {
        OffsetSpec::Indirect {
            base_offset: 2,
            base_relative: false,
            pointer_type: TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            },
            adjustment: 2,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: Endianness::Big,
        }
    }

    /// The `&(N.X)` form: the anchor addition already carries the use-site, so
    /// adding the base again would double-count.
    fn result_relative_spec() -> OffsetSpec {
        match indirect_spec() {
            OffsetSpec::Indirect {
                base_offset,
                base_relative,
                pointer_type,
                adjustment,
                adjustment_op,
                endian,
                ..
            } => OffsetSpec::Indirect {
                base_offset,
                base_relative,
                pointer_type,
                adjustment,
                adjustment_op,
                result_relative: true,
                endian,
            },
            other => other,
        }
    }

    fn context_with_base(base: usize) -> EvaluationContext {
        let mut context = EvaluationContext::new(EvaluationConfig::default());
        context.set_base_offset(base);
        context
    }

    /// Walks every branch of the rebasing rule in one table.
    ///
    /// Each case names the branch it pins and the reason the expected value is
    /// what it is, so a future failure reports a contract rather than two numbers.
    #[test]
    fn test_apply_use_result_base_branch_matrix() {
        let buffer = vec![0u8; 256];

        let cases: &[(&str, OffsetSpec, usize, usize, usize)] = &[
            (
                "use + Indirect + non-zero base rebases: GOTCHAS S3.10 jpeg walk, \
                 use-site 20 + resolved 130 = 150",
                indirect_spec(),
                JPEG_USE_SITE,
                JPEG_RESOLVED,
                JPEG_REBASED,
            ),
            (
                "non-Indirect spec is already in the caller's frame and must not be rebased",
                // 130 == JPEG_RESOLVED; a literal avoids a lossy usize -> i64 cast.
                OffsetSpec::Absolute(130),
                JPEG_USE_SITE,
                JPEG_RESOLVED,
                JPEG_RESOLVED,
            ),
            (
                "zero base is top level (tplink's use-site always resolves to 0): no rebase",
                indirect_spec(),
                0,
                JPEG_RESOLVED,
                JPEG_RESOLVED,
            ),
            (
                "result_relative `&(N.X)` is excluded: the anchor addition already \
                 carries the use-site, so rebasing would double-count",
                result_relative_spec(),
                JPEG_USE_SITE,
                JPEG_RESOLVED,
                JPEG_RESOLVED,
            ),
            (
                "Relative spec is not Indirect: no rebase",
                OffsetSpec::Relative(4),
                JPEG_USE_SITE,
                JPEG_RESOLVED,
                JPEG_RESOLVED,
            ),
            (
                "FromEnd spec is not Indirect: no rebase",
                OffsetSpec::FromEnd(-4),
                JPEG_USE_SITE,
                JPEG_RESOLVED,
                JPEG_RESOLVED,
            ),
        ];

        for (contract, spec, base, resolved, expected) in cases {
            let context = context_with_base(*base);
            let actual = apply_use_result_base(spec, *resolved, &context, &buffer)
                .unwrap_or_else(|e| panic!("{contract}: unexpected error {e:?}"));
            assert_eq!(
                actual, *expected,
                "{contract} (base={base}, resolved={resolved})"
            );
        }
    }

    /// A rebase that cannot fit in `usize` must surface as an evaluation error
    /// rather than wrapping or panicking. Adversarial magic can drive the pointer
    /// value arbitrarily high.
    #[test]
    fn test_apply_use_result_base_overflow_is_invalid_offset_not_panic() {
        let buffer = vec![0u8; 256];
        let context = context_with_base(1);

        let err = apply_use_result_base(&indirect_spec(), usize::MAX, &context, &buffer)
            .expect_err("usize::MAX + base(1) must not silently wrap");

        assert!(
            matches!(
                err,
                LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
            ),
            "overflow must report InvalidOffset, got {err:?}"
        );
    }

    /// A rebased offset past the end of the buffer must be reported, not read.
    /// The buffer is a direct parameter, so this branch is reachable from here.
    #[test]
    fn test_apply_use_result_base_past_end_of_buffer_is_buffer_overrun() {
        // 150 is the rebased target from the jpeg case; a 32-byte buffer cannot
        // hold it.
        let buffer = vec![0u8; 32];
        let context = context_with_base(JPEG_USE_SITE);

        let err = apply_use_result_base(&indirect_spec(), JPEG_RESOLVED, &context, &buffer)
            .expect_err("rebased offset 150 is past the end of a 32-byte buffer");

        assert!(
            matches!(
                err,
                LibmagicError::EvaluationError(EvaluationError::BufferOverrun {
                    offset: JPEG_REBASED
                })
            ),
            "must report BufferOverrun at the rebased offset {JPEG_REBASED}, got {err:?}"
        );
    }

    /// The EOF position itself is a valid resolution target (GOTCHAS S15.1);
    /// only strictly past it is an overrun. This pins the boundary so a future
    /// `>=` tightening is caught here rather than by a dropped child rule.
    #[test]
    fn test_apply_use_result_base_rebase_landing_exactly_at_eof_is_permitted() {
        let buffer = vec![0u8; JPEG_REBASED];
        let context = context_with_base(JPEG_USE_SITE);

        let actual = apply_use_result_base(&indirect_spec(), JPEG_RESOLVED, &context, &buffer)
            .expect("a rebase landing exactly at buffer.len() is a valid position");

        assert_eq!(
            actual, JPEG_REBASED,
            "offset == buffer.len() is the EOF position, not an overrun"
        );
    }
}
