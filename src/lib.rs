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

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

// Re-export modules
pub mod evaluator;
pub mod io;
pub mod output;
pub mod parser;

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

/// Magic rule representation in the AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicRule {
    /// Offset specification for where to read data
    pub offset: OffsetSpec,
    /// Type of data to read and interpret
    pub typ: TypeKind,
    /// Comparison operator to apply
    pub op: Operator,
    /// Expected value for comparison
    pub value: Value,
    /// Human-readable message for this rule
    pub message: String,
    /// Child rules that are evaluated if this rule matches
    pub children: Vec<MagicRule>,
    /// Indentation level for hierarchical rules
    pub level: u32,
}

/// Offset specification for locating data in files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OffsetSpec {
    /// Absolute offset from file start
    Absolute(i64),
    /// Indirect offset through pointer dereferencing
    Indirect {
        /// Base offset to read pointer from
        base_offset: i64,
        /// Type of pointer value
        pointer_type: TypeKind,
        /// Adjustment to add to pointer value
        adjustment: i64,
        /// Endianness for pointer reading
        endian: Endianness,
    },
    /// Relative offset from previous match
    Relative(i64),
    /// Offset from end of file
    FromEnd(i64),
}

/// Data type specifications for interpreting bytes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind {
    /// Single byte
    Byte,
    /// 16-bit integer
    Short {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// 32-bit integer
    Long {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// String data
    String {
        /// Maximum length to read
        max_length: Option<usize>,
    },
}

/// Comparison and bitwise operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operator {
    /// Equality comparison
    Equal,
    /// Inequality comparison
    NotEqual,
    /// Bitwise AND operation
    BitwiseAnd,
}

/// Value types for rule matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// Unsigned integer value
    Uint(u64),
    /// Signed integer value
    Int(i64),
    /// Byte sequence
    Bytes(Vec<u8>),
    /// String value
    String(String),
}

/// Endianness specification
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Endianness {
    /// Little-endian byte order
    Little,
    /// Big-endian byte order
    Big,
    /// Native system byte order
    Native,
}

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
