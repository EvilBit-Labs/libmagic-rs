// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Rule evaluation engine
//!
//! This module contains the core evaluation logic for executing magic rules
//! against file buffers to identify file types.

use crate::parser::ast::MagicRule;
use crate::{EvaluationConfig, LibmagicError};
use serde::{Deserialize, Serialize};

pub mod offset;
pub mod operators;
pub mod strength;
pub mod types;

/// Context for maintaining evaluation state during rule processing
///
/// The `EvaluationContext` tracks the current state of rule evaluation,
/// including the current offset position, recursion depth for nested rules,
/// and configuration settings that control evaluation behavior.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::EvaluationContext;
/// use libmagic_rs::EvaluationConfig;
///
/// let config = EvaluationConfig::default();
/// let context = EvaluationContext::new(config);
///
/// assert_eq!(context.current_offset(), 0);
/// assert_eq!(context.recursion_depth(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Current offset position in the file buffer
    current_offset: usize,
    /// Current recursion depth for nested rule evaluation
    recursion_depth: u32,
    /// Configuration settings for evaluation behavior
    config: EvaluationConfig,
}

impl EvaluationContext {
    /// Create a new evaluation context with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration settings for evaluation behavior
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::evaluator::EvaluationContext;
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::default();
    /// let context = EvaluationContext::new(config);
    /// ```
    #[must_use]
    pub const fn new(config: EvaluationConfig) -> Self {
        Self {
            current_offset: 0,
            recursion_depth: 0,
            config,
        }
    }

    /// Get the current offset position
    ///
    /// # Returns
    ///
    /// The current offset position in the file buffer
    #[must_use]
    pub const fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Set the current offset position
    ///
    /// # Arguments
    ///
    /// * `offset` - The new offset position
    pub fn set_current_offset(&mut self, offset: usize) {
        self.current_offset = offset;
    }

    /// Get the current recursion depth
    ///
    /// # Returns
    ///
    /// The current recursion depth for nested rule evaluation
    #[must_use]
    pub const fn recursion_depth(&self) -> u32 {
        self.recursion_depth
    }

    /// Increment the recursion depth
    ///
    /// # Returns
    ///
    /// `Ok(())` if the recursion depth is within limits, or `Err(LibmagicError)`
    /// if the maximum recursion depth would be exceeded
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::EvaluationError` if incrementing would exceed
    /// the maximum recursion depth configured in the evaluation config.
    pub fn increment_recursion_depth(&mut self) -> Result<(), LibmagicError> {
        if self.recursion_depth >= self.config.max_recursion_depth {
            return Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::recursion_limit_exceeded(self.recursion_depth),
            ));
        }
        self.recursion_depth += 1;
        Ok(())
    }

    /// Decrement the recursion depth
    ///
    /// # Errors
    ///
    /// Returns an error if the recursion depth is already 0, as this indicates
    /// a programming error in the evaluation logic (mismatched increment/decrement calls).
    pub fn decrement_recursion_depth(&mut self) -> Result<(), LibmagicError> {
        if self.recursion_depth == 0 {
            return Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::internal_error(
                    "Attempted to decrement recursion depth below 0",
                ),
            ));
        }
        self.recursion_depth -= 1;
        Ok(())
    }

    /// Get a reference to the evaluation configuration
    ///
    /// # Returns
    ///
    /// A reference to the `EvaluationConfig` used by this context
    #[must_use]
    pub const fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Check if evaluation should stop at the first match
    ///
    /// # Returns
    ///
    /// `true` if evaluation should stop at the first match, `false` otherwise
    #[must_use]
    pub const fn should_stop_at_first_match(&self) -> bool {
        self.config.stop_at_first_match
    }

    /// Get the maximum string length allowed
    ///
    /// # Returns
    ///
    /// The maximum string length that should be read during evaluation
    #[must_use]
    pub const fn max_string_length(&self) -> usize {
        self.config.max_string_length
    }

    /// Check if MIME type mapping is enabled
    ///
    /// # Returns
    ///
    /// `true` if MIME type mapping should be performed, `false` otherwise
    #[must_use]
    pub const fn enable_mime_types(&self) -> bool {
        self.config.enable_mime_types
    }

    /// Get the evaluation timeout in milliseconds
    ///
    /// # Returns
    ///
    /// The timeout duration in milliseconds, or `None` if no timeout is set
    #[must_use]
    pub const fn timeout_ms(&self) -> Option<u64> {
        self.config.timeout_ms
    }

    /// Reset the context to initial state while preserving configuration
    ///
    /// This resets the current offset and recursion depth to 0, but keeps
    /// the same configuration settings.
    pub fn reset(&mut self) {
        self.current_offset = 0;
        self.recursion_depth = 0;
    }
}

/// Result of evaluating a magic rule
///
/// Contains information about a successful rule match, including the rule
/// that matched and its associated message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleMatch {
    /// The message associated with the matching rule
    pub message: String,
    /// The offset where the match occurred
    pub offset: usize,
    /// The rule level (depth in hierarchy)
    pub level: u32,
    /// The matched value
    pub value: crate::parser::ast::Value,
    /// Confidence score (0.0 to 1.0)
    ///
    /// Calculated based on match depth in the rule hierarchy.
    /// Deeper matches indicate more specific file type identification
    /// and thus higher confidence.
    pub confidence: f64,
}

impl RuleMatch {
    /// Calculate confidence score based on rule depth
    ///
    /// Formula: min(1.0, 0.3 + (level * 0.2))
    /// - Level 0 (root): 0.3
    /// - Level 1: 0.5
    /// - Level 2: 0.7
    /// - Level 3: 0.9
    /// - Level 4+: 1.0 (capped)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::RuleMatch;
    ///
    /// assert!((RuleMatch::calculate_confidence(0) - 0.3).abs() < 0.001);
    /// assert!((RuleMatch::calculate_confidence(3) - 0.9).abs() < 0.001);
    /// assert!((RuleMatch::calculate_confidence(10) - 1.0).abs() < 0.001);
    /// ```
    #[must_use]
    pub fn calculate_confidence(level: u32) -> f64 {
        (0.3 + (f64::from(level) * 0.2)).min(1.0)
    }
}

/// Evaluate a single magic rule against a file buffer
///
/// This function performs the core rule evaluation by:
/// 1. Resolving the rule's offset specification to an absolute position
/// 2. Reading and interpreting bytes at that position according to the rule's type
/// 3. Applying the rule's operator to compare the read value with the expected value
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
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If offset resolution fails, buffer access is out of bounds,
///   or type interpretation fails
pub fn evaluate_single_rule(
    rule: &MagicRule,
    buffer: &[u8],
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    // Step 1: Resolve the offset specification to an absolute position
    let absolute_offset = offset::resolve_offset(&rule.offset, buffer)?;

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
/// * `context` - Mutable evaluation context for state management
///
/// # Returns
///
/// Returns `Ok(Vec<RuleMatch>)` containing all matches found. Errors in individual rules
/// are logged and skipped to allow evaluation to continue. Only returns `Err(LibmagicError)`
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

    for rule in rules {
        // Check timeout periodically (every 16 rules) to reduce syscall overhead
        rule_count = rule_count.wrapping_add(1);
        if rule_count.trailing_zeros() >= 4
            && let Some(timeout_ms) = context.timeout_ms()
            && start_time.elapsed().as_millis() > u128::from(timeout_ms)
        {
            return Err(LibmagicError::Timeout { timeout_ms });
        }

        // Evaluate the current rule with graceful error handling
        let match_data = match evaluate_single_rule(rule, buffer) {
            Ok(data) => data,
            Err(
                LibmagicError::EvaluationError(
                    crate::error::EvaluationError::BufferOverrun { .. }
                    | crate::error::EvaluationError::InvalidOffset { .. }
                    | crate::error::EvaluationError::TypeReadError(_),
                )
                | LibmagicError::IoError(_),
            ) => {
                // Expected evaluation errors for individual rules -- skip gracefully
                continue;
            }
            Err(e) => {
                // Unexpected errors (InternalError, UnsupportedType, etc.) should propagate
                return Err(e);
            }
        };

        if let Some((absolute_offset, read_value)) = match_data {
            let match_result = RuleMatch {
                message: rule.message.clone(),
                offset: absolute_offset,
                level: rule.level,
                value: read_value,
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
                    Err(LibmagicError::Timeout { .. }) => {
                        // Timeout is critical, propagate it up
                        context.decrement_recursion_depth()?;
                        return Err(LibmagicError::Timeout {
                            timeout_ms: context.timeout_ms().unwrap_or(0),
                        });
                    }
                    Err(LibmagicError::EvaluationError(
                        crate::error::EvaluationError::RecursionLimitExceeded { .. },
                    )) => {
                        // Recursion limit is critical, propagate it up
                        context.decrement_recursion_depth()?;
                        return Err(LibmagicError::EvaluationError(
                            crate::error::EvaluationError::RecursionLimitExceeded {
                                depth: context.recursion_depth(),
                            },
                        ));
                    }
                    Err(
                        LibmagicError::EvaluationError(
                            crate::error::EvaluationError::BufferOverrun { .. }
                            | crate::error::EvaluationError::InvalidOffset { .. }
                            | crate::error::EvaluationError::TypeReadError(_),
                        )
                        | LibmagicError::IoError(_),
                    ) => {
                        // Expected child evaluation errors -- skip gracefully
                    }
                    Err(e) => {
                        // Unexpected errors in children should propagate
                        context.decrement_recursion_depth()?;
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
    let mut context = EvaluationContext::new(config.clone());
    evaluate_rules(rules, buffer, &mut context)
}

#[cfg(test)]
mod tests;
