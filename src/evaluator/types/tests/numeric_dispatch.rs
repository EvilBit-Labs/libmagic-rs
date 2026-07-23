// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `TypeReadError` basics and `read_typed_value` numeric/string/date
//! dispatch tests -- byte/short/long/quad/float/double/date/qdate
//! coverage, native-endian behavior, buffer-overrun edge cases, and
//! parity checks against the individual `read_*` functions.

use super::*;

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
