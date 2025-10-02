//! Grammar parsing for magic files using nom parser combinators
//!
//! This module implements the parsing logic for magic file syntax, converting
//! text-based magic rules into the AST representation defined in ast.rs.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, hex_digit1, multispace0, none_of, one_of},
    combinator::{map, opt, recognize},
    error::Error as NomError,
    multi::many0,
    sequence::pair,
};

use crate::parser::ast::{OffsetSpec, Operator, Value};

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
/// - `&` for bitwise AND
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
/// assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));
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

    if hex_chars.is_empty() || hex_chars.len() % 2 != 0 {
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
    let mut bytes = Vec::new();
    let mut chars = hex_chars.chars();
    while let (Some(c1), Some(c2)) = (chars.next(), chars.next()) {
        let hex_str = format!("{c1}{c2}");
        if let Ok(byte_val) = u8::from_str_radix(&hex_str, 16) {
            bytes.push(byte_val);
        } else {
            return Err(nom::Err::Error(NomError::new(
                input,
                nom::error::ErrorKind::MapRes,
            )));
        }
    }

    let remaining = &input[hex_chars.len()..];
    Ok((remaining, bytes))
}

/// Parse a hex byte sequence (e.g., "\\x7f\\x45\\x4c\\x46" or "7f454c46")
fn parse_hex_bytes(input: &str) -> IResult<&str, Vec<u8>> {
    alt((parse_hex_bytes_with_prefix, parse_hex_bytes_no_prefix)).parse(input)
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
    {
        if let Ok(octal_value) = u8::from_str_radix(octal_str, 8) {
            return Ok((remaining, octal_value as char));
        }
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
        _ => escaped_char, // Fallback for other characters
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
fn parse_numeric_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;
    let (input, number) = parse_number(input)?;
    let (input, _) = multispace0(input)?;

    // Convert to appropriate Value variant based on sign
    let value = if number >= 0 {
        Value::Uint(number.unsigned_abs())
    } else {
        Value::Int(number)
    };

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

    // Handle empty input case
    if input.is_empty() {
        return Ok((input, Value::Bytes(vec![])));
    }

    // Try to parse different value types in order of specificity
    let (input, value) = alt((
        // Try quoted string first
        map(parse_quoted_string, Value::String),
        // Try hex byte sequence before numeric (to catch patterns like "7f", "ab", etc.)
        map(parse_hex_bytes, Value::Bytes),
        // Try numeric value last (including hex numbers with 0x prefix)
        parse_numeric_value,
    ))
    .parse(input)?;

    Ok((input, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to test parsing with various whitespace patterns
    #[allow(dead_code)] // TODO: Use this helper in future whitespace tests
    fn test_with_whitespace_variants<T, F>(input: &str, expected: &T, parser: F)
    where
        T: Clone + PartialEq + std::fmt::Debug,
        F: Fn(&str) -> IResult<&str, T>,
    {
        // Test with various whitespace patterns - pre-allocate Vec with known capacity
        let mut whitespace_variants = Vec::with_capacity(9);
        whitespace_variants.extend([
            format!(" {input}"),    // Leading space
            format!("  {input}"),   // Leading spaces
            format!("\t{input}"),   // Leading tab
            format!("{input} "),    // Trailing space
            format!("{input}  "),   // Trailing spaces
            format!("{input}\t"),   // Trailing tab
            format!(" {input} "),   // Both leading and trailing space
            format!("  {input}  "), // Both leading and trailing spaces
            format!("\t{input}\t"), // Both leading and trailing tabs
        ]);

        for variant in whitespace_variants {
            assert_eq!(
                parser(&variant),
                Ok(("", expected.clone())),
                "Failed to parse with whitespace: '{variant}'"
            );
        }
    }

    /// Helper function to test number parsing with remaining input
    fn test_number_with_remaining_input() {
        // Pre-allocate with known capacity for better performance
        let test_cases = [
            ("123abc", 123, "abc"),
            ("0xFF rest", 255, " rest"),
            ("-42 more", -42, " more"),
            ("0x10,next", 16, ",next"),
        ];

        for (input, expected_num, expected_remaining) in test_cases {
            assert_eq!(
                parse_number(input),
                Ok((expected_remaining, expected_num)),
                "Failed to parse number with remaining input: '{input}'"
            );
        }
    }

    #[test]
    fn test_parse_decimal_number() {
        assert_eq!(parse_decimal_number("123"), Ok(("", 123)));
        assert_eq!(parse_decimal_number("0"), Ok(("", 0)));
        assert_eq!(parse_decimal_number("999"), Ok(("", 999)));

        // Should fail on non-digits
        assert!(parse_decimal_number("abc").is_err());
        assert!(parse_decimal_number("").is_err());
    }

    #[test]
    fn test_parse_hex_number() {
        assert_eq!(parse_hex_number("0x0"), Ok(("", 0)));
        assert_eq!(parse_hex_number("0x10"), Ok(("", 16)));
        assert_eq!(parse_hex_number("0xFF"), Ok(("", 255)));
        assert_eq!(parse_hex_number("0xabc"), Ok(("", 2748)));
        assert_eq!(parse_hex_number("0xABC"), Ok(("", 2748)));

        // Should fail without 0x prefix
        assert!(parse_hex_number("FF").is_err());
        assert!(parse_hex_number("10").is_err());

        // Should fail on invalid hex digits
        assert!(parse_hex_number("0xGG").is_err());
    }

    #[test]
    fn test_parse_number_positive() {
        // Decimal numbers
        assert_eq!(parse_number("0"), Ok(("", 0)));
        assert_eq!(parse_number("123"), Ok(("", 123)));
        assert_eq!(parse_number("999"), Ok(("", 999)));

        // Hexadecimal numbers
        assert_eq!(parse_number("0x0"), Ok(("", 0)));
        assert_eq!(parse_number("0x10"), Ok(("", 16)));
        assert_eq!(parse_number("0xFF"), Ok(("", 255)));
        assert_eq!(parse_number("0xabc"), Ok(("", 2748)));
    }

    #[test]
    fn test_parse_number_negative() {
        // Negative decimal numbers
        assert_eq!(parse_number("-1"), Ok(("", -1)));
        assert_eq!(parse_number("-123"), Ok(("", -123)));
        assert_eq!(parse_number("-999"), Ok(("", -999)));

        // Negative hexadecimal numbers
        assert_eq!(parse_number("-0x1"), Ok(("", -1)));
        assert_eq!(parse_number("-0x10"), Ok(("", -16)));
        assert_eq!(parse_number("-0xFF"), Ok(("", -255)));
        assert_eq!(parse_number("-0xabc"), Ok(("", -2748)));
    }

    #[test]
    fn test_parse_number_edge_cases() {
        // Zero with different formats
        assert_eq!(parse_number("0"), Ok(("", 0)));
        assert_eq!(parse_number("-0"), Ok(("", 0)));
        assert_eq!(parse_number("0x0"), Ok(("", 0)));
        assert_eq!(parse_number("-0x0"), Ok(("", 0)));

        // Large numbers
        assert_eq!(parse_number("2147483647"), Ok(("", 2_147_483_647))); // i32::MAX
        assert_eq!(parse_number("-2147483648"), Ok(("", -2_147_483_648))); // i32::MIN
        assert_eq!(parse_number("0x7FFFFFFF"), Ok(("", 2_147_483_647))); // i32::MAX in hex

        // Should fail on invalid input
        assert!(parse_number("").is_err());
        assert!(parse_number("abc").is_err());
        assert!(parse_number("0xGG").is_err());
        assert!(parse_number("--123").is_err());
    }

    #[test]
    fn test_parse_number_with_remaining_input() {
        // Use helper function to reduce code duplication
        test_number_with_remaining_input();
    }

    #[test]
    fn test_parse_offset_absolute_positive() {
        assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
        assert_eq!(parse_offset("123"), Ok(("", OffsetSpec::Absolute(123))));
        assert_eq!(parse_offset("999"), Ok(("", OffsetSpec::Absolute(999))));

        // Hexadecimal offsets
        assert_eq!(parse_offset("0x0"), Ok(("", OffsetSpec::Absolute(0))));
        assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16))));
        assert_eq!(parse_offset("0xFF"), Ok(("", OffsetSpec::Absolute(255))));
        assert_eq!(parse_offset("0xabc"), Ok(("", OffsetSpec::Absolute(2748))));
    }

    #[test]
    fn test_parse_offset_absolute_negative() {
        assert_eq!(parse_offset("-1"), Ok(("", OffsetSpec::Absolute(-1))));
        assert_eq!(parse_offset("-123"), Ok(("", OffsetSpec::Absolute(-123))));
        assert_eq!(parse_offset("-999"), Ok(("", OffsetSpec::Absolute(-999))));

        // Negative hexadecimal offsets
        assert_eq!(parse_offset("-0x1"), Ok(("", OffsetSpec::Absolute(-1))));
        assert_eq!(parse_offset("-0x10"), Ok(("", OffsetSpec::Absolute(-16))));
        assert_eq!(parse_offset("-0xFF"), Ok(("", OffsetSpec::Absolute(-255))));
        assert_eq!(
            parse_offset("-0xabc"),
            Ok(("", OffsetSpec::Absolute(-2748)))
        );
    }

    #[test]
    fn test_parse_offset_with_whitespace() {
        // Leading whitespace
        assert_eq!(parse_offset(" 123"), Ok(("", OffsetSpec::Absolute(123))));
        assert_eq!(parse_offset("  0x10"), Ok(("", OffsetSpec::Absolute(16))));
        assert_eq!(parse_offset("\t-42"), Ok(("", OffsetSpec::Absolute(-42))));

        // Trailing whitespace
        assert_eq!(parse_offset("123 "), Ok(("", OffsetSpec::Absolute(123))));
        assert_eq!(parse_offset("0x10  "), Ok(("", OffsetSpec::Absolute(16))));
        assert_eq!(parse_offset("-42\t"), Ok(("", OffsetSpec::Absolute(-42))));

        // Both leading and trailing whitespace
        assert_eq!(parse_offset(" 123 "), Ok(("", OffsetSpec::Absolute(123))));
        assert_eq!(parse_offset("  0x10  "), Ok(("", OffsetSpec::Absolute(16))));
        assert_eq!(parse_offset("\t-42\t"), Ok(("", OffsetSpec::Absolute(-42))));
    }

    #[test]
    fn test_parse_offset_with_remaining_input() {
        // Should parse offset and leave remaining input
        assert_eq!(
            parse_offset("123 byte"),
            Ok(("byte", OffsetSpec::Absolute(123)))
        );
        assert_eq!(parse_offset("0xFF ="), Ok(("=", OffsetSpec::Absolute(255))));
        assert_eq!(
            parse_offset("-42,next"),
            Ok((",next", OffsetSpec::Absolute(-42)))
        );
        assert_eq!(
            parse_offset("0x10\tlong"),
            Ok(("long", OffsetSpec::Absolute(16)))
        );
    }

    #[test]
    fn test_parse_offset_edge_cases() {
        // Zero with different formats
        assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
        assert_eq!(parse_offset("-0"), Ok(("", OffsetSpec::Absolute(0))));
        assert_eq!(parse_offset("0x0"), Ok(("", OffsetSpec::Absolute(0))));
        assert_eq!(parse_offset("-0x0"), Ok(("", OffsetSpec::Absolute(0))));

        // Large offsets
        assert_eq!(
            parse_offset("2147483647"),
            Ok(("", OffsetSpec::Absolute(2_147_483_647)))
        );
        assert_eq!(
            parse_offset("-2147483648"),
            Ok(("", OffsetSpec::Absolute(-2_147_483_648)))
        );
        assert_eq!(
            parse_offset("0x7FFFFFFF"),
            Ok(("", OffsetSpec::Absolute(2_147_483_647)))
        );

        // Should fail on invalid input
        assert!(parse_offset("").is_err());
        assert!(parse_offset("abc").is_err());
        assert!(parse_offset("0xGG").is_err());
        assert!(parse_offset("--123").is_err());
    }

    #[test]
    fn test_parse_offset_common_magic_file_values() {
        // Common offsets found in magic files
        assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0)))); // File start
        assert_eq!(parse_offset("4"), Ok(("", OffsetSpec::Absolute(4)))); // After magic number
        assert_eq!(parse_offset("16"), Ok(("", OffsetSpec::Absolute(16)))); // Common header offset
        assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16)))); // Same as above in hex
        assert_eq!(parse_offset("512"), Ok(("", OffsetSpec::Absolute(512)))); // Sector boundary
        assert_eq!(parse_offset("0x200"), Ok(("", OffsetSpec::Absolute(512)))); // Same in hex

        // Negative offsets (from end of file)
        assert_eq!(parse_offset("-4"), Ok(("", OffsetSpec::Absolute(-4)))); // 4 bytes from end
        assert_eq!(parse_offset("-16"), Ok(("", OffsetSpec::Absolute(-16)))); // 16 bytes from end
        assert_eq!(parse_offset("-0x10"), Ok(("", OffsetSpec::Absolute(-16)))); // Same in hex
    }

    #[test]
    fn test_parse_offset_boundary_values() {
        // Test boundary values that might cause issues
        assert_eq!(parse_offset("1"), Ok(("", OffsetSpec::Absolute(1))));
        assert_eq!(parse_offset("-1"), Ok(("", OffsetSpec::Absolute(-1))));

        // Powers of 2 (common in binary formats)
        assert_eq!(parse_offset("256"), Ok(("", OffsetSpec::Absolute(256))));
        assert_eq!(parse_offset("0x100"), Ok(("", OffsetSpec::Absolute(256))));
        assert_eq!(parse_offset("1024"), Ok(("", OffsetSpec::Absolute(1024))));
        assert_eq!(parse_offset("0x400"), Ok(("", OffsetSpec::Absolute(1024))));

        // Large but reasonable file offsets
        assert_eq!(
            parse_offset("1048576"),
            Ok(("", OffsetSpec::Absolute(1_048_576)))
        ); // 1MB
        assert_eq!(
            parse_offset("0x100000"),
            Ok(("", OffsetSpec::Absolute(1_048_576)))
        );
    }

    // Operator parsing tests
    #[test]
    fn test_parse_operator_equality() {
        // Single equals sign
        assert_eq!(parse_operator("="), Ok(("", Operator::Equal)));

        // Double equals sign
        assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));

        // With whitespace
        assert_eq!(parse_operator(" = "), Ok(("", Operator::Equal)));
        assert_eq!(parse_operator("  ==  "), Ok(("", Operator::Equal)));
        assert_eq!(parse_operator("\t=\t"), Ok(("", Operator::Equal)));
    }

    #[test]
    fn test_parse_operator_inequality() {
        // Not equals
        assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));

        // Alternative not equals syntax
        assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));

        // With whitespace
        assert_eq!(parse_operator(" != "), Ok(("", Operator::NotEqual)));
        assert_eq!(parse_operator("  <>  "), Ok(("", Operator::NotEqual)));
        assert_eq!(parse_operator("\t!=\t"), Ok(("", Operator::NotEqual)));
    }

    #[test]
    fn test_parse_operator_bitwise_and() {
        // Bitwise AND
        assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));

        // With whitespace
        assert_eq!(parse_operator(" & "), Ok(("", Operator::BitwiseAnd)));
        assert_eq!(parse_operator("  &  "), Ok(("", Operator::BitwiseAnd)));
        assert_eq!(parse_operator("\t&\t"), Ok(("", Operator::BitwiseAnd)));
    }

    #[test]
    fn test_parse_operator_with_remaining_input() {
        // Should parse operator and leave remaining input
        assert_eq!(parse_operator("= 123"), Ok(("123", Operator::Equal)));
        assert_eq!(
            parse_operator("!= value"),
            Ok(("value", Operator::NotEqual))
        );
        assert_eq!(parse_operator("& 0xFF"), Ok(("0xFF", Operator::BitwiseAnd)));
        assert_eq!(
            parse_operator("== \"string\""),
            Ok(("\"string\"", Operator::Equal))
        );
        assert_eq!(parse_operator("<> test"), Ok(("test", Operator::NotEqual)));
    }

    #[test]
    fn test_parse_operator_precedence() {
        // Test that longer operators are matched first
        // This ensures "==" is parsed as Equal, not "=" followed by "="
        assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));
        assert_eq!(parse_operator("== extra"), Ok(("extra", Operator::Equal)));

        // Test that "!=" is parsed correctly, not as "!" followed by "="
        assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));
        assert_eq!(
            parse_operator("!= extra"),
            Ok(("extra", Operator::NotEqual))
        );

        // Test that "<>" is parsed correctly
        assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));
        assert_eq!(
            parse_operator("<> extra"),
            Ok(("extra", Operator::NotEqual))
        );
    }

    #[test]
    fn test_parse_operator_invalid_input() {
        // Should fail on invalid operators
        assert!(parse_operator("").is_err());
        assert!(parse_operator("abc").is_err());
        assert!(parse_operator("123").is_err());
        assert!(parse_operator(">").is_err());
        assert!(parse_operator("<").is_err());
        assert!(parse_operator("!").is_err());
        assert!(parse_operator("===").is_err()); // Too many equals
        assert!(parse_operator("&&").is_err()); // Double ampersand not supported
    }

    #[test]
    fn test_parse_operator_edge_cases() {
        // Test operators at start of various contexts - multispace0 consumes all whitespace
        assert_eq!(parse_operator("=\n"), Ok(("", Operator::Equal)));
        assert_eq!(parse_operator("!=\r\n"), Ok(("", Operator::NotEqual)));
        assert_eq!(parse_operator("&\t\t"), Ok(("", Operator::BitwiseAnd)));

        // Test with mixed whitespace
        assert_eq!(parse_operator(" \t = \t "), Ok(("", Operator::Equal)));
        assert_eq!(parse_operator("\t != \t"), Ok(("", Operator::NotEqual)));
        assert_eq!(parse_operator(" \t& \t "), Ok(("", Operator::BitwiseAnd)));
    }

    #[test]
    fn test_parse_operator_common_magic_file_patterns() {
        // Test patterns commonly found in magic files
        assert_eq!(
            parse_operator("= 0x7f454c46"),
            Ok(("0x7f454c46", Operator::Equal))
        );
        assert_eq!(parse_operator("!= 0"), Ok(("0", Operator::NotEqual)));
        assert_eq!(
            parse_operator("& 0xFF00"),
            Ok(("0xFF00", Operator::BitwiseAnd))
        );
        assert_eq!(
            parse_operator("== \"ELF\""),
            Ok(("\"ELF\"", Operator::Equal))
        );
        assert_eq!(parse_operator("<> \"\""), Ok(("\"\"", Operator::NotEqual)));

        // Test with various spacing patterns found in real magic files
        assert_eq!(
            parse_operator("=\t0x504b0304"),
            Ok(("0x504b0304", Operator::Equal))
        );
        assert_eq!(parse_operator("!=  0"), Ok(("0", Operator::NotEqual)));
        assert_eq!(
            parse_operator("&   0xFFFF"),
            Ok(("0xFFFF", Operator::BitwiseAnd))
        );
    }

    #[test]
    fn test_parse_operator_all_variants() {
        // Ensure all operator variants are tested
        let test_cases = [
            ("=", Operator::Equal),
            ("==", Operator::Equal),
            ("!=", Operator::NotEqual),
            ("<>", Operator::NotEqual),
            ("&", Operator::BitwiseAnd),
        ];

        for (input, expected) in test_cases {
            assert_eq!(
                parse_operator(input),
                Ok(("", expected)),
                "Failed to parse operator: '{input}'"
            );
        }
    }

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
        assert_eq!(parse_value("-0"), Ok(("", Value::Uint(0))));
        assert_eq!(parse_value("0x0"), Ok(("", Value::Uint(0))));
        assert_eq!(parse_value("-0x0"), Ok(("", Value::Uint(0))));

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

        // Empty hex bytes
        assert_eq!(parse_value(""), Ok(("", Value::Bytes(vec![]))));
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

    #[test]
    fn test_parse_operator_whitespace_handling() {
        // Test comprehensive whitespace handling
        let operators = ["=", "==", "!=", "<>", "&"];
        let whitespace_patterns = [
            "",     // No whitespace
            " ",    // Single space
            "  ",   // Multiple spaces
            "\t",   // Tab
            "\t\t", // Multiple tabs
            " \t",  // Mixed space and tab
            "\t ",  // Mixed tab and space
        ];

        for op in operators {
            for leading_ws in whitespace_patterns {
                for trailing_ws in whitespace_patterns {
                    let input = format!("{leading_ws}{op}{trailing_ws}");
                    let result = parse_operator(&input);

                    assert!(
                        result.is_ok(),
                        "Failed to parse operator with whitespace: '{input}'"
                    );

                    let (remaining, _) = result.unwrap();
                    assert_eq!(remaining, "", "Unexpected remaining input for: '{input}'");
                }
            }
        }
    }
}
