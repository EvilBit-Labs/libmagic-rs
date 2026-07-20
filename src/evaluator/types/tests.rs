// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::{Endianness, SearchFlags, StringFlags};

#[test]
fn test_type_read_error_display() {
    let error = TypeReadError::BufferOverrun {
        offset: 10,
        buffer_len: 5,
    };
    let msg = format!("{error}");
    assert!(msg.contains("offset 10"));
    assert!(msg.contains("buffer length is 5"));
}

#[test]
fn test_unsupported_type_error_variants() {
    let error = TypeReadError::UnsupportedType {
        type_name: "CustomType".to_string(),
    };
    assert!(format!("{error}").contains("CustomType"));
    assert!(format!("{error:?}").contains("UnsupportedType"));

    assert_eq!(
        error,
        TypeReadError::UnsupportedType {
            type_name: "CustomType".to_string(),
        }
    );
}

#[test]
fn test_read_typed_value_numeric_dispatch() {
    let byte = read_typed_value(&[0x7f, 0x46], 0, &TypeKind::Byte { signed: false }).unwrap();
    assert_eq!(byte, Value::Uint(0x7f));

    let short = read_typed_value(
        &[0x34, 0x12, 0x78, 0x56],
        0,
        &TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap();
    assert_eq!(short, Value::Uint(0x1234));

    let short_signed = read_typed_value(
        &[0x80, 0x00, 0x7f, 0xff],
        0,
        &TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        },
    )
    .unwrap();
    assert_eq!(short_signed, Value::Int(-32768));

    let long = read_typed_value(
        &[0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56],
        0,
        &TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap();
    assert_eq!(long, Value::Uint(0x1234_5678));

    let long_signed = read_typed_value(
        &[0x80, 0x00, 0x00, 0x00],
        0,
        &TypeKind::Long {
            endian: Endianness::Big,
            signed: true,
        },
    )
    .unwrap();
    assert_eq!(long_signed, Value::Int(-2_147_483_648));

    let quad = read_typed_value(
        &[0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12],
        0,
        &TypeKind::Quad {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap();
    assert_eq!(quad, Value::Uint(0x1234_5678_90ab_cdef));
}

#[test]
fn test_read_typed_value_float_dispatch() {
    // IEEE 754 little-endian 1.0f32: 0x3f800000
    let float_result = read_typed_value(
        &[0x00, 0x00, 0x80, 0x3f],
        0,
        &TypeKind::Float {
            endian: Endianness::Little,
        },
    )
    .unwrap();
    assert_eq!(float_result, Value::Float(1.0));

    // IEEE 754 big-endian 1.0f64: 0x3ff0000000000000
    let double_result = read_typed_value(
        &[0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        0,
        &TypeKind::Double {
            endian: Endianness::Big,
        },
    )
    .unwrap();
    assert_eq!(double_result, Value::Float(1.0));

    // Float buffer overrun: 3 bytes is too few for a 4-byte float
    let float_err = read_typed_value(
        &[0x00, 0x00, 0x80],
        0,
        &TypeKind::Float {
            endian: Endianness::Little,
        },
    )
    .unwrap_err();
    assert_eq!(
        float_err,
        TypeReadError::BufferOverrun {
            offset: 0,
            buffer_len: 3,
        }
    );

    // Double buffer overrun: 7 bytes is too few for an 8-byte double
    let double_err = read_typed_value(
        &[0x00; 7],
        0,
        &TypeKind::Double {
            endian: Endianness::Big,
        },
    )
    .unwrap_err();
    assert_eq!(
        double_err,
        TypeReadError::BufferOverrun {
            offset: 0,
            buffer_len: 7,
        }
    );
}

#[test]
fn test_read_typed_value_native_endian() {
    let result = read_typed_value(
        &[0x34, 0x12],
        0,
        &TypeKind::Short {
            endian: Endianness::Native,
            signed: false,
        },
    )
    .unwrap();

    match result {
        Value::Uint(val) => assert!(val == 0x1234 || val == 0x3412),
        _ => panic!("Expected Value::Uint variant"),
    }
}

#[test]
fn test_read_typed_value_string_dispatch() {
    let buffer = b"Hello\x00World\x00";

    let result = read_typed_value(
        buffer,
        0,
        &TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
    )
    .unwrap();
    assert_eq!(result, Value::String("Hello".to_string()));

    let result = read_typed_value(
        b"VeryLongString\x00",
        0,
        &TypeKind::String {
            max_length: Some(4),
            flags: StringFlags::default(),
        },
    )
    .unwrap();
    assert_eq!(result, Value::String("Very".to_string()));
}

#[test]
fn test_read_typed_value_buffer_overrun() {
    let short_error = read_typed_value(
        &[0x12],
        0,
        &TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap_err();
    assert_eq!(
        short_error,
        TypeReadError::BufferOverrun {
            offset: 0,
            buffer_len: 1
        }
    );

    let long_error = read_typed_value(
        &[0x12, 0x34],
        0,
        &TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap_err();
    assert_eq!(
        long_error,
        TypeReadError::BufferOverrun {
            offset: 0,
            buffer_len: 2
        }
    );
}

#[test]
fn test_read_typed_value_all_supported_types() {
    let buffer = &[0x7f, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a];
    let test_cases = [
        (TypeKind::Byte { signed: false }, 0, Value::Uint(0x7f)),
        (
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            1,
            Value::Uint(0x1234),
        ),
        (
            TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            },
            1,
            Value::Uint(0x3412),
        ),
        (
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            1,
            Value::Uint(0x5678_1234),
        ),
        (
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            1,
            Value::Uint(0x3412_7856),
        ),
        (
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            },
            1,
            Value::Uint(0x9abc_1234_5678_1234),
        ),
    ];

    for (type_kind, offset, expected) in test_cases {
        let result = read_typed_value(buffer, offset, &type_kind).unwrap();
        assert_eq!(result, expected, "Failed for type: {type_kind:?}");
    }
}

#[test]
fn test_coerce_value_to_type_float_rounds_to_f32() {
    // 0.1 as f64 differs from 0.1 as f32-widened-to-f64
    let f64_val = Value::Float(0.1_f64);
    let coerced = coerce_value_to_type(
        &f64_val,
        &TypeKind::Float {
            endian: Endianness::Native,
        },
    );
    // After coercion, value should match f32 precision
    #[allow(clippy::cast_possible_truncation)]
    let expected = f64::from(0.1_f64 as f32);
    assert_eq!(*coerced, Value::Float(expected));
}

#[test]
fn test_coerce_value_to_type_double_preserves_f64() {
    // Double should not alter the f64 value
    let val = Value::Float(0.1_f64);
    let coerced = coerce_value_to_type(
        &val,
        &TypeKind::Double {
            endian: Endianness::Native,
        },
    );
    assert_eq!(*coerced, Value::Float(0.1_f64));
}

#[test]
fn test_read_typed_value_date() {
    // 0x00000001 BE = 1 second after epoch
    let buffer = &[0x00, 0x00, 0x00, 0x01];
    let result = read_typed_value(
        buffer,
        0,
        &TypeKind::Date {
            endian: Endianness::Big,
            utc: true,
        },
    )
    .unwrap();
    assert_eq!(
        result,
        Value::String("Thu Jan  1 00:00:01 1970".to_string())
    );

    // Same bytes in LE = 0x01000000 = 16777216 seconds
    let result_le = read_typed_value(
        buffer,
        0,
        &TypeKind::Date {
            endian: Endianness::Little,
            utc: true,
        },
    )
    .unwrap();
    match result_le {
        Value::String(_) => {}
        other => panic!("Expected Value::String, got {other:?}"),
    }
}

#[test]
fn test_read_typed_value_qdate() {
    // 0x0000000000000001 BE = 1 second after epoch
    let buffer = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    let result = read_typed_value(
        buffer,
        0,
        &TypeKind::QDate {
            endian: Endianness::Big,
            utc: true,
        },
    )
    .unwrap();
    assert_eq!(
        result,
        Value::String("Thu Jan  1 00:00:01 1970".to_string())
    );

    // Same bytes in LE
    let result_le = read_typed_value(
        buffer,
        0,
        &TypeKind::QDate {
            endian: Endianness::Little,
            utc: true,
        },
    )
    .unwrap();
    match result_le {
        Value::String(_) => {}
        other => panic!("Expected Value::String, got {other:?}"),
    }
}

#[test]
fn test_read_typed_value_signed_vs_unsigned() {
    let buffer = &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff];

    let unsigned_short = read_typed_value(
        buffer,
        0,
        &TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap();
    let signed_short = read_typed_value(
        buffer,
        0,
        &TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
    )
    .unwrap();
    assert_eq!(unsigned_short, Value::Uint(65535));
    assert_eq!(signed_short, Value::Int(-1));

    let unsigned_long = read_typed_value(
        buffer,
        0,
        &TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
    )
    .unwrap();
    let signed_long = read_typed_value(
        buffer,
        0,
        &TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
    )
    .unwrap();
    assert_eq!(unsigned_long, Value::Uint(4_294_967_295));
    assert_eq!(signed_long, Value::Int(-1));
}

#[test]
fn test_read_typed_value_consistency_with_direct_calls() {
    let buffer = &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xde, 0xf0];

    assert_eq!(
        read_byte(buffer, 0, false).unwrap(),
        read_typed_value(buffer, 0, &TypeKind::Byte { signed: false }).unwrap()
    );
    assert_eq!(
        read_short(buffer, 0, Endianness::Little, false).unwrap(),
        read_typed_value(
            buffer,
            0,
            &TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
        )
        .unwrap()
    );
    assert_eq!(
        read_long(buffer, 0, Endianness::Big, true).unwrap(),
        read_typed_value(
            buffer,
            0,
            &TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
        )
        .unwrap()
    );
}

#[test]
fn test_read_typed_value_empty_buffer() {
    for type_kind in [
        TypeKind::Byte { signed: false },
        TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
    ] {
        let result = read_typed_value(&[], 0, &type_kind);
        assert!(result.is_err());
        match result.unwrap_err() {
            TypeReadError::BufferOverrun { offset, buffer_len } => {
                assert_eq!(offset, 0);
                assert_eq!(buffer_len, 0);
            }
            other => panic!("Expected BufferOverrun error, got {other:?}"),
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn test_coerce_value_to_type() {
    let cases = [
        (
            Value::Uint(0xff),
            TypeKind::Byte { signed: true },
            Value::Int(-1),
        ),
        (
            Value::Uint(0x80),
            TypeKind::Byte { signed: true },
            Value::Int(-128),
        ),
        (
            Value::Uint(0xfe),
            TypeKind::Byte { signed: true },
            Value::Int(-2),
        ),
        (
            Value::Uint(0x7f),
            TypeKind::Byte { signed: true },
            Value::Uint(0x7f),
        ),
        (
            Value::Uint(0xff),
            TypeKind::Byte { signed: false },
            Value::Uint(0xff),
        ),
        (
            Value::Uint(0xffff),
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(-1),
        ),
        (
            Value::Uint(0x8000),
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(-32768),
        ),
        (
            Value::Uint(0x7fff),
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Uint(0x7fff),
        ),
        (
            Value::Uint(0xffff_ffff),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(-1),
        ),
        (
            Value::Uint(0x8000_0000),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(-2_147_483_648),
        ),
        (
            Value::Uint(0x7fff_ffff),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Uint(0x7fff_ffff),
        ),
        (
            Value::Uint(0xffff_ffff_ffff_ffff),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(-1),
        ),
        (
            Value::Uint(0x8000_0000_0000_0000),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(i64::MIN),
        ),
        (
            Value::Uint(0x7fff_ffff_ffff_ffff),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Uint(0x7fff_ffff_ffff_ffff),
        ),
        (
            Value::Uint(0xffff_ffff_ffff_ffff),
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: false,
            },
            Value::Uint(0xffff_ffff_ffff_ffff),
        ),
        (
            Value::Int(-1),
            TypeKind::Byte { signed: true },
            Value::Int(-1),
        ),
        (
            Value::Int(42),
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            },
            Value::Int(42),
        ),
        (
            Value::Uint(0xff),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            Value::Uint(0xff),
        ),
        (
            Value::Float(3.125),
            TypeKind::Float {
                endian: Endianness::Native,
            },
            // 3.125 rounded to f32 precision then widened back to f64
            Value::Float(f64::from(3.125_f32)),
        ),
        (
            Value::Float(3.125),
            TypeKind::Double {
                endian: Endianness::Native,
            },
            Value::Float(3.125),
        ),
    ];

    for (i, (input, type_kind, expected)) in cases.iter().enumerate() {
        let result = coerce_value_to_type(input, type_kind);
        assert_eq!(
            *result, *expected,
            "Case {i}: coerce({input:?}, {type_kind:?})"
        );
    }
}

#[test]
fn test_coerce_value_to_type_date_numeric() {
    // Numeric expected values for Date types should be formatted as timestamp strings
    let date_type = TypeKind::Date {
        endian: Endianness::Big,
        utc: true,
    };

    // Uint(0) -> epoch string
    let result = coerce_value_to_type(&Value::Uint(0), &date_type);
    assert_eq!(
        *result,
        Value::String("Thu Jan  1 00:00:00 1970".to_string())
    );

    // Uint(1_000_000_000) -> known date
    let result = coerce_value_to_type(&Value::Uint(1_000_000_000), &date_type);
    assert_eq!(
        *result,
        Value::String("Sun Sep  9 01:46:40 2001".to_string())
    );

    // Int(0) -> epoch string
    let result = coerce_value_to_type(&Value::Int(0), &date_type);
    assert_eq!(
        *result,
        Value::String("Thu Jan  1 00:00:00 1970".to_string())
    );

    // Negative Int should pass through unchanged
    let result = coerce_value_to_type(&Value::Int(-1), &date_type);
    assert_eq!(*result, Value::Int(-1));

    // String values should pass through unchanged
    let s = Value::String("already a string".to_string());
    let result = coerce_value_to_type(&s, &date_type);
    assert_eq!(*result, s);
}

#[test]
fn test_coerce_value_to_type_qdate_numeric() {
    // Numeric expected values for QDate types should be formatted as timestamp strings
    let qdate_type = TypeKind::QDate {
        endian: Endianness::Big,
        utc: true,
    };

    let result = coerce_value_to_type(&Value::Uint(0), &qdate_type);
    assert_eq!(
        *result,
        Value::String("Thu Jan  1 00:00:00 1970".to_string())
    );

    let result = coerce_value_to_type(&Value::Uint(1_000_000_000), &qdate_type);
    assert_eq!(
        *result,
        Value::String("Sun Sep  9 01:46:40 2001".to_string())
    );
}

#[test]
fn test_coerce_date_matches_read_date() {
    // Verify that coerced numeric operands match the Value::String from read_date
    let buffer = &[0x3B, 0x9A, 0xCA, 0x00]; // 1_000_000_000 in BE
    let date_type = TypeKind::Date {
        endian: Endianness::Big,
        utc: true,
    };

    let read_val = read_date(buffer, 0, Endianness::Big, true).unwrap();
    let coerced = coerce_value_to_type(&Value::Uint(1_000_000_000), &date_type);
    assert_eq!(
        read_val, *coerced,
        "Coerced value should match read_date output"
    );
}

#[test]
fn test_coerce_qdate_matches_read_qdate() {
    // Verify that coerced numeric operands match the Value::String from read_qdate
    let buffer = &[0x00, 0x00, 0x00, 0x00, 0x3B, 0x9A, 0xCA, 0x00]; // 1_000_000_000 in BE
    let qdate_type = TypeKind::QDate {
        endian: Endianness::Big,
        utc: true,
    };

    let read_val = read_qdate(buffer, 0, Endianness::Big, true).unwrap();
    let coerced = coerce_value_to_type(&Value::Uint(1_000_000_000), &qdate_type);
    assert_eq!(
        read_val, *coerced,
        "Coerced value should match read_qdate output"
    );
}

// ============================================================
// bytes_consumed tests
// ============================================================

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

// =============================================================================
// fix-system-magic-regex-graceful, U1: `Value::Bytes` backstop for
// `TypeKind::Regex`.
//
// The parser can currently miscategorize an escape-heavy regex pattern
// (e.g. `\^[\040\t]{0,50}\\.asciiz`) as `Value::Bytes` instead of
// `Value::String` (see `parse_value`'s hex/mixed-ascii branch). Before this
// fix, both `read_typed_value_with_pattern` and `read_pattern_match`
// rejected `Value::Bytes` regex patterns with `UnsupportedType`, unlike the
// sibling `TypeKind::Search` arms which already accepted both variants
// (GOTCHAS S2.4). See docs/plans/2026-07-17-001-fix-system-magic-regex-
// graceful-plan.md.
// =============================================================================

fn regex_type() -> TypeKind {
    TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    }
}

/// Happy path (regression guard): a `Value::String` pattern still matches
/// through `read_pattern_match`, unaffected by the new `Bytes` arm.
#[test]
fn test_read_pattern_match_regex_string_pattern_still_matches() {
    let typ = regex_type();
    let pattern = Value::String("foobar[0-9]+".to_string());
    let result = read_pattern_match(b"prefix foobar123 suffix", 0, &typ, Some(&pattern), 8192)
        .expect("read_pattern_match should not error for a valid String pattern");
    assert!(
        matches!(result, Some(Value::String(ref s)) if s == "foobar123"),
        "expected a match on the String pattern, got {result:?}"
    );
}

/// A `Value::Bytes` regex pattern (the miscategorized-escape case) must be
/// accepted by `read_pattern_match`, not rejected as `UnsupportedType`.
#[test]
fn test_read_pattern_match_regex_accepts_bytes_pattern() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"^[ \t]*\\.asciiz".to_vec());
    let result = read_pattern_match(b"\t.asciiz \"hi\"", 0, &typ, Some(&pattern), 8192)
        .expect("read_pattern_match must accept a Value::Bytes regex pattern, not UnsupportedType");
    assert!(
        result.is_some(),
        "expected the Bytes pattern to match the leading-whitespace buffer, got {result:?}"
    );
}

/// The same `Value::Bytes` acceptance must hold for
/// `read_typed_value_with_pattern` (the non-engine dispatch entry point),
/// mirroring the `read_pattern_match` arm exactly.
#[test]
fn test_read_typed_value_with_pattern_regex_accepts_bytes_pattern() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"[0-9]+".to_vec());
    let result = read_typed_value_with_pattern(b"abc123def", 0, &typ, Some(&pattern), 8192)
        .expect("read_typed_value_with_pattern must accept a Value::Bytes regex pattern");
    assert_eq!(
        result,
        Value::String("123".to_string()),
        "expected the matched digits, got {result:?}"
    );
}

/// Zero-width match contract (GOTCHAS S2.5) must be preserved for a
/// `Value::Bytes` pattern: `^` matches at position 0 with an empty capture,
/// which is `Ok(Some(Value::String("")))`, distinct from a genuine miss.
#[test]
fn test_read_pattern_match_regex_bytes_zero_width_match() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"^".to_vec());
    let result = read_pattern_match(b"hello", 0, &typ, Some(&pattern), 8192)
        .expect("zero-width Bytes pattern should not error");
    assert_eq!(
        result,
        Some(Value::String(String::new())),
        "zero-width match must be Some(empty string), not None (GOTCHAS S2.5)"
    );
}

/// A `Value::Bytes` pattern that does not match the buffer must produce a
/// genuine miss (`Ok(None)`), not an error.
#[test]
fn test_read_pattern_match_regex_bytes_pattern_miss() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"xyz".to_vec());
    let result =
        read_pattern_match(b"abcdef", 0, &typ, Some(&pattern), 8192).expect("miss is not an error");
    assert_eq!(result, None, "non-matching Bytes pattern must be Ok(None)");
}

/// KTD6: a `Value::Bytes` regex pattern containing a byte that is not
/// valid UTF-8 must not panic. `String::from_utf8_lossy` substitutes
/// U+FFFD for the invalid byte before compiling -- this test only pins the
/// no-panic / graceful-result contract; the `warn!` emission itself is
/// verified by code inspection of `decode_regex_bytes_pattern` since this
/// crate has no log-capturing test seam (no `test-log`/`tracing-test`
/// dev-dependency).
#[test]
fn test_read_pattern_match_regex_bytes_invalid_utf8_does_not_panic() {
    let typ = regex_type();
    // 0xFF is never valid UTF-8 in any position.
    let pattern = Value::Bytes(vec![0xFF, b'a']);
    let result = read_pattern_match(b"\xEF\xBF\xBDa tail", 0, &typ, Some(&pattern), 8192);
    // Whether this happens to match the lossily-substituted U+FFFD encoding
    // or not is incidental; the load-bearing assertion is that decoding an
    // invalid-UTF-8 Bytes pattern never panics and always yields a valid
    // Result.
    assert!(
        result.is_ok(),
        "invalid-UTF-8 Bytes pattern must not error, got {result:?}"
    );
}

/// Missing pattern (`None`) must still be a hard `UnsupportedType` error in
/// both dispatch functions -- U2's engine-level graceful skip depends on
/// this remaining an `Err`, not silently becoming a non-match here.
#[test]
fn test_regex_missing_pattern_still_errors_in_both_dispatch_fns() {
    let typ = regex_type();

    let pattern_match_result = read_pattern_match(b"abc", 0, &typ, None, 8192);
    assert!(
        matches!(
            pattern_match_result,
            Err(TypeReadError::UnsupportedType { ref type_name }) if type_name == "regex without string pattern"
        ),
        "read_pattern_match with no pattern must still error, got {pattern_match_result:?}"
    );

    let typed_value_result = read_typed_value_with_pattern(b"abc", 0, &typ, None, 8192);
    assert!(
        matches!(
            typed_value_result,
            Err(TypeReadError::UnsupportedType { ref type_name }) if type_name == "regex without string pattern"
        ),
        "read_typed_value_with_pattern with no pattern must still error, got {typed_value_result:?}"
    );
}

// =============================================================================
// U2 predicate helpers: `is_missing_pattern_operand` / `is_regex_compile_failure`
// =============================================================================

#[test]
fn test_is_missing_pattern_operand_recognizes_known_messages() {
    let recognized: &[&str] = &[
        "regex without string pattern",
        "search without string/bytes pattern",
        "string with flags requires string/bytes pattern",
    ];
    for msg in recognized {
        assert!(
            is_missing_pattern_operand(msg),
            "expected {msg:?} to be recognized as a missing-pattern-operand condition"
        );
    }
}

#[test]
fn test_is_missing_pattern_operand_rejects_other_unsupported_type_messages() {
    // R3 narrowness guard: these must NOT be recognized, or a genuine
    // capability gap / operator-misuse error would be silently swallowed.
    let not_recognized: &[String] = &[
        "regex compile error: some failure".to_string(),
        "meta-type Offset cannot be read as a value".to_string(),
        "operator GreaterThan is not supported for pattern-bearing type".to_string(),
        "read_pattern_match called on non-pattern type".to_string(),
    ];
    for msg in not_recognized {
        assert!(
            !is_missing_pattern_operand(msg),
            "expected {msg:?} to NOT be recognized as a missing-pattern-operand condition"
        );
    }
}

#[test]
fn test_is_regex_compile_failure_matches_only_compile_error_prefix() {
    assert!(is_regex_compile_failure(
        "regex compile error: Compiled regex exceeds size limit of 1048576 bytes."
    ));
    assert!(!is_regex_compile_failure("regex without string pattern"));
    assert!(!is_regex_compile_failure(
        "search without string/bytes pattern"
    ));
}

#[test]
fn test_bytes_consumed_regex_with_string_pattern() {
    // Regression guard for GOTCHAS 2.1: variable-width variants must be
    // matched explicitly in `bytes_consumed_with_pattern` or relative
    // offsets silently corrupt. This test exercises the dispatch path
    // and verifies the match-end byte count matches the reader's view.
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("World".to_string());
    // "World" starts at index 7 in the buffer, length 5, so a scan from
    // offset 0 consumes 7+5=12 bytes.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        12
    );
}

/// Regression guard: `bytes_consumed_with_pattern`'s `Regex` arm must
/// accept a `Value::Bytes` pattern, mirroring U1's read-side acceptance
/// (`read_pattern_match` / `read_typed_value_with_pattern`). This was
/// caught by `prop_arbitrary_rule_evaluation_never_panics` firing the
/// `debug_assert` in the pre-fix `other => { debug_assert!(false, ...) }`
/// arm for a `NotEqual` regex rule with a `Value::Bytes` pattern -- a
/// successful Bytes-pattern regex match would advance the anchor by 0
/// (silently stalling) instead of the correct match-end distance, and the
/// property test additionally caught the `debug_assert!(false, ...)`
/// firing as a panic in debug builds.
#[test]
fn test_bytes_consumed_regex_with_bytes_pattern() {
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::Bytes(b"World".to_vec());
    // "World" starts at index 7, length 5, so a scan from offset 0
    // consumes 7+5=12 bytes -- matching the Value::String equivalent in
    // `test_bytes_consumed_regex_with_string_pattern`.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        12
    );
}

#[test]
fn test_bytes_consumed_regex_no_match_returns_zero() {
    let buf = b"abcdef";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("xyz".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_regex_zero_width_match_returns_zero() {
    // Zero-width match at position 0 means match_end=0 so the anchor
    // stays put. Cross-check with the direct reader in regex.rs.
    let buf = b"hello";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("^".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_regex_start_offset_flag_uses_match_start() {
    // /s flag changes the anchor advance to match-start instead of
    // match-end. Regression guard for V2.
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags {
            start_offset: true,
            ..crate::parser::ast::RegexFlags::default()
        },
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("World".to_string());
    // Match-start for "World" at index 7 is 7, not 12.
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 7);
}

#[test]
fn test_bytes_consumed_search_with_pattern_is_match_end() {
    // Regression guard for the pre-fix behavior that returned the
    // entire window size instead of match-end. Per GNU `file` softmagic.c
    // FILE_SEARCH, the anchor advances to `base + match_idx + pattern.len()`.
    let buf = b"abcWorld_xyz";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(10),
        flags: SearchFlags::default(),
    };
    let pattern = Value::String("World".to_string());
    // "World" is at index 3, length 5, match-end = 8.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        8,
        "expected match-end (8), not window-end (10)"
    );
}

#[test]
fn test_bytes_consumed_search_no_match_returns_zero() {
    let buf = b"abcdefghij";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(10),
        flags: SearchFlags::default(),
    };
    let pattern = Value::String("XYZ".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_search_bytes_pattern_works() {
    // Value::Bytes is an alternative pattern shape for search -- verify
    // the dispatch path accepts it and computes the same match-end as a
    // Value::String pattern would.
    let buf = &[0x00, 0xff, 0xde, 0xad, 0xbe, 0xef, 0x11];
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(7),
        flags: SearchFlags::default(),
    };
    let pattern = Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
    // 0xde at index 2, length 4, match-end = 6.
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 6);
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

/// Regression: when a `TypeKind::String` rule's comparison value is a
/// `Value::Bytes` (e.g., parser produces `Value::Bytes([0x7f, 'E', 'L',
/// 'F'])` for the `\177ELF` ELF magic via `parse_mixed_hex_ascii`), the
/// read path uses `read_string_exact(buffer, offset, b.len())` and so
/// the consume path must agree -- otherwise the relative-offset anchor
/// mis-advances by the NUL-scan length on a NUL-free ELF header. This
/// is the same dual-purpose-helper-sync rule documented in GOTCHAS S6.4
/// for `read_string` <-> `read_string_exact`. The bug pattern is the
/// same class as the original 3-bug fix this PR addresses; the fix here
/// closes the consume-side gap that comment-analyzer (PR #233 review)
/// flagged as load-bearing for ELF-style rules.
#[test]
fn test_bytes_consumed_string_with_bytes_pattern_is_exact_length() {
    use crate::parser::ast::Value;

    // Buffer with no NUL anywhere -- typical ELF header. If the consume
    // path had fallen through to the NUL-scan branch, this would return
    // the full buffer length (16) instead of the pattern length (4).
    let buf: &[u8] = &[
        0x7f, 0x45, 0x4c, 0x46, // \x7fELF
        0x02, 0x01, 0x01, 0x00, // ELF metadata
        0x00, 0x00, 0x00, 0x00, // padding
        0x00, 0x00, 0x00, 0x00, // padding
    ];
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    let pattern = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);

    let consumed = bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern));
    assert_eq!(
        consumed, 4,
        "Bytes pattern of length 4 must consume exactly 4 bytes, not the NUL-scan length"
    );

    // Buffer-overrun case: pattern longer than remaining buffer -> 0.
    let short_buf: &[u8] = &[0x7f, 0x45];
    assert_eq!(
        bytes_consumed_with_pattern(short_buf, 0, &typ, Some(&pattern)),
        0,
        "Bytes pattern longer than buffer must return 0 (overrun)"
    );

    // Offset overflow case.
    assert_eq!(
        bytes_consumed_with_pattern(buf, usize::MAX, &typ, Some(&pattern)),
        0,
        "usize::MAX offset must return 0 via checked_add"
    );
}

// -----------------------------------------------------------------------
// H hardening: pin the `decode_regex_bytes_pattern` warn!-on-real-
// substitution contract (KTD6) with a real log-capture seam
// (`testing_logger`), rather than code inspection only.
// -----------------------------------------------------------------------

/// Test-only helper: `testing_logger::CapturedLog` does not implement
/// `Debug`, so format captured logs manually for failure messages.
fn format_logs(logs: &[testing_logger::CapturedLog]) -> String {
    logs.iter()
        .map(|l| format!("{:?}: {}", l.level, l.body))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A `Value::Bytes` regex pattern containing a byte `>= 0x80` that is not
/// valid UTF-8 triggers a real lossy substitution (`from_utf8_lossy`
/// replaces it with U+FFFD); `decode_regex_bytes_pattern` must `warn!`
/// because the compiled regex now silently diverges from the raw bytes
/// the target buffer is matched against.
#[test]
fn decode_regex_bytes_pattern_warns_on_real_utf8_substitution() {
    testing_logger::setup();
    let decoded = decode_regex_bytes_pattern(&[0xFF, b'a']);
    // Sanity: the function is still infallible and produces SOME string
    // (the lossy replacement), never panics.
    assert!(decoded.contains('a'));
    testing_logger::validate(|captured_logs| {
        let warn_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("not valid UTF-8"))
            .collect();
        assert_eq!(
            warn_logs.len(),
            1,
            "expected exactly one lossy-substitution warning, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            warn_logs[0].level,
            log::Level::Warn,
            "lossy UTF-8 substitution must log at warn!, not another level -- got {:?}",
            warn_logs[0].level
        );
    });
}

/// The converse: valid-UTF-8 bytes must NOT trigger the substitution
/// warning at all -- the guard is keyed on `str::from_utf8` actually
/// failing, not merely on the input being `Value::Bytes`.
#[test]
fn decode_regex_bytes_pattern_does_not_warn_on_valid_utf8() {
    testing_logger::setup();
    let decoded = decode_regex_bytes_pattern(b"hello[0-9]+");
    assert_eq!(decoded, "hello[0-9]+");
    testing_logger::validate(|captured_logs| {
        assert!(
            captured_logs.is_empty(),
            "valid UTF-8 bytes must not trigger any log entry, got {:?}",
            format_logs(captured_logs)
        );
    });
}

/// `flip_type_endian` mirrors libmagic `softmagic.c` `cvt_flip`: it swaps
/// only the explicit little/big-endian numeric, float, and date families,
/// leaves `Endianness::Native` untouched, and returns every non-endian type
/// (including `String16`, which is deliberately absent from `cvt_flip`)
/// unchanged. The `signed`/`utc` attributes are preserved.
#[test]
fn test_flip_type_endian_matches_cvt_flip() {
    use crate::parser::ast::TypeKind;

    // (input, expected-after-flip). Endian-bearing types swap LE<->BE.
    let flipped: &[(TypeKind, TypeKind)] = &[
        (
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
        ),
        (
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
        ),
        (
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            },
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            },
        ),
        (
            TypeKind::Float {
                endian: Endianness::Big,
            },
            TypeKind::Float {
                endian: Endianness::Little,
            },
        ),
        (
            TypeKind::Double {
                endian: Endianness::Little,
            },
            TypeKind::Double {
                endian: Endianness::Big,
            },
        ),
        (
            TypeKind::Date {
                endian: Endianness::Big,
                utc: true,
            },
            TypeKind::Date {
                endian: Endianness::Little,
                utc: true,
            },
        ),
        (
            TypeKind::QDate {
                endian: Endianness::Little,
                utc: false,
            },
            TypeKind::QDate {
                endian: Endianness::Big,
                utc: false,
            },
        ),
    ];
    for (input, expected) in flipped {
        assert_eq!(
            &flip_type_endian(input),
            expected,
            "endian-bearing type must swap LE<->BE: {input:?}"
        );
    }

    // Native-endian and non-endian types must be returned unchanged.
    let unchanged: &[TypeKind] = &[
        TypeKind::Short {
            endian: Endianness::Native,
            signed: true,
        },
        TypeKind::Long {
            endian: Endianness::Native,
            signed: false,
        },
        TypeKind::Byte { signed: true },
        // String16 is intentionally NOT in libmagic's cvt_flip.
        TypeKind::String16 {
            endian: Endianness::Big,
        },
        TypeKind::String16 {
            endian: Endianness::Little,
        },
    ];
    for typ in unchanged {
        assert_eq!(
            &flip_type_endian(typ),
            typ,
            "native/non-endian type must be unchanged: {typ:?}"
        );
    }
}
