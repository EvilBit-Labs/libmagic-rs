// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Single-rule evaluation tests for `byte`/`short`/`long` numeric
//! types (endianness, signedness, equality/bitwise operators) and
//! basic absolute/negative/from-end offset resolution.

use super::*;

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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0xff, 0x45, 0x4c, 0x46]; // 0xff has high bit set
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 0x7f has high bit clear
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x34, 0x12, 0x56, 0x78]; // 0x1234 in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x1234 in big-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0xff, 0x7f, 0x00, 0x00]; // 0x7fff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0xff, 0xff, 0x00, 0x00]; // 0xffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x78, 0x56, 0x34, 0x12, 0x00]; // 0x12345678 in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78, 0x00]; // 0x12345678 in big-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0x7f, 0x00]; // 0x7fffffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
        value_transform: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0xff, 0x00]; // 0xffffffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_different_offsets() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "ELF class byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_negative_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(-1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x46),
        message: "Last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_from_end_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::FromEnd(-2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "Second to last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}
