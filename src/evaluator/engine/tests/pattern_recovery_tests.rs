// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for the S2.1 pattern-operand-missing graceful-skip
//! contract across all three engine dispatch sites, for `regex`
//! rules, plus the S2.4 fatal (non-allowlisted) unsupported-type
//! propagation and pathological regex-compile-failure skip.

use super::*;

/// FLOOR ANCHOR (U2 execution note): this is the regression test that must
/// be written and observed to FAIL before the engine fix is wired in. Prior
/// to the fix, `evaluate_rules` returns `Err(LibmagicError::EvaluationError(
/// EvaluationError::TypeReadError(TypeReadError::UnsupportedType { .. })))`
/// for a top-level pattern-less regex rule -- aborting analysis of every
/// target when the system magic DB contains such a rule. The floor
/// requirement (R1/R2) is that `evaluate_rules` must return `Ok` and treat
/// the broken rule as a non-match.
#[test]
fn test_evaluate_rules_skips_pattern_less_regex_rule_gracefully() {
    let rule = broken_pattern_regex_rule("broken top-level regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_rules(&[rule], b"anything to scan", &mut context);
    let matches =
        result.expect("evaluate_rules must not fatally abort on a pattern-less regex rule");
    assert!(
        matches.is_empty(),
        "the broken rule must contribute no match, got {matches:?}"
    );
}

/// All-three-sites parity (a): a broken pattern-less regex at the TOP LEVEL
/// alongside a normal sibling rule -- the sibling must still match and the
/// broken rule must be silently skipped.
#[test]
fn test_pattern_operand_skip_at_top_level_site() {
    let broken = broken_pattern_regex_rule("broken top-level regex", 0);
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
        .expect("top-level pattern-less regex must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// All-three-sites parity (b): a broken pattern-less regex as a CHILD under
/// a matched parent (the inline child-recursion catch arm, ~L1108-1118).
/// The parent match must still be emitted even though its child is broken.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site() {
    let broken_child = broken_pattern_regex_rule("broken child regex", 1);
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
        .expect("a broken child regex must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// All-three-sites parity (c): a broken pattern-less regex as a child of a
/// `default` rule -- exercises the `evaluate_children_or_warn` path
/// (~L522-530). The `default` match must still be emitted.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site() {
    let broken_child = broken_pattern_regex_rule("broken default child regex", 1);
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
        .expect("a broken child regex under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

/// NEGATIVE (pins R3 narrowness): an `UnsupportedType` condition that is
/// NOT the missing-pattern-operand class -- here, a non-Equal/NotEqual
/// operator on a pattern-bearing type -- must still propagate fatally.
/// This proves U2's skip did not widen into swallowing the whole
/// `UnsupportedType` variant.
#[test]
fn test_evaluate_rules_propagates_non_pattern_missing_unsupported_type() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::GreaterThan,
        value: Value::String("[0-9]+".to_string()),
        message: "bogus ordering".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_rules(&[rule], b"abc123", &mut context);
    assert!(
        matches!(result, Err(LibmagicError::EvaluationError(_))),
        "a non-pattern-missing UnsupportedType must still propagate, got {result:?}"
    );
}

/// NEGATIVE: a regex whose pattern fails to compile under the
/// `REGEX_COMPILE_SIZE_LIMIT` (1 MiB) CWE-1333 denial-of-service guard is skipped (not
/// fatal) per KTD5, but this is a distinct, louder-logged condition than
/// the ordinary missing-pattern skip. There is no log-capturing test seam
/// in this crate (no `test-log`/`tracing-test` dev-dependency), so this
/// test asserts only the behavioral half of the contract -- the rule is
/// skipped, not fatal -- and the `warn!` vs `debug!` split is verified by
/// code inspection (`log_pattern_operand_skip` in `engine/mod.rs`).
#[test]
fn test_pathological_regex_compile_failure_is_skipped_not_fatal() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("a{1000000}".to_string()),
        message: "pathological regex".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection must be skipped, not fatal");
    assert!(
        matches.is_empty(),
        "the pathological regex rule must contribute no match, got {matches:?}"
    );
}

/// IO/offset arms untouched (KTD4 regression guard): a `BufferOverrun`
/// condition (via an anchor pinned to `usize::MAX`) must still be skipped
/// exactly as before, proving U2 added a new arm rather than replacing the
/// pre-existing IO/offset catch set.
#[test]
fn test_buffer_overrun_still_skipped_after_pattern_operand_guard_added() {
    let buffer = [0xAA, 0xBB, 0xCC, 0xDD];
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX);

    let rule = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-zero-near-sat-regression".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let matches = evaluate_rules(&[rule], &buffer, &mut ctx)
        .expect("BufferOverrun must still be skipped gracefully, not propagated");
    assert!(
        matches.is_empty(),
        "Relative(0) at usize::MAX anchor must skip, not match or panic"
    );
}
