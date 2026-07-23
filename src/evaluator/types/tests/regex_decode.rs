// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! fix-system-magic-regex-graceful, U1: `Value::Bytes` backstop for
//! `TypeKind::Regex`.
//!
//! The parser can currently miscategorize an escape-heavy regex pattern
//! (e.g. `\^[\040\t]{0,50}\\.asciiz`) as `Value::Bytes` instead of
//! `Value::String` (see `parse_value`'s hex/mixed-ascii branch). Before this
//! fix, both `read_typed_value_with_pattern` and `read_pattern_match`
//! rejected `Value::Bytes` regex patterns with `UnsupportedType`, unlike the
//! sibling `TypeKind::Search` arms which already accepted both variants
//! (GOTCHAS S2.4). See docs/plans/2026-07-17-001-fix-system-magic-regex-
//! graceful-plan.md.
//!
//! Also covers the `decode_regex_bytes_pattern` warn!-on-real-substitution
//! contract (KTD6), pinned via `testing_logger` log capture.

use super::*;

fn regex_type() -> TypeKind {
    TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    }
}

/// Happy path (regression guard): a `Value::String` pattern still matches
/// through `read_pattern_match`, unaffected by the new `Bytes` arm.
#[test]
fn test_read_pattern_match_regex_string_pattern_still_matches() {
    let typ = regex_type();
    let pattern = Value::String("foobar[0-9]+".to_string());
    let result = read_pattern_match(b"prefix foobar123 suffix", 0, &typ, Some(&pattern), 8192)
        .expect("read_pattern_match should not error for a valid String pattern");
    assert!(
        matches!(result, Some(Value::String(ref s)) if s == "foobar123"),
        "expected a match on the String pattern, got {result:?}"
    );
}

/// A `Value::Bytes` regex pattern (the miscategorized-escape case) must be
/// accepted by `read_pattern_match`, not rejected as `UnsupportedType`.
#[test]
fn test_read_pattern_match_regex_accepts_bytes_pattern() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"^[ \t]*\\.asciiz".to_vec());
    let result = read_pattern_match(b"\t.asciiz \"hi\"", 0, &typ, Some(&pattern), 8192)
        .expect("read_pattern_match must accept a Value::Bytes regex pattern, not UnsupportedType");
    assert!(
        result.is_some(),
        "expected the Bytes pattern to match the leading-whitespace buffer, got {result:?}"
    );
}

/// The same `Value::Bytes` acceptance must hold for
/// `read_typed_value_with_pattern` (the non-engine dispatch entry point),
/// mirroring the `read_pattern_match` arm exactly.
#[test]
fn test_read_typed_value_with_pattern_regex_accepts_bytes_pattern() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"[0-9]+".to_vec());
    let result = read_typed_value_with_pattern(b"abc123def", 0, &typ, Some(&pattern), 8192)
        .expect("read_typed_value_with_pattern must accept a Value::Bytes regex pattern");
    assert_eq!(
        result,
        Value::String("123".to_string()),
        "expected the matched digits, got {result:?}"
    );
}

/// Zero-width match contract (GOTCHAS S2.5) must be preserved for a
/// `Value::Bytes` pattern: `^` matches at position 0 with an empty capture,
/// which is `Ok(Some(Value::String("")))`, distinct from a genuine miss.
#[test]
fn test_read_pattern_match_regex_bytes_zero_width_match() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"^".to_vec());
    let result = read_pattern_match(b"hello", 0, &typ, Some(&pattern), 8192)
        .expect("zero-width Bytes pattern should not error");
    assert_eq!(
        result,
        Some(Value::String(String::new())),
        "zero-width match must be Some(empty string), not None (GOTCHAS S2.5)"
    );
}

/// A `Value::Bytes` pattern that does not match the buffer must produce a
/// genuine miss (`Ok(None)`), not an error.
#[test]
fn test_read_pattern_match_regex_bytes_pattern_miss() {
    let typ = regex_type();
    let pattern = Value::Bytes(b"xyz".to_vec());
    let result =
        read_pattern_match(b"abcdef", 0, &typ, Some(&pattern), 8192).expect("miss is not an error");
    assert_eq!(result, None, "non-matching Bytes pattern must be Ok(None)");
}

/// KTD6: a `Value::Bytes` regex pattern containing a byte that is not
/// valid UTF-8 must not panic. `String::from_utf8_lossy` substitutes
/// U+FFFD for the invalid byte before compiling -- this test pins the
/// no-panic / graceful-result contract; the `warn!` emission itself is
/// asserted separately via the `testing_logger`-based tests further down
/// this module (see `format_logs` and its callers).
#[test]
fn test_read_pattern_match_regex_bytes_invalid_utf8_does_not_panic() {
    let typ = regex_type();
    // 0xFF is never valid UTF-8 in any position.
    let pattern = Value::Bytes(vec![0xFF, b'a']);
    let result = read_pattern_match(b"\xEF\xBF\xBDa tail", 0, &typ, Some(&pattern), 8192);
    // Whether this happens to match the lossily-substituted U+FFFD encoding
    // or not is incidental; the load-bearing assertion is that decoding an
    // invalid-UTF-8 Bytes pattern never panics and always yields a valid
    // Result.
    assert!(
        result.is_ok(),
        "invalid-UTF-8 Bytes pattern must not error, got {result:?}"
    );
}

/// Missing pattern (`None`) must still be a hard `UnsupportedType` error in
/// both dispatch functions -- U2's engine-level graceful skip depends on
/// this remaining an `Err`, not silently becoming a non-match here.
#[test]
fn test_regex_missing_pattern_still_errors_in_both_dispatch_fns() {
    let typ = regex_type();

    let pattern_match_result = read_pattern_match(b"abc", 0, &typ, None, 8192);
    assert!(
        matches!(
            pattern_match_result,
            Err(TypeReadError::MissingPatternOperand { ref type_name }) if type_name == "regex without string pattern"
        ),
        "read_pattern_match with no pattern must still error, got {pattern_match_result:?}"
    );

    let typed_value_result = read_typed_value_with_pattern(b"abc", 0, &typ, None, 8192);
    assert!(
        matches!(
            typed_value_result,
            Err(TypeReadError::MissingPatternOperand { ref type_name }) if type_name == "regex without string pattern"
        ),
        "read_typed_value_with_pattern with no pattern must still error, got {typed_value_result:?}"
    );
}

// -----------------------------------------------------------------------
// H hardening: pin the `decode_regex_bytes_pattern` warn!-on-real-
// substitution contract (KTD6) with a real log-capture seam
// (`testing_logger`), rather than code inspection only.
// -----------------------------------------------------------------------

/// Test-only helper: `testing_logger::CapturedLog` does not implement
/// `Debug`, so format captured logs manually for failure messages.
fn format_logs(logs: &[testing_logger::CapturedLog]) -> String {
    logs.iter()
        .map(|l| format!("{:?}: {}", l.level, l.body))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A `Value::Bytes` regex pattern containing a byte `>= 0x80` that is not
/// valid UTF-8 triggers a real lossy substitution (`from_utf8_lossy`
/// replaces it with U+FFFD); `decode_regex_bytes_pattern` must `warn!`
/// because the compiled regex now silently diverges from the raw bytes
/// the target buffer is matched against.
#[test]
fn decode_regex_bytes_pattern_warns_on_real_utf8_substitution() {
    testing_logger::setup();
    let decoded = decode_regex_bytes_pattern(&[0xFF, b'a']);
    // Sanity: the function is still infallible and produces SOME string
    // (the lossy replacement), never panics.
    assert!(decoded.contains('a'));
    testing_logger::validate(|captured_logs| {
        let warn_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("not valid UTF-8"))
            .collect();
        assert_eq!(
            warn_logs.len(),
            1,
            "expected exactly one lossy-substitution warning, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            warn_logs[0].level,
            log::Level::Warn,
            "lossy UTF-8 substitution must log at warn!, not another level -- got {:?}",
            warn_logs[0].level
        );
    });
}

/// The converse: valid-UTF-8 bytes must NOT trigger the substitution
/// warning at all -- the guard is keyed on `str::from_utf8` actually
/// failing, not merely on the input being `Value::Bytes`.
#[test]
fn decode_regex_bytes_pattern_does_not_warn_on_valid_utf8() {
    testing_logger::setup();
    let decoded = decode_regex_bytes_pattern(b"hello[0-9]+");
    assert_eq!(decoded, "hello[0-9]+");
    testing_logger::validate(|captured_logs| {
        assert!(
            captured_logs.is_empty(),
            "valid UTF-8 bytes must not trigger any log entry, got {:?}",
            format_logs(captured_logs)
        );
    });
}
