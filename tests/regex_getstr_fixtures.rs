// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

// Helper functions in this file (not themselves `#[test]` fns) call
// `.expect()` on evaluation results; clippy's `allow-expect-in-tests`
// (clippy.toml) only recognizes code directly inside `#[test]` bodies as
// test code, not helpers called from them. See `tests/integration_tests.rs`
// and `tests/directory_loading_tests.rs` for the same established pattern.
#![allow(clippy::expect_used)]

//! Behavioral positive/negative fixture matrix for the getstr-resolved
//! `regex` rules in `/usr/share/file/magic/assembler` (U5 of the
//! `fix/system-magic-regex-graceful` plan).
//!
//! These tests prove the getstr-**resolved** patterns actually match the
//! right inputs and reject the wrong ones -- not merely that evaluation
//! completes without a fatal error. Per the plan's Verification Contract:
//! "Accuracy is the core value ... a non-crashing wrong answer is a
//! failure."
//!
//! # Why AST-built rules, not a parsed `.magic` file
//!
//! `parse_text_magic_file` is fail-fast (GOTCHAS S3.11): a single
//! unparseable line anywhere in a real `.magic` file aborts the entire
//! load, and the system magic DB (`/usr/share/file/magic/`) may not be
//! present in CI. So these always-run fixtures build the equivalent
//! `MagicRule` tree programmatically via the AST, mirroring
//! `tests/evaluator_tests.rs::test_regex_eol_corpus`. The real-DB,
//! real-file assertions (loading `/usr/share/file/magic/` itself and
//! diffing against GNU `file`) live in the separately gated
//! `tests/system_magic_dir.rs`.
//!
//! # Resolved pattern provenance
//!
//! The seven `assembler` rules (`grep -nE 'regex' /usr/share/file/magic/assembler`)
//! all share one pattern shape, differing only in the trailing directive
//! keyword:
//!
//! ```text
//! 0   regex   \^[\040\t]{0,50}\\.KEYWORD      assembler source text
//! ```
//!
//! for `KEYWORD` in `asciiz`, `byte`, `even`, `globl`, `text`, `file`,
//! `type`. Applying the getstr escape table documented in
//! `src/parser/grammar/getstr/mod.rs` (verified against GNU `file`'s
//! `apprentice.c::getstr`):
//!
//! - `\^`   -> unrecognized escape -> drop backslash -> `^`
//! - `\040` -> octal escape -> space byte (`0x20`)
//! - `\t`   -> named escape -> raw tab byte (`0x09`)
//! - `\\`   -> escape of backslash -> literal `\`
//! - `.`    -> not preceded by an unconsumed backslash -> passthrough
//!
//! so `\\.` resolves to the two-character sequence backslash + dot
//! (`\.`), which the regex engine reads as an *escaped* (literal) dot --
//! not the wildcard metacharacter. The full resolved pattern is
//! `^[ \t]{0,50}\.KEYWORD` (matching the plan's worked example exactly:
//! `\^[\040\t]{0,50}\\.asciiz` -> `^[ \t]{0,50}\.asciiz`).
//!
//! Rule messages for these rules are the plain literal `"assembler
//! source text"` with no `%`-format specifiers and no leading `\b`
//! (backspace), so GOTCHAS S14's printf-substitution and
//! backspace-concatenation conventions do not apply here.

use libmagic_rs::evaluator::evaluate_rules;
use libmagic_rs::parser::ast::{RegexCount, RegexFlags};
use libmagic_rs::{
    EvaluationConfig, EvaluationContext, MagicRule, OffsetSpec, Operator, TypeKind, Value,
};

/// Build a top-level `regex` rule using the getstr-resolved pattern text,
/// exactly as `src/parser/grammar/getstr/mod.rs` would produce it for a
/// magic-file `regex` rule with no flags and no explicit count.
fn assembler_regex_rule(resolved_pattern: &str) -> MagicRule {
    MagicRule::new(
        OffsetSpec::Absolute(0),
        TypeKind::Regex {
            flags: RegexFlags::default(),
            count: RegexCount::Default,
        },
        Operator::Equal,
        Value::String(resolved_pattern.to_string()),
        "assembler source text".to_string(),
    )
}

/// Evaluate `rule` against `buffer` and report whether it produced a
/// top-level match.
fn matches(rule: &MagicRule, buffer: &[u8]) -> bool {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let result = evaluate_rules(std::slice::from_ref(rule), buffer, &mut context)
        .expect("evaluation must not fail fatally");
    !result.is_empty()
}

/// Getstr-resolved pattern for `\^[\040\t]{0,50}\\.KEYWORD`, given
/// `KEYWORD`. See the module doc for the escape-by-escape derivation.
fn resolved_pattern_for(keyword: &str) -> String {
    format!("^[ \t]{{0,50}}\\.{keyword}")
}

/// One positive + one negative fixture per affected `assembler` regex
/// rule (all seven `regex` rules in
/// `/usr/share/file/magic/assembler`), pinning both sides of the match
/// per the plan's accuracy mandate.
#[test]
fn test_all_affected_assembler_regex_rules_positive_and_negative() {
    // (keyword, positive buffer that must match, negative buffer that
    // must not match, scenario description)
    let cases: &[(&str, &[u8], &[u8], &str)] = &[
        (
            "asciiz",
            b"\t.asciiz \"hi\"",
            b"xyz .asciiz",
            "leading tab then .asciiz matches; non-whitespace prefix does not",
        ),
        (
            "byte",
            b"    .byte 0x01\n",
            b"a.byte 0x01\n",
            "leading spaces then .byte matches; non-whitespace prefix does not",
        ),
        (
            "even",
            b".even\n",
            b"1.even\n",
            "zero leading whitespace then .even matches; digit prefix does not",
        ),
        (
            "globl",
            b"\t\t.globl main\n",
            b"foo.globl main\n",
            "multiple leading tabs then .globl matches; word prefix does not",
        ),
        (
            "text",
            b"  .text\n",
            b"#.text\n",
            "leading spaces then .text matches; punctuation prefix does not",
        ),
        (
            "file",
            b"\t.file \"a.s\"\n",
            b"z.file \"a.s\"\n",
            ".file with leading tab matches; letter prefix does not",
        ),
        (
            "type",
            b".type foo,@function\n",
            b"_type foo,@function\n",
            ".type at column 0 matches; underscore prefix does not",
        ),
    ];

    for (keyword, positive, negative, description) in cases {
        let pattern = resolved_pattern_for(keyword);
        let rule = assembler_regex_rule(&pattern);

        assert!(
            matches(&rule, positive),
            "keyword {keyword:?} ({description}): expected positive buffer {:?} to match \
             resolved pattern {pattern:?}",
            String::from_utf8_lossy(positive)
        );
        assert!(
            !matches(&rule, negative),
            "keyword {keyword:?} ({description}): expected negative buffer {:?} to NOT match \
             resolved pattern {pattern:?}",
            String::from_utf8_lossy(negative)
        );
    }
}

/// `{0,50}` boundary: zero leading whitespace characters is within
/// bounds (the lower bound of the quantifier) and must match; 51 leading
/// spaces exceeds the upper bound and must not match anywhere in the
/// buffer (there is no embedded newline to give `^` a second anchor
/// point).
#[test]
fn test_leading_whitespace_quantifier_boundary() {
    let rule = assembler_regex_rule(&resolved_pattern_for("asciiz"));

    let zero_whitespace = b".asciiz";
    assert!(
        matches(&rule, zero_whitespace),
        "0 leading whitespace chars is within {{0,50}}'s lower bound and must match"
    );

    let fifty_one_spaces: Vec<u8> = " "
        .repeat(51)
        .into_bytes()
        .into_iter()
        .chain(*b".asciiz")
        .collect();
    assert!(
        !matches(&rule, &fifty_one_spaces),
        "51 leading spaces exceeds {{0,50}}'s upper bound and must not match"
    );

    // Sanity check the boundary is exercised precisely: exactly 50
    // leading spaces (the upper bound, inclusive) must still match.
    let fifty_spaces: Vec<u8> = " "
        .repeat(50)
        .into_bytes()
        .into_iter()
        .chain(*b".asciiz")
        .collect();
    assert!(
        matches(&rule, &fifty_spaces),
        "50 leading spaces is exactly {{0,50}}'s upper bound (inclusive) and must match"
    );
}

/// The resolved pattern's `\.` must be a literal escaped dot, not the
/// regex wildcard metacharacter `.`. This is the behavioral proof that
/// the getstr resolver correctly emits the two-character `\.` sequence
/// (backslash from resolving the magic-file `\\`, followed by the
/// passthrough literal `.`) rather than a bare unescaped `.`. If the
/// resolver regressed to emitting a bare `.`, this test would start
/// passing on `Xasciiz` (since `.` as a wildcard matches any single
/// byte), so it is a real regression guard, not a tautology.
#[test]
fn test_escaped_dot_is_literal_not_wildcard() {
    let rule = assembler_regex_rule(&resolved_pattern_for("asciiz"));

    assert!(
        matches(&rule, b".asciiz"),
        "a literal dot followed by the keyword must match"
    );
    assert!(
        !matches(&rule, b"Xasciiz"),
        "a wildcard-eligible non-dot character in the dot's position must NOT match -- \
         if this passes, \\. regressed to an unescaped wildcard `.`"
    );
}

/// Behavioral coverage for the KTD3 `>= 0x80` byte re-encoding path.
///
/// None of the seven affected `assembler` rules contain an escape that
/// resolves to a byte `>= 0x80`, so per the plan's U5 requirement this is
/// a synthetic AST rule built the same way the getstr resolver would
/// build one: a resolved pattern containing a regex-native `\xHH` escape
/// (as produced by `push_resolved_byte` in
/// `src/parser/grammar/getstr/mod.rs` for a magic-file escape like
/// `\377` or `\xFF`), asserted to match a buffer containing the raw
/// byte and to reject a buffer that lacks it.
///
/// This exercises `src/evaluator/types/regex.rs::build_regex`'s
/// `unicode(false)` setting end-to-end: with Unicode mode enabled (the
/// `regex::bytes::Regex` default), `\xff` matches the *UTF-8 encoding*
/// of U+00FF (`0xC3 0xBF`), not the raw byte `0xFF` -- silently breaking
/// this exact contract. See the regression test colocated with
/// `build_regex` (`test_read_regex_high_byte_escape_matches_raw_byte_not_utf8_encoding`
/// in `src/evaluator/types/regex.rs`) for the isolated unit-level proof;
/// this integration-level test proves the same contract holds when
/// exercised through the full `MagicRule` -> `evaluate_rules` path that
/// real magic-file rules use.
#[test]
fn test_high_byte_escape_synthetic_rule_matches_raw_byte_and_rejects_without_it() {
    // Getstr-resolved form of a hypothetical `regex \xFF marker` rule:
    // the parser would emit the raw byte 0xFF re-encoded as the
    // regex-native `\xff` hex escape (KTD3), never as a raw 0x80+ byte
    // pushed directly into the `String`.
    let resolved_pattern = "marker\\xff";
    let rule = assembler_regex_rule(resolved_pattern);

    let with_high_byte: Vec<u8> = b"marker".iter().copied().chain([0xffu8]).collect();
    assert!(
        matches(&rule, &with_high_byte),
        "resolved pattern {resolved_pattern:?} must match a buffer containing the raw byte 0xFF"
    );

    let without_high_byte = b"markerX";
    assert!(
        !matches(&rule, without_high_byte),
        "resolved pattern {resolved_pattern:?} must NOT match a buffer lacking the raw byte 0xFF"
    );

    // Negative control: the UTF-8 *encoding* of U+00FF (the codepoint
    // \xff would denote under Unicode-mode regex semantics) must not be
    // mistaken for the raw byte -- pins the same contract from the
    // opposite direction.
    let utf8_encoded_codepoint: Vec<u8> = b"marker".iter().copied().chain([0xc3u8, 0xbf]).collect();
    assert!(
        !matches(&rule, &utf8_encoded_codepoint),
        "resolved pattern {resolved_pattern:?} must NOT match the UTF-8 encoding of U+00FF -- \
         if this passes, byte-level matching regressed back to Unicode-scalar matching"
    );
}
