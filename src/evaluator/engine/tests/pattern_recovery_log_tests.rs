// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tests pinning the pathological regex-compile-failure skip under
//! the remaining two dispatch sites, and the `debug!`/`warn!` log
//! level contract for missing-pattern-operand vs. compile-failure
//! skips (via `testing_logger`).

use super::*;

/// Child-recursion site: a regex compile-size rejection under a matched
/// parent must not abort the parent's match.
#[test]
fn test_pathological_regex_compile_failure_skipped_under_matched_parent_child_recursion_site() {
    let broken_child = pathological_regex_rule("broken compile child regex", 1);
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
    let matches = evaluate_rules(&[parent], b"aaaa", &mut context)
        .expect("a regex compile-size rejection under a matched parent must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a regex compile-size rejection as a
/// child of `default` must not abort the `default` match.
#[test]
fn test_pathological_regex_compile_failure_skipped_under_default_children_or_warn_site() {
    let broken_child = pathological_regex_rule("broken compile default child regex", 1);
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
    let matches = evaluate_rules(&[default_rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection under default children must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

/// The ordinary missing-pattern-operand skip (top-level site) logs at
/// `debug!`, not `warn!` -- it is an expected, low-severity data
/// condition, not a security-relevant signal.
#[test]
fn test_missing_pattern_operand_skip_logs_at_debug_level() {
    testing_logger::setup();
    let rule = broken_pattern_regex_rule("broken top-level regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"anything to scan", &mut context)
        .expect("pattern-less regex must be skipped, not fatal");
    assert!(matches.is_empty());
    testing_logger::validate(|captured_logs| {
        let skip_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("Skipping top-level rule"))
            .collect();
        assert_eq!(
            skip_logs.len(),
            1,
            "expected exactly one skip log entry, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            skip_logs[0].level,
            log::Level::Debug,
            "missing-pattern-operand skip must log at debug!, not warn! -- \
             got {:?}: {:?}",
            skip_logs[0].level,
            skip_logs[0].body
        );
    });
}

/// A regex compile-size rejection (`REGEX_COMPILE_SIZE_LIMIT`,
/// CWE-1333) logs at `warn!`, not `debug!` -- a malicious or pathological
/// magic file's rejection must stay visible in logs even though
/// evaluation of the rest of the file continues.
#[test]
fn test_regex_compile_failure_skip_logs_at_warn_level() {
    testing_logger::setup();
    let rule = pathological_regex_rule("pathological regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection must be skipped, not fatal");
    assert!(matches.is_empty());
    testing_logger::validate(|captured_logs| {
        let skip_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("regex compile failure"))
            .collect();
        assert_eq!(
            skip_logs.len(),
            1,
            "expected exactly one compile-failure log entry, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            skip_logs[0].level,
            log::Level::Warn,
            "regex compile-failure skip must log at warn!, not debug! -- \
             got {:?}: {:?}",
            skip_logs[0].level,
            skip_logs[0].body
        );
    });
}
