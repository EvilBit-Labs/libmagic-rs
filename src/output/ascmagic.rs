// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Minimal text/data fallback classification, modeled on GNU `file`'s
//! `file_ascmagic` (`src/ascmagic.c`).
//!
//! When no magic rule produces a usable description -- either because no
//! rule matched at all, or because every rule that matched carries no
//! description text (GOTCHAS S13.2) -- GNU `file` never prints a blank
//! line. It falls back to a basic content classification: `"empty"` for a
//! zero-byte file, `"ASCII text"` for plain textual content, a Unicode
//! variant for valid non-ASCII UTF-8, and `"data"` for anything else
//! (binary content).
//!
//! # Scope
//!
//! This is a deliberately narrow subset of GNU `file`'s real charset
//! detection, which additionally distinguishes ISO-8859 variants, UTF-16,
//! line-ending styles, and several "text with X" qualifiers (escape
//! sequences, overstriking, CRLF terminators, byte-order marks, etc. --
//! see `src/ascmagic.c` and `src/encoding.c` upstream). Replicating that
//! fully is out of scope for this fallback: the goal here is solely to
//! ensure the CLI never emits a blank description for a readable file
//! (the assembler-source-text and plain-ASCII-text bugs this module
//! fixes), not full charset fidelity. Every classification below is a
//! true subset of what GNU `file` would print for the same input -- e.g.
//! `file` prints `"ASCII text, with CRLF line terminators"` for a
//! CRLF-terminated buffer where we print plain `"ASCII text"` -- so
//! differential tests that check for a specific classification (rather
//! than exact byte-for-byte output) still hold.

/// Bytes GNU `file`'s `ascmagic`/`encoding` text test treats as part of
/// ordinary "text" content: printable ASCII (0x20..=0x7E) plus the common
/// control characters that appear in real-world text files (tab, LF, CR,
/// vertical tab, form feed) and a few legacy terminal-control bytes GNU
/// `file` still classifies as text -- bell, backspace, and escape --
/// which it reports as `"ASCII text, with ..."` qualifiers rather than
/// reclassifying as binary `"data"`. This fallback does not reproduce
/// those qualifiers (see the module doc), but a buffer containing only
/// these bytes is still `"ASCII text"`, not `"data"`.
fn is_text_safe_byte(b: u8) -> bool {
    matches!(
        b,
        0x20..=0x7E | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C | 0x07 | 0x08 | 0x1B
    )
}

/// Classify a buffer using the minimal text/data fallback described in
/// the module doc.
///
/// Returns one of `"empty"`, `"ASCII text"`, `"UTF-8 Unicode text"`, or
/// `"data"`. This is intentionally infallible -- there is no input for
/// which classification can fail, so the caller never needs to handle
/// an error path here (matching the evaluator's graceful-degradation
/// discipline: a fallback that can itself fail would defeat its purpose).
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::ascmagic::classify_fallback;
///
/// assert_eq!(classify_fallback(b""), "empty");
/// assert_eq!(classify_fallback(b"hello world\n"), "ASCII text");
/// assert_eq!(classify_fallback(&[0x00, 0x01, 0x02, 0xff]), "data");
/// ```
#[must_use]
pub fn classify_fallback(buffer: &[u8]) -> &'static str {
    if buffer.is_empty() {
        return "empty";
    }

    if buffer.iter().all(|&b| is_text_safe_byte(b)) {
        return "ASCII text";
    }

    // Valid non-ASCII UTF-8 only counts as text when every low byte (< 0x80) is
    // itself text-safe. A NUL or other binary control byte is a strong binary
    // signal even inside an otherwise-valid UTF-8 buffer -- GNU `file`
    // classifies such input as data, and the ASCII branch above already treats
    // these bytes as non-text. High bytes (>= 0x80) are exempt from the check
    // because they are the UTF-8 continuation/lead bytes the `from_utf8` gate
    // validates. Without this, a valid-UTF-8 buffer carrying an embedded NUL
    // would be mislabelled "UTF-8 Unicode text".
    let has_non_ascii_byte = buffer.iter().any(|&b| b >= 0x80);
    let no_binary_control_byte = buffer.iter().all(|&b| b >= 0x80 || is_text_safe_byte(b));
    if has_non_ascii_byte && no_binary_control_byte && std::str::from_utf8(buffer).is_ok() {
        return "UTF-8 Unicode text";
    }

    "data"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_empty_buffer_as_empty() {
        assert_eq!(classify_fallback(b""), "empty");
    }

    #[test]
    fn classifies_plain_ascii_as_ascii_text() {
        let cases: &[(&str, &[u8])] = &[
            ("simple sentence", b"hello world this is plain text\n"),
            ("tabs and newlines", b"a\tb\nc\r\nd\n"),
            ("vertical tab and form feed", b"a\x0bb\x0cc\n"),
            ("bell, backspace, escape", b"a\x07b\x08c\x1bd\n"),
        ];
        for (label, input) in cases {
            assert_eq!(
                classify_fallback(input),
                "ASCII text",
                "case {label:?} should classify as ASCII text"
            );
        }
    }

    #[test]
    fn classifies_valid_non_ascii_utf8_as_utf8_unicode_text() {
        let cases: &[(&str, &[u8])] = &[
            ("accented latin", "caf\u{e9} r\u{e9}sum\u{e9}\n".as_bytes()),
            ("bom prefix", &[0xEF, 0xBB, 0xBF, b'h', b'i']),
            ("multi-byte cjk", "\u{4f60}\u{597d}\n".as_bytes()),
        ];
        for (label, input) in cases {
            assert_eq!(
                classify_fallback(input),
                "UTF-8 Unicode text",
                "case {label:?} should classify as UTF-8 Unicode text"
            );
        }
    }

    #[test]
    fn classifies_binary_content_as_data() {
        let cases: &[(&str, &[u8])] = &[
            ("null byte in otherwise-ascii text", b"hello\x00world\n"),
            ("random high bytes", &[0x80, 0x81, 0x82, 0x83]),
            ("invalid utf8 continuation-only", &[0xC0, 0x80]),
            ("elf magic", &[0x7f, b'E', b'L', b'F']),
        ];
        for (label, input) in cases {
            assert_eq!(
                classify_fallback(input),
                "data",
                "case {label:?} should classify as data"
            );
        }
    }
}
