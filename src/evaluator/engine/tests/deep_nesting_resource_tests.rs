// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Deep-nesting recursion-limit tests and resource-exhaustion
//! stress tests (large rule counts, large buffers) that must
//! complete or time out without panicking.

use super::*;

#[test]
fn test_deep_nesting_twenty_levels_all_match() {
    // Build a 20-level linear nested rule tree. With max_recursion_depth=20,
    // every level should match. A 20-level tree only needs 19 increments of
    // the recursion counter (the leaf has no children), so it fits under the
    // default limit exactly.
    let (root, buffer) = build_linear_nested_chain(20);
    let rules = vec![root];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, &buffer, &mut context)
        .expect("20-level chain should evaluate without error under default limit");

    assert_eq!(
        matches.len(),
        20,
        "Every one of 20 nested levels should match, got {}",
        matches.len()
    );
    for (i, m) in matches.iter().enumerate() {
        assert_eq!(m.message, format!("Level {i}"));
    }
}

#[test]
fn test_deep_nesting_exceeds_limit_returns_recursion_error() {
    // Same 20-level tree, but with max_recursion_depth=5. Evaluation must
    // return RecursionLimitExceeded and must not panic.
    let (root, buffer) = build_linear_nested_chain(20);
    let rules = vec![root];

    let config = EvaluationConfig {
        max_recursion_depth: 5,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, &buffer, &mut context);
    let err = result.expect_err("Expected RecursionLimitExceeded for 20-level tree with limit 5");

    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            )
        ),
        "Expected RecursionLimitExceeded, got: {err:?}"
    );
}

#[test]
fn test_resource_exhaustion_large_rule_count_completes_or_times_out() {
    // Generate 2000 independent flat rules. Each rule reads byte 0 and
    // compares against a distinct value; only one will match. Evaluation
    // must either complete successfully or return a Timeout error -- it
    // must never panic and must finish well under the test timeout.
    let rule_count: u32 = 2000;
    let rules: Vec<MagicRule> = (0..rule_count)
        .map(|i| MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i & 0xFF)),
            message: format!("Rule {i}"),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        })
        .collect();

    // Buffer containing 0x00 so exactly the rules with value 0 match
    // (there will be several because we mask with 0xFF above).
    let buffer = vec![0u8; 64];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        // Generous timeout: even CI should finish 2000 trivial byte reads
        // in well under 10 seconds.
        timeout_ms: Some(10_000),
    };
    let mut context = EvaluationContext::new(config);

    let start = std::time::Instant::now();
    let result = evaluate_rules(&rules, &buffer, &mut context);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "Large-rule-count evaluation took too long: {elapsed:?}"
    );

    match result {
        Ok(matches) => {
            // Rules whose (i & 0xFF) == 0 match byte 0: that's rule_count/256
            // rounded up. Verify we got at least one match and did not panic.
            assert!(
                !matches.is_empty(),
                "Expected at least one match in large rule set"
            );
        }
        Err(LibmagicError::Timeout { .. }) => {
            // Acceptable: the timeout fired instead of completing.
        }
        Err(e) => panic!("Unexpected error from large rule set evaluation: {e:?}"),
    }
}

#[test]
fn test_resource_exhaustion_large_buffer_completes_without_panic() {
    // 1 MiB buffer of zeros. Evaluate a handful of rules across it and
    // verify no panic and reasonable runtime.
    let buffer = vec![0u8; 1024 * 1024];

    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "zero byte at 0".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0),
            message: "zero long at 0".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(512 * 1024),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "zero byte at mid".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
        // Out-of-bounds rule should fail gracefully, not panic.
        MagicRule {
            offset: OffsetSpec::Absolute(i64::try_from(buffer.len() + 100).unwrap()),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "out of bounds".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        },
    ];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: Some(10_000),
    };
    let mut context = EvaluationContext::new(config);

    let start = std::time::Instant::now();
    let matches = evaluate_rules(&rules, &buffer, &mut context)
        .expect("Large-buffer evaluation should not return an error");
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "Large-buffer evaluation took too long: {elapsed:?}"
    );

    // Three in-bounds rules should match; the out-of-bounds rule should
    // silently fail without panicking.
    assert_eq!(
        matches.len(),
        3,
        "Expected 3 matches (in-bounds rules only), got {}",
        matches.len()
    );
}
