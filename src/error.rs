//! Error types for the libmagic-rs library.
//!
//! This module defines the error types used throughout the library for
//! consistent error handling and reporting.

/// Main error type for the libmagic-rs library.
///
/// This enum represents all possible errors that can occur during
/// magic file parsing, rule evaluation, and file I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum LibmagicError {
    /// Error that occurred during magic file parsing.
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),

    /// Error that occurred during rule evaluation.
    #[error("Evaluation error: {0}")]
    EvaluationError(#[from] EvaluationError),

    /// I/O error that occurred during file operations.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Evaluation timeout exceeded.
    #[error("Evaluation timeout exceeded after {timeout_ms}ms")]
    Timeout {
        /// The timeout duration in milliseconds
        timeout_ms: u64,
    },

    /// Invalid configuration parameter.
    #[error("Configuration error: {reason}")]
    ConfigError {
        /// Description of the configuration issue
        reason: String,
    },

    /// File I/O error with structured context (path, operation).
    ///
    /// Unlike `IoError` which wraps a generic `std::io::Error`, this variant
    /// preserves the structured error information from file I/O operations
    /// (e.g., file path, operation type).
    #[error("File error: {0}")]
    FileError(String),
}

/// Errors that can occur during magic file parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Invalid syntax in magic file.
    #[error("Invalid syntax at line {line}: {message}")]
    InvalidSyntax {
        /// The line number where the error occurred
        line: usize,
        /// The error message describing the syntax issue
        message: String,
    },

    /// Unsupported magic file feature.
    #[error("Unsupported feature at line {line}: {feature}")]
    UnsupportedFeature {
        /// The line number where the error occurred
        line: usize,
        /// The name of the unsupported feature
        feature: String,
    },

    /// Invalid offset specification.
    #[error("Invalid offset specification at line {line}: {offset}")]
    InvalidOffset {
        /// The line number where the error occurred
        line: usize,
        /// The invalid offset specification
        offset: String,
    },

    /// Invalid type specification.
    #[error("Invalid type specification at line {line}: {type_spec}")]
    InvalidType {
        /// The line number where the error occurred
        line: usize,
        /// The invalid type specification
        type_spec: String,
    },

    /// Invalid operator specification.
    #[error("Invalid operator at line {line}: {operator}")]
    InvalidOperator {
        /// The line number where the error occurred
        line: usize,
        /// The invalid operator
        operator: String,
    },

    /// Invalid value specification.
    #[error("Invalid value at line {line}: {value}")]
    InvalidValue {
        /// The line number where the error occurred
        line: usize,
        /// The invalid value specification
        value: String,
    },

    /// Unsupported magic file format.
    #[error("Unsupported format at line {line}: {format_type}\n{message}")]
    UnsupportedFormat {
        /// The line number where the error occurred
        line: usize,
        /// The type of unsupported format
        format_type: String,
        /// Detailed message with guidance
        message: String,
    },

    /// I/O error occurred during file operations.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Errors that can occur during rule evaluation.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// Buffer overrun during file reading.
    #[error("Buffer overrun at offset {offset}")]
    BufferOverrun {
        /// The offset where the buffer overrun occurred
        offset: usize,
    },

    /// Invalid offset calculation.
    #[error("Invalid offset: {offset}")]
    InvalidOffset {
        /// The invalid offset value
        offset: i64,
    },

    /// Unsupported type during evaluation.
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// The name of the unsupported type
        type_name: String,
    },

    /// Recursion limit exceeded during evaluation.
    #[error("Recursion limit exceeded (depth: {depth})")]
    RecursionLimitExceeded {
        /// The recursion depth that was exceeded
        depth: u32,
    },

    /// String length limit exceeded.
    #[error("String length limit exceeded: {length} > {max_length}")]
    StringLengthExceeded {
        /// The actual string length
        length: usize,
        /// The maximum allowed length
        max_length: usize,
    },

    /// Invalid string encoding.
    #[error("Invalid string encoding at offset {offset}")]
    InvalidStringEncoding {
        /// The offset where the invalid encoding was found
        offset: usize,
    },

    /// Evaluation timeout exceeded.
    #[error("Evaluation timeout exceeded after {timeout_ms}ms")]
    Timeout {
        /// The timeout duration in milliseconds
        timeout_ms: u64,
    },

    /// Type reading error during evaluation.
    #[error("Type reading error: {0}")]
    TypeReadError(#[from] crate::evaluator::types::TypeReadError),

    /// Internal error indicating a bug in the evaluation logic.
    #[error("Internal error: {message}")]
    InternalError {
        /// Description of the internal error
        message: String,
    },
}

impl ParseError {
    /// Create a new `InvalidSyntax` error.
    #[must_use]
    pub fn invalid_syntax(line: usize, message: impl Into<String>) -> Self {
        Self::InvalidSyntax {
            line,
            message: message.into(),
        }
    }

    /// Create a new `UnsupportedFeature` error.
    #[must_use]
    pub fn unsupported_feature(line: usize, feature: impl Into<String>) -> Self {
        Self::UnsupportedFeature {
            line,
            feature: feature.into(),
        }
    }

    /// Create a new `InvalidOffset` error.
    #[must_use]
    pub fn invalid_offset(line: usize, offset: impl Into<String>) -> Self {
        Self::InvalidOffset {
            line,
            offset: offset.into(),
        }
    }

    /// Create a new `InvalidType` error.
    #[must_use]
    pub fn invalid_type(line: usize, type_spec: impl Into<String>) -> Self {
        Self::InvalidType {
            line,
            type_spec: type_spec.into(),
        }
    }

    /// Create a new `InvalidOperator` error.
    #[must_use]
    pub fn invalid_operator(line: usize, operator: impl Into<String>) -> Self {
        Self::InvalidOperator {
            line,
            operator: operator.into(),
        }
    }

    /// Create a new `InvalidValue` error.
    #[must_use]
    pub fn invalid_value(line: usize, value: impl Into<String>) -> Self {
        Self::InvalidValue {
            line,
            value: value.into(),
        }
    }

    /// Create a new `UnsupportedFormat` error.
    #[must_use]
    pub fn unsupported_format(
        line: usize,
        format_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::UnsupportedFormat {
            line,
            format_type: format_type.into(),
            message: message.into(),
        }
    }
}

impl EvaluationError {
    /// Create a new `BufferOverrun` error.
    #[must_use]
    pub fn buffer_overrun(offset: usize) -> Self {
        Self::BufferOverrun { offset }
    }

    /// Create a new `InvalidOffset` error.
    #[must_use]
    pub fn invalid_offset(offset: i64) -> Self {
        Self::InvalidOffset { offset }
    }

    /// Create a new `UnsupportedType` error.
    #[must_use]
    pub fn unsupported_type(type_name: impl Into<String>) -> Self {
        Self::UnsupportedType {
            type_name: type_name.into(),
        }
    }

    /// Create a new `RecursionLimitExceeded` error.
    #[must_use]
    pub fn recursion_limit_exceeded(depth: u32) -> Self {
        Self::RecursionLimitExceeded { depth }
    }

    /// Create a new `StringLengthExceeded` error.
    #[must_use]
    pub fn string_length_exceeded(length: usize, max_length: usize) -> Self {
        Self::StringLengthExceeded { length, max_length }
    }

    /// Create a new `InvalidStringEncoding` error.
    #[must_use]
    pub fn invalid_string_encoding(offset: usize) -> Self {
        Self::InvalidStringEncoding { offset }
    }

    /// Create a new `Timeout` error.
    #[must_use]
    pub fn timeout(timeout_ms: u64) -> Self {
        Self::Timeout { timeout_ms }
    }

    /// Create a new `InternalError` error.
    #[must_use]
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_libmagic_error_from_parse_error() {
        let parse_error = ParseError::invalid_syntax(10, "unexpected token");
        let libmagic_error = LibmagicError::from(parse_error);

        match libmagic_error {
            LibmagicError::ParseError(_) => (),
            _ => panic!("Expected ParseError variant"),
        }
    }

    #[test]
    fn test_libmagic_error_from_evaluation_error() {
        let eval_error = EvaluationError::buffer_overrun(100);
        let libmagic_error = LibmagicError::from(eval_error);

        match libmagic_error {
            LibmagicError::EvaluationError(_) => (),
            _ => panic!("Expected EvaluationError variant"),
        }
    }

    #[test]
    fn test_libmagic_error_from_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let libmagic_error = LibmagicError::from(io_error);

        match libmagic_error {
            LibmagicError::IoError(_) => (),
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_parse_error_display() {
        let error = ParseError::invalid_syntax(5, "missing operator");
        let display = format!("{error}");
        assert_eq!(display, "Invalid syntax at line 5: missing operator");
    }

    #[test]
    fn test_parse_error_unsupported_feature() {
        let error = ParseError::unsupported_feature(12, "regex patterns");
        let display = format!("{error}");
        assert_eq!(display, "Unsupported feature at line 12: regex patterns");
    }

    #[test]
    fn test_parse_error_invalid_offset() {
        let error = ParseError::invalid_offset(8, "invalid_offset_spec");
        let display = format!("{error}");
        assert_eq!(
            display,
            "Invalid offset specification at line 8: invalid_offset_spec"
        );
    }

    #[test]
    fn test_parse_error_invalid_type() {
        let error = ParseError::invalid_type(15, "unknown_type");
        let display = format!("{error}");
        assert_eq!(
            display,
            "Invalid type specification at line 15: unknown_type"
        );
    }

    #[test]
    fn test_parse_error_invalid_operator() {
        let error = ParseError::invalid_operator(20, "??");
        let display = format!("{error}");
        assert_eq!(display, "Invalid operator at line 20: ??");
    }

    #[test]
    fn test_parse_error_invalid_value() {
        let error = ParseError::invalid_value(25, "malformed_hex");
        let display = format!("{error}");
        assert_eq!(display, "Invalid value at line 25: malformed_hex");
    }

    #[test]
    fn test_evaluation_error_buffer_overrun() {
        let error = EvaluationError::buffer_overrun(1024);
        let display = format!("{error}");
        assert_eq!(display, "Buffer overrun at offset 1024");
    }

    #[test]
    fn test_evaluation_error_invalid_offset() {
        let error = EvaluationError::invalid_offset(-50);
        let display = format!("{error}");
        assert_eq!(display, "Invalid offset: -50");
    }

    #[test]
    fn test_evaluation_error_unsupported_type() {
        let error = EvaluationError::unsupported_type("complex_type");
        let display = format!("{error}");
        assert_eq!(display, "Unsupported type: complex_type");
    }

    #[test]
    fn test_evaluation_error_recursion_limit() {
        let error = EvaluationError::recursion_limit_exceeded(100);
        let display = format!("{error}");
        assert_eq!(display, "Recursion limit exceeded (depth: 100)");
    }

    #[test]
    fn test_evaluation_error_string_length_exceeded() {
        let error = EvaluationError::string_length_exceeded(2048, 1024);
        let display = format!("{error}");
        assert_eq!(display, "String length limit exceeded: 2048 > 1024");
    }

    #[test]
    fn test_evaluation_error_invalid_string_encoding() {
        let error = EvaluationError::invalid_string_encoding(512);
        let display = format!("{error}");
        assert_eq!(display, "Invalid string encoding at offset 512");
    }

    #[test]
    fn test_evaluation_error_internal_error() {
        let error = EvaluationError::internal_error("recursion depth underflow");
        let display = format!("{error}");
        assert_eq!(display, "Internal error: recursion depth underflow");
    }

    #[test]
    fn test_libmagic_error_display_parse() {
        let parse_error = ParseError::invalid_syntax(10, "unexpected token");
        let libmagic_error = LibmagicError::from(parse_error);
        let display = format!("{libmagic_error}");
        assert_eq!(
            display,
            "Parse error: Invalid syntax at line 10: unexpected token"
        );
    }

    #[test]
    fn test_libmagic_error_display_evaluation() {
        let eval_error = EvaluationError::buffer_overrun(100);
        let libmagic_error = LibmagicError::from(eval_error);
        let display = format!("{libmagic_error}");
        assert_eq!(display, "Evaluation error: Buffer overrun at offset 100");
    }

    #[test]
    fn test_libmagic_error_display_io() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let libmagic_error = LibmagicError::from(io_error);
        let display = format!("{libmagic_error}");
        assert!(display.starts_with("I/O error:"));
        assert!(display.contains("access denied"));
    }

    #[test]
    fn test_error_debug_formatting() {
        let error = LibmagicError::ParseError(ParseError::invalid_syntax(5, "test"));
        let debug = format!("{error:?}");
        assert!(debug.contains("ParseError"));
        assert!(debug.contains("InvalidSyntax"));
    }

    #[test]
    fn test_parse_error_constructors() {
        let error1 = ParseError::invalid_syntax(1, "test");
        let error2 = ParseError::unsupported_feature(2, "feature");
        let error3 = ParseError::invalid_offset(3, "offset");
        let error4 = ParseError::invalid_type(4, "type");
        let error5 = ParseError::invalid_operator(5, "op");
        let error6 = ParseError::invalid_value(6, "value");

        // Test that all constructors work
        assert!(matches!(error1, ParseError::InvalidSyntax { .. }));
        assert!(matches!(error2, ParseError::UnsupportedFeature { .. }));
        assert!(matches!(error3, ParseError::InvalidOffset { .. }));
        assert!(matches!(error4, ParseError::InvalidType { .. }));
        assert!(matches!(error5, ParseError::InvalidOperator { .. }));
        assert!(matches!(error6, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn test_evaluation_error_constructors() {
        let error1 = EvaluationError::buffer_overrun(100);
        let error2 = EvaluationError::invalid_offset(-1);
        let error3 = EvaluationError::unsupported_type("test");
        let error4 = EvaluationError::recursion_limit_exceeded(50);
        let error5 = EvaluationError::string_length_exceeded(100, 50);
        let error6 = EvaluationError::invalid_string_encoding(200);

        // Test that all constructors work
        assert!(matches!(error1, EvaluationError::BufferOverrun { .. }));
        assert!(matches!(error2, EvaluationError::InvalidOffset { .. }));
        assert!(matches!(error3, EvaluationError::UnsupportedType { .. }));
        assert!(matches!(
            error4,
            EvaluationError::RecursionLimitExceeded { .. }
        ));
        assert!(matches!(
            error5,
            EvaluationError::StringLengthExceeded { .. }
        ));
        assert!(matches!(
            error6,
            EvaluationError::InvalidStringEncoding { .. }
        ));
    }

    #[test]
    fn test_parse_error_unsupported_format() {
        let error = ParseError::unsupported_format(
            0,
            "binary .mgc",
            "Binary files not supported. Use --use-builtin option.",
        );
        let display = format!("{error}");
        assert!(display.contains("Unsupported format"));
        assert!(display.contains("binary .mgc"));
        assert!(display.contains("--use-builtin"));
    }

    #[test]
    fn test_parse_error_unsupported_format_constructor() {
        let error = ParseError::unsupported_format(5, "test_format", "test message");

        match error {
            ParseError::UnsupportedFormat {
                line,
                format_type,
                message,
            } => {
                assert_eq!(line, 5);
                assert_eq!(format_type, "test_format");
                assert_eq!(message, "test message");
            }
            _ => panic!("Expected UnsupportedFormat variant"),
        }
    }
}
