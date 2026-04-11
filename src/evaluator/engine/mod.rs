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

use crate::parser::ast::MagicRule;
use crate::{EvaluationConfig, LibmagicError};

use super::{EvaluationContext, RecursionGuard, RuleMatch, offset, operators, types};
use log::{debug, warn};

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
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    use crate::parser::ast::TypeKind;

    // Step 1: Resolve the offset specification to an absolute position.
    let absolute_offset =
        offset::resolve_offset_with_context(&rule.offset, buffer, last_match_end)?;

    // Step 2 & 3: Dispatch on type category. Pattern-bearing types
    // (Regex, Search) take a different path from fixed-width types
    // because the rule's `value` operand is the *pattern*, not an
    // expected matched value. Running those through `apply_operator`
    // would compare matched text ("123") against the pattern literal
    // ("[0-9]+") and produce false negatives on any regex with
    // metacharacters.
    let (matched, read_value) = match &rule.typ {
        TypeKind::Regex { .. } | TypeKind::Search { .. } => {
            evaluate_pattern_rule(rule, buffer, absolute_offset)?
        }
        _ => evaluate_value_rule(rule, buffer, absolute_offset)?,
    };
    Ok(matched.then_some((absolute_offset, read_value)))
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
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<Vec<RuleMatch>, LibmagicError> {
    let mut matches = Vec::with_capacity(8);
    let start_time = std::time::Instant::now();
    let mut rule_count = 0u32;

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
        // Check timeout periodically (every 16 rules) to reduce syscall overhead
        rule_count = rule_count.wrapping_add(1);
        if rule_count.trailing_zeros() >= 4
            && let Some(timeout_ms) = context.timeout_ms()
            && start_time.elapsed().as_millis() >= u128::from(timeout_ms)
        {
            return Err(LibmagicError::Timeout { timeout_ms });
        }

        // Evaluate the current rule with graceful error handling.
        // Pass the GNU `file` anchor so OffsetSpec::Relative resolves
        // correctly against the previous match's end position.
        let match_data =
            match evaluate_single_rule_with_anchor(rule, buffer, context.last_match_end()) {
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
    // Clear the thread-local regex compile cache so it is bounded to
    // the lifetime of a single top-level evaluation call. Cache
    // entries from a previous rule set would otherwise persist on the
    // current thread until process exit. See
    // `evaluator::types::regex::reset_regex_cache` for rationale.
    crate::evaluator::types::regex::reset_regex_cache();
    let mut context = EvaluationContext::new(config.clone());
    evaluate_rules(rules, buffer, &mut context)
}

#[cfg(test)]
mod tests;
