// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! String-family bareword value parsing for magic file rules.
//!
//! Covers the value-dispatch fallback used by `string`/`pstring`/`string16`/
//! `search` rules (see GOTCHAS S3.6, S3.12, S6.7): a leading quoted-string
//! or hex/escape-byte attempt, followed by a bareword fallback that never
//! interprets the token as a number. Extracted from `grammar/mod.rs` as a
//! pure code-motion split (issue #391 Unit U4) -- no behavior changes; the
//! getstr/regex special-casing and drop-backslash escape logic are moved
//! intact.

use nom::{IResult, character::complete::multispace0, error::Error as NomError};

use crate::parser::ast::Value;

use super::value;

/// Parse the comparison value for a string-family type.
///
/// libmagic never interprets a `string`/`pstring`/`string16`/`search`
/// comparison value as a number: `0 string >0` compares against the
/// literal ASCII byte `'0'` (0x30), and `>0.6.1` against the literal
/// characters `0.6.1` -- not the integer 0 or the float 0.6. The generic
/// [`parse_value`] tries its float and integer branches (see
/// `value::parse_value`) before falling through, so a bareword like `0` or
/// `0.6.1` was captured as `Value::Uint`/`Value::Float`. A subsequent
/// comparison against the string field read from the file then yields no
/// ordering (`String` vs `Uint`/`Float` is incomparable), so the rule
/// silently never matched -- breaking real `>0` idioms such as
/// `\b, name %s` / `face %s` / `palette %s` and version compares like
/// `>0.6.1 ... version %s`.
///
/// This parser mirrors [`parse_value`]'s ordering for the two branches
/// that are correct for string-family values -- a leading whitespace trim,
/// then a quoted string (-> `Value::String`), then a hex/escape byte
/// sequence (-> `Value::Bytes`, e.g. gzip's `\037\213` or `\177ELF`) -- but
/// replaces the numeric (float/integer) branches with
/// [`parse_bare_string_value`], so every remaining bareword resolves to a
/// `Value::String`. The leading `multispace0` is load-bearing: it ensures
/// the hex branch sees byte-identical input to what `parse_value` fed it,
/// so an escape-heavy value cannot fall through to the lossy-UTF-8
/// `parse_bare_string_value` path and corrupt a high byte (see the
/// `high-byte-utf8-corruption-class` note).
///
/// Hex-*letter* barewords (`>AB`, `cafebabe`) still resolve to
/// `Value::Bytes` via the unchanged hex branch, matching GOTCHAS S3.12 --
/// only the numeric subset changes here.
///
/// # Errors
/// Returns a nom parsing error only when the value is empty/whitespace-only
/// (via [`parse_bare_string_value`]); quoted and hex forms are attempted
/// first and never error out of this function on a non-empty token.
pub(super) fn parse_string_family_value(input: &str) -> IResult<&str, Value> {
    // Trim leading whitespace up front so the hex branch below receives the
    // same (trimmed) input `parse_value` would have handed it.
    let (input, _) = multispace0(input)?;
    if let Ok((rest, s)) = value::parse_quoted_string(input) {
        return Ok((rest, Value::String(s)));
    }
    if let Ok((rest, bytes)) = value::parse_hex_bytes(input) {
        return Ok((rest, Value::Bytes(bytes)));
    }
    parse_bare_string_value(input)
}

/// Parse a bare (unquoted) single-token string literal as a `Value::String`.
///
/// Used only as a fallback for string-family types (`string`, `pstring`,
/// `regex`, `search`) when the strict [`parse_value`] alternatives all
/// fail. Consumes leading whitespace, then reads a run of non-whitespace
/// characters as the literal value, **interpreting magic(5) escape
/// sequences** along the way: `\0`, `\n`, `\r`, `\t`, `\\`, `\"`, `\'`,
/// `\NNN` (3-digit octal), and `\xNN` (hex). This supports magic-file
/// rules like `0 string PNCIHISK\0 ...` where the trailing `\0` denotes
/// a literal NUL byte that must be present in the file.
///
/// Without escape interpretation, the comparison value stored in the
/// AST is the literal six-byte string `\` + `0` instead of `\x00`, and
/// the rule never matches against a real on-disk byte sequence ending
/// in NUL. This was a regression that prevented even simple top-level
/// rules from matching when loaded from a magic file.
///
/// # Errors
/// Returns a nom parsing error if the input contains no non-whitespace
/// token (e.g. it is empty or consists entirely of whitespace).
pub(super) fn parse_bare_string_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;
    if input.is_empty() || input.starts_with(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
    {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut remaining = input;
    while let Some(ch) = remaining.chars().next() {
        if ch.is_whitespace() || ch == '\n' || ch == '\r' {
            break;
        }
        if ch == '\\' {
            // Try a hex byte (`\xNN`) first since `parse_escape_sequence`
            // doesn't recognise it.
            if let Ok((rest, b)) = value::parse_hex_byte_with_prefix(remaining) {
                bytes.push(b);
                remaining = rest;
                continue;
            }
            if let Ok((rest, esc)) = value::parse_escape_sequence(remaining) {
                // `parse_escape_sequence` returns a `char`, but the
                // escape table covers single-byte values (NUL, control
                // chars, octal `\NNN` clamped to a `u8`). Cast back to
                // `u8` so the buffer stays byte-accurate when the
                // value is later compared against file bytes.
                let code = esc as u32;
                if let Ok(byte) = u8::try_from(code) {
                    bytes.push(byte);
                } else {
                    // Escape produced a non-byte char (shouldn't happen
                    // with the current escape grammar, but guard
                    // anyway). Encode as UTF-8 so we never lose data
                    // silently.
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(esc.encode_utf8(&mut buf).as_bytes());
                }
                remaining = rest;
                continue;
            }
            // Lone `\` followed by an unrecognised escape char: magic(5)
            // getstr DROPS the backslash and keeps the character literally
            // (`\<` -> `<`, `\^` -> `^`, `\ ` -> a literal space that
            // continues the token because it lands in `bytes` rather than
            // being re-examined by the whitespace-terminator check). This
            // matches GNU `file`; the earlier "keep the backslash" behavior
            // broke real rules like sgml's `0 string \<?xml\ version=`, which
            // then never matched an actual `<?xml ...` document (XML files
            // fell through to "ASCII text"). A genuine literal backslash is
            // written `\\` and is already resolved by `parse_escape_sequence`
            // above, so this branch only ever drops a backslash that was
            // escaping a non-special character. A trailing lone `\` at
            // end-of-input has no following char, so it stays literal.
            if let Some(next) = remaining[1..].chars().next() {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(next.encode_utf8(&mut buf).as_bytes());
                remaining = &remaining[1 + next.len_utf8()..];
            } else {
                bytes.push(b'\\');
                remaining = &remaining[1..];
            }
            continue;
        }
        // Plain character: encode as UTF-8 (ASCII is one byte; non-ASCII
        // is 2-4 bytes which matches how the file would store the same
        // characters in a UTF-8 magic file).
        let mut buf = [0u8; 4];
        let utf8 = ch.encode_utf8(&mut buf).as_bytes();
        bytes.extend_from_slice(utf8);
        remaining = &remaining[ch.len_utf8()..];
    }

    if bytes.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    // Mirror `read_string_exact` (evaluator/types/string.rs): when the
    // resolved bytes are valid UTF-8, return `Value::String` so `%s`
    // output renders normally; when they are NOT -- e.g. a bareword like
    // OS/2 INF's `HSP\x01\x9b\x00` (0x9b is invalid UTF-8), or an octal
    // form like `AB\376` -- return the RAW bytes as `Value::Bytes`. A
    // lossy `String` decode would turn 0x9b into U+FFFD (3 bytes 0xEF BF
    // BD), which BOTH inflates the pattern's byte length (6 -> 8, so
    // `read_string_exact` reads the wrong number of bytes) AND changes the
    // byte value at that position, so the rule would silently never match.
    // Cross-type `String`/`Bytes` equality and ordering (GOTCHAS S2.3)
    // compare by byte sequence, so either variant compares correctly
    // against the read value. This completes the read/parse symmetry:
    // `read_string_exact` was fixed to return `Value::Bytes` on non-UTF-8
    // slices, but the parse side kept lossy-decoding until this change.
    match String::from_utf8(bytes) {
        Ok(s) => Ok((remaining, Value::String(s))),
        Err(e) => Ok((remaining, Value::Bytes(e.into_bytes()))),
    }
}
