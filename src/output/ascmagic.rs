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
//! This ports the byte-level subset of GNU `file`'s `ascmagic.c` and
//! `encoding.c`: ASCII, UTF-8, ISO-8859 and non-ISO extended-ASCII
//! classification, plus the line-terminator, long-line, escape and
//! overstrike qualifiers. Still out of scope: UTF-16/UTF-32, UTF-7, EBCDIC
//! and BOM stripping; such buffers classify as `"data"`.

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

/// Classify a buffer with the text/data fallback described in the module
/// doc, following upstream `file_encoding` order.
///
/// Returns one of `"empty"`, `"ASCII text"`, `"Unicode text, UTF-8 text"`,
/// `"ISO-8859 text"`, `"Non-ISO extended-ASCII text"`, or `"data"`. This is
/// intentionally infallible -- there is no input for which classification
/// can fail, so the caller never needs to handle an error path here.
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

    // Upstream `looks_latin1` then `looks_extended`.
    if buffer
        .iter()
        .all(|&b| matches!(text_char_class(b), TextClass::Text | TextClass::Latin1))
    {
        return "ISO-8859 text";
    }
    if buffer
        .iter()
        .all(|&b| text_char_class(b) != TextClass::Binary)
    {
        return "Non-ISO extended-ASCII text";
    }

    "data"
}

/// Upstream `MAXLINELEN` (`ascmagic.c`): longest line not reported as long.
const MAXLINELEN: usize = 300;
/// Upstream `FILE_BYTES_MAX` (`file.h`): how much of a file `file` reads.
const BYTES_MAX: usize = 1_048_576;
/// Upstream `FILE_ENCODING_MAX` (`file.h`): how much of that read
/// `file_encoding` and the text pass inspect.
const ENCODING_MAX: usize = 65_536;

/// The views of a buffer that GNU `file`'s text handling works on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextWindow<'a> {
    /// First `ENCODING_MAX` bytes of the read after trailing NULs are
    /// trimmed (`file_ascmagic`'s `trim_nuls`): what is classified, scanned
    /// for qualifiers, and evaluated by the text pass.
    pub(crate) scan: &'a [u8],
    /// First `ENCODING_MAX` bytes of the untrimmed read: what
    /// `file_buffer`'s `looks_text` hint for the `/b` and `/t` skips sees.
    pub(crate) hint: &'a [u8],
}

/// Upstream `trim_nuls`: drop trailing NULs, keeping at least one byte.
fn trim_nuls(buf: &[u8]) -> &[u8] {
    let mut len = buf.len();
    while len > 1 && buf.get(len - 1) == Some(&0) {
        len -= 1;
    }
    buf.get(..len).unwrap_or(buf)
}

/// The views of `buffer` the text classification is computed over.
///
/// `file` reads at most `BYTES_MAX` bytes and inspects at most
/// `ENCODING_MAX` of them for text, so looking further would both diverge
/// from it and cost a full scan on every evaluation.
#[must_use]
pub(crate) fn text_window(buffer: &[u8]) -> TextWindow<'_> {
    let read = buffer.get(..BYTES_MAX).unwrap_or(buffer);
    let trimmed = trim_nuls(read);
    TextWindow {
        scan: trimmed.get(..ENCODING_MAX).unwrap_or(trimmed),
        hint: read.get(..ENCODING_MAX).unwrap_or(read),
    }
}

/// The bytes the text pass evaluates rules against.
///
/// `file_ascmagic` runs its softmagic pass over `encode_utf8(ubuf)`, so
/// a single-byte class (`ISO-8859 text`, `Non-ISO extended-ASCII text`)
/// is first widened byte-for-code-point to UTF-8; ASCII and UTF-8 windows
/// are already in that form. Multi-byte encodings are #524.
pub(crate) fn text_pass_buffer<'a>(scan: &'a [u8], class: &str) -> std::borrow::Cow<'a, [u8]> {
    if !matches!(class, "ISO-8859 text" | "Non-ISO extended-ASCII text") {
        return std::borrow::Cow::Borrowed(scan);
    }
    let mut out = Vec::with_capacity(scan.len() * 2);
    for &b in scan {
        if b < 0x80 {
            out.push(b);
        } else {
            out.push(0xC0 | (b >> 6));
            out.push(0x80 | (b & 0x3F));
        }
    }
    std::borrow::Cow::Owned(out)
}
/// Code point of the X3.64 "next line" character.
const NEL: u32 = 0x85;

/// Port of the qualifier scan in `ascmagic.c::file_ascmagic_with_encoding`.
///
/// Returns the text-class suffix (each piece begins with `, with`), or an
/// empty string when nothing applies. Scans Unicode scalar values when
/// [`looks_utf8`] reports multi-byte UTF-8, otherwise bytes. A lone
/// trailing CR counts for nothing: file 5.45 dropped 5.41's post-loop
/// `seen_cr` flush.
pub(crate) fn text_qualifiers(text: &[u8]) -> String {
    let utf8 = match (looks_utf8(text), std::str::from_utf8(text)) {
        (Utf8Look::Multibyte, Ok(s)) => Some(s),
        _ => None,
    };
    match utf8 {
        Some(s) => scan_qualifiers(s.chars().map(u32::from)),
        None => scan_qualifiers(text.iter().map(|&b| u32::from(b))),
    }
}

fn scan_qualifiers(code_points: impl Iterator<Item = u32>) -> String {
    let (mut n_crlf, mut n_cr, mut n_lf, mut n_nel) = (0_usize, 0_usize, 0_usize, 0_usize);
    let (mut has_escapes, mut has_backspace, mut seen_cr) = (false, false, false);
    let mut longest = 0_usize;
    // Index of the first char of the current line (upstream last_line_end + 1).
    let mut line_start = 0_usize;
    for (i, c) in code_points.enumerate() {
        if c == u32::from(b'\n') {
            if seen_cr {
                n_crlf += 1;
            } else {
                n_lf += 1;
            }
            line_start = i + 1;
        } else if seen_cr {
            n_cr += 1;
        }
        seen_cr = c == u32::from(b'\r');
        if seen_cr {
            line_start = i + 1;
        }
        if c == NEL {
            n_nel += 1;
            line_start = i + 1;
        }
        let line_len = i + 1 - line_start;
        if line_len > MAXLINELEN {
            longest = longest.max(line_len);
        }
        has_escapes |= c == 0x1B;
        has_backspace |= c == 0x08;
    }
    let mut out = String::new();
    if longest > 0 {
        out.push_str(", with very long lines (");
        out.push_str(&longest.to_string());
        out.push(')');
    }
    let none = n_crlf == 0 && n_cr == 0 && n_nel == 0 && n_lf == 0;
    if none || n_crlf != 0 || n_cr != 0 || n_nel != 0 {
        let kinds: Vec<&str> = [(n_crlf, "CRLF"), (n_cr, "CR"), (n_lf, "LF"), (n_nel, "NEL")]
            .iter()
            .filter(|(n, _)| *n != 0)
            .map(|&(_, name)| name)
            .collect();
        out.push_str(", with ");
        out.push_str(&if none {
            "no".to_string()
        } else {
            kinds.join(", ")
        });
        out.push_str(" line terminators");
    }
    if has_escapes {
        out.push_str(", with escape sequences");
    }
    if has_backspace {
        out.push_str(", with overstriking");
    }
    out
}

/// Port of file 5.45's description rewrite: swap a trailing ` text` (or
/// ` text executable`) for the text class, then append `qualifiers`.
pub(crate) fn append_text_class(desc: &str, class: &str, qualifiers: &str) -> String {
    let mut head = if desc.is_empty() {
        class.to_string()
    } else if let Some(base) = desc.strip_suffix(" text") {
        format!("{base}, {class}")
    } else if let Some(base) = desc.strip_suffix(" text executable") {
        format!("{base}, {class} executable")
    } else {
        format!("{desc}, {class}")
    };
    head.push_str(qualifiers);
    head
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_window_trims_trailing_nuls_then_caps_at_encoding_max() {
        let big = vec![b'a'; BYTES_MAX + 10];
        let w = text_window(&big);
        assert_eq!((w.scan.len(), w.hint.len()), (ENCODING_MAX, ENCODING_MAX));

        let padded = [vec![b'a'; 70000], vec![0; 10]].concat();
        assert_eq!(text_window(&padded).scan.len(), ENCODING_MAX);

        let w = text_window(b"ab\0\0");
        assert_eq!((w.scan, w.hint), (&b"ab"[..], &b"ab\0\0"[..]));

        // trim_nuls keeps at least one byte.
        assert_eq!(text_window(b"\0\0").scan, b"\0");
        assert_eq!(text_window(b"").scan, b"");
    }

    #[test]
    fn text_pass_buffer_widens_single_byte_classes_to_utf8() {
        let cases: &[(&str, &str, &[u8], &[u8])] = &[
            (
                "latin1 byte becomes two bytes",
                "ISO-8859 text",
                b"a\xffb",
                b"a\xc3\xbfb",
            ),
            (
                "c1 byte too",
                "Non-ISO extended-ASCII text",
                b"\x85",
                b"\xc2\x85",
            ),
            ("ascii untouched", "ASCII text", b"a\xffb", b"a\xffb"),
            (
                "utf8 untouched",
                "Unicode text, UTF-8 text",
                b"\xc3\xa9",
                b"\xc3\xa9",
            ),
        ];
        for (label, class, input, expected) in cases {
            assert_eq!(
                &*text_pass_buffer(input, class),
                *expected,
                "case {label:?}"
            );
        }
    }

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

    #[test]
    fn classifies_8bit_text_per_file_encoding_order() {
        let cases: &[(&str, &[u8], &str)] = &[
            ("latin1 high byte", b"hello\xff\n", "ISO-8859 text"),
            (
                "only C1 control (0x90)",
                b"hi\x90\n",
                "Non-ISO extended-ASCII text",
            ),
            (
                "latin1 plus C1 mix",
                &[0xC0, 0x80],
                "Non-ISO extended-ASCII text",
            ),
            (
                "all C1",
                &[0x80, 0x81, 0x82, 0x83],
                "Non-ISO extended-ASCII text",
            ),
            ("NUL stays data", b"hi\xff\x00\n", "data"),
        ];
        for (label, input, expected) in cases {
            assert_eq!(classify_fallback(input), *expected, "case {label:?}");
        }
    }

    #[test]
    fn text_qualifiers_match_upstream_scan() {
        // Long-line N: ll = i - last_line_end, fires when ll > 300, so a
        // line of L >= 301 chars reports L (first line: last_line_end = -1).
        let long_nt = b"a".repeat(301);
        let long_lf = [b"a".repeat(301), b"\n".to_vec()].concat();
        let edge300 = b"a".repeat(300);
        let combined = [b"a".repeat(301), b"\r\n\x1b\x08".to_vec()].concat();
        let utf8_301 = "\u{e9}".as_bytes().repeat(301);
        let utf8_200 = "\u{e9}".as_bytes().repeat(200);
        let cases: &[(&str, &[u8], &str)] = &[
            ("LF only", b"a\nb\n", ""),
            ("no terminator", b"abc", ", with no line terminators"),
            ("CRLF", b"a\r\nb\r\n", ", with CRLF line terminators"),
            (
                "CR then LF separately",
                b"a\rb\n",
                ", with CR, LF line terminators",
            ),
            (
                "lone trailing CR counts for nothing (file 5.45)",
                b"a\r",
                ", with no line terminators",
            ),
            ("NEL alone", b"a\x85b", ", with NEL line terminators"),
            ("escape", b"a\x1bb\n", ", with escape sequences"),
            ("backspace", b"a\x08b\n", ", with overstriking"),
            (
                "300 bytes is not long",
                &edge300,
                ", with no line terminators",
            ),
            (
                "301 bytes no terminator: N=301",
                &long_nt,
                ", with very long lines (301), with no line terminators",
            ),
            (
                "301 bytes then LF: N=301",
                &long_lf,
                ", with very long lines (301)",
            ),
            (
                "upstream order",
                &combined,
                ", with very long lines (301), with CRLF line terminators, \
                 with escape sequences, with overstriking",
            ),
            (
                "utf8 counts code points: 301 chars",
                &utf8_301,
                ", with very long lines (301), with no line terminators",
            ),
            (
                "utf8 counts code points: 200 chars",
                &utf8_200,
                ", with no line terminators",
            ),
        ];
        for (label, input, expected) in cases {
            assert_eq!(text_qualifiers(input), *expected, "case {label:?}");
        }
    }

    #[test]
    fn append_text_class_rewrites_description_like_file_545() {
        let cases: &[(&str, &str, &str, &str)] = &[
            ("", "ASCII text", "", "ASCII text"),
            ("c program text", "ASCII text", "", "c program, ASCII text"),
            (
                "POSIX shell script text executable",
                "ASCII text",
                "",
                "POSIX shell script, ASCII text executable",
            ),
            (
                "GEDCOM genealogy text version 5.5",
                "ASCII text",
                "",
                "GEDCOM genealogy text version 5.5, ASCII text",
            ),
            ("foo text text", "ASCII text", "", "foo text, ASCII text"),
            ("contexts", "ASCII text", "", "contexts, ASCII text"),
            (
                "c program text",
                "ASCII text",
                ", with CRLF line terminators",
                "c program, ASCII text, with CRLF line terminators",
            ),
        ];
        for (desc, class, quals, expected) in cases {
            assert_eq!(
                append_text_class(desc, class, quals),
                *expected,
                "desc {desc:?}"
            );
        }
    }
}
