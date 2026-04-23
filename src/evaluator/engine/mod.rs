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
use std::sync::atomic::{AtomicBool, Ordering};

/// RAII guard that saves the GNU `file` previous-match anchor on entry and
/// restores it on drop.
///
/// `MetaType::Indirect` re-evaluates the root rule list at the resolved
/// offset, which means it must seed the anchor with that offset for the
/// nested call and then put the caller's anchor back when it returns.
/// Without an RAII wrapper, every early-return path inside the indirect
/// branch would have to remember to restore the anchor manually.
struct AnchorScope<'a> {
    context: &'a mut EvaluationContext,
    saved_anchor: usize,
}

impl<'a> AnchorScope<'a> {
    /// Save the current anchor and seed the context with `new_anchor`.
    fn enter(context: &'a mut EvaluationContext, new_anchor: usize) -> Self {
        let saved_anchor = context.last_match_end();
        context.set_last_match_end(new_anchor);
        Self {
            context,
            saved_anchor,
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
    }
}

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
}

impl<'a> SubroutineScope<'a> {
    fn enter(context: &'a mut EvaluationContext, use_site: usize) -> Self {
        let saved_anchor = context.last_match_end();
        let saved_base = context.base_offset();
        context.set_last_match_end(use_site);
        context.set_base_offset(use_site);
        Self {
            context,
            saved_anchor,
            saved_base,
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
    }
}

/// Process-local once guard for the "use directive without rule environment"
/// warning. Ensures we surface the misconfiguration exactly once per process
/// so low-level programmatic consumers of [`evaluate_rules`] (tests, fuzz
/// harnesses) that intentionally run without a `MagicDatabase`-attached
/// environment do not flood the log on every `Use` rule they encounter.
static USE_WITHOUT_RULE_ENV_WARNED: AtomicBool = AtomicBool::new(false);

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
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte { signed: true },
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF magic".to_string(),
///     children: vec![],
///     level: 0,
///     strength_modifier: None,
/// };
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
    // `indirect`) are silent no-ops in this phase -- the parser
    // preserves them in the AST but the evaluator does not yet wire
    // them into any control-flow behavior. Short-circuiting here with
    // `Ok(None)` keeps them out of the value/pattern paths (which
    // would otherwise surface `TypeReadError::UnsupportedType`).
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
        TypeKind::Meta(MetaType::Use(_)) => {
            // `Use` is dispatched inline by `evaluate_rules` so it can
            // push the subroutine's matches into the caller's match
            // vector. Reaching this arm means the rule went through the
            // single-rule path (e.g. via `evaluate_single_rule`) which
            // lacks that wiring; treat it as a silent no-op.
            return Ok(None);
        }
        TypeKind::Meta(_) => return Ok(None),
        TypeKind::Regex { .. } | TypeKind::Search { .. } => {
            evaluate_pattern_rule(rule, buffer, absolute_offset)?
        }
        _ => evaluate_value_rule(rule, buffer, absolute_offset)?,
    };
    Ok(matched.then_some((absolute_offset, read_value)))
}

/// Evaluate a `TypeKind::Meta(MetaType::Use(name))` rule inline.
///
/// Looks up `name` in the context's rule environment, temporarily sets the
/// GNU `file` previous-match anchor to the resolved offset, and recursively
/// evaluates the subroutine's rules against `buffer`. Any matches produced
/// by the subroutine are returned in document order and are intended to be
/// pushed into the caller's match vector *before* the synthetic `Use` match
/// itself (matching GNU `file` behavior where a `use` site is replaced by
/// its expansion in the output).
///
/// Returns `Ok((Some(absolute_offset), matches))` on a successful resolution
/// (even if the subroutine produced no matches), or `Ok((None, vec![]))`
/// when:
/// - the context has no rule environment attached (programmatic consumers
///   bypassing `MagicDatabase`)
/// - the referenced name is not in the table (logged at warn level)
///
/// Recursion-limit propagation is handled via [`RecursionGuard`] so that a
/// subroutine calling `use` on itself triggers `RecursionLimitExceeded`
/// instead of a stack overflow.
fn evaluate_use_rule(
    rule: &MagicRule,
    name: &str,
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
    let subroutine_matches = {
        let mut scope = SubroutineScope::enter(context, absolute_offset);
        let mut guard = RecursionGuard::enter(scope.context())?;
        evaluate_rules(&subroutine_rules, buffer, guard.context())?
    };

    Ok((Some(absolute_offset), subroutine_matches))
}

/// Evaluate a pattern-bearing rule (`TypeKind::Regex` / `TypeKind::Search`).
///
/// `read_pattern_match` returns `Some(value)` on a successful match
/// (possibly zero-width, e.g., `a*`) and `None` on a genuine miss; the
/// engine translates those directly into `Equal`/`NotEqual`. Any other
/// operator on a pattern-bearing type is a magic-file semantic bug and
/// surfaces as [`TypeReadError::UnsupportedType`] -- the earlier
/// fallthrough to `apply_operator` masked this by producing nonsense
/// ordering comparisons against the pattern source text.
///
/// On a miss we return `Value::String(String::new())` as a display
/// placeholder; the engine has already decided `matched = false` by
/// then, so the placeholder only affects display and
/// `bytes_consumed_with_pattern` (which re-derives the match position
/// from the pattern, not this value).
fn evaluate_pattern_rule(
    rule: &MagicRule,
    buffer: &[u8],
    absolute_offset: usize,
) -> Result<(bool, crate::parser::ast::Value), LibmagicError> {
    let match_outcome =
        types::read_pattern_match(buffer, absolute_offset, &rule.typ, Some(&rule.value))
            .map_err(|e| LibmagicError::EvaluationError(e.into()))?;
    let pattern_found = match_outcome.is_some();
    let matched = match &rule.op {
        crate::parser::ast::Operator::Equal => pattern_found,
        crate::parser::ast::Operator::NotEqual => !pattern_found,
        other => {
            return Err(LibmagicError::EvaluationError(
                types::TypeReadError::UnsupportedType {
                    type_name: format!(
                        "operator {other:?} is not supported for pattern-bearing type {:?}; only Equal (=) and NotEqual (!=) are allowed",
                        rule.typ
                    ),
                }
                .into(),
            ));
        }
    };
    let value = match_outcome.unwrap_or_else(|| crate::parser::ast::Value::String(String::new()));
    Ok((matched, value))
}

/// Evaluate a value-based rule (all non-pattern-bearing `TypeKind` variants).
///
/// Reads the typed value at `absolute_offset`, coerces the rule's
/// expected value to the target type's signedness/width (zero-copy via
/// `Cow::Borrowed` on the hot path), and applies the operator.
/// `BitwiseNot` needs type-aware width masking so the complement is
/// computed at the type's natural width (e.g. byte `NOT 0x00 = 0xFF`,
/// not `u64::MAX`).
fn evaluate_value_rule(
    rule: &MagicRule,
    buffer: &[u8],
    absolute_offset: usize,
) -> Result<(bool, crate::parser::ast::Value), LibmagicError> {
    let read_value =
        types::read_typed_value_with_pattern(buffer, absolute_offset, &rule.typ, Some(&rule.value))
            .map_err(|e| LibmagicError::EvaluationError(e.into()))?;

    let expected_value = types::coerce_value_to_type(&rule.value, &rule.typ);
    let expected_ref: &crate::parser::ast::Value = expected_value.as_ref();

    let matched = match &rule.op {
        crate::parser::ast::Operator::BitwiseNot => {
            operators::apply_bitwise_not_with_width(&read_value, expected_ref, rule.typ.bit_width())
        }
        op => operators::apply_operator(op, &read_value, expected_ref),
    };
    Ok((matched, read_value))
}

/// Evaluate a rule's children under the standard recursion-guard/graceful-skip discipline.
///
/// This helper centralises the `RecursionGuard` + `evaluate_rules` + error-dispatch
/// pattern that is identical across the `Default`, `Indirect`, `Offset`, and `Use`
/// meta-type arms in [`evaluate_rules`]. Extracting it prevents the four copies
/// from drifting apart during future maintenance.
///
/// # Behaviour
///
/// * If `rule.children` is empty the function is a no-op (returns `Ok(())`).
/// * Child matches are appended to `matches` in document order.
/// * `LibmagicError::Timeout` and `LibmagicError::EvaluationError(RecursionLimitExceeded)`
///   propagate immediately as `Err` so the caller can bail out.
/// * Data-dependent errors (`BufferOverrun`, `InvalidOffset`,
///   `TypeReadError::BufferOverrun`, `TypeReadError::InvalidPStringLength`,
///   `IoError`) are logged at `warn!` and swallowed; the parent match
///   already in `matches` is left intact. This mirrors the defensive
///   comment in each arm: the inner `evaluate_rules` already catches and
///   logs individual child failures, so this arm only fires if that
///   strategy changes.
///
/// # Arguments
///
/// * `rule`      – The parent rule whose children will be evaluated.
/// * `rule_kind` – A short label for the rule kind used in the `warn!`
///   message (e.g. `"default"`, `"indirect"`, `"offset"`, `"use"`).
/// * `buffer`    – The file buffer passed to the recursive call.
/// * `context`   – Mutable evaluation context; the recursion depth is
///   incremented on entry and decremented on drop via [`RecursionGuard`].
/// * `matches`   – Output vector; child matches are appended here.
fn evaluate_children_or_warn(
    rule: &MagicRule,
    rule_kind: &str,
    buffer: &[u8],
    context: &mut EvaluationContext,
    matches: &mut Vec<RuleMatch>,
) -> Result<(), LibmagicError> {
    if rule.children.is_empty() {
        return Ok(());
    }
    let mut guard = RecursionGuard::enter(context)?;
    match evaluate_rules(&rule.children, buffer, guard.context()) {
        Ok(child_matches) => {
            matches.extend(child_matches);
        }
        Err(LibmagicError::Timeout { timeout_ms }) => {
            return Err(LibmagicError::Timeout { timeout_ms });
        }
        Err(
            e @ (LibmagicError::EvaluationError(
                crate::error::EvaluationError::BufferOverrun { .. }
                | crate::error::EvaluationError::InvalidOffset { .. }
                | crate::error::EvaluationError::TypeReadError(
                    crate::evaluator::types::TypeReadError::BufferOverrun { .. }
                    | crate::evaluator::types::TypeReadError::InvalidPStringLength { .. },
                ),
            )
            | LibmagicError::IoError(_)),
        ) => {
            warn!(
                "Discarding child evaluation under {} rule '{}' due to unexpected error: {} -- parent match is still emitted",
                rule_kind, rule.message, e
            );
        }
        Err(e) => return Err(e),
    }
    Ok(())
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
/// let parent_rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte { signed: true },
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF".to_string(),
///     children: vec![
///         MagicRule {
///             offset: OffsetSpec::Absolute(4),
///             typ: TypeKind::Byte { signed: true },
///             op: Operator::Equal,
///             value: Value::Uint(2),
///             message: "64-bit".to_string(),
///             children: vec![],
///             level: 1,
///             strength_modifier: None,
///         }
///     ],
///     level: 0,
///     strength_modifier: None,
/// };
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
    let mut sibling_matched: bool = false;

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
    let entry_anchor = context.last_match_end();
    let is_child_sibling_list = context.recursion_depth() > 0;

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
        // sibling matched. It does not produce a match, evaluate
        // children, or advance the anchor.
        if let TypeKind::Meta(MetaType::Clear) = &rule.typ {
            sibling_matched = false;
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

                let match_result = RuleMatch {
                    message: rule.message.clone(),
                    offset: context.last_match_end(),
                    level: rule.level,
                    value: crate::parser::ast::Value::Uint(0),
                    type_kind: rule.typ.clone(),
                    confidence: RuleMatch::calculate_confidence(rule.level),
                };
                matches.push(match_result);

                // `default` is treated as a successful match at this
                // level, so its children are evaluated under the same
                // recursion-guard pattern as every other successful rule.
                evaluate_children_or_warn(rule, "default", buffer, context, &mut matches)?;

                sibling_matched = true;

                if matches.len() > matches_before && context.should_stop_at_first_match() {
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
            let Some(root_rules) = context.rule_env().map(|e| e.root_rules.clone()) else {
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

            let indirect_match = RuleMatch {
                message: rule.message.clone(),
                offset: absolute_offset,
                level: rule.level,
                value: crate::parser::ast::Value::String("indirect".to_string()),
                type_kind: rule.typ.clone(),
                confidence: RuleMatch::calculate_confidence(rule.level),
            };
            matches.push(indirect_match);

            // Indirect counts as a match for `sibling_matched` regardless of
            // whether the sub-evaluation produced any matches -- the directive
            // itself successfully dispatched.
            sibling_matched = true;

            // Recursion guard + anchor scope: nested indirect / use cycles
            // surface as `RecursionLimitExceeded` instead of a stack overflow,
            // and the caller's anchor is restored on every exit path.
            {
                let mut guard = RecursionGuard::enter(context)?;
                let mut anchor_scope = AnchorScope::enter(guard.context(), 0);
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

            if matches.len() > matches_before && context.should_stop_at_first_match() {
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
            if !matches!(rule.op, crate::parser::ast::Operator::AnyValue) {
                debug!(
                    "offset rule '{}': non-`x` operator {:?} not supported; skipping",
                    rule.message, rule.op
                );
                continue;
            }

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

            let matches_before = matches.len();

            // Advance the anchor BEFORE emitting the match so sibling
            // rules resolve their relative offsets against the offset
            // directive's resolved position. Same discipline as
            // `Indirect` and every other value-bearing rule.
            context.set_last_match_end(absolute_offset);

            let offset_match = RuleMatch {
                message: rule.message.clone(),
                offset: absolute_offset,
                level: rule.level,
                value: crate::parser::ast::Value::Uint(absolute_offset as u64),
                type_kind: rule.typ.clone(),
                confidence: RuleMatch::calculate_confidence(rule.level),
            };
            matches.push(offset_match);

            sibling_matched = true;

            // Evaluate children under the recursion-guard pattern used
            // by every other successful rule.
            evaluate_children_or_warn(rule, "offset", buffer, context, &mut matches)?;

            if matches.len() > matches_before && context.should_stop_at_first_match() {
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
        if let TypeKind::Meta(MetaType::Use(name)) = &rule.typ {
            let matches_before = matches.len();
            let use_resolved = match evaluate_use_rule(rule, name, buffer, context) {
                Ok((Some(absolute_offset), subroutine_matches)) => {
                    matches.extend(subroutine_matches);

                    // A `use` rule itself does not produce a surface
                    // `RuleMatch` in GNU `file` output; the subroutine's
                    // rules carry the visible messages. We therefore only
                    // advance the anchor (to the use-site offset, which
                    // may have been moved by the subroutine; since we
                    // restored it above, we now re-advance to the
                    // use-site offset so subsequent sibling rules resolve
                    // relative offsets from the use-site end).
                    context.set_last_match_end(absolute_offset);
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
            // short-circuiting, halt evaluation of further siblings.
            if matches.len() > matches_before && context.should_stop_at_first_match() {
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
        ) {
            Ok(data) => data,
            Err(
                e @ (LibmagicError::EvaluationError(
                    crate::error::EvaluationError::BufferOverrun { .. }
                    | crate::error::EvaluationError::InvalidOffset { .. }
                    | crate::error::EvaluationError::TypeReadError(
                        crate::evaluator::types::TypeReadError::BufferOverrun { .. }
                        | crate::evaluator::types::TypeReadError::InvalidPStringLength { .. },
                    ),
                )
                | LibmagicError::IoError(_)),
            ) => {
                // Expected data-dependent evaluation errors -- skip gracefully.
                // TypeReadError::UnsupportedType is intentionally NOT caught here
                // so that evaluator capability gaps propagate as errors.
                debug!("Skipping rule '{}': {}", rule.message, e);
                continue;
            }
            Err(e) => {
                // Unexpected errors (InternalError, UnsupportedType, etc.) should propagate
                return Err(e);
            }
        };

        if let Some((absolute_offset, read_value)) = match_data {
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

            let match_result = RuleMatch {
                message: rule.message.clone(),
                offset: absolute_offset,
                level: rule.level,
                value: read_value,
                type_kind: rule.typ.clone(),
                confidence: RuleMatch::calculate_confidence(rule.level),
            };
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
                    Err(e) => {
                        // Unexpected errors in children (including RecursionLimitExceeded)
                        // should propagate. The guard drops here, decrementing the depth.
                        return Err(e);
                    }
                }
                // `guard` drops here, decrementing the recursion depth.
            }

            // Stop at first match if configured to do so
            if context.should_stop_at_first_match() {
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
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte { signed: true },
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF magic".to_string(),
///     children: vec![],
///     level: 0,
///     strength_modifier: None,
/// };
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
    // callers (matching the `Use`-without-env contract), but we log the
    // misconfiguration at `debug!` level so consumer tests can detect
    // env-less `indirect` usage. Using `debug_assert!` would panic in test
    // builds and break the "evaluator never panics" invariant documented in
    // GOTCHAS S2.4 -- a misconfigured caller should get a no-op, not a crash.
    if contains_indirect_rule(rules) {
        debug!(
            "{}",
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
fn contains_indirect_rule(rules: &[MagicRule]) -> bool {
    rules.iter().any(|rule| {
        matches!(rule.typ, TypeKind::Meta(MetaType::Indirect))
            || contains_indirect_rule(&rule.children)
    })
}

#[cfg(test)]
mod tests;
