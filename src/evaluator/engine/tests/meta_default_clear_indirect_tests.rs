// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests for `MetaType::Default`, `MetaType::Clear`, and
//! `MetaType::Indirect` dispatch.
//!
//! Shared helpers (`default_rule`, `clear_rule`, `byte_eq_rule`,
//! `indirect_rule`, `make_context_with_env`, `build_name_table`) live in
//! the parent `tests/mod.rs` module.

use super::*;

#[test]
fn test_default_fires_when_no_sibling_matched() {
    let rules = vec![default_rule("DEFAULT-FIRES", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "default with no prior sibling match should fire"
    );
    assert_eq!(matches[0].message, "DEFAULT-FIRES");
}

#[test]
fn test_default_skipped_when_sibling_matched() {
    // Disable stop-at-first-match so we can see whether the default would
    // have fired or not.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        byte_eq_rule(0, 0xAA, "real-match"),
        default_rule("DEFAULT-SKIPPED", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "default after a successful sibling should not fire"
    );
    assert_eq!(matches[0].message, "real-match");
}

#[test]
fn test_default_fires_only_once() {
    // Two consecutive default rules: the first sets sibling_matched, so
    // the second must not fire.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        default_rule("FIRST-DEFAULT", vec![]),
        default_rule("SECOND-DEFAULT", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "only the first default should fire when no real sibling matched"
    );
    assert_eq!(matches[0].message, "FIRST-DEFAULT");
}

#[test]
fn test_default_children_evaluated() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let child = byte_eq_rule(0, 0xAA, "default-child");
    let rules = vec![default_rule("PARENT-DEFAULT", vec![child])];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "default rule's children must be evaluated when the default fires"
    );
    assert_eq!(matches[0].message, "PARENT-DEFAULT");
    assert_eq!(matches[1].message, "default-child");
}

#[test]
fn test_clear_resets_sibling_matched() {
    // Sequence: byte-match, default-skipped, clear, default-fires.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        byte_eq_rule(0, 0xAA, "byte-match"),
        default_rule("DEFAULT-SKIPPED", vec![]),
        clear_rule(),
        default_rule("DEFAULT-FIRES-AFTER-CLEAR", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "clear must reset sibling_matched so a later default fires"
    );
    assert_eq!(matches[0].message, "byte-match");
    assert_eq!(matches[1].message, "DEFAULT-FIRES-AFTER-CLEAR");
}

#[test]
fn test_clear_at_top_is_noop() {
    let rules = vec![clear_rule(), default_rule("AFTER-CLEAR", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "clear at top of list is a no-op; default after still fires"
    );
    assert_eq!(matches[0].message, "AFTER-CLEAR");
}

#[test]
fn test_clear_does_not_produce_match() {
    let rules = vec![clear_rule()];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(matches.is_empty(), "clear alone must produce no match");
}

#[test]
fn test_default_clear_per_level_isolation() {
    // Parent has its own sibling_matched flag. The child list runs with a
    // fresh flag, so a child-level `default` must fire even though the
    // parent's flag is true.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "parent-match".to_string(),
        children: vec![
            byte_eq_rule(1, 0xBB, "child-byte-match"),
            default_rule("CHILD-DEFAULT-SKIPPED", vec![]),
            clear_rule(),
            default_rule("CHILD-DEFAULT-AFTER-CLEAR", vec![]),
        ],
        level: 0,
        strength_modifier: None,
    };
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();

    // Expected order: parent-match, child-byte-match, CHILD-DEFAULT-AFTER-CLEAR
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "parent-match",
            "child-byte-match",
            "CHILD-DEFAULT-AFTER-CLEAR"
        ],
        "child-level sibling_matched must be isolated from parent-level state"
    );
}

#[test]
fn test_indirect_evaluates_root_rules_at_offset() {
    // Root rules: detect a "ZIP-like" header (0x50 0x4b) at offset 0 of the
    // sub-buffer. The indirect rule fires at offset 4 of the outer buffer,
    // which means the sub-buffer starts at byte 4. Place 0x50 0x4b there.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let root_rule = byte_eq_rule(0, 0x50, "ZIP-like-header");
    let root_rules: Vec<MagicRule> = vec![root_rule];

    // Build an environment where root_rules is the same as the rules we
    // dispatch into.
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    // Buffer: ELF magic at offset 0, ZIP-like at offset 4. The indirect
    // rule is the trigger; the root re-entry detects 0x50 at sub-buffer 0.
    let buffer = [0x7fu8, 0x45, 0x4c, 0x46, 0x50, 0x4b, 0x03, 0x04];
    let rules = vec![indirect_rule(4, "indirect-trigger", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| m.message == "ZIP-like-header"),
        "indirect must dispatch root rules against the sub-buffer at offset 4; got {matches:?}"
    );
}

#[test]
fn test_indirect_out_of_bounds_is_noop() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(&[byte_eq_rule(0, 0x00, "root")] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let buffer = [0u8; 4];
    // Indirect at offset 100, which is well past the 4-byte buffer.
    let rules = vec![indirect_rule(100, "indirect-oob", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "indirect past buffer end must be a graceful no-op"
    );
}

#[test]
fn test_indirect_without_env_is_noop() {
    // Property tests synthesize Indirect rules without an attached
    // RuleEnvironment, so this path must be a graceful no-op (matching the
    // `Use`-without-env contract). The engine logs at `debug!` rather than
    // panicking via `debug_assert!` to preserve the never-panics invariant
    // exercised by `prop_arbitrary_rule_evaluation_never_panics`.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let buffer = [0u8; 4];
    let rules = vec![indirect_rule(0, "indirect-no-env", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "indirect without env must produce no matches"
    );
}

#[test]
fn test_indirect_recursion_limit() {
    // Root rules contain an indirect rule that points back to offset 0,
    // creating an infinite re-entry chain. Must surface as
    // `RecursionLimitExceeded`, not stack overflow.
    let inner_indirect = indirect_rule(0, "recursive-indirect", vec![]);
    let root_rules: Vec<MagicRule> = vec![inner_indirect];
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(EvaluationConfig::default()).with_rule_env(env);

    let buffer = [0u8; 8];
    let rules = vec![indirect_rule(0, "outer-indirect", vec![])];
    let result = evaluate_rules(&rules, &buffer, &mut context);
    assert!(
        matches!(
            result,
            Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            ))
        ),
        "infinite indirect recursion must surface RecursionLimitExceeded, got {result:?}"
    );
}

/// Regression test for `RFn1`: `indirect` inside a `use` subroutine must reset
/// `base_offset` to 0 when re-entering root rules.
///
/// Before the fix, `AnchorScope::enter` only saved/restored `last_match_end`.
/// When `indirect` fired inside a `use` subroutine that had set `base_offset`
/// to a non-zero use-site, the re-entered root rules inherited that
/// `base_offset`. Every positive `Absolute(N)` offset in the root rules was
/// then biased by the outer use-site offset, causing reads at the wrong
/// positions (e.g., a rule at `Absolute(0)` would read from `use-site + 0`
/// rather than from byte 0 of the sub-buffer).
///
/// Layout of this test:
/// - Outer buffer: 8 bytes
///   - Bytes 0-3: dummy header (0x11 0x22 0x33 0x44)
///   - Bytes 4-7: embedded payload (0xAA 0xBB 0xCC 0xDD)
/// - A `use sub` rule at offset 4 dispatches a subroutine (`base_offset=4`).
/// - The subroutine contains an `indirect` rule also at offset 0 (use-site
///   relative, resolves to absolute 4 under `base_offset` biasing = byte 4).
/// - The root rules check byte 0 of the sub-buffer (== outer byte 4) for 0xAA.
/// - After the fix: root rules see `base_offset=0`, so `Absolute(0)` reads
///   `sub-buffer[0]` == 0xAA => match.
/// - Before the fix: root rules inherited `base_offset=4`, so `Absolute(0)` was
///   biased to effective offset 4 of the 4-byte sub-buffer => `BufferOverrun` =>
///   no match.
#[test]
fn test_indirect_inside_use_subroutine_resets_base_offset() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Root rules: check that byte 0 of the re-entered sub-buffer equals 0xAA.
    // `Absolute(0)` must resolve to sub-buffer[0], NOT sub-buffer[4] (which
    // would be the biased result if base_offset leaked from the use subroutine).
    let root_rules: Vec<MagicRule> = vec![byte_eq_rule(0, 0xAA, "root-payload-match")];

    // Subroutine body: an `indirect` rule at offset 0 (relative to the use-site
    // base). Inside a use subroutine with base_offset=4, `Absolute(0)` resolves
    // to absolute 4 -- the start of the payload in the outer buffer. That slice
    // becomes the sub-buffer passed to root rule re-entry.
    let subroutine_body: Vec<MagicRule> = vec![indirect_rule(0, "inner-indirect", vec![])];

    let table = build_name_table(vec![("sub", subroutine_body)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    // Buffer: 4 dummy bytes, then 4 payload bytes starting with 0xAA.
    let buffer = [0x11u8, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC, 0xDD];
    // `use sub` at use-site offset 4 -- sets base_offset=4 for the subroutine.
    let rules = vec![use_rule_at("sub", 4)];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| m.message == "root-payload-match"),
        "indirect inside use must reset base_offset to 0 so root rules read from \
         sub-buffer[0], not sub-buffer[base+0]; got {matches:?}"
    );
}
