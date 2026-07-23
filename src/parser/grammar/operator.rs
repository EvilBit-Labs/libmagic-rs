// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Comparison operator parsing for magic file rules.
//!
//! Extracted from `grammar/mod.rs` as a pure code-motion split (issue #391
//! Unit U4) -- no behavior changes.

use nom::{IResult, character::complete::multispace0};

use crate::parser::ast::Operator;

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
/// ```ignore
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

    let bytes = input.as_bytes();
    let err = || nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag));

    // Dispatch on the first byte and inspect the second byte to choose between
    // long-form and short-form operators. Boundary checks reject invalid
    // sequences like "===", "&&", "^^", "~~", and "x42".
    let (op, consumed) = match bytes.first().copied() {
        Some(b'=') => {
            // "=" or "==" -- reject "===" (and longer runs of '=')
            if bytes.get(1).copied() == Some(b'=') {
                if bytes.get(2).copied() == Some(b'=') {
                    return Err(err());
                }
                (Operator::Equal, 2)
            } else {
                (Operator::Equal, 1)
            }
        }
        // "!=" or bare "!" -- both map to NotEqual. magic(5) uses the bare
        // form (e.g., `!0xb8c0078e` means "not equal to 0xb8c0078e"); the
        // `!=` form is accepted as a convenience and matches operators in
        // other parts of this parser.
        Some(b'!') => {
            if bytes.get(1).copied() == Some(b'=') {
                (Operator::NotEqual, 2)
            } else {
                (Operator::NotEqual, 1)
            }
        }
        Some(b'<') => {
            // "<=", "<>", or bare "<"
            match bytes.get(1).copied() {
                Some(b'=') => (Operator::LessEqual, 2),
                Some(b'>') => (Operator::NotEqual, 2),
                _ => (Operator::LessThan, 1),
            }
        }
        Some(b'>') => {
            // ">=" or bare ">"
            if bytes.get(1).copied() == Some(b'=') {
                (Operator::GreaterEqual, 2)
            } else {
                (Operator::GreaterThan, 1)
            }
        }
        Some(b'&') => {
            // Reject "&&"
            if bytes.get(1).copied() == Some(b'&') {
                return Err(err());
            }
            (Operator::BitwiseAnd, 1)
        }
        Some(b'^') => {
            // Reject "^^"
            if bytes.get(1).copied() == Some(b'^') {
                return Err(err());
            }
            (Operator::BitwiseXor, 1)
        }
        Some(b'~') => {
            // Reject "~~"
            if bytes.get(1).copied() == Some(b'~') {
                return Err(err());
            }
            (Operator::BitwiseNot, 1)
        }
        Some(b'x') => {
            // Word boundary: 'x' must not be followed by an alphanumeric or '_'
            // (e.g., "x42" or "xfoo" is not AnyValue).
            if input
                .get(1..)
                .is_some_and(|s| s.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
            {
                return Err(err());
            }
            (Operator::AnyValue, 1)
        }
        _ => return Err(err()),
    };

    let remaining = &input[consumed..];
    let (remaining, _) = multispace0(remaining)?;
    Ok((remaining, op))
}
