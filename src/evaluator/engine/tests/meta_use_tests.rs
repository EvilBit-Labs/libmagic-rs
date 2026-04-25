// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for `MetaType::Use` dispatch and the subroutine `base_offset`
//! biasing that `use`-site evaluation depends on.
//!
//! Helpers (`use_rule`, `use_rule_at`, `build_name_table`, `byte_eq_rule`,
//! `make_context_with_env`) live in the parent `tests/mod.rs` module so
//! the companion `meta_default_clear_indirect_tests` and
//! `meta_offset_tests` submodules can share them.

use super::*;

#[test]
fn test_use_known_name_evaluates_subroutine() {
    // The subroutine `part2` reads byte 3 and expects 0x42.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "sub-match".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    }];
    let table = build_name_table(vec![("part2", subroutine)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0x00u8, 0x00, 0x00, 0x42, 0x00];
    let rules = vec![use_rule("part2")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "subroutine should produce exactly one match"
    );
    assert_eq!(matches[0].message, "sub-match");
}

#[test]
fn test_use_unknown_name_returns_no_match() {
    // Empty name table so the lookup fails; the evaluator should not panic
    // and should produce zero matches.
    let table = NameTable::empty();
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0x00u8, 0x42];
    let rules = vec![use_rule("nonexistent")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(matches.is_empty(), "unknown name should yield no matches");
}

#[test]
fn test_use_without_rule_env_returns_no_match() {
    // A default context has no rule_env attached; `use` rules should be
    // silent no-ops in that case rather than returning an error or panicking.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let buffer = [0x00u8, 0x42];
    let rules = vec![use_rule("part2")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "Use with no rule_env should produce no matches"
    );
}

#[test]
fn test_use_recursion_limit() {
    // Build a mutually-recursive pair: subroutine A calls B, B calls A.
    // With the default recursion limit, this should surface as
    // `RecursionLimitExceeded` rather than a stack overflow.
    let a_body = vec![use_rule("b")];
    let b_body = vec![use_rule("a")];
    let table = build_name_table(vec![("a", a_body), ("b", b_body)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0u8; 8];
    let rules = vec![use_rule("a")];
    let result = evaluate_rules(&rules, &buffer, &mut context);
    assert!(
        matches!(
            result,
            Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            ))
        ),
        "mutual recursion through use must surface RecursionLimitExceeded, got {result:?}"
    );
}

#[test]
fn test_use_child_rules_evaluated_after_subroutine() {
    // `Use` itself does not expose a visible RuleMatch today, so we cover
    // the "subroutine matches come first" invariant by verifying that the
    // subroutine's match appears in the output and is followed by a
    // sibling rule's match in the surrounding scope.
    //
    // `EvaluationConfig::default()` sets `stop_at_first_match = true`, which
    // (correctly, after the Comment 2 fix) short-circuits sibling iteration
    // once the `use` path produces a match. To exercise the ordering
    // invariant between the subroutine and its sibling we opt into the
    // "completeness" semantics by disabling first-match short-circuit.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "sibling".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let rules = vec![use_rule("sub"), sibling];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "sub-head");
    assert_eq!(matches[1].message, "sibling");
}

#[test]
fn test_use_stop_at_first_match_short_circuits_siblings() {
    // Comment 2 regression guard: with the default
    // `stop_at_first_match = true` config, a successful `use` subroutine
    // must prevent later sibling top-level rules from being evaluated,
    // matching the short-circuit semantics every other rule kind obeys.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "sibling".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let rules = vec![use_rule("sub"), sibling];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "stop-at-first-match must halt sibling iteration once the use path produces a match"
    );
    assert_eq!(matches[0].message, "sub-head");
}

#[test]
fn test_use_rule_children_are_evaluated() {
    // Comment 1 regression guard: a `use` rule with its own children must
    // descend into those children after the subroutine runs, so that
    // libmagic chains like `>>0 use part2` followed by continuation rules
    // continue producing matches in document order.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    // Disable stop-at-first-match so both the subroutine and the child
    // rule are visible in the match vector.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let child = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "use-child".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let mut use_with_child = use_rule("sub");
    use_with_child.children = vec![child];

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let matches = evaluate_rules(&[use_with_child], &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "use rule's own children must run after the subroutine"
    );
    assert_eq!(matches[0].message, "sub-head");
    assert_eq!(matches[1].message, "use-child");
}

#[test]
fn test_name_rule_leaked_is_noop() {
    // Programmatic consumers may construct a Name rule directly and pass
    // it to the evaluator (e.g. property tests). The evaluator must not
    // panic; it should instead treat the rule as a silent no-op.
    let leaked = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Name("orphan".to_string())),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: String::new(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[leaked], &[0u8; 4], &mut context).unwrap();
    assert!(matches.is_empty(), "leaked Name rule should be a no-op");
}

// =======================================================================
// Subroutine base_offset biasing (issue #42 -- use-site offset
// propagation). Critical coverage per post-PR code review.
// =======================================================================

#[test]
fn test_use_subroutine_absolute_offset_biased_by_use_site() {
    // Regression guard: if `SubroutineScope::enter` fails to seed
    // `base_offset` with the use-site offset, a subroutine rule at
    // `Absolute(0)` will read from buffer[0] instead of
    // buffer[use_site]. This test proves the bias is active by
    // placing distinct magic bytes at two different positions and
    // verifying that the subroutine reads the use-site one.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Subroutine body: a single rule reading at Absolute(0). Without
    // base_offset biasing this resolves to file position 0. With
    // biasing it resolves to the use-site (position 8 in this test).
    let subroutine_body = vec![byte_eq_rule(0, 0x42, "sub-match-at-base")];
    let name_table = build_name_table(vec![("sub", subroutine_body)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(name_table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });

    // Use-site at offset 8. buffer[0] = 0x00 (would fail with bias
    // missing); buffer[8] = 0x42 (required for bias-active success).
    let mut buffer = vec![0u8; 16];
    buffer[8] = 0x42;

    let mut context = EvaluationContext::new(config).with_rule_env(env);
    let rules = vec![use_rule_at("sub", 8)];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.iter().any(|m| m.message == "sub-match-at-base"),
        "subroutine rule at Absolute(0) must be biased by use-site offset 8 \
         -- reading buffer[8] = 0x42. If bias missing, reads buffer[0] = 0x00 \
         and the test fails. got {matches:?}"
    );
}

#[test]
fn test_use_subroutine_relative_offset_unaffected_by_use_site() {
    // Companion to the bias test above: `Relative(N)` is resolved
    // against `last_match_end`, which `SubroutineScope` also seeds
    // to the use-site. We verify the Relative rule reads at the
    // use-site + N, NOT at use-site + base + N (which would be a
    // double-bias bug).
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Subroutine body: a Relative(0) rule that reads at the
    // use-site (seeded via last_match_end).
    let mut rel_rule = byte_eq_rule(0, 0x42, "rel-sub-match");
    rel_rule.offset = OffsetSpec::Relative(0);
    let subroutine_body = vec![rel_rule];
    let name_table = build_name_table(vec![("rsub", subroutine_body)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(name_table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });

    let mut buffer = vec![0u8; 16];
    buffer[5] = 0x42;

    let mut context = EvaluationContext::new(config).with_rule_env(env);
    let rules = vec![use_rule_at("rsub", 5)];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.iter().any(|m| m.message == "rel-sub-match"),
        "subroutine Relative(0) rule must read at use-site (5) via last_match_end, \
         not at use-site+base (10). got {matches:?}"
    );
}

#[test]
fn test_continuation_sibling_reset_after_bytes_consumed() {
    // Stronger regression guard than
    // `test_offset_does_not_advance_anchor_for_continuation_siblings`,
    // which used Relative(0) on both siblings and was trivially
    // non-advancing. Here the first sibling consumes actual bytes,
    // so if the `is_child_sibling_list` reset is removed the second
    // sibling would read from a shifted anchor.
    //
    // Parent byte at 0 matches 0x01 -> anchor = 1.
    // Sibling-1: Long at &0 (resolves to 1, reads 4 bytes,
    //            advances anchor to 5 WITHOUT the reset).
    // Sibling-2: Byte at &0 (must resolve to 1, not 5).
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    let long_sibling = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Long {
            endian: crate::parser::ast::Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x0403_0201),
        message: "long-sibling".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let byte_sibling = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        // buffer[1] = 0x01 -- if reset is removed, sibling-2 reads
        // buffer[5] instead and matches 0x42 (wrong!).
        value: Value::Uint(0x01),
        message: "byte-sibling-sees-parent-anchor".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "parent".to_string(),
        children: vec![long_sibling, byte_sibling],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    // buffer[0]=0x01 parent; buffer[1..5]=0x01,0x02,0x03,0x04 long
    // match; buffer[5]=0x42 bait for missing-reset failure.
    let buffer = [0x01u8, 0x01, 0x02, 0x03, 0x04, 0x42, 0x00];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["parent", "long-sibling", "byte-sibling-sees-parent-anchor"],
        "byte-sibling must read buffer[1]=0x01 via parent-level anchor reset; \
         if reset is missing it reads buffer[5]=0x42 and test fails. got {matches:?}"
    );
}
