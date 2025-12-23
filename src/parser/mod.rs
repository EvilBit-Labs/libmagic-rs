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

#[derive(Debug)]
struct LineInfo {
    content: String,
    line_number: usize,
    level: u32,
}

impl LineInfo{
    fn new(content: String, line_number: usize, level:u32) -> Self {
        Self {
            content,
            line_number,
            level
        }
    }
}

fn preprocess_lines(input: &str) -> Result<Vec<LineInfo>, ParseError> {
    let mut lines_info:Vec<LineInfo>  = Vec::new(); 
    let mut line_buf = String::new();
    for (i, mut line) in input.lines().enumerate(){
        let mut level = 0;
        line = line.trim();
        if line.starts_with("#") || line.is_empty(){
            continue;
        }
        if line.starts_with(">"){
            for char in line.chars(){
                if char == '>' {
                    level += 1;
                }
                else {
                    break;
                }
            }
            line = line.trim_start_matches(">");
        }
        line_buf.push_str(line.trim());
        if line.ends_with("\\"){
            line_buf = match line_buf.strip_suffix("\\") {
                Some(line_cont) => line_cont.to_string(),
                None => line_buf
            };
            continue;
        }
        lines_info.push(LineInfo::new(line_buf.to_owned(), i+1, level));
        line_buf.clear();
    }
    Ok(lines_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_preprocess_simple_line() {
        let input = r#"
    # Comment lines start with 
    #offset  type  operator  value  message

    # Example: ELF file detection
    0       string    \x7fELF         ELF
    >4      byte      1               32-bit
    >4      byte      2               64-bit
    >>16    leshort   >0              executable

    # Continuation lines end with backslash\
    0       string    PK\003\004     ZIP archive data, \
            at least v2.0 to extract
            "#;
        let result = preprocess_lines(input);
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_preprocess_basic_rules() {
        let input = r#"
    0 string \x7fELF ELF executable
    >4 byte 1 32-bit
    >>16 leshort >0 executable
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].level, 0);
        assert_eq!(result[1].level, 1);
        assert_eq!(result[2].level, 2);
        assert!(result[0].content.contains("ELF executable"));
        assert!(result[1].content.contains("32-bit"));
    }

    #[test]
    fn test_preprocess_continuation_basic() {
        let input = r#"
    0 string PK\003\004 ZIP archive, \
        at least v2.0 to extract
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 1);
        let content = &result[0].content;
        assert!(content.contains("PK\\003\\004"));
        assert!(content.contains("ZIP archive, at least v2.0 to extract"));
    }

    #[test]
    fn test_preprocess_continuation_with_trailing_whitespace() {
        let input = r#"
    0 string TEST message \
        continued
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].content.contains("message continued"));
    }

    #[test]
    fn test_preprocess_multiple_continuations() {
        let input = r#"
    0 string LONG Long message \
        that spans \
        three lines
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].content.contains("Long message that spans three lines"));
    }

    #[test]
    fn test_preprocess_comments_and_empty_lines() {
        let input = r#"

    # Full-line comment

    # Another comment


    0 string TEST Test rule

    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.trim(), "0 string TEST Test rule");
    }

    #[test]
    fn test_preprocess_leading_whitespace_and_indentation() {
        let input = r#"
        0 string INDENT Indented top-level
        >4 byte 1 Child with indent
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].level, 0);
        assert_eq!(result[1].level, 1);
        assert!(result[0].content.contains("Indented top-level"));
        assert!(result[1].content.contains("Child with indent"));
    }

    #[test]
    fn test_preprocess_mixed_complex_case() {
        let input = r#"
    # Header

    0 string \x4d5a MS-DOS executable

    0 string PK\003\004 ZIP archive, \
        version %d.%d

    >16 lelong >0 compressed data

    # Footer comment
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result[0].content.contains("MS-DOS executable"));
        assert!(result[1].content.contains("ZIP archive"));
        assert!(result[2].content.contains("compressed data"));
        assert_eq!(result[2].level, 1);
    }

    #[test]
    fn test_preprocess_empty_file() {
        let input = "";
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_preprocess_only_comments_and_whitespace() {
        let input = r#"


    # Only comments


        
    # End

    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_preprocess_no_continuation_backslash() {
        let input = r#"
    0 string NORMAL Normal rule
    "#;
        let result = preprocess_lines(input).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].content.contains("\\"));
    }

    #[test]
    fn test_preprocess_with_comment() {
        let input = "# This is a comment\n0 string test Rule";
        let result = preprocess_lines(input);
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_preprocess_empty_line() {
        let input = "0 string test\n\n0 byte 1";
        let result = preprocess_lines(input);
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_preprocess_with_hierarchy() {
        let input = "0 string ELF\n>4 byte 1 32-bit";
        let result = preprocess_lines(input);
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_info_creation() {
        let line = LineInfo::new("test content".to_string(), 1, 0);
        println!("LineInfo: {:#?}", line);
        assert_eq!(line.content, "test content");
        assert_eq!(line.line_number, 1);
        assert_eq!(line.level, 0);
    }
}