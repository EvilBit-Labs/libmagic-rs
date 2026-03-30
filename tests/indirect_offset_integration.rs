// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for indirect offset parsing and evaluation
//!
//! Exercises the full pipeline: write a magic file with indirect-offset syntax,
//! load it through `MagicDatabase::load_from_file()`, evaluate buffers, and
//! assert correct match / no-match behavior.

use std::fs;
use std::io::Write;

use libmagic_rs::MagicDatabase;
use tempfile::TempDir;

/// Build a PE-like buffer where offset 0x3c holds a big-endian 4-byte pointer
/// to the PE signature (`PE\0\0`).
///
/// Layout:
///   [0x00] "MZ" DOS header stub
///   [0x3c] 4-byte big-endian pointer -> 0x80 (PE header location)
///   [0x80] "PE\0\0" signature
fn build_pe_like_buffer() -> Vec<u8> {
    let mut buf = vec![0u8; 0x84];
    // DOS stub magic
    buf[0] = b'M';
    buf[1] = b'Z';
    // Big-endian pointer at 0x3c -> 0x80
    buf[0x3c] = 0x00;
    buf[0x3d] = 0x00;
    buf[0x3e] = 0x00;
    buf[0x3f] = 0x80;
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

    // Use .L (big-endian long) for deterministic cross-platform behavior.
    // String values must be quoted for the parser.
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"0 string "MZ" DOS executable"#).unwrap();
    writeln!(f, r#">(0x3c.L) string "PE" (PE)"#).unwrap();

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
    writeln!(f, r#">(0x3c.L) string "PE" (PE)"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Buffer has "MZ" but the pointer at 0x3c points beyond the buffer
    let mut buf = vec![0u8; 0x40];
    buf[0] = b'M';
    buf[1] = b'Z';
    // Pointer at 0x3c -> 0xFF (beyond buffer length)
    buf[0x3c] = 0x00;
    buf[0x3d] = 0x00;
    buf[0x3e] = 0x00;
    buf[0x3f] = 0xFF;

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
fn test_indirect_offset_with_adjustment() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("adj.magic");

    // Indirect offset with +4 adjustment: read pointer at 0, add 4, check there
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"(0.L+4) string "MAGIC" Adjusted match"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Pointer at offset 0 = 0x00000006 (big-endian), +4 = 10, "MAGIC" at offset 10
    let mut buf = vec![0u8; 20];
    buf[0] = 0x00;
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x06;
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
    writeln!(f, r#"(0.b) string "A" byte ptr"#).unwrap();
    writeln!(f, r#"(0.B) string "A" Byte ptr"#).unwrap();
    writeln!(f, r#"(0.s) string "A" short native ptr"#).unwrap();
    writeln!(f, r#"(0.S) string "A" short BE ptr"#).unwrap();
    writeln!(f, r#"(0.l) string "A" long native ptr"#).unwrap();
    writeln!(f, r#"(0.L) string "A" long BE ptr"#).unwrap();
    writeln!(f, r#"(0.q) string "A" quad native ptr"#).unwrap();
    writeln!(f, r#"(0.Q) string "A" quad BE ptr"#).unwrap();

    let result = MagicDatabase::load_from_file(&magic_path);
    assert!(
        result.is_ok(),
        "Loading magic file with all indirect specifiers should succeed: {:?}",
        result.err()
    );
}
