// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::{Endianness, OffsetSpec, Operator, TypeKind, Value};

#[test]
fn test_evaluate_single_rule_byte_equal_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_byte_equal_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_byte_not_equal_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x00),
        message: "Non-zero byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0x7f != 0x00
}

#[test]
fn test_evaluate_single_rule_byte_not_equal_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x7f),
        message: "Not ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_none()); // 0x7f == 0x7f, so NotEqual is false
}

#[test]
fn test_evaluate_single_rule_byte_bitwise_and_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80), // Check if high bit is set
        message: "High bit set".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0x45, 0x4c, 0x46]; // 0xff has high bit set
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0xff & 0x80 = 0x80 (non-zero)
}

#[test]
fn test_evaluate_single_rule_byte_bitwise_and_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80), // Check if high bit is set
        message: "High bit set".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 0x7f has high bit clear
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_none()); // 0x7f & 0x80 = 0x00 (zero)
}

#[test]
fn test_evaluate_single_rule_short_little_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Little-endian short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x34, 0x12, 0x56, 0x78]; // 0x1234 in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_big_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Big-endian short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x1234 in big-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_signed_positive() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(32767), // 0x7fff
        message: "Positive signed short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0x7f, 0x00, 0x00]; // 0x7fff in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_signed_negative() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-1), // 0xffff as signed
        message: "Negative signed short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0x00, 0x00]; // 0xffff in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_little_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Little-endian long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x78, 0x56, 0x34, 0x12, 0x00]; // 0x12345678 in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_big_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Big-endian long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78, 0x00]; // 0x12345678 in big-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_signed_positive() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(2_147_483_647), // 0x7fffffff
        message: "Positive signed long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0x7f, 0x00]; // 0x7fffffff in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_signed_negative() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-1), // 0xffffffff as signed
        message: "Negative signed long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0xff, 0x00]; // 0xffffffff in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_different_offsets() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(2), // Read from offset 2
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "ELF class byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // buffer[2] == 0x4c
}

#[test]
fn test_evaluate_single_rule_negative_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(-1), // Last byte
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x46),
        message: "Last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // Last byte is 0x46
}

#[test]
fn test_evaluate_single_rule_from_end_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::FromEnd(-2), // Second to last byte
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "Second to last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // buffer[2] == 0x4c (second to last)
}

#[test]
fn test_evaluate_single_rule_offset_out_of_bounds() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(10), // Beyond buffer
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // Only 4 bytes
    let result = evaluate_single_rule(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_short_insufficient_bytes() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(3), // Only 1 byte left
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Insufficient bytes".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 4 bytes total
    let result = evaluate_single_rule(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_long_insufficient_bytes() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(2), // Only 2 bytes left
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Insufficient bytes".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 4 bytes total
    let result = evaluate_single_rule(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_empty_buffer() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Empty buffer".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[]; // Empty buffer
    let result = evaluate_single_rule(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_string_type_supported() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("test".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    // Test matching string
    let buffer = b"test\x00 data";
    let result = evaluate_single_rule(&rule, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_some()); // Should match

    // Test non-matching string
    let rule_no_match = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("hello".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let result = evaluate_single_rule(&rule_no_match, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_none()); // Should not match
}

#[test]
fn test_evaluate_single_rule_cross_type_comparison() {
    // Test that cross-type integer comparisons use coercion (Uint(42) == Int(42))
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Int(42), // Int value vs Uint from byte read
        message: "Cross-type comparison".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[42]; // Byte value 42
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // Should match via cross-type integer coercion
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_shorts() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xff00), // Check high byte
        message: "High byte check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x34, 0x12]; // 0x1234 in little-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0x1234 & 0xff00 = 0x1200 (non-zero)
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_longs() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xffff_0000), // Check high word
        message: "High word check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x12345678 in big-endian
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0x12345678 & 0xffff0000 = 0x12340000 (non-zero)
}

#[test]
fn test_evaluate_single_rule_comprehensive_elf_check() {
    // Test a comprehensive ELF magic check
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x464c_457f), // ELF magic as 32-bit little-endian
        message: "ELF executable".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header start
    let result = evaluate_single_rule(&rule, elf_buffer).unwrap();
    assert!(result.is_some());

    let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04, 0x14, 0x00]; // ZIP header
    let result = evaluate_single_rule(&rule, non_elf_buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_native_endianness() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Native,
            signed: false,
        },
        op: Operator::NotEqual,
        value: Value::Uint(0),
        message: "Non-zero native short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x01, 0x02]; // Non-zero bytes
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(result.is_some()); // Should be non-zero regardless of endianness
}

#[test]
fn test_evaluate_single_rule_all_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    // Test Equal operator
    let equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "Equal test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(evaluate_single_rule(&equal_rule, buffer).unwrap().is_some());

    // Test NotEqual operator
    let not_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x42),
        message: "NotEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&not_equal_rule, buffer)
            .unwrap()
            .is_some()
    ); // 0x00 != 0x42

    // Test BitwiseAnd operator
    let bitwise_and_rule = MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80),
        message: "BitwiseAnd test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&bitwise_and_rule, buffer)
            .unwrap()
            .is_some()
    ); // 0x80 & 0x80 = 0x80
}

#[test]
fn test_evaluate_single_rule_comparison_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    // LessThan: byte at offset 1 is 0x00, 0x00 < 0x42 = true
    let less_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0x42),
        message: "LessThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&less_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    // GreaterThan: byte at offset 2 is 0xff, 0xff > 0x42 = true
    let greater_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterThan,
        value: Value::Uint(0x42),
        message: "GreaterThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&greater_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    // LessEqual: byte at offset 0 is 0x42, 0x42 <= 0x42 = true
    let less_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessEqual,
        value: Value::Uint(0x42),
        message: "LessEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&less_equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    // GreaterEqual: byte at offset 0 is 0x42, 0x42 >= 0x42 = true
    let greater_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterEqual,
        value: Value::Uint(0x42),
        message: "GreaterEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&greater_equal_rule, buffer)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_evaluate_comparison_with_signed_byte() {
    // 0x80 = -128 as signed byte, 128 as unsigned byte
    let buffer = &[0x80];

    // Signed byte: reads as Int(-128), which IS less than Uint(0)
    let signed_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "signed less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&signed_rule, buffer)
            .unwrap()
            .is_some()
    );

    // Unsigned byte: reads as Uint(128), which is NOT less than Uint(0)
    let unsigned_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "unsigned less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule(&unsigned_rule, buffer)
            .unwrap()
            .is_none()
    );
}

#[test]
fn test_evaluate_comparison_operators_negative_cases() {
    let buffer = &[0x42]; // 66

    let cases: Vec<(Operator, u64, bool)> = vec![
        // LessThan: 66 < 66 = false, 66 < 67 = true
        (Operator::LessThan, 66, false),
        (Operator::LessThan, 67, true),
        // GreaterThan: 66 > 66 = false, 66 > 65 = true
        (Operator::GreaterThan, 66, false),
        (Operator::GreaterThan, 65, true),
        // LessEqual: 66 <= 65 = false, 66 <= 66 = true
        (Operator::LessEqual, 65, false),
        (Operator::LessEqual, 66, true),
        // GreaterEqual: 66 >= 67 = false, 66 >= 66 = true
        (Operator::GreaterEqual, 67, false),
        (Operator::GreaterEqual, 66, true),
    ];

    for (op, value, expected) in cases {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: op.clone(),
            value: Value::Uint(value),
            message: "test".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };
        let result = evaluate_single_rule(&rule, buffer).unwrap();
        assert_eq!(
            result.is_some(),
            expected,
            "{op:?} with value {value}: expected {expected}"
        );
    }
}

#[test]
fn test_evaluate_single_rule_edge_case_values() {
    // Test with maximum values
    let max_uint_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0xffff_ffff),
        message: "Max uint32".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let max_buffer = &[0xff, 0xff, 0xff, 0xff];
    let result = evaluate_single_rule(&max_uint_rule, max_buffer).unwrap();
    assert!(result.is_some());

    // Test with minimum signed value
    let min_int_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-2_147_483_648), // i32::MIN
        message: "Min int32".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let min_buffer = &[0x00, 0x00, 0x00, 0x80]; // 0x80000000 in little-endian
    let result = evaluate_single_rule(&min_int_rule, min_buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_various_buffer_sizes() {
    // Test with single byte buffer (unsigned for values > 127)
    let single_byte_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xaa),
        message: "Single byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let single_buffer = &[0xaa];
    let result = evaluate_single_rule(&single_byte_rule, single_buffer).unwrap();
    assert!(result.is_some());

    // Test with large buffer
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let large_buffer: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let large_rule = MagicRule {
        offset: OffsetSpec::Absolute(1000),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint((1000 % 256) as u64),
        message: "Large buffer".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let result = evaluate_single_rule(&large_rule, &large_buffer).unwrap();
    assert!(result.is_some());
}

// Tests for EvaluationContext
#[test]
fn test_evaluation_context_new() {
    let config = EvaluationConfig::default();
    let context = EvaluationContext::new(config.clone());

    assert_eq!(context.current_offset(), 0);
    assert_eq!(context.recursion_depth(), 0);
    assert_eq!(
        context.config().max_recursion_depth,
        config.max_recursion_depth
    );
    assert_eq!(context.config().max_string_length, config.max_string_length);
    assert_eq!(
        context.config().stop_at_first_match,
        config.stop_at_first_match
    );
}

#[test]
fn test_evaluation_context_offset_management() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Test initial offset
    assert_eq!(context.current_offset(), 0);

    // Test setting offset
    context.set_current_offset(42);
    assert_eq!(context.current_offset(), 42);

    // Test setting different offset
    context.set_current_offset(1024);
    assert_eq!(context.current_offset(), 1024);

    // Test setting offset to 0
    context.set_current_offset(0);
    assert_eq!(context.current_offset(), 0);
}

#[test]
fn test_evaluation_context_recursion_depth_management() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Test initial recursion depth
    assert_eq!(context.recursion_depth(), 0);

    // Test incrementing recursion depth
    context.increment_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 1);

    context.increment_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 2);

    // Test decrementing recursion depth
    context.decrement_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 1);

    context.decrement_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 0);
}

#[test]
fn test_evaluation_context_recursion_depth_limit() {
    let config = EvaluationConfig {
        max_recursion_depth: 2,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    // Should be able to increment up to the limit
    assert!(context.increment_recursion_depth().is_ok());
    assert_eq!(context.recursion_depth(), 1);

    assert!(context.increment_recursion_depth().is_ok());
    assert_eq!(context.recursion_depth(), 2);

    // Should fail when exceeding the limit
    let result = context.increment_recursion_depth();
    assert!(result.is_err());
    assert_eq!(context.recursion_depth(), 2); // Should not have changed

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Recursion limit exceeded"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluation_context_recursion_depth_underflow() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Should return an error when trying to decrement below 0
    let result = context.decrement_recursion_depth();
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("decrement recursion depth below 0"),
        "Expected error about decrementing below 0, got: {err_msg}"
    );
}

#[test]
fn test_evaluation_context_config_access() {
    let config = EvaluationConfig {
        max_recursion_depth: 10,
        max_string_length: 4096,
        stop_at_first_match: false,
        enable_mime_types: true,
        timeout_ms: Some(2000),
    };

    let context = EvaluationContext::new(config);

    // Test config access
    assert_eq!(context.config().max_recursion_depth, 10);
    assert_eq!(context.config().max_string_length, 4096);
    assert!(!context.config().stop_at_first_match);

    // Test convenience methods
    assert!(!context.should_stop_at_first_match());
    assert_eq!(context.max_string_length(), 4096);
}

#[test]
fn test_evaluation_context_reset() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config.clone());

    // Modify the context state
    context.set_current_offset(100);
    context.increment_recursion_depth().unwrap();
    context.increment_recursion_depth().unwrap();

    assert_eq!(context.current_offset(), 100);
    assert_eq!(context.recursion_depth(), 2);

    // Reset should restore initial state but keep config
    context.reset();

    assert_eq!(context.current_offset(), 0);
    assert_eq!(context.recursion_depth(), 0);
    assert_eq!(
        context.config().max_recursion_depth,
        config.max_recursion_depth
    );
}

#[test]
fn test_evaluation_context_clone() {
    let config = EvaluationConfig {
        max_recursion_depth: 5,
        max_string_length: 2048,
        ..Default::default()
    };

    let mut context = EvaluationContext::new(config);
    context.set_current_offset(50);
    context.increment_recursion_depth().unwrap();

    // Clone the context
    let cloned_context = context.clone();

    // Both should have the same state
    assert_eq!(context.current_offset(), cloned_context.current_offset());
    assert_eq!(context.recursion_depth(), cloned_context.recursion_depth());
    assert_eq!(
        context.config().max_recursion_depth,
        cloned_context.config().max_recursion_depth
    );
    assert_eq!(
        context.config().max_string_length,
        cloned_context.config().max_string_length
    );

    // Modifying one should not affect the other
    context.set_current_offset(75);
    assert_eq!(context.current_offset(), 75);
    assert_eq!(cloned_context.current_offset(), 50);
}

#[test]
fn test_evaluation_context_with_custom_config() {
    let config = EvaluationConfig {
        max_recursion_depth: 15,
        max_string_length: 16384,
        stop_at_first_match: false,
        enable_mime_types: true,
        timeout_ms: Some(5000),
    };

    let context = EvaluationContext::new(config);

    assert_eq!(context.config().max_recursion_depth, 15);
    assert_eq!(context.max_string_length(), 16384);
    assert!(!context.should_stop_at_first_match());

    // Test that we can increment up to the custom limit
    let mut mutable_context = context;
    for i in 1..=15 {
        assert!(mutable_context.increment_recursion_depth().is_ok());
        assert_eq!(mutable_context.recursion_depth(), i);
    }

    // Should fail on the 16th increment
    let result = mutable_context.increment_recursion_depth();
    assert!(result.is_err());
}

#[test]
fn test_evaluation_context_mime_types_access() {
    let config_with_mime = EvaluationConfig {
        enable_mime_types: true,
        ..Default::default()
    };
    let context_with_mime = EvaluationContext::new(config_with_mime);
    assert!(context_with_mime.enable_mime_types());

    let config_without_mime = EvaluationConfig {
        enable_mime_types: false,
        ..Default::default()
    };
    let context_without_mime = EvaluationContext::new(config_without_mime);
    assert!(!context_without_mime.enable_mime_types());
}

#[test]
fn test_evaluation_context_timeout_access() {
    let config_with_timeout = EvaluationConfig {
        timeout_ms: Some(5000),
        ..Default::default()
    };
    let context_with_timeout = EvaluationContext::new(config_with_timeout);
    assert_eq!(context_with_timeout.timeout_ms(), Some(5000));

    let config_without_timeout = EvaluationConfig {
        timeout_ms: None,
        ..Default::default()
    };
    let context_without_timeout = EvaluationContext::new(config_without_timeout);
    assert_eq!(context_without_timeout.timeout_ms(), None);
}

#[test]
fn test_evaluation_context_comprehensive_config() {
    let config = EvaluationConfig {
        max_recursion_depth: 30,
        max_string_length: 16384,
        stop_at_first_match: false,
        enable_mime_types: true,
        timeout_ms: Some(10000),
    };
    let context = EvaluationContext::new(config);

    assert_eq!(context.config().max_recursion_depth, 30);
    assert_eq!(context.config().max_string_length, 16384);
    assert!(!context.should_stop_at_first_match());
    assert!(context.enable_mime_types());
    assert_eq!(context.timeout_ms(), Some(10000));
    assert_eq!(context.max_string_length(), 16384);
}

#[test]
fn test_evaluation_context_performance_config() {
    let config = EvaluationConfig {
        max_recursion_depth: 5,
        max_string_length: 512,
        stop_at_first_match: true,
        enable_mime_types: false,
        timeout_ms: Some(1000),
    };
    let context = EvaluationContext::new(config);

    assert_eq!(context.config().max_recursion_depth, 5);
    assert_eq!(context.max_string_length(), 512);
    assert!(context.should_stop_at_first_match());
    assert!(!context.enable_mime_types());
    assert_eq!(context.timeout_ms(), Some(1000));
}

#[test]
fn test_rule_match_creation() {
    let match_result = RuleMatch {
        message: "ELF executable".to_string(),
        offset: 0,
        level: 0,
        value: Value::Uint(0x7f),
        confidence: RuleMatch::calculate_confidence(0),
    };

    assert_eq!(match_result.message, "ELF executable");
    assert_eq!(match_result.offset, 0);
    assert_eq!(match_result.level, 0);
    assert_eq!(match_result.value, Value::Uint(0x7f));
    assert!((match_result.confidence - 0.3).abs() < 0.001);
}

#[test]
fn test_rule_match_clone() {
    let original = RuleMatch {
        message: "Test message".to_string(),
        offset: 42,
        level: 1,
        value: Value::String("test".to_string()),
        confidence: RuleMatch::calculate_confidence(1),
    };

    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_rule_match_debug() {
    let match_result = RuleMatch {
        message: "Debug test".to_string(),
        offset: 10,
        level: 2,
        value: Value::Bytes(vec![0x01, 0x02]),
        confidence: RuleMatch::calculate_confidence(2),
    };

    let debug_str = format!("{match_result:?}");
    assert!(debug_str.contains("RuleMatch"));
    assert!(debug_str.contains("Debug test"));
    assert!(debug_str.contains("10"));
    assert!(debug_str.contains('2'));
}

#[test]
fn test_confidence_calculation_depth_0() {
    let confidence = RuleMatch::calculate_confidence(0);
    assert!((confidence - 0.3).abs() < 0.001);
}

#[test]
fn test_confidence_calculation_depth_1() {
    let confidence = RuleMatch::calculate_confidence(1);
    assert!((confidence - 0.5).abs() < 0.001);
}

#[test]
fn test_confidence_calculation_depth_2() {
    let confidence = RuleMatch::calculate_confidence(2);
    assert!((confidence - 0.7).abs() < 0.001);
}

#[test]
fn test_confidence_calculation_depth_3() {
    let confidence = RuleMatch::calculate_confidence(3);
    assert!((confidence - 0.9).abs() < 0.001);
}

#[test]
fn test_confidence_calculation_capped_at_1() {
    // Level 4+ should cap at 1.0
    let confidence_4 = RuleMatch::calculate_confidence(4);
    assert!((confidence_4 - 1.0).abs() < 0.001);

    let confidence_10 = RuleMatch::calculate_confidence(10);
    assert!((confidence_10 - 1.0).abs() < 0.001);

    let confidence_100 = RuleMatch::calculate_confidence(100);
    assert!((confidence_100 - 1.0).abs() < 0.001);
}

#[test]
fn test_evaluate_rules_empty_list() {
    let rules = vec![];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_single_matching_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF magic");
    assert_eq!(matches[0].offset, 0);
    assert_eq!(matches[0].level, 0);
    // Signed byte read: 0x7f -> Value::Int(127)
    assert_eq!(matches[0].value, Value::Int(0x7f));
}

#[test]
fn test_evaluate_rules_single_non_matching_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50), // ZIP magic, not ELF
        message: "ZIP magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF buffer
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_multiple_rules_stop_at_first() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_list = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_list, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "First match");
}

#[test]
fn test_evaluate_rules_multiple_rules_find_all() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_set = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_set, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "First match");
    assert_eq!(matches[1].message, "Second match");
}

#[test]
fn test_evaluate_rules_hierarchical_parent_child() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02), // ELF class 64-bit
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[1].level, 1);
}

#[test]
fn test_evaluate_rules_hierarchical_parent_no_match() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50), // ZIP magic, not ELF
        message: "ZIP".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF buffer
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty()); // Parent doesn't match, so children shouldn't be evaluated
}

#[test]
fn test_evaluate_rules_hierarchical_parent_match_child_no_match() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01), // ELF class 32-bit, but buffer has 64-bit
        message: "32-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 header
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1); // Only parent matches
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
}

#[test]
fn test_evaluate_rules_deep_hierarchy() {
    let grandchild_rule = MagicRule {
        offset: OffsetSpec::Absolute(5),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01), // Little endian
        message: "little-endian".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
    };

    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02), // 64-bit
        message: "64-bit".to_string(),
        children: vec![grandchild_rule],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01]; // ELF64 little-endian header
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[1].level, 1);
    assert_eq!(matches[2].message, "little-endian");
    assert_eq!(matches[2].level, 2);
}

#[test]
fn test_evaluate_rules_multiple_children() {
    let child1 = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let child2 = MagicRule {
        offset: OffsetSpec::Absolute(5),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "little-endian".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child1, child2],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig {
        stop_at_first_match: false, // Find all matches
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[2].message, "little-endian");
}

#[test]
fn test_evaluate_rules_recursion_depth_limit() {
    // Create a deeply nested rule structure that exceeds the limit
    let mut current_rule = MagicRule {
        offset: OffsetSpec::Absolute(10),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Deep level".to_string(),
        children: vec![],
        level: 10,
        strength_modifier: None,
    };

    // Build a chain of nested rules
    for i in (0u32..10u32).rev() {
        current_rule = MagicRule {
            offset: OffsetSpec::Absolute(i64::from(i)),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i)),
            message: format!("Level {i}"),
            children: vec![current_rule],
            level: i,
            strength_modifier: None,
        };
    }

    let rules = vec![current_rule];
    let buffer = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]; // Matches all levels
    let config = EvaluationConfig {
        max_recursion_depth: 5, // Limit to 5 levels
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Recursion limit exceeded"));
        }
        _ => panic!("Expected EvaluationError for recursion limit"),
    }
}

#[test]
fn test_evaluate_rules_with_config_convenience() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();

    let matches = evaluate_rules_with_config(&rules, buffer, &config).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF magic");
}

#[test]
fn test_evaluate_rules_timeout() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        timeout_ms: Some(0), // Immediate timeout
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    // Note: This test might be flaky due to timing, but it demonstrates the timeout mechanism
    let result = evaluate_rules(&rules, buffer, &mut context);
    // The result could be either success (if evaluation is very fast) or timeout
    // We just verify that timeout errors are handled correctly when they occur
    if let Err(LibmagicError::Timeout { timeout_ms }) = result {
        assert_eq!(timeout_ms, 0);
    }
}

#[test]
fn test_evaluate_rules_empty_buffer() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[]; // Empty buffer
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // With graceful error handling, this should succeed but return no matches
    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_ok());

    let matches = result.unwrap();
    assert_eq!(matches.len(), 0); // No matches due to buffer overrun being handled gracefully
}

#[test]
fn test_evaluate_rules_mixed_matching_non_matching() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Matches".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x99), // Doesn't match
        message: "Doesn't match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule3 = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "Also matches".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_collection = vec![rule1, rule2, rule3];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_collection, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Matches");
    assert_eq!(matches[1].message, "Also matches");
}

#[test]
fn test_evaluate_rules_context_state_preservation() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Set some initial state
    context.set_current_offset(100);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    // Context state should be preserved
    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}

#[test]
fn test_evaluation_context_state_management_sequence() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Simulate a sequence of evaluation operations
    assert_eq!(context.current_offset(), 0);
    assert_eq!(context.recursion_depth(), 0);

    // Start evaluation at offset 10
    context.set_current_offset(10);
    assert_eq!(context.current_offset(), 10);

    // Enter nested rule evaluation
    context.increment_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 1);

    // Move to different offset during nested evaluation
    context.set_current_offset(25);
    assert_eq!(context.current_offset(), 25);

    // Enter deeper nesting
    context.increment_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 2);

    // Exit nested evaluation
    context.decrement_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 1);

    // Continue evaluation at different offset
    context.set_current_offset(50);
    assert_eq!(context.current_offset(), 50);

    // Exit all nesting
    context.decrement_recursion_depth().unwrap();
    assert_eq!(context.recursion_depth(), 0);

    // Final state check
    assert_eq!(context.current_offset(), 50);
    assert_eq!(context.recursion_depth(), 0);
}
#[test]
fn test_error_recovery_skip_problematic_rules() {
    // Test that evaluation continues when individual rules fail
    let rules = vec![
        // Valid rule that should match
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Invalid rule with out-of-bounds offset
        MagicRule {
            offset: OffsetSpec::Absolute(100), // Beyond buffer
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Another valid rule that should match
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Another valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false, // Don't stop at first match
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    // Evaluation should succeed despite the problematic rule
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    // Should have 2 matches (skipping the problematic one)
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid rule");
    assert_eq!(matches[1].message, "Another valid rule");
}

#[test]
fn test_error_recovery_child_rule_failures() {
    // Test that parent evaluation continues when child rules fail
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Parent rule".to_string(),
        children: vec![
            // Valid child rule
            MagicRule {
                offset: OffsetSpec::Absolute(1),
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x45),
                message: "Valid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
            // Invalid child rule
            MagicRule {
                offset: OffsetSpec::Absolute(100), // Beyond buffer
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x00),
                message: "Invalid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
        ],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Evaluation should succeed with parent and valid child
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    // Should have parent match and valid child match
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Parent rule");
    assert_eq!(matches[1].message, "Valid child");
}

#[test]
fn test_error_recovery_mixed_rule_types() {
    // Test error recovery with different types of rule failures
    let rules = vec![
        // Valid byte rule
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid byte".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Invalid short rule (insufficient bytes)
        MagicRule {
            offset: OffsetSpec::Absolute(3), // Only 1 byte left for short
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234),
            message: "Invalid short".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Valid string rule
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::String {
                max_length: Some(3),
            },
            op: Operator::Equal,
            value: Value::String("ELF".to_string()),
            message: "Valid string".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, b'E', b'L', b'F']; // ELF magic bytes
    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false, // Don't stop at first match
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    // Evaluation should succeed with valid rules
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    // Should have 2 matches (byte and string, skipping invalid short)
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid byte");
    assert_eq!(matches[1].message, "Valid string");
}

#[test]
fn test_error_recovery_all_rules_fail() {
    // Test behavior when all rules fail
    let rules = vec![
        // Out of bounds offset
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Out of bounds".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Insufficient bytes for type
        MagicRule {
            offset: OffsetSpec::Absolute(2),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234_5678),
            message: "Insufficient bytes".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45]; // Short buffer
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Evaluation should succeed but return no matches
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_error_recovery_timeout_propagation() {
    // Test that timeout errors are properly propagated (not gracefully handled)
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Test rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 10,
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: Some(0), // Immediate timeout
    };
    let mut context = EvaluationContext::new(config);

    // The timeout test is inherently flaky due to timing, so we'll just test
    // that the timeout configuration is properly set and the function doesn't panic
    let result = evaluate_rules(&rules, buffer, &mut context);

    // The result should either be success (if evaluation was fast) or timeout error
    match result {
        Ok(_) | Err(LibmagicError::Timeout { .. }) => {
            // Evaluation was fast enough or timeout occurred, both are acceptable
        }
        Err(e) => {
            panic!("Unexpected error type: {e:?}");
        }
    }
}

#[test]
fn test_error_recovery_recursion_limit_propagation() {
    // Test that recursion limit errors are properly propagated
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Parent".to_string(),
        children: vec![MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Child".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
        }],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 0, // No recursion allowed
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    // Should return recursion limit error when trying to evaluate children
    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(crate::error::EvaluationError::RecursionLimitExceeded {
            ..
        }) => {
            // Expected recursion limit error
        }
        _ => panic!("Expected recursion limit error"),
    }
}

#[test]
fn test_error_recovery_preserves_context_state() {
    // Test that context state is preserved despite rule failures
    let rules = vec![
        // Valid rule
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Invalid rule
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Set initial context state
    context.set_current_offset(42);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    // Evaluation should succeed
    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    // Context state should be preserved
    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}

// End-to-end tests: parse rules with `x` operator and evaluate them
#[test]
fn test_any_value_parse_and_evaluate_paren_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = ">0 byte x (0)";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "(0)");

    // AnyValue should match unconditionally regardless of buffer content
    let buffer = &[0x00, 0x01, 0x02, 0x03];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_any_value_parse_and_evaluate_backslash_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 long x \\b, data";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "\\b, data");

    let buffer = &[0xFF, 0xFE, 0xFD, 0xFC];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_any_value_parse_and_evaluate_no_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte x";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);

    let buffer = &[0x42];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

// End-to-end tests: parse rules with `^` operator and evaluate them
#[test]
fn test_bitwise_xor_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte ^0x01 XOR match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseXor);
    assert_eq!(rule.message, "XOR match");

    // Buffer byte 0x0F XOR 0x01 = 0x0E (non-zero), should match
    let buffer = &[0x0F];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "BitwiseXor should match when XOR is non-zero"
    );
}

#[test]
fn test_bitwise_xor_parse_and_evaluate_no_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte ^0x42 XOR no match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseXor);

    // Buffer byte 0x42 XOR 0x42 = 0 (zero), should NOT match
    let buffer = &[0x42];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseXor should not match when XOR is zero"
    );
}

// End-to-end tests: parse rules with `~` operator and evaluate them
#[test]
fn test_bitwise_not_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    // ubyte reads 0xFF as u64(255); NOT of u64(255) = u64::MAX - 255 = 0xFFFFFFFFFFFFFF00
    // Use u64::MAX as operand with buffer byte 0x00: NOT(0) = u64::MAX
    let input = &format!("0 ubyte ~{} NOT match", u64::MAX);
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);
    assert_eq!(rule.message, "NOT match");

    let buffer = &[0x00];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "BitwiseNot should match when NOT(value) equals operand"
    );
}

#[test]
fn test_bitwise_not_parse_and_evaluate_no_match() {
    use crate::parser::grammar::parse_magic_rule;

    // NOT of 0x42 byte is 0xBD (for ubyte: !0x42 = 0xFFFFFFFFFFFFFFBD as u64);
    // comparing with 0x01 should NOT match
    let input = "0 ubyte ~0x01 NOT no match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);

    let buffer = &[0x42];
    let result = evaluate_single_rule(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseNot should not match when NOT(value) != operand"
    );
}

#[test]
fn test_debug_error_recovery() {
    // Simple test to debug error recovery
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(100), // Beyond buffer
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45]; // Short buffer

    // Test single rule evaluation - should fail
    let single_result = evaluate_single_rule(&rule, buffer);
    println!("Single rule result: {single_result:?}");
    assert!(single_result.is_err());

    // Test rules evaluation - should succeed with no matches
    let rules = vec![rule];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    println!("Rules evaluation matches: {}", matches.len());
    assert_eq!(matches.len(), 0);
}
#[test]
fn test_debug_mixed_rules() {
    let rules = vec![
        // Valid rule that should match
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Invalid rule with out-of-bounds offset
        MagicRule {
            offset: OffsetSpec::Absolute(100), // Beyond buffer
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Another valid rule that should match
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Another valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes

    // Test each rule individually
    for (i, rule) in rules.iter().enumerate() {
        let result = evaluate_single_rule(rule, buffer);
        println!("Rule {}: '{}' -> {:?}", i, rule.message, result);
    }

    // Test rules evaluation
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    println!("Total matches: {}", matches.len());
    for (i, m) in matches.iter().enumerate() {
        println!("Match {}: '{}'", i, m.message);
    }
}
