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
mod config;
pub mod error;
pub mod evaluator;
pub mod io;
pub mod mime;
pub mod output;
pub mod parser;
pub mod tags;

pub use config::EvaluationConfig;

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
        let mut rules = crate::builtin_rules::get_builtin_rules();
        crate::evaluator::strength::sort_rules_by_strength_recursive(&mut rules);
        Ok(Self {
            rules,
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
        let mut rules = parser::load_magic_file(path.as_ref()).map_err(|e| match e {
            ParseError::IoError(io_err) => LibmagicError::IoError(io_err),
            other => LibmagicError::ParseError(other),
        })?;
        crate::evaluator::strength::sort_rules_by_strength_recursive(&mut rules);

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
    /// # Security
    ///
    /// This method has a time-of-check/time-of-use (TOCTOU) window between
    /// resolving the path and memory-mapping the file
    /// ([CWE-367](https://cwe.mitre.org/data/definitions/367.html)).
    /// Internally, `evaluate_file` first calls `std::fs::metadata(path)` to
    /// detect the empty-file case, then opens and memory-maps the file via
    /// [`io::FileBuffer::new`], which itself re-validates file metadata
    /// (regular file, size bounds) before calling `create_memory_mapping`.
    /// Between these validation steps and the final `mmap` call, the path
    /// may be swapped (for example, via a symlink replacement or rename)
    /// by another process. The content that gets mapped may therefore
    /// differ from the file that passed validation.
    ///
    /// The I/O layer mitigates the common shapes of this attack by
    /// canonicalizing the path and rejecting special file types, and the
    /// mapping itself is read-only, so a successful exploit cannot corrupt
    /// the victim file. The residual risk is that `evaluate_file` may
    /// classify a different file than the caller intended.
    ///
    /// **For adversarial or untrusted environments, prefer
    /// [`MagicDatabase::evaluate_buffer`]**: load the bytes yourself using
    /// whatever resource-bounded, TOCTOU-aware I/O strategy your
    /// application requires (e.g., `openat` with `O_NOFOLLOW`, holding an
    /// open file descriptor across validation and read), then pass the
    /// in-memory slice directly to `evaluate_buffer`. See
    /// [the security assurance case](https://evilbit-labs.github.io/libmagic-rs/security-assurance.html)
    /// for the residual-risk discussion.
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

        // Load the file into memory. Reuse the metadata we just read instead
        // of having FileBuffer::new call canonicalize+metadata again.
        let file_buffer = FileBuffer::from_path_and_metadata(path, &file_metadata)?;
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
            self.mime_mapper
                .get_mime_type(&description)
                .map(String::from)
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
/// # Relationship to [`crate::output::EvaluationResult`]
///
/// This is the **library-facing** result type returned by [`MagicDatabase::evaluate_file`]
/// and [`MagicDatabase::evaluate_buffer`].
/// It carries a rolled-up description, MIME type, and confidence score along with
/// raw [`evaluator::RuleMatch`] values. It intentionally does **not** carry the
/// analyzed filename or a surface-level error string, because those are caller
/// concerns (a caller may evaluate an in-memory buffer that has no filename).
///
/// The parallel type [`crate::output::EvaluationResult`] is the **output-facing**
/// result used by the CLI and JSON/text formatters. It adds `filename` and
/// `error`, carries enriched [`crate::output::MatchResult`] values (with
/// extracted tags), and uses `u32` counters in its metadata to match the JSON
/// output schema.
///
/// The two types are **intentionally distinct** — do not try to unify them.
/// Convert library → output explicitly via
/// [`crate::output::EvaluationResult::from_library_result`], which is the single
/// named conversion point. Any drift between the two hierarchies should be
/// resolved there, not by back-channel field copying in call sites.
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
