// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! ID3 synchsafe reads (`TypeKind::Id3`, magic(5) `i`/`I` pointers, issue
//! #237). Expected values are the upstream `cvt_id3` arithmetic: each byte
//! contributes its low 7 bits, and a set high bit is masked, never rejected.

use super::*;

fn id3(endian: Endianness) -> TypeKind {
    TypeKind::Id3 { endian }
}

#[test]
fn test_read_id3_big_endian_decode_table() {
    let cases: &[(&[u8], u64, &str)] = &[
        (
            &[0x00, 0x00, 0x10, 0x24],
            2084,
            "JW07022A fixture size field",
        ),
        (&[0x00, 0x00, 0x02, 0x01], 257, "two low groups"),
        (
            &[0x7f, 0x7f, 0x7f, 0x7f],
            0x0fff_ffff,
            "maximum 28-bit value",
        ),
        (
            &[0x80, 0x80, 0x80, 0x80],
            0,
            "only high bits set mask to zero",
        ),
        (
            &[0xff, 0x00, 0x00, 0x00],
            0x0fe0_0000,
            "high bit masked in top group",
        ),
        (
            &[0x80, 0x80, 0x90, 0xa4],
            2084,
            "malformed high bits decode as masked",
        ),
    ];

    for (bytes, expected, name) in cases {
        let got = read_typed_value(bytes, 0, &id3(Endianness::Big)).unwrap();
        assert_eq!(got, Value::Uint(*expected), "case: {name}");
    }
}

#[test]
fn test_read_id3_byte_order_is_resolved_before_decoding() {
    let bytes = [0x24, 0x10, 0x00, 0x00];

    let little = read_typed_value(&bytes, 0, &id3(Endianness::Little)).unwrap();
    let big = read_typed_value(&bytes, 0, &id3(Endianness::Big)).unwrap();

    assert_eq!(little, Value::Uint(2084), "`i` reads little-endian");
    assert_eq!(
        big,
        Value::Uint((0x24 << 21) | (0x10 << 14)),
        "`I` reads big-endian"
    );

    // Only a hand-built AST reaches `Native`; it follows host byte order.
    let native = read_typed_value(&bytes, 0, &id3(Endianness::Native)).unwrap();
    let host = if cfg!(target_endian = "little") {
        little
    } else {
        big
    };
    assert_eq!(native, host, "`Native` reads in host byte order");
}

#[test]
fn test_read_id3_reads_at_offset_and_rejects_short_buffer() {
    let buffer = [0xAA, 0x00, 0x00, 0x10, 0x24];

    assert_eq!(
        read_typed_value(&buffer, 1, &id3(Endianness::Big)).unwrap(),
        Value::Uint(2084)
    );
    assert!(matches!(
        read_typed_value(&buffer, 2, &id3(Endianness::Big)),
        Err(TypeReadError::BufferOverrun { .. })
    ));
}

#[test]
fn test_id3_is_a_four_byte_thirty_two_bit_type() {
    let typ = id3(Endianness::Big);
    assert_eq!(typ.bit_width(), Some(32));
    assert_eq!(bytes_consumed_with_pattern(&[0u8; 8], 0, &typ, None), 4);
}
