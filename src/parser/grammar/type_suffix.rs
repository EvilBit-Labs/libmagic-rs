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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Operator, RegexCount, RegexFlags};

    // ---------- parse_attached_operator ----------

    #[test]
    fn test_parse_attached_operator_no_ampersand_returns_none() {
        // No `&` prefix: no operator, input passed through unchanged.
        let (rest, op) = parse_attached_operator("=value").expect("should parse");
        assert_eq!(rest, "=value");
        assert_eq!(op, None);
    }

    #[test]
    fn test_parse_attached_operator_bare_ampersand_returns_bitwise_and() {
        // Bare `&` (not followed by a digit or another `&`): the
        // standalone bitwise-AND operator. The caller typically
        // pairs it with a separate numeric value.
        let (rest, op) = parse_attached_operator("& ").expect("should parse");
        assert_eq!(rest, " ");
        assert_eq!(op, Some(Operator::BitwiseAnd));
    }

    #[test]
    fn test_parse_attached_operator_decimal_mask_returns_mask() {
        let (rest, op) = parse_attached_operator("&255 trailing").expect("should parse");
        assert_eq!(rest, " trailing");
        assert_eq!(op, Some(Operator::BitwiseAndMask(255)));
    }

    #[test]
    fn test_parse_attached_operator_hex_mask_returns_mask() {
        let (rest, op) = parse_attached_operator("&0xf0000000").expect("should parse");
        assert_eq!(rest, "");
        assert_eq!(op, Some(Operator::BitwiseAndMask(0xf000_0000)));
    }

    #[test]
    fn test_parse_attached_operator_full_u64_mask() {
        // Verify the unsigned-number parser handles a full 64-bit mask.
        let (rest, op) = parse_attached_operator("&0xffffffffffffffff").expect("should parse");
        assert_eq!(rest, "");
        assert_eq!(op, Some(Operator::BitwiseAndMask(u64::MAX)));
    }

    #[test]
    fn test_parse_attached_operator_double_ampersand_is_hard_error() {
        // `&&` is not a valid operator -- hard parse error.
        let result = parse_attached_operator("&&");
        assert!(result.is_err(), "&& must be rejected at parse time");
    }

    // ---------- parse_regex_suffix ----------

    #[test]
    fn test_parse_regex_suffix_flags_only() {
        // `c` alone -> case-insensitive flag, Default count.
        let (rest, (flags, count)) = parse_regex_suffix("regex/c", "c").expect("c flag");
        assert_eq!(rest, "");
        assert!(flags.case_insensitive);
        assert!(!flags.start_offset);
        assert_eq!(count, RegexCount::Default);
    }

    #[test]
    fn test_parse_regex_suffix_interleaved_flag_and_count() {
        // `c1l` -> case_insensitive + Lines(Some(1)) (digit after
        // flag letter, followed by another flag letter).
        let (rest, (flags, count)) = parse_regex_suffix("regex/c1l", "c1l").expect("c1l");
        assert_eq!(rest, "");
        assert!(flags.case_insensitive);
        assert_eq!(count, RegexCount::Lines(::std::num::NonZeroU32::new(1)));
    }

    #[test]
    fn test_parse_regex_suffix_bytes_only() {
        // `100` alone -> RegexCount::Bytes(100), no flags.
        let (rest, (flags, count)) = parse_regex_suffix("regex/100", "100").expect("100");
        assert_eq!(rest, "");
        assert_eq!(flags, RegexFlags::default());
        assert_eq!(
            count,
            RegexCount::Bytes(::std::num::NonZeroU32::new(100).unwrap())
        );
    }

    #[test]
    fn test_parse_regex_suffix_lines_none_shorthand() {
        // Bare `l` alone -> Lines(None).
        let (rest, (_flags, count)) = parse_regex_suffix("regex/l", "l").expect("l");
        assert_eq!(rest, "");
        assert_eq!(count, RegexCount::Lines(None));
    }

    #[test]
    fn test_parse_regex_suffix_duplicate_count_rejected() {
        // `1l2` -> second count triggers a hard parse error.
        let result = parse_regex_suffix("regex/1l2", "1l2");
        assert!(result.is_err(), "duplicate count must be rejected");
    }

    #[test]
    fn test_parse_regex_suffix_zero_count_rejected() {
        // `0` -> NonZeroU32::new(0) fails, hard parse error.
        let result = parse_regex_suffix("regex/0", "0");
        assert!(result.is_err(), "zero count must be rejected");
    }

    #[test]
    fn test_parse_regex_suffix_bare_slash_rejected() {
        // Empty suffix (no modifier at all) -> hard parse error.
        let result = parse_regex_suffix("regex/", "");
        assert!(
            result.is_err(),
            "bare regex/ with no modifier must be rejected"
        );
    }

    #[test]
    fn test_parse_regex_suffix_stops_at_operator_boundary() {
        // `c=` -> `c` is the flag, `=` is left for parse_operator.
        let (rest, (flags, count)) = parse_regex_suffix("regex/c=foo", "c=foo").expect("c=");
        assert_eq!(rest, "=foo", "should leave = for parse_operator");
        assert!(flags.case_insensitive);
        assert_eq!(count, RegexCount::Default);
    }

    // ---------- parse_search_suffix ----------

    #[test]
    fn test_parse_search_suffix_decimal_range() {
        let (rest, range) = parse_search_suffix("search/256", "256").expect("256");
        assert_eq!(rest, "");
        assert_eq!(range, NonZeroUsize::new(256).unwrap());
    }

    #[test]
    fn test_parse_search_suffix_zero_rejected() {
        let result = parse_search_suffix("search/0", "0");
        assert!(result.is_err(), "search/0 must be rejected");
    }

    #[test]
    fn test_parse_search_suffix_empty_rejected() {
        let result = parse_search_suffix("search/", "");
        assert!(result.is_err(), "search/ with empty range must be rejected");
    }

    #[test]
    fn test_parse_search_suffix_leaves_trailing_space() {
        let (rest, range) = parse_search_suffix("search/256 rest", "256 rest").expect("256 rest");
        assert_eq!(rest, " rest");
        assert_eq!(range.get(), 256);
    }

    // ---------- parse_pstring_suffix ----------

    #[test]
    fn test_parse_pstring_suffix_width_letters() {
        use crate::parser::ast::PStringLengthWidth;
        assert_eq!(
            parse_pstring_suffix("B").unwrap(),
            ("", PStringLengthWidth::OneByte, false)
        );
        assert_eq!(
            parse_pstring_suffix("H").unwrap(),
            ("", PStringLengthWidth::TwoByteBE, false)
        );
        assert_eq!(
            parse_pstring_suffix("h").unwrap(),
            ("", PStringLengthWidth::TwoByteLE, false)
        );
        assert_eq!(
            parse_pstring_suffix("L").unwrap(),
            ("", PStringLengthWidth::FourByteBE, false)
        );
        assert_eq!(
            parse_pstring_suffix("l").unwrap(),
            ("", PStringLengthWidth::FourByteLE, false)
        );
    }

    #[test]
    fn test_parse_pstring_suffix_width_plus_j_flag() {
        use crate::parser::ast::PStringLengthWidth;
        assert_eq!(
            parse_pstring_suffix("HJ").unwrap(),
            ("", PStringLengthWidth::TwoByteBE, true)
        );
    }

    #[test]
    fn test_parse_pstring_suffix_bare_j_is_one_byte_includes_itself() {
        use crate::parser::ast::PStringLengthWidth;
        assert_eq!(
            parse_pstring_suffix("J").unwrap(),
            ("", PStringLengthWidth::OneByte, true)
        );
    }

    #[test]
    fn test_parse_pstring_suffix_unknown_letter_rejected() {
        assert!(parse_pstring_suffix("Z").is_err());
    }
}
