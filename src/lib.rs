// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Rust Libmagic - A pure-Rust implementation of libmagic
//!
//! This library provides safe, efficient file type identification through magic rule evaluation.
//! It parses magic files into an Abstract Syntax Tree (AST) and evaluates them against file
//! buffers using memory-mapped I/O for optimal performance.
//!
//! # Security Features
//!
//! This implementation prioritizes security through:
//! - **Memory Safety**: Pure Rust with no unsafe code (except in vetted dependencies)
//! - **Bounds Checking**: Comprehensive validation of all buffer accesses
//! - **Resource Limits**: Configurable limits to prevent resource exhaustion attacks
//! - **Input Validation**: Strict validation of magic files and configuration
//! - **Error Handling**: Secure error messages that don't leak sensitive information
//! - **Timeout Protection**: Configurable timeouts to prevent denial of service
//!
//! # Examples
//!
//! ## Complete Workflow: Load → Evaluate → Output
//!
//! ```rust,no_run
//! use libmagic_rs::MagicDatabase;
//!
//! // Load magic rules from a text file
//! let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
//!
//! // Evaluate a file to determine its type
//! let result = db.evaluate_file("sample.bin")?;
//! println!("File type: {}", result.description);
//!
//! // Access metadata about loaded rules
//! if let Some(path) = db.source_path() {
//!     println!("Rules loaded from: {}", path.display());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Loading from a Directory
//!
//! ```rust,no_run
//! use libmagic_rs::MagicDatabase;
//!
//! // Load all magic files from a directory (Magdir pattern)
//! let db = MagicDatabase::load_from_file("/usr/share/misc/magic.d")?;
//!
//! // Evaluate multiple files
//! for file in &["file1.bin", "file2.bin", "file3.bin"] {
//!     let result = db.evaluate_file(file)?;
//!     println!("{}: {}", file, result.description);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Error Handling for Binary Files
//!
//! ```rust,no_run
//! use libmagic_rs::MagicDatabase;
//!
//! // Attempt to load a binary .mgc file
//! match MagicDatabase::load_from_file("/usr/share/misc/magic.mgc") {
//!     Ok(db) => {
//!         let result = db.evaluate_file("sample.bin")?;
//!         println!("File type: {}", result.description);
//!     }
//!     Err(e) => {
//!         eprintln!("Error loading magic file: {}", e);
//!         eprintln!("Hint: Binary .mgc files are not supported.");
//!         eprintln!("Use --use-builtin option to use built-in rules,");
//!         eprintln!("or provide a text-based magic file or directory.");
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Debugging with Source Path Metadata
//!
//! ```rust,no_run
//! use libmagic_rs::MagicDatabase;
//!
//! let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
//!
//! // Use source_path() for debugging and logging
//! if let Some(source) = db.source_path() {
//!     println!("Loaded {} from {}",
//!              "magic rules",
//!              source.display());
//! }
//!
//! // Evaluate files with source tracking
//! let result = db.evaluate_file("sample.bin")?;
//! println!("Detection result: {}", result.description);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// Re-export modules
pub mod builtin_rules;
pub mod error;
pub mod evaluator;
pub mod io;
pub mod mime;
pub mod output;
pub mod parser;
pub mod tags;

/// Build-time helpers for compiling magic rules.
///
/// This module contains functionality used by the build script to parse magic files
/// and generate Rust code for built-in rules. It is only available during tests and
/// documentation builds to enable comprehensive testing of the build process.
#[cfg(any(test, doc))]
pub mod build_helpers;

// Re-export core AST types for convenience
pub use parser::ast::{
    Endianness, MagicRule, OffsetSpec, Operator, PStringLengthWidth, StrengthModifier, TypeKind,
    Value,
};

// Re-export evaluator types for convenience
pub use evaluator::{EvaluationContext, RuleMatch};

// Re-export error types for convenience
pub use error::{EvaluationError, LibmagicError, ParseError};

/// Result type for library operations
pub type Result<T> = std::result::Result<T, LibmagicError>;

impl From<crate::io::IoError> for LibmagicError {
    fn from(err: crate::io::IoError) -> Self {
        // Preserve the structured error message (includes path and operation context)
        LibmagicError::FileError(err.to_string())
    }
}

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

/// Main interface for magic rule database
#[derive(Debug)]
pub struct MagicDatabase {
    rules: Vec<MagicRule>,
    config: EvaluationConfig,
    /// Optional path to the source magic file or directory from which rules were loaded.
    /// This is used for debugging and logging purposes.
    source_path: Option<PathBuf>,
    /// Cached MIME type mapper to avoid rebuilding the lookup table on every evaluation
    mime_mapper: mime::MimeMapper,
}

impl MagicDatabase {
    /// Create a database using built-in magic rules.
    ///
    /// Loads magic rules that are compiled into the library binary at build time
    /// from `src/builtin_rules.magic`. These rules provide high-confidence detection
    /// for common file types including executables (ELF, PE/DOS), archives (ZIP, TAR,
    /// GZIP), images (JPEG, PNG, GIF, BMP), and documents (PDF).
    ///
    /// # Errors
    ///
    /// Currently always returns `Ok`. In future implementations, this may return
    /// an error if the built-in rules fail to load or validate.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::MagicDatabase;
    ///
    /// let db = MagicDatabase::with_builtin_rules()?;
    /// let result = db.evaluate_buffer(b"\x7fELF")?;
    /// // Returns actual file type detection (e.g., "ELF")
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_builtin_rules() -> Result<Self> {
        Self::with_builtin_rules_and_config(EvaluationConfig::default())
    }

    /// Create database with built-in rules and custom configuration.
    ///
    /// Loads built-in magic rules compiled at build time and applies the specified
    /// evaluation configuration (e.g., custom timeout settings).
    ///
    /// # Arguments
    ///
    /// * `config` - Custom evaluation configuration to use with the built-in rules
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError` if the configuration is invalid (e.g., timeout is zero).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::{MagicDatabase, EvaluationConfig};
    ///
    /// let config = EvaluationConfig {
    ///     timeout_ms: Some(5000), // 5 second timeout
    ///     ..EvaluationConfig::default()
    /// };
    /// let db = MagicDatabase::with_builtin_rules_and_config(config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_builtin_rules_and_config(config: EvaluationConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            rules: crate::builtin_rules::get_builtin_rules(),
            config,
            source_path: None,
            mime_mapper: mime::MimeMapper::new(),
        })
    }

    /// Load magic rules from a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the magic file to load
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::IoError` if the file cannot be read.
    /// Returns `LibmagicError::ParseError` if the magic file format is invalid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::MagicDatabase;
    ///
    /// let db = MagicDatabase::load_from_file("magic.db")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load_from_file_with_config(path, EvaluationConfig::default())
    }

    /// Load from file with custom config (e.g., timeout)
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be read, parsed, or config is invalid
    pub fn load_from_file_with_config<P: AsRef<Path>>(
        path: P,
        config: EvaluationConfig,
    ) -> Result<Self> {
        config.validate()?;
        let rules = parser::load_magic_file(path.as_ref()).map_err(|e| match e {
            ParseError::IoError(io_err) => LibmagicError::IoError(io_err),
            other => LibmagicError::ParseError(other),
        })?;

        Ok(Self {
            rules,
            config,
            source_path: Some(path.as_ref().to_path_buf()),
            mime_mapper: mime::MimeMapper::new(),
        })
    }

    /// Evaluate magic rules against a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to evaluate
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::IoError` if the file cannot be accessed.
    /// Returns `LibmagicError::EvaluationError` if rule evaluation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::MagicDatabase;
    ///
    /// let db = MagicDatabase::load_from_file("magic.db")?;
    /// let result = db.evaluate_file("sample.bin")?;
    /// println!("File type: {}", result.description);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult> {
        use crate::evaluator::evaluate_rules_with_config;
        use crate::io::FileBuffer;
        use std::fs;
        use std::time::Instant;

        let start_time = Instant::now();
        let path = path.as_ref();

        // Check if file is empty - if so, evaluate as empty buffer
        // This allows empty files to be processed like any other file
        let file_metadata = fs::metadata(path)?;
        let file_size = file_metadata.len();

        if file_size == 0 {
            // Empty file - evaluate as empty buffer but preserve file metadata
            let mut result = self.evaluate_buffer_internal(b"", start_time)?;
            result.metadata.file_size = 0;
            result.metadata.magic_file.clone_from(&self.source_path);
            return Ok(result);
        }

        // Load the file into memory
        let file_buffer = FileBuffer::new(path)?;
        let buffer = file_buffer.as_slice();

        // Evaluate rules against the file buffer (build_result handles empty rules/matches)
        let matches = if self.rules.is_empty() {
            vec![]
        } else {
            evaluate_rules_with_config(&self.rules, buffer, &self.config)?
        };

        Ok(self.build_result(matches, file_size, start_time))
    }

    /// Evaluate magic rules against an in-memory buffer
    ///
    /// This method evaluates a byte buffer directly without reading from disk,
    /// which is useful for stdin input or pre-loaded data.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Byte buffer to evaluate
    ///
    /// # Errors
    ///
    /// Returns `LibmagicError::EvaluationError` if rule evaluation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::MagicDatabase;
    ///
    /// let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
    /// let buffer = b"test data";
    /// let result = db.evaluate_buffer(buffer)?;
    /// println!("Buffer type: {}", result.description);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn evaluate_buffer(&self, buffer: &[u8]) -> Result<EvaluationResult> {
        use std::time::Instant;
        self.evaluate_buffer_internal(buffer, Instant::now())
    }

    /// Internal buffer evaluation with externally provided start time
    fn evaluate_buffer_internal(
        &self,
        buffer: &[u8],
        start_time: std::time::Instant,
    ) -> Result<EvaluationResult> {
        use crate::evaluator::evaluate_rules_with_config;

        let file_size = buffer.len() as u64;

        let matches = if self.rules.is_empty() {
            vec![]
        } else {
            evaluate_rules_with_config(&self.rules, buffer, &self.config)?
        };

        Ok(self.build_result(matches, file_size, start_time))
    }

    /// Build an `EvaluationResult` from match results, file size, and start time.
    ///
    /// This is shared between `evaluate_file` and `evaluate_buffer_internal` to
    /// avoid duplicating the result-construction logic.
    fn build_result(
        &self,
        matches: Vec<evaluator::RuleMatch>,
        file_size: u64,
        start_time: std::time::Instant,
    ) -> EvaluationResult {
        let (description, confidence) = if matches.is_empty() {
            ("data".to_string(), 0.0)
        } else {
            (
                Self::concatenate_messages(&matches),
                matches.first().map_or(0.0, |m| m.confidence),
            )
        };

        let mime_type = if self.config.enable_mime_types {
            self.mime_mapper.get_mime_type(&description)
        } else {
            None
        };

        EvaluationResult {
            description,
            mime_type,
            confidence,
            matches,
            metadata: EvaluationMetadata {
                file_size,
                evaluation_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                rules_evaluated: self.rules.len(),
                magic_file: self.source_path.clone(),
                timed_out: false,
            },
        }
    }

    /// Concatenate match messages following libmagic behavior
    ///
    /// Messages are joined with spaces, except when a message starts with
    /// backspace character (\\b) which suppresses the space.
    fn concatenate_messages(matches: &[evaluator::RuleMatch]) -> String {
        let capacity: usize = matches.iter().map(|m| m.message.len() + 1).sum();
        let mut result = String::with_capacity(capacity);
        for m in matches {
            if let Some(rest) = m.message.strip_prefix('\u{0008}') {
                // Backspace suppresses the space and the character itself
                result.push_str(rest);
            } else if !result.is_empty() {
                result.push(' ');
                result.push_str(&m.message);
            } else {
                result.push_str(&m.message);
            }
        }
        result
    }

    /// Returns the evaluation configuration used by this database.
    ///
    /// This provides read-only access to the evaluation configuration for
    /// callers that need to inspect resource limits or evaluation options.
    #[must_use]
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Returns the path from which magic rules were loaded.
    ///
    /// This method returns the source path that was used to load the magic rules
    /// into this database. It is useful for debugging, logging, and tracking the
    /// origin of magic rules.
    ///
    /// # Returns
    ///
    /// - `Some(&Path)` - If the database was loaded from a file or directory using
    ///   [`load_from_file()`](Self::load_from_file)
    /// - `None` - If the database was constructed programmatically or the source
    ///   path was not recorded
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use libmagic_rs::MagicDatabase;
    ///
    /// let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
    /// if let Some(path) = db.source_path() {
    ///     println!("Rules loaded from: {}", path.display());
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

/// Metadata about the evaluation process
///
/// Contains diagnostic information about how the evaluation was performed,
/// including performance metrics and statistics about rule processing.
///
/// # Examples
///
/// ```
/// use libmagic_rs::EvaluationMetadata;
/// use std::path::PathBuf;
///
/// let metadata = EvaluationMetadata {
///     file_size: 8192,
///     evaluation_time_ms: 2.5,
///     rules_evaluated: 42,
///     magic_file: Some(PathBuf::from("/usr/share/misc/magic")),
///     timed_out: false,
/// };
///
/// assert_eq!(metadata.file_size, 8192);
/// assert!(!metadata.timed_out);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetadata {
    /// Size of the analyzed file or buffer in bytes
    pub file_size: u64,
    /// Time taken to evaluate rules in milliseconds
    pub evaluation_time_ms: f64,
    /// Number of top-level rules that were evaluated
    pub rules_evaluated: usize,
    /// Path to the magic file used, or None for built-in rules
    pub magic_file: Option<PathBuf>,
    /// Whether evaluation was stopped due to timeout
    pub timed_out: bool,
}

impl Default for EvaluationMetadata {
    fn default() -> Self {
        Self {
            file_size: 0,
            evaluation_time_ms: 0.0,
            rules_evaluated: 0,
            magic_file: None,
            timed_out: false,
        }
    }
}

/// Result of magic rule evaluation
///
/// Contains the file type description, optional MIME type, confidence score,
/// individual match details, and evaluation metadata.
///
/// # Examples
///
/// ```
/// use libmagic_rs::{EvaluationResult, EvaluationMetadata};
///
/// let result = EvaluationResult {
///     description: "ELF 64-bit executable".to_string(),
///     mime_type: Some("application/x-executable".to_string()),
///     confidence: 0.9,
///     matches: vec![],
///     metadata: EvaluationMetadata::default(),
/// };
///
/// assert_eq!(result.description, "ELF 64-bit executable");
/// assert!(result.confidence > 0.5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Human-readable file type description
    ///
    /// This is the concatenated message from all matching rules,
    /// following libmagic behavior where hierarchical matches
    /// are joined with spaces (unless backspace character is used).
    pub description: String,
    /// Optional MIME type for the detected file type
    ///
    /// Only populated when `enable_mime_types` is set in the configuration.
    pub mime_type: Option<String>,
    /// Confidence score (0.0 to 1.0)
    ///
    /// Based on the depth of the match in the rule hierarchy.
    /// Deeper matches indicate more specific identification.
    pub confidence: f64,
    /// Individual match results from rule evaluation
    ///
    /// Contains details about each rule that matched, including
    /// offset, matched value, and per-match confidence.
    pub matches: Vec<evaluator::RuleMatch>,
    /// Metadata about the evaluation process
    pub metadata: EvaluationMetadata,
}

#[cfg(test)]
mod tests;
