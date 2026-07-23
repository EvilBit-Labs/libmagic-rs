// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `!:strength` directive and line-classification helper tests.
//!
//! Covers `parse_strength_directive`/`is_strength_directive` and the
//! small line-classification helpers (`parse_comment`, `is_empty_line`,
//! `is_comment_line`, `has_continuation`).

use super::*;

#[test]
fn test_parse_comment() {
    assert_eq!(
        parse_comment("# This is a comment"),
        Ok(("", "This is a comment".to_string()))
    );
    assert_eq!(parse_comment("#"), Ok(("", String::new())));
    assert_eq!(
        parse_comment("# ELF executables"),
        Ok(("", "ELF executables".to_string()))
    );
}

#[test]
fn test_parse_comment_with_whitespace() {
    assert_eq!(
        parse_comment("  # Indented comment  "),
        Ok(("", "Indented comment".to_string()))
    );
    assert_eq!(
        parse_comment("\t#\tTabbed comment\t"),
        Ok(("", "Tabbed comment".to_string()))
    );
}

#[test]
fn test_is_empty_line() {
    assert!(is_empty_line(""));
    assert!(is_empty_line("   "));
    assert!(is_empty_line("\t\t"));
    assert!(is_empty_line(" \t \t "));
    assert!(!is_empty_line("0 byte 1"));
    assert!(!is_empty_line("  # comment"));
}

#[test]
fn test_is_comment_line() {
    assert!(is_comment_line("# This is a comment"));
    assert!(is_comment_line("#"));
    assert!(is_comment_line("  # Indented comment"));
    assert!(is_comment_line("\t# Tabbed comment"));
    assert!(!is_comment_line("0 byte 1"));
    assert!(!is_comment_line("string test"));
}

#[test]
fn test_has_continuation() {
    assert!(has_continuation("0 string test \\"));
    assert!(has_continuation("message continues \\"));
    assert!(has_continuation("line ends with backslash\\"));
    assert!(has_continuation("  trailing whitespace  \\  "));
    assert!(!has_continuation("0 string test"));
    assert!(!has_continuation("no continuation"));
    assert!(!has_continuation("backslash in middle \\ here"));
}

// Strength directive tests
#[test]
fn test_parse_strength_directive_add() {
    assert_eq!(
        parse_strength_directive("!:strength +10"),
        Ok(("", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("!:strength +0"),
        Ok(("", StrengthModifier::Add(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength +100"),
        Ok(("", StrengthModifier::Add(100)))
    );
}

#[test]
fn test_parse_strength_directive_subtract() {
    assert_eq!(
        parse_strength_directive("!:strength -5"),
        Ok(("", StrengthModifier::Subtract(5)))
    );
    assert_eq!(
        parse_strength_directive("!:strength -0"),
        Ok(("", StrengthModifier::Subtract(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength -50"),
        Ok(("", StrengthModifier::Subtract(50)))
    );
}

#[test]
fn test_parse_strength_directive_multiply() {
    assert_eq!(
        parse_strength_directive("!:strength *2"),
        Ok(("", StrengthModifier::Multiply(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength *10"),
        Ok(("", StrengthModifier::Multiply(10)))
    );
}

#[test]
fn test_parse_strength_directive_divide() {
    assert_eq!(
        parse_strength_directive("!:strength /2"),
        Ok(("", StrengthModifier::Divide(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength /10"),
        Ok(("", StrengthModifier::Divide(10)))
    );
}

#[test]
fn test_parse_strength_directive_set_explicit() {
    assert_eq!(
        parse_strength_directive("!:strength =50"),
        Ok(("", StrengthModifier::Set(50)))
    );
    assert_eq!(
        parse_strength_directive("!:strength =0"),
        Ok(("", StrengthModifier::Set(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength =100"),
        Ok(("", StrengthModifier::Set(100)))
    );
}

#[test]
fn test_parse_strength_directive_set_bare() {
    // Bare number implies Set
    assert_eq!(
        parse_strength_directive("!:strength 50"),
        Ok(("", StrengthModifier::Set(50)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 0"),
        Ok(("", StrengthModifier::Set(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 100"),
        Ok(("", StrengthModifier::Set(100)))
    );
}

#[test]
fn test_parse_strength_directive_with_whitespace() {
    assert_eq!(
        parse_strength_directive("  !:strength +10"),
        Ok(("", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("\t!:strength -5"),
        Ok(("", StrengthModifier::Subtract(5)))
    );
    assert_eq!(
        parse_strength_directive("!:strength  *2"),
        Ok(("", StrengthModifier::Multiply(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength   50"),
        Ok(("", StrengthModifier::Set(50)))
    );
}

#[test]
fn test_parse_strength_directive_space_after_operator() {
    // GNU `file` magic(5) parsers accept whitespace between the operator
    // and the operand. Real-world example: the Minix filesystem entries
    // in /usr/share/file/magic/filesystems use `!:strength / 2`.
    assert_eq!(
        parse_strength_directive("!:strength + 10"),
        Ok(("", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("!:strength - 5"),
        Ok(("", StrengthModifier::Subtract(5)))
    );
    assert_eq!(
        parse_strength_directive("!:strength * 2"),
        Ok(("", StrengthModifier::Multiply(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength / 2"),
        Ok(("", StrengthModifier::Divide(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength = 100"),
        Ok(("", StrengthModifier::Set(100)))
    );
    // Tabs between operator and number are also permitted.
    assert_eq!(
        parse_strength_directive("!:strength /\t2"),
        Ok(("", StrengthModifier::Divide(2)))
    );
}

#[test]
fn test_parse_strength_directive_with_remaining_input() {
    // Should leave remaining content after the directive
    assert_eq!(
        parse_strength_directive("!:strength +10 extra"),
        Ok((" extra", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 50\n"),
        Ok(("\n", StrengthModifier::Set(50)))
    );
}

#[test]
fn test_parse_strength_directive_invalid() {
    // Should fail on invalid input
    assert!(parse_strength_directive("").is_err());
    assert!(parse_strength_directive("!:invalid").is_err());
    assert!(parse_strength_directive("strength +10").is_err());
    assert!(parse_strength_directive("0 byte 1").is_err());
}

#[test]
fn test_is_strength_directive() {
    assert!(is_strength_directive("!:strength +10"));
    assert!(is_strength_directive("!:strength -5"));
    assert!(is_strength_directive("!:strength 50"));
    assert!(is_strength_directive("  !:strength +10"));
    assert!(is_strength_directive("\t!:strength *2"));

    assert!(!is_strength_directive("0 byte 1"));
    assert!(!is_strength_directive("# comment"));
    assert!(!is_strength_directive(""));
    assert!(!is_strength_directive("!:mime application/pdf"));
}
