//! Grammar parsing for magic files using nom parser combinators
//!
//! This module implements the parsing logic for magic file syntax, converting
//! text-based magic rules into the AST representation defined in ast.rs.

use nom::{
    IResult, Parser,
    bytes::complete::tag,
    character::complete::{char, digit1, hex_digit1, multispace0},
    combinator::opt,
};

use crate::parser::ast::OffsetSpec;

/// Parse a decimal number
fn parse_decimal_number(input: &str) -> IResult<&str, i64> {
    let (input, digits) = digit1(input)?;
    let number = digits.parse::<i64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })?;
    Ok((input, number))
}

/// Parse a hexadecimal number (with 0x prefix)
fn parse_hex_number(input: &str) -> IResult<&str, i64> {
    let (input, _) = tag("0x")(input)?;
    let (input, hex_str) = hex_digit1(input)?;

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

    // Check if input starts with "0x" - if so, it must be a valid hex number
    let remaining_input = input;
    if remaining_input.starts_with("0x") {
        let (input, number) = parse_hex_number(input)?;
        let result = if sign.is_some() { -number } else { number };
        Ok((input, result))
    } else {
        let (input, number) = parse_decimal_number(input)?;
        let result = if sign.is_some() { -number } else { number };
        Ok((input, result))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
        // Should parse number and leave remaining input
        assert_eq!(parse_number("123abc"), Ok(("abc", 123)));
        assert_eq!(parse_number("0xFF rest"), Ok((" rest", 255)));
        assert_eq!(parse_number("-42 more"), Ok((" more", -42)));
        assert_eq!(parse_number("0x10,next"), Ok((",next", 16)));
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
}
