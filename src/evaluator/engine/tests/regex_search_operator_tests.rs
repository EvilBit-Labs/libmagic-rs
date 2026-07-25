// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for `regex`/`search` pattern-bearing type matching,
//! including metacharacter matches, `NotEqual` search semantics,
//! and the S2.4 rejection of non-equality operators on
//! pattern-bearing types.

use super::*;

/// A regex rule whose pattern contains metacharacters must succeed when the
/// pattern actually matches the buffer. Prior to this fix, the engine compared
/// the matched text (e.g., "123") against the pattern literal ("[0-9]+") via
/// `apply_operator`, which failed for any real regex.
#[test]
fn test_regex_rule_with_metacharacters_matches() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("[0-9]+".to_string()),
        message: "has digits".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"abc123def", &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "has digits");
}

/// A regex rule whose pattern does not match must not match, confirming that
/// the logical-match shortcut only fires on a non-empty reader result.
#[test]
fn test_regex_rule_with_metacharacters_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("[0-9]+".to_string()),
        message: "has digits".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"abcdef", &mut context).unwrap();
    assert!(matches.is_empty());
}

/// A search rule with `Operator::NotEqual` succeeds only when the literal
/// pattern is absent from the window.
#[test]
fn test_search_rule_not_equal_succeeds_when_pattern_absent() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(64),
            flags: SearchFlags::default(),
        },
        op: Operator::NotEqual,
        value: Value::String("needle".to_string()),
        message: "no needle".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"plain haystack", &mut context).unwrap();
    assert_eq!(matches.len(), 1);
}

/// A non-Equal/NotEqual operator on a pattern-bearing type must surface as
/// a hard error, not silently produce an ordering comparison against the
/// pattern source text. Pre-fix, `regex > "[0-9]+"` matched by coincidence
/// whenever the empty "no match" sentinel happened to lexicographically
/// exceed the pattern literal.
#[test]
fn test_regex_rule_with_ordering_operator_is_rejected() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::GreaterThan,
        value: Value::String("[0-9]+".to_string()),
        message: "bogus".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_single_rule(&rule, b"abcdef", &mut context);
    match result {
        Err(LibmagicError::EvaluationError(_)) => {}
        other => panic!("expected EvaluationError for ordering operator on regex, got {other:?}"),
    }
}

#[test]
fn test_search_rule_with_bitwise_operator_is_rejected() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(32),
            flags: SearchFlags::default(),
        },
        op: Operator::BitwiseAnd,
        value: Value::String("needle".to_string()),
        message: "bogus".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_single_rule(&rule, b"plain haystack", &mut context);
    assert!(
        matches!(result, Err(LibmagicError::EvaluationError(_))),
        "expected EvaluationError for bitwise operator on search"
    );
}

/// A child rule with `OffsetSpec::Relative(0)` after a parent regex match
/// must resolve to `parent_absolute_offset + match_length`, so the byte the
/// child reads is the first byte *after* the parent's match. This is the
/// regression test GOTCHAS 2.1 warns about: if `bytes_consumed_with_pattern`
/// returns the wrong number for `TypeKind::Regex`, the child lands at the
/// wrong offset and either misses or matches the wrong byte.
#[test]
fn test_regex_parent_advances_anchor_for_relative_child() {
    // Buffer: "abc123X" -- parent regex "abc" matches bytes 0..3, so a
    // Relative(0) child should read byte 3 = '1' (0x31). A Relative(-1)
    // child would read byte 2 = 'c' (0x63).
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'1')),
        message: "first digit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("abc".to_string()),
        message: "abc prefix".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc123X", &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "expected parent + child match, got {}: {matches:?}",
        matches.len()
    );
    assert_eq!(matches[0].message, "abc prefix");
    assert_eq!(matches[1].message, "first digit");
}
