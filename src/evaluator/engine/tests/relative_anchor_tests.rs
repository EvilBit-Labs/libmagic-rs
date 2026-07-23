// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for relative-offset (`&N`) anchor resolution against
//! [`EvaluationContext::last_match_end`], including saturation and
//! zero-anchor edge cases.

use super::*;

#[test]
fn test_evaluate_single_rule_relative_resolves_against_anchor_zero() {
    // Public evaluate_single_rule has no EvaluationContext, so OffsetSpec::Relative
    // resolves against an implicit anchor of 0 -- equivalent to absolute offset N.
    // Pin this contract so a future refactor cannot silently regress to UnsupportedType.
    let rule = MagicRule {
        offset: OffsetSpec::Relative(3),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xCC),
        message: "relative-no-context".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    // Anchor=0 + delta 3 -> reads at absolute offset 3.
    let buffer = &[0xAA, 0xBB, 0xDD, 0xCC, 0xEE];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "evaluate_single_rule with Relative(3) should resolve to absolute 3"
    );
    let (offset, value) = result.unwrap();
    assert_eq!(offset, 3);
    assert_eq!(value, Value::Uint(0xCC));
}

#[test]
fn test_evaluate_rules_anchor_near_saturation_skips_relative_child_gracefully() {
    // Pin the contract that an anchor at or near `usize::MAX` does not
    // panic and instead causes subsequent Relative rules to fail bounds
    // checks gracefully. We can't construct a real match at usize::MAX
    // (no realistic buffer is that big), so inject the saturated anchor
    // directly via the pub(crate) setter and then evaluate a Relative rule.
    use crate::EvaluationConfig;
    use crate::evaluator::EvaluationContext;

    let buffer = [0xAA, 0xBB, 0xCC, 0xDD];
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX);

    // Relative(0) -> target = usize::MAX, which is >= buffer.len() and
    // returns BufferOverrun -> graceful skip in evaluate_rules.
    let rule_zero = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-zero-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let matches = evaluate_rules(&[rule_zero], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(0) at usize::MAX anchor must skip, not match or panic"
    );

    // Relative(+1) -> checked_add_signed -> overflow -> InvalidOffset -> skip.
    ctx.set_last_match_end(usize::MAX);
    let rule_pos = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-plus-one-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let matches = evaluate_rules(&[rule_pos], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(+1) at usize::MAX anchor must skip via InvalidOffset, not panic"
    );

    // Relative(-N) where N is small -> usize::MAX - N, still >= buffer.len() -> skip.
    ctx.set_last_match_end(usize::MAX);
    let rule_neg = MagicRule {
        offset: OffsetSpec::Relative(-1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-minus-one-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let matches = evaluate_rules(&[rule_neg], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(-1) at usize::MAX anchor must skip, not panic"
    );
}

#[test]
fn test_evaluate_single_rule_relative_negative_with_zero_anchor_errors() {
    // Public evaluate_single_rule uses an implicit anchor of 0. A negative
    // Relative delta underflows the anchor and must return
    // EvaluationError::InvalidOffset -- NOT Ok(None) (the "no match" path)
    // and NOT Absolute(-N)-style from-end semantics. Pin the contract so a
    // future refactor can't silently convert this to a graceful skip.
    use crate::LibmagicError;
    use crate::error::EvaluationError;

    let rule = MagicRule {
        offset: OffsetSpec::Relative(-1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-neg-top-level".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let buffer = &[0xAA, 0xBB];
    let err = evaluate_single_rule_legacy(&rule, buffer).unwrap_err();
    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset { offset: -1 })
        ),
        "Relative(-1) at anchor 0 must Err(InvalidOffset), got {err:?}"
    );
}

#[test]
fn test_evaluate_single_rule_relative_zero_resolves_to_buffer_start() {
    // Relative(0) with anchor=0 resolves to absolute 0.
    let rule = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "relative-zero".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = &[0xAA, 0xBB];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap().unwrap();
    assert_eq!(result.0, 0);
}
