// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `ascmagic` text classification port.

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

    // An even-length read trimmed to an odd length gets one byte back.
    assert_eq!(text_window(b"abc\0").scan, b"abc\0");
    assert_eq!(text_window(b"abc\0\0\0").scan, b"abc\0");
    assert_eq!(text_window(b"abcd\0").scan, b"abcd");

    // trim_nuls keeps at least one byte (two after the parity restore).
    assert_eq!(text_window(b"\0\0").scan, b"\0\0");
    assert_eq!(text_window(b"\0\0\0").scan, b"\0");
    assert_eq!(text_window(b"").scan, b"");
}

#[test]
fn text_pass_buffer_widens_single_byte_classes_to_utf8() {
    let cases: &[(&str, TextEncoding, &[u8], &[u8])] = &[
        (
            "latin1 byte becomes two bytes",
            TextEncoding::Latin1,
            b"a\xffb",
            b"a\xc3\xbfb",
        ),
        ("c1 byte too", TextEncoding::Extended, b"\x85", b"\xc2\x85"),
        ("pure ascii untouched", TextEncoding::Ascii, b"abc", b"abc"),
        (
            "ascii NEL byte widens",
            TextEncoding::Ascii,
            b"ab\x85cd",
            b"ab\xc2\x85cd",
        ),
        ("data untouched", TextEncoding::Data, b"a\xffb", b"a\xffb"),
        (
            "utf8 untouched",
            TextEncoding::Utf8,
            b"\xc3\xa9",
            b"\xc3\xa9",
        ),
        (
            "utf8 drops a sequence the window cut through",
            TextEncoding::Utf8,
            b"\xc3\xa9\xc3",
            b"\xc3\xa9",
        ),
    ];
    for (label, class, input, expected) in cases {
        assert_eq!(
            &*text_pass_buffer(input, *class),
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
    let utf8_cut = ["\u{e9}".as_bytes().repeat(301), b"\xc3".to_vec()].concat();
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
        (
            "utf8 cut mid-sequence still counts code points",
            &utf8_cut,
            ", with very long lines (301), with no line terminators",
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
