// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `getstr` escape resolver ([`super::parse_regex_getstr_value`])
//! and integration-level guard tests through [`parse_magic_rule`] pinning
//! that `TypeKind::Regex` (and only `Regex`) is routed through it.
//!
//! U4 content-fidelity table: every row asserts the exact resolved
//! **text**, not merely the `Value` variant, so a future refactor of the
//! escape table cannot silently regress correctness (see the plan's
//! Verification Contract: "Accuracy is the core value").

use super::parse_regex_getstr_value;
use crate::parser::ast::{TypeKind, Value};
use crate::parser::grammar::parse_magic_rule;

/// U4 content-fidelity table: `(input, expected_resolved_regex_text)`.
///
/// Inputs are raw strings so the magic(5) source escapes (backslashes)
/// are represented literally, one-for-one, with no Rust-level
/// re-interpretation. Expected values are normal strings so Rust's own
/// `\t`/`\\` escapes conveniently produce the exact resolved bytes we
/// want to assert (a raw tab byte, a raw backslash byte) -- this is a
/// test-authoring convenience, not a claim that the resolver itself does
/// any Rust-string-escape interpretation.
#[test]
fn getstr_resolver_content_fidelity_table() {
    let cases: &[(&str, &str)] = &[
        // Worked example from the plan (KTD2): `\^` drops the backslash
        // (unknown escape) -> anchor `^`; `\040` is octal space; `\t` is
        // the named tab escape; `\\.` is backslash-then-backslash ->
        // resolves to ONE literal backslash, followed by the unescaped
        // `.` -> together `\.` (escaped dot for the regex engine).
        (r"\^[\040\t]{0,50}\\.asciiz", "^[ \t]{0,50}\\.asciiz"),
        // Octal space in isolation.
        (r"[\040]", "[ ]"),
        // Tab: named getstr escape -> raw tab byte.
        (r"\t", "\t"),
        // Unknown escape: drop the backslash, keep the literal char.
        (r"\^", "^"),
        // Escaped dot: `\\.``(two backslash chars + dot) -> `\.` (one
        // backslash byte + dot) -- required so the *regex* engine sees
        // an escaped, literal dot rather than the "any character"
        // metacharacter.
        (r"\\.", "\\."),
        // Backslash: four literal backslash chars in magic(5) source
        // (two escape pairs) resolve to two literal backslash bytes.
        (r"\\\\", "\\\\"),
        // Hex passthrough of a low (< 0x80) byte.
        (r"\x41", "A"),
        // >= 0x80 re-encoding (KTD3): octal `\377` = 0xFF, hex `\xFF` =
        // 0xFF -- both must produce the SAME regex-native `\xff` text
        // (lowercase, pinned), never a raw 0xFF byte in the `String`.
        (r"\377", "\\xff"),
        (r"\xFF", "\\xff"),
        // Resolved open question (control-char collision): getstr names
        // `\b` as backspace (0x08). Confirmed via `apprentice.c` (see
        // module docs) that this is a real named escape, not dropped.
        // Because the resolver appends the raw byte (not the two-char
        // `\b` escape *sequence*), the regex crate's own `\b`
        // word-boundary meaning is never triggered -- no `\xHH`
        // re-encoding is needed since 0x08 < 0x80.
        (r"\b", "\x08"),
        // Remaining named control escapes (getstr `case 'a'`/`'f'`/`'n'`/
        // `'r'`/`'v'`) completing the coverage `\t`/`\b` started above --
        // each is a distinct match arm in `resolve_getstr_escape`'s
        // `named_byte` table (getstr/mod.rs ~L155-164); a transposition
        // between any two of these bytes (e.g. `\n`<->`\r`) must fail its
        // own row rather than being masked by only testing two of seven.
        (r"\a", "\x07"),
        (r"\f", "\x0C"),
        (r"\n", "\n"),
        (r"\r", "\r"),
        (r"\v", "\x0B"),
    ];

    for &(input, expected) in cases {
        let (remaining, value) =
            parse_regex_getstr_value(input).unwrap_or_else(|e| panic!("{input:?}: {e:?}"));
        assert_eq!(remaining, "", "remaining input for {input:?}");
        assert_eq!(
            value,
            Value::String(expected.to_string()),
            "resolved text for input {input:?}"
        );
    }
}

#[test]
fn getstr_resolver_ge_0x80_bytes_are_valid_utf8_never_raw() {
    // Belt-and-suspenders on top of the table above: explicitly check
    // the resolved String's byte content never contains a raw 0xFF (or
    // any byte >= 0x80) and remains valid UTF-8 (guaranteed by the type,
    // but asserted here to make the invariant visible in the test).
    for input in [r"\377", r"\xFF", r"\x80", r"\200"] {
        let (_, value) = parse_regex_getstr_value(input).expect(input);
        let Value::String(s) = value else {
            panic!("expected Value::String for {input:?}");
        };
        assert!(
            s.bytes().all(|b| b < 0x80),
            "resolved text for {input:?} must be pure ASCII (the >= 0x80 byte is \
             re-encoded as \\xHH text, never appended raw): got {s:?}"
        );
        assert!(
            std::str::from_utf8(s.as_bytes()).is_ok(),
            "resolved text for {input:?} must be valid UTF-8"
        );
    }
}

#[test]
fn getstr_resolver_hex_escape_with_no_digits_falls_back_to_literal_x() {
    // getstr quirk: `\x` immediately followed by a non-hex character
    // leaves `val` at its initialized default, the literal character
    // `'x'` (0x78). Replicated here for fidelity even though it is a
    // corner case unlikely to appear in real magic files.
    let (remaining, value) = parse_regex_getstr_value(r"\xZZ").expect(r"\xZZ");
    assert_eq!(value, Value::String("xZZ".to_string()));
    assert_eq!(remaining, "");
}

#[test]
fn getstr_resolver_stops_at_unescaped_whitespace() {
    let (remaining, value) = parse_regex_getstr_value("foobar[0-9]+ trailing message").unwrap();
    assert_eq!(value, Value::String("foobar[0-9]+".to_string()));
    assert_eq!(remaining, " trailing message");
}

#[test]
fn getstr_resolver_rejects_empty_or_whitespace_only_input() {
    assert!(parse_regex_getstr_value("").is_err());
    assert!(parse_regex_getstr_value("   ").is_err());
}

/// R2/highest-value coverage gap: a pattern that resolves to NOTHING at
/// all -- a lone trailing backslash with no character following it (an
/// incomplete escape, getstr's `case '\0': ... goto out;`, getstr/mod.rs
/// ~L123-127) -- must hit the `resolved.is_empty()` -> `Err` path
/// (~L135-139), not silently succeed with an empty pattern. An empty
/// `Value::String("")` regex pattern would compile (the empty regex
/// matches everything, zero-width) and every rule using it would
/// "succeed" nonsensically instead of being rejected at parse time. This
/// is exactly the class of fatal-abort-turned-silent-wrong-answer bug
/// this PR exists to close: a graceful `Err` here is the correct,
/// desired outcome, not a bug.
#[test]
fn getstr_resolver_rejects_lone_trailing_backslash_incomplete_escape() {
    let result = parse_regex_getstr_value("\\");
    assert!(
        result.is_err(),
        "a lone trailing backslash with nothing following it must be rejected \
         (incomplete escape resolves to zero characters), got {result:?}"
    );
}

/// End-to-end guard for the same condition through the full
/// `parse_magic_rule` pipeline (`grammar/mod.rs` ~L1198-1201): when the
/// getstr resolver rejects a bareword regex pattern, `parse_magic_rule`
/// does NOT propagate that error unconditionally -- it retries via
/// `parse_value(input).map_err(|_| orig_err)?`, using `orig_err` (the
/// getstr failure) only if the fallback ALSO fails.
///
/// For a lone trailing backslash specifically, this test confirms
/// (empirically, not by assumption) that the fallback SUCCEEDS: a bare
/// `\` with nothing following is captured by `parse_value`'s
/// hex/mixed-ascii branch as a single raw byte (`Value::Bytes([0x5c])`),
/// exactly like the analogous `search` cross-wiring guard above
/// (`parse_magic_rule_search_with_same_escaped_pattern_is_unaffected`).
/// The overall pipeline is therefore graceful in the strongest sense --
/// it recovers to a well-formed `Ok` rule rather than needing to fall
/// back to an `Err` -- and, critically, it never panics.
#[test]
fn parse_magic_rule_regex_lone_trailing_backslash_pattern_recovers_via_value_fallback() {
    let input = "0 regex \\";
    let (remaining, rule) = parse_magic_rule(input)
        .expect("the parse_value fallback must recover a bare trailing backslash gracefully");
    assert_eq!(remaining, "");
    assert!(matches!(rule.typ, TypeKind::Regex { .. }));
    assert_eq!(
        rule.value,
        Value::Bytes(vec![0x5c]),
        "parse_value's hex/mixed-ascii fallback captures the lone backslash as a raw byte"
    );
}

/// A genuinely-Err round trip: when the value token is truly EMPTY (no
/// pattern at all follows the `regex` keyword -- not even a lone
/// backslash), BOTH the getstr resolver (`input.is_empty()` guard,
/// getstr/mod.rs ~L104) AND the `parse_value` fallback (`parse_value`'s
/// own explicit empty-input check, `grammar/value.rs` ~L366-371) reject
/// it, so `parse_magic_rule` has no recovery path and returns `Err`
/// gracefully -- never a panic.
///
/// Per GOTCHAS S3.11, `parse_text_magic_file` is fail-fast: in a real
/// magic file, a single rule like this would abort the WHOLE file load
/// rather than being skipped. This test only pins that the failure mode
/// at the single-rule parser level is a clean `Err`; it does not claim
/// any file-level skip-and-continue behavior exists (it does not).
#[test]
fn parse_magic_rule_regex_empty_pattern_errs_gracefully() {
    let input = "0 regex ";
    let result = parse_magic_rule(input);
    assert!(
        result.is_err(),
        "a regex rule with a completely empty value token must be rejected \
         gracefully (Err), never panic: got {result:?}"
    );
}

// ---------------------------------------------------------------------
// U3 variant-guard tests (via the full `parse_magic_rule` pipeline)
// ---------------------------------------------------------------------

#[test]
fn parse_magic_rule_regex_escaped_pattern_yields_string_not_bytes() {
    let input = r"0 regex \^[\040\t]{0,50}\\.asciiz assembler source text";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert!(matches!(rule.typ, TypeKind::Regex { .. }));
    assert_eq!(
        rule.value,
        Value::String("^[ \t]{0,50}\\.asciiz".to_string()),
        "regex pattern must resolve to Value::String, not Value::Bytes"
    );
    assert_eq!(rule.message, "assembler source text");
}

#[test]
fn parse_magic_rule_regex_non_escaped_pattern_still_value_string() {
    // No regression: a plain, non-escaped regex pattern must keep
    // parsing as `Value::String` exactly as before this fix.
    let input = "0 regex foobar[0-9]+ numeric suffix";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(rule.value, Value::String("foobar[0-9]+".to_string()));
    assert_eq!(rule.message, "numeric suffix");
}

#[test]
fn parse_magic_rule_regex_quoted_pattern_unaffected_by_getstr_routing() {
    // Quoted regex values are NOT routed through the getstr resolver --
    // they keep using `parse_quoted_string`'s own (different) escape
    // table, exactly as before. This guards against regressing the
    // pre-existing `test_parse_magic_rule_regex_and_search` coverage in
    // `grammar/tests/mod.rs`.
    let input = r#"0 regex/c "hello" case-insensitive match"#;
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(rule.value, Value::String("hello".to_string()));
}

#[test]
fn parse_magic_rule_search_with_same_escaped_pattern_drops_unknown_escape_backslash() {
    // The SAME escaped source text, used on a `search` rule instead of
    // `regex`, still goes through `parse_hex_bytes`/`parse_mixed_hex_ascii`
    // (not the getstr resolver, which is regex-only) and still yields
    // `Value::Bytes`. But `parse_mixed_hex_ascii`'s unrecognized-escape
    // fallback now matches GNU `file` getstr: the standalone backslash on
    // `\^` is DROPPED and `^` is kept literally, exactly as the `regex`
    // path resolves it (see
    // `parse_magic_rule_regex_escaped_pattern_yields_string_not_bytes`
    // above -- same bytes, different Value variant). `\040`->space and
    // `\t`->tab are recognized escapes (unchanged); `\\.`->`\.` is an
    // escaped backslash then a literal dot (unchanged). This is the
    // getstr-parity fix that also lets `0 string \<?xml\ version=` match
    // XML documents (the escaped space no longer truncates the token).
    let input = r"0 search/80 \^[\040\t]{0,50}\\.asciiz test";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert!(matches!(rule.typ, TypeKind::Search { .. }));
    match &rule.value {
        Value::Bytes(bytes) => {
            assert_eq!(
                bytes, b"^[ \t]{0,50}\\.asciiz",
                "unrecognized `\\^` escape drops the backslash (getstr parity), \
                 matching the regex path's resolved bytes"
            );
        }
        other => panic!("expected search to still produce Value::Bytes, got {other:?}"),
    }
    assert_eq!(rule.message, "test");
}

#[test]
fn assembler_asciiz_rule_matches_gnu_file_semantics_end_to_end() {
    // End-to-end sanity check against the real trigger rule from the
    // macOS-shipped system magic DB (`/usr/share/file/magic/assembler:5`):
    //
    //   0    regex    \^[\040\t]{0,50}\\.asciiz    assembler source text
    //
    // The source line is reproduced verbatim here (not read from the
    // filesystem) so this test is portable and does not depend on the
    // system magic DB being installed. It exercises the full pipeline:
    // parse -> getstr-resolve -> compile as `regex::bytes::Regex` ->
    // match/no-match against representative inputs (GNU `file`
    // semantics: an optional run of up to 50 leading spaces/tabs, then a
    // LITERAL `.asciiz`, anchored at line start).
    let line = "0\tregex\t\\^[\\040\\t]{0,50}\\\\.asciiz\t\tassembler source text";
    let (_, rule) = parse_magic_rule(line).expect(line);
    let Value::String(pattern) = &rule.value else {
        panic!("expected Value::String, got {:?}", rule.value);
    };

    let re = regex::bytes::Regex::new(pattern).expect("resolved pattern must compile");

    assert!(
        re.is_match(b"\t.asciiz \"hi\""),
        "leading tab then .asciiz must match"
    );
    assert!(
        re.is_match(b".asciiz \"hi\""),
        "zero leading whitespace (lower bound of {{0,50}}) must match"
    );
    assert!(
        !re.is_match(b"xyz .asciiz"),
        "non-whitespace before .asciiz must not match"
    );
    assert!(
        !re.is_match(b"Xasciiz"),
        "escaped dot must be a literal '.', not the any-character wildcard"
    );

    let too_much_padding = format!("{}{}", " ".repeat(51), ".asciiz");
    assert!(
        !re.is_match(too_much_padding.as_bytes()),
        "more than 50 leading whitespace chars must exceed the {{0,50}} upper bound"
    );
}

#[test]
fn parse_magic_rule_regex_with_ge_0x80_escape_does_not_abort_parse() {
    // R5/KTD3: an escape-produced byte >= 0x80 must not panic, corrupt
    // UTF-8, or fail the parse (which would abort the WHOLE fail-fast
    // file load per GOTCHAS S3.11 -- relocating the fatal-abort class
    // this fix kills from evaluator to parser).
    let input = r"0 regex \377 high byte octal escape";
    let (remaining, rule) = parse_magic_rule(input).expect(input);
    assert_eq!(remaining, "");
    assert_eq!(rule.value, Value::String("\\xff".to_string()));

    let input_hex = r"0 regex \xFF high byte hex escape";
    let (remaining, rule) = parse_magic_rule(input_hex).expect(input_hex);
    assert_eq!(remaining, "");
    assert_eq!(rule.value, Value::String("\\xff".to_string()));
}
