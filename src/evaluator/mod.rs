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

/// Shared environment attached to an [`EvaluationContext`] so the engine can
/// resolve whole-database operations (currently: `Use` subroutine lookups;
/// eventually `indirect` whole-tree re-entry).
///
/// Stored as an `Arc` so cloning a context across recursive calls is cheap
/// and the rule data can be shared safely across threads.
#[derive(Debug, Clone)]
pub(crate) struct RuleEnvironment {
    /// Named subroutine table, keyed by identifier.
    pub(crate) name_table: std::sync::Arc<crate::parser::name_table::NameTable>,
    /// Top-level rule list retained for future whole-database operations.
    #[allow(dead_code)]
    pub(crate) root_rules: std::sync::Arc<[crate::parser::ast::MagicRule]>,
}

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
    /// End offset of the most recent successful match.
    ///
    /// This is the GNU `file`/libmagic anchor used to resolve relative
    /// (`&+N` / `&-N`) offsets. It is updated to the end of the most
    /// recently matched rule -- the value may *increase or decrease* as
    /// successive rules match at different positions; it is not a
    /// high-watermark. A fresh context starts with this set to 0, which
    /// matches libmagic's behavior of resolving top-level relative offsets
    /// from the file start.
    last_match_end: usize,
    /// Current recursion depth for nested rule evaluation
    recursion_depth: u32,
    /// Configuration settings for evaluation behavior
    config: EvaluationConfig,
    /// Optional rule environment (name table + root rules) threaded from
    /// [`MagicDatabase`](crate::MagicDatabase). Evaluations that come in
    /// through the low-level [`evaluate_rules`] / [`evaluate_rules_with_config`]
    /// surface (tests, programmatic consumers) run with `rule_env = None`,
    /// in which case `MetaType::Use` rules are silent no-ops.
    rule_env: Option<std::sync::Arc<RuleEnvironment>>,
    /// Base offset applied to absolute offset resolution.
    ///
    /// Normally 0. When evaluating a subroutine body via `MetaType::Use`,
    /// this is set to the use-site offset so that the subroutine's
    /// `OffsetSpec::Absolute(n)` rules resolve to `base + n` (matching
    /// magic(5) / libmagic semantics: subroutines see offsets relative
    /// to the caller's invocation point, not absolute file positions).
    /// Restored to the caller's value on subroutine exit via the
    /// `BaseOffsetScope` RAII guard in `engine/mod.rs`.
    base_offset: usize,
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
            last_match_end: 0,
            recursion_depth: 0,
            config,
            rule_env: None,
            base_offset: 0,
        }
    }

    /// Read-only access to the subroutine base offset. Non-zero only
    /// during a `MetaType::Use` body evaluation.
    #[must_use]
    pub(crate) const fn base_offset(&self) -> usize {
        self.base_offset
    }

    /// Set the subroutine base offset.
    ///
    /// `pub(crate)` and owned by the engine's `BaseOffsetScope` RAII
    /// guard -- no external caller should set this directly.
    pub(crate) fn set_base_offset(&mut self, offset: usize) {
        self.base_offset = offset;
    }

    /// Attach a rule environment to this context.
    ///
    /// The environment carries the name-subroutine table and root rule list
    /// so the engine can resolve `MetaType::Use` rules and (eventually)
    /// `MetaType::Indirect` re-entries. Intended to be called once by
    /// [`MagicDatabase`](crate::MagicDatabase) before handing the context
    /// to [`evaluate_rules`].
    #[must_use]
    pub(crate) fn with_rule_env(mut self, env: std::sync::Arc<RuleEnvironment>) -> Self {
        self.rule_env = Some(env);
        self
    }

    /// Read-only access to the attached rule environment, if any.
    #[must_use]
    pub(crate) fn rule_env(&self) -> Option<&RuleEnvironment> {
        self.rule_env.as_deref()
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

    /// Get the end offset of the most recent successful match.
    ///
    /// This is the GNU `file`/libmagic anchor used to resolve relative
    /// (`&+N` / `&-N`) offset specifications. A fresh context returns 0,
    /// which makes top-level relative offsets resolve from the file start.
    ///
    /// `pub(crate)` because the anchor is an internal engine detail; external
    /// consumers should not couple to it.
    #[must_use]
    pub(crate) const fn last_match_end(&self) -> usize {
        self.last_match_end
    }

    /// Set the end offset of the most recent successful match.
    ///
    /// Called by the evaluation engine after a rule matches, to advance the
    /// anchor used by subsequent relative offset resolution. The new value
    /// is typically `match_offset + bytes_consumed_by_type`.
    ///
    /// `pub(crate)` because external callers should not be able to inject
    /// arbitrary anchor state. External callers that need to clear the
    /// anchor between buffer evaluations should call
    /// `EvaluationContext::reset()`, which resets the anchor, current
    /// offset, and recursion depth together.
    pub(crate) fn set_last_match_end(&mut self, offset: usize) {
        self.last_match_end = offset;
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
    pub(crate) fn increment_recursion_depth(&mut self) -> Result<(), LibmagicError> {
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
    pub(crate) fn decrement_recursion_depth(&mut self) -> Result<(), LibmagicError> {
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
        self.last_match_end = 0;
        self.recursion_depth = 0;
        self.base_offset = 0;
    }
}

/// RAII guard that increments recursion depth on entry and decrements on drop.
///
/// Replaces the manual `increment_recursion_depth` / `decrement_recursion_depth`
/// pair with a scope-based guard, eliminating the risk of mismatched calls and
/// the need to swallow cleanup errors on error-return paths.
///
/// Obtain a guard via [`RecursionGuard::enter`], which borrows the context
/// mutably for the guard's lifetime. Use [`RecursionGuard::context`] to access
/// the borrowed context for the duration of the recursive call. The guard
/// automatically decrements the recursion depth when it goes out of scope.
///
/// The guard is `pub(crate)` because recursion-depth management is an internal
/// detail of the evaluation engine.
pub(crate) struct RecursionGuard<'a> {
    context: &'a mut EvaluationContext,
}

impl<'a> RecursionGuard<'a> {
    /// Enter a new recursion level, incrementing the context's recursion depth.
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::EvaluationError` if incrementing would exceed
    /// the maximum recursion depth configured in the evaluation config.
    pub(crate) fn enter(context: &'a mut EvaluationContext) -> Result<Self, LibmagicError> {
        context.increment_recursion_depth()?;
        Ok(Self { context })
    }

    /// Access the underlying context for the duration of the guard.
    pub(crate) fn context(&mut self) -> &mut EvaluationContext {
        self.context
    }
}

impl Drop for RecursionGuard<'_> {
    fn drop(&mut self) {
        // Safe to ignore: `decrement_recursion_depth` only fails when the
        // depth is already 0, which is impossible here because `enter` just
        // incremented it and the depth is only mutated through guard pairs.
        let result = self.context.decrement_recursion_depth();
        debug_assert!(
            result.is_ok(),
            "RecursionGuard invariant violated: decrement failed after successful enter()"
        );
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
