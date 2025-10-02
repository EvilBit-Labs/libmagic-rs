//! Text output formatting for magic rule evaluation results
//!
//! This module provides functionality to format evaluation results in a human-readable
//! text format compatible with the GNU `file` command output style.

use crate::output::{EvaluationResult, MatchResult};

/// Format a single match result as text
///
/// Converts a match result into a human-readable string format similar to
/// the GNU `file` command output. The format includes the message from the
/// matching rule.
///
/// # Arguments
///
/// * `result` - The match result to format
///
/// # Returns
///
/// A formatted string containing the match message
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{MatchResult, text::format_text_result};
/// use libmagic_rs::parser::ast::Value;
///
/// let result = MatchResult::new(
///     "ELF 64-bit LSB executable".to_string(),
///     0,
///     Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])
/// );
///
/// let formatted = format_text_result(&result);
/// assert_eq!(formatted, "ELF 64-bit LSB executable");
/// ```
#[must_use]
pub fn format_text_result(result: &MatchResult) -> String {
    result.message.clone()
}

/// Format multiple match results as concatenated text
///
/// Combines multiple match results into a single text string, with messages
/// separated by commas and spaces. This follows the GNU `file` command convention
/// of showing hierarchical matches in a single line.
///
/// # Arguments
///
/// * `results` - Vector of match results to format
///
/// # Returns
///
/// A formatted string with all match messages concatenated
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{MatchResult, text::format_text_output};
/// use libmagic_rs::parser::ast::Value;
///
/// let results = vec![
///     MatchResult::new(
///         "ELF 64-bit LSB executable".to_string(),
///         0,
///         Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])
///     ),
///     MatchResult::new(
///         "x86-64".to_string(),
///         18,
///         Value::Uint(0x3e)
///     ),
///     MatchResult::new(
///         "dynamically linked".to_string(),
///         16,
///         Value::Uint(0x02)
///     ),
/// ];
///
/// let formatted = format_text_output(&results);
/// assert_eq!(formatted, "ELF 64-bit LSB executable, x86-64, dynamically linked");
/// ```
#[must_use]
pub fn format_text_output(results: &[MatchResult]) -> String {
    if results.is_empty() {
        return "data".to_string(); // Default fallback for unknown files
    }

    results
        .iter()
        .map(|result| result.message.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// Format an evaluation result as text with filename
///
/// Formats a complete evaluation result in the style of the GNU `file` command,
/// including the filename followed by a colon and the formatted match results.
///
/// # Arguments
///
/// * `evaluation` - The evaluation result to format
///
/// # Returns
///
/// A formatted string in the format "filename: description"
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::{EvaluationResult, MatchResult, EvaluationMetadata, text::format_evaluation_result};
/// use libmagic_rs::parser::ast::Value;
/// use std::path::PathBuf;
///
/// let result = MatchResult::new(
///     "PNG image data".to_string(),
///     0,
///     Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47])
/// );
///
/// let metadata = EvaluationMetadata::new(2048, 1.5, 10, 1);
/// let evaluation = EvaluationResult::new(
///     PathBuf::from("image.png"),
///     vec![result],
///     metadata
/// );
///
/// let formatted = format_evaluation_result(&evaluation);
/// assert_eq!(formatted, "image.png: PNG image data");
/// ```
#[must_use]
pub fn format_evaluation_result(evaluation: &EvaluationResult) -> String {
    let filename = evaluation
        .filename
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let description = if evaluation.matches.is_empty() {
        if let Some(ref error) = evaluation.error {
            format!("ERROR: {error}")
        } else {
            "data".to_string()
        }
    } else {
        format_text_output(&evaluation.matches)
    };

    format!("{filename}: {description}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::EvaluationMetadata;
    use crate::parser::ast::Value;
    use std::path::PathBuf;

    #[test]
    fn test_format_text_result() {
        let result = MatchResult::new(
            "ELF 64-bit LSB executable".to_string(),
            0,
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
        );

        let formatted = format_text_result(&result);
        assert_eq!(formatted, "ELF 64-bit LSB executable");
    }

    #[test]
    fn test_format_text_result_with_special_characters() {
        let result = MatchResult::new(
            "Text file with UTF-8 Unicode (with BOM) text".to_string(),
            0,
            Value::Bytes(vec![0xef, 0xbb, 0xbf]),
        );

        let formatted = format_text_result(&result);
        assert_eq!(formatted, "Text file with UTF-8 Unicode (with BOM) text");
    }

    #[test]
    fn test_format_text_output_single_result() {
        let results = vec![MatchResult::new(
            "PNG image data".to_string(),
            0,
            Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47]),
        )];

        let formatted = format_text_output(&results);
        assert_eq!(formatted, "PNG image data");
    }

    #[test]
    fn test_format_text_output_multiple_results() {
        let results = vec![
            MatchResult::new(
                "ELF 64-bit LSB executable".to_string(),
                0,
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            ),
            MatchResult::new("x86-64".to_string(), 18, Value::Uint(0x3e)),
            MatchResult::new("version 1 (SYSV)".to_string(), 7, Value::Uint(0x01)),
            MatchResult::new("dynamically linked".to_string(), 16, Value::Uint(0x02)),
        ];

        let formatted = format_text_output(&results);
        assert_eq!(
            formatted,
            "ELF 64-bit LSB executable, x86-64, version 1 (SYSV), dynamically linked"
        );
    }

    #[test]
    fn test_format_text_output_empty_results() {
        let results = vec![];
        let formatted = format_text_output(&results);
        assert_eq!(formatted, "data");
    }

    #[test]
    fn test_format_text_output_with_confidence_variations() {
        // Test that confidence doesn't affect text output (it's not shown in text format)
        let results = vec![
            MatchResult::with_metadata(
                "JPEG image data".to_string(),
                0,
                2,
                Value::Bytes(vec![0xff, 0xd8]),
                vec!["image".to_string(), "jpeg".to_string()],
                95,
                Some("image/jpeg".to_string()),
            ),
            MatchResult::with_metadata(
                "JFIF standard 1.01".to_string(),
                6,
                5,
                Value::String("JFIF\0".to_string()),
                vec!["image".to_string(), "jpeg".to_string(), "jfif".to_string()],
                85,
                None,
            ),
        ];

        let formatted = format_text_output(&results);
        assert_eq!(formatted, "JPEG image data, JFIF standard 1.01");
    }

    #[test]
    fn test_format_evaluation_result_with_matches() {
        let result = MatchResult::new(
            "PNG image data".to_string(),
            0,
            Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47]),
        );

        let metadata = EvaluationMetadata::new(2048, 1.5, 10, 1);
        let evaluation = EvaluationResult::new(
            PathBuf::from("/home/user/images/photo.png"),
            vec![result],
            metadata,
        );

        let formatted = format_evaluation_result(&evaluation);
        assert_eq!(formatted, "photo.png: PNG image data");
    }

    #[test]
    fn test_format_evaluation_result_with_multiple_matches() {
        let results = vec![
            MatchResult::new(
                "ELF 64-bit LSB executable".to_string(),
                0,
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            ),
            MatchResult::new("x86-64".to_string(), 18, Value::Uint(0x3e)),
            MatchResult::new("dynamically linked".to_string(), 16, Value::Uint(0x02)),
        ];

        let metadata = EvaluationMetadata::new(8192, 3.2, 25, 3);
        let evaluation = EvaluationResult::new(PathBuf::from("/usr/bin/ls"), results, metadata);

        let formatted = format_evaluation_result(&evaluation);
        assert_eq!(
            formatted,
            "ls: ELF 64-bit LSB executable, x86-64, dynamically linked"
        );
    }

    #[test]
    fn test_format_evaluation_result_no_matches() {
        let metadata = EvaluationMetadata::new(1024, 0.8, 5, 0);
        let evaluation = EvaluationResult::new(PathBuf::from("unknown.bin"), vec![], metadata);

        let formatted = format_evaluation_result(&evaluation);
        assert_eq!(formatted, "unknown.bin: data");
    }

    #[test]
    fn test_format_evaluation_result_with_error() {
        let metadata = EvaluationMetadata::new(0, 0.0, 0, 0);
        let evaluation = EvaluationResult::with_error(
            PathBuf::from("missing.txt"),
            "File not found".to_string(),
            metadata,
        );

        let formatted = format_evaluation_result(&evaluation);
        assert_eq!(formatted, "missing.txt: ERROR: File not found");
    }

    #[test]
    fn test_format_evaluation_result_filename_extraction() {
        // Test various path formats
        let metadata = EvaluationMetadata::new(100, 0.5, 1, 0);

        // Unix absolute path
        let eval1 = EvaluationResult::new(
            PathBuf::from("/home/user/document.pdf"),
            vec![],
            metadata.clone(),
        );
        let formatted1 = format_evaluation_result(&eval1);
        assert_eq!(formatted1, "document.pdf: data");

        // Windows path - on Unix systems, this will be treated as a single component
        // so we need to handle this case differently
        let eval2 = EvaluationResult::new(
            PathBuf::from(r"C:\Users\user\file.exe"),
            vec![],
            metadata.clone(),
        );
        let formatted2 = format_evaluation_result(&eval2);
        // On Unix systems, Windows paths are treated as single components
        // so we expect the full path as the filename
        assert_eq!(formatted2, r"C:\Users\user\file.exe: data");

        // Relative path
        let eval3 =
            EvaluationResult::new(PathBuf::from("./test/sample.dat"), vec![], metadata.clone());
        let formatted3 = format_evaluation_result(&eval3);
        assert_eq!(formatted3, "sample.dat: data");

        // Just filename
        let eval4 = EvaluationResult::new(PathBuf::from("simple.txt"), vec![], metadata);
        let formatted4 = format_evaluation_result(&eval4);
        assert_eq!(formatted4, "simple.txt: data");
    }

    #[test]
    fn test_format_evaluation_result_edge_cases() {
        let metadata = EvaluationMetadata::new(0, 0.0, 0, 0);

        // Empty filename (should use "unknown")
        let eval1 = EvaluationResult::new(PathBuf::from(""), vec![], metadata.clone());
        let formatted1 = format_evaluation_result(&eval1);
        assert_eq!(formatted1, "unknown: data");

        // Path with no filename component
        let eval2 = EvaluationResult::new(PathBuf::from("/"), vec![], metadata);
        let formatted2 = format_evaluation_result(&eval2);
        assert_eq!(formatted2, "unknown: data");
    }

    #[test]
    fn test_format_text_output_preserves_message_order() {
        // Ensure that the order of messages is preserved in output
        let results = vec![
            MatchResult::new("First".to_string(), 0, Value::Uint(1)),
            MatchResult::new("Second".to_string(), 4, Value::Uint(2)),
            MatchResult::new("Third".to_string(), 8, Value::Uint(3)),
        ];

        let formatted = format_text_output(&results);
        assert_eq!(formatted, "First, Second, Third");
    }

    #[test]
    fn test_format_text_result_handles_empty_message() {
        let result = MatchResult::new(String::new(), 0, Value::Uint(0));
        let formatted = format_text_result(&result);
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_text_output_with_empty_messages() {
        let results = vec![
            MatchResult::new("Valid message".to_string(), 0, Value::Uint(1)),
            MatchResult::new(String::new(), 4, Value::Uint(2)),
            MatchResult::new("Another message".to_string(), 8, Value::Uint(3)),
        ];

        let formatted = format_text_output(&results);
        assert_eq!(formatted, "Valid message, , Another message");
    }

    #[test]
    fn test_format_text_output_realistic_file_types() {
        // Test with realistic file type detection results

        // PDF file
        let pdf_results = vec![
            MatchResult::new(
                "PDF document".to_string(),
                0,
                Value::String("%PDF".to_string()),
            ),
            MatchResult::new(
                "version 1.4".to_string(),
                5,
                Value::String("1.4".to_string()),
            ),
        ];
        assert_eq!(
            format_text_output(&pdf_results),
            "PDF document, version 1.4"
        );

        // ZIP archive
        let zip_results = vec![
            MatchResult::new(
                "Zip archive data".to_string(),
                0,
                Value::String("PK".to_string()),
            ),
            MatchResult::new("at least v2.0 to extract".to_string(), 4, Value::Uint(20)),
        ];
        assert_eq!(
            format_text_output(&zip_results),
            "Zip archive data, at least v2.0 to extract"
        );

        // JPEG image
        let jpeg_results = vec![
            MatchResult::new(
                "JPEG image data".to_string(),
                0,
                Value::Bytes(vec![0xff, 0xd8]),
            ),
            MatchResult::new(
                "JFIF standard 1.01".to_string(),
                6,
                Value::String("JFIF".to_string()),
            ),
            MatchResult::new("resolution (DPI)".to_string(), 13, Value::Uint(1)),
            MatchResult::new("density 72x72".to_string(), 14, Value::Uint(72)),
        ];
        assert_eq!(
            format_text_output(&jpeg_results),
            "JPEG image data, JFIF standard 1.01, resolution (DPI), density 72x72"
        );
    }
}
