// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for `MetaType::Offset` dispatch and the related
//! `evaluate_children_or_warn` helper.
//!
//! Shared helpers (`offset_rule`, `byte_eq_rule`) live in the parent
//! `tests/mod.rs` module.

use super::*;

#[test]
fn test_offset_emits_match_with_resolved_position() {
    let rules = vec![offset_rule(5, "pos=%lld", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 10], &mut context).unwrap();
    assert_eq!(matches.len(), 1, "offset rule must emit exactly one match");
    assert_eq!(matches[0].offset, 5, "match.offset is the resolved offset");
    assert_eq!(
        matches[0].value,
        Value::Uint(5),
        "match.value carries the resolved offset for format substitution"
    );
    assert_eq!(matches[0].message, "pos=%lld");
}

#[test]
fn test_offset_at_zero() {
    // Regression guard: offset 0 must still produce a match (not be
    // indistinguishable from "no match").
    let rules = vec![offset_rule(0, "top", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, Value::Uint(0));
}

#[test]
fn test_offset_out_of_bounds_graceful_skip() {
    // Offset past the end of the buffer is a data-dependent skip, not an
    // error. Matches the Indirect dispatch's graceful-skip discipline.
    let rules = vec![offset_rule(1_000_000, "unreachable", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "offset past buffer end must produce no match"
    );
}

#[test]
fn test_offset_non_x_operator_is_skipped() {
    // magic(5) only allows `x` on an `offset` rule. Anything else is
    // semantically undefined -> debug-log + skip.
    let mut rule = offset_rule(0, "bogus", vec![]);
    rule.op = Operator::Equal;
    rule.value = Value::Uint(5);
    let rules = vec![rule];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "offset rule with non-AnyValue operator must be skipped"
    );
}

#[test]
fn test_offset_evaluates_children() {
    // A child byte rule at offset 0 runs AFTER the parent offset rule
    // fires. The child's own offset is resolved independently.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let mut parent = offset_rule(
        0,
        "parent-offset",
        vec![byte_eq_rule(0, 0x42, "child-byte")],
    );
    // Child level must be deeper than parent per MagicRule::validate.
    parent.children[0].level = 1;
    let buffer = [0x42u8, 0x00, 0x00];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(messages, vec!["parent-offset", "child-byte"]);
}

#[test]
fn test_offset_advances_anchor_for_children() {
    // An offset rule at position 5 advances `last_match_end` to 5 *for its
    // children* -- but NOT for sibling rules at the same level. This
    // matches libmagic's continuation-level semantics: each sibling at
    // level L resolves `&N` against the parent-level anchor, not against
    // the previous sibling's advance. See the `entry_anchor` discipline
    // in `evaluate_rules`.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // A child of the offset rule uses &0 to resolve at the offset rule's
    // resolved position (5). buffer[5] = 0x42.
    let mut child = byte_eq_rule(0, 0x42, "child-at-offset-anchor");
    child.offset = OffsetSpec::Relative(0);
    child.level = 1;

    let buffer = [0x00u8, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00];
    let rules = vec![offset_rule(5, "mark", vec![child])];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches
            .iter()
            .any(|m| m.message == "child-at-offset-anchor"),
        "child of offset rule must resolve against offset's anchor (5); got {matches:?}"
    );
}

#[test]
fn test_offset_does_not_advance_anchor_for_continuation_siblings() {
    // Regression guard for the libmagic continuation-sibling anchor
    // semantic: two CHILD siblings at the same level resolve `&N`
    // against the parent-level anchor, not against the previous
    // sibling's advance. This is gated on `recursion_depth > 0`;
    // top-level siblings still chain (see
    // `relative_anchor_can_decrease_...` in the relative-offset
    // integration tests).
    //
    // Parent `byte` at offset 0 matches 0x01 -> anchor = 1. Two
    // child siblings at &0 must both read buffer[1] = 0x42. If the
    // first child incorrectly advanced the anchor to 2, the second
    // would read buffer[2] = 0x00 and miss.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "parent".to_string(),
        children: vec![
            MagicRule {
                offset: OffsetSpec::Relative(0),
                typ: TypeKind::Byte { signed: false },
                op: Operator::Equal,
                value: Value::Uint(0x42),
                message: "sibling-1".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
                value_transform: None,
            },
            MagicRule {
                offset: OffsetSpec::Relative(0),
                typ: TypeKind::Byte { signed: false },
                op: Operator::Equal,
                value: Value::Uint(0x42),
                message: "sibling-2".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
                value_transform: None,
            },
        ],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = [0x01u8, 0x42, 0x00, 0x00];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["parent", "sibling-1", "sibling-2"],
        "both continuation siblings must resolve against parent anchor (1); \
         if sibling-1 advanced the anchor to 2, sibling-2 would read \
         buffer[2]=0x00 and fail"
    );
}

#[test]
fn test_offset_sets_sibling_matched() {
    // An offset rule match suppresses a following `default` sibling --
    // same discipline as any other matching rule.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        offset_rule(0, "offset-match", vec![]),
        default_rule("DEFAULT-SUPPRESSED", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["offset-match"],
        "default must be suppressed when offset sibling matched; got {matches:?}"
    );
}

// =======================================================================
// evaluate_children_or_warn graceful-error helper (issue #42 close-out)
// =======================================================================

#[test]
fn test_evaluate_children_or_warn_swallows_buffer_overrun_keeps_parent_match() {
    // Regression guard for the extracted `evaluate_children_or_warn`
    // helper: a child with an absolute offset past the buffer end must
    // produce a `BufferOverrun` that is swallowed (warn-logged) rather
    // than propagated. The parent match must still appear in the
    // results. Covers the graceful-skip arm for all four dispatch
    // sites (Default/Indirect/Offset/Use) via the Offset arm -- they
    // all delegate to the same helper.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Child rule at absolute offset 1000 reads a byte -- far past the
    // tiny buffer we supply. The helper should catch the BufferOverrun
    // and warn-log, not fail the evaluation.
    let child = MagicRule {
        offset: OffsetSpec::Absolute(1000),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "unreachable-child".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = offset_rule(0, "parent-offset-match", vec![child]);

    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &[0u8; 4], &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["parent-offset-match"],
        "parent match must survive a child's BufferOverrun; child must be silently skipped, got {matches:?}"
    );
}
