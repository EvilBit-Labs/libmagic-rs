// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `parse_type_and_operator` suffix-flag parsing tests.
//!
//! Covers pstring length-width suffixes, quad bitwise-mask suffixes,
//! and regex/search count and flag suffixes.

use super::*;

#[test]
fn test_parse_type_and_operator_pstring_standalone_and() {
    let (remaining, (typ, op, _)) = parse_type_and_operator("pstring& ").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(
        typ,
        TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false
        }
    );
    assert_eq!(op, Some(Operator::BitwiseAnd));
}

#[test]
fn test_parse_type_and_operator_quad_full_width_mask() {
    // Full u64 mask (0xffffffffffffffff) must parse successfully, not silently
    // fall back to standalone '&' leaving the mask as leftover input.
    let (remaining, (typ, op, _)) = parse_type_and_operator("uquad&0xffffffffffffffff").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(
        typ,
        TypeKind::Quad {
            endian: Endianness::Native,
            signed: false,
        }
    );
    assert_eq!(op, Some(Operator::BitwiseAndMask(u64::MAX)));
}

#[test]
fn test_parse_type_and_operator_quad_mask_various() {
    // Hex mask within i64 range
    let (remaining, (_, op, _)) = parse_type_and_operator("quad&0x7fffffffffffffff").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAndMask(i64::MAX as u64)));

    // Decimal mask
    let (remaining, (_, op, _)) = parse_type_and_operator("uquad&255").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAndMask(255)));

    // Standalone '&' (no digits following) still works
    let (remaining, (_, op, _)) = parse_type_and_operator("uquad& ").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAnd));
}

#[test]
fn test_parse_type_and_operator_mask_overflow_fails() {
    // Decimal value exceeding u64::MAX must fail, not silently reinterpret
    let result = parse_type_and_operator("uquad&99999999999999999999");
    assert!(
        result.is_err(),
        "overflowing mask should produce a parse error"
    );

    // Hex value exceeding u64 (17 hex digits) must fail
    let result = parse_type_and_operator("uquad&0x1ffffffffffffffff");
    assert!(
        result.is_err(),
        "overflowing hex mask should produce a parse error"
    );
}

#[test]
fn test_parse_type_and_operator_pstring_suffixes() {
    use crate::parser::ast::TypeKind;
    let cases: &[(&str, PStringLengthWidth, bool, &str)] = &[
        ("pstring", PStringLengthWidth::OneByte, false, ""),
        ("pstring/B", PStringLengthWidth::OneByte, false, ""),
        ("pstring/H", PStringLengthWidth::TwoByteBE, false, ""),
        ("pstring/h", PStringLengthWidth::TwoByteLE, false, ""),
        ("pstring/L", PStringLengthWidth::FourByteBE, false, ""),
        ("pstring/l", PStringLengthWidth::FourByteLE, false, ""),
        ("pstring/H =", PStringLengthWidth::TwoByteBE, false, "="),
        ("pstring/J", PStringLengthWidth::OneByte, true, ""),
        ("pstring/BJ", PStringLengthWidth::OneByte, true, ""),
        ("pstring/HJ", PStringLengthWidth::TwoByteBE, true, ""),
        ("pstring/hJ", PStringLengthWidth::TwoByteLE, true, ""),
        ("pstring/LJ", PStringLengthWidth::FourByteBE, true, ""),
        ("pstring/lJ", PStringLengthWidth::FourByteLE, true, ""),
    ];
    for &(input, expected_width, expected_j, expected_rest) in cases {
        let (rest, (kind, op, _)) = parse_type_and_operator(input).expect(input);
        assert_eq!(rest, expected_rest, "rest for input: {input}");
        assert!(op.is_none(), "operator for input: {input}");
        match kind {
            TypeKind::PString {
                max_length,
                length_width,
                length_includes_itself,
            } => {
                assert_eq!(max_length, None, "max_length for input: {input}");
                assert_eq!(
                    length_width, expected_width,
                    "length_width for input: {input}"
                );
                assert_eq!(
                    length_includes_itself, expected_j,
                    "length_includes_itself for input: {input}"
                );
            }
            _ => panic!("Expected PString for input: {input}, got {kind:?}"),
        }
    }
}

#[test]
fn test_parse_type_and_operator_regex_and_search_suffixes() {
    use crate::parser::ast::{RegexCount, RegexFlags, TypeKind};
    use std::num::{NonZeroU32, NonZeroUsize};

    fn rx(case: bool, start: bool, count: RegexCount) -> TypeKind {
        TypeKind::Regex {
            flags: RegexFlags {
                case_insensitive: case,
                start_offset: start,
            },
            count,
        }
    }
    fn sr(n: usize) -> TypeKind {
        TypeKind::Search {
            range: NonZeroUsize::new(n),
            flags: SearchFlags::default(),
        }
    }
    fn nz(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).unwrap()
    }

    let cases: &[(&str, TypeKind, &str)] = &[
        ("regex", rx(false, false, RegexCount::Default), ""),
        ("regex/c", rx(true, false, RegexCount::Default), ""),
        ("regex/l", rx(false, false, RegexCount::Lines(None)), ""),
        ("regex/s", rx(false, true, RegexCount::Default), ""),
        ("regex/cl", rx(true, false, RegexCount::Lines(None)), ""),
        ("regex/lc", rx(true, false, RegexCount::Lines(None)), ""),
        ("regex/cs", rx(true, true, RegexCount::Default), ""),
        ("regex/csl", rx(true, true, RegexCount::Lines(None)), ""),
        (
            "regex/1l",
            rx(false, false, RegexCount::Lines(Some(nz(1)))),
            "",
        ),
        (
            "regex/l1",
            rx(false, false, RegexCount::Lines(Some(nz(1)))),
            "",
        ),
        ("regex/1c", rx(true, false, RegexCount::Bytes(nz(1))), ""),
        (
            "regex/256",
            rx(false, false, RegexCount::Bytes(nz(256))),
            "",
        ),
        ("regex/c =", rx(true, false, RegexCount::Default), "="),
        ("search/256", sr(256), ""),
        ("search/1", sr(1), ""),
        ("search/256 =", sr(256), "="),
        // Bare `search` (no `/N`) parses with an open (`None`) range =
        // scan-to-EOF, matching GNU `file`'s implementation.
        (
            "search",
            TypeKind::Search {
                range: None,
                flags: SearchFlags::default(),
            },
            "",
        ),
    ];
    for &(input, ref expected_kind, expected_rest) in cases {
        let (rest, (kind, op, _)) = parse_type_and_operator(input).expect(input);
        assert_eq!(rest, expected_rest, "rest for input: {input}");
        assert!(op.is_none(), "operator for input: {input}");
        assert_eq!(&kind, expected_kind, "kind for input: {input}");
    }
}

#[test]
fn test_parse_type_and_operator_bare_search_accepted_zero_rejected() {
    use crate::parser::ast::TypeKind;
    // Bare `search` (no /N suffix) is ACCEPTED and parses to an open
    // (`None`) range = scan-to-EOF. magic(5) documents the count as
    // required, but the reference `file` binary accepts the bare form
    // (`str_range == 0`); rmagic follows the implementation. Real system
    // magic uses this (e.g. pdf `>8 search /Count`, `0 search
    // ##fileformat=VCFv`).
    let (_, (kind, _, _)) =
        parse_type_and_operator("search").expect("bare search must parse (scan-to-EOF)");
    assert!(
        matches!(kind, TypeKind::Search { range: None, .. }),
        "bare search must yield range None, got {kind:?}"
    );
    // `search/0` is still rejected -- an explicit zero-width scan is
    // unrepresentable (`NonZeroUsize`).
    assert!(parse_type_and_operator("search/0").is_err());
}

#[test]
fn test_parse_type_and_operator_regex_invalid_suffix() {
    // Bare slash with no flags or count
    assert!(parse_type_and_operator("regex/").is_err());
    // Unrecognized flag letter
    assert!(parse_type_and_operator("regex/z").is_err());
    // Non-operator trailing character is still rejected
    assert!(parse_type_and_operator("regex/cz").is_err());
    // regex/0 is rejected because a zero count has no valid semantics
    // (our parser uses NonZeroU32 to express "user specified a count").
    assert!(parse_type_and_operator("regex/0").is_err());
    // regex/l0 -- zero count with line flag, same rejection path.
    assert!(parse_type_and_operator("regex/l0").is_err());
}

#[test]
fn test_parse_type_and_operator_regex_rejects_duplicate_count() {
    // Libmagic accepts these with a "multiple ranges" stderr warning;
    // we prefer a hard parse error so magic-file bugs surface at parse
    // time rather than silently using the last-seen count.
    assert!(
        parse_type_and_operator("regex/1l2l").is_err(),
        "regex/1l2l should reject the second count"
    );
    assert!(
        parse_type_and_operator("regex/1c2l").is_err(),
        "regex/1c2l should reject the second count"
    );
    assert!(
        parse_type_and_operator("regex/l1l2").is_err(),
        "regex/l1l2 should reject the second count"
    );
    // Valid single-count forms must still parse.
    assert!(parse_type_and_operator("regex/1l").is_ok());
    assert!(parse_type_and_operator("regex/l1").is_ok());
}

#[test]
fn test_parse_type_and_operator_regex_operator_adjacent() {
    use crate::parser::ast::{Operator, RegexCount, RegexFlags, TypeKind};

    // `regex/c=` should leave `=` for parse_operator, matching the `regex/c =`
    // (space-separated) behavior and mirroring `search/256=`.
    let (rest, (kind, op, _)) = parse_type_and_operator("regex/c=").expect("regex/c=");
    assert_eq!(rest, "=");
    assert!(op.is_none());
    assert_eq!(
        kind,
        TypeKind::Regex {
            flags: RegexFlags {
                case_insensitive: true,
                ..RegexFlags::default()
            },
            count: RegexCount::Default,
        }
    );

    // `regex/l!=` should leave `!=` for parse_operator.
    let (rest, (kind, op, _)) = parse_type_and_operator("regex/l!=").expect("regex/l!=");
    assert_eq!(rest, "!=");
    assert!(op.is_none());
    assert_eq!(
        kind,
        TypeKind::Regex {
            flags: RegexFlags::default(),
            count: RegexCount::Lines(None),
        }
    );

    // Confirm the full pipeline parses the operator correctly through
    // parse_type_and_operator + parse_operator chaining.
    let (rest, (_, _, _)) = parse_type_and_operator("regex/c=foo").expect("regex/c=foo");
    let (rest_after_op, op) = crate::parser::grammar::parse_operator(rest).expect("operator");
    assert_eq!(op, Operator::Equal);
    assert_eq!(rest_after_op, "foo");
}
