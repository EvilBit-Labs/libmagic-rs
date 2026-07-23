// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for hierarchical parent/child rule evaluation: matching
//! and non-matching parents, deep nesting, and multiple children.

use super::*;

#[test]
fn test_evaluate_rules_hierarchical_parent_child() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
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
        value_transform: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50),
        message: "ZIP".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_hierarchical_parent_match_child_no_match() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "32-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
}

#[test]
fn test_evaluate_rules_deep_hierarchy() {
    let grandchild_rule = MagicRule {
        offset: OffsetSpec::Absolute(5),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "little-endian".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
        value_transform: None,
    };

    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![grandchild_rule],
        level: 1,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[2].message, "little-endian");
}
