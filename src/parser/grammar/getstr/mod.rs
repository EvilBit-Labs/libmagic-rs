// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! GNU `file`-compatible `getstr` escape resolution for `regex` pattern values.
//!
//! `TypeKind::Regex` patterns are captured here as a getstr-**resolved**
//! [`Value::String`] instead of falling through the generic
//! `parse_hex_bytes`/`parse_bare_string_value` bareword path used by the
//! other string-family types. Two problems make this its own module:
//!
//! 1. `parse_value`'s `alt(...)` tries `parse_hex_bytes` (see
//!    `super::value`) before any string interpretation, so a `regex`
//!    pattern beginning with a magic(5) escape (`\^`, `\040`, `\t`, `\x..`)
//!    was previously captured as `Value::Bytes` instead of `Value::String`
//!    -- the evaluator's `Regex` arms only accepted `Value::String`,
//!    producing a fatal `UnsupportedType` error for every such rule.
//! 2. Rust's `regex` crate does not interpret octal escapes (`\040`) the
//!    way GNU `file`'s `getstr` does, so handing the RAW escape text to
//!    `regex::bytes::Regex::new` is not an option -- the pattern must be
//!    getstr-resolved (escapes replaced by their resolved bytes) *before*
//!    it reaches the regex compiler, exactly as GNU `file` does before
//!    calling `regcomp`.
//!
//! # Escape table (verified against GNU `file` source, not inferred)
//!
//! The table below was read directly from `file/file`'s `src/apprentice.c`,
//! function `getstr` (`master` branch, fetched during this fix; the
//! function body starts with the comment "Convert a string containing C
//! character escapes. Stop at an unescaped space or tab."). The relevant
//! `switch (c = *s++)` after a backslash is seen:
//!
//! - `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v` resolve to the corresponding
//!   C control-character byte: `0x07`, `0x08`, `0x0C`, `0x0A`, `0x0D`,
//!   `0x09`, `0x0B`.
//! - `\0`-`\7` starts a 1-3 digit **octal** escape (`\NNN`); the resolved
//!   value is truncated to a byte exactly like getstr's `CAST(char, val)`.
//! - `\x` starts a 0-2 digit **hex** escape (`\xNN`). If no valid hex
//!   digit follows, getstr's `val` variable keeps its initialized default
//!   of the literal character `'x'` (`0x78`) -- an intentional getstr
//!   quirk, replicated here for fidelity.
//! - **Every other escaped character** -- the "relation" characters
//!   `> < & ^ = !`, backslash itself, space, an escaped `.`, an escaped
//!   literal tab character, and any genuinely unrecognized escape such as
//!   `\^` -- falls through getstr's `default:` case into the shared
//!   `case ' ':` body, whose only effect is `*p++ = c;`. In other words:
//!   **the backslash is dropped and the following character is kept
//!   literally.** This is not special-cased per character in the C
//!   source; it is the general fallback for anything not in the named
//!   list above.
//!
//! ## Resolved open question: control-char / regex-shorthand collision
//!
//! GNU `file`'s getstr resolves `\b` to the raw control byte `0x08`
//! (backspace) -- Rust's `regex` crate assigns `\b` (the two-character
//! ASCII sequence backslash + `b`) the *different* meaning "word boundary
//! assertion". Confirmed from the `apprentice.c` source above that this is
//! genuinely how getstr behaves (case `'b'`: `*p++ = '\b';`), so the
//! collision the plan flagged is real *at the character level* -- but it
//! does **not** manifest as an actual bug here, because this resolver
//! never re-emits the two-character sequence `\` + `b`. It appends the
//! single resolved byte `0x08` directly to the pattern text. A lone raw
//! `0x08` byte, unescaped, has no special meaning to the `regex` crate --
//! it is matched as an ordinary literal byte, not reinterpreted as the
//! `\b` escape (that requires the literal two-character source sequence
//! `\b`, which this resolver by construction never produces for a
//! getstr-resolved byte `< 0x80`). The same reasoning applies to any
//! other named control escape (`\a`, `\f`, `\n`, `\r`, `\t`, `\v`): each
//! resolves to a single raw byte, not to escape *syntax*, so it cannot be
//! reinterpreted by the downstream regex compiler. No `\xHH` re-encoding
//! is needed for these -- only bytes `>= 0x80` require it (see
//! [`push_resolved_byte`]), because those cannot be represented as a
//! single-byte Rust `char`/UTF-8 sequence at all.
//!
//! Unrelated but worth noting for anyone reading a `.magic` file with
//! `\d`, `\s`, or `\w` in a `regex` pattern expecting PCRE-style character
//! classes: getstr does **not** special-case those letters, so they fall
//! into the "everything else" bucket above and are demoted to the bare
//! literal character (`\d` -> `d`, `\s` -> `s`, `\w` -> `w`). That is
//! genuine upstream libmagic behavior, not a bug in this resolver.

use nom::IResult;
use nom::character::complete::multispace0;
use nom::error::Error as NomError;

use crate::parser::ast::Value;

/// Parse a single (unquoted) `regex` pattern token, resolving magic(5)
/// `getstr` escapes into the byte sequence GNU `file` would produce, and
/// return it as a [`Value::String`] ready for `regex::bytes::Regex::new`.
///
/// Token boundary: consumes non-whitespace characters, stopping at the
/// first *unescaped* whitespace character -- mirroring getstr's
/// `isspace(c)` early-exit (see module docs). Quoted (`"..."`) regex
/// values are **not** handled here; callers should keep using
/// [`super::value::parse_value`] for those (see `GOTCHAS.md` S3.6 and the
/// existing quoted-regex round-trip test for why quoting is preserved
/// as-is rather than getstr-resolved).
///
/// # Errors
/// Returns a nom parsing error if the input is empty, starts with
/// whitespace, or (after escape resolution) yields no characters at all.
pub(super) fn parse_regex_getstr_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;
    if input.is_empty() || input.starts_with(char::is_whitespace) {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    let mut resolved = String::new();
    let mut remaining = input;

    while let Some(ch) = remaining.chars().next() {
        if ch.is_whitespace() {
            break;
        }
        if ch == '\\' {
            let after_backslash = &remaining[ch.len_utf8()..];
            if let Some(rest) = resolve_getstr_escape(&mut resolved, after_backslash) {
                remaining = rest;
            } else {
                // Incomplete escape at end of token (a lone trailing
                // backslash): getstr's `case '\0': ... goto out;` stops
                // scanning here without emitting the backslash.
                remaining = after_backslash;
                break;
            }
            continue;
        }
        resolved.push(ch);
        remaining = &remaining[ch.len_utf8()..];
    }

    if resolved.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    Ok((remaining, Value::String(resolved)))
}

/// Resolve a single getstr escape sequence, appending its resolved
/// representation to `resolved`. `rest` is the input immediately
/// following the triggering backslash. Returns the remaining input after
/// the escape, or `None` if `rest` is empty (an incomplete trailing
/// escape -- see [`parse_regex_getstr_value`]).
fn resolve_getstr_escape<'a>(resolved: &mut String, rest: &'a str) -> Option<&'a str> {
    let c = rest.chars().next()?;
    let after = &rest[c.len_utf8()..];

    // Named single-character control escapes (getstr `case 'a'`..`'v'`).
    let named_byte = match c {
        'a' => Some(0x07u8),
        'b' => Some(0x08),
        'f' => Some(0x0C),
        'n' => Some(0x0A),
        'r' => Some(0x0D),
        't' => Some(0x09),
        'v' => Some(0x0B),
        _ => None,
    };
    if let Some(byte) = named_byte {
        push_resolved_byte(resolved, byte);
        return Some(after);
    }

    if c.is_ascii_digit() && ('0'..='7').contains(&c) {
        let (byte, rest_after_octal) = resolve_octal_escape(c, after);
        push_resolved_byte(resolved, byte);
        return Some(rest_after_octal);
    }

    if c == 'x' {
        let (byte, rest_after_hex) = resolve_hex_escape(after);
        push_resolved_byte(resolved, byte);
        return Some(rest_after_hex);
    }

    // getstr `default:` / shared `case ' ':` body: drop the backslash,
    // keep the literal character. Covers the "relation" characters
    // (`> < & ^ = !`), backslash itself, space, an escaped `.`, an
    // escaped literal tab, and any unrecognized escape (`\^`, `\d`, ...).
    resolved.push(c);
    Some(after)
}

/// Resolve a 1-3 digit octal escape (`\NNN`), matching getstr's
/// look-ahead-up-to-2-more-digits logic. `first` is the digit already
/// consumed by the caller's dispatch (guaranteed `'0'..='7'`).
fn resolve_octal_escape(first: char, rest: &str) -> (u8, &str) {
    let mut val: u32 = first.to_digit(8).unwrap_or(0);
    let mut remaining = rest;

    for _ in 0..2 {
        let Some(next) = remaining.chars().next() else {
            break;
        };
        if !('0'..='7').contains(&next) {
            break;
        }
        val = (val << 3) | next.to_digit(8).unwrap_or(0);
        remaining = &remaining[next.len_utf8()..];
    }

    // A 3-digit octal escape can exceed 255 (e.g. `\777` = 511); getstr
    // truncates via `CAST(char, val)`. `as u8` matches that truncation.
    #[allow(clippy::cast_possible_truncation)]
    let byte = val as u8;
    (byte, remaining)
}

/// Resolve a 0-2 digit hex escape (`\xNN`), matching getstr's quirky
/// "no digits following `\x`" default of the literal character `'x'`.
fn resolve_hex_escape(rest: &str) -> (u8, &str) {
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return (b'x', rest);
    };
    let Some(high) = first.to_digit(16) else {
        return (b'x', rest);
    };
    let after_first = &rest[first.len_utf8()..];

    let Some(second) = after_first.chars().next() else {
        #[allow(clippy::cast_possible_truncation)]
        return (high as u8, after_first);
    };
    let Some(low) = second.to_digit(16) else {
        #[allow(clippy::cast_possible_truncation)]
        return (high as u8, after_first);
    };

    let after_second = &after_first[second.len_utf8()..];
    #[allow(clippy::cast_possible_truncation)]
    let byte = ((high << 4) | low) as u8;
    (byte, after_second)
}

/// Append a resolved getstr byte to the in-progress pattern text.
///
/// Bytes `< 0x80` are valid ASCII / single-byte UTF-8 and are pushed
/// directly as a `char`. Bytes `>= 0x80` cannot be pushed as a raw `char`
/// (`0x80`-`0xFF` are not valid standalone Unicode scalar values, and
/// `String` must stay valid UTF-8 -- pushing one would panic or require
/// lossy reinterpretation that corrupts the intended byte value). Per the
/// plan's KTD3, such bytes are instead re-encoded as a regex-native
/// `\xHH` hex escape *in the output text*, so the regex engine -- not the
/// `String` -- carries the byte. This is a Rust-`String`-vs-C-`char*`
/// type-system accommodation, not a getstr behavior: GNU `file`'s C
/// implementation just writes the raw byte into an untyped buffer handed
/// to `regcomp`.
fn push_resolved_byte(resolved: &mut String, byte: u8) {
    use std::fmt::Write as _;

    if byte < 0x80 {
        resolved.push(byte as char);
    } else {
        // `write!` to a `String` is infallible; the `Result` only
        // exists to satisfy the generic `fmt::Write` trait.
        #[allow(clippy::let_underscore_must_use)]
        let _ = write!(resolved, "\\x{byte:02x}");
    }
}

#[cfg(test)]
mod tests;
