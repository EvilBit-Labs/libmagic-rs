// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for indirect offset parsing and evaluation
//!
//! Exercises the full pipeline: write a magic file with indirect-offset syntax,
//! load it through `MagicDatabase::load_from_file()`, evaluate buffers, and
//! assert correct match / no-match behavior.
//!
//! GNU `file` semantics: lowercase specifiers are little-endian, uppercase are
//! big-endian. Pointer types are signed by default (GOTCHAS S6.3).
//! Adjustment is parsed after the closing paren: `(base.type)+adj`.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(clippy::indexing_slicing)]

use std::fs;
use std::io::Write;

use libmagic_rs::MagicDatabase;
use tempfile::TempDir;

/// Build a PE-like buffer where offset 0x3c holds a little-endian 4-byte pointer
/// to the PE signature (`PE\0\0`).
///
/// Layout:
///   [0x00] "MZ" DOS header stub
///   [0x3c] 4-byte little-endian pointer -> 0x80 (PE header location)
///   [0x80] "PE\0\0" signature
fn build_pe_like_buffer() -> Vec<u8> {
    let mut buf = vec![0u8; 0x84];
    // DOS stub magic
    buf[0] = b'M';
    buf[1] = b'Z';
    // Little-endian pointer at 0x3c -> 0x80
    buf[0x3c] = 0x80;
    buf[0x3d] = 0x00;
    buf[0x3e] = 0x00;
    buf[0x3f] = 0x00;
    // PE signature at 0x80
    buf[0x80] = b'P';
    buf[0x81] = b'E';
    buf[0x82] = 0x00;
    buf[0x83] = 0x00;
    buf
}

#[test]
fn test_indirect_offset_pe_detection_via_magic_file() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("pe.magic");

    // Use lowercase .l (little-endian long) -- GNU `file` semantics.
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"0 string "MZ" DOS executable"#).unwrap();
    writeln!(f, r#">(0x3c.l) string "PE" (PE)"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    let buf = build_pe_like_buffer();
    let result = db.evaluate_buffer(&buf).unwrap();

    assert!(
        result.description.contains("DOS executable"),
        "Expected DOS executable match, got: {}",
        result.description
    );
    assert!(
        result.description.contains("(PE)"),
        "Expected PE child match via indirect offset, got: {}",
        result.description
    );
}

#[test]
fn test_indirect_offset_no_match_when_pointer_out_of_bounds() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("pe.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"0 string "MZ" DOS executable"#).unwrap();
    writeln!(f, r#">(0x3c.l) string "PE" (PE)"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Buffer has "MZ" but the LE pointer at 0x3c points beyond the buffer
    let mut buf = vec![0u8; 0x40];
    buf[0] = b'M';
    buf[1] = b'Z';
    // Little-endian pointer at 0x3c -> 0xFF (beyond buffer length)
    buf[0x3c] = 0xFF;
    buf[0x3d] = 0x00;
    buf[0x3e] = 0x00;
    buf[0x3f] = 0x00;

    let result = db.evaluate_buffer(&buf).unwrap();

    // The parent "MZ" rule should still match
    assert!(
        result.description.contains("DOS executable"),
        "Expected DOS match even when child fails, got: {}",
        result.description
    );
    // But the PE child should NOT match (pointer out of bounds)
    assert!(
        !result.description.contains("(PE)"),
        "PE child should not match when pointer is out of bounds, got: {}",
        result.description
    );
}

#[test]
fn test_indirect_offset_with_adjustment_after_paren() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("adj.magic");

    // Adjustment AFTER closing paren: (base.type)+adj
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"(0.l)+4 string "MAGIC" Adjusted match"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // LE pointer at offset 0 = 0x06 (little-endian), +4 = 10, "MAGIC" at offset 10
    let mut buf = vec![0u8; 20];
    buf[0] = 0x06;
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x00;
    buf[10] = b'M';
    buf[11] = b'A';
    buf[12] = b'G';
    buf[13] = b'I';
    buf[14] = b'C';

    let result = db.evaluate_buffer(&buf).unwrap();
    assert!(
        result.description.contains("Adjusted match"),
        "Expected adjusted indirect match, got: {}",
        result.description
    );
}

#[test]
fn test_indirect_offset_byte_specifier() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("byte_ptr.magic");

    // Use .b (byte pointer): read 1 byte at offset 0, use as offset
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"(0.b) string "OK" Byte pointer match"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Byte at offset 0 = 5, so check for "OK" at offset 5
    let mut buf = vec![0u8; 10];
    buf[0] = 5;
    buf[5] = b'O';
    buf[6] = b'K';

    let result = db.evaluate_buffer(&buf).unwrap();
    assert!(
        result.description.contains("Byte pointer match"),
        "Expected byte pointer match, got: {}",
        result.description
    );
}

#[test]
fn test_indirect_offset_loading_does_not_error() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("load.magic");

    // Verify the parsing path succeeds for all specifier variants
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"(0.b) string "A" byte LE ptr"#).unwrap();
    writeln!(f, r#"(0.B) string "A" Byte LE ptr"#).unwrap();
    writeln!(f, r#"(0.s) string "A" short LE ptr"#).unwrap();
    writeln!(f, r#"(0.S) string "A" short BE ptr"#).unwrap();
    writeln!(f, r#"(0.l) string "A" long LE ptr"#).unwrap();
    writeln!(f, r#"(0.L) string "A" long BE ptr"#).unwrap();
    writeln!(f, r#"(0.q) string "A" quad LE ptr"#).unwrap();
    writeln!(f, r#"(0.Q) string "A" quad BE ptr"#).unwrap();

    let result = MagicDatabase::load_from_file(&magic_path);
    assert!(
        result.is_ok(),
        "Loading magic file with all indirect specifiers should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_indirect_offset_child_with_adjustment_after_paren() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("pe_adj.magic");

    // Child rule with (base.type)+adj syntax
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"0 string "MZ" DOS executable"#).unwrap();
    writeln!(f, r#">(0x3c.l)+4 string "PE" (PE+4)"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // LE pointer at 0x3c = 0x7C, +4 = 0x80, "PE" at 0x80
    let mut buf = vec![0u8; 0x84];
    buf[0] = b'M';
    buf[1] = b'Z';
    buf[0x3c] = 0x7C;
    buf[0x3d] = 0x00;
    buf[0x3e] = 0x00;
    buf[0x3f] = 0x00;
    buf[0x80] = b'P';
    buf[0x81] = b'E';

    let result = db.evaluate_buffer(&buf).unwrap();
    assert!(
        result.description.contains("DOS executable"),
        "Expected DOS match, got: {}",
        result.description
    );
    assert!(
        result.description.contains("(PE+4)"),
        "Expected child match with adjustment, got: {}",
        result.description
    );
}

/// A hand-built spec whose pointer type and outer endian disagree is rejected
/// instead of being read with whichever field the resolver happened to use.
#[test]
fn test_indirect_pointer_endianness_mismatch_is_an_error() {
    use libmagic_rs::evaluator::offset::resolve_offset;
    use libmagic_rs::parser::ast::{Endianness, IndirectAdjustmentOp, OffsetSpec, TypeKind};

    let mismatched = |pointer_type: TypeKind| OffsetSpec::Indirect {
        base_offset: 0,
        base_relative: false,
        pointer_type,
        adjustment: 0,
        adjustment_op: IndirectAdjustmentOp::Add,
        result_relative: false,
        endian: Endianness::Little,
    };
    let buffer = [0u8; 16];
    let big = Endianness::Big;

    for typ in [
        TypeKind::Short {
            endian: big,
            signed: true,
        },
        TypeKind::Long {
            endian: big,
            signed: true,
        },
        TypeKind::Id3 { endian: big },
        TypeKind::Quad {
            endian: big,
            signed: true,
        },
    ] {
        assert!(
            resolve_offset(&mismatched(typ.clone()), &buffer).is_err(),
            "mismatched endianness on {typ:?} must be rejected"
        );
    }
}

/// ID3 synchsafe pointers (`i`/`I`, issue #237). The tag size bytes decode to
/// 2084 as synchsafe but read as 4132 as a plain long, so a sentinel at each
/// `size + 10` target shows which offset the pointer resolved to.
#[test]
fn test_id3_pointer_resolves_synchsafe_offset() {
    const SYNCHSAFE_TARGET: usize = 2084 + 10;
    const PLAIN_LONG_TARGET: usize = 4132 + 10;

    let cases: &[(&str, &str, [u8; 4], &str)] = &[
        (
            "`I` big-endian",
            "6.I+10",
            [0x00, 0x00, 0x10, 0x24],
            "Tag at=0xaa",
        ),
        (
            "`i` little-endian",
            "6.i+10",
            [0x24, 0x10, 0x00, 0x00],
            "Tag at=0xaa",
        ),
        (
            "set high bits are masked",
            "6.I+10",
            [0x80, 0x80, 0x90, 0xa4],
            "Tag at=0xaa",
        ),
        (
            "plain `L` reads the raw long",
            "6.L+10",
            [0x00, 0x00, 0x10, 0x24],
            "Tag at=0xbb",
        ),
        (
            "decoded offset past the buffer",
            "6.I+10",
            [0x7f, 0x7f, 0x7f, 0x7f],
            "Tag",
        ),
    ];

    for (name, pointer, size_bytes, expected) in cases {
        let temp_dir = TempDir::new().unwrap();
        let magic_path = temp_dir.path().join("id3.magic");
        let mut f = fs::File::create(&magic_path).unwrap();
        writeln!(f, "0 string ID3 Tag").unwrap();
        writeln!(f, ">({pointer}) ubyte x at=0x%02x").unwrap();
        drop(f);
        let db = MagicDatabase::load_from_file(&magic_path).unwrap();

        let mut buf = vec![0u8; PLAIN_LONG_TARGET + 8];
        buf[..3].copy_from_slice(b"ID3");
        buf[6..10].copy_from_slice(size_bytes);
        buf[SYNCHSAFE_TARGET] = 0xAA;
        buf[PLAIN_LONG_TARGET] = 0xBB;

        let result = db.evaluate_buffer(&buf).unwrap();
        assert_eq!(result.description, *expected, "case: {name}");
    }
}
