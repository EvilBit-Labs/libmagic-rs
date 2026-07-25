// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `bytes_consumed_with_pattern` tests for fixed-width types (byte through
//! qdate), NUL-terminated/length-capped `String`, and every `PString`
//! length-prefix width/flag combination -- all called with `pattern:
//! None`, i.e. the non-pattern-bearing dispatch paths.

use super::*;

use crate::parser::ast::PStringLengthWidth;

#[test]
fn test_bytes_consumed_fixed_width_types() {
    // 16-byte buffer at offset 0: every fixed-width type tested below fits
    // inside the bounds guard (`offset + width <= buffer.len()`). A separate
    // test `test_bytes_consumed_fixed_width_returns_zero_past_end` exercises
    // the guard's 0-return path at and past the boundary.
    let buf = &[0u8; 16];

    let cases: &[(TypeKind, usize)] = &[
        (TypeKind::Byte { signed: false }, 1),
        (TypeKind::Byte { signed: true }, 1),
        (
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            2,
        ),
        (
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
            2,
        ),
        (
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            4,
        ),
        (
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: false,
            },
            8,
        ),
        (
            TypeKind::Float {
                endian: Endianness::Little,
            },
            4,
        ),
        (
            TypeKind::Double {
                endian: Endianness::Big,
            },
            8,
        ),
        (
            TypeKind::Date {
                endian: Endianness::Little,
                utc: false,
            },
            4,
        ),
        (
            TypeKind::QDate {
                endian: Endianness::Big,
                utc: true,
            },
            8,
        ),
    ];

    for (typ, expected) in cases {
        let consumed = bytes_consumed_with_pattern(buf, 0, typ, None);
        assert_eq!(
            consumed, *expected,
            "fixed-width width mismatch for {typ:?}"
        );
    }
}

#[test]
fn test_bytes_consumed_string_with_nul() {
    // "MZ\0" -> matches "MZ" and consumes 3 bytes (2 + NUL).
    let buf = b"MZ\x00rest";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 3);
}

#[test]
fn test_bytes_consumed_string_at_offset() {
    // String starting mid-buffer.
    let buf = b"PREFIXabc\x00tail";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 6, &typ, None), 4); // "abc" + NUL
}

#[test]
fn test_bytes_consumed_string_no_nul_in_buffer() {
    // No NUL terminator -- consumes to end of buffer (no extra byte for NUL).
    let buf = b"NoNull";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 6);
}

#[test]
fn test_bytes_consumed_string_empty() {
    // Empty string at offset 0 -- just the NUL.
    let buf = b"\x00rest";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 1);
}

#[test]
fn test_bytes_consumed_string_max_length_caps() {
    // max_length = 4, NUL is at index 14 -- read stops at 4 chars, no NUL consumed.
    let buf = b"VeryLongString\x00rest";
    let typ = TypeKind::String {
        max_length: Some(4),
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 4);
}

#[test]
fn test_bytes_consumed_string_max_length_finds_nul() {
    // max_length = 10 but NUL is at index 5 -- read stops at NUL, consumes 6.
    let buf = b"Short\x00LongerSuffix";
    let typ = TypeKind::String {
        max_length: Some(10),
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 6);
}

#[test]
fn test_bytes_consumed_pstring_one_byte() {
    // \x05Hello -- prefix(1) + payload(5) = 6
    let buf = b"\x05Hello";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::OneByte,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 6);
}

#[test]
fn test_bytes_consumed_pstring_two_byte_be() {
    // \x00\x05Hello -- prefix(2) + payload(5) = 7
    let buf = b"\x00\x05Hello";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::TwoByteBE,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 7);
}

#[test]
fn test_bytes_consumed_pstring_two_byte_le() {
    let buf = b"\x05\x00Hello";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::TwoByteLE,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 7);
}

#[test]
fn test_bytes_consumed_pstring_four_byte_be() {
    let buf = b"\x00\x00\x00\x01x";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteBE,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 5);
}

#[test]
fn test_bytes_consumed_pstring_j_flag() {
    // /J: stored length 4 -> 4 - 1 (prefix) = 3 bytes payload, total 4
    let buf = b"\x04abc";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::OneByte,
        length_includes_itself: true,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 4);
}

#[test]
fn test_bytes_consumed_pstring_empty() {
    // \x00 -- prefix says length 0, total 1 (just the prefix)
    let buf = b"\x00";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::OneByte,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 1);
}

#[test]
fn test_bytes_consumed_pstring_max_length_caps() {
    // Stored length 10, max_length 5 -- consume prefix(1) + 5 = 6
    let buf = b"\x0aHelloWorld";
    let typ = TypeKind::PString {
        max_length: Some(5),
        length_width: PStringLengthWidth::OneByte,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 6);
}

#[test]
fn test_bytes_consumed_pstring_j_flag_underflow_multi_byte() {
    // /J with TwoByteBE: stored length 1, prefix width 2 -> underflow -> 0.
    let buf = b"\x00\x01xx";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::TwoByteBE,
        length_includes_itself: true,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 0);

    // /J with FourByteLE: stored length 3, prefix width 4 -> underflow -> 0.
    let buf = b"\x03\x00\x00\x00xx";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteLE,
        length_includes_itself: true,
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 0);
}

#[test]
fn test_bytes_consumed_pstring_clamps_oversized_prefix_be() {
    // FourByteBE prefix says 0xFFFFFFFF (4 GB), but the buffer only has
    // 3 bytes after the prefix. bytes_consumed must clamp to the remaining
    // buffer length, not advance the anchor to ~4 GB.
    let buf = b"\xFF\xFF\xFF\xFFabc";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteBE,
        length_includes_itself: false,
    };
    // 4 (prefix) + min(0xFFFFFFFF, 3) = 4 + 3 = 7
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 7);
}

#[test]
fn test_bytes_consumed_pstring_clamps_oversized_prefix_le() {
    let buf = b"\xFF\xFF\xFF\xFFhello";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteLE,
        length_includes_itself: false,
    };
    // 4 + min(0xFFFFFFFF, 5) = 9
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, None), 9);
}

#[test]
fn test_bytes_consumed_string_at_past_end_returns_zero() {
    // Variable-width branch: out-of-bounds offset returns 0, which keeps
    // the anchor in place. The engine guarantees this is never called for
    // a successful read, but the path is exercised here for the contract.
    let buf = b"abc";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    assert_eq!(bytes_consumed_with_pattern(buf, 10, &typ, None), 0);
}

#[test]
fn test_bytes_consumed_fixed_width_returns_zero_past_end() {
    // Fixed-width branch is bounds-checked: if offset + width > buffer.len()
    // it returns 0, mirroring the variable-width path. The engine never
    // calls bytes_consumed at an out-of-bounds offset, but the guard makes
    // the contract self-consistent for any future caller.
    let buf = b"abc";
    let typ = TypeKind::Byte { signed: false };
    // offset == buf.len() leaves no room for a 1-byte read.
    assert_eq!(bytes_consumed_with_pattern(buf, 3, &typ, None), 0);
    // Way past end.
    assert_eq!(bytes_consumed_with_pattern(buf, 100, &typ, None), 0);
    // Last valid index: 1-byte read fits.
    assert_eq!(bytes_consumed_with_pattern(buf, 2, &typ, None), 1);

    // Multi-byte fixed-width type at the boundary.
    let typ_long = TypeKind::Long {
        endian: Endianness::Little,
        signed: false,
    };
    let buf4 = b"abcd";
    // offset 0 + width 4 == buf.len() -> fits
    assert_eq!(bytes_consumed_with_pattern(buf4, 0, &typ_long, None), 4);
    // offset 1 + width 4 == 5 > buf.len() -> 0
    assert_eq!(bytes_consumed_with_pattern(buf4, 1, &typ_long, None), 0);
    // overflow: offset = usize::MAX, width = 4 -> checked_add returns None -> 0
    assert_eq!(
        bytes_consumed_with_pattern(buf4, usize::MAX, &typ_long, None),
        0
    );
}
