// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Grammar parsing for magic files using nom parser combinators
//!
//! This module implements the parsing logic for magic file syntax, converting
//! text-based magic rules into the AST representation defined in ast.rs.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, digit1, hex_digit1, multispace0, none_of, one_of},
    combinator::{map, opt, recognize},
    error::Error as NomError,
    multi::many0,
    sequence::pair,
};

use crate::parser::ast::{MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

/// Parse a decimal number with overflow protection
fn parse_decimal_number(input: &str) -> IResult<&str, i64> {
    let (input, digits) = digit1(input)?;

    // Check for potential overflow before parsing
    if digits.len() > 19 {
        // i64::MAX has 19 digits, so anything longer will definitely overflow
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::MapRes,
        )));
    }

    let number = digits.parse::<i64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })?;
    Ok((input, number))
}

/// Parse a decimal number as unsigned `u64` with overflow protection
fn parse_unsigned_decimal_number(input: &str) -> IResult<&str, u64> {
    let (input, digits) = digit1(input)?;

    // u64::MAX (18446744073709551615) has 20 digits
    if digits.len() > 20 {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::MapRes,
        )));
    }

    let number = digits.parse::<u64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })?;
    Ok((input, number))
}

/// Parse a hexadecimal number (with 0x prefix) with overflow protection
fn parse_hex_number(input: &str) -> IResult<&str, i64> {
    let (input, _) = tag("0x")(input)?;
    let (input, hex_str) = hex_digit1(input)?;

    // Check for potential overflow - i64 can hold up to 16 hex digits (0x7FFFFFFFFFFFFFFF)
    if hex_str.len() > 16 {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::MapRes,
        )));
    }

    let number = i64::from_str_radix(hex_str, 16).map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })?;

    Ok((input, number))
}

/// Parse a hexadecimal number (with 0x prefix) as unsigned `u64`
fn parse_unsigned_hex_number(input: &str) -> IResult<&str, u64> {
    let (input, _) = tag("0x")(input)?;
    let (input, hex_str) = hex_digit1(input)?;

    // u64 can hold up to 16 hex digits (0xFFFFFFFFFFFFFFFF)
    if hex_str.len() > 16 {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::MapRes,
        )));
    }

    let number = u64::from_str_radix(hex_str, 16).map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })?;

    Ok((input, number))
}

/// Parse a non-negative number as unsigned `u64`
///
/// Supports both decimal and hexadecimal (0x prefix) formats.
/// Does not handle a leading minus sign -- callers handle sign detection.
fn parse_unsigned_number(input: &str) -> IResult<&str, u64> {
    if input.starts_with("0x") {
        parse_unsigned_hex_number(input)
    } else {
        parse_unsigned_decimal_number(input)
    }
}

/// Parse a decimal or hexadecimal number
///
/// Supports both decimal (123, -456) and hexadecimal (0x1a2b, -0xFF) formats.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_number;
///
/// assert_eq!(parse_number("123"), Ok(("", 123)));
/// assert_eq!(parse_number("0x1a"), Ok(("", 26)));
/// assert_eq!(parse_number("-42"), Ok(("", -42)));
/// assert_eq!(parse_number("-0xFF"), Ok(("", -255)));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input is empty or contains no valid digits
/// - Hexadecimal number lacks proper "0x" prefix or contains invalid hex digits
/// - Number cannot be parsed as a valid `i64` value
/// - Input contains invalid characters for the detected number format
pub fn parse_number(input: &str) -> IResult<&str, i64> {
    let (input, sign) = opt(char('-')).parse(input)?;
    let is_negative = sign.is_some();

    // Check if input starts with "0x" - if so, it must be a valid hex number
    let (input, number) = if input.starts_with("0x") {
        parse_hex_number(input)?
    } else {
        parse_decimal_number(input)?
    };

    // Apply sign with overflow checking
    let result = if is_negative {
        number.checked_neg().ok_or_else(|| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
        })?
    } else {
        number
    };

    Ok((input, result))
}

/// Parse an offset specification for absolute offsets
///
/// Supports decimal and hexadecimal formats, both positive and negative.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_offset;
/// use libmagic_rs::parser::ast::OffsetSpec;
///
/// assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
/// assert_eq!(parse_offset("123"), Ok(("", OffsetSpec::Absolute(123))));
/// assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16))));
/// assert_eq!(parse_offset("-4"), Ok(("", OffsetSpec::Absolute(-4))));
/// assert_eq!(parse_offset("-0xFF"), Ok(("", OffsetSpec::Absolute(-255))));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - The input contains invalid number format (propagated from `parse_number`)
/// - Input is empty or contains no parseable offset value
/// - The offset value cannot be represented as a valid `i64`
pub fn parse_offset(input: &str) -> IResult<&str, OffsetSpec> {
    let (input, _) = multispace0(input)?;
    let (input, offset_value) = parse_number(input)?;
    let (input, _) = multispace0(input)?;

    Ok((input, OffsetSpec::Absolute(offset_value)))
}

/// Parse comparison operators for magic rules
///
/// Supports both symbolic and text representations of operators:
/// - `=` or `==` for equality
/// - `!=` or `<>` for inequality
/// - `<` for less-than
/// - `>` for greater-than
/// - `<=` for less-than-or-equal
/// - `>=` for greater-than-or-equal
/// - `&` for bitwise AND
/// - `^` for bitwise XOR
/// - `~` for bitwise NOT
/// - `x` for any value (always matches)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_operator;
/// use libmagic_rs::parser::ast::Operator;
///
/// assert_eq!(parse_operator("="), Ok(("", Operator::Equal)));
/// assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));
/// assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));
/// assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));
/// assert_eq!(parse_operator("<"), Ok(("", Operator::LessThan)));
/// assert_eq!(parse_operator(">"), Ok(("", Operator::GreaterThan)));
/// assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));
/// assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));
/// assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));
/// assert_eq!(parse_operator("^"), Ok(("", Operator::BitwiseXor)));
/// assert_eq!(parse_operator("~"), Ok(("", Operator::BitwiseNot)));
/// assert_eq!(parse_operator("x"), Ok(("", Operator::AnyValue)));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input does not start with a recognized operator symbol
/// - Input is empty or contains no valid operator
/// - Operator syntax is incomplete (e.g., just `!` without `=`)
pub fn parse_operator(input: &str) -> IResult<&str, Operator> {
    let (input, _) = multispace0(input)?;

    // Try to parse each operator, starting with longer ones first
    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("==")(input) {
        // Check that we don't have another '=' following (to reject "===")
        if remaining.starts_with('=') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::Equal));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("!=")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::NotEqual));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("<>")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::NotEqual));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("<=")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::LessEqual));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>(">=")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::GreaterEqual));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("=")(input) {
        // Check that we don't have another '=' following (to reject "==")
        if remaining.starts_with('=') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::Equal));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("&")(input) {
        // Check that we don't have another '&' following (to reject "&&")
        if remaining.starts_with('&') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::BitwiseAnd));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("^")(input) {
        if remaining.starts_with('^') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::BitwiseXor));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("~")(input) {
        if remaining.starts_with('~') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::BitwiseNot));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("x")(input) {
        // Ensure 'x' is not followed by alphanumeric (e.g., "x42" is not AnyValue)
        if remaining.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::AnyValue));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("<")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::LessThan));
    }

    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>(">")(input) {
        let (remaining, _) = multispace0(remaining)?;
        return Ok((remaining, Operator::GreaterThan));
    }

    // If no operator matches, return an error
    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Tag,
    )))
}

/// Parse a single hex byte with \x prefix
fn parse_hex_byte_with_prefix(input: &str) -> IResult<&str, u8> {
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
fn parse_hex_bytes_with_prefix(input: &str) -> IResult<&str, Vec<u8>> {
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
fn parse_mixed_hex_ascii(input: &str) -> IResult<&str, Vec<u8>> {
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
fn parse_hex_bytes_no_prefix(input: &str) -> IResult<&str, Vec<u8>> {
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
fn parse_hex_bytes(input: &str) -> IResult<&str, Vec<u8>> {
    alt((
        parse_mixed_hex_ascii,
        parse_hex_bytes_with_prefix,
        parse_hex_bytes_no_prefix,
    ))
    .parse(input)
}

/// Parse escape sequences in strings
fn parse_escape_sequence(input: &str) -> IResult<&str, char> {
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
fn parse_quoted_string(input: &str) -> IResult<&str, String> {
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

/// Parse a numeric value (integer)
///
/// Non-negative literals are parsed directly as `u64` so the full unsigned
/// 64-bit range is representable (required for `uquad` values above `i64::MAX`).
/// Negative literals go through the signed `i64` path.
fn parse_numeric_value(input: &str) -> IResult<&str, Value> {
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

/// Parse string and numeric literals for magic rule values
///
/// Supports:
/// - Quoted strings with escape sequences: "Hello\nWorld", "ELF\0"
/// - Numeric literals (decimal): 123, -456
/// - Numeric literals (hexadecimal): 0x1a2b, -0xFF
/// - Hex byte sequences: \\x7f\\x45\\x4c\\x46 or 7f454c46
///
/// # Examples
///
/// ```
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
        // Try numeric value last (for pure numbers like 0x123, 1, etc.)
        parse_numeric_value,
    ))
    .parse(input)?;

    Ok((input, value))
}

/// Parse a type specification with an optional attached bitwise-AND mask operator
/// (e.g., `lelong&0xf0000000`).
///
/// Returns the `TypeKind` and an optional `Operator`.
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected format
pub fn parse_type_and_operator(input: &str) -> IResult<&str, (TypeKind, Option<Operator>)> {
    let (input, _) = multispace0(input)?;

    let (input, type_name) = crate::parser::types::parse_type_keyword(input)?;

    // Check for attached operator with mask (like &0xf0000000)
    // Uses unsigned parsing so full u64 masks (e.g. 0xffffffffffffffff) are supported.
    // If '&' is followed by digits/0x but the mask parse fails (overflow, etc.),
    // we return a hard error instead of silently falling back to standalone '&'.
    let (input, attached_op) = if let Some(after_amp) = input.strip_prefix('&') {
        if after_amp.starts_with("0x") || after_amp.starts_with(|c: char| c.is_ascii_digit()) {
            // '&' followed by what looks like a number -- must parse as mask
            let (rest, mask) = parse_unsigned_number(after_amp).map_err(|_| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
            })?;
            (rest, Some(Operator::BitwiseAndMask(mask)))
        } else if after_amp.starts_with('&') {
            // Reject '&&' -- not valid operator syntax
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        } else {
            // Standalone '&' (no digits following)
            (after_amp, Some(Operator::BitwiseAnd))
        }
    } else {
        (input, None)
    };

    let (input, _) = multispace0(input)?;

    let type_kind = crate::parser::types::type_keyword_to_kind(type_name);

    Ok((input, (type_kind, attached_op)))
}

/// Parse a type specification (byte, short, long, quad, string, etc.)
///
/// Supports various type formats found in magic files:
/// - `byte` / `ubyte` - single byte (signed / unsigned)
/// - `short` / `ushort` - 16-bit integer (native endian, signed / unsigned)
/// - `leshort` / `uleshort` - 16-bit little-endian integer
/// - `beshort` / `ubeshort` - 16-bit big-endian integer
/// - `long` / `ulong` - 32-bit integer (native endian, signed / unsigned)
/// - `lelong` / `ulelong` - 32-bit little-endian integer
/// - `belong` / `ubelong` - 32-bit big-endian integer
/// - `quad` / `uquad` - 64-bit integer (native endian, signed / unsigned)
/// - `lequad` / `ulequad` - 64-bit little-endian integer
/// - `bequad` / `ubequad` - 64-bit big-endian integer
/// - `string` - null-terminated string
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_type;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness};
///
/// assert_eq!(parse_type("byte"), Ok(("", TypeKind::Byte { signed: true })));
/// assert_eq!(parse_type("leshort"), Ok(("", TypeKind::Short { endian: Endianness::Little, signed: true })));
/// assert_eq!(parse_type("bequad"), Ok(("", TypeKind::Quad { endian: Endianness::Big, signed: true })));
/// assert_eq!(parse_type("string"), Ok(("", TypeKind::String { max_length: None })));
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match any known type
pub fn parse_type(input: &str) -> IResult<&str, TypeKind> {
    let (input, (type_kind, _)) = parse_type_and_operator(input)?;
    Ok((input, type_kind))
}

/// Parse the indentation level and offset for magic rules
///
/// Handles both absolute offsets and hierarchical child rules with `>` prefix.
/// Child rules can be nested multiple levels deep with multiple `>` characters.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_rule_offset;
/// use libmagic_rs::parser::ast::OffsetSpec;
///
/// // Absolute offset
/// assert_eq!(parse_rule_offset("0"), Ok(("", (0, OffsetSpec::Absolute(0)))));
/// assert_eq!(parse_rule_offset("16"), Ok(("", (0, OffsetSpec::Absolute(16)))));
///
/// // Child rule (level 1)
/// assert_eq!(parse_rule_offset(">4"), Ok(("", (1, OffsetSpec::Absolute(4)))));
///
/// // Nested child rule (level 2)
/// assert_eq!(parse_rule_offset(">>8"), Ok(("", (2, OffsetSpec::Absolute(8)))));
/// ```
/// Parse rule offset with hierarchy level (> prefixes) and offset specification
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected offset format
pub fn parse_rule_offset(input: &str) -> IResult<&str, (u32, OffsetSpec)> {
    let (input, _) = multispace0(input)?;

    // Count the number of '>' characters for nesting level
    let (input, level_chars) = many0(char('>')).parse(input)?;
    let level = u32::try_from(level_chars.len()).unwrap_or(0);

    // Parse the offset after the '>' characters
    let (input, offset_spec) = parse_offset(input)?;

    Ok((input, (level, offset_spec)))
}

/// Parse the message part of a magic rule
///
/// The message is everything after the value until the end of the line.
/// It may contain format specifiers and can be empty.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_message;
///
/// assert_eq!(parse_message("ELF executable"), Ok(("", "ELF executable".to_string())));
/// assert_eq!(parse_message(""), Ok(("", "".to_string())));
/// assert_eq!(parse_message("  \tPDF document  "), Ok(("", "PDF document".to_string())));
/// ```
/// Parse the message/description part of a magic rule
///
/// # Errors
/// Returns a nom parsing error if the input cannot be parsed as a message
pub fn parse_message(input: &str) -> IResult<&str, String> {
    let (input, _) = multispace0(input)?;

    // Take everything until end of line, trimming whitespace
    // Use take_while instead of take_while1 to handle empty messages
    let (input, message_text) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
    let message = message_text.trim().to_string();

    Ok((input, message))
}

/// Parse a strength directive (`!:strength` line)
///
/// Parses the `!:strength` directive that modifies rule strength.
/// Format: `!:strength [+|-|*|/|=]N` or `!:strength N`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_strength_directive;
/// use libmagic_rs::parser::ast::StrengthModifier;
///
/// assert_eq!(parse_strength_directive("!:strength +10"), Ok(("", StrengthModifier::Add(10))));
/// assert_eq!(parse_strength_directive("!:strength -5"), Ok(("", StrengthModifier::Subtract(5))));
/// assert_eq!(parse_strength_directive("!:strength *2"), Ok(("", StrengthModifier::Multiply(2))));
/// assert_eq!(parse_strength_directive("!:strength /2"), Ok(("", StrengthModifier::Divide(2))));
/// assert_eq!(parse_strength_directive("!:strength =50"), Ok(("", StrengthModifier::Set(50))));
/// assert_eq!(parse_strength_directive("!:strength 50"), Ok(("", StrengthModifier::Set(50))));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input doesn't start with `!:strength`
/// - The modifier value cannot be parsed as a valid integer
/// - The operator is invalid
pub fn parse_strength_directive(input: &str) -> IResult<&str, StrengthModifier> {
    // Helper to safely convert i64 to i32 with clamping to valid strength range.
    // This prevents silent truncation to 0 on overflow while keeping values in bounds.
    fn clamp_to_i32(n: i64) -> i32 {
        // Use i64::from for lossless conversion, then clamp and convert back
        let clamped = n.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
        // Safe to unwrap: clamped value is guaranteed to be in i32 range
        i32::try_from(clamped).unwrap()
    }

    let (input, _) = multispace0(input)?;
    let (input, _) = tag("!:strength")(input)?;
    let (input, _) = multispace0(input)?;

    // Parse the operator: +, -, *, /, = or bare number (implies =)
    let (input, modifier) = alt((
        // +N -> Add
        map(pair(char('+'), parse_number), |(_, n)| {
            StrengthModifier::Add(clamp_to_i32(n))
        }),
        // -N -> Subtract (note: parse_number handles negative, so we need special handling)
        map(pair(char('-'), parse_decimal_number), |(_, n)| {
            StrengthModifier::Subtract(clamp_to_i32(n))
        }),
        // *N -> Multiply
        map(pair(char('*'), parse_number), |(_, n)| {
            StrengthModifier::Multiply(clamp_to_i32(n))
        }),
        // /N -> Divide
        map(pair(char('/'), parse_number), |(_, n)| {
            StrengthModifier::Divide(clamp_to_i32(n))
        }),
        // =N -> Set
        map(pair(char('='), parse_number), |(_, n)| {
            StrengthModifier::Set(clamp_to_i32(n))
        }),
        // Bare number -> Set
        map(parse_number, |n| StrengthModifier::Set(clamp_to_i32(n))),
    ))
    .parse(input)?;

    Ok((input, modifier))
}

/// Check if a line is a strength directive (starts with !:strength)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::is_strength_directive;
///
/// assert!(is_strength_directive("!:strength +10"));
/// assert!(is_strength_directive("  !:strength -5"));
/// assert!(!is_strength_directive("0 byte 1"));
/// ```
#[must_use]
pub fn is_strength_directive(input: &str) -> bool {
    input.trim().starts_with("!:strength")
}

/// Parse a complete magic rule line from text format
///
/// Parses a complete magic rule in the format:
/// `[>...]offset type [operator] value [message]`
///
/// Where:
/// - `>...` indicates child rule nesting level (optional)
/// - `offset` is the byte offset to read from
/// - `type` is the data type (byte, short, long, string, etc.)
/// - `operator` is the comparison operator (=, !=, &) - defaults to = if omitted
/// - `value` is the expected value to compare against
/// - `message` is the human-readable description (optional)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_magic_rule;
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
///
/// // Basic rule
/// let input = "0 string \\x7fELF ELF executable";
/// let (_, rule) = parse_magic_rule(input).unwrap();
/// assert_eq!(rule.level, 0);
/// assert_eq!(rule.message, "ELF executable");
///
/// // Child rule
/// let input = ">4 byte 1 32-bit";
/// let (_, rule) = parse_magic_rule(input).unwrap();
/// assert_eq!(rule.level, 1);
/// assert_eq!(rule.message, "32-bit");
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - The offset specification is invalid
/// - The type specification is not recognized
/// - The operator is invalid (if present)
/// - The value cannot be parsed
/// - The input format doesn't match the expected magic rule syntax
pub fn parse_magic_rule(input: &str) -> IResult<&str, MagicRule> {
    let (input, _) = multispace0(input)?;

    // Parse the offset with nesting level
    let (input, (level, offset)) = parse_rule_offset(input)?;

    // Parse the type and any attached operator
    let (input, (typ, attached_op)) = parse_type_and_operator(input)?;

    // Try to parse a separate operator (optional - use attached operator if present)
    let (input, separate_op) = opt(parse_operator).parse(input)?;
    let op = attached_op.or(separate_op).unwrap_or(Operator::Equal);

    // For AnyValue (`x`), no operand is needed -- treat remaining text as message
    let (input, value) = if op == Operator::AnyValue {
        (input, Value::Uint(0))
    } else {
        parse_value(input)?
    };

    // Parse the message (optional - everything remaining on the line)
    let (input, message) = if input.trim().is_empty() {
        (input, String::new())
    } else {
        parse_message(input)?
    };

    let rule = MagicRule {
        offset,
        typ,
        op,
        value,
        message,
        children: vec![], // Children will be added during hierarchical parsing
        level,
        strength_modifier: None, // Will be set during directive parsing
    };

    Ok((input, rule))
}

/// Parse a comment line (starts with #)
///
/// Comments in magic files start with '#' and continue to the end of the line.
/// This function consumes the entire comment line.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::parse_comment;
///
/// assert_eq!(parse_comment("# This is a comment"), Ok(("", "This is a comment".to_string())));
/// assert_eq!(parse_comment("#"), Ok(("", "".to_string())));
/// ```
/// Parse a comment line (starting with #)
///
/// # Errors
/// Returns a nom parsing error if the input is not a valid comment
pub fn parse_comment(input: &str) -> IResult<&str, String> {
    let (input, _) = multispace0(input)?;
    let (input, _) = char('#').parse(input)?;
    let (input, comment_text) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
    let comment = comment_text.trim().to_string();
    Ok((input, comment))
}

/// Check if a line is empty or contains only whitespace
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::is_empty_line;
///
/// assert!(is_empty_line(""));
/// assert!(is_empty_line("   "));
/// assert!(is_empty_line("\t\t"));
/// assert!(!is_empty_line("0 byte 1"));
/// ```
#[must_use]
pub fn is_empty_line(input: &str) -> bool {
    input.trim().is_empty()
}

/// Check if a line is a comment (starts with #)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::is_comment_line;
///
/// assert!(is_comment_line("# This is a comment"));
/// assert!(is_comment_line("#"));
/// assert!(is_comment_line("  # Indented comment"));
/// assert!(!is_comment_line("0 byte 1"));
/// ```
#[must_use]
pub fn is_comment_line(input: &str) -> bool {
    input.trim().starts_with('#')
}

/// Check if a line ends with a continuation character (\)
///
/// Magic files support line continuation with backslash at the end of lines.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::grammar::has_continuation;
///
/// assert!(has_continuation("0 string test \\"));
/// assert!(has_continuation("message continues \\"));
/// assert!(!has_continuation("0 string test"));
/// ```
#[must_use]
pub fn has_continuation(input: &str) -> bool {
    input.trim_end().ends_with('\\')
}
#[cfg(test)]
mod tests;
