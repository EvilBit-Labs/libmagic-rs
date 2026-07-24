// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Graceful-skip logging and child-recursion dispatch shared across the
//! meta-type arms of [`super::evaluate_rules`].
//!
//! Extracted from `engine/mod.rs` as a pure code-motion split (issue #391
//! item 1, Unit U3). `log_pattern_operand_skip` centralizes the debug!/warn!
//! logging for the narrow, allowlisted pattern-operand-skip exception (see
//! GOTCHAS S2.1), and `evaluate_children_or_warn` centralizes the
//! `RecursionGuard` + `evaluate_rules` + error-dispatch pattern that is
//! identical across the `Default`, `Clear`, `Indirect`, `Offset`, and `Use`
//! meta-type arms in `evaluate_rules`.

use super::evaluate_rules;
use crate::LibmagicError;
use crate::evaluator::{EvaluationContext, RecursionGuard, RuleMatch, types};
use crate::parser::ast::MagicRule;
use log::{debug, warn};

/// Logs the graceful skip of a pattern-bearing-type rule whose
/// `TypeReadError` is one of the two narrow allowlisted skip conditions --
/// [`types::TypeReadError::MissingPatternOperand`] or
/// [`types::TypeReadError::RegexCompileError`] (see
/// `TypeReadError::is_pattern_skip` / `is_regex_compile_failure`).
///
/// Shared by all three engine catch sites (`evaluate_children_or_warn`, the
/// top-level dispatch match, and the inline child-recursion match) so a
/// future rewording of the log message only needs one touch point (DRY,
/// AGENTS.md). Split by KTD5 (fix-system-magic-regex-graceful plan): the
/// ordinary missing-pattern case is `debug!`-logged (an expected,
/// low-severity data condition -- e.g. the root-cause parser
/// miscategorization this plan also fixes), while a regex compile failure
/// (which includes the `REGEX_COMPILE_SIZE_LIMIT` CWE-1333 denial-of-service guard) is
/// `warn!`-logged so a malicious or pathological magic file's rejection is
/// not silently invisible, even though the rest of the file's evaluation
/// continues (R1: no fatal abort of the whole evaluation).
pub(crate) fn log_pattern_operand_skip(
    site_label: &str,
    rule_message: &str,
    err: &types::TypeReadError,
) {
    if err.is_regex_compile_failure() {
        warn!(
            "Skipping {site_label} rule '{rule_message}' due to regex compile failure: {err} -- this may indicate a malicious or pathological magic file"
        );
    } else {
        debug!("Skipping {site_label} rule '{rule_message}': {err}");
    }
}

/// Evaluate a rule's children under the standard recursion-guard/graceful-skip discipline.
///
/// This helper centralises the `RecursionGuard` + `evaluate_rules` + error-dispatch
/// pattern that is identical across the `Default`, `Indirect`, `Offset`, and `Use`
/// meta-type arms in [`evaluate_rules`]. Extracting it prevents the four copies
/// from drifting apart during future maintenance.
///
/// # Behaviour
///
/// * If `rule.children` is empty the function is a no-op (returns `Ok(())`).
/// * Child matches are appended to `matches` in document order.
/// * `LibmagicError::Timeout` and `LibmagicError::EvaluationError(RecursionLimitExceeded)`
///   propagate immediately as `Err` so the caller can bail out.
/// * Data-dependent errors (`BufferOverrun`, `InvalidOffset`,
///   `InvalidValueTransform`, `TypeReadError::BufferOverrun`,
///   `TypeReadError::InvalidPStringLength`, `IoError`) are logged at `warn!`
///   and swallowed; the parent match
///   already in `matches` is left intact. This mirrors the defensive
///   comment in each arm: the inner `evaluate_rules` already catches and
///   logs individual child failures, so this arm only fires if that
///   strategy changes.
///
/// # Arguments
///
/// * `rule`      – The parent rule whose children will be evaluated.
/// * `rule_kind` – A short label for the rule kind used in the `warn!`
///   message (e.g. `"default"`, `"indirect"`, `"offset"`, `"use"`).
/// * `buffer`    – The file buffer passed to the recursive call.
/// * `context`   – Mutable evaluation context; the recursion depth is
///   incremented on entry and decremented on drop via [`RecursionGuard`].
/// * `matches`   – Output vector; child matches are appended here.
pub(crate) fn evaluate_children_or_warn(
    rule: &MagicRule,
    rule_kind: &str,
    buffer: &[u8],
    context: &mut EvaluationContext,
    matches: &mut Vec<RuleMatch>,
) -> Result<(), LibmagicError> {
    if rule.children.is_empty() {
        return Ok(());
    }
    let mut guard = RecursionGuard::enter(context)?;
    match evaluate_rules(&rule.children, buffer, guard.context()) {
        Ok(child_matches) => {
            matches.extend(child_matches);
        }
        Err(LibmagicError::Timeout { timeout_ms }) => {
            return Err(LibmagicError::Timeout { timeout_ms });
        }
        // `RecursionLimitExceeded` is listed explicitly (rather than
        // relying on the catch-all below) so a future maintainer adding
        // another swallowed variant cannot accidentally swallow it.
        // Both this arm and the catch-all intentionally propagate via
        // `return Err(e)`; `match_same_arms` is suppressed because the
        // explicit arm's purpose is documentation and future-proofing,
        // not different behavior. See GOTCHAS S13 for the recursion-
        // depth guard contract.
        #[allow(clippy::match_same_arms)]
        Err(
            e @ LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. },
            ),
        ) => return Err(e),
        Err(
            e @ (LibmagicError::EvaluationError(
                crate::error::EvaluationError::BufferOverrun { .. }
                | crate::error::EvaluationError::InvalidOffset { .. }
                | crate::error::EvaluationError::InvalidValueTransform { .. }
                | crate::error::EvaluationError::TypeReadError(
                    crate::evaluator::types::TypeReadError::BufferOverrun { .. }
                    | crate::evaluator::types::TypeReadError::InvalidPStringLength { .. },
                ),
            )
            | LibmagicError::IoError(_)),
        ) => {
            warn!(
                "Discarding child evaluation under {} rule '{}' due to unexpected error: {} -- parent match is still emitted",
                rule_kind, rule.message, e
            );
        }
        // Narrow graceful-skip (KTD4): a pattern-bearing type evaluated
        // without a usable pattern operand, or a regex compile failure
        // (including the REGEX_COMPILE_SIZE_LIMIT DoS guard), must not
        // abort the whole evaluation -- see `log_pattern_operand_skip` and
        // the top-level dispatch match below for the full contract. This
        // arm is defensive: under the current implementation, individual
        // child failures are already caught and logged inside the
        // recursive `evaluate_rules` call (they never propagate here); it
        // guards against a future change to that strategy.
        Err(LibmagicError::EvaluationError(crate::error::EvaluationError::TypeReadError(
            ref tre,
        ))) if tre.is_pattern_skip() => {
            log_pattern_operand_skip(rule_kind, &rule.message, tre);
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
