// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::Endianness;

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

    let result = read_typed_value(buffer, 0, &TypeKind::String { max_length: None }).unwrap();
    assert_eq!(result, Value::String("Hello".to_string()));

    let result = read_typed_value(
        b"VeryLongString\x00",
        0,
        &TypeKind::String {
            max_length: Some(4),
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
            TypeKind::String { max_length: None },
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
        let consumed = bytes_consumed(buf, 0, typ);
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
    let typ = TypeKind::String { max_length: None };
    assert_eq!(bytes_consumed(buf, 0, &typ), 3);
}

#[test]
fn test_bytes_consumed_string_at_offset() {
    // String starting mid-buffer.
    let buf = b"PREFIXabc\x00tail";
    let typ = TypeKind::String { max_length: None };
    assert_eq!(bytes_consumed(buf, 6, &typ), 4); // "abc" + NUL
}

#[test]
fn test_bytes_consumed_string_no_nul_in_buffer() {
    // No NUL terminator -- consumes to end of buffer (no extra byte for NUL).
    let buf = b"NoNull";
    let typ = TypeKind::String { max_length: None };
    assert_eq!(bytes_consumed(buf, 0, &typ), 6);
}

#[test]
fn test_bytes_consumed_string_empty() {
    // Empty string at offset 0 -- just the NUL.
    let buf = b"\x00rest";
    let typ = TypeKind::String { max_length: None };
    assert_eq!(bytes_consumed(buf, 0, &typ), 1);
}

#[test]
fn test_bytes_consumed_string_max_length_caps() {
    // max_length = 4, NUL is at index 14 -- read stops at 4 chars, no NUL consumed.
    let buf = b"VeryLongString\x00rest";
    let typ = TypeKind::String {
        max_length: Some(4),
    };
    assert_eq!(bytes_consumed(buf, 0, &typ), 4);
}

#[test]
fn test_bytes_consumed_string_max_length_finds_nul() {
    // max_length = 10 but NUL is at index 5 -- read stops at NUL, consumes 6.
    let buf = b"Short\x00LongerSuffix";
    let typ = TypeKind::String {
        max_length: Some(10),
    };
    assert_eq!(bytes_consumed(buf, 0, &typ), 6);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 6);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 7);
}

#[test]
fn test_bytes_consumed_pstring_two_byte_le() {
    let buf = b"\x05\x00Hello";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::TwoByteLE,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed(buf, 0, &typ), 7);
}

#[test]
fn test_bytes_consumed_pstring_four_byte_be() {
    let buf = b"\x00\x00\x00\x01x";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteBE,
        length_includes_itself: false,
    };
    assert_eq!(bytes_consumed(buf, 0, &typ), 5);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 4);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 1);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 6);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 0);

    // /J with FourByteLE: stored length 3, prefix width 4 -> underflow -> 0.
    let buf = b"\x03\x00\x00\x00xx";
    let typ = TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::FourByteLE,
        length_includes_itself: true,
    };
    assert_eq!(bytes_consumed(buf, 0, &typ), 0);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 7);
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
    assert_eq!(bytes_consumed(buf, 0, &typ), 9);
}

#[test]
fn test_bytes_consumed_string_at_past_end_returns_zero() {
    // Variable-width branch: out-of-bounds offset returns 0, which keeps
    // the anchor in place. The engine guarantees this is never called for
    // a successful read, but the path is exercised here for the contract.
    let buf = b"abc";
    let typ = TypeKind::String { max_length: None };
    assert_eq!(bytes_consumed(buf, 10, &typ), 0);
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
    assert_eq!(bytes_consumed(buf, 3, &typ), 0);
    // Way past end.
    assert_eq!(bytes_consumed(buf, 100, &typ), 0);
    // Last valid index: 1-byte read fits.
    assert_eq!(bytes_consumed(buf, 2, &typ), 1);

    // Multi-byte fixed-width type at the boundary.
    let typ_long = TypeKind::Long {
        endian: Endianness::Little,
        signed: false,
    };
    let buf4 = b"abcd";
    // offset 0 + width 4 == buf.len() -> fits
    assert_eq!(bytes_consumed(buf4, 0, &typ_long), 4);
    // offset 1 + width 4 == 5 > buf.len() -> 0
    assert_eq!(bytes_consumed(buf4, 1, &typ_long), 0);
    // overflow: offset = usize::MAX, width = 4 -> checked_add returns None -> 0
    assert_eq!(bytes_consumed(buf4, usize::MAX, &typ_long), 0);
}
