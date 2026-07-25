// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests exercising every [`Operator`] variant end-to-end: full
//! comparison-operator coverage, `x` (any-value), and the bitwise
//! XOR/NOT operators via parse-and-evaluate round trips.

use super::*;

#[test]
fn test_evaluate_single_rule_all_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    let equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "Equal test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let not_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x42),
        message: "NotEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&not_equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let bitwise_and_rule = MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80),
        message: "BitwiseAnd test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&bitwise_and_rule, buffer)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_evaluate_single_rule_comparison_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    let less_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0x42),
        message: "LessThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&less_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    let greater_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterThan,
        value: Value::Uint(0x42),
        message: "GreaterThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&greater_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    let less_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessEqual,
        value: Value::Uint(0x42),
        message: "LessEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&less_equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let greater_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterEqual,
        value: Value::Uint(0x42),
        message: "GreaterEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&greater_equal_rule, buffer)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_evaluate_comparison_with_signed_byte() {
    let buffer = &[0x80];

    let signed_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "signed less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&signed_rule, buffer)
            .unwrap()
            .is_some()
    );

    let unsigned_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "unsigned less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    assert!(
        evaluate_single_rule_legacy(&unsigned_rule, buffer)
            .unwrap()
            .is_none()
    );
}

#[test]
fn test_evaluate_comparison_operators_negative_cases() {
    let buffer = &[0x42];

    let cases: Vec<(Operator, u64, bool)> = vec![
        (Operator::LessThan, 66, false),
        (Operator::LessThan, 67, true),
        (Operator::GreaterThan, 66, false),
        (Operator::GreaterThan, 65, true),
        (Operator::LessEqual, 65, false),
        (Operator::LessEqual, 66, true),
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
            value_transform: None,
        };
        let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
        assert_eq!(
            result.is_some(),
            expected,
            "{op:?} with value {value}: expected {expected}"
        );
    }
}

#[test]
fn test_evaluate_single_rule_edge_case_values() {
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
        value_transform: None,
    };

    let max_buffer = &[0xff, 0xff, 0xff, 0xff];
    let result = evaluate_single_rule_legacy(&max_uint_rule, max_buffer).unwrap();
    assert!(result.is_some());

    let min_int_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-2_147_483_648),
        message: "Min int32".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let min_buffer = &[0x00, 0x00, 0x00, 0x80];
    let result = evaluate_single_rule_legacy(&min_int_rule, min_buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_various_buffer_sizes() {
    let single_byte_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xaa),
        message: "Single byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let single_buffer = &[0xaa];
    let result = evaluate_single_rule_legacy(&single_byte_rule, single_buffer).unwrap();
    assert!(result.is_some());

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
        value_transform: None,
    };

    let result = evaluate_single_rule_legacy(&large_rule, &large_buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_any_value_parse_and_evaluate_paren_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = ">0 byte x (0)";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "(0)");

    let buffer = &[0x00, 0x01, 0x02, 0x03];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_bitwise_xor_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte ^0x01 XOR match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseXor);
    assert_eq!(rule.message, "XOR match");

    let buffer = &[0x0F];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
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

    let buffer = &[0x42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseXor should not match when XOR is zero"
    );
}

#[test]
fn test_bitwise_not_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 ubyte ~0xFF NOT match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);
    assert_eq!(rule.message, "NOT match");

    let buffer = &[0x00];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "BitwiseNot should match when NOT(value) equals operand at byte width"
    );
}

#[test]
fn test_bitwise_not_parse_and_evaluate_no_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 ubyte ~0x01 NOT no match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);

    let buffer = &[0x42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseNot should not match when NOT(value) != operand"
    );
}
