// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Regular-expression matching for magic rule evaluation.
//!
//! Implements the `regex` `TypeKind` using `regex::bytes::RegexBuilder` so that
//! matching is binary-safe (patterns are applied to the raw byte buffer, not
//! a UTF-8 string). A successful match returns `Ok(Some(Value::String(...)))`
//! -- the matched bytes, with invalid UTF-8 replaced via `from_utf8_lossy`.
//! A miss returns `Ok(None)`. The `Option` is the structured "no match"
//! signal, which lets the engine distinguish a legitimate zero-width match
//! (e.g., `^`, `a*`, lookaheads) from a genuine miss -- both of which would
//! otherwise collapse to `Value::String(String::new())`.

use super::TypeReadError;
use crate::parser::ast::Value;
use regex::bytes::{Regex, RegexBuilder};

/// Compile `pattern` with the magic-rule regex flags applied.
///
/// When `start_of_line` is true, the pattern is wrapped in `^(?:...)` so that
/// matches must occur at the start of a line (combined with multi-line mode,
/// `^` anchors to the beginning of any line). The original pattern is placed
/// inside a non-capturing group so any internal anchors, alternations, or
/// backreferences continue to behave correctly after wrapping.
fn build_regex(
    pattern: &str,
    case_insensitive: bool,
    start_of_line: bool,
) -> Result<Regex, regex::Error> {
    let owned;
    let effective_pattern: &str = if start_of_line {
        owned = format!("^(?:{pattern})");
        &owned
    } else {
        pattern
    };
    RegexBuilder::new(effective_pattern)
        .case_insensitive(case_insensitive)
        .multi_line(start_of_line)
        .build()
}

/// Scan `buffer` starting at `offset` for the first match of `pattern`.
///
/// # Arguments
///
/// * `buffer` - File buffer to scan
/// * `offset` - Starting position within the buffer
/// * `pattern` - Regex source string (from the rule's `Value::String` operand)
/// * `case_insensitive` - Enable case-insensitive matching (`/c` flag)
/// * `start_of_line` - Anchor matches to line starts (`/l` flag). When true,
///   the pattern is wrapped in multi-line mode with a `^` anchor so it only
///   matches at the start of a line.
///
/// # Returns
///
/// * `Ok(Some(Value::String(matched_text)))` on a successful match -- invalid
///   UTF-8 in the matched bytes is replaced with U+FFFD via
///   `from_utf8_lossy`. The matched text may legitimately be empty for
///   zero-width matches (e.g., `^`, `a*`, or lookaheads).
/// * `Ok(None)` when the pattern does not match anywhere in the remaining
///   buffer.
///
/// # Errors
///
/// * `TypeReadError::BufferOverrun` if `offset >= buffer.len()`.
/// * `TypeReadError::UnsupportedType` if `pattern` fails to compile as a
///   regex (the error variant is reused to avoid adding a new enum variant;
///   the `type_name` field carries the compilation error message).
pub fn read_regex(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    case_insensitive: bool,
    start_of_line: bool,
) -> Result<Option<Value>, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let regex = build_regex(pattern, case_insensitive, start_of_line).map_err(|e| {
        TypeReadError::UnsupportedType {
            type_name: format!("regex compile error: {e}"),
        }
    })?;

    let remaining = &buffer[offset..];

    Ok(regex
        .find(remaining)
        .map(|m| Value::String(String::from_utf8_lossy(m.as_bytes()).into_owned())))
}

/// Re-run `pattern` against `buffer` at `offset` and return the end offset
/// of the first match, relative to `offset` (i.e., the number of bytes that
/// should be added to the GNU `file` previous-match anchor).
///
/// Returns `0` on any failure -- offset past buffer end, invalid pattern, or
/// no match. The infallible contract matches the rest of `bytes_consumed`:
/// the engine only calls this after a successful read, so the defensive
/// paths are belt-and-braces for misuse.
///
/// Note: the regex is compiled twice per successful match -- once in
/// `read_regex` and again here. Caching the compiled `Regex` would require
/// threading it through `TypeReadError`/`Value` or adding a second return
/// channel, both of which complicate the reader API for a micro-optimization.
/// The duplicated compile is a deliberate simplicity-over-caching trade-off.
#[must_use]
pub(super) fn regex_bytes_consumed(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    case_insensitive: bool,
    start_of_line: bool,
) -> usize {
    let Some(remaining) = buffer.get(offset..) else {
        debug_assert!(
            false,
            "regex_bytes_consumed: offset {offset} > buffer.len() {} -- engine invariant violated (called without a preceding successful read_regex)",
            buffer.len()
        );
        return 0;
    };
    let Ok(regex) = build_regex(pattern, case_insensitive, start_of_line) else {
        debug_assert!(
            false,
            "regex_bytes_consumed: failed to re-compile pattern {pattern:?} -- engine invariant violated (read_regex already succeeded)"
        );
        return 0;
    };
    regex.find(remaining).map_or(0, |m| m.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_regex_basic_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "World", false, false).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_no_match_returns_none() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "xyz", false, false).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_case_insensitive() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", true, false).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_case_sensitive_no_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", false, false).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_start_of_line() {
        let buffer = b"first line\nsecond line";
        let result = read_regex(buffer, 0, "^second", false, true).unwrap();
        assert_eq!(result, Some(Value::String("second".to_string())));
    }

    #[test]
    fn test_read_regex_start_of_line_cases() {
        // With `/l` enabled, a bare (unanchored) pattern must still be
        // anchored to a line start: `"line"` appears mid-line on both
        // lines so it misses, while `"second"` occurs at a line start so
        // it matches. The `regex_bytes_consumed` check mirrors the
        // mid-line miss to verify the anchor helper stays put.
        let buffer = b"first line\nsecond line";
        let cases: &[(&str, Option<Value>)] = &[
            ("line", None),
            ("second", Some(Value::String("second".to_string()))),
        ];
        for (pattern, expected) in cases {
            let result = read_regex(buffer, 0, pattern, false, true).unwrap();
            assert_eq!(&result, expected, "pattern {pattern:?}");
        }
        assert_eq!(regex_bytes_consumed(buffer, 0, "line", false, true), 0);
    }

    #[test]
    fn test_read_regex_start_of_line_no_anchor_match() {
        // Without multi_line, ^ only matches buffer start
        let buffer = b"first line\nsecond line";
        let result = read_regex(buffer, 0, "^second", false, false).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_zero_width_start_anchor_matches() {
        // `^` matches zero-width at position 0. The fix for C3 requires
        // this to be reported as `Some(Value::String(""))`, not `None`.
        // Previously collapsed to "no match" because the engine checked
        // `!s.is_empty()`.
        let buffer = b"hello";
        let result = read_regex(buffer, 0, "^", false, false).unwrap();
        assert_eq!(
            result,
            Some(Value::String(String::new())),
            "^ is a legitimate zero-width match, not a miss"
        );
    }

    #[test]
    fn test_read_regex_zero_width_star_matches_empty() {
        // `a*` matches an empty span at the first non-'a' byte (or
        // position 0 if there are no 'a' bytes). Same regression guard as
        // the anchor case -- a legitimate zero-width match must not
        // collapse to None.
        let buffer = b"xyz";
        let result = read_regex(buffer, 0, "a*", false, false).unwrap();
        assert_eq!(result, Some(Value::String(String::new())));
    }

    #[test]
    fn test_read_regex_at_offset() {
        let buffer = b"prefix_World!";
        let result = read_regex(buffer, 7, "World", false, false).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_offset_past_end() {
        let buffer = b"Hello";
        let result = read_regex(buffer, 10, "x", false, false);
        assert!(matches!(
            result,
            Err(TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 5
            })
        ));
    }

    #[test]
    fn test_read_regex_invalid_pattern() {
        let buffer = b"Hello";
        let result = read_regex(buffer, 0, "[unclosed", false, false);
        assert!(matches!(result, Err(TypeReadError::UnsupportedType { .. })));
    }

    #[test]
    fn test_read_regex_binary_safe() {
        let buffer = &[0x00, 0xff, 0xfe, 0x41, 0x42, 0x43];
        let result = read_regex(buffer, 0, "ABC", false, false).unwrap();
        assert_eq!(result, Some(Value::String("ABC".to_string())));
    }

    #[test]
    fn test_read_regex_character_class() {
        let buffer = b"abc123def";
        let result = read_regex(buffer, 0, "[0-9]+", false, false).unwrap();
        assert_eq!(result, Some(Value::String("123".to_string())));
    }

    #[test]
    fn test_regex_bytes_consumed_match() {
        let buffer = b"Hello, World!";
        assert_eq!(regex_bytes_consumed(buffer, 0, "World", false, false), 12);
    }

    #[test]
    fn test_regex_bytes_consumed_no_match() {
        let buffer = b"Hello";
        assert_eq!(regex_bytes_consumed(buffer, 0, "xyz", false, false), 0);
    }

    #[test]
    fn test_regex_bytes_consumed_zero_width_match_returns_zero() {
        // A zero-width match at position 0 means match_end == 0, so the
        // anchor should not advance. Regression guard for the interaction
        // between C3 fix and bytes_consumed.
        let buffer = b"hello";
        assert_eq!(regex_bytes_consumed(buffer, 0, "^", false, false), 0);
    }
}
