// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! The two-pass text classification and the `, <text class>` tail (issue
//! #382, GOTCHAS S13.7). Every expectation below was measured against GNU
//! `file` 5.41 (macOS) and 5.45 (Alpine) with inline magic; the golden rows
//! at the end pin the 5.45 description rewrite independently of the macOS
//! host, which does not perform it.

#![allow(clippy::expect_used)]

mod common;

use libmagic_rs::MagicDatabase;
use std::io::Write;
use tempfile::TempDir;

fn db(magic: &str) -> (TempDir, MagicDatabase) {
    let dir = TempDir::new().expect("test setup");
    let magic_dir = dir.path().join("magic");
    std::fs::create_dir_all(&magic_dir).expect("test setup");
    let path = magic_dir.join("inline");
    std::fs::File::create(&path)
        .expect("test setup")
        .write_all(magic.as_bytes())
        .expect("test setup");
    let db = MagicDatabase::load_from_file(&path).expect("test setup");
    (dir, db)
}

fn describe(magic: &str, buffer: &[u8]) -> String {
    let (_dir, db) = db(magic);
    db.evaluate_buffer(buffer).expect("test setup").description
}

const REGEX6: &str = "0 regex QQTEXT6 TOPMSG6\n";

/// The measured matrix: which pass an entry runs in, when the tail appears,
/// and which text class and qualifiers it carries.
#[test]
#[allow(clippy::too_many_lines)]
fn two_pass_matrix_matches_gnu_file() {
    let utf8 = "QQTEXT6 \u{e9}\n";
    // The shorter long line comes first so "longest" is not "first".
    let two_long = [
        vec![b'b'; 400],
        b"\nQQTEXT6".to_vec(),
        vec![b'a'; 493],
        b"\n".to_vec(),
    ]
    .concat();
    let cases: &[(&str, &str, &[u8], &str)] = &[
        (
            "plain string: BIN pass, no tail",
            "0 string QQTEXT5 TOPMSG5\n",
            b"QQTEXT5\n",
            "TOPMSG5",
        ),
        (
            "string/b on a text buffer is skipped",
            "0 string/b QQTEXT7 TOPMSG7\n",
            b"QQTEXT7\n",
            "ASCII text",
        ),
        (
            "string/t: TEXT pass, tail",
            "0 string/t QQTEXT4 TOPMSG4\n",
            b"QQTEXT4\n",
            "TOPMSG4, ASCII text",
        ),
        (
            "string/t on a binary buffer is skipped",
            "0 string/t QQTEXT4 TOPMSG4\n",
            b"QQTEXT4\0\x01",
            "data",
        ),
        (
            "regex: TEXT pass, tail",
            REGEX6,
            b"QQTEXT6\n",
            "TOPMSG6, ASCII text",
        ),
        (
            "regex entry with a byte child: child runs in the text pass",
            "0 regex QQTEXT2 TOPMSG2\n>7 byte x CHILD=%d\n",
            b"QQTEXT2A\n",
            "TOPMSG2 CHILD=65, ASCII text",
        ),
        (
            "search: TEXT pass, tail",
            "0 search/100 QQTEXT3 TOPMSG3\n",
            b"xxQQTEXT3\n",
            "TOPMSG3, ASCII text",
        ),
        (
            "string with NUL on binary: no tail",
            "0 string QQ\\0BIN TOPMSG8\n",
            b"QQ\0BIN\x01",
            "TOPMSG8",
        ),
        (
            "search with NUL pattern is BIN: no tail",
            "0 search/8 QQ\\0 TOPMSG9\n",
            b"QQ\0\x01",
            "TOPMSG9",
        ),
        (
            "message-less string gate with a regex child prints in pass 1: no tail",
            "0 string QQTEXT1\n>0 regex QQTEXT1 TEXTCHILD\n",
            b"QQTEXT1\n",
            "TEXTCHILD",
        ),
        (
            "belong entry with a string child: BIN pass, no tail",
            "0 belong 0x51515445 TOPMSGB\n>4 string XTB \\b, SUB\n",
            b"QQTEXTB\n",
            "TOPMSGB, SUB",
        ),
        (
            "UTF-8 buffer",
            REGEX6,
            utf8.as_bytes(),
            "TOPMSG6, Unicode text, UTF-8 text",
        ),
        (
            "CRLF qualifier",
            REGEX6,
            b"QQTEXT6\r\n",
            "TOPMSG6, ASCII text, with CRLF line terminators",
        ),
        (
            "Latin-1 buffer",
            REGEX6,
            b"QQTEXT6 \xff\n",
            "TOPMSG6, ISO-8859 text",
        ),
        (
            "escape qualifier",
            REGEX6,
            b"QQTEXT6\x1b\n",
            "TOPMSG6, ASCII text, with escape sequences",
        ),
        (
            "no match on text: class plus qualifiers",
            REGEX6,
            b"QQOTHER",
            "ASCII text, with no line terminators",
        ),
        (
            "top-level use is never evaluated",
            "0 use foo\n0 name foo\n>0 string QQ NAMEMSG\n",
            b"QQTEXT\n",
            "ASCII text",
        ),
        (
            "top-level default is never evaluated",
            "0 default x DEFMSG\n",
            b"QQTEXT\n",
            "ASCII text",
        ),
        (
            "top-level indirect is never evaluated",
            "0 indirect x INDMSG\n",
            b"QQTEXT\n",
            "ASCII text",
        ),
        (
            "one-byte buffer",
            REGEX6,
            b"x",
            "very short file (no magic)",
        ),
        ("empty buffer", REGEX6, b"", "empty"),
        (
            "empty buffer never reaches magic",
            "0 offset x OFFSETMSG\n",
            b"",
            "empty",
        ),
        (
            "text pass sees ASCII's NEL byte as its UTF-8 encoding",
            "0 string/t ab\\302\\205 NELMSG\n",
            b"ab\x85cd\n",
            "NELMSG, ASCII text, with LF, NEL line terminators",
        ),
        (
            "text pass runs over the UTF-8 widening of a Latin-1 buffer",
            "0 string/t \\303\\277 YMSG\n",
            b"\xff\n",
            "YMSG, ISO-8859 text",
        ),
        (
            "binary buffer with no match",
            REGEX6,
            b"\0\x01\x02\xff",
            "data",
        ),
        (
            "pass order beats strength order: the weaker byte entry prints first",
            "0 regex QQTEXTX TEXTMSG\n0 byte 0x51 BINMSG\n",
            b"QQTEXTX\n",
            "BINMSG",
        ),
        (
            "message-less gate whose child misses does not suppress the text pass",
            "0 string QQTEXTG\n>0 string ZZ NOPE\n0 regex QQTEXTG TEXTMSG\n",
            b"QQTEXTG\n",
            "TEXTMSG, ASCII text",
        ),
        (
            "message-less gate whose child misses prints nothing",
            "0 string QQTEXTG\n>0 string ZZ NOPE\n",
            b"QQTEXTG\n",
            "ASCII text",
        ),
        (
            "use body is unfiltered: its text regex runs in the binary pass",
            "0 name sub NAMEMSG\n>0 regex QQTEXTU BODYMSG\n0 byte x\n>0 use sub\n",
            b"QQTEXTU\n",
            "NAMEMSG BODYMSG",
        ),
        (
            "one-byte buffer even when a rule would match",
            "0 byte 0x78 BYTEMSG\n",
            b"x",
            "very short file (no magic)",
        ),
        (
            "regex with a parsed high byte is a binary entry: no tail",
            "0 regex QQTEXTH\\377 HB\n",
            b"QQTEXTH\xff\n",
            "HB",
        ),
        (
            "search with a parsed high byte is a binary entry: no tail",
            "0 search/8 QQTEXTS\\377 SB\n",
            b"QQTEXTS\xff\n",
            "SB",
        ),
        (
            "two long lines report the longest",
            REGEX6,
            &two_long,
            "TOPMSG6, ASCII text, with very long lines (500)",
        ),
        (
            "extended-ASCII buffer",
            "0 regex QQTEXTE EMSG\n",
            b"QQTEXTE\x81\n",
            "EMSG, Non-ISO extended-ASCII text",
        ),
        (
            "top-level offset is a binary entry",
            "0 offset x OFFSETMSG\n",
            b"QQTEXTO\n",
            "OFFSETMSG",
        ),
        (
            "top-level clear is never evaluated",
            "0 clear x CLRMSG\n",
            b"QQTEXTC\n",
            "ASCII text",
        ),
    ];
    for (label, magic, buffer, expected) in cases {
        assert_eq!(describe(magic, buffer), *expected, "case {label:?}");
    }
}

/// `FILE_BYTES_MAX` is 7 MiB since file 5.44 (1 MiB through 5.43). The
/// trailing NULs of a 1 MiB + 1 read are not trailing once the read extends
/// past them, so a 5.44+ host classifies the window as `data`; a 5.41 host
/// (measured) stops reading at 1 MiB and prints `TOPMSG6, ASCII text`. The
/// 7 MiB + 1 row pins the upper edge the same way.
#[test]
fn the_read_cap_is_file_544s_7_mib() {
    const MIB: usize = 1_048_576;
    let padded = |len: usize| [b"QQTEXT6\n".to_vec(), vec![0; len - 8], b"b".to_vec()].concat();
    assert_eq!(describe(REGEX6, &padded(MIB)), "data", "1 MiB + 1");
    assert_eq!(
        describe(REGEX6, &padded(7 * MIB)),
        "TOPMSG6, ASCII text",
        "7 MiB + 1"
    );
}

/// `file_ascmagic` trims trailing NULs from the read before classifying, and
/// `file_encoding` plus the text pass inspect only the first
/// `FILE_ENCODING_MAX` (64 KiB) bytes of that trimmed read, while the `/b`
/// `/t` hint comes from the untrimmed bytes. Measured on file-5.41.
#[test]
fn trailing_nuls_and_the_64k_encoding_window_match_gnu_file() {
    let search = "0 search/100000 QQTEXTS SMSG\n";
    let long_line = [vec![b'a'; 70000], b"\n".to_vec()].concat();
    let nuls_after_64k = [b"hello\n".to_vec(), vec![b' '; 65540], vec![0; 4]].concat();
    let hit_after_64k = [vec![b'x'; 70000], b"QQTEXTS\n".to_vec()].concat();
    let hit_before_64k = [vec![b'x'; 60000], b"QQTEXTS\n".to_vec()].concat();
    let cjk_cut = ["\u{4e00}".as_bytes().repeat(25000), b"\n".to_vec()].concat();
    let cases: &[(&str, &str, &[u8], &str)] = &[
        (
            "trailing NULs are trimmed before the text pass",
            REGEX6,
            b"QQTEXT6\n\0\0\0",
            "TOPMSG6, ASCII text",
        ),
        (
            "string/t is still skipped: the hint sees the untrimmed NUL",
            "0 string/t QQTEXT4 TOPMSG4\n",
            b"QQTEXT4\n\0\0",
            "ASCII text",
        ),
        (
            "no match, trailing NULs",
            REGEX6,
            b"hello\n\0\0",
            "ASCII text",
        ),
        (
            "two bytes then NUL",
            REGEX6,
            b"ab\0",
            "ASCII text, with no line terminators",
        ),
        ("embedded NUL stays binary", REGEX6, b"ab\0cd", "data"),
        (
            "long line is measured over the 64 KiB window",
            REGEX6,
            &long_line,
            "ASCII text, with very long lines (65536), with no line terminators",
        ),
        (
            "NULs after 64 KiB are trimmed, then the window applies",
            REGEX6,
            &nuls_after_64k,
            "ASCII text, with very long lines (65530)",
        ),
        (
            "text pass does not see past 64 KiB",
            search,
            &hit_after_64k,
            "ASCII text, with very long lines (65536), with no line terminators",
        ),
        (
            "text pass sees a hit before 64 KiB",
            search,
            &hit_before_64k,
            "SMSG, ASCII text, with very long lines (60007)",
        ),
        (
            "a 3-byte sequence cut by the window is dropped: 21845 code points",
            REGEX6,
            &cjk_cut,
            "Unicode text, UTF-8 text, with very long lines (21845), with no line terminators",
        ),
    ];
    for (label, magic, buffer, expected) in cases {
        assert_eq!(describe(magic, buffer), *expected, "case {label:?}");
    }
}

/// Measured on file-5.45 (Alpine via Docker, 2026-10-04) with inline magic
/// mirroring the system DB's c-lang, shell, and sgml shapes. These pin the
/// ` text` / ` text executable` rewrite, which the macOS 5.41 build skips.
#[test]
fn linux_golden_rows_pin_the_description_rewrite() {
    let cases: &[(&str, &str, &[u8], &str)] = &[
        (
            "C source",
            "0 search/8192 #include C source text\n",
            b"#include <stdio.h>\nint main(void) { return 0; }\n",
            "C source, ASCII text",
        ),
        (
            "shell script",
            "0 string/t #!/bin/sh POSIX shell script text executable\n",
            b"#!/bin/sh\necho hi\n",
            "POSIX shell script, ASCII text executable",
        ),
        (
            "XML",
            "0 string/t \\<?xml XML 1.0 document text\n",
            b"<?xml version=\"1.0\"?>\n<a/>\n",
            "XML 1.0 document, ASCII text",
        ),
    ];
    for (label, magic, buffer, expected) in cases {
        assert_eq!(describe(magic, buffer), *expected, "golden row {label:?}");
    }
}

/// The same inline magic through the host's `file`, compared after the
/// shared rewrite normalizer. Buffers use the `QQTEXT` prefix, which matches
/// no system rule on either measured host.
#[test]
fn oracle_rows_agree_with_host_file_after_normalization() {
    use common::magic_oracle::{
        file_honors_magic_dir, file_says, has_file_binary, normalize_text_class_rewrite, skip,
    };
    if !has_file_binary() {
        skip("`file` is not installed");
        return;
    }
    let magic = "0 string/t QQTEXT4 TOPMSG4\n\
                 0 regex QQTEXT6 TOPMSG6\n\
                 0 regex QQTEXT2 TOPMSG2\n>7 byte x CHILD=%d\n\
                 0 string QQTEXT5 TOPMSG5\n\
                 0 string/t QQTEXTS POSIX shell script text executable\n\
                 0 search/8192 QQTEXTC C source text\n";
    let utf8 = "QQTEXT6 \u{e9}\n";
    let buffers: &[(&str, &[u8])] = &[
        ("string_t", b"QQTEXT4\n"),
        ("regex", b"QQTEXT6\n"),
        ("regex_child", b"QQTEXT2A\n"),
        ("plain_string", b"QQTEXT5\n"),
        ("crlf", b"QQTEXT6\r\n"),
        ("utf8", utf8.as_bytes()),
        ("latin1", b"QQTEXT6 \xff\n"),
        ("escape", b"QQTEXT6\x1b\n"),
        // A `regex` spanning the whole buffer does not match in `file` (pre-existing
        // regex-window divergence, tracked separately); `string/t` does.
        ("no_terminator", b"QQTEXT4"),
        ("shell_exec", b"QQTEXTS\necho\n"),
        ("c_source", b"x QQTEXTC\n"),
        ("one_byte", b"x"),
    ];
    let (dir, db) = db(magic);
    let magic_dir = dir.path().join("magic");
    if let Err(reason) = file_honors_magic_dir(&magic_dir) {
        skip(&format!(
            "`file` rejects MAGIC={}: {reason}",
            magic_dir.display()
        ));
        return;
    }
    for (name, buffer) in buffers {
        let target = dir.path().join(name);
        std::fs::write(&target, buffer).expect("test setup");
        let ours = db.evaluate_buffer(buffer).expect("test setup").description;
        let theirs = normalize_text_class_rewrite(&file_says(
            &magic_dir,
            target.to_str().expect("test setup"),
        ));
        assert_eq!(ours, theirs, "oracle row {name:?}");
    }
}
