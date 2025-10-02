//! Rust Libmagic - A pure-Rust implementation of libmagic
//!
//! This library provides safe, efficient file type identification through magic rule evaluation.
//! It parses magic files into an Abstract Syntax Tree (AST) and evaluates them against file
//! buffers using memory-mapped I/O for optimal performance.
//!
//! # Examples
//!
//! ```rust,no_run
//! use libmagic_rs::{MagicDatabase, EvaluationConfig};
//!
//! // Load magic rules from file
//! let db = MagicDatabase::load_from_file("magic.db")?;
//!
//! // Evaluate a file
//! let result = db.evaluate_file("sample.bin")?;
//! println!("File type: {}", result.description);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::path::Path;
use thiserror::Error;

// Re-export modules
pub mod evaluator;
pub mod io;
pub mod output;
pub mod parser;

// Re-export core AST types for convenience
pub use parser::ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};

// Re-export evaluator types for convenience
pub use evaluator::{EvaluationContext, MatchResult};

/// Core error types for the library
#[derive(Debug, Error)]
pub enum LibmagicError {
    /// Parse error in magic file
    #[error("Parse error at line {line}: {message}")]
    ParseError {
        /// Line number where error occurred
        line: usize,
        /// Error message
        message: String,
    },

    /// Evaluation error during rule processing
    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    /// I/O error accessing files
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid magic file format
    #[error("Invalid magic file format: {0}")]
    InvalidFormat(String),

    /// Evaluation timeout exceeded
    #[error("Evaluation timeout exceeded after {timeout_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },
}

/// Result type for library operations
pub type Result<T> = std::result::Result<T, LibmagicError>;

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
#[derive(Debug, Clone, PartialEq)]
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
    pub fn performance() -> Self {
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
    pub fn comprehensive() -> Self {
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
    /// Checks that all configuration values are within reasonable bounds.
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::InvalidFormat` if any configuration values
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
        if self.max_recursion_depth == 0 {
            return Err(LibmagicError::InvalidFormat(
                "max_recursion_depth must be greater than 0".to_string(),
            ));
        }

        if self.max_recursion_depth > 1000 {
            return Err(LibmagicError::InvalidFormat(
                "max_recursion_depth must not exceed 1000".to_string(),
            ));
        }

        if self.max_string_length == 0 {
            return Err(LibmagicError::InvalidFormat(
                "max_string_length must be greater than 0".to_string(),
            ));
        }

        if self.max_string_length > 1_048_576 {
            // 1MB limit
            return Err(LibmagicError::InvalidFormat(
                "max_string_length must not exceed 1MB".to_string(),
            ));
        }

        if let Some(timeout) = self.timeout_ms {
            if timeout == 0 {
                return Err(LibmagicError::InvalidFormat(
                    "timeout_ms must be greater than 0 if specified".to_string(),
                ));
            }

            if timeout > 300_000 {
                // 5 minute limit
                return Err(LibmagicError::InvalidFormat(
                    "timeout_ms must not exceed 300000 (5 minutes)".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Main interface for magic rule database
#[derive(Debug)]
#[allow(dead_code)] // Fields will be used in future implementation
pub struct MagicDatabase {
    rules: Vec<MagicRule>,
    config: EvaluationConfig,
}

impl MagicDatabase {
    /// Load magic rules from a file
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::IoError` if the file cannot be read.
    /// Returns `LibmagicError::ParseError` if the magic file format is invalid.
    pub fn load_from_file<P: AsRef<Path>>(_path: P) -> Result<Self> {
        // TODO: Implement magic file loading
        Ok(Self {
            rules: Vec::new(),
            config: EvaluationConfig::default(),
        })
    }

    /// Evaluate magic rules against a file
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::IoError` if the file cannot be accessed.
    /// Returns `LibmagicError::EvaluationError` if rule evaluation fails.
    pub fn evaluate_file<P: AsRef<Path>>(&self, _path: P) -> Result<EvaluationResult> {
        // TODO: Implement file evaluation
        Ok(EvaluationResult {
            description: "data".to_string(),
            mime_type: None,
            confidence: 0.0,
        })
    }
}

/// Result of magic rule evaluation
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    /// Human-readable file type description
    pub description: String,
    /// Optional MIME type
    pub mime_type: Option<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluation_config_default() {
        let config = EvaluationConfig::default();

        assert_eq!(config.max_recursion_depth, 20);
        assert_eq!(config.max_string_length, 8192);
        assert!(config.stop_at_first_match);
        assert!(!config.enable_mime_types);
        assert_eq!(config.timeout_ms, None);
    }

    #[test]
    fn test_evaluation_config_new() {
        let config = EvaluationConfig::new();
        let default_config = EvaluationConfig::default();

        assert_eq!(config, default_config);
    }

    #[test]
    fn test_evaluation_config_performance() {
        let config = EvaluationConfig::performance();

        assert_eq!(config.max_recursion_depth, 10);
        assert_eq!(config.max_string_length, 1024);
        assert!(config.stop_at_first_match);
        assert!(!config.enable_mime_types);
        assert_eq!(config.timeout_ms, Some(1000));
    }

    #[test]
    fn test_evaluation_config_comprehensive() {
        let config = EvaluationConfig::comprehensive();

        assert_eq!(config.max_recursion_depth, 50);
        assert_eq!(config.max_string_length, 32768);
        assert!(!config.stop_at_first_match);
        assert!(config.enable_mime_types);
        assert_eq!(config.timeout_ms, Some(30000));
    }

    #[test]
    fn test_evaluation_config_validate_valid() {
        let config = EvaluationConfig::default();
        assert!(config.validate().is_ok());

        let performance_config = EvaluationConfig::performance();
        assert!(performance_config.validate().is_ok());

        let comprehensive_config = EvaluationConfig::comprehensive();
        assert!(comprehensive_config.validate().is_ok());
    }

    #[test]
    fn test_evaluation_config_validate_zero_recursion_depth() {
        let config = EvaluationConfig {
            max_recursion_depth: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("max_recursion_depth must be greater than 0"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_excessive_recursion_depth() {
        let config = EvaluationConfig {
            max_recursion_depth: 1001,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("max_recursion_depth must not exceed 1000"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_zero_string_length() {
        let config = EvaluationConfig {
            max_string_length: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("max_string_length must be greater than 0"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_excessive_string_length() {
        let config = EvaluationConfig {
            max_string_length: 1_048_577, // 1MB + 1
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("max_string_length must not exceed 1MB"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_zero_timeout() {
        let config = EvaluationConfig {
            timeout_ms: Some(0),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("timeout_ms must be greater than 0 if specified"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_excessive_timeout() {
        let config = EvaluationConfig {
            timeout_ms: Some(300_001), // 5 minutes + 1ms
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("timeout_ms must not exceed 300000"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_evaluation_config_validate_boundary_values() {
        // Test minimum valid values
        let min_config = EvaluationConfig {
            max_recursion_depth: 1,
            max_string_length: 1,
            timeout_ms: Some(1),
            ..Default::default()
        };
        assert!(min_config.validate().is_ok());

        // Test maximum valid values
        let max_config = EvaluationConfig {
            max_recursion_depth: 1000,
            max_string_length: 1_048_576, // 1MB
            timeout_ms: Some(300_000),    // 5 minutes
            ..Default::default()
        };
        assert!(max_config.validate().is_ok());
    }

    #[test]
    fn test_evaluation_config_clone() {
        let config = EvaluationConfig {
            max_recursion_depth: 15,
            max_string_length: 4096,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(5000),
        };

        let cloned_config = config.clone();
        assert_eq!(config, cloned_config);
    }

    #[test]
    fn test_evaluation_config_debug() {
        let config = EvaluationConfig::default();
        let debug_str = format!("{config:?}");

        assert!(debug_str.contains("EvaluationConfig"));
        assert!(debug_str.contains("max_recursion_depth"));
        assert!(debug_str.contains("max_string_length"));
        assert!(debug_str.contains("stop_at_first_match"));
        assert!(debug_str.contains("enable_mime_types"));
        assert!(debug_str.contains("timeout_ms"));
    }

    #[test]
    fn test_evaluation_config_partial_eq() {
        let config1 = EvaluationConfig::default();
        let config2 = EvaluationConfig::default();
        let config3 = EvaluationConfig::performance();

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_evaluation_config_custom_values() {
        let config = EvaluationConfig {
            max_recursion_depth: 25,
            max_string_length: 16384,
            stop_at_first_match: false,
            enable_mime_types: true,
            timeout_ms: Some(10000),
        };

        assert_eq!(config.max_recursion_depth, 25);
        assert_eq!(config.max_string_length, 16384);
        assert!(!config.stop_at_first_match);
        assert!(config.enable_mime_types);
        assert_eq!(config.timeout_ms, Some(10000));

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_libmagic_error_timeout() {
        let error = LibmagicError::Timeout { timeout_ms: 5000 };
        let error_str = error.to_string();

        assert!(error_str.contains("Evaluation timeout exceeded"));
        assert!(error_str.contains("5000ms"));
    }

    #[test]
    fn test_libmagic_error_timeout_debug() {
        let error = LibmagicError::Timeout { timeout_ms: 1000 };
        let debug_str = format!("{error:?}");

        assert!(debug_str.contains("Timeout"));
        assert!(debug_str.contains("1000"));
    }
}
