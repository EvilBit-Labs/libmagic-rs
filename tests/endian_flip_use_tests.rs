// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end coverage for the magic(5) `use \^name` endian-flip flag
//! (issue #236, libmagic `softmagic.c` `cvt_flip`).
//!
//! A `use \^name` invocation flips the little/big endianness of every
//! endian-bearing read inside the invoked subroutine, and the flip
//! propagates into nested plain `use` calls. This is what lets the system
//! `images` magic parse a **big-endian** TIFF with a subroutine whose reads
//! are all declared `leshort`:
//!
//! ```text
//! 0      string  MM\x00\x2a   TIFF image data, big-endian
//! >(4.L) use     \^tiff_ifd
//! 0      string  II\x2a\x00   TIFF image data, little-endian
//! >(4.l) use     tiff_ifd
//! 0      name    tiff_ifd
//! >0     leshort x            \b, direntries=%d
//! >2     use     tiff_entry
//! ```
//!
//! Before the fix rmagic dropped the `\^` prefix, so a big-endian TIFF read
//! `direntries` (and every `tiff_entry` tag) with little-endian byte order,
//! producing byte-swapped garbage (`direntries=22` -> `5632`) and no
//! width/height/compression fields.
//!
//! These tests use a synthetic TIFF-like structure that mirrors the real
//! magic's shape (an indirect `(4.L)`/`(4.l)` pointer into a subroutine that
//! reads `leshort`, with a nested plain `use` one level down) so the flip and
//! its propagation are pinned hermetically, independent of the system magic
//! database or any on-disk sample file.

#![allow(clippy::expect_used)]

use std::io::Write;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::NamedTempFile;

/// Magic mirroring the real `images` TIFF shape: a big-endian header uses
/// `\^ifd` (flip), a little-endian header uses plain `ifd` (no flip). The
/// `ifd` subroutine reads `leshort` for direntries and then invokes a nested
/// plain `use inner` that reads another `leshort` -- proving the flip reaches
/// the nested subroutine.
const TIFF_LIKE_MAGIC: &str = "0\tstring\tMM\\x00\\x2a\tBE-TIFF\n\
     >(4.L)\tuse\t\\^ifd\n\
     0\tstring\tII\\x2a\\x00\tLE-TIFF\n\
     >(4.l)\tuse\tifd\n\
     0\tname\tifd\n\
     >0\tleshort\tx\t\\b, direntries=%d\n\
     >2\tuse\tinner\n\
     0\tname\tinner\n\
     >0\tleshort\tx\t\\b, tag=%d\n";

fn db_from_magic(magic: &str) -> MagicDatabase {
    let mut f = NamedTempFile::new().expect("temp magic file");
    f.write_all(magic.as_bytes()).expect("write magic");
    f.flush().expect("flush magic");
    MagicDatabase::load_from_file_with_config(f.path(), EvaluationConfig::default())
        .expect("magic must load")
}

/// A 12-byte big-endian TIFF-like buffer:
/// - `MM\x00\x2a` header, then a big-endian 4-byte IFD pointer (= 8)
/// - at offset 8: big-endian `direntries` = 3 (`00 03`)
/// - at offset 10: big-endian `tag` = 0x0100 = 256 (`01 00`)
///
/// Read with little-endian byte order (the un-flipped bug) these would be
/// 0x0300 = 768 and 0x0001 = 1 respectively.
const BE_TIFF: &[u8] = &[
    0x4D, 0x4D, 0x00, 0x2A, // "MM\0*"
    0x00, 0x00, 0x00, 0x08, // BE IFD offset = 8
    0x00, 0x03, // BE direntries = 3
    0x01, 0x00, // BE tag = 0x0100 = 256
];

/// The little-endian mirror: `II\x2a\x00`, LE pointer, LE direntries = 3
/// (`03 00`), LE tag = 0x0100 (`00 01`). Invoked via plain `use ifd` (no
/// flip), so the declared `leshort` reads are already correct.
const LE_TIFF: &[u8] = &[
    0x49, 0x49, 0x2A, 0x00, // "II*\0"
    0x08, 0x00, 0x00, 0x00, // LE IFD offset = 8
    0x03, 0x00, // LE direntries = 3
    0x00, 0x01, // LE tag = 0x0100 = 256
];

#[test]
fn flip_use_reads_big_endian_tiff_direntries_and_propagates_to_nested_use() {
    let db = db_from_magic(TIFF_LIKE_MAGIC);
    let result = db.evaluate_buffer(BE_TIFF).expect("evaluate BE TIFF");
    // `\^ifd` flips leshort->beshort for the direntries read, AND the nested
    // `>2 use inner` inherits the flip so its `tag` leshort is also read
    // big-endian. Without the fix this was `direntries=768, tag=1`.
    assert_eq!(
        result.description.as_str(),
        "BE-TIFF, direntries=3, tag=256",
        "`use \\^ifd` must read the big-endian subroutine (and its nested use) with flipped endianness"
    );
}

#[test]
fn plain_use_leaves_little_endian_tiff_unflipped() {
    // Negative control: the little-endian header uses plain `use ifd` (no
    // `\^`), so the declared `leshort` reads stay little-endian and produce
    // the same logical values. This guards against the flip leaking onto
    // non-`\^` invocations.
    let db = db_from_magic(TIFF_LIKE_MAGIC);
    let result = db.evaluate_buffer(LE_TIFF).expect("evaluate LE TIFF");
    assert_eq!(
        result.description.as_str(),
        "LE-TIFF, direntries=3, tag=256",
        "a plain `use` must NOT flip endianness"
    );
}

#[test]
fn nested_flip_use_toggles_back_to_unflipped() {
    // A `\^use` nested inside an already-flipped subroutine toggles the flip
    // OFF again (libmagic's `flip = !flip`), so the innermost reads use their
    // declared endianness. Here the outer BE header enters `\^outer` (flip
    // on), and `outer` invokes `\^middle` (flip off again): `middle`'s
    // `leshort` must read little-endian.
    let magic = "0\tstring\tZZ\tROOT\n\
         >2\tuse\t\\^outer\n\
         0\tname\touter\n\
         >0\tuse\t\\^middle\n\
         0\tname\tmiddle\n\
         >0\tleshort\tx\t\\b, v=%d\n";
    let db = db_from_magic(magic);
    // Signature "ZZ", then at offset 2 the value bytes `03 00`. With the flip
    // toggled twice (on then off) the read is little-endian: 0x0003 = 3.
    // A single (un-toggled) flip would read big-endian 0x0300 = 768.
    let buf: &[u8] = &[0x5A, 0x5A, 0x03, 0x00];
    let result = db
        .evaluate_buffer(buf)
        .expect("evaluate nested-flip buffer");
    assert_eq!(
        result.description.as_str(),
        "ROOT, v=3",
        "a `\\^use` nested inside a flipped subroutine must toggle the flip back off"
    );
}
