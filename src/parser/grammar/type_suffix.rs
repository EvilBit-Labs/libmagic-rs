// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type-suffix parsers for the magic-file grammar.
//!
//! This submodule holds the suffix parsers extracted from
//! `parse_type_and_operator` so the top-level grammar module stays
//! under the 800-line size cap documented in AGENTS.md.
//!
//! Each parser takes the "rest of line after the `/`" and returns the
//! parsed data plus any remaining input. The top-level
//! `parse_type_and_operator` orchestrates these helpers after
//! `parse_type_keyword` identifies the type name.

use nom::error::{Error, ErrorKind};
use nom::{Err as NomErr, IResult};
use std::num::{NonZeroU32, NonZeroUsize};

use super::numbers::{parse_decimal_number, parse_unsigned_number};
use crate::parser::ast::{Operator, PStringLengthWidth, RegexCount, RegexFlags};

/// Parse a `pstring` suffix `/[BHhLl][J]?` or bare `/J`.
///
/// Returns the remaining input, the chosen `PStringLengthWidth`, and
/// whether the `/J` "length includes prefix width" flag was set.
///
/// # Errors
///
/// Returns a nom parse error if the character following `/` is not one
/// of the recognized width letters (`B`, `H`, `h`, `L`, `l`) or `J`.
pub(super) fn parse_pstring_suffix(
    input: &str,
) -> Result<(&str, PStringLengthWidth, bool), NomErr<Error<&str>>> {
    // Parse width character
    let (rest, width) = if let Some(rest) = input.strip_prefix('B') {
        (rest, PStringLengthWidth::OneByte)
    } else if let Some(rest) = input.strip_prefix('H') {
        (rest, PStringLengthWidth::TwoByteBE)
    } else if let Some(rest) = input.strip_prefix('h') {
        (rest, PStringLengthWidth::TwoByteLE)
    } else if let Some(rest) = input.strip_prefix('L') {
        (rest, PStringLengthWidth::FourByteBE)
    } else if let Some(rest) = input.strip_prefix('l') {
        (rest, PStringLengthWidth::FourByteLE)
    } else if let Some(rest) = input.strip_prefix('J') {
        // Bare /J with no width = default OneByte + self-inclusive
        return Ok((rest, PStringLengthWidth::OneByte, true));
    } else {
        // Unrecognized suffix character after '/'
        return Err(NomErr::Error(Error::new(input, ErrorKind::OneOf)));
    };

    // Parse optional J flag after width character
    let (rest, includes_j) = if let Some(rest) = rest.strip_prefix('J') {
        (rest, true)
    } else {
        (rest, false)
    };

    Ok((rest, width, includes_j))
}

/// Parse a `regex` suffix made up of `/c`, `/s`, `/l` flag letters and
/// an optional decimal count, interleaved in any order.
///
/// Accepts the modifier sequence that follows the `/` on a `regex/...`
/// rule. Flag letters and a decimal count can appear in any order with
/// these rules:
///
/// - `c` sets [`RegexFlags::case_insensitive`], `s` sets
///   [`RegexFlags::start_offset`], `l` marks the scan window as
///   line-based (this is collapsed into the [`RegexCount::Lines`]
///   variant at the bottom of this function, not stored as a flag
///   field).
/// - A digit sequence is parsed as a `NonZeroU32` count. A second
///   digit sequence is a hard parse error (libmagic accepts duplicates
///   with a `"multiple ranges"` stderr warning; we prefer failing fast
///   so magic-file bugs surface at parse time).
/// - Scanning stops at whitespace or at an operator boundary character
///   (`=`, `!`, `<`, `>`, `&`, `^`, `~`, `x`) so forms like `regex/c=`
///   leave the operator for `parse_operator` to handle.
/// - A bare `regex/` with no modifier is a parse error.
/// - A zero count (`regex/0`) is a parse error because `NonZeroU32`
///   makes it unrepresentable.
///
/// Returns the remaining input (outer slice, so the caller can
/// propagate error offsets), the parsed [`RegexFlags`], and the
/// collapsed [`RegexCount`] variant.
///
/// # Arguments
///
/// * `input` - The full parser input *before* the `/`. Error messages
///   are reported against this slice.
/// * `suffix_rest` - The slice after consuming the leading `/`, i.e.,
///   the modifier characters themselves.
///
/// # Errors
///
/// See the rules list above.
pub(super) fn parse_regex_suffix<'a>(
    input: &'a str,
    suffix_rest: &'a str,
) -> IResult<&'a str, (RegexFlags, RegexCount)> {
    let mut flags = RegexFlags::default();
    let mut count_value: Option<NonZeroU32> = None;
    let mut line_based = false;
    let mut any_modifier = false;

    let mut rest = suffix_rest;

    loop {
        if let Some(next) = rest.strip_prefix('c') {
            flags.case_insensitive = true;
            rest = next;
            any_modifier = true;
        } else if let Some(next) = rest.strip_prefix('s') {
            flags.start_offset = true;
            rest = next;
            any_modifier = true;
        } else if let Some(next) = rest.strip_prefix('l') {
            line_based = true;
            rest = next;
            any_modifier = true;
        } else if rest.starts_with(|c: char| c.is_ascii_digit()) {
            // Reject a second numeric count: libmagic accepts it with
            // a "multiple ranges" warning but we prefer a hard error
            // so magic-file bugs surface at parse time.
            if count_value.is_some() {
                return Err(NomErr::Error(Error::new(input, ErrorKind::Digit)));
            }
            let (after_number, n) = parse_decimal_number(rest)
                .map_err(|_| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;
            // Reject zero and overflow with a clear parse error.
            let parsed = u32::try_from(n)
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or_else(|| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;
            count_value = Some(parsed);
            rest = after_number;
            any_modifier = true;
        } else {
            match rest.chars().next() {
                Some(c) if c.is_whitespace() => break,
                None | Some('=' | '!' | '<' | '>' | '&' | '^' | '~' | 'x') => break,
                Some(_) => {
                    return Err(NomErr::Error(Error::new(input, ErrorKind::Tag)));
                }
            }
        }
    }

    if !any_modifier {
        return Err(NomErr::Error(Error::new(input, ErrorKind::Tag)));
    }

    // Collapse the (line_based, count) local pair into the RegexCount variant.
    let count = if line_based {
        RegexCount::Lines(count_value)
    } else if let Some(n) = count_value {
        RegexCount::Bytes(n)
    } else {
        RegexCount::Default
    };

    Ok((rest, (flags, count)))
}

/// Parse an optional attached bitwise operator after a type specifier
/// (e.g., `lelong&0xf0000000` or bare `lelong&`).
///
/// Recognizes:
///
/// - `&<number>` or `&0x<hex>` -> `Operator::BitwiseAndMask(mask)`
/// - bare `&` (not followed by a digit or `&`) -> `Operator::BitwiseAnd`
/// - `&&` -> parse error (not a valid operator)
/// - no `&` -> `None`
///
/// Uses unsigned parsing so full `u64` masks (e.g., `0xffffffffffffffff`)
/// are supported. If `&` is followed by digits/`0x` but the mask parse
/// fails (overflow, etc.), returns a hard error rather than silently
/// falling back to standalone `&`.
///
/// # Arguments
///
/// * `input` - The full parser input *before* any `&`; used for error
///   positioning on failure.
///
/// # Errors
///
/// Returns a nom parse error on `&&` or a malformed mask literal.
pub(super) fn parse_attached_operator(input: &str) -> IResult<&str, Option<Operator>> {
    if let Some(after_amp) = input.strip_prefix('&') {
        if after_amp.starts_with("0x") || after_amp.starts_with(|c: char| c.is_ascii_digit()) {
            // '&' followed by what looks like a number -- must parse as mask
            let (rest, mask) = parse_unsigned_number(after_amp)
                .map_err(|_| NomErr::Error(Error::new(input, ErrorKind::MapRes)))?;
            Ok((rest, Some(Operator::BitwiseAndMask(mask))))
        } else if after_amp.starts_with('&') {
            // Reject '&&' -- not valid operator syntax
            Err(NomErr::Error(Error::new(input, ErrorKind::Tag)))
        } else {
            // Standalone '&' (no digits following)
            Ok((after_amp, Some(Operator::BitwiseAnd)))
        }
    } else {
        Ok((input, None))
    }
}

/// Parse a `search` suffix `/N` where `N` is a non-zero decimal count.
///
/// Per GNU `file` magic(5), the range is mandatory; bare `search` and
/// `search/0` are parse errors, enforced here via `NonZeroUsize`.
///
/// # Arguments
///
/// * `input` - The full parser input *before* the `/`; used for error
///   positioning.
/// * `suffix_rest` - The slice after consuming the leading `/`, i.e.,
///   the decimal count itself.
///
/// # Errors
///
/// Returns a nom parse error if the count is missing, non-numeric,
/// zero, or overflows `usize`.
pub(super) fn parse_search_suffix<'a>(
    input: &'a str,
    suffix_rest: &'a str,
) -> IResult<&'a str, NonZeroUsize> {
    let (rest, n) = parse_decimal_number(suffix_rest)
        .map_err(|_| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;
    let range = usize::try_from(n)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;
    Ok((rest, range))
}
