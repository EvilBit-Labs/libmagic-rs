// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Line-level parsing helpers for magic files.
//!
//! Covers the rule message parse, the `!:strength` directive, and the
//! comment/blank-line/continuation predicates used by the preprocessing
//! pipeline. Extracted from `grammar/mod.rs` as a pure code-motion split
//! (issue #391 Unit U4) -- no behavior changes.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, multispace0},
    sequence::preceded,
};

use crate::parser::ast::StrengthModifier;

use super::numbers::parse_decimal_number;
use super::parse_number;

/// Parse the message part of a magic rule
///
/// The message is everything after the value until the end of the line.
/// It may contain format specifiers and can be empty.
///
/// # Examples
///
/// ```ignore
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
/// ```ignore
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
    // Clamping to `[i32::MIN, i32::MAX]` is lossless via `as i32`, so no
    // `unwrap()`/`expect()` is needed (AGENTS.md bans panic markers in
    // library code regardless of whether the unwrap is provably safe).
    #[allow(clippy::cast_possible_truncation)]
    fn clamp_to_i32(n: i64) -> i32 {
        n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    let (input, _) = multispace0(input)?;
    let (input, _) = tag("!:strength")(input)?;
    let (input, _) = multispace0(input)?;

    // Parse the operator: +, -, *, /, = or bare number (implies =).
    // Optional whitespace is permitted between the operator character and
    // the operand to match GNU `file` magic(5) parsers, which accept forms
    // like `!:strength / 2` in real-world magic files (e.g., the Minix
    // entries in /usr/share/file/magic/filesystems).
    //
    // Use `preceded` + `Parser::map` (nom 8 idiom) rather than
    // `map(pair(..), |(_, n)| ..)` so the throwaway `_` goes away and
    // the composition matches the rest of this file's `.parse(input)?`
    // style. Tuples implement `Parser` in nom 8 and are used as the first
    // element of `preceded` to consume the operator char plus any trailing
    // whitespace.
    let (input, modifier) = alt((
        // +N -> Add
        preceded((char('+'), multispace0), parse_number)
            .map(|n| StrengthModifier::Add(clamp_to_i32(n))),
        // -N -> Subtract (parse_number handles negative directly; we
        // need parse_decimal_number after the explicit `-` consumer
        // so the sign is applied exactly once).
        preceded((char('-'), multispace0), parse_decimal_number)
            .map(|n| StrengthModifier::Subtract(clamp_to_i32(n))),
        // *N -> Multiply
        preceded((char('*'), multispace0), parse_number)
            .map(|n| StrengthModifier::Multiply(clamp_to_i32(n))),
        // /N -> Divide
        preceded((char('/'), multispace0), parse_number)
            .map(|n| StrengthModifier::Divide(clamp_to_i32(n))),
        // =N -> Set
        preceded((char('='), multispace0), parse_number)
            .map(|n| StrengthModifier::Set(clamp_to_i32(n))),
        // Bare number -> Set
        parse_number.map(|n| StrengthModifier::Set(clamp_to_i32(n))),
    ))
    .parse(input)?;

    Ok((input, modifier))
}

/// Check if a line is a strength directive (starts with !:strength)
///
/// # Examples
///
/// ```ignore
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

/// Parse a comment line (starts with #)
///
/// Comments in magic files start with '#' and continue to the end of the line.
/// This function consumes the entire comment line.
///
/// # Examples
///
/// ```ignore
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
/// ```ignore
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
/// ```ignore
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
/// ```ignore
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
