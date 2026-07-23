// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Basic [`evaluate_rules`] list-semantics tests (empty list, single
//! match/non-match, stop-at-first-match, find-all) plus the
//! message-less-match-does-not-shadow-a-later-match regression
//! coverage (GOTCHAS S13.2).

use super::*;

#[test]
fn test_evaluate_rules_empty_list() {
    let rules = vec![];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_single_matching_rule() {
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

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF magic");
    assert_eq!(matches[0].offset, 0);
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[0].value, Value::Int(0x7f));
}

#[test]
fn test_evaluate_rules_single_non_matching_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50),
        message: "ZIP magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_multiple_rules_stop_at_first() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rule_list = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_list, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "First match");
}

#[test]
fn test_evaluate_rules_multiple_rules_find_all() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let rule_set = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_set, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "First match");
    assert_eq!(matches[1].message, "Second match");
}

/// GOTCHAS S13.2 (refined): a message-less top-level match must not
/// shadow a later, message-bearing sibling under `stop_at_first_match:
/// true`. This is the exact shape of the assembler-source-text /
/// plain-ASCII-text blank-output bug -- a gating rule with no message
/// matches first in strength order and used to terminate evaluation
/// before the real classification rule was ever tried.
#[test]
fn test_evaluate_rules_message_less_match_does_not_stop_at_first_match() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let gating_rule = message_only_byte_rule(0, 0xAA, "");
    let real_rule = message_only_byte_rule(1, 0xBB, "Second match");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[gating_rule, real_rule], buffer, &mut context).unwrap();

    // Both rules matched: the message-less gating rule did not terminate
    // the search, so the message-bearing rule behind it was reached and
    // its match is present.
    assert_eq!(matches.len(), 2, "both rules should have matched");
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "Second match");
}

/// Reverse of the above: when the message-BEARING rule comes first, the
/// original `stop_at_first_match` short-circuit still applies -- this
/// fix only relaxes the stop condition for message-less matches, it does
/// not disable early-exit for the common (and performance-sensitive)
/// case where the very first top-level rule already produces output.
#[test]
fn test_evaluate_rules_message_bearing_match_still_stops_at_first_match() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let real_rule = message_only_byte_rule(0, 0xAA, "First match");
    let gating_rule = message_only_byte_rule(1, 0xBB, "");
    let never_reached = message_only_byte_rule(2, 0xCC, "Should not be reached");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[real_rule, gating_rule, never_reached],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(
        matches.len(),
        1,
        "evaluation must still stop right after the first message-bearing match"
    );
    assert_eq!(matches[0].message, "First match");
}

/// Several message-less matches in a row must all be skipped over (not
/// discarded -- just not treated as terminating) until a message-bearing
/// rule is reached, at which point the usual stop-at-first-match
/// short-circuit applies again.
#[test]
fn test_evaluate_rules_multiple_message_less_matches_before_a_real_one() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let gating_one = message_only_byte_rule(0, 0xAA, "");
    let gating_two = message_only_byte_rule(1, 0xBB, "");
    let real_rule = message_only_byte_rule(2, 0xCC, "Real message");
    let never_reached = message_only_byte_rule(3, 0xDD, "Should not be reached");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[gating_one, gating_two, real_rule, never_reached],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "");
    assert_eq!(matches[2].message, "Real message");
}

/// Genuinely-no-usable-output case: every top-level rule matches but
/// none of them carries a message. Under `stop_at_first_match: true`
/// evaluation must run to exhaustion (there is nothing to stop at) --
/// all matches are collected, and it is the caller's (here:
/// `MagicDatabase::build_result`'s) job to fall back to text/data
/// classification when the resulting description is empty.
#[test]
fn test_evaluate_rules_all_message_less_matches_runs_to_exhaustion() {
    let buffer = &[0xAA, 0xBB];
    let gating_one = message_only_byte_rule(0, 0xAA, "");
    let gating_two = message_only_byte_rule(1, 0xBB, "");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[gating_one, gating_two], buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2, "both message-less matches are retained");
    assert!(matches.iter().all(|m| m.message.is_empty()));
}

/// A message consisting solely of whitespace, or solely of the GNU
/// `file` backspace continuation marker (`\b`), is just as
/// "message-less" as an empty string for `stop_at_first_match` purposes
/// -- see `is_message_bearing`'s doc comment for the rationale.
#[test]
fn test_evaluate_rules_whitespace_and_backspace_only_messages_do_not_stop() {
    let buffer = &[0xAA, 0xBB, 0xCC];
    let whitespace_only = message_only_byte_rule(0, 0xAA, "   ");
    let backspace_only = message_only_byte_rule(1, 0xBB, "\u{8}");
    let real_rule = message_only_byte_rule(2, 0xCC, "Real message");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[whitespace_only, backspace_only, real_rule],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[2].message, "Real message");
}

/// Regression for the `is_message_bearing` literal-`\b`-marker gap (PR #376
/// review finding). The GNU `file` no-separator marker most often reaches the
/// evaluator as the LITERAL two-character sequence `\b` (backslash + `'b'`),
/// not the raw `U+0008` byte, because the message parser preserves description
/// text verbatim (GOTCHAS S14.1). A rule whose message is exactly the literal
/// marker renders to empty in `concatenate_messages`, so it must be classified
/// message-less here too -- otherwise it could win the `stop_at_first_match`
/// race and shadow a later, more specific rule (the S13.2 blank-output bug
/// class). The prior implementation only trimmed `U+0008` and would have
/// treated `"\\b"` as message-bearing.
#[test]
fn test_literal_backspace_marker_message_is_message_less_and_does_not_stop() {
    // Direct predicate: both marker forms (and whitespace-padded variants) are
    // message-less; a marker WITH trailing content is message-bearing.
    assert!(
        !is_message_bearing("\\b"),
        "the literal `\\b` marker alone must be message-less"
    );
    assert!(
        !is_message_bearing("\u{8}"),
        "the raw U+0008 marker alone must be message-less"
    );
    assert!(
        !is_message_bearing("  \\b  "),
        "a whitespace-padded literal marker must be message-less"
    );
    assert!(
        is_message_bearing("\\bversion"),
        "a literal marker WITH content must be message-bearing"
    );
    assert!(
        is_message_bearing("plain"),
        "plain text must be message-bearing"
    );

    // End-to-end: a literal-`\b`-only gating rule must NOT stop evaluation
    // before a later real rule under stop_at_first_match.
    let buffer = &[0xAA, 0xBB];
    let literal_marker_only = message_only_byte_rule(0, 0xAA, "\\b");
    let real_rule = message_only_byte_rule(1, 0xBB, "Real message");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[literal_marker_only, real_rule], buffer, &mut context).unwrap();

    assert_eq!(
        matches.len(),
        2,
        "the literal-marker-only rule must not stop evaluation before the real rule"
    );
    assert_eq!(matches[1].message, "Real message");
}

/// A message-less top-level rule whose CHILD produces real output text
/// still counts as "producing output" for `stop_at_first_match`
/// purposes -- this is the normal, common shape for gating rules like
/// `c-lang`'s `0 search/8192 "#include"` -> `>0 regex \^#include c`
/// chain, and must keep stopping at the first sibling that (directly or
/// via a descendant) yields a description; only a rule with NO
/// message-bearing output anywhere in its subtree should be skipped
/// past.
#[test]
fn test_evaluate_rules_message_less_parent_with_message_bearing_child_still_stops() {
    let child_rule = message_only_byte_rule(1, 0xBB, "child message");
    let mut parent_rule = message_only_byte_rule(0, 0xAA, "");
    parent_rule.children = vec![child_rule];

    let never_reached = message_only_byte_rule(2, 0xCC, "Should not be reached");

    let buffer = &[0xAA, 0xBB, 0xCC];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[parent_rule, never_reached], buffer, &mut context).unwrap();

    assert_eq!(
        matches.len(),
        2,
        "parent (message-less) + child (message-bearing) should be present, \
         and evaluation should stop there"
    );
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "child message");
}

#[test]
fn test_stop_at_first_match_does_not_truncate_child_siblings() {
    // Regression guard: `stop_at_first_match` is a TOP-LEVEL classification
    // concept. Once a parent matches, ALL of its matching child siblings must
    // render even under `stop_at_first_match: true` -- the break must not fire
    // inside a child sibling list. (An earlier revision applied the break at
    // every recursion level, so the first message-bearing child silently
    // truncated the rest -- dropping gzip's "max compression", "from Unix",
    // "original size ..." fragments after the first match. See the
    // `EvaluationConfig::stop_at_first_match` top-level-only contract.)
    let mut parent = message_only_byte_rule(0, 0xAA, "parent");
    parent.children = vec![
        message_only_byte_rule(1, 0xBB, "child-1"),
        message_only_byte_rule(2, 0xCC, "child-2"),
        message_only_byte_rule(3, 0xDD, "child-3"),
    ];

    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[parent], buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["parent", "child-1", "child-2", "child-3"],
        "all matching child siblings must render under stop_at_first_match; \
         the break must not fire inside a child sibling list"
    );
}
