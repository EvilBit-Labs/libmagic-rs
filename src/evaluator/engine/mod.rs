// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Core evaluation engine for magic rules.
//!
//! This module contains the core recursive evaluation logic for executing magic
//! rules against file buffers. It is responsible for:
//! - Evaluating a single rule via [`evaluate_single_rule`] (a thin wrapper
//!   around `evaluate_rules` that delegates one rule through the full
//!   context-aware pipeline)
//! - Evaluating hierarchical rule sets with context (`evaluate_rules`)
//! - Providing a convenience wrapper for evaluation with configuration
//!   (`evaluate_rules_with_config`)

use crate::parser::ast::{MagicRule, MetaType, TypeKind};
use crate::{EvaluationConfig, LibmagicError};

use super::{EvaluationContext, RecursionGuard, RuleMatch, offset, operators, types};
use log::{debug, warn};
// Gated to debug builds: after the engine module split, mod.rs's only atomic
// user is the `#[cfg(debug_assertions)]` INDIRECT_WITHOUT_RULE_ENV_WARNED guard
// below. In release builds that item is compiled out, so an ungated import is
// unused and the workspace `warnings = "deny"` lint rejects it.
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicBool, Ordering};

mod output;
mod pattern_recovery;
mod subroutine;
mod value_eval;

pub(crate) use output::*;
pub(crate) use pattern_recovery::*;
pub(crate) use subroutine::*;
pub(crate) use value_eval::*;

/// RAII guard that saves the GNU `file` previous-match anchor **and**
/// `base_offset` on entry and restores both on drop.
///
/// `MetaType::Indirect` re-evaluates the root rule list at the resolved
/// offset. The re-entered rules are top-level-semantic (`base_offset=0`)
/// and must start with a fresh anchor (the resolved indirect offset).
/// When `indirect` fires inside a `MetaType::Use` subroutine, the outer
/// subroutine's non-zero `base_offset` would otherwise leak into the
/// root re-entry, causing every positive absolute offset in the re-entered
/// database to be biased by the outer use-site -- producing reads at the
/// wrong positions. Saving and restoring `base_offset` here prevents that.
///
/// Without an RAII wrapper, every early-return path inside the indirect
/// branch would have to remember to restore both fields manually.
struct AnchorScope<'a> {
    context: &'a mut EvaluationContext,
    saved_anchor: usize,
    saved_base: usize,
}

impl<'a> AnchorScope<'a> {
    /// Save the current anchor and `base_offset`, then seed the context
    /// with `new_anchor` and reset `base_offset` to 0.
    fn enter(context: &'a mut EvaluationContext, new_anchor: usize) -> Self {
        let saved_anchor = context.last_match_end();
        let saved_base = context.base_offset();
        context.set_last_match_end(new_anchor);
        context.set_base_offset(0);
        Self {
            context,
            saved_anchor,
            saved_base,
        }
    }

    /// Access the underlying context for the duration of the guard.
    fn context(&mut self) -> &mut EvaluationContext {
        self.context
    }
}

impl Drop for AnchorScope<'_> {
    fn drop(&mut self) {
        self.context.set_last_match_end(self.saved_anchor);
        self.context.set_base_offset(self.saved_base);
    }
}

/// Process-local once guard for the "`evaluate_rules_with_config` called
/// with an `indirect` rule but without a `RuleEnvironment`" warning.
/// Same rationale as `USE_WITHOUT_RULE_ENV_WARNED`: surface the
/// misconfiguration exactly once per process so a large corpus of
/// env-less `indirect` rules does not flood the log.
// Gated to debug builds like its only use site (the diagnostic guard in
// `evaluate_rules_with_config`); in release builds the item would be dead
// code, which the workspace `warnings = "deny"` lint rejects.
#[cfg(debug_assertions)]
static INDIRECT_WITHOUT_RULE_ENV_WARNED: AtomicBool = AtomicBool::new(false);

/// Evaluate a single magic rule against a file buffer
///
/// This is a thin wrapper around [`evaluate_rules`] that evaluates exactly
/// one top-level rule (and any of its children) against a buffer, using the
/// caller-provided [`EvaluationContext`] to enforce timeout, recursion, and
/// string-size limits. It is a BREAKING API change introduced in pre-1.0:
/// earlier versions took no context and returned `Option<(usize, Value)>`.
///
/// # Arguments
///
/// * `rule` - The magic rule to evaluate
/// * `buffer` - The file buffer to evaluate against
/// * `context` - Mutable evaluation context that carries the configured
///   safety limits (timeout, max recursion depth, max string length) and
///   the GNU `file` previous-match anchor used for relative-offset
///   resolution. Callers reusing a context across multiple buffers must
///   call [`EvaluationContext::reset`](crate::evaluator::EvaluationContext::reset)
///   between calls -- see [`evaluate_rules`] for details.
///
/// # Returns
///
/// Returns `Ok(Vec<RuleMatch>)` containing the parent match (if the rule
/// matched) plus any child matches collected recursively. An empty vector
/// means the rule did not match or was skipped due to a data-dependent
/// evaluation error (buffer overrun, invalid offset, etc.). Only critical
/// failures such as `LibmagicError::Timeout` or recursion-limit exhaustion
/// are returned as `Err`.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::{evaluate_single_rule, EvaluationContext};
/// use libmagic_rs::EvaluationConfig;
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
///
/// // Create a rule to check for ELF magic bytes at offset 0
/// let rule = MagicRule::new(OffsetSpec::Absolute(0), TypeKind::Byte { signed: true }, Operator::Equal, Value::Uint(0x7f), "ELF magic".to_string());
///
/// let mut context = EvaluationContext::new(EvaluationConfig::default());
/// let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
/// let matches = evaluate_single_rule(&rule, elf_buffer, &mut context).unwrap();
/// assert_eq!(matches.len(), 1); // Should match
///
/// context.reset();
/// let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
/// let matches = evaluate_single_rule(&rule, non_elf_buffer, &mut context).unwrap();
/// assert!(matches.is_empty()); // Should not match
/// ```
///
/// # Errors
///
/// * `LibmagicError::Timeout` - If evaluation exceeds the configured timeout
/// * `LibmagicError::EvaluationError` - For critical failures such as the
///   recursion limit being exceeded. Data-dependent errors (buffer overrun,
///   invalid offset, malformed pstring length) are handled gracefully by
///   [`evaluate_rules`] and surface as an empty match vector rather than
///   an error.
pub fn evaluate_single_rule(
    rule: &MagicRule,
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<Vec<RuleMatch>, LibmagicError> {
    evaluate_rules(std::slice::from_ref(rule), buffer, context)
}

/// Internal: evaluate a single rule against a buffer, supplying an explicit
/// anchor for relative-offset resolution.
///
/// This is the worker behind both [`evaluate_single_rule`] (which defaults
/// the anchor to 0) and [`evaluate_rules`] (which threads the anchor from
/// `EvaluationContext::last_match_end()`).
fn evaluate_single_rule_with_anchor(
    rule: &MagicRule,
    buffer: &[u8],
    last_match_end: usize,
    base_offset: usize,
    max_string_length: usize,
    flip_endian: bool,
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    use crate::parser::ast::TypeKind;

    // Step 1: Resolve the offset specification to an absolute position.
    // `base_offset` is non-zero only inside a `MetaType::Use` subroutine
    // body, where it biases positive absolute offsets to the use-site.
    let absolute_offset =
        offset::resolve_offset_with_base(&rule.offset, buffer, last_match_end, base_offset)?;

    // Step 2 & 3: Dispatch on type category. Pattern-bearing types
    // (Regex, Search) take a different path from fixed-width types
    // because the rule's `value` operand is the *pattern*, not an
    // expected matched value. Running those through `apply_operator`
    // would compare matched text ("123") against the pattern literal
    // ("[0-9]+") and produce false negatives on any regex with
    // metacharacters.
    //
    // Meta-type directives (`default`, `clear`, `name`, `use`,
    // `indirect`, `offset`) are dispatched by `evaluate_rules` at the
    // outer loop level (not here) -- this single-rule helper is only
    // invoked for non-meta rules. Short-circuiting the Meta arms here
    // with `Ok(None)` is defense-in-depth for programmatic callers
    // (property tests, fuzz harnesses) that hand-build a Meta rule
    // and feed it directly to `evaluate_single_rule`; without the
    // guard, the value/pattern paths would surface
    // `TypeReadError::UnsupportedType`.
    let (matched, read_value) = match &rule.typ {
        TypeKind::Meta(MetaType::Name(name)) => {
            // `Name` rules are normally hoisted into the name table at
            // parse time and should not reach the evaluator. Programmatic
            // consumers (e.g. fuzz harnesses, property tests) can still
            // construct them directly; treat that as a no-op rather than
            // a hard failure so the evaluator-never-panics invariant is
            // preserved.
            debug!(
                "Name rule '{name}' reached evaluator (likely bypassed name-table extraction); treating as no-op"
            );
            return Ok(None);
        }
        TypeKind::Meta(MetaType::Use { .. }) => {
            // `Use` is dispatched inline by `evaluate_rules` so it can
            // push the subroutine's matches into the caller's match
            // vector. Reaching this arm means the rule went through the
            // single-rule path (e.g. via `evaluate_single_rule`) which
            // lacks that wiring; treat it as a silent no-op.
            return Ok(None);
        }
        TypeKind::Meta(_) => return Ok(None),
        TypeKind::Regex { .. } | TypeKind::Search { .. } => {
            evaluate_pattern_rule(rule, buffer, absolute_offset, max_string_length)?
        }
        // Flagged `string` rules route through the pattern-bearing path
        // (see GOTCHAS S2.4 for the contract) so `compare_string_with_flags`
        // can do the case-fold / whitespace-flexible match in one pass --
        // but ONLY for the equality operators the pattern path supports.
        // An ORDERING operator on a flagged string (e.g. the ubiquitous
        // `string/t >\0` / `string/b >\0` "non-empty text here, print it with
        // %s" idiom in `varied.script`, `sgml`, `linux`, ...) is a
        // lexicographic comparison, not a pattern match; routing it to the
        // pattern path made it a fatal `UnsupportedType` abort that killed the
        // whole file's evaluation. The `/t`/`/b` flags are MIME-output hints
        // with no comparison effect, so such a rule behaves like an unflagged
        // `string >VALUE` and belongs on the value path. Default-flag strings
        // (the common case) also take that value-rule fast path.
        TypeKind::String { flags, .. }
            if !flags.is_empty()
                && matches!(
                    rule.op,
                    crate::parser::ast::Operator::Equal | crate::parser::ast::Operator::NotEqual
                ) =>
        {
            evaluate_pattern_rule(rule, buffer, absolute_offset, max_string_length)?
        }
        _ => evaluate_value_rule(
            rule,
            buffer,
            absolute_offset,
            max_string_length,
            flip_endian,
        )?,
    };
    Ok(matched.then_some((absolute_offset, read_value)))
}

/// Evaluate a list of magic rules against a file buffer with hierarchical processing
///
/// This function implements the core hierarchical rule evaluation algorithm with graceful
/// error handling:
/// 1. Evaluates each top-level rule in sequence
/// 2. If a parent rule matches, evaluates its child rules for refinement
/// 3. Collects all matches or stops at first match based on configuration
/// 4. Maintains evaluation context for recursion limits and state
/// 5. Implements graceful degradation by skipping problematic rules and continuing evaluation
///
/// The hierarchical evaluation follows these principles:
/// - Parent rules must match before children are evaluated
/// - Child rules provide refinement and additional detail
/// - Evaluation can stop at first match or continue for all matches
/// - Recursion depth is limited to prevent infinite loops
/// - Problematic rules are skipped to allow evaluation to continue
///
/// # Arguments
///
/// * `rules` - The list of magic rules to evaluate
/// * `buffer` - The file buffer to evaluate against
/// * `context` - Mutable evaluation context for state management. **Callers
///   reusing a context across multiple buffers must call
///   [`EvaluationContext::reset`](crate::evaluator::EvaluationContext::reset)
///   between calls** -- the GNU `file` previous-match anchor and the
///   recursion-depth counter both advance during evaluation and would
///   otherwise leak across buffers. The same applies when this function
///   returns `Err` mid-evaluation (e.g., `LibmagicError::Timeout` or
///   `RecursionLimitExceeded`): both the anchor and (potentially) the
///   recursion depth are left in a partially-advanced state, and a retry
///   on the same context without `reset()` will resolve relative offsets
///   against the stale anchor and apply the wrong recursion budget.
///   [`evaluate_rules_with_config`] always builds a fresh context and is the
///   safer choice when context reuse isn't required.
///
/// # Returns
///
/// Returns `Ok(Vec<RuleMatch>)` containing all matches found. Errors in individual rules
/// are skipped to allow evaluation to continue. Only returns `Err(LibmagicError)`
/// for critical failures like timeout or recursion limit exceeded.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::{evaluate_rules, EvaluationContext, RuleMatch};
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::EvaluationConfig;
///
/// // Create a hierarchical rule set for ELF files
/// let parent_rule = MagicRule::new(
///     OffsetSpec::Absolute(0),
///     TypeKind::Byte { signed: true },
///     Operator::Equal,
///     Value::Uint(0x7f),
///     "ELF".to_string(),
/// )
/// .with_children(vec![
///     MagicRule::new(
///         OffsetSpec::Absolute(4),
///         TypeKind::Byte { signed: true },
///         Operator::Equal,
///         Value::Uint(2),
///         "64-bit".to_string(),
///     )
///     .with_level(1),
/// ]);
///
/// let rules = vec![parent_rule];
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
/// let config = EvaluationConfig::default();
/// let mut context = EvaluationContext::new(config);
///
/// let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
/// assert_eq!(matches.len(), 2); // Parent and child should both match
/// ```
///
/// # Errors
///
/// * `LibmagicError::Timeout` - If evaluation exceeds configured timeout
/// * `LibmagicError::EvaluationError` - Only for critical failures like recursion limit exceeded
///
/// Individual rule evaluation errors are handled gracefully and do not stop the overall evaluation.
#[allow(clippy::too_many_lines)]
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<Vec<RuleMatch>, LibmagicError> {
    let mut matches = Vec::with_capacity(8);
    let start_time = std::time::Instant::now();
    let mut rule_count = 0u32;

    // Per-level "did any sibling match yet?" flag for `default`/`clear`
    // dispatch. Each recursive descent gets its own fresh flag, so child
    // sibling chains track their own state independently of the parent.
    let mut sibling_matched = false;

    // Per-level entry anchor: captured at the start of this sibling list's
    // evaluation. For CHILD sibling lists (recursion_depth > 0), the
    // GNU `file`/libmagic previous-match anchor is reset to this value
    // between sibling iterations so that `&N` offsets on continuation
    // siblings resolve against the parent-level anchor, not against
    // whatever the *previous sibling* left the anchor at. This matches
    // libmagic's continuation-level model (`ms->c.li[cont_level]`)
    // where each level tracks its own anchor; a sibling at level L does
    // not inherit the post-match anchor of another sibling at level L.
    //
    // TOP-LEVEL siblings (recursion_depth == 0) are independent
    // classification attempts -- each top-level rule intentionally sees
    // the anchor advance that prior top-level rules produced (see
    // GOTCHAS S3.8 and the `relative_anchor_can_decrease_...`
    // integration test). Gate the reset on recursion_depth to preserve
    // that documented discipline while still fixing the continuation-
    // sibling behavior that the GNU `file` `searchbug.magic` fixture
    // relies on.
    //
    // Recursing into a matched rule's children still carries forward the
    // post-match anchor (via the current value of `last_match_end()` at
    // the point of recursion), so child sibling lists see their parent's
    // resolved position as their own entry anchor.
    //
    // INDIRECT RE-ENTRY exception: `MetaType::Indirect` dispatches its
    // sub-evaluation via `RecursionGuard::enter` (to bound the recursion
    // cycle), which forces `recursion_depth > 0`. But an indirect
    // re-entry semantically evaluates the root rule list with TOP-LEVEL
    // sibling semantics -- each rule is an independent classification
    // attempt against the re-entered sub-buffer, NOT a continuation
    // list. The indirect dispatch sets `context.set_indirect_reentry(true)`
    // just before this call; `take_indirect_reentry()` consumes it at
    // entry so only this iteration treats siblings as top-level.
    // Children of matched rules inside the re-entry still see the flag
    // as false (consumed) and correctly fall back to continuation
    // semantics via `recursion_depth > 0`.
    let entry_anchor = context.last_match_end();
    let is_indirect_reentry = context.take_indirect_reentry();
    let is_child_sibling_list = context.recursion_depth() > 0 && !is_indirect_reentry;

    // `stop_at_first_match` is a TOP-LEVEL classification concept (see the
    // `EvaluationConfig::stop_at_first_match` doc): once an outermost rule --
    // or an indirect re-entry, which is itself a fresh top-level
    // classification -- produces a message-bearing match, we stop trying
    // other top-level candidates. It must NOT short-circuit a child /
    // continuation sibling list or a `use` subroutine body: every matching
    // sibling there contributes a detail fragment to the description (e.g.
    // gzip's "max compression", "from Unix", "original size modulo 2^32 N"),
    // and truncating them silently drops multi-part descriptions. This
    // mirrors libmagic, where continuation levels always evaluate every
    // sibling and only the top-level `match()` loop stops at first success.
    // (An earlier revision applied the break at every recursion level, which
    // violated the documented top-level-only contract and truncated gzip's
    // trailing detail after its first message-bearing child.)
    let stop_at_first_match_applies = !is_child_sibling_list;

    // Entry-point timeout check: ensures every recursive descent is bounded
    // and that evaluations of small rule sets (< 16 rules) are still guarded.
    // Without this, the periodic every-16-rules check below never fires for
    // flat rule lists with fewer than 16 rules, and recursion into children
    // also restarts `rule_count` at 0.
    if let Some(timeout_ms) = context.timeout_ms()
        && start_time.elapsed().as_millis() >= u128::from(timeout_ms)
    {
        return Err(LibmagicError::Timeout { timeout_ms });
    }

    for rule in rules {
        // For continuation siblings (child recursion), reset the
        // previous-match anchor to the entry anchor so `&N` offsets
        // resolve against the parent-level position. Top-level
        // siblings (depth 0) keep the chaining behavior documented in
        // GOTCHAS S3.8. See the `entry_anchor` comment above.
        if is_child_sibling_list {
            context.set_last_match_end(entry_anchor);
        }

        // Check timeout periodically (every 16 rules) to reduce syscall overhead
        rule_count = rule_count.wrapping_add(1);
        if rule_count.trailing_zeros() >= 4
            && let Some(timeout_ms) = context.timeout_ms()
            && start_time.elapsed().as_millis() >= u128::from(timeout_ms)
        {
            return Err(LibmagicError::Timeout { timeout_ms });
        }

        // `Clear` resets the per-level "sibling matched" flag so a
        // subsequent `default` sibling can fire even if an earlier
        // sibling matched. Matching libmagic's `FILE_CLEAR`, the flag is
        // unconditionally reset and NEVER re-set to `true` afterward
        // (clear does not participate in the "a sibling matched" chain).
        //
        // libmagic's `FILE_CLEAR` also COUNTS as a match -- its `x` test
        // always succeeds -- and `mprint` renders its description when it
        // is non-empty. So a `clear` carrying message text must emit that
        // text (c-lang's `>>&0 clear x program text` is the only such rule
        // in the system DB, producing the "program text" fragment of
        // `c program text`). Verified against real `file` (file-5.41):
        // a message-bearing `clear` child prints its message AND still
        // resets the flag so a trailing `default` sibling fires.
        //
        // Emission is guarded on a non-empty message so the many bare
        // `clear x` flag-reset directives throughout the system DB (apple,
        // coff, elf, pmem, ...) behave exactly as before -- no match, no
        // anchor advance. `clear` is 0-width, so the previous-match anchor
        // is intentionally not advanced in either case. Children are
        // evaluated for a message-bearing clear for libmagic fidelity;
        // `evaluate_children_or_warn` is a no-op when there are none.
        if let TypeKind::Meta(MetaType::Clear) = &rule.typ {
            sibling_matched = false;

            if !rule.message.is_empty() {
                let matches_before = matches.len();

                let match_result = RuleMatch::new(
                    rule.message.clone(),
                    context.last_match_end(),
                    rule.level,
                    crate::parser::ast::Value::Uint(0),
                    rule.typ.clone(),
                    RuleMatch::calculate_confidence(rule.level),
                );
                matches.push(match_result);

                evaluate_children_or_warn(rule, "clear", buffer, context, &mut matches)?;

                if stop_at_first_match_applies
                    && matches.len() > matches_before
                    && context.should_stop_at_first_match()
                    && has_message_bearing_match(&matches, matches_before)
                {
                    break;
                }
            }
            continue;
        }

        // `Default` fires only when no earlier sibling at this level has
        // matched yet. The anchor is intentionally not advanced -- the
        // directive does not consume bytes -- but its children are
        // evaluated and the per-level "sibling matched" flag is set so
        // any later `default` sibling at the same level is suppressed.
        if let TypeKind::Meta(MetaType::Default) = &rule.typ {
            if !sibling_matched {
                let matches_before = matches.len();

                let match_result = RuleMatch::new(
                    rule.message.clone(),
                    context.last_match_end(),
                    rule.level,
                    crate::parser::ast::Value::Uint(0),
                    rule.typ.clone(),
                    RuleMatch::calculate_confidence(rule.level),
                );
                matches.push(match_result);

                // `default` is treated as a successful match at this
                // level, so its children are evaluated under the same
                // recursion-guard pattern as every other successful rule.
                evaluate_children_or_warn(rule, "default", buffer, context, &mut matches)?;

                sibling_matched = true;

                if stop_at_first_match_applies
                    && matches.len() > matches_before
                    && context.should_stop_at_first_match()
                    && has_message_bearing_match(&matches, matches_before)
                {
                    break;
                }
            }
            continue;
        }

        // `Indirect` re-evaluates the root rule list at the resolved
        // offset, mirroring libmagic's indirect-type semantics. The
        // sub-evaluation runs against `buffer[absolute_offset..]` with a
        // fresh anchor (0) so relative offsets inside the root rules
        // resolve correctly; the caller's anchor is restored on exit
        // via `AnchorScope`. Without an attached `RuleEnvironment`
        // (programmatic consumers bypassing `MagicDatabase`) the
        // directive is a silent no-op.
        if let TypeKind::Meta(MetaType::Indirect) = &rule.typ {
            // Resolve the offset first so a malformed offset surfaces
            // as a graceful skip rather than a hard error.
            let absolute_offset = match offset::resolve_offset_with_base(
                &rule.offset,
                buffer,
                context.last_match_end(),
                context.base_offset(),
            ) {
                Ok(o) => o,
                Err(
                    e @ LibmagicError::EvaluationError(
                        crate::error::EvaluationError::BufferOverrun { .. }
                        | crate::error::EvaluationError::InvalidOffset { .. },
                    ),
                ) => {
                    debug!("Skipping indirect rule '{}': {}", rule.message, e);
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Pull the root rules out of the rule environment. Without
            // an environment there is nothing to re-enter, so this is a
            // silent no-op (matching the `Use`-without-env behavior).
            //
            // We use `debug!` rather than `debug_assert!` here because
            // property tests (`prop_arbitrary_rule_evaluation_never_panics`)
            // synthesize arbitrary `TypeKind::Meta(MetaType::Indirect)`
            // rules and run them without attaching a `RuleEnvironment`;
            // a panic on this path would break the never-panics invariant.
            // See GOTCHAS S2.1 for the same rationale on the leaked-Name arm.
            let Some(root_rules) = context
                .rule_env()
                .map(|e| std::sync::Arc::clone(&e.root_rules))
            else {
                debug!(
                    "indirect rule '{}' evaluated without a rule environment; treating as no-op",
                    rule.message
                );
                continue;
            };

            // Bounds-check before slicing. An indirect offset past the
            // end of the buffer is a data-dependent skip, not an error.
            let Some(sub_buffer) = buffer.get(absolute_offset..) else {
                debug!(
                    "Skipping indirect rule '{}': offset {} past buffer end ({} bytes)",
                    rule.message,
                    absolute_offset,
                    buffer.len()
                );
                continue;
            };

            let matches_before = matches.len();

            // Advance the GNU `file` previous-match anchor to the indirect's
            // resolved offset and emit a `RuleMatch` for the indirect rule
            // itself BEFORE descending into the root re-entry or children.
            // This matches the shared successful-match flow used by every
            // other rule kind: advance anchor first, record the match, then
            // recurse. Without this, sibling rules of the `indirect` resolve
            // their relative offsets against the stale anchor and the
            // directive's own `message` never surfaces in the output.
            context.set_last_match_end(absolute_offset);

            let indirect_match = RuleMatch::new(
                rule.message.clone(),
                absolute_offset,
                rule.level,
                crate::parser::ast::Value::String("indirect".to_string()),
                rule.typ.clone(),
                RuleMatch::calculate_confidence(rule.level),
            );
            matches.push(indirect_match);

            // Indirect counts as a match for `sibling_matched` regardless of
            // whether the sub-evaluation produced any matches -- the directive
            // itself successfully dispatched.
            sibling_matched = true;

            // Recursion guard + anchor scope: nested indirect / use cycles
            // surface as `RecursionLimitExceeded` instead of a stack overflow,
            // and the caller's anchor is restored on every exit path.
            //
            // Mark the upcoming `evaluate_rules` call as a top-level
            // re-entry (consumed at entry) so sibling anchor-reset
            // semantics do NOT fire -- root rules in the re-entered
            // database chain their anchors across siblings like any
            // other top-level evaluation.
            {
                let mut guard = RecursionGuard::enter(context)?;
                let mut anchor_scope = AnchorScope::enter(guard.context(), 0);
                anchor_scope.context().set_indirect_reentry(true);
                match evaluate_rules(&root_rules, sub_buffer, anchor_scope.context()) {
                    Ok(sub_matches) => {
                        matches.extend(sub_matches);
                    }
                    Err(LibmagicError::Timeout { timeout_ms }) => {
                        return Err(LibmagicError::Timeout { timeout_ms });
                    }
                    Err(e) => return Err(e),
                }
                // anchor_scope drops here, restoring the saved anchor
                // (which is now `absolute_offset`, set above before the
                // scope was entered).
                // guard drops next, decrementing the recursion depth.
            }

            // Evaluate the indirect rule's own children under the same
            // recursion-guard pattern used by every other successful rule.
            evaluate_children_or_warn(rule, "indirect", buffer, context, &mut matches)?;

            if stop_at_first_match_applies
                && matches.len() > matches_before
                && context.should_stop_at_first_match()
                && has_message_bearing_match(&matches, matches_before)
            {
                break;
            }
            continue;
        }

        // `Offset` reports the resolved file offset as the rule's read
        // value, matching GNU `file`'s `FILE_OFFSET` semantics: the match
        // emits a value-bearing `RuleMatch` whose `value` is the absolute
        // position, which downstream message formatting substitutes into
        // `%lld` / `%d` specifiers via `output::format::format_magic_message`.
        //
        // Per magic(5) the only legal operator is `x` (AnyValue); any
        // other operator is a magic-file semantic error. Matching the
        // evaluator's graceful-skip discipline, we `debug!`-log and skip
        // rather than erroring -- a rogue rule shouldn't poison the rest
        // of the evaluation.
        if let TypeKind::Meta(MetaType::Offset) = &rule.typ {
            // Resolve the offset first so a malformed offset surfaces as
            // a graceful skip rather than a hard error. Mirrors the
            // `Indirect` dispatch above.
            let absolute_offset = match offset::resolve_offset_with_base(
                &rule.offset,
                buffer,
                context.last_match_end(),
                context.base_offset(),
            ) {
                Ok(o) => o,
                Err(
                    e @ LibmagicError::EvaluationError(
                        crate::error::EvaluationError::BufferOverrun { .. }
                        | crate::error::EvaluationError::InvalidOffset { .. },
                    ),
                ) => {
                    debug!("Skipping offset rule '{}': {}", rule.message, e);
                    continue;
                }
                Err(e) => return Err(e),
            };

            // The magic(5) `offset` pseudo-type treats the resolved offset
            // itself as the read value. `offset x` is a bare AnyValue
            // placeholder that always matches (used purely to report the
            // position via `%lld`). A comparison operator (`offset >48`,
            // `offset <48`, `offset =N`, ...) tests the resolved offset
            // against the operand -- e.g. gzip's `>>-0 offset >48` gates
            // the trailing "original size modulo 2^32" trailer on the file
            // being long enough to carry it, and its `>>-0 offset <48`
            // sibling reports "truncated" otherwise. Skip the rule (a
            // non-match) when the comparison fails so the false branch and
            // its children do not render.
            let offset_value = crate::parser::ast::Value::Uint(absolute_offset as u64);
            let offset_matched = match &rule.op {
                crate::parser::ast::Operator::AnyValue => true,
                op => operators::apply_operator(op, &offset_value, &rule.value),
            };
            if !offset_matched {
                continue;
            }

            let matches_before = matches.len();

            // Advance the anchor BEFORE emitting the match so sibling
            // rules resolve their relative offsets against the offset
            // directive's resolved position. Same discipline as
            // `Indirect` and every other value-bearing rule.
            context.set_last_match_end(absolute_offset);

            let offset_match = RuleMatch::new(
                rule.message.clone(),
                absolute_offset,
                rule.level,
                offset_value,
                rule.typ.clone(),
                RuleMatch::calculate_confidence(rule.level),
            );
            matches.push(offset_match);

            sibling_matched = true;

            // Evaluate children under the recursion-guard pattern used
            // by every other successful rule.
            evaluate_children_or_warn(rule, "offset", buffer, context, &mut matches)?;

            if stop_at_first_match_applies
                && matches.len() > matches_before
                && context.should_stop_at_first_match()
                && has_message_bearing_match(&matches, matches_before)
            {
                break;
            }
            continue;
        }

        // `Use` is handled inline so the subroutine's matches can be
        // spliced into the caller's match vector in document order.
        // Routing this through `evaluate_single_rule_with_anchor` would
        // force the helper to return a `Vec<RuleMatch>`, which would
        // reshape the single-rule return type for every other variant.
        //
        // On a successful use path we must also descend into the rule's
        // own children, matching the flow of every other successful rule
        // kind. libmagic chains like `>>0 use part2` often carry
        // continuation rules (siblings and descendants of the `use` site)
        // that depend on the anchor the subroutine left behind; skipping
        // them produces user-visible false negatives.
        if let TypeKind::Meta(MetaType::Use { name, flip_endian }) = &rule.typ {
            let matches_before = matches.len();
            let use_resolved = match evaluate_use_rule(rule, name, *flip_endian, buffer, context) {
                Ok((Some(terminal_anchor), subroutine_matches)) => {
                    matches.extend(subroutine_matches);

                    // A `use` rule does not produce a surface
                    // `RuleMatch` itself -- the subroutine's rules
                    // carry the visible messages. Advance the
                    // caller's anchor to the subroutine's TERMINAL
                    // anchor (where the subroutine left `last_match_end`),
                    // not the use-site offset. This makes `use`
                    // behave like inlining the subroutine: sibling
                    // rules after the `use` see `&N` resolve against
                    // the subroutine's final match position.
                    context.set_last_match_end(terminal_anchor);
                    true
                }
                Ok((None, _)) => {
                    // No environment, or name not found -- silent no-op.
                    false
                }
                Err(
                    e @ LibmagicError::EvaluationError(
                        crate::error::EvaluationError::BufferOverrun { .. }
                        | crate::error::EvaluationError::InvalidOffset { .. },
                    ),
                ) => {
                    debug!("Skipping use rule '{name}': {e}");
                    false
                }
                Err(e) => return Err(e),
            };

            // Evaluate the use rule's own children exactly like any other
            // successful rule. Subroutine matches are already appended
            // above, so children are spliced in after them to preserve
            // document order. The recursion guard mirrors the non-`Use`
            // path so a `use`-site chain cannot blow past the configured
            // recursion limit.
            if use_resolved {
                evaluate_children_or_warn(rule, "use", buffer, context, &mut matches)?;
            }

            // A successful `use` site is treated as a sibling match for
            // `default`/`clear` dispatch purposes -- subsequent `default`
            // siblings should not fire if the subroutine resolved.
            if use_resolved {
                sibling_matched = true;
            }

            // Apply stop-at-first-match with the same semantics as every
            // other successful rule kind: if this `use` site contributed
            // any matches (either from the subroutine or from its own
            // children) and the caller configured first-match
            // short-circuiting, halt evaluation of further siblings --
            // but only once one of those matches actually carries usable
            // description text (see `has_message_bearing_match`).
            if stop_at_first_match_applies
                && matches.len() > matches_before
                && context.should_stop_at_first_match()
                && has_message_bearing_match(&matches, matches_before)
            {
                break;
            }
            continue;
        }

        // Evaluate the current rule with graceful error handling.
        // Pass the GNU `file` anchor so OffsetSpec::Relative resolves
        // correctly against the previous match's end position.
        let match_data = match evaluate_single_rule_with_anchor(
            rule,
            buffer,
            context.last_match_end(),
            context.base_offset(),
            context.max_string_length(),
            context.flip_endian(),
        ) {
            Ok(data) => data,
            Err(
                e @ (LibmagicError::EvaluationError(
                    crate::error::EvaluationError::BufferOverrun { .. }
                    | crate::error::EvaluationError::InvalidOffset { .. }
                    | crate::error::EvaluationError::InvalidValueTransform { .. }
                    | crate::error::EvaluationError::TypeReadError(
                        crate::evaluator::types::TypeReadError::BufferOverrun { .. }
                        | crate::evaluator::types::TypeReadError::InvalidPStringLength { .. },
                    ),
                )
                | LibmagicError::IoError(_)),
            ) => {
                // Expected data-dependent evaluation errors -- skip gracefully.
                // TypeReadError::UnsupportedType is intentionally NOT caught
                // here (except the narrow exception in the arm immediately
                // below) so that evaluator capability gaps propagate as
                // errors.
                debug!("Skipping rule '{}': {}", rule.message, e);
                continue;
            }
            // Narrow graceful-skip (KTD4, fix-system-magic-regex-graceful
            // plan; variant-keyed since issue #391 item 2): a pattern-bearing
            // type (`Regex`/`Search`/flagged `String`) evaluated without a
            // usable `String`/`Bytes` pattern operand, or a regex compile
            // failure (including the `REGEX_COMPILE_SIZE_LIMIT` CWE-1333 DoS
            // guard), must not abort the whole file's evaluation (R1/R2).
            // The skip is keyed on the dedicated `MissingPatternOperand` /
            // `RegexCompileError` variants via `TypeReadError::is_pattern_skip`
            // -- NOT on `UnsupportedType`, so any genuine capability gap
            // (an unwired `TypeKind` variant, a non-Equal/NotEqual operator
            // on a pattern-bearing type, a `Meta` read as a value) stays an
            // `UnsupportedType` and falls through to the catch-all below and
            // propagates (R3). See `log_pattern_operand_skip` for the
            // debug!/warn! split.
            Err(LibmagicError::EvaluationError(crate::error::EvaluationError::TypeReadError(
                ref tre,
            ))) if tre.is_pattern_skip() => {
                log_pattern_operand_skip("top-level", &rule.message, tre);
                continue;
            }
            Err(e) => {
                // Unexpected errors (InternalError, other UnsupportedType
                // conditions, etc.) should propagate.
                return Err(e);
            }
        };

        if let Some((absolute_offset, read_value)) = match_data {
            let matches_before = matches.len();

            // Advance the GNU `file` previous-match anchor BEFORE recursing
            // into children, so children and their descendants see the new
            // anchor. The anchor is updated unconditionally to the end of
            // this match -- it may move forward or backward depending on
            // where successive rules match (it is *not* a high-watermark).
            let consumed = types::bytes_consumed_with_pattern(
                buffer,
                absolute_offset,
                &rule.typ,
                Some(&rule.value),
            );
            let new_anchor = absolute_offset.saturating_add(consumed);
            context.set_last_match_end(new_anchor);

            // Mark this level as "matched" so any subsequent `default`
            // sibling at the same level is suppressed, matching libmagic's
            // default-after-match semantics.
            sibling_matched = true;

            let match_result = RuleMatch::new(
                rule.message.clone(),
                absolute_offset,
                rule.level,
                read_value,
                rule.typ.clone(),
                RuleMatch::calculate_confidence(rule.level),
            );
            matches.push(match_result);

            // If this rule has children, evaluate them recursively
            if !rule.children.is_empty() {
                // Check recursion depth limit - this is a critical error that should stop evaluation.
                // `RecursionGuard` decrements the depth on drop, so every exit path below
                // (Ok, graceful warn!, or early-return via `?`) restores the counter.
                let mut guard = RecursionGuard::enter(context)?;

                // Recursively evaluate child rules with graceful error handling
                match evaluate_rules(&rule.children, buffer, guard.context()) {
                    Ok(child_matches) => {
                        matches.extend(child_matches);
                    }
                    Err(LibmagicError::Timeout { timeout_ms }) => {
                        // Timeout is critical, propagate it up (guard drops here).
                        return Err(LibmagicError::Timeout { timeout_ms });
                    }
                    Err(
                        e @ (LibmagicError::EvaluationError(
                            crate::error::EvaluationError::BufferOverrun { .. }
                            | crate::error::EvaluationError::InvalidOffset { .. }
                            | crate::error::EvaluationError::InvalidValueTransform { .. }
                            | crate::error::EvaluationError::TypeReadError(
                                crate::evaluator::types::TypeReadError::BufferOverrun { .. }
                                | crate::evaluator::types::TypeReadError::InvalidPStringLength {
                                    ..
                                },
                            ),
                        )
                        | LibmagicError::IoError(_)),
                    ) => {
                        // Defensive: under the current implementation, individual child
                        // failures are caught and logged inside the recursive evaluate_rules
                        // call (they never propagate here). This arm guards against future
                        // changes that might alter that error-handling strategy.
                        //
                        // If this fires, the parent match is still emitted but the entire
                        // child subtree is silently dropped -- which means a partial,
                        // possibly-incorrect classification is returned to the caller.
                        // Logged at warn! (not debug!) so the asymmetry is visible.
                        warn!(
                            "Discarding child evaluation under rule '{}' due to unexpected error: {} -- parent match is still emitted; investigate the recursive evaluate_rules error-handling path",
                            rule.message, e
                        );
                    }
                    // Narrow graceful-skip (KTD4): same allowlist as the
                    // top-level dispatch match above and
                    // `evaluate_children_or_warn` -- a pattern-bearing type
                    // evaluated without a usable pattern operand, or a
                    // regex compile failure, must not abort the parent's
                    // match. Defensive: individual child failures are
                    // already caught inside the recursive `evaluate_rules`
                    // call and never reach here under the current
                    // implementation; this arm guards against a future
                    // change to that strategy.
                    Err(LibmagicError::EvaluationError(
                        crate::error::EvaluationError::TypeReadError(ref tre),
                    )) if tre.is_pattern_skip() => {
                        log_pattern_operand_skip("child", &rule.message, tre);
                    }
                    Err(e) => {
                        // Unexpected errors in children (including RecursionLimitExceeded)
                        // should propagate. The guard drops here, decrementing the depth.
                        return Err(e);
                    }
                }
                // `guard` drops here, decrementing the recursion depth.
            }

            // Stop at first match if configured to do so -- but only once
            // this rule (or one of its descendants) actually contributed
            // usable description text. A message-less match (e.g. a
            // gating rule used purely to trigger a child) must not shadow
            // a later, more specific top-level rule that would otherwise
            // produce real output (GOTCHAS S13.2).
            if stop_at_first_match_applies
                && context.should_stop_at_first_match()
                && has_message_bearing_match(&matches, matches_before)
            {
                break;
            }
        }
    }

    Ok(matches)
}

/// Evaluate magic rules with a fresh context
///
/// This is a convenience function that creates a new evaluation context
/// and evaluates the rules. Useful for simple evaluation scenarios.
///
/// # Arguments
///
/// * `rules` - The list of magic rules to evaluate
/// * `buffer` - The file buffer to evaluate against
/// * `config` - Configuration for evaluation behavior
///
/// # Returns
///
/// Returns `Ok(Vec<RuleMatch>)` containing all matches found, or `Err(LibmagicError)`
/// if evaluation fails.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::{evaluate_rules_with_config, RuleMatch};
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::EvaluationConfig;
///
/// let rule = MagicRule::new(OffsetSpec::Absolute(0), TypeKind::Byte { signed: true }, Operator::Equal, Value::Uint(0x7f), "ELF magic".to_string());
///
/// let rules = vec![rule];
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46];
/// let config = EvaluationConfig::default();
///
/// let matches = evaluate_rules_with_config(&rules, buffer, &config).unwrap();
/// assert_eq!(matches.len(), 1);
/// assert_eq!(matches[0].message, "ELF magic");
/// ```
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If rule evaluation fails
/// * `LibmagicError::Timeout` - If evaluation exceeds configured timeout
pub fn evaluate_rules_with_config(
    rules: &[MagicRule],
    buffer: &[u8],
    config: &EvaluationConfig,
) -> Result<Vec<RuleMatch>, LibmagicError> {
    // Validate the configuration before constructing a context so that
    // out-of-range values (e.g. zero recursion depth, excessive timeouts)
    // are rejected at the API boundary rather than triggering subtle
    // failures during evaluation.
    config.validate()?;
    // Diagnostic guard: `evaluate_rules_with_config` builds a context
    // without an attached `RuleEnvironment`, which means any
    // `MetaType::Indirect` rule reached during evaluation is silently
    // no-op'd at runtime. That is the intentional behavior for low-level
    // callers (matching the `Use`-without-env contract), but we surface
    // the misconfiguration at `warn!` level (once per process) so a
    // consumer who wires up env-less `indirect` rules will see the
    // diagnostic in default logging rather than only at debug level.
    // The tree walk runs only in debug builds -- in release builds the
    // `cfg(debug_assertions)` gate prevents the O(n) scan on every
    // top-level evaluation. Using `debug_assert!` would panic in test
    // builds and break the "evaluator never panics" invariant documented
    // in GOTCHAS S2.4 -- a misconfigured caller should get a no-op with
    // a log entry, not a crash.
    #[cfg(debug_assertions)]
    if contains_indirect_rule(rules)
        && !INDIRECT_WITHOUT_RULE_ENV_WARNED.swap(true, Ordering::Relaxed)
    {
        warn!(
            "{} (subsequent occurrences suppressed)",
            crate::error::EvaluationError::indirect_without_environment()
        );
    }
    // Clear the thread-local regex compile cache so it is bounded to
    // the lifetime of a single top-level evaluation call. Cache
    // entries from a previous rule set would otherwise persist on the
    // current thread until process exit. See
    // `evaluator::types::regex::reset_regex_cache` for rationale.
    crate::evaluator::types::regex::reset_regex_cache();
    let mut context = EvaluationContext::new(config.clone());
    evaluate_rules(rules, buffer, &mut context)
}

/// Recursively walk `rules` (including children) looking for any
/// [`MetaType::Indirect`] directive.
///
/// Used by the diagnostic guard in [`evaluate_rules_with_config`]: the
/// low-level `_with_config` entry point builds a context without a
/// [`crate::evaluator::RuleEnvironment`], so any `indirect` rule is
/// silently no-op'd at runtime. The check logs the misconfiguration at
/// `debug!` level so consumer tests can detect it without panicking (see
/// GOTCHAS S2.4 for why `debug_assert!` would be wrong here).
// Gated to debug builds like its only caller (see the diagnostic guard in
// `evaluate_rules_with_config`).
#[cfg(debug_assertions)]
fn contains_indirect_rule(rules: &[MagicRule]) -> bool {
    rules.iter().any(|rule| {
        matches!(rule.typ, TypeKind::Meta(MetaType::Indirect))
            || contains_indirect_rule(&rule.children)
    })
}

#[cfg(test)]
mod tests;
