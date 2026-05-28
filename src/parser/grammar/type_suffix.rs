// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type-suffix parsers for the magic-file grammar.
//!
//! This submodule holds the suffix parsers extracted from
//! `parse_type_and_operator` so the top-level grammar module stays
//! under the 500-600 line size target documented in repository coding guidelines.
//!
//! Each parser takes the "rest of line after the `/`" and returns the
//! parsed data plus any remaining input. The top-level
//! `parse_type_and_operator` orchestrates these helpers after
//! `parse_type_keyword` identifies the type name.

use nom::error::{Error, ErrorKind};
use nom::{Err as NomErr, IResult};
use std::num::{NonZeroU32, NonZeroUsize};

use super::numbers::{parse_decimal_number, parse_number, parse_unsigned_number};
use crate::parser::ast::{
    Operator, PStringLengthWidth, RegexCount, RegexFlags, SearchFlags, ValueTransform,
    ValueTransformOp,
};

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

/// Parse an optional pre-comparison value transform after a type specifier.
///
/// magic(5) allows `+`, `-`, `*`, `/`, `%`, `|`, and `^` between the type
/// keyword and the comparison value. The transform is applied to the value
/// read from the file before the rule's comparison operator runs (and
/// before printf-style format substitution, so `%d` reflects the
/// post-transform number).
///
/// Bitwise AND (`&MASK`) is handled separately by [`parse_attached_operator`]
/// because it predates this enum and is encoded at the operator layer via
/// [`Operator::BitwiseAndMask`].
///
/// Returns `Ok((rest, None))` when the next character is not one of the
/// recognized transform operators -- the caller continues parsing the rest
/// of the rule normally.
///
/// # Examples
///
/// - `+1` -> `Some(ValueTransform { Add, 1 })`
/// - `-3` -> `Some(ValueTransform { Sub, 3 })`
/// - `*2` -> `Some(ValueTransform { Mul, 2 })`
/// - `/1073741824` -> `Some(ValueTransform { Div, 1073741824 })`
/// - bare `=foo` -> `None` (no transform; caller handles the operator)
///
/// # Errors
///
/// Returns a nom parse error if a recognized transform operator is followed
/// by something that does not parse as a signed number (e.g., `+abc`).
pub(super) fn parse_value_transform(input: &str) -> IResult<&str, Option<ValueTransform>> {
    // Single-byte lookahead to dispatch on the operator. We deliberately
    // do not consume `&` here -- `parse_attached_operator` handles that
    // case via `Operator::BitwiseAndMask`. We also reject `--` and
    // `++` etc. by parsing exactly one operator byte, then a single
    // signed number.
    let bytes = input.as_bytes();
    let op = match bytes.first().copied() {
        Some(b'+') => ValueTransformOp::Add,
        Some(b'-') => {
            // `-` must be followed by a digit/0x to count as a transform.
            // A bare `-` is not a transform; let the caller handle it
            // (no other parser path uses bare `-` at this position, but
            // this guard keeps the grammar future-proof).
            if !matches!(bytes.get(1).copied(), Some(c) if c.is_ascii_digit()) {
                return Ok((input, None));
            }
            ValueTransformOp::Sub
        }
        Some(b'*') => ValueTransformOp::Mul,
        Some(b'/') => ValueTransformOp::Div,
        Some(b'%') => ValueTransformOp::Mod,
        Some(b'|') => ValueTransformOp::Or,
        Some(b'^') => {
            // Reject `^^` (defensive; matches the `^^` rejection in
            // parse_operator).
            if matches!(bytes.get(1).copied(), Some(b'^')) {
                return Ok((input, None));
            }
            ValueTransformOp::Xor
        }
        _ => return Ok((input, None)),
    };

    // Consume the operator byte and parse the operand. `parse_number`
    // accepts decimal and hex (`0x...`) plus signs, so `+0xff` and
    // `*0x10` work alongside the common decimal forms. The op byte
    // already encoded the sign for `Sub`, but `parse_number` is fine
    // with a leading digit -- it does not require a sign character.
    let after_op = &input[1..];
    let (rest, operand) =
        parse_number(after_op).map_err(|_| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;

    Ok((rest, Some(ValueTransform { op, operand })))
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

/// Parse a `search` suffix `/N[/<flags>]` where `N` is a non-zero count.
///
/// Per GNU `file` magic(5), the range is mandatory; bare `search` and
/// `search/0` are parse errors, enforced here via `NonZeroUsize`. The
/// range accepts both decimal (`search/256`) and hexadecimal
/// (`search/0xffff`, `search/0x93e4f`) literals -- many real magic files
/// use hex for large search windows (e.g., archive:254 scans up to
/// 0x93e4f bytes for the third tar archive in a Debian package).
///
/// magic(5) also allows trailing flag letters (`/w` whitespace-optional,
/// `/b` blank-handling / binary-hint, `/B` binary-hint, `/W` compact-
/// whitespace, `/c`/`/C` case-insensitive lower/upper, `/t` text-hint,
/// `/T` trim, `/s` search-start anchor). Each letter is recorded on the
/// returned [`SearchFlags`] so the evaluator can dispatch on scan and
/// anchor-advance semantics. Duplicate letters are idempotent -- setting
/// the same field twice has no extra side effect, matching libmagic's
/// per-letter `STRING_*` bitfield accumulation.
///
/// Trailing non-operator characters after the count and flags are
/// rejected as a hard parse error so that `search/256foo` fails at parse
/// time instead of being silently re-interpreted as `search/256`
/// followed by a value string `foo`.
///
/// # Arguments
///
/// * `input` - The full parser input *before* the `/`; used for error
///   positioning.
/// * `suffix_rest` - The slice after consuming the leading `/`, i.e.,
///   the count itself.
///
/// # Errors
///
/// Returns a nom parse error if the count is missing, non-numeric,
/// zero, overflows `usize`, or is followed by a non-operator character.
pub(super) fn parse_search_suffix<'a>(
    input: &'a str,
    suffix_rest: &'a str,
) -> IResult<&'a str, (NonZeroUsize, SearchFlags)> {
    let (mut rest, n) = parse_unsigned_number(suffix_rest)
        .map_err(|_| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;

    // Optional `/<flags>` after the count (e.g., `search/256/w`,
    // `search/4261301/s`). magic(5) flag letters are the same set as
    // for `string` plus the search-only `/s` "search-start" anchor.
    let mut flags = SearchFlags::default();
    if let Some(after_slash) = rest.strip_prefix('/') {
        let mut consumed = 0usize;
        for ch in after_slash.chars() {
            match ch {
                'W' => flags.compact_whitespace = true,
                'w' => flags.compact_optional_whitespace = true,
                'c' => flags.ignore_lowercase = true,
                'C' => flags.ignore_uppercase = true,
                'T' => flags.trim = true,
                't' => flags.text_test = true,
                // `/b` and `/B` share the binary-hint semantics for
                // `search`. (`/B` on `pstring` is the 1-byte length-
                // width letter; that is a separate dispatch path keyed
                // on the type keyword.)
                'B' | 'b' => flags.bin_test = true,
                's' => flags.start_anchor = true,
                'f' => flags.full_word = true,
                _ => break,
            }
            consumed += ch.len_utf8();
        }
        if consumed > 0 {
            rest = &after_slash[consumed..];
        } else {
            // `/` not followed by a known flag letter is a parse error
            // (matches the strictness of the trailing-junk check
            // below). Leave the error position pointed at `input` so
            // the caller's diagnostics are meaningful.
            return Err(NomErr::Error(Error::new(input, ErrorKind::Tag)));
        }
    }

    // Reject trailing junk so `search/256foo` and `search/256/sfoo`
    // fail hard instead of silently becoming `search/256` + value
    // string `foo`. Same operator-boundary set as parse_regex_suffix.
    match rest.chars().next() {
        Some(c) if c.is_whitespace() => {}
        None | Some('=' | '!' | '<' | '>' | '&' | '^' | '~' | 'x') => {}
        Some(_) => {
            return Err(NomErr::Error(Error::new(input, ErrorKind::Tag)));
        }
    }
    let range = usize::try_from(n)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| NomErr::Error(Error::new(input, ErrorKind::Digit)))?;
    Ok((rest, (range, flags)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Operator, RegexCount, RegexFlags, SearchFlags};

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
        let (rest, (range, flags)) = parse_search_suffix("search/256", "256").expect("256");
        assert_eq!(rest, "");
        assert_eq!(range, NonZeroUsize::new(256).unwrap());
        assert_eq!(flags, SearchFlags::default());
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
        let (rest, (range, flags)) =
            parse_search_suffix("search/256 rest", "256 rest").expect("256 rest");
        assert_eq!(rest, " rest");
        assert_eq!(range.get(), 256);
        assert_eq!(flags, SearchFlags::default());
    }

    /// Regression test for PR #215 review finding (CodeRabbit): trailing
    /// junk after a `search/N` range must be a hard parse error rather
    /// than silently re-interpreted as `search/N` followed by a value
    /// string. Without the trailing-junk check, `search/256foo` parses
    /// as `search/256` with remainder `foo`, which is then handed to
    /// the value parser and produces a valid-but-wrong rule.
    #[test]
    fn test_parse_search_suffix_trailing_junk_rejected() {
        let result = parse_search_suffix("search/256foo", "256foo");
        assert!(
            result.is_err(),
            "search/256foo must be rejected (trailing non-operator junk after the range)"
        );
    }

    /// Confirm that the operator-boundary characters are still accepted
    /// after the range so forms like `search/256=value` continue to
    /// work. `=` here is consumed by `parse_operator` in the grammar
    /// layer, not by `parse_search_suffix`.
    #[test]
    fn test_parse_search_suffix_operator_boundary_allowed() {
        for boundary in ['=', '!', '<', '>', '&', '^', '~', 'x'] {
            let suffix = format!("256{boundary}value");
            let input = format!("search/{suffix}");
            let (rest, (range, flags)) = parse_search_suffix(&input, &suffix)
                .unwrap_or_else(|_| panic!("boundary char '{boundary}' should be allowed"));
            assert_eq!(rest, format!("{boundary}value"));
            assert_eq!(range.get(), 256);
            assert_eq!(flags, SearchFlags::default());
        }
    }

    // ----- SearchFlags per-letter assignment (issue #235) -----

    #[test]
    fn test_parse_search_suffix_flag_s_sets_start_anchor() {
        // `/s` is the search-start anchor flag, the load-bearing flag
        // motivating this work (TGA footer, sfnt name table, etc.).
        let (rest, (range, flags)) =
            parse_search_suffix("search/256/s", "256/s").expect("search/256/s");
        assert_eq!(rest, "");
        assert_eq!(range.get(), 256);
        assert_eq!(
            flags,
            SearchFlags {
                start_anchor: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_lowercase_c_sets_ignore_lowercase() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/c", "256/c").expect("search/256/c");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                ignore_lowercase: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_uppercase_c_sets_ignore_uppercase() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/C", "256/C").expect("search/256/C");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                ignore_uppercase: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_lowercase_w_sets_compact_optional_whitespace() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/w", "256/w").expect("search/256/w");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                compact_optional_whitespace: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_uppercase_w_sets_compact_whitespace() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/W", "256/W").expect("search/256/W");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                compact_whitespace: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_lowercase_t_sets_text_test() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/t", "256/t").expect("search/256/t");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                text_test: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_uppercase_t_sets_trim() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/T", "256/T").expect("search/256/T");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                trim: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_lowercase_b_sets_bin_test() {
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/b", "256/b").expect("search/256/b");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                bin_test: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_uppercase_b_sets_bin_test() {
        // `/B` shares the binary-hint semantics with `/b` for search.
        // (`/B` on pstring is the 1-byte length-width letter — a separate
        // dispatch path on the type keyword.)
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/B", "256/B").expect("search/256/B");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                bin_test: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_combination_cs() {
        // Both letters set their fields; order does not matter.
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/cs", "256/cs").expect("search/256/cs");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                ignore_lowercase: true,
                start_anchor: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_flag_combination_s_w_c_t_order_agnostic() {
        // Four flags across all letter cases; order-agnostic accumulation.
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/sWcT", "256/sWcT").expect("search/256/sWcT");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                start_anchor: true,
                compact_whitespace: true,
                ignore_lowercase: true,
                trim: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_duplicate_letter_is_idempotent() {
        // `cc` sets `ignore_lowercase` twice with no side effect, matching
        // libmagic's per-letter STRING_* bitfield accumulation.
        let (rest, (_range, flags)) =
            parse_search_suffix("search/256/cc", "256/cc").expect("search/256/cc");
        assert_eq!(rest, "");
        assert_eq!(
            flags,
            SearchFlags {
                ignore_lowercase: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_f_sets_full_word() {
        // `/f` is STRING_FULL_WORD — post-match word-boundary check.
        let (rest, (range, flags)) =
            parse_search_suffix("search/256/f", "256/f").expect("search/256/f");
        assert_eq!(rest, "");
        assert_eq!(range.get(), 256);
        assert_eq!(
            flags,
            SearchFlags {
                full_word: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_search_suffix_unknown_flag_letter_rejected_via_trailing_junk() {
        // `z` is not a search flag letter. The flag loop stops at `z`,
        // leaving `zoo` as remainder; the trailing-junk gate then rejects.
        let result = parse_search_suffix("search/256/szoo", "256/szoo");
        assert!(
            result.is_err(),
            "unknown flag letter must be rejected by the trailing-junk gate"
        );
    }

    #[test]
    fn test_parse_search_suffix_flags_then_operator_boundary() {
        // After consuming `/s`, the `=value` remainder is left for
        // `parse_operator` to handle.
        let (rest, (range, flags)) =
            parse_search_suffix("search/256/s=value", "256/s=value").expect("search/256/s=value");
        assert_eq!(rest, "=value");
        assert_eq!(range.get(), 256);
        assert_eq!(
            flags,
            SearchFlags {
                start_anchor: true,
                ..Default::default()
            }
        );
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

    // ========================================================================
    // parse_value_transform tests
    // ========================================================================

    #[test]
    fn test_parse_value_transform_add() {
        let (rest, t) = parse_value_transform("+1 rest").unwrap();
        assert_eq!(rest, " rest");
        assert_eq!(
            t,
            Some(ValueTransform {
                op: ValueTransformOp::Add,
                operand: 1,
            })
        );
    }

    #[test]
    fn test_parse_value_transform_div_with_decimal() {
        // Regression: filesystems uses `ulequad/1073741824` to convert
        // sectors to GiB.
        let (rest, t) = parse_value_transform("/1073741824 rest").unwrap();
        assert_eq!(rest, " rest");
        assert_eq!(
            t,
            Some(ValueTransform {
                op: ValueTransformOp::Div,
                operand: 1_073_741_824,
            })
        );
    }

    #[test]
    fn test_parse_value_transform_all_ops() {
        let cases: &[(&str, ValueTransformOp, i64)] = &[
            ("+5", ValueTransformOp::Add, 5),
            ("-3", ValueTransformOp::Sub, 3),
            ("*7", ValueTransformOp::Mul, 7),
            ("/2", ValueTransformOp::Div, 2),
            ("%4", ValueTransformOp::Mod, 4),
            ("|0xff", ValueTransformOp::Or, 0xff),
            ("^0x80", ValueTransformOp::Xor, 0x80),
        ];
        for (input, expected_op, expected_value) in cases {
            let (_, parsed) =
                parse_value_transform(input).unwrap_or_else(|_| panic!("Failed to parse {input}"));
            let t = parsed.unwrap_or_else(|| panic!("No transform parsed for {input}"));
            assert_eq!(t.op, *expected_op, "wrong op for {input}");
            assert_eq!(t.operand, *expected_value, "wrong operand for {input}");
        }
    }

    #[test]
    fn test_parse_value_transform_no_match_returns_none() {
        // Bare `=` is not a value transform -- caller handles it as the
        // comparison operator.
        assert_eq!(parse_value_transform("=42").unwrap(), ("=42", None));
        // Bare `&` falls through to the operator parser, NOT this one.
        assert_eq!(parse_value_transform("&0xff").unwrap(), ("&0xff", None));
        // Bare `-` without a digit is not a transform either.
        assert_eq!(parse_value_transform("-foo").unwrap(), ("-foo", None));
    }
}
