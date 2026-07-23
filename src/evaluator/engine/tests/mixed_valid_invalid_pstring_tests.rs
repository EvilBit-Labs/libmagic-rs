// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests mixing valid and invalid rules in the same rule set, and
//! `pstring` (Pascal string) single-rule matching including a
//! matched pstring parent with a child rule.

use super::*;

#[test]
fn test_evaluate_rules_skips_out_of_bounds_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(100),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0x7f, 0x45];

    let single_result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(single_result.is_err());

    let rules = vec![rule];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_mixed_valid_and_invalid_rules_yield_valid_matches() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Another valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];

    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
}

// ============================================================
// PString (Pascal string) Tests
// ============================================================

#[test]
fn test_evaluate_single_rule_pstring_match() {
    // Pascal string: length byte (5) followed by "Hello"
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("Hello".to_string()),
        message: "Pascal string detected".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[5, b'H', b'e', b'l', b'l', b'o'];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "PString rule should match when buffer contains matching pascal string"
    );
}

#[test]
fn test_evaluate_single_rule_pstring_no_match() {
    // Pascal string in buffer is "Hello", rule expects "World"
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("World".to_string()),
        message: "Should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[5, b'H', b'e', b'l', b'l', b'o'];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "PString rule should not match when strings differ"
    );
}

#[test]
fn test_evaluate_single_rule_pstring_with_child_rule() {
    // Parent: PString at offset 0 matches "ELF"
    // Child: byte at offset 4 equals 0x02
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("ELF".to_string()),
        message: "Pascal ELF".to_string(),
        children: vec![MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x02),
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
            value_transform: None,
        }],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    // Buffer: length=3, "ELF", then 0x02 at offset 4
    let buffer = &[3, b'E', b'L', b'F', 0x02];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "Both parent PString and child byte rules should match"
    );
}
