// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Engine-dispatch tests for flagged `string` rules (issue #234).
//!
//! These tests pin the engine boundary: a `TypeKind::String` rule whose
//! `flags` field is non-default must route through `evaluate_pattern_rule`
//! (same path as regex/search), not the default value-rule fast path.
//! The visible effect is that `/c` makes case-insensitive matches succeed
//! where the byte-exact path would fail, and `/W` advances the
//! relative-offset anchor by the file's actual whitespace consumption.
//!
//! The lower-level algorithm tests for `compare_string_with_flags` live in
//! `src/evaluator/types/string.rs::tests`; the integration / conformance
//! suite lives in `tests/string_flags_integration.rs`. This file covers
//! only the engine-routing layer.
//!
//! Split out from the parent `tests/mod.rs` per the `CodeRabbit` PR #288
//! review (thread `PRRT_kwDOP5Naes6E-aov`) to keep the parent file
//! closer to the project's file-size guideline.

use super::*;
use crate::parser::ast::StringFlags;

fn make_flagged_string_rule(pattern: &str, flags: StringFlags, op: Operator) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags,
        },
        op,
        value: Value::String(pattern.to_string()),
        message: format!("matched {pattern}"),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

#[test]
fn test_flagged_string_c_case_insensitive_matches_via_engine() {
    let rule = make_flagged_string_rule(
        "foo",
        StringFlags::default().with_ignore_lowercase(true),
        Operator::Equal,
    );
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"FOObar", &mut context).unwrap();
    assert_eq!(matches.len(), 1, "string/c foo should match FOObar");
    assert_eq!(matches[0].message, "matched foo");
}

#[test]
fn test_flagged_string_c_does_not_match_when_uppercase_pattern_position_differs() {
    // GOTCHAS S6.5 asymmetry: pattern `FoO` with /c matches `FoO`, `Foo`,
    // `FOO` but NOT `fOO` (uppercase 'F' position is literal).
    let rule = make_flagged_string_rule(
        "FoO",
        StringFlags::default().with_ignore_lowercase(true),
        Operator::Equal,
    );
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"fOOrest", &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "string/c FoO must not match fOO (asymmetric /c contract)"
    );
}

#[test]
fn test_flagged_string_default_flags_use_value_rule_fast_path() {
    // When flags are default, the rule goes through the existing
    // byte-exact path (NOT the new pattern-bearing path). We can't
    // observe the dispatcher choice directly, but we can verify that
    // case-sensitive matching is still in effect.
    let rule = make_flagged_string_rule("foo", StringFlags::default(), Operator::Equal);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"FOObar", &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "default-flag string must use case-sensitive matching"
    );
}

#[test]
fn test_flagged_string_not_equal_inverts_match() {
    // Same shape as the regex/search NotEqual test pattern. /c on a
    // pattern that does NOT match the file should produce a NotEqual hit.
    let rule = make_flagged_string_rule(
        "xyz",
        StringFlags::default().with_ignore_lowercase(true),
        Operator::NotEqual,
    );
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"FOObar", &mut context).unwrap();
    assert_eq!(matches.len(), 1, "NotEqual fires when pattern misses");
}

#[test]
fn test_flagged_string_ordering_operator_is_rejected() {
    // Same contract as regex/search: only Equal/NotEqual are allowed on
    // pattern-bearing rules. GreaterThan must surface as EvaluationError.
    let rule = make_flagged_string_rule(
        "foo",
        StringFlags::default().with_ignore_lowercase(true),
        Operator::GreaterThan,
    );
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_single_rule(&rule, b"FOObar", &mut context);
    assert!(
        matches!(result, Err(LibmagicError::EvaluationError(_))),
        "expected EvaluationError for ordering operator on flagged string"
    );
}

#[test]
fn test_flagged_string_w_whitespace_consumes_extra_file_bytes_for_anchor() {
    // Load-bearing: when /W matches "a b" against "a    b", the anchor
    // must advance by 6 (file bytes consumed), not 3 (pattern length).
    // A relative-offset child reads from the post-match position.
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(b'!'.into()),
        message: "exclaim".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default().with_compact_whitespace(true),
        },
        op: Operator::Equal,
        value: Value::String("a b".to_string()),
        message: "match".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    // File: "a    b!" -> parent consumes 6 bytes, child reads byte at
    // offset 6 which is '!'. The contract from U3 test
    // `test_compare_string_with_flags_consumed_bytes_drives_anchor` is
    // what makes this work end-to-end.
    let matches = evaluate_rules(&[parent], b"a    b!", &mut context).unwrap();
    assert_eq!(matches.len(), 2, "parent + child must both match");
    assert_eq!(matches[1].message, "exclaim");
}
