// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type keyword parsing for magic file types
//!
//! This module handles parsing and classification of magic file type keywords
//! (byte, short, long, quad, string, etc.) into their corresponding [`TypeKind`]
//! representations. It extracts the type keyword recognition from the grammar
//! module to keep type-specific logic cohesive and manageable as new types are
//! added.

use nom::{IResult, Parser, branch::alt, bytes::complete::tag};

use crate::parser::ast::{Endianness, PStringLengthWidth, TypeKind};

/// Parse a type keyword from magic file input
///
/// Recognizes all supported type keywords and returns the matched keyword string.
/// Type keywords are organized by bit width (64, 32, 16, 8 bits) with longest
/// prefixes matched first within each group to avoid ambiguous partial matches.
///
/// # Supported Keywords
///
/// - 64-bit: `ubequad`, `ulequad`, `uquad`, `bequad`, `lequad`, `quad`
/// - 32-bit: `ubelong`, `ulelong`, `ulong`, `belong`, `lelong`, `long`
/// - 16-bit: `ubeshort`, `uleshort`, `ushort`, `beshort`, `leshort`, `short`
/// - 8-bit: `ubyte`, `byte`
/// - String: `pstring`, `string`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::types::parse_type_keyword;
///
/// let (rest, keyword) = parse_type_keyword("bequad rest").unwrap();
/// assert_eq!(keyword, "bequad");
/// assert_eq!(rest, " rest");
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if the input doesn't start with a known type keyword.
pub fn parse_type_keyword(input: &str) -> IResult<&str, &str> {
    alt((
        // 64-bit types (6 branches)
        alt((
            tag("ubequad"),
            tag("ulequad"),
            tag("uquad"),
            tag("bequad"),
            tag("lequad"),
            tag("quad"),
        )),
        // 32-bit types (6 branches)
        alt((
            tag("ubelong"),
            tag("ulelong"),
            tag("ulong"),
            tag("belong"),
            tag("lelong"),
            tag("long"),
        )),
        // 16-bit types (6 branches)
        alt((
            tag("ubeshort"),
            tag("uleshort"),
            tag("ushort"),
            tag("beshort"),
            tag("leshort"),
            tag("short"),
        )),
        // 8-bit types (2 branches)
        alt((tag("ubyte"), tag("byte"))),
        // Float/double types (6 branches)
        alt((
            tag("bedouble"),
            tag("ledouble"),
            tag("double"),
            tag("befloat"),
            tag("lefloat"),
            tag("float"),
        )),
        // Date types -- 32-bit (date) and 64-bit (qdate)
        alt((
            tag("beqldate"),
            tag("leqldate"),
            tag("beqdate"),
            tag("leqdate"),
            tag("qldate"),
            tag("qdate"),
            tag("beldate"),
            tag("leldate"),
            tag("bedate"),
            tag("ldate"),
            tag("ledate"),
            tag("date"),
        )),
        // String types (and regex/search, which share the string-type family)
        alt((tag("pstring"), tag("search"), tag("regex"), tag("string"))),
    ))
    .parse(input)
}

/// Convert a type keyword string to its corresponding [`TypeKind`]
///
/// Maps a previously parsed type keyword (from [`parse_type_keyword`]) to the
/// appropriate `TypeKind` variant with correct endianness and signedness settings.
///
/// # Conventions
///
/// - Unprefixed types are signed (libmagic default): `byte`, `short`, `long`, `quad`
/// - `u` prefix indicates unsigned: `ubyte`, `ushort`, `ulong`, `uquad`
/// - `be` prefix indicates big-endian: `beshort`, `belong`, `bequad`
/// - `le` prefix indicates little-endian: `leshort`, `lelong`, `lequad`
/// - No endian prefix means native endianness
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::types::type_keyword_to_kind;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness};
///
/// assert_eq!(type_keyword_to_kind("byte"), TypeKind::Byte { signed: true });
/// assert_eq!(type_keyword_to_kind("ubyte"), TypeKind::Byte { signed: false });
/// assert_eq!(
///     type_keyword_to_kind("beshort"),
///     TypeKind::Short { endian: Endianness::Big, signed: true }
/// );
/// ```
///
/// # Panics
///
/// Panics if `type_name` is not a recognized type keyword. This function should
/// only be called with values returned by [`parse_type_keyword`].
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn type_keyword_to_kind(type_name: &str) -> TypeKind {
    match type_name {
        // BYTE types (8-bit)
        "byte" => TypeKind::Byte { signed: true },
        "ubyte" => TypeKind::Byte { signed: false },

        // SHORT types (16-bit)
        "short" => TypeKind::Short {
            endian: Endianness::Native,
            signed: true,
        },
        "ushort" => TypeKind::Short {
            endian: Endianness::Native,
            signed: false,
        },
        "leshort" => TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
        "uleshort" => TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        "beshort" => TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        },
        "ubeshort" => TypeKind::Short {
            endian: Endianness::Big,
            signed: false,
        },

        // LONG types (32-bit)
        "long" => TypeKind::Long {
            endian: Endianness::Native,
            signed: true,
        },
        "ulong" => TypeKind::Long {
            endian: Endianness::Native,
            signed: false,
        },
        "lelong" => TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        "ulelong" => TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        "belong" => TypeKind::Long {
            endian: Endianness::Big,
            signed: true,
        },
        "ubelong" => TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },

        // QUAD types (64-bit)
        "quad" => TypeKind::Quad {
            endian: Endianness::Native,
            signed: true,
        },
        "uquad" => TypeKind::Quad {
            endian: Endianness::Native,
            signed: false,
        },
        "lequad" => TypeKind::Quad {
            endian: Endianness::Little,
            signed: true,
        },
        "ulequad" => TypeKind::Quad {
            endian: Endianness::Little,
            signed: false,
        },
        "bequad" => TypeKind::Quad {
            endian: Endianness::Big,
            signed: true,
        },
        "ubequad" => TypeKind::Quad {
            endian: Endianness::Big,
            signed: false,
        },

        // FLOAT types (32-bit)
        "float" => TypeKind::Float {
            endian: Endianness::Native,
        },
        "befloat" => TypeKind::Float {
            endian: Endianness::Big,
        },
        "lefloat" => TypeKind::Float {
            endian: Endianness::Little,
        },

        // DOUBLE types (64-bit)
        "double" => TypeKind::Double {
            endian: Endianness::Native,
        },
        "bedouble" => TypeKind::Double {
            endian: Endianness::Big,
        },
        "ledouble" => TypeKind::Double {
            endian: Endianness::Little,
        },

        // DATE types (32-bit Unix timestamp)
        "date" => TypeKind::Date {
            endian: Endianness::Native,
            utc: true,
        },
        "ldate" => TypeKind::Date {
            endian: Endianness::Native,
            utc: false,
        },
        "bedate" => TypeKind::Date {
            endian: Endianness::Big,
            utc: true,
        },
        "beldate" => TypeKind::Date {
            endian: Endianness::Big,
            utc: false,
        },
        "ledate" => TypeKind::Date {
            endian: Endianness::Little,
            utc: true,
        },
        "leldate" => TypeKind::Date {
            endian: Endianness::Little,
            utc: false,
        },

        // QDATE types (64-bit Unix timestamp)
        "qdate" => TypeKind::QDate {
            endian: Endianness::Native,
            utc: true,
        },
        "qldate" => TypeKind::QDate {
            endian: Endianness::Native,
            utc: false,
        },
        "beqdate" => TypeKind::QDate {
            endian: Endianness::Big,
            utc: true,
        },
        "beqldate" => TypeKind::QDate {
            endian: Endianness::Big,
            utc: false,
        },
        "leqdate" => TypeKind::QDate {
            endian: Endianness::Little,
            utc: true,
        },
        "leqldate" => TypeKind::QDate {
            endian: Endianness::Little,
            utc: false,
        },

        // STRING types
        "string" => TypeKind::String { max_length: None },
        // Default to 1-byte prefix; suffix parsing handled in grammar/mod.rs
        "pstring" => TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },

        // REGEX type -- suffix parsing (flags) handled in grammar/mod.rs
        "regex" => TypeKind::Regex {
            case_insensitive: false,
            start_of_line: false,
        },

        // SEARCH type -- range parsing handled in grammar/mod.rs
        "search" => TypeKind::Search { range: None },

        _ => unreachable!("type_keyword_to_kind called with unknown type: {type_name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Endianness;

    // ============================================================
    // parse_type_keyword tests
    // ============================================================

    #[test]
    fn test_parse_type_keyword_byte_variants() {
        assert_eq!(parse_type_keyword("byte rest"), Ok((" rest", "byte")));
        assert_eq!(parse_type_keyword("ubyte rest"), Ok((" rest", "ubyte")));
    }

    #[test]
    fn test_parse_type_keyword_short_variants() {
        let cases = [
            ("short", "short"),
            ("ushort", "ushort"),
            ("leshort", "leshort"),
            ("uleshort", "uleshort"),
            ("beshort", "beshort"),
            ("ubeshort", "ubeshort"),
        ];
        for (input, expected) in cases {
            let input_with_rest = format!("{input} rest");
            let (rest, keyword) = parse_type_keyword(&input_with_rest).unwrap();
            assert_eq!(keyword, expected, "Failed for input: {input}");
            assert_eq!(rest, " rest", "Wrong remaining for input: {input}");
        }
    }

    #[test]
    fn test_parse_type_keyword_long_variants() {
        let cases = ["long", "ulong", "lelong", "ulelong", "belong", "ubelong"];
        for input in cases {
            let input_with_rest = format!("{input} rest");
            let (rest, keyword) = parse_type_keyword(&input_with_rest).unwrap();
            assert_eq!(keyword, input, "Failed for: {input}");
            assert_eq!(rest, " rest");
        }
    }

    #[test]
    fn test_parse_type_keyword_quad_variants() {
        let cases = ["quad", "uquad", "lequad", "ulequad", "bequad", "ubequad"];
        for input in cases {
            let input_with_rest = format!("{input} rest");
            let (rest, keyword) = parse_type_keyword(&input_with_rest).unwrap();
            assert_eq!(keyword, input, "Failed for: {input}");
            assert_eq!(rest, " rest");
        }
    }

    #[test]
    fn test_parse_type_keyword_string() {
        assert_eq!(parse_type_keyword("string rest"), Ok((" rest", "string")));
    }

    #[test]
    fn test_parse_type_keyword_unknown() {
        assert!(parse_type_keyword("unknown rest").is_err());
    }

    #[test]
    fn test_parse_type_keyword_empty() {
        assert!(parse_type_keyword("").is_err());
    }

    // ============================================================
    // type_keyword_to_kind tests
    // ============================================================

    #[test]
    fn test_type_keyword_to_kind_byte() {
        assert_eq!(
            type_keyword_to_kind("byte"),
            TypeKind::Byte { signed: true }
        );
        assert_eq!(
            type_keyword_to_kind("ubyte"),
            TypeKind::Byte { signed: false }
        );
    }

    #[test]
    fn test_type_keyword_to_kind_short_endianness() {
        assert_eq!(
            type_keyword_to_kind("short"),
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true
            }
        );
        assert_eq!(
            type_keyword_to_kind("leshort"),
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true
            }
        );
        assert_eq!(
            type_keyword_to_kind("beshort"),
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true
            }
        );
    }

    #[test]
    fn test_type_keyword_to_kind_unsigned_variants() {
        assert_eq!(
            type_keyword_to_kind("ushort"),
            TypeKind::Short {
                endian: Endianness::Native,
                signed: false
            }
        );
        assert_eq!(
            type_keyword_to_kind("ulong"),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: false
            }
        );
        assert_eq!(
            type_keyword_to_kind("uquad"),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: false
            }
        );
    }

    #[test]
    fn test_type_keyword_to_kind_signed_defaults() {
        // libmagic types are signed by default
        assert_eq!(
            type_keyword_to_kind("long"),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true
            }
        );
        assert_eq!(
            type_keyword_to_kind("quad"),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true
            }
        );
    }

    #[test]
    fn test_type_keyword_to_kind_string() {
        assert_eq!(
            type_keyword_to_kind("string"),
            TypeKind::String { max_length: None }
        );
    }

    #[test]
    fn test_parse_type_keyword_pstring() {
        assert_eq!(parse_type_keyword("pstring rest"), Ok((" rest", "pstring")));
    }

    #[test]
    fn test_type_keyword_to_kind_pstring() {
        assert_eq!(
            type_keyword_to_kind("pstring"),
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        );
    }

    #[test]
    fn test_pstring_keyword_defaults_to_one_byte_width() {
        // pstring keyword alone should produce OneByte length_width
        // (suffix parsing is handled by grammar/mod.rs, not types.rs)
        let kind = type_keyword_to_kind("pstring");
        match kind {
            TypeKind::PString {
                max_length,
                length_width,
                length_includes_itself: _,
            } => {
                assert_eq!(
                    max_length, None,
                    "pstring default should have no max_length"
                );
                assert_eq!(
                    length_width,
                    PStringLengthWidth::OneByte,
                    "pstring default should be OneByte"
                );
            }
            _ => panic!("Expected TypeKind::PString, got {kind:?}"),
        }
    }

    #[test]
    fn test_pstring_keyword_does_not_consume_suffix() {
        // parse_type_keyword should only consume "pstring", leaving suffix for grammar
        let (rest, keyword) = parse_type_keyword("pstring/H =value").unwrap();
        assert_eq!(keyword, "pstring");
        assert_eq!(
            rest, "/H =value",
            "Suffix should remain unconsumed by type keyword parser"
        );
    }

    #[test]
    fn test_pstring_keyword_boundary() {
        // pstring at exact boundary (no trailing input)
        let (rest, keyword) = parse_type_keyword("pstring").unwrap();
        assert_eq!(keyword, "pstring");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_pstring_before_operator() {
        // pstring followed by whitespace then operator
        let (rest, keyword) = parse_type_keyword("pstring =hello").unwrap();
        assert_eq!(keyword, "pstring");
        assert_eq!(rest, " =hello");
    }

    #[test]
    fn test_roundtrip_all_keywords() {
        // Verify that every keyword parsed by parse_type_keyword can be
        // converted to a TypeKind by type_keyword_to_kind
        let keywords = [
            "byte", "ubyte", "short", "ushort", "leshort", "uleshort", "beshort", "ubeshort",
            "long", "ulong", "lelong", "ulelong", "belong", "ubelong", "quad", "uquad", "lequad",
            "ulequad", "bequad", "ubequad", "float", "befloat", "lefloat", "double", "bedouble",
            "ledouble", "date", "ldate", "bedate", "beldate", "ledate", "leldate", "qdate",
            "qldate", "beqdate", "beqldate", "leqdate", "leqldate", "pstring", "string", "regex",
            "search",
        ];
        for keyword in keywords {
            let (rest, parsed) = parse_type_keyword(keyword).unwrap();
            assert_eq!(rest, "", "Keyword {keyword} should consume all input");
            // Should not panic
            let _ = type_keyword_to_kind(parsed);
        }
    }
}
