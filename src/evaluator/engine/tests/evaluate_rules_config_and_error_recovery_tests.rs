// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for [`EvaluationConfig`]-driven behavior (recursion depth,
//! timeouts, buffer edge cases) and the evaluator's graceful
//! error-recovery/skip-problematic-rule semantics.

use super::*;

#[test]
fn test_evaluate_rules_recursion_depth_limit() {
    let mut current_rule = MagicRule {
        offset: OffsetSpec::Absolute(10),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Deep level".to_string(),
        children: vec![],
        level: 10,
        strength_modifier: None,
        value_transform: None,
    };

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
            value_transform: None,
        };
    }

    let rules = vec![current_rule];
    let buffer = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    let config = EvaluationConfig {
        max_recursion_depth: 5,
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
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        timeout_ms: Some(0),
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(
        matches!(result, Err(LibmagicError::Timeout { timeout_ms: 0 })),
        "Expected timeout error, got: {result:?}"
    );
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
        value_transform: None,
    };

    let rules = vec![rule];
    let buffer = &[];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_ok());

    let matches = result.unwrap();
    assert_eq!(matches.len(), 0);
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
        value_transform: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x99),
        message: "Doesn't match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    context.set_current_offset(100);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}

#[test]
fn test_error_recovery_skip_problematic_rules() {
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
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid rule");
    assert_eq!(matches[1].message, "Another valid rule");
}

#[test]
fn test_error_recovery_child_rule_failures() {
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Parent rule".to_string(),
        children: vec![
            MagicRule {
                offset: OffsetSpec::Absolute(1),
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x45),
                message: "Valid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
                value_transform: None,
            },
            MagicRule {
                offset: OffsetSpec::Absolute(100),
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x00),
                message: "Invalid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
                value_transform: None,
            },
        ],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Parent rule");
    assert_eq!(matches[1].message, "Valid child");
}

#[test]
fn test_error_recovery_mixed_rule_types() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid byte".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(3),
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
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::String {
                max_length: Some(3),
                flags: StringFlags::default(),
            },
            op: Operator::Equal,
            value: Value::String("ELF".to_string()),
            message: "Valid string".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
    ];

    let buffer = &[0x7f, b'E', b'L', b'F'];
    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid byte");
    assert_eq!(matches[1].message, "Valid string");
}

#[test]
fn test_error_recovery_all_rules_fail() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Out of bounds".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
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
            value_transform: None,
        },
    ];

    let buffer = &[0x7f, 0x45];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_error_recovery_timeout_propagation() {
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Test rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 10,
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: Some(0),
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);

    assert!(
        matches!(result, Err(LibmagicError::Timeout { timeout_ms: 0 })),
        "Expected timeout error, got: {result:?}"
    );
}

#[test]
fn test_error_recovery_recursion_limit_propagation() {
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
            value_transform: None,
        }],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 0,
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(crate::error::EvaluationError::RecursionLimitExceeded {
            ..
        }) => {}
        _ => panic!("Expected recursion limit error"),
    }
}

#[test]
fn test_error_recovery_preserves_context_state() {
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
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    context.set_current_offset(42);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}
