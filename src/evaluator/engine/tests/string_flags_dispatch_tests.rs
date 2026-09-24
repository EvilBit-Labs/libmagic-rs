// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Engine-dispatch tests for flagged `string` rules (issue #234).
//!
//! These tests pin the engine boundary: a `TypeKind::String` rule whose
//! `flags` field is non-default must route through `evaluate_pattern_rule`
//! (same path as regex/search), not the default value-rule fast path.
//! The visible effect is that `/c` makes case-insensitive matches succeed
//! where the byte-exact path would fail, and a relative-offset child of a
//! `/w`/`/W`-flagged parent lands at the pattern's own declared length
//! (GOTCHAS S6.8), not at however many file bytes the flag walked.
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
fn test_flagged_string_w_whitespace_anchor_uses_declared_pattern_length() {
    // R6, measured against real `file`-5.41 (`moffset()` in softmagic.c
    // advances by `m->vallen`, the pattern's own declared length,
    // unconditional on flags): when /W matches "a b" against "a    b",
    // the anchor must advance by 3 (pattern length), NOT 6 (file bytes
    // walked). A relative-offset child therefore lands on one of the
    // whitespace bytes /W consumed (index 3), not on the '!' that
    // follows the walked region (index 6).
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b' ')),
        message: "space-at-declared-length".to_string(),
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
    // File: "a    b!" -> parent's declared pattern length is 3, so the
    // anchor lands at offset 3 (a whitespace byte /W walked over), NOT
    // offset 6 (the byte after the walk, '!').
    let matches = evaluate_rules(&[parent], b"a    b!", &mut context).unwrap();
    assert_eq!(matches.len(), 2, "parent + child must both match");
    assert_eq!(matches[1].message, "space-at-declared-length");
}

#[test]
fn test_flagged_string_w_optional_whitespace_anchor_matches_measured_oracle_fixtures() {
    // R6, measured against real `file`-5.41. Each fixture pairs a `/w`
    // (whitespace-optional) pattern with a buffer whose file-side
    // whitespace walk differs from the pattern's declared length; the
    // relative-offset child must land at `offset + pattern.len()`,
    // never at the walked position.
    struct Fixture {
        pattern: &'static str,
        buffer: &'static [u8],
        expected_byte: u8,
    }
    let fixtures = [
        Fixture {
            pattern: "#! ",
            buffer: b"#!/bin/xx",
            expected_byte: b'b',
        },
        Fixture {
            pattern: "#! ",
            buffer: b"#! /bin/xx",
            expected_byte: b'/',
        },
        Fixture {
            pattern: "#! ",
            buffer: b"#!   /bin/xx",
            expected_byte: b' ',
        },
        Fixture {
            pattern: "#! X",
            buffer: b"#!Xbin/xx",
            expected_byte: b'i',
        },
        Fixture {
            pattern: "#! X",
            buffer: b"#!  Xbin/xx",
            expected_byte: b'X',
        },
    ];

    for fx in fixtures {
        let child = MagicRule {
            offset: OffsetSpec::Relative(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(u64::from(fx.expected_byte)),
            message: "anchor-byte".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
            value_transform: None,
        };
        let parent = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::String {
                max_length: None,
                flags: StringFlags::default().with_compact_optional_whitespace(true),
            },
            op: Operator::Equal,
            value: Value::String(fx.pattern.to_string()),
            message: "shebang".to_string(),
            children: vec![child],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        };
        let mut context =
            EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
        let matches = evaluate_rules(&[parent], fx.buffer, &mut context).unwrap_or_else(|e| {
            panic!(
                "fixture pattern {:?} buffer {:?} failed to evaluate: {e}",
                fx.pattern, fx.buffer
            )
        });
        assert_eq!(
            matches.len(),
            2,
            "fixture pattern {:?} buffer {:?}: parent + child must both match at the \
             declared-length anchor (byte {:?})",
            fx.pattern,
            fx.buffer,
            fx.expected_byte as char,
        );
    }
}

#[test]
fn test_flagged_string_w_near_end_of_buffer_anchor_does_not_panic_and_child_is_non_match() {
    // R6/R8: the declared-length anchor can land past the end of a short
    // buffer when /w consumed zero optional whitespace bytes. The
    // relative child's own read then hits a buffer overrun and simply
    // fails to match -- no panic, and the parent's own match still
    // stands.
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'X')),
        message: "unreachable".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default().with_compact_optional_whitespace(true),
        },
        op: Operator::Equal,
        value: Value::String("#! X".to_string()),
        message: "shebang".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    // Buffer ends exactly where the /w walk ends ("#!X", 3 bytes), one
    // byte short of the pattern's declared length (4).
    let matches = evaluate_rules(&[parent], b"#!X", &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "parent matches but the child's declared-length anchor (4) is past EOF (3), \
         so it must not match -- and evaluation must not panic"
    );
}

#[test]
fn test_string_ordering_renders_full_field_not_compared_prefix() {
    // libmagic compares a `string >VALUE` rule prefix-limited to
    // `pattern.len()` (`file_strncmp` with `vallen`) but RENDERS the full
    // string field (`p->s`). The engine must therefore return the full field
    // as the match's DISPLAY value, not just the compared prefix -- otherwise
    // sgml's `>15 string/t >\0 %.3s ...` renders the XML version `1` instead
    // of `1.0`. Here the pattern is 5 bytes ("0.6.1") but the display value
    // must be the whole field.
    let rule = make_flagged_string_rule("0.6.1", StringFlags::default(), Operator::GreaterThan);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"0.6.2 and more text", &mut context).unwrap();
    assert_eq!(matches.len(), 1, "0.6.2 > 0.6.1 must match");
    assert_eq!(
        matches[0].value,
        Value::String("0.6.2 and more text".to_string()),
        "display value must be the FULL string field, not the compared 5-byte prefix"
    );
}

#[test]
fn test_string_ordering_comparison_stays_prefix_limited() {
    // Guard against a future revert to full-field COMPARISON. libmagic
    // compares only the first `pattern.len()` bytes: buffer "0.6.10" has
    // prefix "0.6.1" == pattern, so `> 0.6.1` is FALSE even though the full
    // string "0.6.10" sorts after "0.6.1". Verified against the real `file`
    // binary (file-5.41): `0.6.10` does NOT match `>0.6.1`, but `0.6.2` does.
    let rule = make_flagged_string_rule("0.6.1", StringFlags::default(), Operator::GreaterThan);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let no_match = evaluate_rules(&[rule], b"0.6.10more", &mut context).unwrap();
    assert!(
        no_match.is_empty(),
        "prefix '0.6.1' equals the pattern, so `> 0.6.1` must be false (prefix-limited compare)"
    );

    // Positive control: a genuinely-greater prefix matches.
    let rule2 = make_flagged_string_rule("0.6.1", StringFlags::default(), Operator::GreaterThan);
    let mut ctx2 = EvaluationContext::new(EvaluationConfig::default());
    let m = evaluate_rules(&[rule2], b"0.6.2", &mut ctx2).unwrap();
    assert_eq!(m.len(), 1, "0.6.2 > 0.6.1 within the compared prefix");
}

#[test]
fn test_string_ordering_full_field_display_does_not_move_relative_anchor() {
    // The full-field DISPLAY read must not disturb the relative-offset
    // anchor: `bytes_consumed_with_pattern` re-derives the advance from the
    // PATTERN (5 bytes for "0.6.1"), never from the display value. A child at
    // Relative(0) must resolve to offset 5 (just past the compared prefix),
    // NOT to the full-field length. Pins the advisor's anchor-stability
    // concern for the decoupled display read.
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'X')),
        message: "at-offset-5".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        op: Operator::GreaterThan,
        value: Value::String("0.6.1".to_string()),
        message: "ver".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    // buffer: bytes 0..4 = "0.6.2" (matches >0.6.1), byte 5 = 'X' (child
    // target), then a long tail that becomes the full-field display value.
    let matches =
        evaluate_rules(&[parent], b"0.6.2X and a long trailing field", &mut context).unwrap();
    assert_eq!(matches.len(), 2, "parent + child must both match");
    assert_eq!(matches[1].message, "at-offset-5");
    assert_eq!(
        matches[1].offset, 5,
        "child must resolve just past the 5-byte pattern, not the full field"
    );
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
