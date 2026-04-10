// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Evaluation configuration for magic rule processing.
//!
//! Defines [`EvaluationConfig`], which controls recursion depth, string length
//! limits, matching strategy, MIME type mapping, and timeouts during rule
//! evaluation. Extracted from `lib.rs` to keep that module under the project's
//! file-size limit.

use crate::Result;
use crate::error::LibmagicError;

/// Configuration for rule evaluation
///
/// This struct controls various aspects of magic rule evaluation behavior,
/// including performance limits, output options, and matching strategies.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::EvaluationConfig;
///
/// // Use default configuration
/// let config = EvaluationConfig::default();
///
/// // Create custom configuration
/// let custom_config = EvaluationConfig {
///     max_recursion_depth: 10,
///     max_string_length: 4096,
///     stop_at_first_match: false, // Get all matches
///     enable_mime_types: true,
///     timeout_ms: Some(5000), // 5 second timeout
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationConfig {
    /// Maximum recursion depth for nested rules
    ///
    /// This prevents infinite recursion in malformed magic files and limits
    /// the depth of rule hierarchy traversal. Default is 20.
    pub max_recursion_depth: u32,

    /// Maximum string length to read
    ///
    /// This limits the amount of data read for string types to prevent
    /// excessive memory usage. Default is 8192 bytes.
    pub max_string_length: usize,

    /// Stop at first match or continue for all matches
    ///
    /// When `true`, evaluation stops after the first matching rule.
    /// When `false`, all rules are evaluated to find all matches.
    /// Default is `true` for performance.
    ///
    /// # Semantics
    ///
    /// "First match" refers to the first *top-level* rule that matches.
    /// Children of the first matching top-level rule are always evaluated
    /// before the stop check; the stop check applies to subsequent
    /// top-level rules. In other words, `stop_at_first_match = true` does
    /// not truncate the child subtree of the matching rule -- it only
    /// prevents later sibling top-level rules from being evaluated. A
    /// successful top-level match therefore returns one parent `RuleMatch`
    /// plus any descendant `RuleMatch` values its children produced.
    pub stop_at_first_match: bool,

    /// Enable MIME type mapping in results
    ///
    /// When `true`, the evaluator will attempt to map file type descriptions
    /// to standard MIME types. Default is `false`.
    pub enable_mime_types: bool,

    /// Timeout for evaluation in milliseconds
    ///
    /// If set, evaluation will be aborted if it takes longer than this duration.
    /// `None` means no timeout. Default is `None`.
    pub timeout_ms: Option<u64>,
}

impl Default for EvaluationConfig {
    /// Returns the default evaluation configuration.
    ///
    /// # Security
    ///
    /// The default configuration has no timeout. When processing untrusted
    /// input, use [`EvaluationConfig::performance()`] or set `timeout_ms`
    /// explicitly to prevent denial of service.
    fn default() -> Self {
        Self {
            max_recursion_depth: 20,
            max_string_length: 8192,
            stop_at_first_match: true,
            enable_mime_types: false,
            timeout_ms: None,
        }
    }
}

impl EvaluationConfig {
    /// Create a new configuration with default values
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::new();
    /// assert_eq!(config.max_recursion_depth, 20);
    /// assert_eq!(config.max_string_length, 8192);
    /// assert!(config.stop_at_first_match);
    /// assert!(!config.enable_mime_types);
    /// assert_eq!(config.timeout_ms, None);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration optimized for performance
    ///
    /// This configuration prioritizes speed over completeness:
    /// - Lower recursion depth limit
    /// - Smaller string length limit
    /// - Stop at first match
    /// - No MIME type mapping
    /// - Short timeout
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::performance();
    /// assert_eq!(config.max_recursion_depth, 10);
    /// assert_eq!(config.max_string_length, 1024);
    /// assert!(config.stop_at_first_match);
    /// assert!(!config.enable_mime_types);
    /// assert_eq!(config.timeout_ms, Some(1000));
    /// ```
    #[must_use]
    pub const fn performance() -> Self {
        Self {
            max_recursion_depth: 10,
            max_string_length: 1024,
            stop_at_first_match: true,
            enable_mime_types: false,
            timeout_ms: Some(1000), // 1 second
        }
    }

    /// Create a configuration optimized for completeness
    ///
    /// This configuration prioritizes finding all matches over speed:
    /// - Higher recursion depth limit
    /// - Larger string length limit
    /// - Find all matches
    /// - Enable MIME type mapping
    /// - Longer timeout
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::comprehensive();
    /// assert_eq!(config.max_recursion_depth, 50);
    /// assert_eq!(config.max_string_length, 32768);
    /// assert!(!config.stop_at_first_match);
    /// assert!(config.enable_mime_types);
    /// assert_eq!(config.timeout_ms, Some(30000));
    /// ```
    #[must_use]
    pub const fn comprehensive() -> Self {
        Self {
            max_recursion_depth: 50,
            max_string_length: 32768,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(30000), // 30 seconds
        }
    }

    /// Validate the configuration settings
    ///
    /// Performs comprehensive security validation of all configuration values
    /// to prevent malicious configurations that could lead to resource exhaustion,
    /// denial of service, or other security issues.
    ///
    /// # Security
    ///
    /// This validation prevents:
    /// - Stack overflow attacks through excessive recursion depth
    /// - Memory exhaustion through oversized string limits
    /// - Denial of service through excessive timeouts
    /// - Integer overflow in configuration calculations
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::ConfigError` if any configuration values
    /// are invalid or out of reasonable bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::EvaluationConfig;
    ///
    /// let config = EvaluationConfig::default();
    /// assert!(config.validate().is_ok());
    ///
    /// let invalid_config = EvaluationConfig {
    ///     max_recursion_depth: 0, // Invalid: must be > 0
    ///     ..Default::default()
    /// };
    /// assert!(invalid_config.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<()> {
        self.validate_recursion_depth()?;
        self.validate_string_length()?;
        self.validate_timeout()?;
        self.validate_resource_combination()?;
        Ok(())
    }

    /// Validate recursion depth to prevent stack overflow attacks
    fn validate_recursion_depth(&self) -> Result<()> {
        const MAX_SAFE_RECURSION_DEPTH: u32 = 1000;

        if self.max_recursion_depth == 0 {
            return Err(LibmagicError::ConfigError {
                reason: "max_recursion_depth must be greater than 0".to_string(),
            });
        }

        if self.max_recursion_depth > MAX_SAFE_RECURSION_DEPTH {
            return Err(LibmagicError::ConfigError {
                reason: format!(
                    "max_recursion_depth must not exceed {MAX_SAFE_RECURSION_DEPTH} to prevent stack overflow"
                ),
            });
        }

        Ok(())
    }

    /// Validate string length to prevent memory exhaustion
    fn validate_string_length(&self) -> Result<()> {
        const MAX_SAFE_STRING_LENGTH: usize = 1_048_576; // 1MB

        if self.max_string_length == 0 {
            return Err(LibmagicError::ConfigError {
                reason: "max_string_length must be greater than 0".to_string(),
            });
        }

        if self.max_string_length > MAX_SAFE_STRING_LENGTH {
            return Err(LibmagicError::ConfigError {
                reason: format!(
                    "max_string_length must not exceed {MAX_SAFE_STRING_LENGTH} bytes to prevent memory exhaustion"
                ),
            });
        }

        Ok(())
    }

    /// Validate timeout to prevent denial of service
    fn validate_timeout(&self) -> Result<()> {
        const MAX_SAFE_TIMEOUT_MS: u64 = 300_000; // 5 minutes

        if let Some(timeout) = self.timeout_ms {
            if timeout == 0 {
                return Err(LibmagicError::ConfigError {
                    reason: "timeout_ms must be greater than 0 if specified".to_string(),
                });
            }

            if timeout > MAX_SAFE_TIMEOUT_MS {
                return Err(LibmagicError::ConfigError {
                    reason: format!(
                        "timeout_ms must not exceed {MAX_SAFE_TIMEOUT_MS} (5 minutes) to prevent denial of service"
                    ),
                });
            }
        }

        Ok(())
    }

    /// Validate resource combination to prevent resource exhaustion
    fn validate_resource_combination(&self) -> Result<()> {
        const HIGH_RECURSION_THRESHOLD: u32 = 100;
        const LARGE_STRING_THRESHOLD: usize = 65536;

        if self.max_recursion_depth > HIGH_RECURSION_THRESHOLD
            && self.max_string_length > LARGE_STRING_THRESHOLD
        {
            return Err(LibmagicError::ConfigError {
                reason: format!(
                    "High recursion depth (>{HIGH_RECURSION_THRESHOLD}) combined with large string length (>{LARGE_STRING_THRESHOLD}) may cause resource exhaustion"
                ),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Presets ──────────────────────────────────────────────────

    #[test]
    fn test_default_validates() {
        assert!(EvaluationConfig::default().validate().is_ok());
    }

    #[test]
    fn test_performance_validates() {
        assert!(EvaluationConfig::performance().validate().is_ok());
    }

    #[test]
    fn test_comprehensive_validates() {
        assert!(EvaluationConfig::comprehensive().validate().is_ok());
    }

    // ── Recursion depth boundaries ──────────────────────────────

    #[test]
    fn test_recursion_depth_zero_rejected() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_recursion_depth_one_accepted() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 1,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_recursion_depth_at_max_accepted() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 1000,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_recursion_depth_above_max_rejected() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 1001,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    // ── String length boundaries ────────────────────────────────

    #[test]
    fn test_string_length_zero_rejected() {
        let cfg = EvaluationConfig {
            max_string_length: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_string_length_one_accepted() {
        let cfg = EvaluationConfig {
            max_string_length: 1,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_string_length_at_max_accepted() {
        let cfg = EvaluationConfig {
            max_string_length: 1_048_576,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_string_length_above_max_rejected() {
        let cfg = EvaluationConfig {
            max_string_length: 1_048_577,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    // ── Timeout boundaries ──────────────────────────────────────

    #[test]
    fn test_timeout_none_accepted() {
        let cfg = EvaluationConfig {
            timeout_ms: None,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_timeout_zero_rejected() {
        let cfg = EvaluationConfig {
            timeout_ms: Some(0),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_timeout_one_accepted() {
        let cfg = EvaluationConfig {
            timeout_ms: Some(1),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_timeout_at_max_accepted() {
        let cfg = EvaluationConfig {
            timeout_ms: Some(300_000),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_timeout_above_max_rejected() {
        let cfg = EvaluationConfig {
            timeout_ms: Some(300_001),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    // ── Resource combination guard ──────────────────────────────

    #[test]
    fn test_high_recursion_with_large_string_rejected() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 101,
            max_string_length: 65537,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_high_recursion_with_normal_string_accepted() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 101,
            max_string_length: 65536,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_normal_recursion_with_large_string_accepted() {
        let cfg = EvaluationConfig {
            max_recursion_depth: 100,
            max_string_length: 65537,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    // ── evaluate_rules_with_config rejects invalid config ───────

    #[test]
    fn test_evaluate_rules_with_config_rejects_invalid() {
        use crate::evaluator::evaluate_rules_with_config;

        let invalid_cfg = EvaluationConfig {
            max_recursion_depth: 0,
            ..Default::default()
        };
        let result = evaluate_rules_with_config(&[], &[], &invalid_cfg);
        assert!(result.is_err());
    }
}
