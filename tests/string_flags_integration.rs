// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration and conformance tests for `string`-type flag semantics
//! (issue #234).
//!
//! Each test corresponds to a real-world magic(5) rule shape that surfaced
//! during scope analysis. The rules are constructed in-memory rather than
//! loaded from a `.magic` file so the test stays self-contained -- the
//! grammar layer is already covered by per-flag tests in
//! `src/parser/grammar/tests/mod.rs`.
//!
//! The tests assert behavior against the contract documented in libmagic
//! `src/softmagic.c` (case-fold direction, whitespace flexibility, word
//! boundary, trim) and `src/file.h` (flag-letter to `STRING_*` constant
//! mapping). GOTCHAS S6.5 / S6.6 cover the non-obvious bits.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate/module instead.
#![allow(clippy::semicolon_outside_block)]

use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
use libmagic_rs::parser::ast::StringFlags;
use libmagic_rs::{EvaluationConfig, MagicRule, OffsetSpec, Operator, TypeKind, Value};

fn cfg() -> EvaluationConfig {
    EvaluationConfig::default().with_stop_at_first_match(false)
}

fn rule(offset: i64, pattern: &str, flags: StringFlags, msg: &str) -> MagicRule {
    MagicRule::new(
        OffsetSpec::Absolute(offset),
        TypeKind::String {
            max_length: None,
            flags,
        },
        Operator::Equal,
        Value::String(pattern.to_string()),
        msg.to_string(),
    )
}

// ---------- /c: case-insensitive ----------

/// ExFAT-style real rule: `/usr/share/file/magic/filesystems:265`
/// uses `string/w =EXFAT` (whitespace variant); the case-insensitive variant
/// is the canonical /c application in archive-style rules.
#[test]
fn string_c_matches_exfat_any_case() {
    let r = rule(
        3,
        "exfat",
        StringFlags::default().with_ignore_lowercase(true),
        "ExFAT filesystem",
    );
    for buf in [
        &b"___EXFAT____"[..],
        &b"___ExFaT____"[..],
        &b"___exfat____"[..],
    ] {
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&r), buf, &mut ctx).unwrap();
        assert_eq!(matches.len(), 1, "string/c exfat should match {buf:?}");
    }
}

// ---------- /w: optional whitespace ----------

/// Python shebang variant: `/usr/share/file/magic/python:219` uses
/// `string/w "#!\040/usr/bin/python"`. The `/w` flag lets the file have
/// zero or more whitespace bytes wherever the pattern has one.
#[test]
fn string_w_matches_python_shebang_with_zero_spaces() {
    let r = rule(
        0,
        "#! /usr/bin/python",
        StringFlags::default().with_compact_optional_whitespace(true),
        "Python script",
    );
    let buf = b"#!/usr/bin/python script.py";
    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[r], buf, &mut ctx).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "string/w must accept zero file whitespace"
    );
}

#[test]
fn string_w_matches_python_shebang_with_multiple_spaces() {
    let r = rule(
        0,
        "#! /usr/bin/python",
        StringFlags::default().with_compact_optional_whitespace(true),
        "Python script",
    );
    let buf = b"#!   /usr/bin/python script.py";
    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[r], buf, &mut ctx).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "string/w must accept multiple file whitespace"
    );
}

// ---------- /b: blank-handling / binary hint (#51 archive rule) ----------

/// FTCOMP real rule: `/usr/share/file/magic/archive:695` uses
/// `24 string/b FTCOMP`. `/b` is a binary-mode hint that does not alter
/// the comparison itself (per current scope), so this is effectively the
/// same as a byte-exact match. The test verifies the flag is captured AND
/// the match works -- a regression guard for the parse-and-drop bug if
/// it ever resurfaces.
#[test]
fn string_b_matches_ftcomp_at_offset_24() {
    let mut buffer = vec![0u8; 24];
    buffer.extend_from_slice(b"FTCOMP_archive_data");
    let r = rule(
        24,
        "FTCOMP",
        StringFlags::default().with_bin_test(true),
        "FTCOMP compressed archive",
    );
    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[r], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 1);
}

// ---------- /T: trim leading/trailing whitespace from pattern ----------

#[test]
fn string_capital_t_trims_pattern_whitespace() {
    // Pattern has leading and trailing spaces; trim should match against
    // a buffer that contains only the inner content. This proves the
    // trim happened at evaluation time before the comparison.
    let r = rule(
        0,
        "  hello  ",
        StringFlags::default().with_trim(true),
        "hello",
    );
    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[r], b"hello world", &mut ctx).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "trim should make `  hello  ` match buffer `hello world`"
    );
}

// ---------- /f: full-word boundary ----------

#[test]
fn string_f_requires_word_boundary_after_match() {
    let r = rule(
        0,
        "int",
        StringFlags::default().with_full_word(true),
        "int keyword",
    );
    // Followed by space -> match.
    {
        let mut ctx = EvaluationContext::new(cfg());
        let m = evaluate_rules(std::slice::from_ref(&r), b"int x = 0", &mut ctx).unwrap();
        assert_eq!(m.len(), 1);
    }
    // Followed by alphanumeric -> no match (not a full word).
    {
        let mut ctx = EvaluationContext::new(cfg());
        let m = evaluate_rules(std::slice::from_ref(&r), b"integer x", &mut ctx).unwrap();
        assert!(m.is_empty(), "`int` inside `integer` must NOT match /f");
    }
    // Followed by underscore -> no match (`_` is a word char per libmagic).
    {
        let mut ctx = EvaluationContext::new(cfg());
        let m = evaluate_rules(&[r], b"int_var", &mut ctx).unwrap();
        assert!(m.is_empty(), "underscore is a word char");
    }
}

// ---------- /cw: combined ----------

#[test]
fn string_cw_combines_case_fold_and_whitespace_flexibility() {
    let r = rule(
        0,
        "foo bar",
        StringFlags::default()
            .with_ignore_lowercase(true)
            .with_compact_optional_whitespace(true),
        "combo",
    );
    // Different case + collapsed whitespace -> match.
    let mut ctx = EvaluationContext::new(cfg());
    let m = evaluate_rules(&[r], b"FOOBAR rest", &mut ctx).unwrap();
    assert_eq!(
        m.len(),
        1,
        "string/cw should match FOOBAR (case-folded, no whitespace)"
    );
}

// ---------- Regression: plain string semantics unchanged ----------

#[test]
fn string_without_flags_remains_case_sensitive() {
    // Sanity: a `string` rule with default flags must behave exactly as
    // before this PR -- byte-exact comparison.
    let r = rule(0, "foo", StringFlags::default(), "plain");
    let mut ctx = EvaluationContext::new(cfg());
    let m = evaluate_rules(&[r], b"FOObar", &mut ctx).unwrap();
    assert!(
        m.is_empty(),
        "default-flag string must still be case-sensitive (no regression)"
    );
}

// ---------- Regression guard: string/B rejected at parse time ----------

/// `/B` is exclusively the `pstring` 1-byte length-width letter, NOT a
/// string flag. The grammar must reject `string/B` rather than silently
/// accepting it (the PR #233 bug surfaced during the #234 scope audit).
/// This guard exercises the `parse_text_magic_file` path -- the same
/// path that loads user-provided `.magic` files at runtime.
#[test]
fn parse_text_magic_string_b_flag_is_rejected() {
    use libmagic_rs::parser::parse_text_magic_file;
    let result = parse_text_magic_file("0 string/B FOO bar\n");
    assert!(
        result.is_err(),
        "string/B should be a parse error -- /B is a pstring suffix, not a string flag"
    );
}

// ---------- Regression guard: /T with all-whitespace pattern ----------

/// `string/T "   "` would silently match every file if we let the empty
/// post-trim pattern through to `compare_string_with_flags` (which
/// returns `Some(0)` for an empty pattern -- the same hazard documented
/// in GOTCHAS S2.5 for regex). The fix in `read_pattern_match` logs a
/// `warn!` and returns `Ok(None)` (no match) so the malformed rule
/// surfaces in logs without aborting evaluation of subsequent rules in
/// the file.
#[test]
fn string_capital_t_with_all_whitespace_pattern_does_not_match_everything() {
    let r = rule(
        0,
        "   ", // pattern is pure whitespace; trim produces empty
        StringFlags::default().with_trim(true),
        "should not match",
    );
    let mut ctx = EvaluationContext::new(cfg());
    // Run against a file the rule would catastrophically over-match if
    // the empty-pattern hazard were unguarded.
    let matches = evaluate_rules(std::slice::from_ref(&r), b"any file content", &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "string/T with all-whitespace pattern must not match every file"
    );
}

// ---------- /T + /f interaction ----------

/// Combining `/T` (trim pattern) with `/f` (require word boundary after
/// match) should work: trim narrows the pattern to its non-whitespace
/// core, then `/f` checks the byte after the matched core.
#[test]
fn string_capital_t_combined_with_f_enforces_boundary_on_trimmed_core() {
    // Pattern " int " trims to "int"; /f then requires the byte after
    // "int" to be EOF or non-word.
    let r = rule(
        0,
        " int ",
        StringFlags::default().with_trim(true).with_full_word(true),
        "int keyword",
    );
    {
        let mut ctx = EvaluationContext::new(cfg());
        let m = evaluate_rules(std::slice::from_ref(&r), b"int x = 0", &mut ctx).unwrap();
        assert_eq!(m.len(), 1, "trimmed 'int' should match with space boundary");
    }
    {
        let mut ctx = EvaluationContext::new(cfg());
        let m = evaluate_rules(std::slice::from_ref(&r), b"integer x", &mut ctx).unwrap();
        assert!(
            m.is_empty(),
            "trimmed 'int' must not match inside 'integer' (/f boundary check)"
        );
    }
}
