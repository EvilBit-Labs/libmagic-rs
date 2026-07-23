// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type keyword parsing tests.
//!
//! Covers `parse_type_and_operator`/`parse_type_keyword`: basic types,
//! string flag letters, endianness, unsigned variants, signed
//! defaults, and `pstring` type parsing.

use super::*;

#[test]
fn test_parse_pstring_invalid_suffix_rejected() {
    // Invalid suffix characters after '/' should produce a parse error
    let invalid_cases = ["pstring/Z", "pstring/X", "pstring/W", "pstring/1"];
    for input in invalid_cases {
        let result = parse_type_and_operator(input);
        assert!(
            result.is_err(),
            "Expected error for invalid suffix: {input}"
        );
    }
}

#[test]
fn test_parse_type_basic() {
    assert_eq!(
        parse_type("byte"),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("short"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("long"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("string"),
        Ok((
            "",
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default()
            }
        ))
    );
}

/// Table-driven coverage for the eight magic(5) `string` flag letters.
///
/// Each row asserts that parsing `string/<flags>` produces a `TypeKind::String`
/// whose `flags` field has exactly the expected booleans set. The intent is to
/// pin both the per-letter mapping AND the combination behavior in a single
/// place so future regressions on either axis fail loudly. See GOTCHAS S6.5
/// (asymmetric `/c`/`/C`) and S6.6 (`/B` is pstring-only, not a string flag).
#[test]
fn test_parse_type_string_flag_letters() {
    type FlagBuilder = fn(StringFlags) -> StringFlags;
    let cases: &[(&str, FlagBuilder)] = &[
        ("string/W", |f| f.with_compact_whitespace(true)),
        ("string/w", |f| f.with_compact_optional_whitespace(true)),
        ("string/c", |f| f.with_ignore_lowercase(true)),
        ("string/C", |f| f.with_ignore_uppercase(true)),
        ("string/t", |f| f.with_text_test(true)),
        ("string/T", |f| f.with_trim(true)),
        ("string/b", |f| f.with_bin_test(true)),
        ("string/f", |f| f.with_full_word(true)),
    ];
    for (input, build) in cases {
        let expected = build(StringFlags::default());
        assert_eq!(
            parse_type(input),
            Ok((
                "",
                TypeKind::String {
                    max_length: None,
                    flags: expected,
                }
            )),
            "parsing {input} produced unexpected flags"
        );
    }
}

/// Flags compose. `/cw` should set both `ignore_lowercase` and
/// `compact_optional_whitespace`; `/wcCtTbf` should set all eight.
#[test]
fn test_parse_type_string_flag_combinations() {
    let (rest, typ) = parse_type("string/cw").expect("string/cw should parse");
    assert_eq!(rest, "");
    let StringFlags {
        ignore_lowercase,
        compact_optional_whitespace,
        ..
    } = match typ {
        TypeKind::String { flags, .. } => flags,
        other => panic!("expected TypeKind::String, got {other:?}"),
    };
    assert!(ignore_lowercase, "/c should set ignore_lowercase");
    assert!(
        compact_optional_whitespace,
        "/w should set compact_optional_whitespace"
    );

    let (rest, typ) = parse_type("string/wcCtTbf").expect("string/wcCtTbf should parse");
    assert_eq!(rest, "");
    let flags = match typ {
        TypeKind::String { flags, .. } => flags,
        other => panic!("expected TypeKind::String, got {other:?}"),
    };
    assert!(flags.compact_optional_whitespace);
    assert!(flags.ignore_lowercase);
    assert!(flags.ignore_uppercase);
    assert!(flags.text_test);
    assert!(flags.trim);
    assert!(flags.bin_test);
    assert!(flags.full_word);
    // `/W` is not in the combination string above; ensure it stayed off so
    // the test discriminates between the two whitespace flags.
    assert!(!flags.compact_whitespace);
}

/// `/B` is NOT a string flag in libmagic -- it is the pstring 1-byte
/// length-width letter. The grammar must reject `string/B` rather than
/// silently accepting it (an earlier PR #233 draft incorrectly included
/// `'B'` in the string-flag set; this is the regression guard).
///
/// Why "rejected" means "the slash and B remain unconsumed": the grammar
/// layer leaves a `/B` it doesn't recognize in `input`, and the value
/// parser then fails on it. Asserting the full `parse_type` result is
/// flaky (the error type from nom is awkward); instead, drive
/// `parse_magic_rule` and verify the rule load fails.
#[test]
fn test_parse_type_string_rejects_b_flag() {
    use crate::parser::parse_text_magic_file;
    let result = parse_text_magic_file("0 string/B FOO bar\n");
    assert!(
        result.is_err(),
        "string/B should be a parse error -- /B is a pstring suffix, not a string flag"
    );
}

#[test]
fn test_parse_type_endianness() {
    assert_eq!(
        parse_type("leshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("beshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("lelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("belong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true
            }
        ))
    );
}

#[test]
fn test_parse_type_with_whitespace() {
    assert_eq!(
        parse_type(" byte "),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("\tstring\t"),
        Ok((
            "",
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default()
            }
        ))
    );
    assert_eq!(
        parse_type("  lelong  "),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
}

#[test]
fn test_parse_type_with_remaining_input() {
    assert_eq!(
        parse_type("byte ="),
        Ok(("=", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("string \\x7f"),
        Ok((
            "\\x7f",
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default()
            }
        ))
    );
}

#[test]
fn test_parse_type_invalid() {
    assert!(parse_type("").is_err());
    assert!(parse_type("invalid").is_err());
    assert!(parse_type("int").is_err());
}

#[test]
fn test_parse_type_unsigned_variants() {
    assert_eq!(
        parse_type("ubyte"),
        Ok(("", TypeKind::Byte { signed: false }))
    );
    assert_eq!(
        parse_type("ushort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubeshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("uleshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("uquad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
}

#[test]
fn test_parse_type_signed_defaults() {
    // In libmagic, unprefixed types are signed by default
    assert_eq!(
        parse_type("byte"),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("short"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("long"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("beshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("belong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("quad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("bequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("lequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            }
        ))
    );
}

// PString type tests
#[test]
fn test_parse_type_pstring() {
    assert_eq!(
        parse_type("pstring"),
        Ok((
            "",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
}

#[test]
fn test_parse_type_pstring_with_remaining_input() {
    assert_eq!(
        parse_type("pstring ="),
        Ok((
            "=",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
    assert_eq!(
        parse_type("pstring \"hello\""),
        Ok((
            "\"hello\"",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
}
