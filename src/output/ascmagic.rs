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
    text_char_class(b) == TextClass::Text
}

/// Classification of a single byte in GNU `file`'s `encoding.c::text_chars`
/// table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextClass {
    /// `F`: never text (NUL, most C0 controls, DEL).
    Binary,
    /// `T`: plain text (BEL BS HT LF VT FF CR ESC, 0x20..=0x7E, NEL).
    Text,
    /// `I`: ISO-8859 high half (0xA0..=0xFF).
    Latin1,
    /// `X`: C1 controls (0x80..=0x9F except NEL); "non-ISO extended ASCII".
    Extended,
}

/// Port of upstream `text_chars[256]` (`encoding.c`).
pub(crate) const fn text_char_class(b: u8) -> TextClass {
    match b {
        0x07..=0x0D | 0x1B | 0x20..=0x7E | 0x85 => TextClass::Text,
        0x80..=0x9F => TextClass::Extended,
        0xA0..=0xFF => TextClass::Latin1,
        _ => TextClass::Binary,
    }
}

/// Result of [`looks_utf8`], mirroring upstream `file_looks_utf8`'s
/// `-1 / 0 / 1 / 2` return codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Utf8Look {
    /// `-1`: not valid UTF-8.
    Invalid,
    /// `0`: valid, but uses control bytes the text table rejects.
    Control,
    /// `1`: 7-bit text (also returned for a lead byte truncated at EOF).
    Ascii,
    /// `2`: valid UTF-8 with at least one multi-byte character.
    Multibyte,
}

/// Lead-byte shape from upstream's `first[]` / `accept_ranges[]` tables:
/// (continuation count, lowest and highest value accepted for the first
/// continuation byte). `None` is an invalid lead byte.
const fn utf8_lead_shape(b: u8) -> Option<(usize, u8, u8)> {
    match b {
        0xC2..=0xDF => Some((1, 0x80, 0xBF)),
        0xE0 => Some((2, 0xA0, 0xBF)),
        0xE1..=0xEC | 0xEE..=0xEF => Some((2, 0x80, 0xBF)),
        0xED => Some((2, 0x80, 0x9F)),
        0xF0 => Some((3, 0x90, 0xBF)),
        0xF1..=0xF3 => Some((3, 0x80, 0xBF)),
        0xF4 => Some((3, 0x80, 0x8F)),
        _ => None,
    }
}

/// Direct port of `encoding.c::file_looks_utf8`, quirks included: a lead
/// byte truncated by end-of-buffer is not an error (the scan just stops),
/// and a rejected control byte anywhere yields [`Utf8Look::Control`] even
/// when multi-byte characters were seen.
pub(crate) fn looks_utf8(buf: &[u8]) -> Utf8Look {
    let finish = |ctrl: bool, gotone: bool| match (ctrl, gotone) {
        (true, _) => Utf8Look::Control,
        (false, true) => Utf8Look::Multibyte,
        (false, false) => Utf8Look::Ascii,
    };
    let mut gotone = false;
    let mut ctrl = false;
    let mut i = 0;
    while let Some(&b) = buf.get(i) {
        i += 1;
        if b & 0x80 == 0 {
            if text_char_class(b) != TextClass::Text {
                ctrl = true;
            }
            continue;
        }
        if b & 0x40 == 0 {
            return Utf8Look::Invalid;
        }
        let Some((following, lo, hi)) = utf8_lead_shape(b) else {
            return Utf8Look::Invalid;
        };
        for n in 0..following {
            let Some(&c) = buf.get(i) else {
                return finish(ctrl, gotone);
            };
            i += 1;
            if (n == 0 && !(lo..=hi).contains(&c)) || c & 0xC0 != 0x80 {
                return Utf8Look::Invalid;
            }
        }
        gotone = true;
    }
    finish(ctrl, gotone)
}

/// Classify a buffer using the minimal text/data fallback described in
/// the module doc.
///
/// Returns one of `"empty"`, `"ASCII text"`, `"Unicode text, UTF-8 text"`, or
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

    // Upstream `file_encoding` order: valid multi-byte UTF-8 with no rejected
    // control byte is Unicode text (`encoding.c` code "Unicode text, UTF-8" +
    // type "text"). A NUL inside otherwise-valid UTF-8 is `Control`, not text.
    if looks_utf8(buffer) == Utf8Look::Multibyte {
        return "Unicode text, UTF-8 text";
    }

    "data"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_utf8_matches_upstream_file_looks_utf8() {
        let cases: &[(&str, &[u8], Utf8Look)] = &[
            ("empty", b"", Utf8Look::Ascii),
            ("plain ascii", b"abc", Utf8Look::Ascii),
            ("nul is a control byte", b"a\0b", Utf8Look::Control),
            ("two-byte char", b"\xc3\xa9", Utf8Look::Multibyte),
            (
                "control wins over multibyte",
                b"\xc3\xa9\0",
                Utf8Look::Control,
            ),
            ("esc is text", b"\xc3\xa9\x1b", Utf8Look::Multibyte),
            ("lone 0xff", b"\xff", Utf8Look::Invalid),
            ("continuation byte first", b"\x80", Utf8Look::Invalid),
            (
                "truncated lead byte at end is ascii",
                b"\xc3",
                Utf8Look::Ascii,
            ),
            (
                "truncated lead byte after text is ascii",
                b"ab\xe2\x82",
                Utf8Look::Ascii,
            ),
            ("overlong 0xc0", b"\xc0\x80", Utf8Look::Invalid),
            ("overlong three-byte", b"\xe0\x80\x80", Utf8Look::Invalid),
            ("utf-16 surrogate", b"\xed\xa0\x80", Utf8Look::Invalid),
            ("above U+10FFFF", b"\xf4\x90\x80\x80", Utf8Look::Invalid),
            ("four-byte emoji", b"\xf0\x9f\x98\x80", Utf8Look::Multibyte),
            ("bad continuation", b"\xc3\x41", Utf8Look::Invalid),
        ];
        for (label, input, expected) in cases {
            assert_eq!(looks_utf8(input), *expected, "case {label:?}");
        }
    }

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
    fn classifies_valid_non_ascii_utf8_as_unicode_utf8_text() {
        let cases: &[(&str, &[u8])] = &[
            ("accented latin", "caf\u{e9} r\u{e9}sum\u{e9}\n".as_bytes()),
            ("bom prefix", &[0xEF, 0xBB, 0xBF, b'h', b'i']),
            ("multi-byte cjk", "\u{4f60}\u{597d}\n".as_bytes()),
        ];
        for (label, input) in cases {
            assert_eq!(
                classify_fallback(input),
                "Unicode text, UTF-8 text",
                "case {label:?} should classify as Unicode text, UTF-8 text"
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
