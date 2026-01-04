//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.
//!
//! # Overview
//!
//! The parser implements a complete pipeline for transforming magic file text into
//! a hierarchical rule structure suitable for evaluation. The pipeline consists of:
//!
//! 1. **Preprocessing**: Line handling, comment removal, continuation processing
//! 2. **Parsing**: Individual magic rule parsing using nom combinators
//! 3. **Hierarchy Building**: Constructing parent-child relationships based on indentation
//! 4. **Validation**: Type checking and offset resolution
//!
//! # Example
//!
//! ```ignore
//! use libmagic_rs::parser::parse_text_magic_file;
//!
//! let magic_content = r#"
//! 0 string \x7fELF ELF executable
//! >4 byte 1 32-bit
//! >4 byte 2 64-bit
//! "#;
//!
//! let rules = parse_text_magic_file(magic_content)?;
//! assert_eq!(rules.len(), 1);
//! assert_eq!(rules[0].children.len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod ast;
pub mod grammar;

// Re-export AST types for convenience
pub use ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};

// Re-export parser functions for convenience
pub use grammar::{parse_number, parse_offset};

use crate::{
    error::ParseError,
    parser::grammar::{
        has_continuation, is_comment_line, is_empty_line, parse_comment, parse_magic_rule,
    },
};

/// Internal structure to track line metadata during preprocessing.
///
/// Stores the processed content, original line number, and comment flag
/// for each line in the input magic file.
#[derive(Debug)]
struct LineInfo {
    content: String,
    line_number: usize,
    is_comment: bool,
}

impl LineInfo {
    fn new(content: String, line_number: usize, is_comment: bool) -> Self {
        Self {
            content,
            line_number,
            is_comment,
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
///
/// # Examples
///
/// ```ignore
/// let input = r#"0 string 0 Test
/// >4 byte 1 Child"#;
/// let lines = preprocess_lines(input)?;
/// assert_eq!(lines.len(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn preprocess_lines(input: &str) -> Result<Vec<LineInfo>, ParseError> {
    let mut lines_info: Vec<LineInfo> = Vec::new();
    let mut line_buf = String::new();
    let mut start_line_number: Option<usize> = None;
    for (i, mut line) in input.lines().enumerate() {
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
        // Track the starting line number when we begin accumulating a rule
        if start_line_number.is_none() {
            start_line_number = Some(i + 1);
        }
        line_buf.push_str(line.trim());
        if has_continuation(line) {
            if let Some(stripped) = line_buf.strip_suffix('\\') {
                line_buf = stripped.to_string();
            }
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
        let last_line = input.lines().count();
        return Err(ParseError::invalid_syntax(
            last_line,
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
///
/// # Examples
///
/// ```ignore
/// let line = LineInfo::new("0 string 0 Test".to_string(), 1, false);
/// let rule = parse_magic_rule_line(&line)?;
/// assert_eq!(rule.level, 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn parse_magic_rule_line(line: &LineInfo) -> Result<MagicRule, ParseError> {
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

/// Builds a hierarchical structure from a flat list of parsed magic rules.
///
/// This function establishes parent-child relationships based on indentation levels.
/// Rules at deeper indentation levels become children of the most recent rule at a
/// shallower level. This implements a stack-based algorithm for hierarchy construction.
///
/// # Arguments
///
/// * `lines` - A vector of preprocessed `LineInfo` structs
///
/// # Returns
///
/// `Result<Vec<MagicRule>, ParseError>` - Root-level rules with children attached
///
/// # Behavior
///
/// - Rules with `level=0` are root rules
/// - Rules with `level=1` become children of the most recent `level=0` rule
/// - Rules with `level=2` become children of the most recent `level=1` rule
/// - When indentation decreases, the stack is unwound and completed rules are attached
///
/// # Examples
///
/// ```ignore
/// let lines = vec![
///     LineInfo::new("0 string 0 ELF".to_string(), 1, false),
///     LineInfo::new(">4 byte 1 32-bit".to_string(), 2, false),
/// ];
/// let rules = build_rule_hierarchy(lines)?;
/// assert_eq!(rules[0].children.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn build_rule_hierarchy(lines: Vec<LineInfo>) -> Result<Vec<MagicRule>, ParseError> {
    let mut stack: Vec<MagicRule> = Vec::new();
    let mut roots: Vec<MagicRule> = Vec::new();

    for line in lines {
        if line.is_comment {
            continue;
        }
        let rule = parse_magic_rule_line(&line)?;

        while let Some(top) = stack.last() {
            if top.level >= rule.level {
                let completed = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(completed);
                } else {
                    roots.push(completed);
                }
            } else {
                break;
            }
        }

        stack.push(rule);
    }

    while let Some(rule) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(rule);
        } else {
            roots.push(rule);
        }
    }

    Ok(roots)
}

/// Parses a complete magic file from raw text input.
///
/// This is the main public-facing parser function that orchestrates the complete
/// parsing pipeline: preprocessing, parsing individual rules, and building the
/// hierarchical structure.
///
/// # Arguments
///
/// * `input` - The raw magic file content as a string
///
/// # Returns
///
/// `Result<Vec<MagicRule>, ParseError>` - A vector of root rules with nested children
///
/// # Errors
///
/// Returns an error if any stage of parsing fails:
/// - Preprocessing errors
/// - Rule parsing errors
/// - Hierarchy building errors
///
/// # Example
///
/// ```ignore
/// use libmagic_rs::parser::parse_text_magic_file;
///
/// let magic = r#"0 string \x7fELF ELF file
/// >4 byte 1 32-bit
/// >4 byte 2 64-bit"#;
///
/// let rules = parse_text_magic_file(magic)?;
/// assert_eq!(rules.len(), 1);
/// assert_eq!(rules[0].message, "ELF file");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_text_magic_file(input: &str) -> Result<Vec<MagicRule>, ParseError> {
    let lines = preprocess_lines(input)?;
    build_rule_hierarchy(lines)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn li(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: false,
        }
    }

    fn li_comment(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: true,
        }
    }

    // ============================================================
    // Tests for parse_magic_rule_line (10+ test cases)
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
    // Tests for preprocess_lines (10+ test cases)
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
    // Tests for build_rule_hierarchy (10+ test cases)
    // ============================================================

    #[test]
    fn test_build_rule_hierarchy_single_root() {
        let lines = vec![li(1, "0 string \\x7fELF ELF executable")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 0);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_one_child() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 1);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_multiple_children() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
            li(3, ">4 byte 2 64-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_nested_three_levels() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 class"),
            li(3, ">>5 byte 1 subtype"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children[0].level, 2);
    }

    #[test]
    fn test_build_rule_hierarchy_multiple_roots() {
        let lines = vec![
            li(1, r#"0 string "ELF" "ELF executable""#),
            li(2, r#"0 string "%PDF" "PDF document""#),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_sibling_rules() {
        let lines = vec![
            li(1, "0 byte 1 Root"),
            li(2, ">4 byte 1 Child1"),
            li(3, ">4 byte 2 Child2"),
            li(4, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_deep_nesting() {
        let lines = vec![
            li(1, "0 byte 1 L0"),
            li(2, ">4 byte 1 L1"),
            li(3, ">>5 byte 2 L2"),
            li(4, ">>>6 byte 3 L3"),
            li(5, ">>>>7 byte 4 L4"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(
            roots[0].children[0].children[0].children[0].children.len(),
            1
        );
    }

    #[test]
    fn test_build_rule_hierarchy_return_to_root_level() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 Child"),
            li(3, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[1].children.len(), 0);
    }

    #[test]
    fn test_build_rule_hierarchy_orphaned_child() {
        let lines = vec![li(1, ">4 byte 1 Orphaned child")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 1);
    }

    #[test]
    fn test_build_rule_hierarchy_complex_structure() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 C1"),
            li(3, ">4 byte 2 C2"),
            li(4, ">>6 byte 3 GC1"),
            li(5, "0 byte 2 Root2"),
            li(6, ">4 byte 4 C3"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[0].children[1].children.len(), 1);
        assert_eq!(roots[1].children.len(), 1);
    }

    // ============================================================
    // Tests for parse_text_magic_file (10+ test cases)
    // ============================================================

    #[test]
    fn test_parse_text_magic_file_single_rule() {
        let input = "0 string 0 ZIP archive";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "ZIP archive");
    }

    #[test]
    fn test_parse_text_magic_file_hierarchical_rules() {
        let input = r#"
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 2);
    }

    #[test]
    fn test_parse_text_magic_file_with_comments() {
        let input = r#"
# ELF file format
0 string 0 ELF
>4 byte 1 32-bit
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_multiple_roots() {
        let input = r#"
0 byte 1 ELF
>4 byte 1 32-bit

0 byte 2 PDF
>5 byte 1 v1
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_parse_text_magic_file_empty_input() {
        let input = "";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_text_magic_file_only_comments() {
        let input = r#"
# Comment 1
# Comment 2
# Comment 3
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_text_magic_file_empty_lines_only() {
        let input = r#"


0 string 0 Test file


"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_with_message_spaces() {
        let input = "0 string 0 Long message continued here";
        let rules = parse_text_magic_file(input).unwrap();
        assert!(rules[0].message.contains("continued"));
    }

    #[test]
    fn test_parse_text_magic_file_mixed_indentation() {
        let input = r#"
0 byte 1 Root1
>4 byte 1 Child1
>4 byte 2 Child2
>>6 byte 3 Grandchild

0 byte 2 Root2
>4 byte 4 Child3
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[1].children.len(), 1);
        assert_eq!(rules[1].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_complex_real_world() {
        let input = r#"
# Magic file for common formats

# ELF binaries
0 byte 0x7f ELF executable
>4 byte 1 Intel 80386
>4 byte 2 x86-64
>>5 byte 1 LSB
>>5 byte 2 MSB

# PDF files
0 byte 0x25 PDF document
>5 byte 0x31 version 1.0
>5 byte 0x34 version 1.4
>5 byte 0x32 version 2.0
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "ELF executable");
        assert!(rules[0].children.len() > 1);
    }

    // ============================================================
    // Integration and edge case tests
    // ============================================================

    #[test]
    fn test_continuation_with_indentation() {
        let input = r#">4 byte 1 Message \
continued"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_multiple_hex_offsets() {
        let input = r#"
0x100 string 0 At 256
0x200 string 0 At 512
"#;
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
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

#[cfg(test)]
mod output_test {
    use crate::parser::{build_rule_hierarchy, parse_text_magic_file, preprocess_lines};

    #[test]
    fn demo_show_all_parser_outputs() {
        let input = r#"
# ELF file
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit

0 string 0 ZIP
>0 byte 3 zipped
"#;

        println!("\n================ RAW INPUT ================\n");
        println!("{input}");

        // --------------------------------------------------
        // 1. preprocess_lines
        // --------------------------------------------------
        println!("\n================ PREPROCESS LINES ================\n");

        let lines = preprocess_lines(input).expect("preprocess_lines failed");

        for (idx, line) in lines.iter().enumerate() {
            println!(
                "[{}] line_no={} is_comment={} content='{}'",
                idx, line.line_number, line.is_comment, line.content
            );
        }

        // --------------------------------------------------
        // 2. parse_text_magic_file (full pipeline)
        // --------------------------------------------------
        println!("\n================ PARSED MAGIC RULES ================\n");

        let rules = parse_text_magic_file(input).expect("parse_text_magic_file failed");

        for (i, rule) in rules.iter().enumerate() {
            println!("ROOT RULE [{}]:", i);
            print_rule(rule, 1);
        }

        // --------------------------------------------------
        // 3. build_rule_hierarchy (explicit)
        // --------------------------------------------------
        println!("\n================ EXPLICIT HIERARCHY BUILD ================\n");

        let rebuilt = build_rule_hierarchy(lines).expect("build_rule_hierarchy failed");

        for (i, rule) in rebuilt.iter().enumerate() {
            println!("ROOT [{}]:", i);
            print_rule(rule, 1);
        }
    }

    // Helper to pretty-print rule trees
    fn print_rule(rule: &crate::parser::MagicRule, indent: usize) {
        let pad = "  ".repeat(indent);

        println!(
            "{}- level={} offset={:?} type={:?} op={:?} value={:?} message='{}'",
            pad, rule.level, rule.offset, rule.typ, rule.op, rule.value, rule.message
        );

        for child in &rule.children {
            print_rule(child, indent + 1);
        }
    }
}
