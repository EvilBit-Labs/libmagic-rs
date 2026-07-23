// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `parse_value` and string-family bareword value parsing tests.
//!
//! Covers `parse_value`'s combined literal handling plus the
//! string-family bareword contract (GOTCHAS S3.6): barewords never
//! parse as numbers for string-family types, and high-byte barewords
//! resolve to `Value::Bytes` rather than a lossy `Value::String`.

use super::*;

#[test]
fn test_parse_value_string_literals() {
    // String value parsing
    assert_eq!(
        parse_value("\"hello\""),
        Ok(("", Value::String("hello".to_string())))
    );
    assert_eq!(
        parse_value("\"ELF\""),
        Ok(("", Value::String("ELF".to_string())))
    );
    assert_eq!(parse_value("\"\""), Ok(("", Value::String(String::new()))));

    // String with escape sequences
    assert_eq!(
        parse_value("\"Line1\\nLine2\""),
        Ok(("", Value::String("Line1\nLine2".to_string())))
    );
    assert_eq!(
        parse_value("\"Tab\\tSeparated\""),
        Ok(("", Value::String("Tab\tSeparated".to_string())))
    );
    assert_eq!(
        parse_value("\"Null\\0Term\""),
        Ok(("", Value::String("Null\0Term".to_string())))
    );
}

#[test]
fn test_parse_value_numeric_literals() {
    // Positive integers
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("999"), Ok(("", Value::Uint(999))));

    // Negative integers
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_value("-123"), Ok(("", Value::Int(-123))));
    assert_eq!(parse_value("-999"), Ok(("", Value::Int(-999))));

    // Hexadecimal values
    assert_eq!(parse_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("0x10"), Ok(("", Value::Uint(16))));
    assert_eq!(parse_value("0xFF"), Ok(("", Value::Uint(255))));
    assert_eq!(parse_value("-0xFF"), Ok(("", Value::Int(-255))));
}

#[test]
fn test_parse_value_hex_byte_sequences() {
    // Hex bytes with \x prefix
    assert_eq!(parse_value("\\x7f"), Ok(("", Value::Bytes(vec![0x7f]))));
    assert_eq!(
        parse_value("\\x7f\\x45\\x4c\\x46"),
        Ok(("", Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])))
    );

    // Hex bytes without prefix
    assert_eq!(parse_value("7f"), Ok(("", Value::Bytes(vec![0x7f]))));
    assert_eq!(
        parse_value("7f454c46"),
        Ok(("", Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])))
    );

    // Mixed case
    assert_eq!(parse_value("aB"), Ok(("", Value::Bytes(vec![0xab]))));
    assert_eq!(parse_value("\\xCd"), Ok(("", Value::Bytes(vec![0xcd]))));
}

#[test]
fn test_parse_value_with_whitespace() {
    // All value types with whitespace - trailing whitespace is consumed by individual parsers
    assert_eq!(
        parse_value(" \"hello\" "),
        Ok(("", Value::String("hello".to_string())))
    );
    assert_eq!(parse_value("  123  "), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("\t-456\t"), Ok(("", Value::Int(-456))));
    // Floats consume trailing whitespace (consistent with integers)
    assert_eq!(parse_value("  3.125  "), Ok(("", Value::Float(3.125))));
    assert_eq!(parse_value("\t-1.0\t"), Ok(("", Value::Float(-1.0))));
    // Hex bytes don't consume trailing whitespace by themselves
    assert_eq!(
        parse_value("  \\x7f\\x45  "),
        Ok(("  ", Value::Bytes(vec![0x7f, 0x45])))
    );
}

#[test]
fn test_parse_value_with_remaining_input() {
    // Should parse value and leave remaining input
    // Note: Individual parsers handle whitespace differently
    assert_eq!(
        parse_value("\"hello\" world"),
        Ok(("world", Value::String("hello".to_string())))
    );
    assert_eq!(
        parse_value("123 rest"),
        Ok(("rest", Value::Uint(123))) // Numeric parser consumes trailing space
    );
    assert_eq!(
        parse_value("-456 more"),
        Ok(("more", Value::Int(-456))) // Numeric parser consumes trailing space
    );
    assert_eq!(
        parse_value("\\x7f\\x45 next"),
        Ok((" next", Value::Bytes(vec![0x7f, 0x45]))) // Hex bytes don't consume trailing space
    );
}

#[test]
fn test_parse_value_edge_cases() {
    // Zero values in different formats
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-0"), Ok(("", Value::Int(0))));
    assert_eq!(parse_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-0x0"), Ok(("", Value::Int(0))));

    // Large values
    assert_eq!(
        parse_value("2147483647"),
        Ok(("", Value::Uint(2_147_483_647)))
    );
    assert_eq!(
        parse_value("-2147483648"),
        Ok(("", Value::Int(-2_147_483_648)))
    );
    assert_eq!(
        parse_value("0x7FFFFFFF"),
        Ok(("", Value::Uint(2_147_483_647)))
    );

    // Empty input should fail
    assert!(parse_value("").is_err());
}

#[test]
fn test_parse_value_invalid_input() {
    // Should fail on completely invalid input
    assert!(parse_value("xyz").is_err()); // Not a valid value format
    assert!(parse_value("0xGG").is_err()); // Invalid hex digits
    assert!(parse_value("\"unclosed").is_err()); // Unclosed string
    assert!(parse_value("--123").is_err()); // Invalid number format
}

#[test]
fn test_parse_value_common_magic_file_patterns() {
    // Test patterns commonly found in magic files
    assert_eq!(
        parse_value("0x7f454c46"),
        Ok(("", Value::Uint(0x7f45_4c46)))
    );
    assert_eq!(
        parse_value("\"ELF\""),
        Ok(("", Value::String("ELF".to_string())))
    );
    assert_eq!(
        parse_value("\\x50\\x4b\\x03\\x04"),
        Ok(("", Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04])))
    );
    assert_eq!(
        parse_value("\"\\377ELF\""),
        Ok(("", Value::String("\u{00ff}ELF".to_string())))
    );
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));
}

#[test]
fn test_parse_value_type_precedence() {
    // Test that parsing precedence works correctly
    // Quoted strings should be parsed as strings, not hex bytes
    assert_eq!(
        parse_value("\"7f\""),
        Ok(("", Value::String("7f".to_string())))
    );

    // Hex patterns should be parsed as bytes when not quoted
    assert_eq!(parse_value("7f"), Ok(("", Value::Bytes(vec![0x7f]))));

    // Numbers should be parsed as numbers when they don't look like hex bytes
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("-123"), Ok(("", Value::Int(-123))));

    // Hex numbers with 0x prefix should be parsed as numbers
    assert_eq!(parse_value("0x123"), Ok(("", Value::Uint(0x123))));
}

#[test]
fn test_parse_value_float_literals() {
    // Positive floats
    assert_eq!(parse_value("3.125"), Ok(("", Value::Float(3.125))));
    assert_eq!(parse_value("0.5"), Ok(("", Value::Float(0.5))));
    assert_eq!(parse_value("100.0"), Ok(("", Value::Float(100.0))));

    // Negative floats
    assert_eq!(parse_value("-1.0"), Ok(("", Value::Float(-1.0))));
    assert_eq!(parse_value("-0.001"), Ok(("", Value::Float(-0.001))));

    // Scientific notation
    assert_eq!(parse_value("2.5e10"), Ok(("", Value::Float(2.5e10))));
    assert_eq!(parse_value("1.0E-3"), Ok(("", Value::Float(1.0e-3))));
    assert_eq!(parse_value("-3.0e+2"), Ok(("", Value::Float(-3.0e+2))));

    // Integers should NOT be parsed as floats
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("-456"), Ok(("", Value::Int(-456))));
    assert_eq!(parse_value("0x1a"), Ok(("", Value::Uint(26))));

    // Float with remaining input (trailing whitespace consumed, like integers)
    assert_eq!(parse_value("1.5 rest"), Ok(("rest", Value::Float(1.5))));

    // Non-finite floats (overflow to infinity) should never produce Value::Float
    // parse_value falls through to other parsers, so we check the result type
    if let Ok((_, value)) = parse_value("1.0e309") {
        assert!(
            !matches!(value, Value::Float(f) if !f.is_finite()),
            "overflow should not produce non-finite Value::Float"
        );
    }
}

#[test]
fn test_parse_value_boundary_conditions() {
    // Test boundary conditions for different value types

    // Single character strings
    assert_eq!(
        parse_value("\"a\""),
        Ok(("", Value::String("a".to_string())))
    );
    assert_eq!(
        parse_value("\"1\""),
        Ok(("", Value::String("1".to_string())))
    );

    // Single hex byte
    assert_eq!(parse_value("ab"), Ok(("", Value::Bytes(vec![0xab]))));
    assert_eq!(parse_value("\\x00"), Ok(("", Value::Bytes(vec![0x00]))));

    // Minimum and maximum values
    assert_eq!(parse_value("1"), Ok(("", Value::Uint(1))));
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));

    // Powers of 2 (common in binary formats)
    assert_eq!(parse_value("256"), Ok(("", Value::Uint(256))));
    assert_eq!(parse_value("0x100"), Ok(("", Value::Uint(256))));
    assert_eq!(parse_value("1024"), Ok(("", Value::Uint(1024))));
    assert_eq!(parse_value("0x400"), Ok(("", Value::Uint(1024))));
}

/// A bareword value on a string-family type must parse as `Value::String`,
/// not `Value::Uint`/`Value::Float` -- for BOTH equality and ordering
/// operators, since `parse_string_family_value` never inspects the
/// operator. Previously `parse_value`'s numeric branches captured
/// `0`/`0.6.1`/`20011231` as numbers, so the subsequent `String`-vs-number
/// comparison never matched and real `>0`/`000` idioms (`\b, name %s`,
/// `face %s`, version compares) silently failed.
///
/// The `search` case deliberately uses an EQUALITY operator: `search` is a
/// pattern-bearing type, and a non-equality operator on it is an intentional
/// `UnsupportedType` fatal gap (GOTCHAS S2.4), so `search >100` -- while it
/// now correctly PARSES to `Value::String("100")` -- is not a construct that
/// evaluates. No real magic rule uses `search`/`regex` with an ordering
/// operator on a numeric bareword; the only real hit (`search/1 >\0`) is a
/// `\0`->`Value::Bytes` value on the unchanged hex branch.
#[test]
fn test_string_family_bareword_numeric_value_parses_as_string() {
    // (input, expected value, expected op)
    let cases: &[(&str, Value, Operator)] = &[
        // Bare equality (no operator token) -- also changed from Uint(0).
        (
            "0 string 000 face %s",
            Value::String("000".to_string()),
            Operator::Equal,
        ),
        (
            "0 string >0 \\b, name %s",
            Value::String("0".to_string()),
            Operator::GreaterThan,
        ),
        (
            "0 string >0.6.1 version %s",
            Value::String("0.6.1".to_string()),
            Operator::GreaterThan,
        ),
        (
            "0 string >20011231 date %s",
            Value::String("20011231".to_string()),
            Operator::GreaterThan,
        ),
        (
            "0 string <9 low %s",
            Value::String("9".to_string()),
            Operator::LessThan,
        ),
        // search-family bareword numeric value is likewise a string.
        // Equality op (not ordering): search is pattern-bearing, see S2.4.
        (
            "0 search/16 100 hit %s",
            Value::String("100".to_string()),
            Operator::Equal,
        ),
    ];

    for (input, expected_value, expected_op) in cases {
        let (_, rule) =
            parse_magic_rule(input).unwrap_or_else(|e| panic!("parse failed for {input:?}: {e:?}"));
        assert_eq!(
            rule.value, *expected_value,
            "value mismatch for {input:?}: got {:?}",
            rule.value
        );
        assert_eq!(rule.op, *expected_op, "operator mismatch for {input:?}");
    }
}

/// The fix must not change the two branches that were already correct for
/// string-family values: quoted strings stay `Value::String`, and
/// hex/escape byte sequences stay `Value::Bytes` (e.g. gzip's `\037\213`).
/// Hex-*letter* barewords also stay `Value::Bytes` per GOTCHAS S3.12 --
/// only the numeric subset changes.
#[test]
fn test_string_family_value_preserves_quoted_and_hex_bytes() {
    // Quoted numeric string stays a String, not a number.
    let (_, rule) = parse_magic_rule("0 string \"0\" literal").unwrap();
    assert_eq!(rule.value, Value::String("0".to_string()));

    // Octal-escape byte sequence stays Bytes (gzip magic).
    let (_, rule) = parse_magic_rule("0 string \\037\\213 gzip").unwrap();
    assert_eq!(rule.value, Value::Bytes(vec![0x1f, 0x8b]));

    // Mixed hex/ascii escape stays Bytes (ELF magic).
    let (_, rule) = parse_magic_rule("0 string \\177ELF elf").unwrap();
    assert_eq!(rule.value, Value::Bytes(vec![0x7f, b'E', b'L', b'F']));

    // Hex-letter bareword stays Bytes (boundary, unchanged -- GOTCHAS S3.12).
    let (_, rule) = parse_magic_rule("0 string >AB thing").unwrap();
    assert_eq!(rule.value, Value::Bytes(vec![0xAB]));
}

#[test]
fn parse_bare_string_value_high_byte_returns_bytes_not_lossy_string() {
    // GOTCHAS S6.7: a bareword string value whose resolved bytes contain a
    // high byte (>= 0x80) that is not valid UTF-8 must be captured as
    // `Value::Bytes` holding the RAW bytes, never lossy-decoded into a
    // `Value::String` (which turns the byte into U+FFFD -- 3 bytes -- and
    // both inflates the pattern length and changes the byte, so the rule
    // silently never matches). This is the parse-side mirror of
    // `read_string_exact` (S6.4). The signature begins with literal ASCII
    // (`HSP`), so it bypasses `parse_mixed_hex_ascii` (which requires a
    // leading `\`) and lands in `parse_bare_string_value`.
    //
    // Table: (input, expected value). Covers both high-byte escape forms
    // (hex `\x9b`, octal `\376`) plus the all-ASCII control that must stay
    // a `Value::String`.
    let cases: &[(&str, Value)] = &[
        // OS/2 INF top-level signature: HSP\x01\x9b\x00 -> raw bytes.
        (
            "0 string HSP\\x01\\x9b\\x00 OS/2 INF",
            Value::Bytes(vec![0x48, 0x53, 0x50, 0x01, 0x9b, 0x00]),
        ),
        // Octal high byte embedded after leading ASCII: AB\376 -> raw bytes.
        (
            "0 string AB\\376 thing",
            Value::Bytes(vec![0x41, 0x42, 0xFE]),
        ),
        // All-ASCII bareword (no high byte) stays a String -- %s renders.
        ("0 string HSP header", Value::String("HSP".to_string())),
    ];

    for (input, expected) in cases {
        let (_, rule) =
            parse_magic_rule(input).unwrap_or_else(|e| panic!("parse failed for {input:?}: {e:?}"));
        assert_eq!(
            rule.value, *expected,
            "value mismatch for {input:?}: got {:?}, expected {expected:?}",
            rule.value
        );
    }
}
