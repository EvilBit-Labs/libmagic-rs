// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Rule evaluation engine
//!
//! This module provides the public interface for magic rule evaluation,
//! including data types for evaluation state and match results, and
//! re-exports the core evaluation functions from submodules.

use crate::{EvaluationConfig, LibmagicError};
use serde::{Deserialize, Serialize};

mod engine;
pub mod offset;
pub mod operators;
pub mod strength;
pub mod types;

pub use engine::{evaluate_rules, evaluate_rules_with_config, evaluate_single_rule};

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
/// Contains information extracted from a successful rule match, including
/// the matched value, position, and confidence score.
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
    /// The type used to read the matched value
    ///
    /// Carries the source `TypeKind` so downstream consumers (e.g., output
    /// formatting) can determine the on-disk width of the matched value.
    pub type_kind: crate::parser::ast::TypeKind,
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

#[cfg(test)]
mod tests;
