// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Primitive number parsers for the magic-file grammar.
//!
//! Provides the decimal/hex/signed/unsigned `i64` and `u64` parsers shared by
//! offset and value parsing. Extracted from `grammar/mod.rs` to keep that
//! module under the project's file-size limit.

use nom::{
    IResult, Parser,
    bytes::complete::tag,
    character::complete::{char, digit1, hex_digit1},
    combinator::opt,
};

/// Parse a decimal number with overflow protection
pub(super) fn parse_decimal_number(input: &str) -> IResult<&str, i64> {
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
pub(super) fn parse_unsigned_decimal_number(input: &str) -> IResult<&str, u64> {
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
pub(super) fn parse_hex_number(input: &str) -> IResult<&str, i64> {
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
pub(super) fn parse_unsigned_hex_number(input: &str) -> IResult<&str, u64> {
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
pub(super) fn parse_unsigned_number(input: &str) -> IResult<&str, u64> {
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
/// ```ignore
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
