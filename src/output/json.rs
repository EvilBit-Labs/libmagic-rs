//! JSON output formatting for magic rule evaluation results
//!
//! This module provides JSON-specific data structures and formatting functions
//! for outputting magic rule evaluation results in a structured format compatible
//! with the original libmagic specification.
//!
//! The JSON output format follows the original spec with fields for text, offset,
//! value, tags, and score, providing a machine-readable alternative to the
//! human-readable text output format.

use serde::{Deserialize, Serialize};

use crate::output::{EvaluationResult, MatchResult};
use crate::parser::ast::Value;

/// JSON representation of a magic rule match result
///
/// This structure follows the original libmagic JSON specification format,
/// providing a standardized way to represent file type detection results
/// in JSON format for programmatic consumption.
///
/// # Fields
///
/// * `text` - Human-readable description of the file type or pattern match
/// * `offset` - Byte offset in the file where the match occurred
/// * `value` - Hexadecimal representation of the matched bytes
/// * `tags` - Array of classification tags derived from the rule hierarchy
/// * `score` - Confidence score for this match (0-100)
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::json::JsonMatchResult;
///
/// let json_result = JsonMatchResult {
///     text: "ELF 64-bit LSB executable".to_string(),
///     offset: 0,
///     value: "7f454c46".to_string(),
///     tags: vec!["executable".to_string(), "elf".to_string()],
///     score: 90,
/// };
///
/// assert_eq!(json_result.text, "ELF 64-bit LSB executable");
/// assert_eq!(json_result.offset, 0);
/// assert_eq!(json_result.value, "7f454c46");
/// assert_eq!(json_result.tags.len(), 2);
/// assert_eq!(json_result.score, 90);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonMatchResult {
    /// Human-readable description of the file type or pattern match
    ///
    /// This field contains the same descriptive text that would appear
    /// in the traditional text output format, providing context about
    /// what type of file or pattern was detected.
    pub text: String,

    /// Byte offset in the file where the match occurred
    ///
    /// Indicates the exact position in the file where the magic rule
    /// found the matching pattern. This is useful for understanding
    /// the structure of the file and for debugging rule evaluation.
    pub offset: usize,

    /// Hexadecimal representation of the matched bytes
    ///
    /// Contains the actual byte values that were matched, encoded as
    /// a hexadecimal string without separators. For string matches,
    /// this represents the UTF-8 bytes of the matched text.
    pub value: String,

    /// Array of classification tags derived from the rule hierarchy
    ///
    /// These tags are extracted from the rule path and provide
    /// machine-readable classification information about the detected
    /// file type. Tags are typically ordered from general to specific.
    pub tags: Vec<String>,

    /// Confidence score for this match (0-100)
    ///
    /// Indicates how confident the detection algorithm is about this
    /// particular match. Higher scores indicate more specific or
    /// reliable patterns, while lower scores may indicate generic
    /// or ambiguous matches.
    pub score: u8,
}

impl JsonMatchResult {
    /// Create a new JSON match result from a `MatchResult`
    ///
    /// Converts the internal `MatchResult` representation to the JSON format
    /// specified in the original libmagic specification, including proper
    /// formatting of the value field and extraction of tags from the rule path.
    ///
    /// # Arguments
    ///
    /// * `match_result` - The internal match result to convert
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{MatchResult, json::JsonMatchResult};
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let match_result = MatchResult::with_metadata(
    ///     "PNG image".to_string(),
    ///     0,
    ///     8,
    ///     Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    ///     vec!["image".to_string(), "png".to_string()],
    ///     85,
    ///     Some("image/png".to_string())
    /// );
    ///
    /// let json_result = JsonMatchResult::from_match_result(&match_result);
    ///
    /// assert_eq!(json_result.text, "PNG image");
    /// assert_eq!(json_result.offset, 0);
    /// assert_eq!(json_result.value, "89504e470d0a1a0a");
    /// assert_eq!(json_result.tags, vec!["image", "png"]);
    /// assert_eq!(json_result.score, 85);
    /// ```
    #[must_use]
    pub fn from_match_result(match_result: &MatchResult) -> Self {
        Self {
            text: match_result.message.clone(),
            offset: match_result.offset,
            value: format_value_as_hex(&match_result.value),
            tags: match_result.rule_path.clone(),
            score: match_result.confidence,
        }
    }

    /// Create a new JSON match result with explicit values
    ///
    /// # Arguments
    ///
    /// * `text` - Human-readable description
    /// * `offset` - Byte offset where match occurred
    /// * `value` - Hexadecimal string representation of matched bytes
    /// * `tags` - Classification tags
    /// * `score` - Confidence score (0-100)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::JsonMatchResult;
    ///
    /// let json_result = JsonMatchResult::new(
    ///     "JPEG image".to_string(),
    ///     0,
    ///     "ffd8".to_string(),
    ///     vec!["image".to_string(), "jpeg".to_string()],
    ///     80
    /// );
    ///
    /// assert_eq!(json_result.text, "JPEG image");
    /// assert_eq!(json_result.value, "ffd8");
    /// assert_eq!(json_result.score, 80);
    /// ```
    #[must_use]
    pub fn new(text: String, offset: usize, value: String, tags: Vec<String>, score: u8) -> Self {
        Self {
            text,
            offset,
            value,
            tags,
            score: score.min(100), // Clamp score to valid range
        }
    }

    /// Add a tag to the tags array
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::JsonMatchResult;
    ///
    /// let mut json_result = JsonMatchResult::new(
    ///     "Archive".to_string(),
    ///     0,
    ///     "504b0304".to_string(),
    ///     vec!["archive".to_string()],
    ///     75
    /// );
    ///
    /// json_result.add_tag("zip".to_string());
    /// assert_eq!(json_result.tags, vec!["archive", "zip"]);
    /// ```
    pub fn add_tag(&mut self, tag: String) {
        self.tags.push(tag);
    }

    /// Set the confidence score, clamping to valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::JsonMatchResult;
    ///
    /// let mut json_result = JsonMatchResult::new(
    ///     "Text".to_string(),
    ///     0,
    ///     "48656c6c6f".to_string(),
    ///     vec![],
    ///     50
    /// );
    ///
    /// json_result.set_score(95);
    /// assert_eq!(json_result.score, 95);
    ///
    /// // Values over 100 are clamped
    /// json_result.set_score(150);
    /// assert_eq!(json_result.score, 100);
    /// ```
    pub fn set_score(&mut self, score: u8) {
        self.score = score.min(100);
    }
}

/// Format a Value as a hexadecimal string for JSON output
///
/// Converts different Value types to their hexadecimal string representation
/// suitable for inclusion in JSON output. Byte arrays are converted directly,
/// while other types are first converted to their byte representation.
///
/// # Arguments
///
/// * `value` - The Value to format as hexadecimal
///
/// # Returns
///
/// A lowercase hexadecimal string without separators or prefixes
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::json::format_value_as_hex;
/// use libmagic_rs::parser::ast::Value;
///
/// let bytes_value = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
/// assert_eq!(format_value_as_hex(&bytes_value), "7f454c46");
///
/// let string_value = Value::String("PNG".to_string());
/// assert_eq!(format_value_as_hex(&string_value), "504e47");
///
/// let uint_value = Value::Uint(0x1234);
/// assert_eq!(format_value_as_hex(&uint_value), "3412000000000000"); // Little-endian u64
/// ```
#[must_use]
pub fn format_value_as_hex(value: &Value) -> String {
    use std::fmt::Write;

    match value {
        Value::Bytes(bytes) => {
            let mut result = String::with_capacity(bytes.len() * 2);
            for &b in bytes {
                write!(&mut result, "{b:02x}").expect("Writing to String should never fail");
            }
            result
        }
        Value::String(s) => {
            let bytes = s.as_bytes();
            let mut result = String::with_capacity(bytes.len() * 2);
            for &b in bytes {
                write!(&mut result, "{b:02x}").expect("Writing to String should never fail");
            }
            result
        }
        Value::Uint(n) => {
            // Convert to little-endian bytes for consistency
            let bytes = n.to_le_bytes();
            let mut result = String::with_capacity(16); // 8 bytes * 2 chars per byte
            for &b in &bytes {
                write!(&mut result, "{b:02x}").expect("Writing to String should never fail");
            }
            result
        }
        Value::Int(n) => {
            // Convert to little-endian bytes for consistency
            let bytes = n.to_le_bytes();
            let mut result = String::with_capacity(16); // 8 bytes * 2 chars per byte
            for &b in &bytes {
                write!(&mut result, "{b:02x}").expect("Writing to String should never fail");
            }
            result
        }
    }
}

/// JSON output structure containing an array of matches
///
/// This structure represents the complete JSON output format for file type
/// detection results, containing an array of matches that can be serialized
/// to JSON for programmatic consumption.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::json::{JsonOutput, JsonMatchResult};
///
/// let json_output = JsonOutput {
///     matches: vec![
///         JsonMatchResult::new(
///             "ELF executable".to_string(),
///             0,
///             "7f454c46".to_string(),
///             vec!["executable".to_string(), "elf".to_string()],
///             90
///         )
///     ]
/// };
///
/// assert_eq!(json_output.matches.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOutput {
    /// Array of match results found during evaluation
    pub matches: Vec<JsonMatchResult>,
}

impl JsonOutput {
    /// Create a new JSON output structure
    ///
    /// # Arguments
    ///
    /// * `matches` - Vector of JSON match results
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::{JsonOutput, JsonMatchResult};
    ///
    /// let matches = vec![
    ///     JsonMatchResult::new(
    ///         "Text file".to_string(),
    ///         0,
    ///         "48656c6c6f".to_string(),
    ///         vec!["text".to_string()],
    ///         60
    ///     )
    /// ];
    ///
    /// let output = JsonOutput::new(matches);
    /// assert_eq!(output.matches.len(), 1);
    /// ```
    #[must_use]
    pub fn new(matches: Vec<JsonMatchResult>) -> Self {
        Self { matches }
    }

    /// Create JSON output from an `EvaluationResult`
    ///
    /// Converts the internal evaluation result to the JSON format specified
    /// in the original libmagic specification.
    ///
    /// # Arguments
    ///
    /// * `result` - The evaluation result to convert
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::{EvaluationResult, MatchResult, EvaluationMetadata, json::JsonOutput};
    /// use libmagic_rs::parser::ast::Value;
    /// use std::path::PathBuf;
    ///
    /// let match_result = MatchResult::with_metadata(
    ///     "Binary data".to_string(),
    ///     0,
    ///     4,
    ///     Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]),
    ///     vec!["binary".to_string()],
    ///     70,
    ///     None
    /// );
    ///
    /// let metadata = EvaluationMetadata::new(1024, 1.5, 10, 1);
    /// let eval_result = EvaluationResult::new(
    ///     PathBuf::from("test.bin"),
    ///     vec![match_result],
    ///     metadata
    /// );
    ///
    /// let json_output = JsonOutput::from_evaluation_result(&eval_result);
    /// assert_eq!(json_output.matches.len(), 1);
    /// assert_eq!(json_output.matches[0].text, "Binary data");
    /// assert_eq!(json_output.matches[0].value, "deadbeef");
    /// ```
    #[must_use]
    pub fn from_evaluation_result(result: &EvaluationResult) -> Self {
        let matches = result
            .matches
            .iter()
            .map(JsonMatchResult::from_match_result)
            .collect();

        Self { matches }
    }

    /// Add a match result to the output
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::{JsonOutput, JsonMatchResult};
    ///
    /// let mut output = JsonOutput::new(vec![]);
    ///
    /// let match_result = JsonMatchResult::new(
    ///     "PDF document".to_string(),
    ///     0,
    ///     "25504446".to_string(),
    ///     vec!["document".to_string(), "pdf".to_string()],
    ///     85
    /// );
    ///
    /// output.add_match(match_result);
    /// assert_eq!(output.matches.len(), 1);
    /// ```
    pub fn add_match(&mut self, match_result: JsonMatchResult) {
        self.matches.push(match_result);
    }

    /// Check if there are any matches
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::JsonOutput;
    ///
    /// let empty_output = JsonOutput::new(vec![]);
    /// assert!(!empty_output.has_matches());
    ///
    /// let output_with_matches = JsonOutput::new(vec![
    ///     libmagic_rs::output::json::JsonMatchResult::new(
    ///         "Test".to_string(),
    ///         0,
    ///         "74657374".to_string(),
    ///         vec![],
    ///         50
    ///     )
    /// ]);
    /// assert!(output_with_matches.has_matches());
    /// ```
    #[must_use]
    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Get the number of matches
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::output::json::{JsonOutput, JsonMatchResult};
    ///
    /// let matches = vec![
    ///     JsonMatchResult::new("Match 1".to_string(), 0, "01".to_string(), vec![], 50),
    ///     JsonMatchResult::new("Match 2".to_string(), 10, "02".to_string(), vec![], 60),
    /// ];
    ///
    /// let output = JsonOutput::new(matches);
    /// assert_eq!(output.match_count(), 2);
    /// ```
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

/// Format match results as JSON output string
///
/// Converts a vector of `MatchResult` objects into a JSON string following
/// the original libmagic specification format. The output contains a matches
/// array with proper field mapping for programmatic consumption.
///
/// # Arguments
///
/// * `match_results` - Vector of match results to format
///
/// # Returns
///
/// A JSON string containing the formatted match results, or an error if
/// serialization fails.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{MatchResult, json::format_json_output};
/// use libmagic_rs::parser::ast::Value;
///
/// let match_results = vec![
///     MatchResult::with_metadata(
///         "ELF 64-bit LSB executable".to_string(),
///         0,
///         4,
///         Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
///         vec!["executable".to_string(), "elf".to_string()],
///         90,
///         Some("application/x-executable".to_string())
///     ),
///     MatchResult::with_metadata(
///         "x86-64 architecture".to_string(),
///         18,
///         2,
///         Value::Uint(0x3e00),
///         vec!["elf".to_string(), "x86_64".to_string()],
///         85,
///         None
///     )
/// ];
///
/// let json_output = format_json_output(&match_results).unwrap();
/// assert!(json_output.contains("\"matches\""));
/// assert!(json_output.contains("\"text\": \"ELF 64-bit LSB executable\""));
/// assert!(json_output.contains("\"offset\": 0"));
/// assert!(json_output.contains("\"value\": \"7f454c46\""));
/// assert!(json_output.contains("\"score\": 90"));
/// ```
///
/// # Errors
///
/// Returns a `serde_json::Error` if the match results cannot be serialized
/// to JSON, which should be rare in practice since all fields are serializable.
pub fn format_json_output(match_results: &[MatchResult]) -> Result<String, serde_json::Error> {
    let mut json_matches = Vec::with_capacity(match_results.len());
    for match_result in match_results {
        json_matches.push(JsonMatchResult::from_match_result(match_result));
    }

    let output = JsonOutput::new(json_matches);
    serde_json::to_string_pretty(&output)
}

/// Format match results as compact JSON output string
///
/// Similar to `format_json_output` but produces compact JSON without
/// pretty-printing for more efficient transmission or storage.
///
/// # Arguments
///
/// * `match_results` - Vector of match results to format
///
/// # Returns
///
/// A compact JSON string containing the formatted match results.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{MatchResult, json::format_json_output_compact};
/// use libmagic_rs::parser::ast::Value;
///
/// let match_results = vec![
///     MatchResult::new(
///         "PNG image".to_string(),
///         0,
///         Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47])
///     )
/// ];
///
/// let json_output = format_json_output_compact(&match_results).unwrap();
/// assert!(!json_output.contains('\n')); // No newlines in compact format
/// assert!(json_output.contains("\"matches\""));
/// ```
///
/// # Errors
///
/// Returns a `serde_json::Error` if the match results cannot be serialized.
pub fn format_json_output_compact(
    match_results: &[MatchResult],
) -> Result<String, serde_json::Error> {
    let mut json_matches = Vec::with_capacity(match_results.len());
    for match_result in match_results {
        json_matches.push(JsonMatchResult::from_match_result(match_result));
    }

    let output = JsonOutput::new(json_matches);
    serde_json::to_string(&output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{EvaluationMetadata, EvaluationResult, MatchResult};
    use std::path::PathBuf;

    #[test]
    fn test_json_match_result_new() {
        let result = JsonMatchResult::new(
            "Test file".to_string(),
            42,
            "74657374".to_string(),
            vec!["test".to_string()],
            75,
        );

        assert_eq!(result.text, "Test file");
        assert_eq!(result.offset, 42);
        assert_eq!(result.value, "74657374");
        assert_eq!(result.tags, vec!["test"]);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn test_json_match_result_score_clamping() {
        let result = JsonMatchResult::new(
            "Test".to_string(),
            0,
            "00".to_string(),
            vec![],
            200, // Over 100
        );

        assert_eq!(result.score, 100);
    }

    #[test]
    fn test_json_match_result_from_match_result() {
        let match_result = MatchResult::with_metadata(
            "ELF 64-bit LSB executable".to_string(),
            0,
            4,
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            vec!["elf".to_string(), "elf64".to_string()],
            95,
            Some("application/x-executable".to_string()),
        );

        let json_result = JsonMatchResult::from_match_result(&match_result);

        assert_eq!(json_result.text, "ELF 64-bit LSB executable");
        assert_eq!(json_result.offset, 0);
        assert_eq!(json_result.value, "7f454c46");
        assert_eq!(json_result.tags, vec!["elf", "elf64"]);
        assert_eq!(json_result.score, 95);
    }

    #[test]
    fn test_json_match_result_add_tag() {
        let mut result = JsonMatchResult::new(
            "Archive".to_string(),
            0,
            "504b0304".to_string(),
            vec!["archive".to_string()],
            80,
        );

        result.add_tag("zip".to_string());
        result.add_tag("compressed".to_string());

        assert_eq!(result.tags, vec!["archive", "zip", "compressed"]);
    }

    #[test]
    fn test_json_match_result_set_score() {
        let mut result = JsonMatchResult::new("Test".to_string(), 0, "00".to_string(), vec![], 50);

        result.set_score(85);
        assert_eq!(result.score, 85);

        // Test clamping
        result.set_score(150);
        assert_eq!(result.score, 100);

        result.set_score(0);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_format_value_as_hex_bytes() {
        let value = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert_eq!(format_value_as_hex(&value), "7f454c46");

        let empty_bytes = Value::Bytes(vec![]);
        assert_eq!(format_value_as_hex(&empty_bytes), "");

        let single_byte = Value::Bytes(vec![0xff]);
        assert_eq!(format_value_as_hex(&single_byte), "ff");
    }

    #[test]
    fn test_format_value_as_hex_string() {
        let value = Value::String("PNG".to_string());
        assert_eq!(format_value_as_hex(&value), "504e47");

        let empty_string = Value::String(String::new());
        assert_eq!(format_value_as_hex(&empty_string), "");

        let unicode_string = Value::String("🦀".to_string());
        // Rust crab emoji in UTF-8: F0 9F A6 80
        assert_eq!(format_value_as_hex(&unicode_string), "f09fa680");
    }

    #[test]
    fn test_format_value_as_hex_uint() {
        let value = Value::Uint(0x1234);
        // Little-endian u64: 0x1234 -> 34 12 00 00 00 00 00 00
        assert_eq!(format_value_as_hex(&value), "3412000000000000");

        let zero = Value::Uint(0);
        assert_eq!(format_value_as_hex(&zero), "0000000000000000");

        let max_value = Value::Uint(u64::MAX);
        assert_eq!(format_value_as_hex(&max_value), "ffffffffffffffff");
    }

    #[test]
    fn test_format_value_as_hex_int() {
        let positive = Value::Int(0x1234);
        assert_eq!(format_value_as_hex(&positive), "3412000000000000");

        let negative = Value::Int(-1);
        // -1 as i64 in little-endian: FF FF FF FF FF FF FF FF
        assert_eq!(format_value_as_hex(&negative), "ffffffffffffffff");

        let zero = Value::Int(0);
        assert_eq!(format_value_as_hex(&zero), "0000000000000000");
    }

    #[test]
    fn test_json_output_new() {
        let matches = vec![
            JsonMatchResult::new(
                "Match 1".to_string(),
                0,
                "01".to_string(),
                vec!["tag1".to_string()],
                60,
            ),
            JsonMatchResult::new(
                "Match 2".to_string(),
                10,
                "02".to_string(),
                vec!["tag2".to_string()],
                70,
            ),
        ];

        let output = JsonOutput::new(matches);
        assert_eq!(output.matches.len(), 2);
        assert_eq!(output.matches[0].text, "Match 1");
        assert_eq!(output.matches[1].text, "Match 2");
    }

    #[test]
    fn test_json_output_from_evaluation_result() {
        let match_results = vec![
            MatchResult::with_metadata(
                "PNG image".to_string(),
                0,
                8,
                Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
                vec!["image".to_string(), "png".to_string()],
                90,
                Some("image/png".to_string()),
            ),
            MatchResult::with_metadata(
                "8-bit color".to_string(),
                25,
                1,
                Value::Uint(8),
                vec!["image".to_string(), "png".to_string(), "color".to_string()],
                75,
                None,
            ),
        ];

        let metadata = EvaluationMetadata::new(2048, 3.2, 15, 2);
        let eval_result = EvaluationResult::new(PathBuf::from("test.png"), match_results, metadata);

        let json_output = JsonOutput::from_evaluation_result(&eval_result);

        assert_eq!(json_output.matches.len(), 2);
        assert_eq!(json_output.matches[0].text, "PNG image");
        assert_eq!(json_output.matches[0].value, "89504e470d0a1a0a");
        assert_eq!(json_output.matches[0].tags, vec!["image", "png"]);
        assert_eq!(json_output.matches[0].score, 90);

        assert_eq!(json_output.matches[1].text, "8-bit color");
        assert_eq!(json_output.matches[1].value, "0800000000000000");
        assert_eq!(json_output.matches[1].tags, vec!["image", "png", "color"]);
        assert_eq!(json_output.matches[1].score, 75);
    }

    #[test]
    fn test_json_output_add_match() {
        let mut output = JsonOutput::new(vec![]);

        let match_result = JsonMatchResult::new(
            "PDF document".to_string(),
            0,
            "25504446".to_string(),
            vec!["document".to_string(), "pdf".to_string()],
            85,
        );

        output.add_match(match_result);
        assert_eq!(output.matches.len(), 1);
        assert_eq!(output.matches[0].text, "PDF document");
    }

    #[test]
    fn test_json_output_has_matches() {
        let empty_output = JsonOutput::new(vec![]);
        assert!(!empty_output.has_matches());

        let output_with_matches = JsonOutput::new(vec![JsonMatchResult::new(
            "Test".to_string(),
            0,
            "74657374".to_string(),
            vec![],
            50,
        )]);
        assert!(output_with_matches.has_matches());
    }

    #[test]
    fn test_json_output_match_count() {
        let empty_output = JsonOutput::new(vec![]);
        assert_eq!(empty_output.match_count(), 0);

        let matches = vec![
            JsonMatchResult::new("Match 1".to_string(), 0, "01".to_string(), vec![], 50),
            JsonMatchResult::new("Match 2".to_string(), 10, "02".to_string(), vec![], 60),
            JsonMatchResult::new("Match 3".to_string(), 20, "03".to_string(), vec![], 70),
        ];

        let output = JsonOutput::new(matches);
        assert_eq!(output.match_count(), 3);
    }

    #[test]
    fn test_json_match_result_serialization() {
        let result = JsonMatchResult::new(
            "JPEG image".to_string(),
            0,
            "ffd8".to_string(),
            vec!["image".to_string(), "jpeg".to_string()],
            80,
        );

        let json = serde_json::to_string(&result).expect("Failed to serialize JsonMatchResult");
        let deserialized: JsonMatchResult =
            serde_json::from_str(&json).expect("Failed to deserialize JsonMatchResult");

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_json_output_serialization() {
        let matches = vec![
            JsonMatchResult::new(
                "ELF executable".to_string(),
                0,
                "7f454c46".to_string(),
                vec!["executable".to_string(), "elf".to_string()],
                95,
            ),
            JsonMatchResult::new(
                "64-bit".to_string(),
                4,
                "02".to_string(),
                vec!["elf".to_string(), "64bit".to_string()],
                85,
            ),
        ];

        let output = JsonOutput::new(matches);

        let json = serde_json::to_string(&output).expect("Failed to serialize JsonOutput");
        let deserialized: JsonOutput =
            serde_json::from_str(&json).expect("Failed to deserialize JsonOutput");

        assert_eq!(output.matches.len(), deserialized.matches.len());
        assert_eq!(output.matches[0].text, deserialized.matches[0].text);
        assert_eq!(output.matches[1].text, deserialized.matches[1].text);
    }

    #[test]
    fn test_json_output_serialization_format() {
        let matches = vec![JsonMatchResult::new(
            "Test file".to_string(),
            0,
            "74657374".to_string(),
            vec!["test".to_string()],
            75,
        )];

        let output = JsonOutput::new(matches);
        let json = serde_json::to_string_pretty(&output).expect("Failed to serialize");

        // Verify the JSON structure matches the expected format
        assert!(json.contains("\"matches\""));
        assert!(json.contains("\"text\": \"Test file\""));
        assert!(json.contains("\"offset\": 0"));
        assert!(json.contains("\"value\": \"74657374\""));
        assert!(json.contains("\"tags\""));
        assert!(json.contains("\"test\""));
        assert!(json.contains("\"score\": 75"));
    }

    #[test]
    fn test_json_match_result_equality() {
        let result1 = JsonMatchResult::new(
            "Test".to_string(),
            0,
            "74657374".to_string(),
            vec!["test".to_string()],
            50,
        );

        let result2 = JsonMatchResult::new(
            "Test".to_string(),
            0,
            "74657374".to_string(),
            vec!["test".to_string()],
            50,
        );

        let result3 = JsonMatchResult::new(
            "Different".to_string(),
            0,
            "74657374".to_string(),
            vec!["test".to_string()],
            50,
        );

        assert_eq!(result1, result2);
        assert_ne!(result1, result3);
    }

    #[test]
    fn test_complex_json_conversion() {
        // Test conversion of a complex match result with all fields populated
        let match_result = MatchResult::with_metadata(
            "ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked"
                .to_string(),
            0,
            4,
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            vec![
                "executable".to_string(),
                "elf".to_string(),
                "elf64".to_string(),
                "x86_64".to_string(),
                "pie".to_string(),
                "dynamic".to_string(),
            ],
            98,
            Some("application/x-pie-executable".to_string()),
        );

        let json_result = JsonMatchResult::from_match_result(&match_result);

        assert_eq!(
            json_result.text,
            "ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked"
        );
        assert_eq!(json_result.offset, 0);
        assert_eq!(json_result.value, "7f454c46");
        assert_eq!(
            json_result.tags,
            vec!["executable", "elf", "elf64", "x86_64", "pie", "dynamic"]
        );
        assert_eq!(json_result.score, 98);
    }

    #[test]
    fn test_format_json_output_single_match() {
        let match_results = vec![MatchResult::with_metadata(
            "PNG image".to_string(),
            0,
            8,
            Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
            vec!["image".to_string(), "png".to_string()],
            90,
            Some("image/png".to_string()),
        )];

        let json_output = format_json_output(&match_results).expect("Failed to format JSON");

        // Verify JSON structure
        assert!(json_output.contains("\"matches\""));
        assert!(json_output.contains("\"text\": \"PNG image\""));
        assert!(json_output.contains("\"offset\": 0"));
        assert!(json_output.contains("\"value\": \"89504e470d0a1a0a\""));
        assert!(json_output.contains("\"tags\""));
        assert!(json_output.contains("\"image\""));
        assert!(json_output.contains("\"png\""));
        assert!(json_output.contains("\"score\": 90"));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 1);
        assert_eq!(parsed.matches[0].text, "PNG image");
        assert_eq!(parsed.matches[0].offset, 0);
        assert_eq!(parsed.matches[0].value, "89504e470d0a1a0a");
        assert_eq!(parsed.matches[0].tags, vec!["image", "png"]);
        assert_eq!(parsed.matches[0].score, 90);
    }

    #[test]
    fn test_format_json_output_multiple_matches() {
        let match_results = vec![
            MatchResult::with_metadata(
                "ELF 64-bit LSB executable".to_string(),
                0,
                4,
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
                vec!["executable".to_string(), "elf".to_string()],
                95,
                Some("application/x-executable".to_string()),
            ),
            MatchResult::with_metadata(
                "x86-64 architecture".to_string(),
                18,
                2,
                Value::Uint(0x3e00),
                vec!["elf".to_string(), "x86_64".to_string()],
                85,
                None,
            ),
            MatchResult::with_metadata(
                "dynamically linked".to_string(),
                16,
                2,
                Value::Uint(0x0200),
                vec!["elf".to_string(), "dynamic".to_string()],
                80,
                None,
            ),
        ];

        let json_output = format_json_output(&match_results).expect("Failed to format JSON");

        // Verify JSON structure contains all matches
        assert!(json_output.contains("\"text\": \"ELF 64-bit LSB executable\""));
        assert!(json_output.contains("\"text\": \"x86-64 architecture\""));
        assert!(json_output.contains("\"text\": \"dynamically linked\""));

        // Verify different offsets are preserved
        assert!(json_output.contains("\"offset\": 0"));
        assert!(json_output.contains("\"offset\": 18"));
        assert!(json_output.contains("\"offset\": 16"));

        // Verify different values are formatted correctly
        assert!(json_output.contains("\"value\": \"7f454c46\""));
        assert!(json_output.contains("\"value\": \"003e000000000000\""));
        assert!(json_output.contains("\"value\": \"0002000000000000\""));

        // Verify it's valid JSON with correct structure
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 3);

        // Verify first match
        assert_eq!(parsed.matches[0].text, "ELF 64-bit LSB executable");
        assert_eq!(parsed.matches[0].offset, 0);
        assert_eq!(parsed.matches[0].score, 95);

        // Verify second match
        assert_eq!(parsed.matches[1].text, "x86-64 architecture");
        assert_eq!(parsed.matches[1].offset, 18);
        assert_eq!(parsed.matches[1].score, 85);

        // Verify third match
        assert_eq!(parsed.matches[2].text, "dynamically linked");
        assert_eq!(parsed.matches[2].offset, 16);
        assert_eq!(parsed.matches[2].score, 80);
    }

    #[test]
    fn test_format_json_output_empty_matches() {
        let match_results: Vec<MatchResult> = vec![];

        let json_output = format_json_output(&match_results).expect("Failed to format JSON");

        // Verify JSON structure for empty matches
        assert!(json_output.contains("\"matches\": []"));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 0);
        assert!(!parsed.has_matches());
    }

    #[test]
    fn test_format_json_output_compact_single_match() {
        let match_results = vec![MatchResult::new(
            "JPEG image".to_string(),
            0,
            Value::Bytes(vec![0xff, 0xd8]),
        )];

        let json_output =
            format_json_output_compact(&match_results).expect("Failed to format compact JSON");

        // Verify it's compact (no newlines or extra spaces)
        assert!(!json_output.contains('\n'));
        assert!(!json_output.contains("  ")); // No double spaces

        // Verify it contains expected content
        assert!(json_output.contains("\"matches\""));
        assert!(json_output.contains("\"text\":\"JPEG image\""));
        assert!(json_output.contains("\"offset\":0"));
        assert!(json_output.contains("\"value\":\"ffd8\""));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 1);
        assert_eq!(parsed.matches[0].text, "JPEG image");
    }

    #[test]
    fn test_format_json_output_compact_multiple_matches() {
        let match_results = vec![
            MatchResult::new("Match 1".to_string(), 0, Value::String("test1".to_string())),
            MatchResult::new(
                "Match 2".to_string(),
                10,
                Value::String("test2".to_string()),
            ),
        ];

        let json_output =
            format_json_output_compact(&match_results).expect("Failed to format compact JSON");

        // Verify it's compact
        assert!(!json_output.contains('\n'));

        // Verify it contains both matches
        assert!(json_output.contains("\"text\":\"Match 1\""));
        assert!(json_output.contains("\"text\":\"Match 2\""));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 2);
    }

    #[test]
    fn test_format_json_output_compact_empty() {
        let match_results: Vec<MatchResult> = vec![];

        let json_output =
            format_json_output_compact(&match_results).expect("Failed to format compact JSON");

        // Verify it's compact and contains empty matches array
        assert!(!json_output.contains('\n'));
        assert!(json_output.contains("\"matches\":[]"));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 0);
    }

    #[test]
    fn test_format_json_output_field_mapping() {
        // Test that all fields are properly mapped from MatchResult to JSON
        let match_result = MatchResult::with_metadata(
            "Test file with all fields".to_string(),
            42,
            8,
            Value::Bytes(vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
            vec![
                "category".to_string(),
                "subcategory".to_string(),
                "specific".to_string(),
            ],
            75,
            Some("application/test".to_string()),
        );

        let json_output = format_json_output(&[match_result]).expect("Failed to format JSON");

        // Verify all fields are present and correctly mapped
        assert!(json_output.contains("\"text\": \"Test file with all fields\""));
        assert!(json_output.contains("\"offset\": 42"));
        assert!(json_output.contains("\"value\": \"0102030405060708\""));
        assert!(json_output.contains("\"tags\""));
        assert!(json_output.contains("\"category\""));
        assert!(json_output.contains("\"subcategory\""));
        assert!(json_output.contains("\"specific\""));
        assert!(json_output.contains("\"score\": 75"));

        // Verify the JSON structure matches the expected format
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 1);

        let json_match = &parsed.matches[0];
        assert_eq!(json_match.text, "Test file with all fields");
        assert_eq!(json_match.offset, 42);
        assert_eq!(json_match.value, "0102030405060708");
        assert_eq!(json_match.tags, vec!["category", "subcategory", "specific"]);
        assert_eq!(json_match.score, 75);
    }

    #[test]
    fn test_format_json_output_different_value_types() {
        let match_results = vec![
            MatchResult::new(
                "Bytes value".to_string(),
                0,
                Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]),
            ),
            MatchResult::new(
                "String value".to_string(),
                10,
                Value::String("Hello, World!".to_string()),
            ),
            MatchResult::new("Uint value".to_string(), 20, Value::Uint(0x1234_5678)),
            MatchResult::new("Int value".to_string(), 30, Value::Int(-42)),
        ];

        let json_output = format_json_output(&match_results).expect("Failed to format JSON");

        // Verify different value types are formatted correctly as hex
        assert!(json_output.contains("\"value\": \"deadbeef\""));
        assert!(json_output.contains("\"value\": \"48656c6c6f2c20576f726c6421\""));
        assert!(json_output.contains("\"value\": \"7856341200000000\""));
        assert!(json_output.contains("\"value\": \"d6ffffffffffffff\""));

        // Verify it's valid JSON
        let parsed: JsonOutput =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");
        assert_eq!(parsed.matches.len(), 4);
    }

    #[test]
    fn test_format_json_output_validation() {
        // Test that the output format matches the original libmagic JSON specification
        let match_result = MatchResult::with_metadata(
            "PDF document".to_string(),
            0,
            4,
            Value::String("%PDF".to_string()),
            vec!["document".to_string(), "pdf".to_string()],
            88,
            Some("application/pdf".to_string()),
        );

        let json_output = format_json_output(&[match_result]).expect("Failed to format JSON");

        // Parse and verify the structure matches the expected format
        let parsed: serde_json::Value =
            serde_json::from_str(&json_output).expect("Generated JSON should be valid");

        // Verify top-level structure
        assert!(parsed.is_object());
        assert!(parsed.get("matches").is_some());
        assert!(parsed.get("matches").unwrap().is_array());

        // Verify match structure
        let matches = parsed.get("matches").unwrap().as_array().unwrap();
        assert_eq!(matches.len(), 1);

        let match_obj = &matches[0];
        assert!(match_obj.get("text").is_some());
        assert!(match_obj.get("offset").is_some());
        assert!(match_obj.get("value").is_some());
        assert!(match_obj.get("tags").is_some());
        assert!(match_obj.get("score").is_some());

        // Verify field types
        assert!(match_obj.get("text").unwrap().is_string());
        assert!(match_obj.get("offset").unwrap().is_number());
        assert!(match_obj.get("value").unwrap().is_string());
        assert!(match_obj.get("tags").unwrap().is_array());
        assert!(match_obj.get("score").unwrap().is_number());

        // Verify field values
        assert_eq!(
            match_obj.get("text").unwrap().as_str().unwrap(),
            "PDF document"
        );
        assert_eq!(match_obj.get("offset").unwrap().as_u64().unwrap(), 0);
        assert_eq!(
            match_obj.get("value").unwrap().as_str().unwrap(),
            "25504446"
        );
        assert_eq!(match_obj.get("score").unwrap().as_u64().unwrap(), 88);

        let tags = match_obj.get("tags").unwrap().as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str().unwrap(), "document");
        assert_eq!(tags[1].as_str().unwrap(), "pdf");
    }
}
