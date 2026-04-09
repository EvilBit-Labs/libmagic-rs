// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Value-literal parsing for magic files.
//!
//! Parses the right-hand side of magic rules (numeric, string, hex-byte, and
//! float literals) into [`Value`] AST nodes. Extracted from
//! `grammar/mod.rs` to keep that module under the project's file-size limit.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, multispace0, none_of, one_of},
    combinator::{map, opt, recognize},
    error::Error as NomError,
    multi::many0,
    sequence::pair,
};

use crate::parser::ast::Value;
use crate::parser::grammar::numbers::{parse_number, parse_unsigned_number};

/// Parse a single hex byte with \x prefix
pub(super) fn parse_hex_byte_with_prefix(input: &str) -> IResult<&str, u8> {
    let (input, _) = tag("\\x")(input)?;
    let (input, hex_str) = recognize(pair(
        one_of("0123456789abcdefABCDEF"),
        one_of("0123456789abcdefABCDEF"),
    ))
    .parse(input)?;
    let byte_val = u8::from_str_radix(hex_str, 16)
        .map_err(|_| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::MapRes)))?;
    Ok((input, byte_val))
}

/// Parse a hex byte sequence starting with \x prefix
pub(super) fn parse_hex_bytes_with_prefix(input: &str) -> IResult<&str, Vec<u8>> {
    if input.starts_with("\\x") {
        many0(parse_hex_byte_with_prefix).parse(input)
    } else {
        Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse a mixed hex and ASCII sequence (like \x7fELF)
pub(super) fn parse_mixed_hex_ascii(input: &str) -> IResult<&str, Vec<u8>> {
    // Must start with \ to be considered an escape sequence
    if !input.starts_with('\\') {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    let mut bytes = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        // Try to parse escape sequences first (hex, octal, etc.)
        if let Ok((new_remaining, escaped_char)) = parse_escape_sequence(remaining) {
            bytes.push(escaped_char as u8);
            remaining = new_remaining;
        } else if let Ok((new_remaining, hex_byte)) = parse_hex_byte_with_prefix(remaining) {
            bytes.push(hex_byte);
            remaining = new_remaining;
        } else if let Ok((new_remaining, ascii_char)) =
            none_of::<&str, &str, NomError<&str>>(" \t\n\r")(remaining)
        {
            // Parse regular ASCII character (not whitespace)
            bytes.push(ascii_char as u8);
            remaining = new_remaining;
        } else {
            // Stop if we can't parse anything more
            break;
        }
    }

    if bytes.is_empty() {
        Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    } else {
        Ok((remaining, bytes))
    }
}

/// Parse a hex byte sequence without prefix (only if it looks like pure hex bytes)
pub(super) fn parse_hex_bytes_no_prefix(input: &str) -> IResult<&str, Vec<u8>> {
    // Only parse as hex bytes if:
    // 1. Input has even number of hex digits (pairs)
    // 2. All characters are hex digits
    // 3. Doesn't start with 0x (that's a number)
    // 4. Contains at least one non-decimal digit (a-f, A-F)

    if input.starts_with("0x") || input.starts_with('-') {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    let hex_chars: String = input.chars().take_while(char::is_ascii_hexdigit).collect();

    if hex_chars.is_empty() || !hex_chars.len().is_multiple_of(2) {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    // Check if it contains non-decimal hex digits (a-f, A-F)
    let has_hex_letters = hex_chars
        .chars()
        .any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    if !has_hex_letters {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    // Parse pairs of hex digits
    let mut bytes = Vec::with_capacity(hex_chars.len() / 2);
    let mut chars = hex_chars.chars();
    while let (Some(c1), Some(c2)) = (chars.next(), chars.next()) {
        // Avoid format! allocation by parsing digits directly
        let digit1 = c1
            .to_digit(16)
            .ok_or_else(|| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::MapRes)))?;
        let digit2 = c2
            .to_digit(16)
            .ok_or_else(|| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::MapRes)))?;
        let byte_val = u8::try_from((digit1 << 4) | digit2)
            .map_err(|_| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::MapRes)))?;
        bytes.push(byte_val);
    }

    let remaining = &input[hex_chars.len()..];
    Ok((remaining, bytes))
}

/// Parse a hex byte sequence (e.g., "\\x7f\\x45\\x4c\\x46", "7f454c46", or "\\x7fELF")
pub(super) fn parse_hex_bytes(input: &str) -> IResult<&str, Vec<u8>> {
    alt((
        parse_mixed_hex_ascii,
        parse_hex_bytes_with_prefix,
        parse_hex_bytes_no_prefix,
    ))
    .parse(input)
}

/// Parse escape sequences in strings
pub(super) fn parse_escape_sequence(input: &str) -> IResult<&str, char> {
    let (input, _) = char('\\')(input)?;

    // Try to parse octal escape sequence first (\377, \123, etc.)
    if let Ok((remaining, octal_str)) = recognize(pair(
        one_of::<&str, &str, NomError<&str>>("0123"),
        pair(
            one_of::<&str, &str, NomError<&str>>("01234567"),
            one_of::<&str, &str, NomError<&str>>("01234567"),
        ),
    ))
    .parse(input)
        && let Ok(octal_value) = u8::from_str_radix(octal_str, 8)
    {
        return Ok((remaining, octal_value as char));
    }

    // Parse standard escape sequences
    let (input, escaped_char) = one_of("nrt\\\"'0")(input)?;

    let result_char = match escaped_char {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '\\' => '\\',
        '"' => '"',
        '\'' => '\'',
        '0' => '\0',
        _ => unreachable!("one_of constrains input to known escape characters"),
    };

    Ok((input, result_char))
}

/// Parse a quoted string with escape sequences
pub(super) fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    let (input, _) = multispace0(input)?;
    let (input, _) = char('"')(input)?;

    let mut result = String::new();
    let mut remaining = input;

    loop {
        // Try to parse an escape sequence first
        if let Ok((new_remaining, escaped_char)) = parse_escape_sequence(remaining) {
            result.push(escaped_char);
            remaining = new_remaining;
            continue;
        }

        // If no escape sequence, try to parse a regular character (not quote or backslash)
        if let Ok((new_remaining, regular_char)) =
            none_of::<&str, &str, NomError<&str>>("\"\\")(remaining)
        {
            result.push(regular_char);
            remaining = new_remaining;
            continue;
        }

        // If neither worked, we should be at the closing quote
        break;
    }

    let (remaining, _) = char('"')(remaining)?;
    let (remaining, _) = multispace0(remaining)?;

    Ok((remaining, result))
}

/// Parse a floating-point literal into `Value::Float(f64)`
///
/// Recognizes numbers with a mandatory decimal point (to distinguish from
/// integers), an optional leading minus sign, and an optional exponent part.
/// Examples: `3.14`, `-1.0`, `2.5e10`, `-0.5E-3`
pub(super) fn parse_float_value(input: &str) -> IResult<&str, f64> {
    let (input, _) = multispace0(input)?;

    let (remaining, float_str) = recognize((
        opt(char('-')),
        digit1,
        char('.'),
        digit1,
        opt((one_of("eE"), opt(one_of("+-")), digit1)),
    ))
    .parse(input)?;

    let value: f64 = float_str
        .parse()
        .map_err(|_| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::MapRes)))?;

    // Reject non-finite floats (NaN, +inf, -inf) to keep AST, JSON, and codegen valid
    if !value.is_finite() {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Float,
        )));
    }

    let (remaining, _) = multispace0(remaining)?;
    Ok((remaining, value))
}

/// Parse a numeric value (integer)
///
/// Non-negative literals are parsed directly as `u64` so the full unsigned
/// 64-bit range is representable (required for `uquad` values above `i64::MAX`).
/// Negative literals go through the signed `i64` path.
pub(super) fn parse_numeric_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;

    let (input, value) = if input.starts_with('-') {
        // Negative: parse as i64
        let (input, number) = parse_number(input)?;
        (input, Value::Int(number))
    } else {
        // Non-negative: parse as u64 to support full unsigned 64-bit range
        let (input, number) = parse_unsigned_number(input)?;
        (input, Value::Uint(number))
    };

    let (input, _) = multispace0(input)?;
    Ok((input, value))
}

/// Parse string, float, and numeric literals for magic rule values
///
/// Supports:
/// - Quoted strings with escape sequences: "Hello\nWorld", "ELF\0"
/// - Floating-point literals: 3.14, -1.0, 2.5e10
/// - Numeric literals (decimal): 123, -456
/// - Numeric literals (hexadecimal): 0x1a2b, -0xFF
/// - Hex byte sequences: \\x7f\\x45\\x4c\\x46 or 7f454c46
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_value;
/// use libmagic_rs::parser::ast::Value;
///
/// // String values
/// assert_eq!(parse_value("\"Hello\""), Ok(("", Value::String("Hello".to_string()))));
/// assert_eq!(parse_value("\"Line1\\nLine2\""), Ok(("", Value::String("Line1\nLine2".to_string()))));
///
/// // Numeric values
/// assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
/// assert_eq!(parse_value("-456"), Ok(("", Value::Int(-456))));
/// assert_eq!(parse_value("0x1a"), Ok(("", Value::Uint(26))));
/// assert_eq!(parse_value("-0xFF"), Ok(("", Value::Int(-255))));
///
/// // Hex byte sequences
/// assert_eq!(parse_value("\\x7f\\x45"), Ok(("", Value::Bytes(vec![0x7f, 0x45]))));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input is empty or contains no valid value
/// - Quoted string is not properly terminated
/// - Numeric value cannot be parsed as a valid integer
/// - Hex byte sequence contains invalid hex digits
/// - Input contains invalid characters for the detected value format
pub fn parse_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;

    // Handle empty input case - should fail for magic rules
    if input.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }

    // Try to parse different value types in order of specificity
    let (input, value) = alt((
        // Try quoted string first
        map(parse_quoted_string, Value::String),
        // Try hex byte sequence before numeric (to catch patterns like "7f", "ab", "\\x7fELF", etc.)
        map(parse_hex_bytes, Value::Bytes),
        // Try float before integer (a float literal is a superset of an integer prefix)
        map(parse_float_value, Value::Float),
        // Try numeric value last (for pure numbers like 0x123, 1, etc.)
        parse_numeric_value,
    ))
    .parse(input)?;

    Ok((input, value))
}
