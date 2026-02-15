// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Line preprocessing for magic file parsing.
//!
//! Handles comment removal, empty line filtering, line continuations,
//! and strength directive parsing during magic file preprocessing.

use crate::error::ParseError;
use crate::parser::ast::{MagicRule, StrengthModifier};
use crate::parser::grammar::{
    has_continuation, is_comment_line, is_empty_line, is_strength_directive, parse_comment,
    parse_magic_rule, parse_strength_directive,
};

/// Internal structure to track line metadata during preprocessing.
///
/// Stores the processed content, original line number, and flags for comment
/// and strength directive lines in the input magic file.
#[derive(Debug)]
pub(crate) struct LineInfo {
    pub(crate) content: String,
    pub(crate) line_number: usize,
    pub(crate) is_comment: bool,
    /// Optional strength modifier parsed from `!:strength` directive
    pub(crate) strength_modifier: Option<StrengthModifier>,
}

impl LineInfo {
    pub(crate) fn new(content: String, line_number: usize, is_comment: bool) -> Self {
        Self {
            content,
            line_number,
            is_comment,
            strength_modifier: None,
        }
    }

    pub(crate) fn with_strength(
        content: String,
        line_number: usize,
        strength_modifier: StrengthModifier,
    ) -> Self {
        Self {
            content,
            line_number,
            is_comment: false,
            strength_modifier: Some(strength_modifier),
        }
    }
}

/// Preprocesses raw magic file input by handling comments, empty lines, and continuations.
///
/// This function performs the following transformations:
/// - Removes empty lines from the input
/// - Handles comment lines (lines starting with '#')
/// - Processes line continuations (lines ending with '\')
/// - Concatenates continued lines into single entries
/// - Preserves original line numbers for error reporting (continued lines
///   are assigned the line number of the first line in the continuation sequence)
///
/// # Arguments
///
/// * `input` - The raw magic file content as a string
///
/// # Returns
///
/// `Result<Vec<LineInfo>, ParseError>` - A vector of processed lines or a parse error
///
/// # Errors
///
/// Returns an error if:
/// - Comment lines cannot be parsed
/// - Input ends with an unterminated line continuation
/// - The input is malformed
pub(crate) fn preprocess_lines(input: &str) -> Result<Vec<LineInfo>, ParseError> {
    let mut lines_info: Vec<LineInfo> = Vec::new();
    let mut line_buf = String::new();
    let mut start_line_number: Option<usize> = None;
    let mut last_line_num = 0usize;
    for (i, mut line) in input.lines().enumerate() {
        last_line_num = i + 1;
        if is_empty_line(line) {
            continue;
        }
        if is_comment_line(line) {
            // Bug 1 fix: If we have an ongoing continuation, discard it before processing comment
            if !line_buf.is_empty() {
                line_buf.clear();
                start_line_number = None;
            }
            let parsed_comment = parse_comment(line)
                .map_err(|_| ParseError::invalid_syntax(i + 1, "Unable to parse comment"))?;
            line = parsed_comment.1.as_str();
            lines_info.push(LineInfo::new(line.trim().to_string(), i + 1, true));
            continue;
        }
        // Handle strength directives (!:strength ...)
        if is_strength_directive(line) {
            // If we have an ongoing continuation, discard it before processing directive
            if !line_buf.is_empty() {
                line_buf.clear();
                start_line_number = None;
            }
            let strength_modifier = parse_strength_directive(line)
                .map_err(|e| {
                    ParseError::invalid_syntax(
                        i + 1,
                        format!("Failed to parse strength directive: {e}"),
                    )
                })?
                .1;
            lines_info.push(LineInfo::with_strength(
                line.trim().to_string(),
                i + 1,
                strength_modifier,
            ));
            continue;
        }
        // Track the starting line number when we begin accumulating a rule
        if start_line_number.is_none() {
            start_line_number = Some(i + 1);
        }
        line_buf.push_str(line.trim());
        if has_continuation(line) {
            // Remove trailing backslash in-place (O(1)) instead of
            // strip_suffix().to_string() which allocates a new String
            line_buf.pop();
            continue;
        }
        // Bug 2 fix: Use the stored starting line number instead of calculating from cont_ctr
        let rule_line_number = start_line_number.unwrap_or(i + 1);
        lines_info.push(LineInfo::new(
            std::mem::take(&mut line_buf),
            rule_line_number,
            false,
        ));
        start_line_number = None;
    }

    // Handle unterminated continuation at end of input
    if !line_buf.is_empty() {
        return Err(ParseError::invalid_syntax(
            last_line_num,
            "Unterminated line continuation",
        ));
    }

    Ok(lines_info)
}

/// Parses a single magic rule line into a `MagicRule` AST node.
///
/// This function takes a preprocessed `LineInfo` and converts it into a `MagicRule`
/// by delegating to the grammar parser. It handles error mapping to include
/// context about which line failed.
///
/// # Arguments
///
/// * `line` - The `LineInfo` struct containing the rule text and metadata
///
/// # Returns
///
/// `Result<MagicRule, ParseError>` - The parsed rule or a parse error
///
/// # Errors
///
/// Returns an error if:
/// - The line is marked as a comment
/// - The rule syntax is invalid
/// - Required fields are missing
/// - Value parsing fails
pub(crate) fn parse_magic_rule_line(line: &LineInfo) -> Result<MagicRule, ParseError> {
    if line.is_comment {
        return Err(ParseError::invalid_syntax(
            line.line_number,
            "Comment lines cannot be parsed as rules",
        ));
    }
    parse_magic_rule(&line.content)
        .map_err(|e| {
            ParseError::invalid_syntax(line.line_number, format!("Failed to parse rule: {e}"))
        })
        .map(|(_, rule)| rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{OffsetSpec, Operator, TypeKind};

    fn li(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: false,
            strength_modifier: None,
        }
    }

    fn li_comment(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: true,
            strength_modifier: None,
        }
    }

    // ============================================================
    // Tests for parse_magic_rule_line (12 test cases)
    // ============================================================

    #[test]
    fn test_parse_magic_rule_line_simple_string() {
        let line = li(1, "0 string \\x7fELF ELF executable");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 0);
        assert_eq!(rule.message, "ELF executable");
    }

    #[test]
    fn test_parse_magic_rule_line_byte_type() {
        let line = li(1, "0 byte 1 ELF");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 0);
        assert!(matches!(rule.typ, TypeKind::Byte));
    }

    #[test]
    fn test_parse_magic_rule_line_with_child_indentation() {
        let line = li(2, ">4 byte 1 32-bit");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 1);
    }

    #[test]
    fn test_parse_magic_rule_line_deep_indentation() {
        let line = li(3, ">>>8 long = 0x12345678 Complex match");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 3);
    }

    #[test]
    fn test_parse_magic_rule_line_not_equal_operator() {
        let line = li(1, "0 byte != 0 Non-zero");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::NotEqual);
    }

    #[test]
    fn test_parse_magic_rule_line_greater_operator() {
        let line = li(1, "0 long = 1000 Number");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::Equal);
    }

    #[test]
    fn test_parse_magic_rule_line_less_operator() {
        let line = li(1, "0 long != 256 Not equal");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::NotEqual);
    }

    #[test]
    fn test_parse_magic_rule_line_bitwise_and_operator() {
        let line = li(1, "0 byte & 0xFF Bitmask");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::BitwiseAnd);
    }

    #[test]
    fn test_parse_magic_rule_line_comment_line_error() {
        let line = li_comment(1, "This is a comment");
        let result = parse_magic_rule_line(&line);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_magic_rule_line_hex_offset() {
        let line = li(1, "0x100 byte 1 PDF document");
        let rule = parse_magic_rule_line(&line).unwrap();
        match rule.offset {
            OffsetSpec::Absolute(offset) => assert_eq!(offset, 0x100),
            _ => panic!("Expected absolute offset"),
        }
    }

    #[test]
    fn test_parse_magic_rule_line_string_with_spaces() {
        let line = li(1, "0 byte 1 Long message with multiple words");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.message, "Long message with multiple words");
    }

    #[test]
    fn test_parse_magic_rule_line_short_type() {
        let line = li(1, "0 short 0x4d5a MS-DOS executable");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert!(matches!(rule.typ, TypeKind::Short { .. }));
    }

    // ============================================================
    // Tests for preprocess_lines (12 test cases)
    // ============================================================

    #[test]
    fn test_preprocess_lines_single_rule() {
        let input = "0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test");
        assert!(!lines[0].is_comment);
    }

    #[test]
    fn test_preprocess_lines_multiple_rules() {
        let input = "0 string 0 Test\n0 byte 1 Byte";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].content, "0 string 0 Test");
        assert_eq!(lines[1].content, "0 byte 1 Byte");
    }

    #[test]
    fn test_preprocess_lines_with_comments() {
        let input = "# Comment\n0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].is_comment);
        assert!(!lines[1].is_comment);
    }

    #[test]
    fn test_preprocess_lines_empty_lines() {
        let input = "0 string 0 Test\n\n0 byte 1 Byte";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_preprocess_lines_leading_empty_lines() {
        let input = "\n\n0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test");
    }

    #[test]
    fn test_preprocess_lines_trailing_empty_lines() {
        let input = "0 string 0 Test\n\n";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_preprocess_lines_line_continuation() {
        let input = "0 string 0 Long message \\\ncontinued here";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Long message continued here");
    }

    #[test]
    fn test_preprocess_lines_multiple_continuations() {
        let input = "0 string 0 Multi \\\nline \\\ncontinuation";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Multi line continuation");
    }

    #[test]
    fn test_preprocess_lines_mixed_comments_and_rules() {
        let input = "# Header\n0 string 0 Test\n# Another comment\n>4 byte 1 Child";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].is_comment);
        assert!(!lines[1].is_comment);
        assert!(lines[2].is_comment);
        assert!(!lines[3].is_comment);
    }

    #[test]
    fn test_preprocess_lines_preserves_line_numbers() {
        let input = "0 string 0 Test\n>4 byte 1 Child";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[1].line_number, 2);
    }

    #[test]
    fn test_preprocess_lines_empty_input() {
        let input = "";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_preprocess_lines_only_comments() {
        let input = "# Comment 1\n# Comment 2\n# Comment 3";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.is_comment));
    }

    // ============================================================
    // Continuation edge case tests
    // ============================================================

    #[test]
    fn test_continuation_at_eof() {
        // Continuation on last line with no following line - should error
        let input = "0 string 0 Test \\";
        let result = preprocess_lines(input);
        assert!(
            result.is_err(),
            "Should error on unterminated continuation at EOF"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{err:?}").contains("Unterminated"),
            "Error should mention unterminated continuation"
        );
    }

    #[test]
    fn test_continuation_with_empty_next() {
        // Empty line after continuation causes unterminated continuation
        // (empty lines are skipped but continuation state persists)
        let input = "0 string 0 Test \\\n\n0 byte 1 Next";
        let lines = preprocess_lines(input).unwrap();
        // The continuation carries through the empty line, so "Next" gets appended
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test 0 byte 1 Next");
    }

    #[test]
    fn test_continuation_into_empty_then_rule() {
        let input = "0 string 0 First \\\n\ncontinued";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 First continued");
    }

    // ============================================================
    // Line number accuracy tests
    // ============================================================

    #[test]
    fn test_line_numbers_with_continuations() {
        let input = "0 string 0 test1\n0 string 0 multi \\\nline \\\ntest\n0 string 0 test2";
        let lines = preprocess_lines(input).unwrap();

        // Line 1: "0 string 0 test1" should report line 1
        assert_eq!(lines[0].line_number, 1);

        // Line 2-4 continuation should report line 2 (first line of continuation)
        assert_eq!(lines[1].line_number, 2);

        // Line 5: "0 string 0 test2" should report line 5
        assert_eq!(lines[2].line_number, 5);
    }

    #[test]
    fn test_line_numbers_with_mixed_content() {
        let input = "# Comment line 1\n0 string 0 rule1\n\n# Another comment\n0 string 0 rule2 \\\ncontinued";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].line_number, 1); // Comment
        assert_eq!(lines[1].line_number, 2); // rule1
        assert_eq!(lines[2].line_number, 4); // Another comment
        assert_eq!(lines[3].line_number, 5); // rule2 (continued on line 6)
    }

    // ============================================================
    // Bug reproduction tests
    // ============================================================

    #[test]
    fn test_bug1_comment_during_continuation() {
        // Bug 1: Comment during continuation should not corrupt line_buf
        // The partial rule should be discarded, leaving only the comment and new rule
        let input = "0 string 0 Partial rule \\\n# This is a comment\n0 byte 1 New rule";
        let lines = preprocess_lines(input).unwrap();

        // The partial rule is discarded, so we should have 2 lines: comment and new rule
        assert_eq!(lines.len(), 2);
        // The comment should be separate and not contain rule content
        let comment_line = lines.iter().find(|l| l.is_comment).unwrap();
        assert!(!comment_line.content.contains("Partial rule"));
        assert_eq!(comment_line.content, "This is a comment");
        // The new rule should be intact
        let rule_line = lines
            .iter()
            .find(|l| !l.is_comment && l.content.contains("New rule"))
            .unwrap();
        assert_eq!(rule_line.content, "0 byte 1 New rule");
    }

    #[test]
    fn test_bug2_empty_line_in_continuation() {
        // Bug 2: Empty line in continuation should not break line number calculation
        let input = "0 string 0 Test \\\n\ncontinued here";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 1);
        // Line number should point to line 1 (where the rule started), not line 3
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].content, "0 string 0 Test continued here");
    }

    #[test]
    fn test_bug2_multiple_empty_lines_in_continuation() {
        // Multiple empty lines in continuation
        let input = "0 string 0 Test \\\n\n\ncontinued here";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 1);
        // Line number should still point to line 1
        assert_eq!(lines[0].line_number, 1);
    }
}
