// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Low-level value literal parsing tests.
//!
//! Covers `parse_hex_bytes`, `parse_escape_sequence`, `parse_quoted_string`,
//! and `parse_numeric_value` -- the building blocks `parse_value` composes.

use super::*;

// Value parsing tests
#[test]
fn test_parse_hex_bytes_with_backslash_x() {
    // Single hex byte with \x prefix
    assert_eq!(parse_hex_bytes("\\x7f"), Ok(("", vec![0x7f])));
    assert_eq!(parse_hex_bytes("\\x45"), Ok(("", vec![0x45])));
    assert_eq!(parse_hex_bytes("\\x00"), Ok(("", vec![0x00])));
    assert_eq!(parse_hex_bytes("\\xFF"), Ok(("", vec![0xFF])));

    // Multiple hex bytes with \x prefix
    assert_eq!(
        parse_hex_bytes("\\x7f\\x45\\x4c\\x46"),
        Ok(("", vec![0x7f, 0x45, 0x4c, 0x46]))
    );
    assert_eq!(
        parse_hex_bytes("\\x50\\x4b\\x03\\x04"),
        Ok(("", vec![0x50, 0x4b, 0x03, 0x04]))
    );
}

#[test]
fn test_parse_hex_bytes_without_prefix() {
    // Single hex byte without prefix (only works if it contains hex letters)
    assert_eq!(parse_hex_bytes("7f"), Ok(("", vec![0x7f])));
    assert_eq!(
        parse_hex_bytes("45"),
        Err(nom::Err::Error(NomError::new(
            "45",
            nom::error::ErrorKind::Tag
        )))
    ); // No hex letters
    assert_eq!(parse_hex_bytes("ab"), Ok(("", vec![0xab])));
    assert_eq!(parse_hex_bytes("FF"), Ok(("", vec![0xFF])));

    // Multiple hex bytes without prefix
    assert_eq!(
        parse_hex_bytes("7f454c46"),
        Ok(("", vec![0x7f, 0x45, 0x4c, 0x46]))
    );
    assert_eq!(
        parse_hex_bytes("504b0304"),
        Ok(("", vec![0x50, 0x4b, 0x03, 0x04]))
    );
}

#[test]
fn test_parse_hex_bytes_mixed_case() {
    // Test mixed case hex digits
    assert_eq!(parse_hex_bytes("aB"), Ok(("", vec![0xab])));
    assert_eq!(parse_hex_bytes("Cd"), Ok(("", vec![0xcd])));
    assert_eq!(parse_hex_bytes("\\xEf"), Ok(("", vec![0xef])));
    assert_eq!(parse_hex_bytes("\\x1A"), Ok(("", vec![0x1a])));
}

#[test]
fn test_parse_hex_bytes_empty() {
    // Empty input should return error (no hex bytes to parse)
    assert_eq!(
        parse_hex_bytes(""),
        Err(nom::Err::Error(NomError::new(
            "",
            nom::error::ErrorKind::Tag
        )))
    );
}

#[test]
fn test_parse_hex_bytes_with_remaining_input() {
    // Should parse hex bytes and leave remaining input
    assert_eq!(
        parse_hex_bytes("7f45 rest"),
        Ok((" rest", vec![0x7f, 0x45]))
    );
    assert_eq!(
        parse_hex_bytes("\\x50\\x4b next"),
        Ok((" next", vec![0x50, 0x4b]))
    );
    assert_eq!(parse_hex_bytes("ab\""), Ok(("\"", vec![0xab])));
}

#[test]
fn test_parse_escape_sequence() {
    // Standard escape sequences
    assert_eq!(parse_escape_sequence("\\n"), Ok(("", '\n')));
    assert_eq!(parse_escape_sequence("\\r"), Ok(("", '\r')));
    assert_eq!(parse_escape_sequence("\\t"), Ok(("", '\t')));
    assert_eq!(parse_escape_sequence("\\\\"), Ok(("", '\\')));
    assert_eq!(parse_escape_sequence("\\\""), Ok(("", '"')));
    assert_eq!(parse_escape_sequence("\\'"), Ok(("", '\'')));
    assert_eq!(parse_escape_sequence("\\0"), Ok(("", '\0')));
}

#[test]
fn test_parse_escape_sequence_with_remaining() {
    // Should parse escape and leave remaining input
    assert_eq!(parse_escape_sequence("\\n rest"), Ok((" rest", '\n')));
    assert_eq!(parse_escape_sequence("\\t\""), Ok(("\"", '\t')));
}

#[test]
fn test_parse_escape_sequence_invalid() {
    // Should fail on invalid escape sequences
    assert!(parse_escape_sequence("n").is_err()); // Missing backslash
    assert!(parse_escape_sequence("\\").is_err()); // Incomplete escape
    assert!(parse_escape_sequence("").is_err()); // Empty input
}

#[test]
fn test_parse_quoted_string_simple() {
    // Simple quoted strings
    assert_eq!(
        parse_quoted_string("\"hello\""),
        Ok(("", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"world\""),
        Ok(("", "world".to_string()))
    );
    assert_eq!(parse_quoted_string("\"\""), Ok(("", String::new())));
}

#[test]
fn test_parse_quoted_string_with_escapes() {
    // Strings with escape sequences
    assert_eq!(
        parse_quoted_string("\"Hello\\nWorld\""),
        Ok(("", "Hello\nWorld".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Tab\\tSeparated\""),
        Ok(("", "Tab\tSeparated".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Quote: \\\"text\\\"\""),
        Ok(("", "Quote: \"text\"".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Backslash: \\\\\""),
        Ok(("", "Backslash: \\".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Null\\0terminated\""),
        Ok(("", "Null\0terminated".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_with_whitespace() {
    // Strings with leading/trailing whitespace
    assert_eq!(
        parse_quoted_string(" \"hello\" "),
        Ok(("", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\t\"world\"\t"),
        Ok(("", "world".to_string()))
    );
    assert_eq!(
        parse_quoted_string("  \"test\"  "),
        Ok(("", "test".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_with_remaining_input() {
    // Should parse string and leave remaining input
    assert_eq!(
        parse_quoted_string("\"hello\" world"),
        Ok(("world", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"test\" = 123"),
        Ok(("= 123", "test".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_invalid() {
    // Should fail on invalid quoted strings
    assert!(parse_quoted_string("hello").is_err()); // No quotes
    assert!(parse_quoted_string("\"hello").is_err()); // Missing closing quote
    assert!(parse_quoted_string("hello\"").is_err()); // Missing opening quote
    assert!(parse_quoted_string("").is_err()); // Empty input
}

#[test]
fn test_parse_numeric_value_positive() {
    // Positive integers
    assert_eq!(parse_numeric_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_numeric_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_numeric_value("999"), Ok(("", Value::Uint(999))));

    // Positive hex values
    assert_eq!(parse_numeric_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_numeric_value("0x10"), Ok(("", Value::Uint(16))));
    assert_eq!(parse_numeric_value("0xFF"), Ok(("", Value::Uint(255))));
    assert_eq!(parse_numeric_value("0xabc"), Ok(("", Value::Uint(2748))));
}

#[test]
fn test_parse_numeric_value_negative() {
    // Negative integers
    assert_eq!(parse_numeric_value("-1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_numeric_value("-123"), Ok(("", Value::Int(-123))));
    assert_eq!(parse_numeric_value("-999"), Ok(("", Value::Int(-999))));

    // Negative hex values
    assert_eq!(parse_numeric_value("-0x1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_numeric_value("-0x10"), Ok(("", Value::Int(-16))));
    assert_eq!(parse_numeric_value("-0xFF"), Ok(("", Value::Int(-255))));
    assert_eq!(parse_numeric_value("-0xabc"), Ok(("", Value::Int(-2748))));
}

#[test]
fn test_parse_numeric_value_with_whitespace() {
    // With leading/trailing whitespace
    assert_eq!(parse_numeric_value(" 123 "), Ok(("", Value::Uint(123))));
    assert_eq!(parse_numeric_value("\t-456\t"), Ok(("", Value::Int(-456))));
    assert_eq!(parse_numeric_value("  0xFF  "), Ok(("", Value::Uint(255))));
}

#[test]
fn test_parse_numeric_value_with_remaining_input() {
    // Should parse number and leave remaining input (numeric parser consumes trailing whitespace)
    assert_eq!(
        parse_numeric_value("123 rest"),
        Ok(("rest", Value::Uint(123)))
    );
    assert_eq!(
        parse_numeric_value("-456 more"),
        Ok(("more", Value::Int(-456)))
    );
    assert_eq!(parse_numeric_value("0xFF)"), Ok((")", Value::Uint(255))));
}

#[test]
fn test_parse_numeric_value_large_unsigned_quad() {
    // Full u64 range -- values above i64::MAX required for uquad
    let test_cases = [
        // u64::MAX in hex
        ("0xffffffffffffffff", Value::Uint(u64::MAX)),
        // u64::MAX in decimal
        ("18446744073709551615", Value::Uint(u64::MAX)),
        // Exactly i64::MAX + 1 (first value that overflows i64)
        ("0x8000000000000000", Value::Uint(0x8000_0000_0000_0000)),
        // i64::MAX + 1 in decimal
        (
            "9223372036854775808",
            Value::Uint(9_223_372_036_854_775_808),
        ),
        // i64::MAX still works as Uint
        ("0x7fffffffffffffff", Value::Uint(i64::MAX as u64)),
        ("9223372036854775807", Value::Uint(i64::MAX as u64)),
        // Common magic constant patterns
        ("0xDEADBEEFDEADBEEF", Value::Uint(0xDEAD_BEEF_DEAD_BEEF)),
        ("0xCAFEBABECAFEBABE", Value::Uint(0xCAFE_BABE_CAFE_BABE)),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            parse_numeric_value(input),
            Ok(("", expected)),
            "Failed to parse large unsigned quad literal: '{input}'"
        );
    }
}
