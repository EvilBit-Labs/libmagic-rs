// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Regular-expression matching for magic rule evaluation.
//!
//! Implements the `regex` `TypeKind` using `regex::bytes::RegexBuilder` so
//! that matching is binary-safe (patterns are applied to the raw byte
//! buffer, not a UTF-8 string). A successful match returns
//! `Ok(Some(Value::String(...)))` -- the matched bytes with invalid UTF-8
//! replaced via `from_utf8_lossy`. A miss returns `Ok(None)`. The `Option`
//! is the structured "no match" signal, which lets the engine distinguish
//! a legitimate zero-width match (e.g., `^`, `a*`, lookaheads) from a
//! genuine miss -- both of which would otherwise collapse to
//! `Value::String(String::new())`.
//!
//! ## Semantics (matching GNU `file`)
//!
//! * **Multi-line mode is always on.** GNU `file`'s `alloc_regex` in
//!   `src/softmagic.c` compiles every regex with `REG_NEWLINE`
//!   unconditionally, so `^` and `$` match at line boundaries and `.`
//!   does not match `\n`. The `/l` flag does **not** control this; it
//!   controls whether the scan window is measured in bytes or lines.
//!
//! * **Scan window is always capped at [`REGEX_MAX_BYTES`] (8192).** This
//!   matches libmagic's `FILE_REGEX_MAX` constant. An explicit `count`
//!   larger than 8192 is clamped. An implicit count (no user-supplied
//!   value) uses the 8192 default directly.
//!
//! * **Line-based window** (`/l` flag): when `flags.line_based` is set,
//!   `count` is a line count. The scan window extends from `offset`
//!   through the end of the Nth line terminator, capped at 8192 bytes.
//!   Libmagic recognizes both `\n` (LF) and `\r\n` (CR+LF) as terminators
//!   and counts them as single lines; this implementation uses the same
//!   semantics via `memchr::memchr2(b'\n', b'\r', ...)`.
//!
//! * **`/s` flag** (`start_offset`): affects only the anchor advance
//!   computed by [`regex_bytes_consumed`]. When set, the anchor moves by
//!   `m.start()` (match-start) instead of `m.end()` (match-end), matching
//!   libmagic's `REGEX_OFFSET_START` / `moffset()` logic.

use super::TypeReadError;
use crate::parser::ast::{REGEX_MAX_BYTES, RegexFlags, Value};
use regex::bytes::{Regex, RegexBuilder};
use std::num::NonZeroU32;

/// Compile `pattern` with the magic-rule regex flags applied.
///
/// Multi-line mode is always enabled (unconditional in libmagic via
/// `REG_NEWLINE`) and `.` does not match newlines. The `case_insensitive`
/// flag is the only compile-time flag the magic-rule interface controls;
/// `line_based` and `start_offset` affect window computation and anchor
/// advance respectively, not regex compilation.
fn build_regex(pattern: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(true)
        .dot_matches_new_line(false)
        .build()
}

/// Compute the scan window for a regex rule at `offset`, applying the
/// 8192-byte cap and the `/l` line-count semantics when requested.
///
/// Returns a slice of `buffer` starting at `offset`:
///
/// * **Byte mode** (`flags.line_based == false`): window length is
///   `min(count.unwrap_or(REGEX_MAX_BYTES), REGEX_MAX_BYTES, remaining)`.
///
/// * **Line mode** (`flags.line_based == true`): window extends from
///   `offset` through the end of the Nth line terminator (inclusive),
///   where N is `count.unwrap_or(u32::MAX)`. `\r\n` and `\n` both count as
///   one line terminator. If the Nth terminator is not found within
///   `REGEX_MAX_BYTES`, the window is truncated to 8192 bytes. If `count`
///   is `None` and no terminator is found at all, the window is the whole
///   buffer tail up to the 8192-byte cap.
fn compute_window(
    buffer: &[u8],
    offset: usize,
    flags: RegexFlags,
    count: Option<NonZeroU32>,
) -> &[u8] {
    let Some(remaining) = buffer.get(offset..) else {
        return &[];
    };
    let byte_cap = remaining.len().min(REGEX_MAX_BYTES);
    let capped = &remaining[..byte_cap];

    if !flags.line_based {
        let count_bytes =
            count.map_or(REGEX_MAX_BYTES, |n| (n.get() as usize).min(REGEX_MAX_BYTES));
        return &capped[..count_bytes.min(capped.len())];
    }

    // Line mode: walk the byte-capped slice counting `\n` (and `\r\n`
    // pairs as one terminator), stopping after the Nth terminator.
    let target_lines = count.map_or(u32::MAX, NonZeroU32::get);
    let mut lines_seen: u32 = 0;
    let mut idx = 0usize;
    while idx < capped.len() {
        match capped[idx] {
            b'\r' => {
                // Treat CR and CRLF as a single terminator.
                let advance = if idx + 1 < capped.len() && capped[idx + 1] == b'\n' {
                    2
                } else {
                    1
                };
                idx += advance;
                lines_seen = lines_seen.saturating_add(1);
            }
            b'\n' => {
                idx += 1;
                lines_seen = lines_seen.saturating_add(1);
            }
            _ => idx += 1,
        }
        if lines_seen >= target_lines {
            break;
        }
    }
    &capped[..idx]
}

/// Scan `buffer` starting at `offset` for the first match of `pattern`.
///
/// # Arguments
///
/// * `buffer` - File buffer to scan
/// * `offset` - Starting position within the buffer
/// * `pattern` - Regex source string (from the rule's `Value::String`
///   operand)
/// * `flags` - Regex modifier flags parsed from the `/[csl]` suffix
/// * `count` - Optional numeric count. Interpretation depends on
///   `flags.line_based`; see [`compute_window`] for the details.
///
/// # Returns
///
/// * `Ok(Some(Value::String(matched_text)))` on a successful match --
///   invalid UTF-8 in the matched bytes is replaced with U+FFFD via
///   `from_utf8_lossy`. The matched text may legitimately be empty for
///   zero-width matches (e.g., `^`, `a*`, or lookaheads).
/// * `Ok(None)` when the pattern does not match anywhere in the scan
///   window.
///
/// # Errors
///
/// * `TypeReadError::BufferOverrun` if `offset >= buffer.len()`.
/// * `TypeReadError::UnsupportedType` if `pattern` fails to compile as a
///   regex (the error variant is reused to avoid adding a new enum
///   variant; the `type_name` field carries the compilation error
///   message).
pub fn read_regex(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    flags: RegexFlags,
    count: Option<NonZeroU32>,
) -> Result<Option<Value>, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let regex = build_regex(pattern, flags.case_insensitive).map_err(|e| {
        TypeReadError::UnsupportedType {
            type_name: format!("regex compile error: {e}"),
        }
    })?;

    let window = compute_window(buffer, offset, flags, count);

    Ok(regex
        .find(window)
        .map(|m| Value::String(String::from_utf8_lossy(m.as_bytes()).into_owned())))
}

/// Re-run `pattern` against `buffer` at `offset` and return the anchor
/// advance for the first match (number of bytes to add to the GNU `file`
/// previous-match anchor).
///
/// When `flags.start_offset` is set (the `/s` modifier), the advance is
/// `m.start()` (match-start). Otherwise the advance is `m.end()`
/// (match-end). This matches libmagic's `REGEX_OFFSET_START` / `moffset()`
/// branch in `src/softmagic.c`.
///
/// Returns `0` on any failure -- offset past buffer end, invalid pattern,
/// or no match. The `debug_assert` guards catch engine-invariant
/// violations (i.e., calls without a preceding successful `read_regex`) in
/// dev/test builds.
///
/// Note: the regex is compiled twice per successful match -- once in
/// `read_regex` and again here. Caching the compiled `Regex` would require
/// threading it through `TypeReadError`/`Value` or adding a second return
/// channel, both of which complicate the reader API for a micro-
/// optimization. The duplicated compile is a deliberate simplicity-over-
/// caching trade-off.
#[must_use]
pub(super) fn regex_bytes_consumed(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    flags: RegexFlags,
    count: Option<NonZeroU32>,
) -> usize {
    if buffer.get(offset..).is_none() {
        debug_assert!(
            false,
            "regex_bytes_consumed: offset {offset} > buffer.len() {} -- engine invariant violated (called without a preceding successful read_regex)",
            buffer.len()
        );
        return 0;
    }
    let Ok(regex) = build_regex(pattern, flags.case_insensitive) else {
        debug_assert!(
            false,
            "regex_bytes_consumed: failed to re-compile pattern {pattern:?} -- engine invariant violated (read_regex already succeeded)"
        );
        return 0;
    };
    let window = compute_window(buffer, offset, flags, count);
    regex.find(window).map_or(0, |m| {
        if flags.start_offset {
            m.start()
        } else {
            m.end()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_flags() -> RegexFlags {
        RegexFlags::default()
    }

    fn flags(case: bool, start: bool, line: bool) -> RegexFlags {
        RegexFlags {
            case_insensitive: case,
            start_offset: start,
            line_based: line,
        }
    }

    #[test]
    fn test_read_regex_basic_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "World", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_no_match_returns_none() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "xyz", no_flags(), None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_case_insensitive() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", flags(true, false, false), None).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_case_sensitive_no_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", no_flags(), None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_multiline_anchor_across_lines() {
        // libmagic always compiles regexes with REG_NEWLINE, so `^` and
        // `$` match at internal line boundaries regardless of the `/l`
        // flag. This test pins the behavior: `^second` on a two-line
        // buffer matches the second line even with no flags set.
        let buffer = b"first line\nsecond line";
        let result = read_regex(buffer, 0, "^second", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("second".to_string())));
    }

    #[test]
    fn test_read_regex_dot_does_not_match_newline() {
        // The REG_NEWLINE flag also makes `.` stop at newlines. A `.+`
        // match against a multi-line buffer must not consume the `\n`.
        let buffer = b"first\nsecond";
        let result = read_regex(buffer, 0, ".+", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("first".to_string())));
    }

    #[test]
    fn test_read_regex_zero_width_start_anchor_matches() {
        // `^` matches zero-width at position 0. Must be reported as
        // `Some(Value::String(""))`, not `None`. Regression guard for C3.
        let buffer = b"hello";
        let result = read_regex(buffer, 0, "^", no_flags(), None).unwrap();
        assert_eq!(
            result,
            Some(Value::String(String::new())),
            "^ is a legitimate zero-width match, not a miss"
        );
    }

    #[test]
    fn test_read_regex_zero_width_star_matches_empty() {
        let buffer = b"xyz";
        let result = read_regex(buffer, 0, "a*", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String(String::new())));
    }

    #[test]
    fn test_read_regex_at_offset() {
        let buffer = b"prefix_World!";
        let result = read_regex(buffer, 7, "World", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_offset_past_end() {
        let buffer = b"Hello";
        let result = read_regex(buffer, 10, "x", no_flags(), None);
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
        let result = read_regex(buffer, 0, "[unclosed", no_flags(), None);
        assert!(matches!(result, Err(TypeReadError::UnsupportedType { .. })));
    }

    #[test]
    fn test_read_regex_binary_safe() {
        let buffer = &[0x00, 0xff, 0xfe, 0x41, 0x42, 0x43];
        let result = read_regex(buffer, 0, "ABC", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("ABC".to_string())));
    }

    #[test]
    fn test_read_regex_character_class() {
        let buffer = b"abc123def";
        let result = read_regex(buffer, 0, "[0-9]+", no_flags(), None).unwrap();
        assert_eq!(result, Some(Value::String("123".to_string())));
    }

    // ------- V1: line-based window -------

    #[test]
    fn test_read_regex_line_based_one_line_caps_scan() {
        // `regex/1l` with a pattern that appears on the second line must
        // miss -- the scan window stops after the first newline.
        let buffer = b"first line\nsecond line\n";
        let one = NonZeroU32::new(1);
        let result = read_regex(buffer, 0, "second", flags(false, false, true), one).unwrap();
        assert_eq!(result, None, "scan should stop after the first line");
    }

    #[test]
    fn test_read_regex_line_based_crlf_terminator() {
        // CRLF (`\r\n`) counts as a single line terminator, matching
        // libmagic's `memchr2('\n', '\r', ...)` logic.
        let buffer = b"line1\r\nline2\r\n";
        let one = NonZeroU32::new(1);
        let second = read_regex(buffer, 0, "line2", flags(false, false, true), one).unwrap();
        assert_eq!(second, None, "CRLF should end the first line");
    }

    #[test]
    fn test_read_regex_line_based_counts_multiple_lines() {
        // `regex/3l` scans up to the third line, so a pattern on line 3
        // matches, but a pattern on line 4 misses.
        let buffer = b"line1\nline2\nline3\nline4\n";
        let three = NonZeroU32::new(3);
        let line3 = read_regex(buffer, 0, "line3", flags(false, false, true), three).unwrap();
        assert_eq!(line3, Some(Value::String("line3".to_string())));

        let line4 = read_regex(buffer, 0, "line4", flags(false, false, true), three).unwrap();
        assert_eq!(line4, None, "line4 is beyond the 3-line window");
    }

    // ------- V5: 8192-byte default cap -------

    #[test]
    fn test_read_regex_default_window_caps_at_8192_bytes() {
        // A buffer larger than 8192 bytes with the pattern past 8192
        // must miss on an un-counted regex, because the scan window is
        // capped at 8192 (FILE_REGEX_MAX).
        let mut buffer = vec![b'a'; 9000];
        buffer.extend_from_slice(b"needle");
        let result = read_regex(&buffer, 0, "needle", no_flags(), None).unwrap();
        assert_eq!(
            result, None,
            "needle past byte 9000 must not match under the 8192 default cap"
        );
    }

    #[test]
    fn test_read_regex_explicit_count_larger_than_cap_still_capped() {
        // Even an explicit `regex/100000` is clamped to 8192 bytes --
        // users cannot opt out of the hard cap.
        let mut buffer = vec![b'a'; 9000];
        buffer.extend_from_slice(b"needle");
        let hundred_thousand = NonZeroU32::new(100_000);
        let result = read_regex(&buffer, 0, "needle", no_flags(), hundred_thousand).unwrap();
        assert_eq!(result, None, "explicit count must still be clamped to 8192");
    }

    #[test]
    fn test_read_regex_small_count_honored() {
        // A small explicit count (e.g., 10 bytes) must be honored -- a
        // pattern past byte 10 misses.
        let buffer = b"abcdefghij_needle_here";
        let ten = NonZeroU32::new(10);
        let result = read_regex(buffer, 0, "needle", no_flags(), ten).unwrap();
        assert_eq!(result, None);
    }

    // ------- regex_bytes_consumed -------

    #[test]
    fn test_regex_bytes_consumed_match_end_by_default() {
        let buffer = b"Hello, World!";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "World", no_flags(), None),
            12
        );
    }

    #[test]
    fn test_regex_bytes_consumed_no_match() {
        let buffer = b"Hello";
        assert_eq!(regex_bytes_consumed(buffer, 0, "xyz", no_flags(), None), 0);
    }

    #[test]
    fn test_regex_bytes_consumed_zero_width_match_returns_zero() {
        let buffer = b"hello";
        assert_eq!(regex_bytes_consumed(buffer, 0, "^", no_flags(), None), 0);
    }

    // ------- V2: /s flag (start_offset) -------

    #[test]
    fn test_regex_bytes_consumed_start_offset_returns_match_start() {
        // Buffer: "abcWorld", pattern "World" matches at index 3, length
        // 5. Without `/s` the anchor advances by 8 (match-end). With `/s`
        // it advances by 3 (match-start), matching libmagic's
        // REGEX_OFFSET_START / moffset() zero-length path.
        let buffer = b"abcWorld";
        let match_end = regex_bytes_consumed(buffer, 0, "World", no_flags(), None);
        let match_start = regex_bytes_consumed(buffer, 0, "World", flags(false, true, false), None);
        assert_eq!(match_end, 8, "default anchor advance is match-end");
        assert_eq!(
            match_start, 3,
            "/s flag advances anchor to match-start instead"
        );
    }

    #[test]
    fn test_regex_bytes_consumed_start_offset_no_match_returns_zero() {
        // /s flag on a non-matching pattern still returns 0 (no advance).
        let buffer = b"Hello";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "xyz", flags(false, true, false), None),
            0
        );
    }
}
