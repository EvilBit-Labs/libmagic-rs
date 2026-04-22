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

use crate::parser::ast::{Endianness, MetaType, PStringLengthWidth, TypeKind};

/// Error returned by [`type_keyword_to_kind`] when the supplied keyword is
/// not a recognized magic type keyword.
///
/// This is a tight, structured error surfaced from a pure mapping function
/// that has no access to line-number context. Callers that *do* have line
/// context (e.g. the grammar layer wrapping a higher-level parse) can
/// convert it into a richer [`crate::error::ParseError`] variant if needed.
/// The struct is `#[non_exhaustive]` so future fields (e.g. suggested
/// alternatives) can be added without a major version bump.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::types::{type_keyword_to_kind, UnknownTypeKeyword};
///
/// let err = type_keyword_to_kind("notarealtype").unwrap_err();
/// assert_eq!(err.keyword, "notarealtype");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
#[error("unknown type keyword: {keyword}")]
pub struct UnknownTypeKeyword {
    /// The keyword string that was not recognized.
    pub keyword: String,
}

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
        // Meta / control-flow directives. `indirect` is listed first so the
        // longest match is tried before `default`, `clear`, `name`, `use`;
        // none of these collide with other supported keywords.
        //
        // `offset` is recognized here so the parser can accept magic files
        // that use it (e.g. `searchbug.magic`). In this phase it is
        // evaluated as a silent no-op via `TypeKind::Meta(MetaType::Offset)`;
        // full offset-reporting semantics are deferred.
        alt((
            tag("indirect"),
            tag("default"),
            tag("offset"),
            tag("clear"),
            tag("name"),
            tag("use"),
        )),
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
/// Returns `Ok(None)` for `regex` and `search`, which cannot be constructed
/// from the keyword alone -- they require suffix parsing (flags/count
/// for regex, mandatory `NonZeroUsize` range for search) that only
/// happens in `parser::grammar::parse_type_and_operator`. Callers that
/// need a complete `TypeKind::Regex` or `TypeKind::Search` must build
/// it directly in the grammar layer, not via this function.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::types::type_keyword_to_kind;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness};
///
/// assert_eq!(type_keyword_to_kind("byte"), Ok(Some(TypeKind::Byte { signed: true })));
/// assert_eq!(type_keyword_to_kind("ubyte"), Ok(Some(TypeKind::Byte { signed: false })));
/// assert_eq!(
///     type_keyword_to_kind("beshort"),
///     Ok(Some(TypeKind::Short { endian: Endianness::Big, signed: true }))
/// );
/// // regex/search require suffix parsing, so the keyword alone returns Ok(None).
/// assert_eq!(type_keyword_to_kind("regex"), Ok(None));
/// assert_eq!(type_keyword_to_kind("search"), Ok(None));
/// // Unknown keywords return a structured error.
/// assert!(type_keyword_to_kind("bogus").is_err());
/// ```
///
/// # Returns
///
/// * `Ok(Some(TypeKind))` for fully-specified keywords (byte, short, long,
///   quad, float, double, date, qdate, string, pstring and all their
///   variants).
/// * `Ok(None)` for suffix-required keywords (`regex`, `search`) which
///   cannot be converted from the keyword alone -- the grammar layer
///   builds their `TypeKind` directly after parsing the suffix.
/// * `Err(UnknownTypeKeyword)` if `type_name` is not a recognized
///   keyword. Under normal control flow this only happens when a caller
///   bypasses [`parse_type_keyword`] (which is the only supported way
///   to produce valid input for this function).
///
/// # Errors
///
/// Returns [`UnknownTypeKeyword`] when `type_name` is not one of the
/// keywords recognized by [`parse_type_keyword`]. This replaces a prior
/// `unreachable!` panic; library code must never panic on untrusted
/// input, and the structured error lets callers translate the failure
/// into their own error type (e.g. a nom parse error or a richer
/// `ParseError::InvalidType`).
pub fn type_keyword_to_kind(type_name: &str) -> Result<Option<TypeKind>, UnknownTypeKeyword> {
    // `regex` and `search` cannot be constructed from the keyword alone.
    // They require suffix parsing (flags/count for regex, mandatory
    // `NonZeroUsize` range for search) which only happens in
    // `parse_type_and_operator` in grammar/mod.rs. Returning `None`
    // here makes the "keyword alone isn't enough" invariant
    // type-enforced instead of relying on a placeholder that the
    // grammar layer is expected to overwrite.
    //
    // `name` and `use` also return `Ok(None)` because their identifier
    // suffix is parsed in the grammar layer, following the same
    // "keyword alone isn't enough" pattern.
    if matches!(type_name, "regex" | "search" | "name" | "use") {
        return Ok(None);
    }

    // Meta / control-flow directives with no trailing operand are fully
    // specified by the keyword alone. `offset` is included here because
    // parser-only support for it lands it in the AST as a silent no-op
    // during this phase; full offset-reporting semantics are deferred.
    match type_name {
        "default" => return Ok(Some(TypeKind::Meta(MetaType::Default))),
        "clear" => return Ok(Some(TypeKind::Meta(MetaType::Clear))),
        "indirect" => return Ok(Some(TypeKind::Meta(MetaType::Indirect))),
        "offset" => return Ok(Some(TypeKind::Meta(MetaType::Offset))),
        _ => {}
    }

    if let Some(kind) = byte_family(type_name)
        .or_else(|| short_family(type_name))
        .or_else(|| long_family(type_name))
        .or_else(|| quad_family(type_name))
        .or_else(|| float_family(type_name))
        .or_else(|| double_family(type_name))
        .or_else(|| date_family(type_name))
        .or_else(|| qdate_family(type_name))
        .or_else(|| string_family(type_name))
    {
        return Ok(Some(kind));
    }

    Err(UnknownTypeKeyword {
        keyword: type_name.to_string(),
    })
}

/// Map a byte-family keyword (`byte`, `ubyte`) to its `TypeKind`.
fn byte_family(name: &str) -> Option<TypeKind> {
    match name {
        "byte" => Some(TypeKind::Byte { signed: true }),
        "ubyte" => Some(TypeKind::Byte { signed: false }),
        _ => None,
    }
}

/// Map a short-family keyword (`short`/`ushort`/`beshort`/...) to its `TypeKind`.
fn short_family(name: &str) -> Option<TypeKind> {
    let (endian, signed) = match name {
        "short" => (Endianness::Native, true),
        "ushort" => (Endianness::Native, false),
        "leshort" => (Endianness::Little, true),
        "uleshort" => (Endianness::Little, false),
        "beshort" => (Endianness::Big, true),
        "ubeshort" => (Endianness::Big, false),
        _ => return None,
    };
    Some(TypeKind::Short { endian, signed })
}

/// Map a long-family keyword (`long`/`ulong`/`belong`/...) to its `TypeKind`.
fn long_family(name: &str) -> Option<TypeKind> {
    let (endian, signed) = match name {
        "long" => (Endianness::Native, true),
        "ulong" => (Endianness::Native, false),
        "lelong" => (Endianness::Little, true),
        "ulelong" => (Endianness::Little, false),
        "belong" => (Endianness::Big, true),
        "ubelong" => (Endianness::Big, false),
        _ => return None,
    };
    Some(TypeKind::Long { endian, signed })
}

/// Map a quad-family keyword (`quad`/`uquad`/`bequad`/...) to its `TypeKind`.
fn quad_family(name: &str) -> Option<TypeKind> {
    let (endian, signed) = match name {
        "quad" => (Endianness::Native, true),
        "uquad" => (Endianness::Native, false),
        "lequad" => (Endianness::Little, true),
        "ulequad" => (Endianness::Little, false),
        "bequad" => (Endianness::Big, true),
        "ubequad" => (Endianness::Big, false),
        _ => return None,
    };
    Some(TypeKind::Quad { endian, signed })
}

/// Map a float-family keyword (`float`/`befloat`/`lefloat`) to its `TypeKind`.
fn float_family(name: &str) -> Option<TypeKind> {
    let endian = match name {
        "float" => Endianness::Native,
        "befloat" => Endianness::Big,
        "lefloat" => Endianness::Little,
        _ => return None,
    };
    Some(TypeKind::Float { endian })
}

/// Map a double-family keyword (`double`/`bedouble`/`ledouble`) to its `TypeKind`.
fn double_family(name: &str) -> Option<TypeKind> {
    let endian = match name {
        "double" => Endianness::Native,
        "bedouble" => Endianness::Big,
        "ledouble" => Endianness::Little,
        _ => return None,
    };
    Some(TypeKind::Double { endian })
}

/// Map a 32-bit date keyword (`date`/`ldate`/`bedate`/...) to its `TypeKind`.
fn date_family(name: &str) -> Option<TypeKind> {
    let (endian, utc) = match name {
        "date" => (Endianness::Native, true),
        "ldate" => (Endianness::Native, false),
        "bedate" => (Endianness::Big, true),
        "beldate" => (Endianness::Big, false),
        "ledate" => (Endianness::Little, true),
        "leldate" => (Endianness::Little, false),
        _ => return None,
    };
    Some(TypeKind::Date { endian, utc })
}

/// Map a 64-bit date keyword (`qdate`/`qldate`/`beqdate`/...) to its `TypeKind`.
fn qdate_family(name: &str) -> Option<TypeKind> {
    let (endian, utc) = match name {
        "qdate" => (Endianness::Native, true),
        "qldate" => (Endianness::Native, false),
        "beqdate" => (Endianness::Big, true),
        "beqldate" => (Endianness::Big, false),
        "leqdate" => (Endianness::Little, true),
        "leqldate" => (Endianness::Little, false),
        _ => return None,
    };
    Some(TypeKind::QDate { endian, utc })
}

/// Map a string-family keyword (`string`, `pstring`) to its `TypeKind`.
///
/// `pstring` defaults to a 1-byte length prefix; the grammar layer
/// overwrites `length_width` / `length_includes_itself` from any
/// trailing `/B`/`/H`/`/h`/`/L`/`/l`/`/J` suffix.
fn string_family(name: &str) -> Option<TypeKind> {
    match name {
        "string" => Some(TypeKind::String { max_length: None }),
        "pstring" => Some(TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        }),
        _ => None,
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
            Ok(Some(TypeKind::Byte { signed: true }))
        );
        assert_eq!(
            type_keyword_to_kind("ubyte"),
            Ok(Some(TypeKind::Byte { signed: false }))
        );
    }

    #[test]
    fn test_type_keyword_to_kind_short_endianness() {
        assert_eq!(
            type_keyword_to_kind("short"),
            Ok(Some(TypeKind::Short {
                endian: Endianness::Native,
                signed: true
            }))
        );
        assert_eq!(
            type_keyword_to_kind("leshort"),
            Ok(Some(TypeKind::Short {
                endian: Endianness::Little,
                signed: true
            }))
        );
        assert_eq!(
            type_keyword_to_kind("beshort"),
            Ok(Some(TypeKind::Short {
                endian: Endianness::Big,
                signed: true
            }))
        );
    }

    #[test]
    fn test_type_keyword_to_kind_unsigned_variants() {
        assert_eq!(
            type_keyword_to_kind("ushort"),
            Ok(Some(TypeKind::Short {
                endian: Endianness::Native,
                signed: false
            }))
        );
        assert_eq!(
            type_keyword_to_kind("ulong"),
            Ok(Some(TypeKind::Long {
                endian: Endianness::Native,
                signed: false
            }))
        );
        assert_eq!(
            type_keyword_to_kind("uquad"),
            Ok(Some(TypeKind::Quad {
                endian: Endianness::Native,
                signed: false
            }))
        );
    }

    #[test]
    fn test_type_keyword_to_kind_signed_defaults() {
        // libmagic types are signed by default
        assert_eq!(
            type_keyword_to_kind("long"),
            Ok(Some(TypeKind::Long {
                endian: Endianness::Native,
                signed: true
            }))
        );
        assert_eq!(
            type_keyword_to_kind("quad"),
            Ok(Some(TypeKind::Quad {
                endian: Endianness::Native,
                signed: true
            }))
        );
    }

    #[test]
    fn test_type_keyword_to_kind_string() {
        assert_eq!(
            type_keyword_to_kind("string"),
            Ok(Some(TypeKind::String { max_length: None }))
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
            Ok(Some(TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }))
        );
    }

    #[test]
    fn test_type_keyword_to_kind_regex_and_search_return_none() {
        // regex and search require suffix parsing (flags/count/range)
        // that only happens in grammar/mod.rs. The keyword-to-kind
        // function deliberately returns Ok(None) for them so callers
        // are forced to use the grammar layer's direct construction.
        assert_eq!(type_keyword_to_kind("regex"), Ok(None));
        assert_eq!(type_keyword_to_kind("search"), Ok(None));
    }

    #[test]
    fn test_type_keyword_to_kind_unknown_returns_err() {
        // Unknown keywords produce a structured error instead of a
        // panic. This path is not reachable through `parse_type_keyword`
        // (which rejects unknown keywords before this function runs),
        // but it is reachable if a caller constructs the input string
        // directly, so the error must be representable.
        let err = type_keyword_to_kind("nonexistent").expect_err("unknown keyword must return Err");
        assert_eq!(err.keyword, "nonexistent");
        // And the Display impl mentions the keyword for debuggability.
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_pstring_keyword_defaults_to_one_byte_width() {
        // pstring keyword alone should produce OneByte length_width
        // (suffix parsing is handled by grammar/mod.rs, not types.rs)
        let kind = type_keyword_to_kind("pstring")
            .expect("pstring is a known keyword")
            .expect("pstring maps to Some(TypeKind)");
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
        // converted to a TypeKind by type_keyword_to_kind. Regex and
        // search are excluded from the conversion side because they
        // require suffix parsing in grammar/mod.rs and deliberately
        // return None from `type_keyword_to_kind`; the keyword
        // parser still recognizes them.
        let convertible_keywords = [
            "byte", "ubyte", "short", "ushort", "leshort", "uleshort", "beshort", "ubeshort",
            "long", "ulong", "lelong", "ulelong", "belong", "ubelong", "quad", "uquad", "lequad",
            "ulequad", "bequad", "ubequad", "float", "befloat", "lefloat", "double", "bedouble",
            "ledouble", "date", "ldate", "bedate", "beldate", "ledate", "leldate", "qdate",
            "qldate", "beqdate", "beqldate", "leqdate", "leqldate", "pstring", "string", "default",
            "clear", "indirect", "offset",
        ];
        for keyword in convertible_keywords {
            let (rest, parsed) = parse_type_keyword(keyword).unwrap();
            assert_eq!(rest, "", "Keyword {keyword} should consume all input");
            assert!(
                type_keyword_to_kind(parsed).is_ok_and(|o| o.is_some()),
                "{keyword} should map to Ok(Some(TypeKind))"
            );
        }
        // regex, search, name, and use are recognized by parse_type_keyword
        // but require grammar-layer suffix parsing (flags/count/range or an
        // identifier) to construct their TypeKind. Verify both sides of
        // this split invariant.
        for keyword in ["regex", "search", "name", "use"] {
            let (rest, parsed) = parse_type_keyword(keyword).unwrap();
            assert_eq!(rest, "", "Keyword {keyword} should consume all input");
            assert_eq!(
                type_keyword_to_kind(parsed),
                Ok(None),
                "{keyword} should return Ok(None) from keyword-to-kind"
            );
        }
    }
}
