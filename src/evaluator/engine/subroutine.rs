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
    let absolute_offset = offset::resolve_offset_with_base(
        &rule.offset,
        buffer,
        context.last_match_end(),
        context.base_offset(),
    )?;

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
    // first output with no separating space.
    let subroutine_matches = if crate::evaluator::strip_no_separator_marker(&rule.message).is_some()
    {
        super::output::attach_no_separator_to_first(subroutine_matches)
    } else {
        subroutine_matches
    };

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

    Ok((Some(terminal_anchor), matches))
}
