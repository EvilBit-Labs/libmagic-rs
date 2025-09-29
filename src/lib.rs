//! Rust Libmagic - A pure-Rust implementation of libmagic
//!
//! This library provides safe, efficient file type identification through magic rule evaluation.
//! It parses magic files into an Abstract Syntax Tree (AST) and evaluates them against file
//! buffers using memory-mapped I/O for optimal performance.
//!
//! # Examples
//!
//! ```rust,no_run
//! use rust_libmagic::{MagicDatabase, EvaluationConfig};
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
}

/// Result type for library operations
pub type Result<T> = std::result::Result<T, LibmagicError>;

/// Configuration for rule evaluation
#[derive(Debug, Clone)]
pub struct EvaluationConfig {
    /// Maximum recursion depth for nested rules
    pub max_recursion_depth: u32,
    /// Maximum string length to read
    pub max_string_length: usize,
    /// Stop at first match or continue for all matches
    pub stop_at_first_match: bool,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            max_recursion_depth: 20,
            max_string_length: 8192,
            stop_at_first_match: true,
        }
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
