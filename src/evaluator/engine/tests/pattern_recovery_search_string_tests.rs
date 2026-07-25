// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! S2.1 pattern-operand-missing graceful-skip coverage for
//! `search` rules and flagged `string` rules, across all three
//! engine dispatch sites.

use super::*;

/// Top-level site: a pattern-less `search` rule is skipped, not fatal.
#[test]
fn test_pattern_operand_skip_at_top_level_site_search() {
    let broken = broken_pattern_search_rule("broken top-level search", 0);
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "leading a".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    let matches = evaluate_rules(&[broken, sibling], b"abc", &mut context)
        .expect("top-level pattern-less search must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// Child-recursion site: a pattern-less `search` rule under a matched
/// parent must not abort the parent's match.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site_search() {
    let broken_child = broken_pattern_search_rule("broken child search", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc", &mut context)
        .expect("a broken child search must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a pattern-less `search` rule as a
/// child of `default` must not abort the `default` match.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site_search() {
    let broken_child = broken_pattern_search_rule("broken default child search", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"anything", &mut context)
        .expect("a broken child search under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

/// Top-level site: a pattern-less flagged `string` rule is skipped, not
/// fatal.
#[test]
fn test_pattern_operand_skip_at_top_level_site_flagged_string() {
    let broken = broken_pattern_flagged_string_rule("broken top-level flagged string", 0);
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "leading a".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    let matches = evaluate_rules(&[broken, sibling], b"abc", &mut context)
        .expect("top-level pattern-less flagged string must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// Child-recursion site: a pattern-less flagged `string` rule under a
/// matched parent must not abort the parent's match.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site_flagged_string() {
    let broken_child = broken_pattern_flagged_string_rule("broken child flagged string", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc", &mut context)
        .expect("a broken child flagged string must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a pattern-less flagged `string` rule
/// as a child of `default` must not abort the `default` match.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site_flagged_string() {
    let broken_child = broken_pattern_flagged_string_rule("broken default child flagged string", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"anything", &mut context)
        .expect("a broken child flagged string under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}
