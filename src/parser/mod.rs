//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.

pub mod ast;
pub mod grammar;

// Re-export AST types for convenience
pub use ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};

// Re-export parser functions for convenience
pub use grammar::{parse_number, parse_offset};

use crate::error::ParseError;

/// Information about a preprocessed line from a magic file
#[derive(Debug, Clone)]
struct LineInfo {
    /// The content of the line after preprocessing
    content: String,
    /// The original line number (1-indexed)
    line_number: usize,
    /// The hierarchy level (0 = top-level, 1 = child, etc.)
    level: u32,
}

/// Preprocess lines from a magic file
///
/// This function handles:
/// - Line continuation (lines ending with backslash)
/// - Comment stripping (everything after # is removed)
/// - Empty line filtering
/// - Hierarchy level detection (counting leading > characters)
/// - Line number tracking for error reporting
///
/// # Arguments
///
/// * `input` - The raw magic file content
///
/// # Returns
///
/// A vector of `LineInfo` structures representing preprocessed lines
///
/// # Errors
///
/// Returns `ParseError` if line continuation is malformed
fn preprocess_lines(input: &str) -> Result<Vec<LineInfo>, ParseError> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut continuation_start_line = 0;

    for (line_idx, raw_line) in input.lines().enumerate() {
        let line_number = line_idx + 1; // 1-indexed line numbers

        // Strip comments - everything after # (but not within escaped strings)
        let line_without_comment = if let Some(hash_pos) = raw_line.find('#') {
            &raw_line[..hash_pos]
        } else {
            raw_line
        };

        // Check for line continuation
        let has_continuation = grammar::has_continuation(line_without_comment);
        let line_content = if has_continuation {
            // Remove the trailing backslash and whitespace
            line_without_comment.trim_end().trim_end_matches('\\')
        } else {
            line_without_comment
        };

        // Append to current line (handling continuation)
        if current_line.is_empty() {
            current_line = line_content.to_string();
            continuation_start_line = line_number;
        } else {
            // Continuing from previous line - add a space separator
            current_line.push(' ');
            current_line.push_str(line_content.trim());
        }

        // If not continuing, process the accumulated line
        if !has_continuation {
            let trimmed = current_line.trim();
            
            // Skip empty lines and comment-only lines
            if !trimmed.is_empty() && !grammar::is_comment_line(trimmed) {
                // Don't strip the > characters - the grammar parser needs them!
                lines.push(LineInfo {
                    content: trimmed.to_string(),
                    line_number: continuation_start_line,
                    level: 0, // Level will be set by parse_magic_rule
                });
            }

            // Reset for next line
            current_line.clear();
            continuation_start_line = 0;
        }
    }

    // Handle case where file ends with continuation
    if !current_line.is_empty() {
        let trimmed = current_line.trim();
        if !trimmed.is_empty() && !grammar::is_comment_line(trimmed) {
            lines.push(LineInfo {
                content: trimmed.to_string(),
                line_number: continuation_start_line,
                level: 0, // Level will be set by parse_magic_rule
            });
        }
    }

    Ok(lines)
}

/// Parse a single magic rule line
///
/// This function takes a preprocessed line and parses it into a `MagicRule`.
/// The level information is already extracted during preprocessing.
///
/// # Arguments
///
/// * `line_info` - Information about the line to parse
///
/// # Returns
///
/// A `MagicRule` structure representing the parsed rule
///
/// # Errors
///
/// Returns `ParseError` if the line cannot be parsed as a valid magic rule
fn parse_magic_rule_line(line_info: &LineInfo) -> Result<MagicRule, ParseError> {
    // Use the grammar parser to parse the rule
    match grammar::parse_magic_rule(&line_info.content) {
        Ok((remaining, rule)) => {
            // Ensure all input was consumed
            if !remaining.trim().is_empty() {
                return Err(ParseError::invalid_syntax(
                    line_info.line_number,
                    format!("Unexpected content after rule: '{}'", remaining),
                ));
            }

            // The grammar parser already set the level correctly from > prefix
            Ok(rule)
        }
        Err(e) => Err(ParseError::invalid_syntax(
            line_info.line_number,
            format!("Failed to parse rule: {e:?}"),
        )),
    }
}

/// Build hierarchical rule structure from flat list of rules
///
/// This function takes a flat list of rules with level information and
/// constructs the parent-child hierarchy by:
/// - Maintaining a stack of parent indices at each level
/// - Attaching child rules to the appropriate parent
/// - Validating level transitions (no jumps > 1)
///
/// # Arguments
///
/// * `rules` - Flat list of rules with level information
///
/// # Returns
///
/// A vector of top-level `MagicRule` structures with nested children
///
/// # Errors
///
/// Returns `ParseError` if:
/// - A child rule has no parent (orphaned child)
/// - Level increases by more than 1 (invalid jump)
fn build_rule_hierarchy(rules: Vec<(MagicRule, usize)>) -> Result<Vec<MagicRule>, ParseError> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let mut top_level_rules = Vec::new();
    // Stack that tracks the path to the current position in the tree
    // Each entry is (rule_index_in_parent, is_top_level)
    let mut parent_path: Vec<(usize, bool)> = Vec::new();

    for (rule, line_number) in rules {
        let level = rule.level as usize;

        // Validate level transitions
        if level > 0 && level > parent_path.len() {
            return Err(ParseError::invalid_syntax(
                line_number,
                format!(
                    "Invalid level jump: jumped from level {} to level {} (can only increase by 1)",
                    parent_path.len(),
                    level
                ),
            ));
        }

        if level == 0 {
            // Top-level rule
            let rule_index = top_level_rules.len();
            top_level_rules.push(rule);
            parent_path.clear();
            parent_path.push((rule_index, true));
        } else {
            // Child rule - need a parent at level-1
            if parent_path.len() < level {
                return Err(ParseError::invalid_syntax(
                    line_number,
                    format!("Orphaned child rule at level {level} (no parent at level {})", level.saturating_sub(1)),
                ));
            }

            // Truncate parent path to level-1 to get the parent
            parent_path.truncate(level);

            // Navigate to the parent using the path
            let parent = navigate_to_parent(&mut top_level_rules, &parent_path)?;
            
            // Add child to parent
            let child_index = parent.children.len();
            parent.children.push(rule);
            
            // Update path to include this new child
            parent_path.push((child_index, false));
        }
    }

    Ok(top_level_rules)
}

/// Navigate to a parent rule using the parent path
///
/// # Errors
///
/// Returns `ParseError` if the path is invalid
fn navigate_to_parent<'a>(
    top_level_rules: &'a mut [MagicRule],
    parent_path: &[(usize, bool)],
) -> Result<&'a mut MagicRule, ParseError> {
    if parent_path.is_empty() {
        return Err(ParseError::invalid_syntax(0, "Empty parent path"));
    }

    // Start with the top-level rule
    let (top_index, is_top) = parent_path[0];
    if !is_top || top_index >= top_level_rules.len() {
        return Err(ParseError::invalid_syntax(0, "Invalid parent path: bad top-level index"));
    }

    let mut current = &mut top_level_rules[top_index];

    // Navigate through children
    for &(child_index, _) in &parent_path[1..] {
        if child_index >= current.children.len() {
            return Err(ParseError::invalid_syntax(0, "Invalid parent path: bad child index"));
        }
        current = &mut current.children[child_index];
    }

    Ok(current)
}

/// Parse a complete text-based magic file
///
/// This function parses an entire magic file and returns a hierarchical
/// tree of `MagicRule` structures. It handles:
/// - Line continuation (backslash at end of lines)
/// - Comments (lines starting with #)
/// - Empty lines (ignored)
/// - Hierarchical rules (using > prefix)
/// - Error reporting with line numbers
///
/// # Arguments
///
/// * `input` - String content of the magic file
///
/// # Returns
///
/// A vector of top-level `MagicRule` structures with nested children
///
/// # Errors
///
/// Returns `ParseError` with line number and description for:
/// - Invalid syntax
/// - Unrecognized types or operators
/// - Malformed offset specifications
/// - Orphaned child rules (> without parent)
/// - Invalid level jumps (e.g., >> without >)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::parse_text_magic_file;
///
/// let magic_content = r#"
/// # ELF executables
/// 0    string    \x7fELF    ELF
/// >4   byte      1          32-bit
/// >4   byte      2          64-bit
/// "#;
///
/// let rules = parse_text_magic_file(magic_content).unwrap();
/// assert_eq!(rules.len(), 1);
/// assert_eq!(rules[0].children.len(), 2);
/// ```
pub fn parse_text_magic_file(input: &str) -> Result<Vec<MagicRule>, ParseError> {
    // Phase 1: Preprocess lines
    let line_infos = preprocess_lines(input)?;

    // Phase 2: Parse each line into a rule
    let mut rules_with_line_numbers = Vec::new();
    for line_info in line_infos {
        let rule = parse_magic_rule_line(&line_info)?;
        rules_with_line_numbers.push((rule, line_info.line_number));
    }

    // Phase 3: Build hierarchy
    build_rule_hierarchy(rules_with_line_numbers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_lines_basic() {
        let input = "0 string test Test file";
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string test Test file");
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].level, 0);
    }

    #[test]
    fn test_preprocess_lines_with_comments() {
        let input = r#"
# This is a comment
0 string test Test file
# Another comment
"#;
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string test Test file");
        assert_eq!(lines[0].line_number, 3);
    }

    #[test]
    fn test_preprocess_lines_with_empty_lines() {
        let input = r#"
0 string test Test file

>4 byte 1 Child rule
"#;
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].level, 0);
        assert_eq!(lines[1].level, 1);
    }

    #[test]
    fn test_preprocess_lines_with_continuation() {
        let input = "0 string test Long message \\\n        continued here";
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].content.contains("Long message"));
        assert!(lines[0].content.contains("continued here"));
    }

    #[test]
    fn test_preprocess_lines_hierarchy() {
        let input = r"
0 string test Parent
>4 byte 1 Child
>>8 byte 2 Grandchild
";
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 3);
        // The > characters are NOT stripped - they're part of the content
        assert!(lines[0].content.starts_with('0'));
        assert!(lines[1].content.starts_with('>'));
        assert!(lines[2].content.starts_with(">>"));
    }

    #[test]
    fn test_preprocess_lines_inline_comments() {
        let input = "0 string test Test file # This is a comment";
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].content.contains("Test file"));
        assert!(!lines[0].content.contains("# This is a comment"));
    }

    #[test]
    fn test_parse_simple_rule() {
        let input = "0 string PK\\x03\\x04 ZIP archive";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "ZIP archive");
        assert_eq!(rules[0].level, 0);
        assert!(rules[0].children.is_empty());
    }

    #[test]
    fn test_parse_hierarchical_rules() {
        let input = r"
0       string    \x7fELF         ELF
>4      byte      1               32-bit
>4      byte      2               64-bit
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "ELF");
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[0].message, "32-bit");
        assert_eq!(rules[0].children[1].message, "64-bit");
        assert_eq!(rules[0].children[0].level, 1);
        assert_eq!(rules[0].children[1].level, 1);
    }

    #[test]
    fn test_parse_nested_hierarchy() {
        let input = r"
0       string    \x7fELF         ELF
>4      byte      1               32-bit
>>16    leshort   1               executable
>4      byte      2               64-bit
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[0].children.len(), 1);
        assert_eq!(rules[0].children[0].children[0].message, "executable");
        assert_eq!(rules[0].children[0].children[0].level, 2);
    }

    #[test]
    fn test_parse_comments_and_empty_lines() {
        let input = r"
# This is a comment

0       string    \x7f    Test file
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "Test file");
    }

    #[test]
    fn test_parse_continuation_lines() {
        let input = "0 string \\x7f Long message \\\ncontinued here";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert!(rules[0].message.contains("Long message"));
        assert!(rules[0].message.contains("continued here"));
    }

    #[test]
    fn test_error_orphaned_child() {
        let input = ">4 byte 1 orphaned";
        let result = parse_text_magic_file(input);
        
        assert!(result.is_err());
        let error_message = match result {
            Err(ParseError::InvalidSyntax { message, .. }) => message,
            _ => panic!("Expected InvalidSyntax error for orphaned child"),
        };
        assert!(error_message.contains("Orphaned") || error_message.contains("Invalid"));
    }

    #[test]
    fn test_error_invalid_level_jump() {
        let input = r"
0       string    \x7f    Parent
>>>4    byte      1       Invalid jump
        ";
        let result = parse_text_magic_file(input);
        
        assert!(result.is_err());
        if let Err(ParseError::InvalidSyntax { message, .. }) = result {
            assert!(message.contains("Invalid level jump") || message.contains("Orphaned"));
        } else {
            panic!("Expected InvalidSyntax error for invalid level jump");
        }
    }

    #[test]
    fn test_parse_multiple_top_level_rules() {
        let input = "0       string    \\x7fELF         ELF\n\
                     >4      byte      1               32-bit\n\
                     \n\
                     0       string    PK\\x03\\x04      ZIP archive\n\
                     >4      leshort   0x0014          version 2.0";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "ELF");
        assert_eq!(rules[1].message, "ZIP archive");
        assert_eq!(rules[0].children.len(), 1);
        assert_eq!(rules[1].children.len(), 1);
    }

    #[test]
    fn test_parse_real_world_magic_file() {
        let input = "# ELF executables\n\
                     0\tstring\t\\x7fELF\tELF\n\
                     >4\tbyte\t1\t32-bit\n\
                     >4\tbyte\t2\t64-bit\n\
                     >5\tbyte\t1\tLSB\n\
                     >5\tbyte\t2\tMSB\n\
                     \n\
                     # ZIP archives\n\
                     0\tstring\tPK\\x03\\x04\tZIP archive\n\
                     0\tstring\tPK\\x05\\x06\tZIP archive (empty)\n\
                     \n\
                     # JPEG images\n\
                     0\tstring\t\\xff\\xd8\\xff\tJPEG image data";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 4); // ELF, 2 ZIP variants, JPEG
        assert_eq!(rules[0].message, "ELF");
        assert_eq!(rules[0].children.len(), 4); // 32-bit, 64-bit, LSB, MSB
    }

    #[test]
    fn test_parse_with_no_message() {
        let input = "0 byte 0x7f";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "");
    }

    #[test]
    fn test_parse_with_hex_offset() {
        let input = "0x10 lelong 0x12345678 Test data";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].offset, OffsetSpec::Absolute(16));
        assert_eq!(rules[0].message, "Test data");
    }

    #[test]
    fn test_parse_with_operators() {
        let input = r"
0 lelong&0xf0000000 0x10000000 MIPS-II
>0 lelong != 0 Non-zero
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "MIPS-II");
        assert!(matches!(rules[0].op, Operator::BitwiseAndMask(_)));
        assert_eq!(rules[0].children.len(), 1);
        assert_eq!(rules[0].children[0].op, Operator::NotEqual);
    }

    #[test]
    fn test_parse_empty_file() {
        let input = "";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_only_comments() {
        let input = r"
# Comment line 1
# Comment line 2
# Comment line 3
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_mixed_levels_same_parent() {
        let input = r"
0 string \x7f Parent
>4 byte 1 Child 1
>8 byte 2 Child 2
>12 byte 3 Child 3
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 3);
        assert_eq!(rules[0].children[0].message, "Child 1");
        assert_eq!(rules[0].children[1].message, "Child 2");
        assert_eq!(rules[0].children[2].message, "Child 3");
    }

    #[test]
    fn test_parse_complex_hierarchy() {
        let input = r"
0 string \x41 Top 1
>4 byte 1 Level 1-1
>>8 byte 2 Level 2-1
>>>12 byte 3 Level 3-1
>>16 byte 4 Level 2-2
>20 byte 5 Level 1-2
0 string \x42 Top 2
        ";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "Top 1");
        assert_eq!(rules[1].message, "Top 2");
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[0].children.len(), 2);
        assert_eq!(rules[0].children[0].children[0].children.len(), 1);
    }

    #[test]
    fn test_error_invalid_syntax() {
        let input = "invalid syntax here";
        let result = parse_text_magic_file(input);
        
        assert!(result.is_err());
        assert!(matches!(result, Err(ParseError::InvalidSyntax { .. })));
    }

    #[test]
    fn test_continuation_across_multiple_lines() {
        let input = "0 string \\x7f First line \\\nsecond line \\\nthird line";
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert!(rules[0].message.contains("First line"));
        assert!(rules[0].message.contains("second line"));
        assert!(rules[0].message.contains("third line"));
    }

    #[test]
    fn test_parse_string_with_escapes() {
        let input = r#"0 string "Hello\nWorld" Text with newline"#;
        let rules = parse_text_magic_file(input).unwrap();
        
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "Text with newline");
    }

    #[test]
    fn test_preprocess_preserves_line_numbers() {
        let input = r"
# Line 1 comment

0 string \x7f Test 1
# Line 4 comment
>4 byte 1 Child
        ";
        let lines = preprocess_lines(input).unwrap();
        
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, 4); // "0 string \x7f Test 1" is on line 4 (after empty line 1, comment line 2, empty line 3)
        assert_eq!(lines[1].line_number, 6); // ">4 byte 1 Child" is on line 6
    }
}
