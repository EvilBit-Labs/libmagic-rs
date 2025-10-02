//! Output formatting module for magic rule evaluation results
//!
//! This module provides data structures and functionality for storing and formatting
//! the results of magic rule evaluation, supporting both text and JSON output formats.

pub mod text;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::parser::ast::Value;

/// Result of a single magic rule match
///
/// Contains all information about a successful rule match, including the matched
/// value, its location in the file, and metadata about the rule that matched.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::MatchResult;
/// use libmagic_rs::parser::ast::Value;
///
/// let result = MatchResult {
///     message: "ELF 64-bit LSB executable".to_string(),
///     offset: 0,
///     length: 4,
///     value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
///     rule_path: vec!["elf".to_string(), "elf64".to_string()],
///     confidence: 90,
///     mime_type: Some("application/x-executable".to_string()),
/// };
///
/// assert_eq!(result.message, "ELF 64-bit LSB executable");
/// assert_eq!(result.offset, 0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchResult {
    /// Human-readable description of the file type or pattern match
    pub message: String,

    /// Byte offset in the file where the match occurred
    pub offset: usize,

    /// Number of bytes that were examined for this match
    pub length: usize,

    /// The actual value that was matched at the specified offset
    pub value: Value,

    /// Hierarchical path of rule names that led to this match
    ///
    /// For nested rules, this contains the sequence of rule identifiers
    /// from the root rule down to the specific rule that matched.
    pub rule_path: Vec<String>,

    /// Confidence score for this match (0-100)
    ///
    /// Higher values indicate more specific or reliable matches.
    /// Generic patterns typically have lower confidence scores.
    pub confidence: u8,

    /// Optional MIME type associated with this match
    ///
    /// When available, provides the standard MIME type corresponding
    /// to the detected file format.
    pub mime_type: Option<String>,
}

/// Complete evaluation result for a file
///
/// Contains all matches found during rule evaluation, along with metadata
/// about the evaluation process and the file being analyzed.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{EvaluationResult, MatchResult, EvaluationMetadata};
/// use libmagic_rs::parser::ast::Value;
/// use std::path::PathBuf;
///
/// let result = EvaluationResult {
///     filename: PathBuf::from("example.bin"),
///     matches: vec![
///         MatchResult {
///             message: "ELF executable".to_string(),
///             offset: 0,
///             length: 4,
///             value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
///             rule_path: vec!["elf".to_string()],
///             confidence: 95,
///             mime_type: Some("application/x-executable".to_string()),
///         }
///     ],
///     metadata: EvaluationMetadata {
///         file_size: 8192,
///         evaluation_time_ms: 2.5,
///         rules_evaluated: 42,
///         rules_matched: 1,
///     },
///     error: None,
/// };
///
/// assert_eq!(result.matches.len(), 1);
/// assert_eq!(result.metadata.file_size, 8192);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Path to the file that was analyzed
    pub filename: PathBuf,

    /// All successful rule matches found during evaluation
    ///
    /// Matches are typically ordered by offset, then by confidence score.
    /// The first match is often considered the primary file type.
    pub matches: Vec<MatchResult>,

    /// Metadata about the evaluation process
    pub metadata: EvaluationMetadata,

    /// Error that occurred during evaluation, if any
    ///
    /// When present, indicates that evaluation was incomplete or failed.
    /// Partial results may still be available in the matches vector.
    pub error: Option<String>,
}

/// Metadata about the evaluation process
///
/// Provides diagnostic information about how the evaluation was performed,
/// including performance metrics and statistics about rule processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetadata {
    /// Size of the analyzed file in bytes
    pub file_size: u64,

    /// Time taken for evaluation in milliseconds
    pub evaluation_time_ms: f64,

    /// Total number of rules that were evaluated
    ///
    /// This includes rules that were tested but did not match.
    pub rules_evaluated: u32,

    /// Number of rules that successfully matched
    pub rules_matched: u32,
}

impl MatchResult {
    /// Create a new match result with basic information
    ///
    /// # Arguments
    ///
    /// * `message` - Human-readable description of the match
    /// * `offset` - Byte offset where the match occurred
    /// * `value` - The matched value
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::MatchResult;
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let result = MatchResult::new(
    ///     "PNG image".to_string(),
    ///     0,
    ///     Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47])
    /// );
    ///
    /// assert_eq!(result.message, "PNG image");
    /// assert_eq!(result.offset, 0);
    /// assert_eq!(result.confidence, 50); // Default confidence
    /// ```
    #[must_use]
    pub fn new(message: String, offset: usize, value: Value) -> Self {
        Self {
            message,
            offset,
            length: match &value {
                Value::Bytes(bytes) => bytes.len(),
                Value::String(s) => s.len(),
                Value::Uint(_) | Value::Int(_) => std::mem::size_of::<u64>(),
            },
            value,
            rule_path: Vec::new(),
            confidence: 50, // Default moderate confidence
            mime_type: None,
        }
    }

    /// Create a new match result with full metadata
    ///
    /// # Arguments
    ///
    /// * `message` - Human-readable description of the match
    /// * `offset` - Byte offset where the match occurred
    /// * `length` - Number of bytes examined
    /// * `value` - The matched value
    /// * `rule_path` - Hierarchical path of rules that led to this match
    /// * `confidence` - Confidence score (0-100)
    /// * `mime_type` - Optional MIME type
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::MatchResult;
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let result = MatchResult::with_metadata(
    ///     "JPEG image".to_string(),
    ///     0,
    ///     2,
    ///     Value::Bytes(vec![0xff, 0xd8]),
    ///     vec!["image".to_string(), "jpeg".to_string()],
    ///     85,
    ///     Some("image/jpeg".to_string())
    /// );
    ///
    /// assert_eq!(result.rule_path.len(), 2);
    /// assert_eq!(result.confidence, 85);
    /// assert_eq!(result.mime_type, Some("image/jpeg".to_string()));
    /// ```
    #[must_use]
    pub fn with_metadata(
        message: String,
        offset: usize,
        length: usize,
        value: Value,
        rule_path: Vec<String>,
        confidence: u8,
        mime_type: Option<String>,
    ) -> Self {
        Self {
            message,
            offset,
            length,
            value,
            rule_path,
            confidence: confidence.min(100), // Clamp to valid range
            mime_type,
        }
    }

    /// Set the confidence score for this match
    ///
    /// The confidence score is automatically clamped to the range 0-100.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::MatchResult;
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let mut result = MatchResult::new(
    ///     "Text file".to_string(),
    ///     0,
    ///     Value::String("Hello".to_string())
    /// );
    ///
    /// result.set_confidence(75);
    /// assert_eq!(result.confidence, 75);
    ///
    /// // Values over 100 are clamped
    /// result.set_confidence(150);
    /// assert_eq!(result.confidence, 100);
    /// ```
    pub fn set_confidence(&mut self, confidence: u8) {
        self.confidence = confidence.min(100);
    }

    /// Add a rule name to the rule path
    ///
    /// This is typically used during evaluation to build up the hierarchical
    /// path of rules that led to a match.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::MatchResult;
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let mut result = MatchResult::new(
    ///     "Archive".to_string(),
    ///     0,
    ///     Value::String("PK".to_string())
    /// );
    ///
    /// result.add_rule_path("archive".to_string());
    /// result.add_rule_path("zip".to_string());
    ///
    /// assert_eq!(result.rule_path, vec!["archive", "zip"]);
    /// ```
    pub fn add_rule_path(&mut self, rule_name: String) {
        self.rule_path.push(rule_name);
    }

    /// Set the MIME type for this match
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::MatchResult;
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let mut result = MatchResult::new(
    ///     "PDF document".to_string(),
    ///     0,
    ///     Value::String("%PDF".to_string())
    /// );
    ///
    /// result.set_mime_type(Some("application/pdf".to_string()));
    /// assert_eq!(result.mime_type, Some("application/pdf".to_string()));
    /// ```
    pub fn set_mime_type(&mut self, mime_type: Option<String>) {
        self.mime_type = mime_type;
    }
}

impl EvaluationResult {
    /// Create a new evaluation result
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the analyzed file
    /// * `matches` - Vector of successful matches
    /// * `metadata` - Evaluation metadata
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, EvaluationMetadata};
    /// use std::path::PathBuf;
    ///
    /// let result = EvaluationResult::new(
    ///     PathBuf::from("test.txt"),
    ///     vec![],
    ///     EvaluationMetadata {
    ///         file_size: 1024,
    ///         evaluation_time_ms: 1.2,
    ///         rules_evaluated: 10,
    ///         rules_matched: 0,
    ///     }
    /// );
    ///
    /// assert_eq!(result.filename, PathBuf::from("test.txt"));
    /// assert!(result.matches.is_empty());
    /// assert!(result.error.is_none());
    /// ```
    #[must_use]
    pub fn new(filename: PathBuf, matches: Vec<MatchResult>, metadata: EvaluationMetadata) -> Self {
        Self {
            filename,
            matches,
            metadata,
            error: None,
        }
    }

    /// Create an evaluation result with an error
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the analyzed file
    /// * `error` - Error message describing what went wrong
    /// * `metadata` - Evaluation metadata (may be partial)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, EvaluationMetadata};
    /// use std::path::PathBuf;
    ///
    /// let result = EvaluationResult::with_error(
    ///     PathBuf::from("missing.txt"),
    ///     "File not found".to_string(),
    ///     EvaluationMetadata {
    ///         file_size: 0,
    ///         evaluation_time_ms: 0.0,
    ///         rules_evaluated: 0,
    ///         rules_matched: 0,
    ///     }
    /// );
    ///
    /// assert_eq!(result.error, Some("File not found".to_string()));
    /// assert!(result.matches.is_empty());
    /// ```
    #[must_use]
    pub fn with_error(filename: PathBuf, error: String, metadata: EvaluationMetadata) -> Self {
        Self {
            filename,
            matches: Vec::new(),
            metadata,
            error: Some(error),
        }
    }

    /// Add a match result to this evaluation
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, MatchResult, EvaluationMetadata};
    /// use libmagic_rs::parser::ast::Value;
    /// use std::path::PathBuf;
    ///
    /// let mut result = EvaluationResult::new(
    ///     PathBuf::from("data.bin"),
    ///     vec![],
    ///     EvaluationMetadata {
    ///         file_size: 512,
    ///         evaluation_time_ms: 0.8,
    ///         rules_evaluated: 5,
    ///         rules_matched: 0,
    ///     }
    /// );
    ///
    /// let match_result = MatchResult::new(
    ///     "Binary data".to_string(),
    ///     0,
    ///     Value::Bytes(vec![0x00, 0x01, 0x02])
    /// );
    ///
    /// result.add_match(match_result);
    /// assert_eq!(result.matches.len(), 1);
    /// ```
    pub fn add_match(&mut self, match_result: MatchResult) {
        self.matches.push(match_result);
    }

    /// Get the primary match (first match with highest confidence)
    ///
    /// Returns the match that is most likely to represent the primary file type.
    /// This is typically the first match, but if multiple matches exist, the one
    /// with the highest confidence score is preferred.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, MatchResult, EvaluationMetadata};
    /// use libmagic_rs::parser::ast::Value;
    /// use std::path::PathBuf;
    ///
    /// let mut result = EvaluationResult::new(
    ///     PathBuf::from("test.exe"),
    ///     vec![
    ///         MatchResult::with_metadata(
    ///             "Executable".to_string(),
    ///             0, 2,
    ///             Value::String("MZ".to_string()),
    ///             vec!["pe".to_string()],
    ///             60,
    ///             None
    ///         ),
    ///         MatchResult::with_metadata(
    ///             "PE32 executable".to_string(),
    ///             60, 4,
    ///             Value::String("PE\0\0".to_string()),
    ///             vec!["pe".to_string(), "pe32".to_string()],
    ///             90,
    ///             Some("application/x-msdownload".to_string())
    ///         ),
    ///     ],
    ///     EvaluationMetadata {
    ///         file_size: 4096,
    ///         evaluation_time_ms: 1.5,
    ///         rules_evaluated: 15,
    ///         rules_matched: 2,
    ///     }
    /// );
    ///
    /// let primary = result.primary_match();
    /// assert!(primary.is_some());
    /// assert_eq!(primary.unwrap().confidence, 90);
    /// ```
    #[must_use]
    pub fn primary_match(&self) -> Option<&MatchResult> {
        self.matches
            .iter()
            .max_by_key(|match_result| match_result.confidence)
    }

    /// Check if the evaluation was successful (no errors)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, EvaluationMetadata};
    /// use std::path::PathBuf;
    ///
    /// let success = EvaluationResult::new(
    ///     PathBuf::from("good.txt"),
    ///     vec![],
    ///     EvaluationMetadata {
    ///         file_size: 100,
    ///         evaluation_time_ms: 0.5,
    ///         rules_evaluated: 3,
    ///         rules_matched: 0,
    ///     }
    /// );
    ///
    /// let failure = EvaluationResult::with_error(
    ///     PathBuf::from("bad.txt"),
    ///     "Parse error".to_string(),
    ///     EvaluationMetadata {
    ///         file_size: 0,
    ///         evaluation_time_ms: 0.0,
    ///         rules_evaluated: 0,
    ///         rules_matched: 0,
    ///     }
    /// );
    ///
    /// assert!(success.is_success());
    /// assert!(!failure.is_success());
    /// ```
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

impl EvaluationMetadata {
    /// Create new evaluation metadata
    ///
    /// # Arguments
    ///
    /// * `file_size` - Size of the analyzed file in bytes
    /// * `evaluation_time_ms` - Time taken for evaluation in milliseconds
    /// * `rules_evaluated` - Number of rules that were tested
    /// * `rules_matched` - Number of rules that matched
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::EvaluationMetadata;
    ///
    /// let metadata = EvaluationMetadata::new(2048, 3.7, 25, 3);
    ///
    /// assert_eq!(metadata.file_size, 2048);
    /// assert_eq!(metadata.evaluation_time_ms, 3.7);
    /// assert_eq!(metadata.rules_evaluated, 25);
    /// assert_eq!(metadata.rules_matched, 3);
    /// ```
    #[must_use]
    pub fn new(
        file_size: u64,
        evaluation_time_ms: f64,
        rules_evaluated: u32,
        rules_matched: u32,
    ) -> Self {
        Self {
            file_size,
            evaluation_time_ms,
            rules_evaluated,
            rules_matched,
        }
    }

    /// Get the match rate as a percentage
    ///
    /// Returns the percentage of evaluated rules that resulted in matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::EvaluationMetadata;
    ///
    /// let metadata = EvaluationMetadata::new(1024, 1.0, 20, 5);
    /// assert_eq!(metadata.match_rate(), 25.0);
    ///
    /// let no_rules = EvaluationMetadata::new(1024, 1.0, 0, 0);
    /// assert_eq!(no_rules.match_rate(), 0.0);
    /// ```
    #[must_use]
    pub fn match_rate(&self) -> f64 {
        if self.rules_evaluated == 0 {
            0.0
        } else {
            (f64::from(self.rules_matched) / f64::from(self.rules_evaluated)) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_result_new() {
        let result = MatchResult::new(
            "Test file".to_string(),
            42,
            Value::String("test".to_string()),
        );

        assert_eq!(result.message, "Test file");
        assert_eq!(result.offset, 42);
        assert_eq!(result.length, 4); // Length of "test"
        assert_eq!(result.value, Value::String("test".to_string()));
        assert!(result.rule_path.is_empty());
        assert_eq!(result.confidence, 50);
        assert!(result.mime_type.is_none());
    }

    #[test]
    fn test_match_result_with_metadata() {
        let result = MatchResult::with_metadata(
            "ELF executable".to_string(),
            0,
            4,
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            vec!["elf".to_string()],
            95,
            Some("application/x-executable".to_string()),
        );

        assert_eq!(result.message, "ELF executable");
        assert_eq!(result.offset, 0);
        assert_eq!(result.length, 4);
        assert_eq!(result.rule_path, vec!["elf"]);
        assert_eq!(result.confidence, 95);
        assert_eq!(
            result.mime_type,
            Some("application/x-executable".to_string())
        );
    }

    #[test]
    fn test_match_result_length_calculation() {
        // Test length calculation for different value types
        let bytes_result = MatchResult::new("Bytes".to_string(), 0, Value::Bytes(vec![1, 2, 3]));
        assert_eq!(bytes_result.length, 3);

        let string_result =
            MatchResult::new("String".to_string(), 0, Value::String("hello".to_string()));
        assert_eq!(string_result.length, 5);

        let uint_result = MatchResult::new("Uint".to_string(), 0, Value::Uint(42));
        assert_eq!(uint_result.length, 8); // size_of::<u64>()

        let int_result = MatchResult::new("Int".to_string(), 0, Value::Int(-42));
        assert_eq!(int_result.length, 8); // size_of::<u64>()
    }

    #[test]
    fn test_match_result_set_confidence() {
        let mut result = MatchResult::new("Test".to_string(), 0, Value::Uint(0));

        result.set_confidence(75);
        assert_eq!(result.confidence, 75);

        // Test clamping to 100
        result.set_confidence(150);
        assert_eq!(result.confidence, 100);

        result.set_confidence(0);
        assert_eq!(result.confidence, 0);
    }

    #[test]
    fn test_match_result_confidence_clamping_in_constructor() {
        let result = MatchResult::with_metadata(
            "Test".to_string(),
            0,
            1,
            Value::Uint(0),
            vec![],
            200, // Over 100
            None,
        );

        assert_eq!(result.confidence, 100);
    }

    #[test]
    fn test_match_result_add_rule_path() {
        let mut result = MatchResult::new("Test".to_string(), 0, Value::Uint(0));

        result.add_rule_path("root".to_string());
        result.add_rule_path("child".to_string());
        result.add_rule_path("grandchild".to_string());

        assert_eq!(result.rule_path, vec!["root", "child", "grandchild"]);
    }

    #[test]
    fn test_match_result_set_mime_type() {
        let mut result = MatchResult::new("Test".to_string(), 0, Value::Uint(0));

        result.set_mime_type(Some("text/plain".to_string()));
        assert_eq!(result.mime_type, Some("text/plain".to_string()));

        result.set_mime_type(None);
        assert!(result.mime_type.is_none());
    }

    #[test]
    fn test_match_result_serialization() {
        let result = MatchResult::with_metadata(
            "PNG image".to_string(),
            0,
            8,
            Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
            vec!["image".to_string(), "png".to_string()],
            90,
            Some("image/png".to_string()),
        );

        let json = serde_json::to_string(&result).expect("Failed to serialize MatchResult");
        let deserialized: MatchResult =
            serde_json::from_str(&json).expect("Failed to deserialize MatchResult");

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_evaluation_result_new() {
        let metadata = EvaluationMetadata::new(1024, 2.5, 10, 2);
        let result = EvaluationResult::new(PathBuf::from("test.bin"), vec![], metadata);

        assert_eq!(result.filename, PathBuf::from("test.bin"));
        assert!(result.matches.is_empty());
        assert!(result.error.is_none());
        assert_eq!(result.metadata.file_size, 1024);
    }

    #[test]
    fn test_evaluation_result_with_error() {
        let metadata = EvaluationMetadata::new(0, 0.0, 0, 0);
        let result = EvaluationResult::with_error(
            PathBuf::from("missing.txt"),
            "File not found".to_string(),
            metadata,
        );

        assert_eq!(result.error, Some("File not found".to_string()));
        assert!(result.matches.is_empty());
        assert!(!result.is_success());
    }

    #[test]
    fn test_evaluation_result_add_match() {
        let metadata = EvaluationMetadata::new(512, 1.0, 5, 0);
        let mut result = EvaluationResult::new(PathBuf::from("data.bin"), vec![], metadata);

        let match_result =
            MatchResult::new("Binary data".to_string(), 0, Value::Bytes(vec![0x00, 0x01]));

        result.add_match(match_result);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].message, "Binary data");
    }

    #[test]
    fn test_evaluation_result_primary_match() {
        let metadata = EvaluationMetadata::new(2048, 3.0, 20, 3);
        let matches = vec![
            MatchResult::with_metadata(
                "Low confidence".to_string(),
                0,
                2,
                Value::String("AB".to_string()),
                vec![],
                30,
                None,
            ),
            MatchResult::with_metadata(
                "High confidence".to_string(),
                10,
                4,
                Value::String("TEST".to_string()),
                vec![],
                95,
                None,
            ),
            MatchResult::with_metadata(
                "Medium confidence".to_string(),
                20,
                3,
                Value::String("XYZ".to_string()),
                vec![],
                60,
                None,
            ),
        ];

        let result = EvaluationResult::new(PathBuf::from("test.dat"), matches, metadata);

        let primary = result.primary_match();
        assert!(primary.is_some());
        assert_eq!(primary.unwrap().message, "High confidence");
        assert_eq!(primary.unwrap().confidence, 95);
    }

    #[test]
    fn test_evaluation_result_primary_match_empty() {
        let metadata = EvaluationMetadata::new(0, 0.0, 0, 0);
        let result = EvaluationResult::new(PathBuf::from("empty.txt"), vec![], metadata);

        assert!(result.primary_match().is_none());
    }

    #[test]
    fn test_evaluation_result_is_success() {
        let metadata = EvaluationMetadata::new(100, 0.5, 3, 1);

        let success = EvaluationResult::new(PathBuf::from("good.txt"), vec![], metadata.clone());

        let failure = EvaluationResult::with_error(
            PathBuf::from("bad.txt"),
            "Error occurred".to_string(),
            metadata,
        );

        assert!(success.is_success());
        assert!(!failure.is_success());
    }

    #[test]
    fn test_evaluation_result_serialization() {
        let match_result = MatchResult::new(
            "Text file".to_string(),
            0,
            Value::String("Hello".to_string()),
        );

        let metadata = EvaluationMetadata::new(1024, 1.5, 8, 1);
        let result =
            EvaluationResult::new(PathBuf::from("hello.txt"), vec![match_result], metadata);

        let json = serde_json::to_string(&result).expect("Failed to serialize EvaluationResult");
        let deserialized: EvaluationResult =
            serde_json::from_str(&json).expect("Failed to deserialize EvaluationResult");

        assert_eq!(result.filename, deserialized.filename);
        assert_eq!(result.matches.len(), deserialized.matches.len());
        assert_eq!(result.metadata.file_size, deserialized.metadata.file_size);
    }

    #[test]
    fn test_evaluation_metadata_new() {
        let metadata = EvaluationMetadata::new(4096, 5.2, 50, 8);

        assert_eq!(metadata.file_size, 4096);
        assert!((metadata.evaluation_time_ms - 5.2).abs() < f64::EPSILON);
        assert_eq!(metadata.rules_evaluated, 50);
        assert_eq!(metadata.rules_matched, 8);
    }

    #[test]
    fn test_evaluation_metadata_match_rate() {
        let metadata = EvaluationMetadata::new(1024, 1.0, 20, 5);
        assert!((metadata.match_rate() - 25.0).abs() < f64::EPSILON);

        let perfect_match = EvaluationMetadata::new(1024, 1.0, 10, 10);
        assert!((perfect_match.match_rate() - 100.0).abs() < f64::EPSILON);

        let no_matches = EvaluationMetadata::new(1024, 1.0, 15, 0);
        assert!((no_matches.match_rate() - 0.0).abs() < f64::EPSILON);

        let no_rules = EvaluationMetadata::new(1024, 1.0, 0, 0);
        assert!((no_rules.match_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evaluation_metadata_serialization() {
        let metadata = EvaluationMetadata::new(2048, 3.7, 25, 4);

        let json =
            serde_json::to_string(&metadata).expect("Failed to serialize EvaluationMetadata");
        let deserialized: EvaluationMetadata =
            serde_json::from_str(&json).expect("Failed to deserialize EvaluationMetadata");

        assert_eq!(metadata.file_size, deserialized.file_size);
        assert!(
            (metadata.evaluation_time_ms - deserialized.evaluation_time_ms).abs() < f64::EPSILON
        );
        assert_eq!(metadata.rules_evaluated, deserialized.rules_evaluated);
        assert_eq!(metadata.rules_matched, deserialized.rules_matched);
    }

    #[test]
    fn test_match_result_equality() {
        let result1 = MatchResult::new("Test".to_string(), 0, Value::Uint(42));

        let result2 = MatchResult::new("Test".to_string(), 0, Value::Uint(42));

        let result3 = MatchResult::new("Different".to_string(), 0, Value::Uint(42));

        assert_eq!(result1, result2);
        assert_ne!(result1, result3);
    }

    #[test]
    fn test_complex_evaluation_result() {
        // Test a complex scenario with multiple matches and full metadata
        let matches = vec![
            MatchResult::with_metadata(
                "ELF 64-bit LSB executable".to_string(),
                0,
                4,
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
                vec!["elf".to_string(), "elf64".to_string()],
                95,
                Some("application/x-executable".to_string()),
            ),
            MatchResult::with_metadata(
                "x86-64 architecture".to_string(),
                18,
                2,
                Value::Uint(0x3e),
                vec!["elf".to_string(), "elf64".to_string(), "x86_64".to_string()],
                85,
                None,
            ),
            MatchResult::with_metadata(
                "dynamically linked".to_string(),
                16,
                2,
                Value::Uint(0x02),
                vec![
                    "elf".to_string(),
                    "elf64".to_string(),
                    "dynamic".to_string(),
                ],
                80,
                None,
            ),
        ];

        let metadata = EvaluationMetadata::new(8192, 4.2, 35, 3);
        let result = EvaluationResult::new(PathBuf::from("/usr/bin/ls"), matches, metadata);

        assert_eq!(result.matches.len(), 3);
        let expected_rate = (3.0 / 35.0) * 100.0;
        assert!((result.metadata.match_rate() - expected_rate).abs() < f64::EPSILON);

        let primary = result.primary_match().unwrap();
        assert_eq!(primary.message, "ELF 64-bit LSB executable");
        assert_eq!(primary.confidence, 95);
        assert_eq!(
            primary.mime_type,
            Some("application/x-executable".to_string())
        );

        // Verify all matches have proper rule paths
        for match_result in &result.matches {
            assert!(!match_result.rule_path.is_empty());
            assert!(match_result.rule_path[0] == "elf");
        }
    }
}
