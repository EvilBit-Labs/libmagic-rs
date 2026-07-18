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
fn test_flagged_string_ordering_operator_uses_lexicographic_value_path() {
    // An ORDERING operator on a flagged string is a lexicographic comparison,
    // not a pattern match, so it routes to the value path (like an unflagged
    // `string >VALUE`) instead of the equality-only pattern-bearing path.
    // This is the ubiquitous `string/t >\0` / `string/b >\0` "there is
    // non-empty text here, print it with %s" idiom (varied.script, sgml,
    // linux, ...). Routing it to the pattern path made it a fatal
    // `UnsupportedType` that aborted the ENTIRE file's evaluation -- e.g.
    // `rmagic` used to error out on a Bourne-Again shell script. The `/t`/`/b`
    // flags are MIME-output hints with no ordering effect. (Case-fold flags
    // like `/c` are also not applied to ordering, but such rules do not occur
    // in real magic files; the value path's byte-lexicographic compare is the
    // correct behavior for the flags that actually pair with `<`/`>`.)
    let rule = make_flagged_string_rule(
        "\0",
        StringFlags::default().with_text_test(true),
        Operator::GreaterThan,
    );
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"FOObar", &mut context)
        .expect("ordering operator on a flagged string must not fatally abort");
    assert_eq!(
        matches.len(),
        1,
        "a non-empty string is lexicographically greater than the null byte"
    );

    // Negative control: an empty buffer has no string > \\0, so no match --
    // and still no abort.
    let rule2 = make_flagged_string_rule(
        "\0",
        StringFlags::default().with_text_test(true),
        Operator::GreaterThan,
    );
    let mut ctx2 = EvaluationContext::new(EvaluationConfig::default());
    let empty = evaluate_rules(&[rule2], b"", &mut ctx2)
        .expect("ordering operator must not abort even on an empty buffer");
    assert!(
        empty.is_empty(),
        "no string is present in an empty buffer, so `>\\0` must not match"
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

#[test]
fn test_flagged_string_any_value_operator_does_not_panic_end_to_end() {
    // Regression: `0 string/b x` -- a flagged string with the AnyValue (`x`)
    // operator, a common message-less gating rule (e.g. compress's zlib
    // detector `0 string/b x` -> `>0 beshort%31 =0` ...) -- routes to the value
    // path (not the equality-only pattern path). Its rule value is the AnyValue
    // placeholder `Uint(0)`, not a string. `bytes_consumed_with_pattern` used to
    // dispatch ALL flagged strings to the pattern walker regardless of operator
    // and hit a `debug_assert!(false)` on the missing string pattern, panicking
    // in debug builds (a no-panic-policy violation). It must instead treat the
    // read like a normal string and evaluate without panicking.
    use crate::parser::grammar::parse_magic_rule;

    let (_, rule) = parse_magic_rule("0\tstring/b\tx\tgating").expect("rule must parse");
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"\x0b\x30 arbitrary binary bytes", &mut context)
        .expect("a flagged string with the AnyValue operator must evaluate without panicking");
    assert!(
        matches.iter().any(|m| m.message.contains("gating")),
        "AnyValue (`x`) matches any buffer, so the gating rule fires"
    );
}
