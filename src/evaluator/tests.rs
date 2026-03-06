// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::Value;

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
        LibmagicError::EvaluationError(crate::error::EvaluationError::RecursionLimitExceeded {
            depth,
        }) => {
            assert_eq!(depth, 2);
        }
        other => panic!("Expected RecursionLimitExceeded, got: {other:?}"),
    }
}

#[test]
fn test_evaluation_context_recursion_depth_underflow() {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    // Should return an error when trying to decrement below 0
    let result = context.decrement_recursion_depth();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(crate::error::EvaluationError::InternalError { .. }) => {}
        other => panic!("Expected InternalError, got: {other:?}"),
    }
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
