// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Single-rule evaluation tests for out-of-bounds/insufficient-byte
//! error paths, `string` type support, cross-type comparison, and
//! bitwise-AND matching on `short`/`long` values.

use super::*;

#[test]
fn test_evaluate_single_rule_offset_out_of_bounds() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(10),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
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
        offset: OffsetSpec::Absolute(3),
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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
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
        value_transform: None,
    };

    let buffer = &[];
    let result = evaluate_single_rule_legacy(&rule, buffer);
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
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("test".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = b"test\x00 data";
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_some());

    let rule_no_match = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("hello".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let result = evaluate_single_rule_legacy(&rule_no_match, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_none());
}

#[test]
fn test_evaluate_single_rule_cross_type_comparison() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Int(42),
        message: "Cross-type comparison".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_shorts() {
    // BitwiseAnd requires ALL masked bits to be set (see GOTCHAS S13.3):
    // `(value & mask) == mask`, not merely "some bit overlaps". The mask
    // here (0xff00) asks "is the entire high byte set" -- so the buffer's
    // high byte must genuinely be 0xff for this to match.
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xff00),
        message: "High byte check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    // Little-endian [0x34, 0xff] -> 0xff34; high byte is 0xff, all mask bits set.
    let matching_buffer = &[0x34, 0xff];
    let result = evaluate_single_rule_legacy(&rule, matching_buffer).unwrap();
    assert!(result.is_some());

    // Little-endian [0x34, 0x12] -> 0x1234; high byte is 0x12, not all mask
    // bits set, so this must NOT match under the corrected semantics.
    let non_matching_buffer = &[0x34, 0x12];
    let result = evaluate_single_rule_legacy(&rule, non_matching_buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_longs() {
    // Same "all masked bits set" contract as the shorts test above, applied
    // to a 32-bit mask spanning the high word.
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xffff_0000),
        message: "High word check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    // Big-endian [0xff, 0xff, 0x56, 0x78] -> 0xffff5678; high word is 0xffff,
    // all mask bits set.
    let matching_buffer = &[0xff, 0xff, 0x56, 0x78];
    let result = evaluate_single_rule_legacy(&rule, matching_buffer).unwrap();
    assert!(result.is_some());

    // Big-endian [0x12, 0x34, 0x56, 0x78] -> 0x12345678; high word is
    // 0x1234, not all mask bits set, so this must NOT match.
    let non_matching_buffer = &[0x12, 0x34, 0x56, 0x78];
    let result = evaluate_single_rule_legacy(&rule, non_matching_buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_comprehensive_elf_check() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x464c_457f),
        message: "ELF executable".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let result = evaluate_single_rule_legacy(&rule, elf_buffer).unwrap();
    assert!(result.is_some());

    let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04, 0x14, 0x00];
    let result = evaluate_single_rule_legacy(&rule, non_elf_buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x01, 0x02];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}
