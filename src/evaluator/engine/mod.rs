// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Core evaluation engine for magic rules.
//!
//! This module contains the core recursive evaluation logic for executing magic
//! rules against file buffers. It is responsible for:
//! - Evaluating individual rules (`evaluate_single_rule`)
//! - Evaluating hierarchical rule sets with context (`evaluate_rules`)
//! - Providing a convenience wrapper for evaluation with configuration
//!   (`evaluate_rules_with_config`)

use crate::parser::ast::MagicRule;
use crate::{EvaluationConfig, LibmagicError};

use super::{EvaluationContext, RuleMatch, offset, operators, types};
use log::{debug, warn};

/// Evaluate a single magic rule against a file buffer
///
/// This function performs the core rule evaluation by:
/// 1. Resolving the rule's offset specification to an absolute position
/// 2. Reading and interpreting bytes at that position according to the rule's type
/// 3. Coercing the expected value to match the type's signedness and bit width
/// 4. Applying the rule's operator to compare the read value with the expected value
///
/// # Arguments
///
/// * `rule` - The magic rule to evaluate
/// * `buffer` - The file buffer to evaluate against
///
/// # Returns
///
/// Returns `Ok(Some((offset, value)))` if the rule matches (with the resolved offset and
/// read value), `Ok(None)` if it doesn't match, or `Err(LibmagicError)` if evaluation
/// fails due to buffer access issues or other errors.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::evaluate_single_rule;
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
/// let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
/// let result = evaluate_single_rule(&rule, elf_buffer).unwrap();
/// assert!(result.is_some()); // Should match
///
/// let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
/// let result = evaluate_single_rule(&rule, non_elf_buffer).unwrap();
/// assert!(result.is_none()); // Should not match
/// ```
///
/// # Relative offset behavior
///
/// If the rule uses [`OffsetSpec::Relative`](crate::parser::ast::OffsetSpec::Relative),
/// this function resolves it against anchor 0. There is no "previous match"
/// context when evaluating a single rule in isolation. The two cases are:
///
/// - **Non-negative delta (`Relative(N)` for `N >= 0`):** resolves to
///   absolute offset `N`, behaving like `Absolute(N)` from the start of the
///   buffer.
/// - **Negative delta (`Relative(-N)` for `N > 0`):** underflows the
///   anchor (`0 - N`) and returns `EvaluationError::InvalidOffset`. This is
///   *not* equivalent to `Absolute(-N)`, which is interpreted as
///   "from end" of the buffer.
///
/// Callers needing the GNU `file` anchor semantics (where relative offsets
/// resolve against the end of the most recent match) should use
/// [`evaluate_rules`] with an
/// [`EvaluationContext`](crate::evaluator::EvaluationContext), which threads
/// the anchor across the rule list.
///
/// **Behavior change:** before the relative-offset feature landed in v0.5,
/// `OffsetSpec::Relative` returned `EvaluationError::UnsupportedType` here.
/// It now resolves successfully against anchor 0. Callers with existing
/// error-handling code that pattern-matched `UnsupportedType` for relative
/// offsets must remove that arm.
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If offset resolution fails, buffer access is out of bounds,
///   or type interpretation fails
pub fn evaluate_single_rule(
    rule: &MagicRule,
    buffer: &[u8],
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    // Default the relative-offset anchor to 0 -- top-level evaluation with
    // no prior match resolves Relative(N) to absolute N (libmagic semantics).
    evaluate_single_rule_with_anchor(rule, buffer, 0)
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
    // Step 1: Resolve the offset specification to an absolute position
    let absolute_offset =
        offset::resolve_offset_with_context(&rule.offset, buffer, last_match_end)?;

    // Step 2: Read and interpret bytes at the resolved offset according to the rule's type
    let read_value = types::read_typed_value(buffer, absolute_offset, &rule.typ)
        .map_err(|e| LibmagicError::EvaluationError(e.into()))?;

    // Step 3: Coerce the rule's expected value to match the type's signedness/width
    let expected_value = types::coerce_value_to_type(&rule.value, &rule.typ);

    // Step 4: Apply the operator to compare the read value with the expected value
    // BitwiseNot needs type-aware bit-width masking so the complement is computed
    // at the type's natural width (e.g., byte NOT of 0x00 = 0xFF, not u64::MAX).
    let matched = match &rule.op {
        crate::parser::ast::Operator::BitwiseNot => operators::apply_bitwise_not_with_width(
            &read_value,
            &expected_value,
            rule.typ.bit_width(),
        ),
        op => operators::apply_operator(op, &read_value, &expected_value),
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
            let consumed = types::bytes_consumed(buffer, absolute_offset, &rule.typ);
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
                // Check recursion depth limit - this is a critical error that should stop evaluation
                context.increment_recursion_depth()?;

                // Recursively evaluate child rules with graceful error handling
                match evaluate_rules(&rule.children, buffer, context) {
                    Ok(child_matches) => {
                        matches.extend(child_matches);
                    }
                    Err(LibmagicError::Timeout { timeout_ms }) => {
                        // Timeout is critical, propagate it up
                        let _ = context.decrement_recursion_depth();
                        return Err(LibmagicError::Timeout { timeout_ms });
                    }
                    Err(
                        e @ LibmagicError::EvaluationError(
                            crate::error::EvaluationError::RecursionLimitExceeded { .. },
                        ),
                    ) => {
                        // Recursion limit is critical, propagate the original error
                        let _ = context.decrement_recursion_depth();
                        return Err(e);
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
                        // Unexpected errors in children should propagate
                        let _ = context.decrement_recursion_depth();
                        return Err(e);
                    }
                }

                // Restore recursion depth
                context.decrement_recursion_depth()?;
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
    let mut context = EvaluationContext::new(config.clone());
    evaluate_rules(rules, buffer, &mut context)
}

#[cfg(test)]
mod tests;
