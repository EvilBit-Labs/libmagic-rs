// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type-specification parsing for magic file rules.
//!
//! Covers the `name`/`use` meta-type identifier parse, the combined
//! type+operator+transform parse (`parse_type_and_operator`), and the
//! standalone `parse_type` helper. Extracted from `grammar/mod.rs` as a
//! pure code-motion split (issue #391 Unit U4) -- no behavior changes.

use nom::{
    IResult, Parser, bytes::complete::take_while, character::complete::multispace0,
    error::Error as NomError,
};

use crate::parser::ast::{MetaType, Operator, TypeKind};

use super::type_suffix::{
    parse_attached_operator, parse_pstring_suffix, parse_regex_suffix, parse_search_suffix,
    parse_value_transform,
};

/// Parse the identifier operand of a `name` / `use` meta-type directive.
///
/// Called from [`parse_type_and_operator`] when the leading keyword is
/// `name` or `use`. Enforces that the keyword is followed by whitespace,
/// an identifier matching `[A-Za-z0-9_-]+`, and no further non-whitespace
/// content on the line. Malformed identifiers such as `part2=foo`
/// (operator-adjacent continuation) or `part 2` (split identifier) are
/// rejected as parse errors rather than silently consumed as a message.
/// Whether `tail` is a GNU `file` no-separator marker alone on its line.
///
/// Used to keep a `use` site's `\b` (which controls spacing) while still
/// dropping a use-site description, which magic(5) has no slot for.
fn is_lone_no_separator_marker(tail: &str) -> bool {
    let line_end = tail.find(['\n', '\r']).unwrap_or(tail.len());
    let line = &tail[..line_end];
    // The marker check is inlined rather than sharing
    // `evaluator::strip_no_separator_marker`: this module is compiled into
    // `build.rs` as well, which cannot reference lib-only modules
    // (GOTCHAS S1.1). Both forms are recognized, matching that helper.
    line.strip_prefix('\u{0008}')
        .or_else(|| line.strip_prefix("\\b"))
        .is_some_and(|rest| rest.trim().is_empty())
}

fn parse_name_or_use_meta<'a>(
    type_name: &str,
    input: &'a str,
) -> IResult<
    &'a str,
    (
        TypeKind,
        Option<Operator>,
        Option<crate::parser::ast::ValueTransform>,
    ),
> {
    use nom::character::complete::space1;

    // Require at least one whitespace character between the keyword and
    // the identifier. `space1` rejects an empty gap, which enforces
    // "bare `name` / `use` with no identifier" as a parse error.
    let (input, _) = space1(input)?;

    // magic(5) allows a `\^` prefix on a `use` identifier to mean "invoke
    // the named subroutine but flip the endianness of every read inside
    // it" (libmagic `softmagic.c` `cvt_flip`). Consume the prefix and
    // record it on `MetaType::Use::flip_endian` so the evaluator can apply
    // the flip (issue #236). The `\^` prefix is meaningless on `name`
    // declarations, so only `use` is inspected.
    let (input, use_flip_endian) = if type_name == "use" {
        input
            .strip_prefix("\\^")
            .map_or((input, false), |rest| (rest, true))
    } else {
        (input, false)
    };

    let (after_id, id) =
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-').parse(input)?;
    if id.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            after_id,
            nom::error::ErrorKind::AlphaNumeric,
        )));
    }

    // The character immediately following the identifier must be
    // whitespace or end-of-input. Anything else (e.g. `=`, `!`, `<`,
    // `>`, `&`, `^`, `~`, `|`, punctuation) means `take_while` truncated
    // a malformed identifier such as `part2=foo`: reject instead of
    // silently treating the leftover text as a message.
    if let Some(next_char) = after_id.chars().next()
        && !matches!(next_char, ' ' | '\t' | '\n' | '\r')
    {
        return Err(nom::Err::Error(NomError::new(
            after_id,
            nom::error::ErrorKind::Alpha,
        )));
    }

    // Handle the trailing text after the identifier. The two directives
    // diverge here, matching GNU `file`:
    //
    // - `use`: magic(5) has no message slot for a `use` site, and GNU
    //   `file` never emits a use-site's own description (verified: a
    //   `use foo BAR` renders no `BAR`). Drop the trailing text up to
    //   end-of-line so the caller parses an empty message.
    // - `name`: the `name` line's OWN description IS emitted when the
    //   subroutine is invoked via `use` -- Mach-O universal `0 name
    //   mach-o \b [`, `0 name matlab4 Matlab v4 mat-file`, `0 name
    //   algol_68 Algol 68 source text`, etc. PRESERVE the trailing text
    //   so the caller's `parse_message` captures it as the rule message
    //   (later stored in the name table by `extract_name_table` and
    //   emitted at the `use` site). Dropping it here is what previously
    //   made rmagic omit those fragments (e.g. the leading `[` of the
    //   Mach-O universal bracket detail).
    //
    // We deliberately do NOT reject embedded whitespace inside the
    // identifier itself (which would be a real malformed rule like
    // `part 2`); that's enforced earlier when `take_while` truncates the
    // identifier on the first non-id character.
    //
    // The one exception on the `use` side is a lone no-separator marker
    // (`>0 use mach-o-cpu \b`): that is a formatting control, not a
    // description, and it must survive so the evaluator can attach the
    // subroutine's first output with no separating space. Across the whole
    // system magic database every `use` site carrying trailing text carries
    // exactly this marker and nothing else, so the marker is preserved only
    // when the rest of the line is blank; any other trailing text is dropped
    // as before.
    let mut tail = after_id;
    if type_name == "use" {
        while let Some(rest) = tail.strip_prefix(' ').or_else(|| tail.strip_prefix('\t')) {
            tail = rest;
        }
        if let Some(next_char) = tail.chars().next()
            && !matches!(next_char, '\n' | '\r')
            && !is_lone_no_separator_marker(tail)
        {
            let line_end = tail.find(['\n', '\r']).unwrap_or(tail.len());
            tail = &tail[line_end..];
        }
    }

    let meta = if type_name == "name" {
        MetaType::Name(id.to_string())
    } else {
        MetaType::Use {
            name: id.to_string(),
            flip_endian: use_flip_endian,
        }
    };
    let (rest, _) = multispace0(tail)?;
    Ok((rest, (TypeKind::Meta(meta), None, None)))
}

/// Parse a type specification with an optional attached bitwise-AND mask operator
/// (e.g., `lelong&0xf0000000`).
///
/// Returns the `TypeKind`, an optional attached `Operator` (`&MASK`), and an
/// optional pre-comparison `ValueTransform` (`+N`, `-N`, `*N`, `/N`, `%N`,
/// `|N`, `^N`).
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_type_and_operator;
/// use libmagic_rs::parser::ast::{TypeKind, Operator, Endianness};
///
/// // Type without operator or transform
/// let (_, (kind, op, transform)) = parse_type_and_operator("lelong").unwrap();
/// assert_eq!(kind, TypeKind::Long { endian: Endianness::Little, signed: true });
/// assert_eq!(op, None);
/// assert_eq!(transform, None);
///
/// // Type with mask operator
/// let (_, (kind, op, _)) = parse_type_and_operator("lelong&0xf0000000").unwrap();
/// assert!(matches!(op, Some(Operator::BitwiseAndMask(_))));
///
/// // Type with arithmetic transform
/// let (_, (kind, op, transform)) = parse_type_and_operator("lelong+1").unwrap();
/// assert_eq!(op, None);
/// assert!(transform.is_some());
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected format
pub fn parse_type_and_operator(
    input: &str,
) -> IResult<
    &str,
    (
        TypeKind,
        Option<Operator>,
        Option<crate::parser::ast::ValueTransform>,
    ),
> {
    use crate::parser::ast::{PStringLengthWidth, RegexCount, RegexFlags};

    let (input, _) = multispace0(input)?;

    let (mut input, type_name) = crate::parser::types::parse_type_keyword(input)?;

    // `name` and `use` are meta-type directives with a mandatory
    // identifier suffix. They short-circuit the operator/value parse
    // path via `parse_name_or_use_meta`, which also rejects malformed
    // identifiers (operator-adjacent continuations like `part2=foo` or
    // split identifiers like `part 2`).
    if type_name == "name" || type_name == "use" {
        return parse_name_or_use_meta(type_name, input);
    }

    // Handle pstring suffixes: /B, /H, /h, /L, /l, and optional /J modifier
    let mut pstring_length_width = PStringLengthWidth::OneByte;
    let mut pstring_length_includes_itself = false;
    if type_name == "pstring"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, width, includes_j) = parse_pstring_suffix(suffix_rest)?;
        input = rest;
        pstring_length_width = width;
        pstring_length_includes_itself = includes_j;
    }

    // Handle regex suffixes via the extracted helper. See
    // `grammar/type_suffix.rs::parse_regex_suffix` for the full
    // "any-order flag/count interleaving, duplicate counts rejected"
    // semantics and the `RegexCount` collapse logic.
    let mut regex_flags = RegexFlags::default();
    let mut regex_count = RegexCount::Default;
    if type_name == "regex"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, (flags, count)) = parse_regex_suffix(input, suffix_rest)?;
        regex_flags = flags;
        regex_count = count;
        input = rest;
    }

    // Handle search suffix. `search/N[/flags]` supplies an explicit scan
    // window (e.g., `search/256`, `search/256/s`, `search/256/cs`); the
    // count `N` is a `NonZeroUsize`, so `search/0` is rejected. A bare
    // `search` (no `/` suffix, e.g. `>8 search /Count`) is ALSO accepted:
    // its `range` stays `None`, meaning scan-to-end-of-buffer. magic(5)
    // documents the count as required, but the reference `file` binary
    // accepts the bare form (`str_range == 0`), so we follow the
    // implementation rather than the spec. Disambiguation is unambiguous:
    // a ranged suffix attaches `/` directly to the keyword, while a bare
    // search is followed by whitespace then the value.
    let mut search_range: Option<::std::num::NonZeroUsize> = None;
    let mut search_flags = crate::parser::ast::SearchFlags::default();
    if type_name == "search"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, (range, flags)) = parse_search_suffix(input, suffix_rest)?;
        search_range = Some(range);
        search_flags = flags;
        input = rest;
    }

    // Handle string flag suffixes (e.g., `string/w`, `string/cW`).
    // magic(5) flag letters per libmagic `src/file.h`:
    //   `/W` STRING_COMPACT_WHITESPACE
    //   `/w` STRING_COMPACT_OPTIONAL_WHITESPACE
    //   `/c` STRING_IGNORE_LOWERCASE  (pattern lowercase => file folded)
    //   `/C` STRING_IGNORE_UPPERCASE  (pattern uppercase => file folded)
    //   `/t` STRING_TEXTTEST          (text-mode hint)
    //   `/T` STRING_TRIM              (trim pattern leading/trailing ws)
    //   `/b` STRING_BINTEST           (binary-mode hint)
    //   `/f` STRING_FULL_WORD         (post-match word-boundary check)
    //
    // `/B` is deliberately NOT accepted here -- it is the pstring
    // 1-byte length-width letter (`CHAR_PSTRING_1_BE`) and is not a
    // string flag in libmagic. An earlier draft of this loop
    // accepted `'B'`; that was wrong and is now rejected. See
    // GOTCHAS S6.6.
    let mut string_flags = crate::parser::ast::StringFlags::default();
    if type_name == "string"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let mut consumed = 0usize;
        for ch in suffix_rest.chars() {
            match ch {
                'W' => string_flags.compact_whitespace = true,
                'w' => string_flags.compact_optional_whitespace = true,
                'c' => string_flags.ignore_lowercase = true,
                'C' => string_flags.ignore_uppercase = true,
                't' => string_flags.text_test = true,
                'T' => string_flags.trim = true,
                'b' => string_flags.bin_test = true,
                'f' => string_flags.full_word = true,
                _ => break,
            }
            consumed += ch.len_utf8();
        }
        if consumed > 0 {
            input = &suffix_rest[consumed..];
        }
        // If `consumed == 0`, the `/` is not followed by a known flag
        // letter. Leave `input` pointing at the `/` so the value
        // parser (or the trailing-junk check) fails meaningfully. This
        // is the path that rejects `string/B` and `string/x`.
    }

    // Check for a pre-comparison value transform (e.g., `lelong+1` or
    // `ulequad/1073741824`). magic(5) supports `+`, `-`, `*`, `/`, `%`,
    // `|`, and `^` between the type keyword and the comparison value;
    // the transform runs on the read value before the comparison
    // operator and before printf-style format substitution.
    let (input, value_transform) = parse_value_transform(input)?;

    // Check for an attached bitwise operator with optional mask (e.g.,
    // `&0xf0000000` or bare `&`). See `type_suffix::parse_attached_operator`
    // for the recognized forms and their error behavior. magic(5) does
    // not allow combining `&MASK` with another value transform on the
    // same rule, so the parsers are sequential and either-or in
    // practice.
    let (input, attached_op) = parse_attached_operator(input)?;

    let (input, _) = multispace0(input)?;

    // Build Regex/Search directly from the parsed suffixes; fall back to
    // `type_keyword_to_kind` for every other type. PString still uses the
    // patch-after-construct pattern because `type_keyword_to_kind` supplies
    // its `max_length` default and the suffix parser only produces the
    // length-width and `/J` flag.
    let type_kind = match type_name {
        "regex" => TypeKind::Regex {
            flags: regex_flags,
            count: regex_count,
        },
        "search" => TypeKind::Search {
            // `None` range = bare `search` = scan-to-EOF (see the suffix
            // handling above); `Some(n)` = `search/N`.
            range: search_range,
            flags: search_flags,
        },
        _ => {
            // `type_keyword_to_kind` returns:
            //  * `Ok(Some(kind))` for every fully-specified keyword
            //    (byte, short, long, quad, float/double, dates,
            //    string, pstring and variants).
            //  * `Ok(None)` for suffix-required keywords (`regex`,
            //    `search`), which are handled by the match arms above
            //    and should never reach this branch.
            //  * `Err(UnknownTypeKeyword)` for a keyword that was never
            //    produced by `parse_type_keyword`. Under the grammar's
            //    normal flow this is unreachable because `type_name`
            //    was just returned by `parse_type_keyword`, but the
            //    function is `pub` and we do not rely on panics to
            //    enforce the invariant -- we convert both "shouldn't
            //    happen" cases into a nom parse error anchored at the
            //    current input position so the parser can backtrack or
            //    report a clean failure without aborting the process.
            let Ok(Some(mut kind)) = crate::parser::types::type_keyword_to_kind(type_name) else {
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )));
            };
            if let TypeKind::PString { max_length, .. } = kind {
                kind = TypeKind::PString {
                    max_length,
                    length_width: pstring_length_width,
                    length_includes_itself: pstring_length_includes_itself,
                };
            }
            // Stamp the parsed string flags onto the `string` variant.
            // `type_keyword_to_kind` returns `flags: StringFlags::default()`
            // because the flag-bearing suffix is grammar-layer only; the
            // type-keyword layer never sees it.
            if let TypeKind::String { max_length, .. } = kind {
                kind = TypeKind::String {
                    max_length,
                    flags: string_flags,
                };
            }
            kind
        }
    };

    Ok((input, (type_kind, attached_op, value_transform)))
}

/// Parse a type specification (byte, short, long, quad, string, etc.)
///
/// Supports various type formats found in magic files:
/// - `byte` / `ubyte` - single byte (signed / unsigned)
/// - `short` / `ushort` - 16-bit integer (native endian, signed / unsigned)
/// - `leshort` / `uleshort` - 16-bit little-endian integer
/// - `beshort` / `ubeshort` - 16-bit big-endian integer
/// - `long` / `ulong` - 32-bit integer (native endian, signed / unsigned)
/// - `lelong` / `ulelong` - 32-bit little-endian integer
/// - `belong` / `ubelong` - 32-bit big-endian integer
/// - `quad` / `uquad` - 64-bit integer (native endian, signed / unsigned)
/// - `lequad` / `ulequad` - 64-bit little-endian integer
/// - `bequad` / `ubequad` - 64-bit big-endian integer
/// - `string` - null-terminated string
/// - `pstring` - Pascal string (length-prefixed, supports `/B` (1-byte, default), `/H` or `/h` (2-byte), `/L` or `/l` (4-byte) suffixes)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_type;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness};
///
/// assert_eq!(parse_type("byte"), Ok(("", TypeKind::Byte { signed: true })));
/// assert_eq!(parse_type("leshort"), Ok(("", TypeKind::Short { endian: Endianness::Little, signed: true })));
/// assert_eq!(parse_type("bequad"), Ok(("", TypeKind::Quad { endian: Endianness::Big, signed: true })));
/// assert_eq!(parse_type("string"), Ok(("", TypeKind::String { max_length: None, flags: StringFlags::default() })));
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match any known type
#[allow(dead_code)] // Standalone helper exercised by grammar unit tests.
pub fn parse_type(input: &str) -> IResult<&str, TypeKind> {
    let (input, (type_kind, _, _)) = parse_type_and_operator(input)?;
    Ok((input, type_kind))
}
