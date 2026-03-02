// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

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
//! # Format Detection and Loading
//!
//! The module automatically detects and handles three types of magic file formats:
//! - **Text files**: Human-readable magic rule definitions
//! - **Directories**: Collections of magic files (Magdir pattern)
//! - **Binary files**: Compiled .mgc files (currently unsupported)
//!
//! ## Unified Loading API
//!
//! The recommended entry point for loading magic files is [`load_magic_file()`], which
//! automatically detects the format and dispatches to the appropriate handler:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_file;
//! use std::path::Path;
//!
//! // Works with text files
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic"))?;
//!
//! // Also works with directories
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic.d"))?;
//!
//! // Binary .mgc files return an error with guidance
//! match load_magic_file(Path::new("/usr/share/misc/magic.mgc")) {
//!     Ok(rules) => { /* ... */ },
//!     Err(e) => eprintln!("Use --use-builtin for binary files: {}", e),
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Three-Tier Loading Strategy
//!
//! The loading process works as follows:
//!
//! 1. **Format Detection**: [`detect_format()`] examines the path to determine the file type
//! 2. **Dispatch to Handler**:
//!    - Text files -> [`parse_text_magic_file()`] after reading contents
//!    - Directories -> [`load_magic_directory()`] to load and merge all files
//!    - Binary files -> Returns error suggesting `--use-builtin` option
//! 3. **Return Merged Rules**: All rules are returned in a single `Vec<MagicRule>`
//!
//! # Examples
//!
//! ## Loading Magic Files (Recommended)
//!
//! Use the unified [`load_magic_file()`] API for automatic format detection:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_file;
//! use std::path::Path;
//!
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic"))?;
//! println!("Loaded {} magic rules", rules.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Parsing Text Content Directly
//!
//! For parsing magic rule text that's already in memory:
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
//!
//! ## Loading a Directory Explicitly
//!
//! For Magdir-style directories containing multiple magic files:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_directory;
//! use std::path::Path;
//!
//! // Directory structure:
//! // /usr/share/file/magic.d/
//! //   ├── elf
//! //   ├── archive
//! //   └── text
//!
//! let rules = load_magic_directory(Path::new("/usr/share/file/magic.d"))?;
//! // Rules from all files are merged in alphabetical order by filename
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Migration Note
//!
//! **For users upgrading from direct function calls:**
//!
//! - **Old approach**: Call `detect_format()` then dispatch manually
//! - **New approach**: Use `load_magic_file()` for automatic dispatching
//!
//! The individual functions (`parse_text_magic_file()`, `load_magic_directory()`)
//! remain available for advanced use cases where you need direct control.
//!
//! **Key differences:**
//! - `load_magic_file()`: Unified API with automatic format detection (recommended)
//! - `parse_text_magic_file()`: Parses a single text string containing magic rules
//! - `load_magic_directory()`: Loads and merges all magic files from a directory
//! - `detect_format()`: Low-level format detection (now called internally by `load_magic_file()`)
//!
//! **Error handling in `load_magic_directory()`:**
//! - Critical errors (I/O failures, invalid UTF-8): Returns `ParseError` immediately
//! - Non-critical errors (parse failures in individual files): Logs warning to stderr and continues

pub mod ast;
#[allow(dead_code)]
pub(crate) mod codegen;
mod format;
pub mod grammar;
mod hierarchy;
mod loader;
pub(crate) mod preprocessing;
pub mod types;

// Re-export AST types for convenience
pub use ast::{Endianness, MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

// Re-export parser functions for convenience
pub use grammar::{parse_number, parse_offset};

// Re-export format detection and loading
pub use format::{MagicFileFormat, detect_format};
pub use loader::{load_magic_directory, load_magic_file};

// Internal re-exports for sibling modules and tests
pub(crate) use hierarchy::build_rule_hierarchy;
pub(crate) use preprocessing::preprocess_lines;

use crate::error::ParseError;

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
        let input = r"
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 2);
    }

    #[test]
    fn test_parse_text_magic_file_with_comments() {
        let input = r"
# ELF file format
0 string 0 ELF
>4 byte 1 32-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_multiple_roots() {
        let input = r"
0 byte 1 ELF
>4 byte 1 32-bit

0 byte 2 PDF
>5 byte 1 v1
";
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
        let input = r"
# Comment 1
# Comment 2
# Comment 3
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_text_magic_file_empty_lines_only() {
        let input = r"


0 string 0 Test file


";
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
        let input = r"
0 byte 1 Root1
>4 byte 1 Child1
>4 byte 2 Child2
>>6 byte 3 Grandchild

0 byte 2 Root2
>4 byte 4 Child3
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[1].children.len(), 1);
        assert_eq!(rules[1].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_complex_real_world() {
        let input = r"
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
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "ELF executable");
        assert!(rules[0].children.len() > 1);
    }

    // ============================================================
    // Strength directive integration tests
    // ============================================================

    #[test]
    fn test_parse_text_magic_file_with_strength_directive() {
        let input = r"
!:strength +10
0 string \\x7fELF ELF executable
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Add(10)));
    }

    #[test]
    fn test_parse_text_magic_file_strength_applies_to_next_rule() {
        let input = r"
!:strength *2
0 string \\x7fELF ELF executable
0 string \\x50\\x4b ZIP archive
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        // Strength should only apply to the immediately following rule
        assert_eq!(
            rules[0].strength_modifier,
            Some(StrengthModifier::Multiply(2))
        );
        assert_eq!(rules[1].strength_modifier, None);
    }

    #[test]
    fn test_parse_text_magic_file_strength_with_child_rules() {
        let input = r"
!:strength =50
0 string \\x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        // Strength applies to root rule
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Set(50)));
        // Children should not have strength modifier
        assert_eq!(rules[0].children[0].strength_modifier, None);
        assert_eq!(rules[0].children[1].strength_modifier, None);
    }

    #[test]
    fn test_parse_text_magic_file_multiple_strength_directives() {
        let input = r"
!:strength +10
0 string \\x7fELF ELF executable
!:strength -5
0 string \\x50\\x4b ZIP archive
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Add(10)));
        assert_eq!(
            rules[1].strength_modifier,
            Some(StrengthModifier::Subtract(5))
        );
    }

    #[test]
    fn test_parse_text_magic_file_strength_all_operators() {
        let inputs = [
            ("!:strength +20\n0 byte 1 Test", StrengthModifier::Add(20)),
            (
                "!:strength -15\n0 byte 1 Test",
                StrengthModifier::Subtract(15),
            ),
            (
                "!:strength *3\n0 byte 1 Test",
                StrengthModifier::Multiply(3),
            ),
            ("!:strength /2\n0 byte 1 Test", StrengthModifier::Divide(2)),
            ("!:strength =100\n0 byte 1 Test", StrengthModifier::Set(100)),
            ("!:strength 50\n0 byte 1 Test", StrengthModifier::Set(50)),
        ];

        for (input, expected_modifier) in inputs {
            let rules = parse_text_magic_file(input).unwrap();
            assert_eq!(
                rules[0].strength_modifier,
                Some(expected_modifier),
                "Failed for input: {input}"
            );
        }
    }

    // ============================================================
    // Integration and edge case tests
    // ============================================================

    #[test]
    fn test_continuation_with_indentation() {
        let input = r">4 byte 1 Message \
continued";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_multiple_hex_offsets() {
        let input = r"
0x100 string 0 At 256
0x200 string 0 At 512
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
    }

    // ============================================================
    // Overflow protection tests
    // ============================================================

    #[test]
    fn test_overflow_decimal_too_many_digits() {
        use crate::parser::grammar::parse_number;
        // Test exactly 20 digits (should fail - over i64 max)
        let result = parse_number("12345678901234567890");
        assert!(result.is_err(), "Should reject 20+ decimal digits");
    }

    #[test]
    fn test_overflow_hex_too_many_digits() {
        use crate::parser::grammar::parse_number;
        // Test 17 hex digits (should fail)
        let result = parse_number("0x10000000000000000");
        assert!(result.is_err(), "Should reject 17+ hex digits");
    }

    #[test]
    fn test_overflow_i64_max() {
        use crate::parser::grammar::parse_number;
        // i64::MAX = 9223372036854775807
        let result = parse_number("9223372036854775807");
        assert!(result.is_ok(), "Should accept i64::MAX");
    }

    #[test]
    fn test_overflow_i64_max_plus_one() {
        use crate::parser::grammar::parse_number;
        // i64::MAX + 1 should fail
        let result = parse_number("9223372036854775808");
        assert!(result.is_err(), "Should reject i64::MAX + 1");
    }

    // ============================================================
    // Line number accuracy test (uses parse_text_magic_file)
    // ============================================================

    #[test]
    fn test_error_reports_correct_line_for_continuation() {
        // When a continued rule fails to parse, error should show the starting line
        let input = "0 string 0 valid\n0 invalid \\\nsyntax here\n0 string 0 valid2";
        let result = parse_text_magic_file(input);

        match result {
            Err(ref e) => {
                // Error should mention line 2 (start of the bad rule), not line 3
                let error_str = format!("{e:?}");
                assert!(
                    error_str.contains("line 2") || error_str.contains("line: 2"),
                    "Error should reference line 2, got: {error_str}"
                );
            }
            Ok(_) => panic!("Expected InvalidSyntax error"),
        }
    }
}

#[cfg(test)]
mod output_test {
    use crate::parser::{build_rule_hierarchy, parse_text_magic_file, preprocess_lines};

    #[test]
    fn demo_show_all_parser_outputs() {
        let input = r"
# ELF file
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit

0 string 0 ZIP
>0 byte 3 zipped
";

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
            println!("ROOT RULE [{i}]:");
            print_rule(rule, 1);
        }

        // --------------------------------------------------
        // 3. build_rule_hierarchy (explicit)
        // --------------------------------------------------
        println!("\n================ EXPLICIT HIERARCHY BUILD ================\n");

        let rebuilt = build_rule_hierarchy(lines).expect("build_rule_hierarchy failed");

        for (i, rule) in rebuilt.iter().enumerate() {
            println!("ROOT [{i}]:");
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
