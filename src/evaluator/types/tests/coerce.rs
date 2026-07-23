// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `coerce_value_to_type` tests -- signed/unsigned reinterpretation across
//! byte/short/long/quad widths, float/double precision rounding, and the
//! numeric-to-timestamp-string formatting for `Date`/`QDate` targets,
//! cross-checked against `read_date`/`read_qdate` output.

use super::*;

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
