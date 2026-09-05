// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate/module instead.
#![allow(clippy::modulo_arithmetic)]

use super::*;
use crate::parser::ast::{
    Endianness, IndirectAdjustmentOp, OffsetSpec, Operator, SearchFlags, StringFlags, TypeKind,
    Value,
};

/// Legacy one-shot single-rule helper used by the engine unit tests.
///
/// The public [`evaluate_single_rule`] API was reshaped in todo 025 to
/// accept a mutable [`EvaluationContext`] and return `Vec<RuleMatch>` by
/// delegating through [`evaluate_rules`]. That delegation folds
/// data-dependent errors (buffer overrun, invalid offset, etc.) into an
/// empty vector -- great for library callers, but many of the tests
/// below were written against the older raw evaluator which returned
/// `Result<Option<(usize, Value)>, LibmagicError>` and specifically
/// asserted the `Err` path on out-of-bounds reads. This helper preserves
/// that lower-level contract so the historical tests keep exercising the
/// raw evaluator semantics without being rewritten en masse; the new
/// public surface is covered by its own targeted tests.
fn evaluate_single_rule_legacy(
    rule: &MagicRule,
    buffer: &[u8],
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    evaluate_single_rule_with_anchor(
        rule,
        buffer,
        0,
        0,
        crate::evaluator::types::DEFAULT_MAX_STRING_LENGTH,
        false,
    )
}

/// Build a flat, top-level, message-only byte rule matching a distinct
/// value at a distinct offset. Shared by the `stop_at_first_match`
/// message-bearing tests below so each test only needs to state the
/// interesting bit: which offsets carry which messages.
fn message_only_byte_rule(offset: i64, byte: u8, message: &str) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(byte)),
        message: message.to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

// ============================================================
// Deep nesting & resource exhaustion safety tests (todo 028)
// ============================================================

/// Build a linear chain of `depth` nested rules, each reading a distinct byte.
/// Level 0 reads buffer[0], level 1 reads buffer[1], ..., level (depth-1) reads
/// buffer[depth-1]. The buffer is constructed so every level matches.
fn build_linear_nested_chain(depth: u32) -> (MagicRule, Vec<u8>) {
    assert!(depth > 0, "depth must be > 0");
    let buffer: Vec<u8> = (0..depth).map(|i| (i & 0xFF) as u8).collect();

    // Start with the deepest (innermost) rule and build outward.
    let last = depth - 1;
    let mut current = MagicRule {
        offset: OffsetSpec::Absolute(i64::from(last)),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(last & 0xFF)),
        message: format!("Level {last}"),
        children: vec![],
        level: last,
        strength_modifier: None,
        value_transform: None,
    };

    for i in (0..last).rev() {
        current = MagicRule {
            offset: OffsetSpec::Absolute(i64::from(i)),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i & 0xFF)),
            message: format!("Level {i}"),
            children: vec![current],
            level: i,
            strength_modifier: None,
            value_transform: None,
        };
    }

    (current, buffer)
}

// =============================================================================
// fix-system-magic-regex-graceful, U2: narrow graceful-skip of the
// missing-pattern-operand `TypeReadError::UnsupportedType` condition.
//
// Before this fix, a regex/search rule whose `value` operand was not a
// `String`/`Bytes` pattern (GOTCHAS S2.4) caused `evaluate_rules` to
// propagate a fatal `Err`, aborting evaluation of the ENTIRE rule set (and,
// via `MagicDatabase`, the entire magic file) rather than skipping just the
// one broken rule. See docs/plans/2026-07-17-001-fix-system-magic-regex-
// graceful-plan.md.
// =============================================================================

/// Builds a `TypeKind::Regex` rule whose `value` is `Value::Uint(0)` --
/// neither `Value::String` nor `Value::Bytes` -- so `read_pattern_match`
/// always returns `Err(UnsupportedType { type_name: "regex without string
/// pattern" })`, regardless of U1's `Value::Bytes` backstop. This isolates
/// U2's engine-level skip from U1's evaluator-level acceptance.
fn broken_pattern_regex_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

// -----------------------------------------------------------------------
// C2 hardening: the missing-pattern-operand skip is asserted end-to-end
// only for `Regex` above. `Search` and flagged `String` share the SAME
// allowlisted consts (`types::SEARCH_MISSING_PATTERN_MSG` /
// `types::FLAGGED_STRING_MISSING_PATTERN_MSG`) and the same three engine
// catch sites, so this closes R2 for every pattern-bearing type and
// guards the C1 const extraction against silent drift.
// -----------------------------------------------------------------------

/// Builds a `TypeKind::Search` rule whose `value` is `Value::Uint(0)` --
/// neither `Value::String` nor `Value::Bytes` -- so `read_pattern_match`
/// always returns `Err(UnsupportedType { type_name:
/// SEARCH_MISSING_PATTERN_MSG })`.
fn broken_pattern_search_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(16),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Builds a flagged `TypeKind::String` rule (non-empty `flags`, routing
/// through the pattern-bearing path per GOTCHAS S2.4) whose `value` is
/// `Value::Uint(0)`, so `read_pattern_match` always returns
/// `Err(UnsupportedType { type_name: FLAGGED_STRING_MISSING_PATTERN_MSG })`.
fn broken_pattern_flagged_string_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags {
                ignore_lowercase: true,
                ..StringFlags::default()
            },
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

// -----------------------------------------------------------------------
// E hardening: the compile-failure (warn!) skip is proven end-to-end only
// at the top-level dispatch site above
// (`test_pathological_regex_compile_failure_is_skipped_not_fatal`). Add
// the two missing sites for 3-site parity with the missing-pattern
// (debug!) coverage.
// -----------------------------------------------------------------------

/// Builds a `TypeKind::Regex` rule whose pattern is syntactically valid
/// (`Value::String`, so U1's `Value::Bytes` backstop is irrelevant here)
/// but rejected by the `REGEX_COMPILE_SIZE_LIMIT` (1 MiB) CWE-1333
/// denial-of-service guard at compile time.
fn pathological_regex_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("a{1000000}".to_string()),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

// -----------------------------------------------------------------------
// H hardening: pin the debug!/warn! log-level contract with a real
// log-capture seam (`testing_logger`, which captures the `log` facade
// this crate uses -- not `tracing`). Previously these contracts were
// asserted by code inspection only.
// -----------------------------------------------------------------------

/// Test-only helper: `testing_logger::CapturedLog` does not implement
/// `Debug`, so format captured logs manually for failure messages.
fn format_logs(logs: &[testing_logger::CapturedLog]) -> String {
    logs.iter()
        .map(|l| format!("{:?}: {}", l.level, l.body))
        .collect::<Vec<_>>()
        .join(", ")
}

mod deep_nesting_resource_tests;
mod evaluate_rules_basic_and_message_tests;
mod evaluate_rules_config_and_error_recovery_tests;
mod evaluate_rules_hierarchy_tests;
mod mixed_valid_invalid_pstring_tests;
mod operator_tests;
mod pattern_recovery_log_tests;
mod pattern_recovery_search_string_tests;
mod pattern_recovery_tests;
mod regex_search_operator_tests;
mod relative_anchor_tests;
mod search_anchor_endtoend_tests;
mod single_rule_error_and_type_tests;
mod single_rule_numeric_type_tests;

mod helpers;
pub(super) use crate::evaluator::RuleEnvironment;
pub(super) use crate::parser::ast::MetaType;
pub(super) use crate::parser::name_table::NameTable;
pub(super) use helpers::meta::*;

// Submodule declarations
#[cfg(test)]
mod meta_default_clear_indirect_tests;
#[cfg(test)]
mod meta_offset_tests;
#[cfg(test)]
mod meta_use_indirect_framing_tests;
#[cfg(test)]
mod meta_use_tests;
#[cfg(test)]
mod string_flags_dispatch_tests;
