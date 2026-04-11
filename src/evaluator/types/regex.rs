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
//! a legitimate zero-width match (e.g., `^`, `a*`, or `.{0}`) from a
//! genuine miss -- both of which would otherwise collapse to
//! `Value::String(String::new())`. (Note: the Rust `regex` crate does
//! not support look-around assertions; those are excluded for
//! linear-time matching guarantees.)
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
//! * **Line-based window** ([`RegexCount::Lines`]): when `count` is
//!   [`RegexCount::Lines(Some(n))`], the scan window extends from
//!   `offset` through the end of the Nth line terminator, capped at
//!   8192 bytes. [`RegexCount::Lines(None)`] (the `regex/l` shorthand)
//!   walks terminators to the end of the capped window and is
//!   behaviorally equivalent to [`RegexCount::Default`]. Following GNU
//!   `file`'s `softmagic.c` line-counting loop, three terminator
//!   sequences each count as a single line: LF (`\n`), CRLF (`\r\n`,
//!   consumed as one terminator), and bare CR (`\r`, for classic Mac
//!   line endings).
//!
//! [`RegexCount::Lines`]: crate::parser::ast::RegexCount::Lines
//! [`RegexCount::Lines(Some(n))`]: crate::parser::ast::RegexCount::Lines
//! [`RegexCount::Lines(None)`]: crate::parser::ast::RegexCount::Lines
//! [`RegexCount::Default`]: crate::parser::ast::RegexCount::Default
//!
//! * **`/s` flag** (`start_offset`): affects only the anchor advance
//!   computed by [`regex_bytes_consumed`]. When set, the anchor moves by
//!   `m.start()` (match-start) instead of `m.end()` (match-end), matching
//!   libmagic's `REGEX_OFFSET_START` / `moffset()` logic.

use super::TypeReadError;
use crate::parser::ast::{RegexFlags, Value};
use regex::bytes::{Regex, RegexBuilder};

/// The hard upper bound on regex scan window size, matching GNU `file`'s
/// `FILE_REGEX_MAX` constant in `src/file.h`. Any regex rule -- including
/// ones with explicit counts larger than this -- is capped at this many
/// bytes to prevent runaway scans against large buffers.
///
/// This constant lives in the evaluator module because it is runtime
/// evaluation policy, not AST shape. Putting it in `parser::ast` would
/// couple the build-script compilation unit to an evaluator-only concern
/// (see GOTCHAS S1.1 — `ast.rs` is shared with `build.rs`).
pub(crate) const REGEX_MAX_BYTES: usize = 8192;

/// Compile `pattern` with the magic-rule regex flags applied.
///
/// Multi-line mode is always enabled (unconditional in libmagic via
/// `REG_NEWLINE`) and `.` does not match newlines. The `case_insensitive`
/// flag is the only compile-time flag the magic-rule interface controls;
/// the `RegexCount` variant (passed elsewhere) and the `start_offset`
/// flag affect window computation and anchor advance respectively, not
/// regex compilation.
fn build_regex(pattern: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(true)
        .dot_matches_new_line(false)
        .build()
}

/// Compute the scan window for a regex rule at `offset`, applying the
/// 8192-byte cap and the `RegexCount` variant semantics.
///
/// Returns a slice of `buffer` starting at `offset`:
///
/// * [`RegexCount::Default`]: window length is
///   `min(REGEX_MAX_BYTES, remaining)`.
/// * [`RegexCount::Bytes(n)`]: window length is
///   `min(n, REGEX_MAX_BYTES, remaining)`.
/// * [`RegexCount::Lines(count)`]: window extends from `offset` through
///   the end of the Nth line terminator (inclusive), where N is
///   `count.unwrap_or(u32::MAX)`. Three terminator sequences each count
///   as a single line: LF (`\n`), CRLF (`\r\n`, consumed as one), and
///   bare CR (`\r`, for classic Mac line endings). If the Nth terminator
///   is not found within `REGEX_MAX_BYTES`, the window is truncated to
///   8192 bytes. If `count` is `None` (the `regex/l` shorthand) and no
///   terminator is found at all, the window is the whole buffer tail up
///   to the 8192-byte cap.
///
/// [`RegexCount::Default`]: crate::parser::ast::RegexCount::Default
/// [`RegexCount::Bytes(n)`]: crate::parser::ast::RegexCount::Bytes
/// [`RegexCount::Lines(count)`]: crate::parser::ast::RegexCount::Lines
fn compute_window(buffer: &[u8], offset: usize, count: crate::parser::ast::RegexCount) -> &[u8] {
    use crate::parser::ast::RegexCount;
    let Some(remaining) = buffer.get(offset..) else {
        debug_assert!(
            false,
            "compute_window: offset {offset} > buffer.len() {} -- caller must bounds-check",
            buffer.len()
        );
        return &[];
    };
    let byte_cap = remaining.len().min(REGEX_MAX_BYTES);
    let capped = &remaining[..byte_cap];

    match count {
        // `Default` and `Lines(None)` both produce the full byte-capped
        // window. For `Lines(None)` the line walk would complete without
        // ever hitting its break condition (the walk can only see at
        // most 8192 terminators, far fewer than the `u32::MAX` implicit
        // target), so skip the walk entirely.
        RegexCount::Default | RegexCount::Lines(None) => capped,
        RegexCount::Bytes(n) => {
            let count_bytes = (n.get() as usize).min(REGEX_MAX_BYTES);
            &capped[..count_bytes.min(capped.len())]
        }
        RegexCount::Lines(Some(target)) => {
            // Walk the byte-capped slice counting LF, CR, and CRLF
            // pairs as single terminators. Stop after the Nth
            // terminator. Uses `.get()` bounds-checked access per
            // the project's memory-safety rule (AGENTS.md "Memory
            // Safety First"), even though the loop condition
            // `idx < capped.len()` already guarantees `capped[idx]`
            // would be in-bounds -- `.get()` makes the CRLF look-
            // ahead (`idx + 1`) cleanly `None` at the window edge
            // without needing a separate `idx + 1 < capped.len()`
            // guard.
            let target_lines = target.get();
            let mut lines_seen: u32 = 0;
            let mut idx = 0usize;
            while lines_seen < target_lines {
                match capped.get(idx) {
                    Some(b'\r') => {
                        // Treat CR and CRLF as a single terminator.
                        let advance = if matches!(capped.get(idx + 1), Some(b'\n')) {
                            2
                        } else {
                            1
                        };
                        idx += advance;
                        lines_seen += 1;
                    }
                    Some(b'\n') => {
                        idx += 1;
                        lines_seen += 1;
                    }
                    Some(_) => idx += 1,
                    None => break,
                }
            }
            &capped[..idx]
        }
    }
}

/// Scan `buffer` starting at `offset` for the first match of `pattern`.
///
/// # Arguments
///
/// * `buffer` - File buffer to scan
/// * `offset` - Starting position within the buffer
/// * `pattern` - Regex source string (from the rule's `Value::String`
///   operand)
/// * `flags` - Regex modifier flags (`/c`, `/s`)
/// * `count` - Scan window specifier ([`RegexCount`] variant)
///
/// # Returns
///
/// * `Ok(Some(Value::String(matched_text)))` on a successful match --
///   invalid UTF-8 in the matched bytes is replaced with U+FFFD via
///   `from_utf8_lossy`. The matched text may legitimately be empty for
///   zero-width matches (e.g., `^`, `a*`, or `.{0}`).
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
///
/// [`RegexCount`]: crate::parser::ast::RegexCount
pub fn read_regex(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    flags: RegexFlags,
    count: crate::parser::ast::RegexCount,
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

    let window = compute_window(buffer, offset, count);

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
    count: crate::parser::ast::RegexCount,
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
    let window = compute_window(buffer, offset, count);
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
    use crate::parser::ast::RegexCount;
    use std::num::NonZeroU32;

    fn no_flags() -> RegexFlags {
        RegexFlags::default()
    }

    /// Helper: plain `regex` (default window, no `/c`, no `/s`).
    fn default_count() -> RegexCount {
        RegexCount::Default
    }

    /// Helper: `regex/Nl` with a specific line count.
    fn lines(n: u32) -> RegexCount {
        RegexCount::Lines(NonZeroU32::new(n))
    }

    /// Helper: `regex/l` with no explicit line count.
    fn lines_unbounded() -> RegexCount {
        RegexCount::Lines(None)
    }

    /// Helper: `regex/N` byte count.
    fn bytes(n: u32) -> RegexCount {
        RegexCount::Bytes(NonZeroU32::new(n).expect("nonzero byte count"))
    }

    /// Helper for `/c` case-insensitive only.
    fn case_flag() -> RegexFlags {
        RegexFlags {
            case_insensitive: true,
            start_offset: false,
        }
    }

    /// Helper for `/s` start-offset only.
    fn start_flag() -> RegexFlags {
        RegexFlags {
            case_insensitive: false,
            start_offset: true,
        }
    }

    #[test]
    fn test_read_regex_basic_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "World", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_no_match_returns_none() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "xyz", no_flags(), default_count()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_case_insensitive() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", case_flag(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_case_sensitive_no_match() {
        let buffer = b"Hello, World!";
        let result = read_regex(buffer, 0, "world", no_flags(), default_count()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_regex_multiline_anchor_across_lines() {
        // libmagic always compiles regexes with REG_NEWLINE, so `^` and
        // `$` match at internal line boundaries regardless of line-count
        // mode. This test pins the behavior: `^second` on a two-line
        // buffer matches the second line even with no flags set.
        let buffer = b"first line\nsecond line";
        let result = read_regex(buffer, 0, "^second", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("second".to_string())));
    }

    #[test]
    fn test_read_regex_dot_does_not_match_newline() {
        // The REG_NEWLINE flag also makes `.` stop at newlines. A `.+`
        // match against a multi-line buffer must not consume the `\n`.
        let buffer = b"first\nsecond";
        let result = read_regex(buffer, 0, ".+", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("first".to_string())));
    }

    #[test]
    fn test_read_regex_zero_width_start_anchor_matches() {
        // `^` matches zero-width at position 0. Must be reported as
        // `Some(Value::String(""))`, not `None`. Regression guard for C3.
        let buffer = b"hello";
        let result = read_regex(buffer, 0, "^", no_flags(), default_count()).unwrap();
        assert_eq!(
            result,
            Some(Value::String(String::new())),
            "^ is a legitimate zero-width match, not a miss"
        );
    }

    #[test]
    fn test_read_regex_zero_width_star_matches_empty() {
        let buffer = b"xyz";
        let result = read_regex(buffer, 0, "a*", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String(String::new())));
    }

    #[test]
    fn test_read_regex_at_offset() {
        let buffer = b"prefix_World!";
        let result = read_regex(buffer, 7, "World", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_regex_offset_past_end() {
        let buffer = b"Hello";
        let result = read_regex(buffer, 10, "x", no_flags(), default_count());
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
        let result = read_regex(buffer, 0, "[unclosed", no_flags(), default_count());
        assert!(matches!(result, Err(TypeReadError::UnsupportedType { .. })));
    }

    #[test]
    fn test_read_regex_binary_safe() {
        let buffer = &[0x00, 0xff, 0xfe, 0x41, 0x42, 0x43];
        let result = read_regex(buffer, 0, "ABC", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("ABC".to_string())));
    }

    #[test]
    fn test_read_regex_character_class() {
        let buffer = b"abc123def";
        let result = read_regex(buffer, 0, "[0-9]+", no_flags(), default_count()).unwrap();
        assert_eq!(result, Some(Value::String("123".to_string())));
    }

    // ------- V1: line-based window -------

    #[test]
    fn test_read_regex_line_based_one_line_caps_scan() {
        // `regex/1l` with a pattern that appears on the second line must
        // miss -- the scan window stops after the first newline.
        let buffer = b"first line\nsecond line\n";
        let result = read_regex(buffer, 0, "second", no_flags(), lines(1)).unwrap();
        assert_eq!(result, None, "scan should stop after the first line");
    }

    #[test]
    fn test_read_regex_line_based_crlf_terminator() {
        // CRLF (`\r\n`) counts as a single line terminator.
        let buffer = b"line1\r\nline2\r\n";
        let second = read_regex(buffer, 0, "line2", no_flags(), lines(1)).unwrap();
        assert_eq!(second, None, "CRLF should end the first line");
    }

    #[test]
    fn test_read_regex_line_based_counts_multiple_lines() {
        // `regex/3l` scans up to the third line, so a pattern on line 3
        // matches, but a pattern on line 4 misses.
        let buffer = b"line1\nline2\nline3\nline4\n";
        let line3 = read_regex(buffer, 0, "line3", no_flags(), lines(3)).unwrap();
        assert_eq!(line3, Some(Value::String("line3".to_string())));

        let line4 = read_regex(buffer, 0, "line4", no_flags(), lines(3)).unwrap();
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
        let result = read_regex(&buffer, 0, "needle", no_flags(), default_count()).unwrap();
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
        let result = read_regex(&buffer, 0, "needle", no_flags(), bytes(100_000)).unwrap();
        assert_eq!(result, None, "explicit count must still be clamped to 8192");
    }

    #[test]
    fn test_read_regex_small_count_honored() {
        // A small explicit count (e.g., 10 bytes) must be honored -- a
        // pattern past byte 10 misses.
        let buffer = b"abcdefghij_needle_here";
        let result = read_regex(buffer, 0, "needle", no_flags(), bytes(10)).unwrap();
        assert_eq!(result, None);
    }

    // ------- 8192-byte boundary regression guards -------

    #[test]
    fn test_read_regex_pattern_ending_exactly_at_byte_8192_matches() {
        // Buffer: 8186 filler bytes + "needle" (6 bytes) at indices
        // 8186..8192. The last byte of the match is at index 8191, which
        // is the last byte the 8192-byte cap allows. Must match.
        let mut buffer = vec![b'a'; 8186];
        buffer.extend_from_slice(b"needle");
        buffer.extend_from_slice(b"trailing");
        let result = read_regex(&buffer, 0, "needle", no_flags(), default_count()).unwrap();
        assert_eq!(
            result,
            Some(Value::String("needle".to_string())),
            "pattern ending at byte 8191 (last byte inside cap) must match"
        );
    }

    #[test]
    fn test_read_regex_pattern_starting_at_byte_8192_misses() {
        // Buffer: 8192 filler + "needle" at index 8192 (first byte past
        // the cap). Must miss.
        let mut buffer = vec![b'a'; 8192];
        buffer.extend_from_slice(b"needle");
        let result = read_regex(&buffer, 0, "needle", no_flags(), default_count()).unwrap();
        assert_eq!(
            result, None,
            "pattern starting at byte 8192 is one byte past the cap"
        );
    }

    #[test]
    fn test_read_regex_pattern_straddling_8192_boundary_misses() {
        // Buffer: 8190 filler + "needle" at indices 8190..8196. The
        // pattern's last 4 bytes are past the cap, so it must miss even
        // though the first 2 bytes are inside.
        let mut buffer = vec![b'a'; 8190];
        buffer.extend_from_slice(b"needle");
        buffer.extend(std::iter::repeat_n(b'z', 100));
        let result = read_regex(&buffer, 0, "needle", no_flags(), default_count()).unwrap();
        assert_eq!(
            result, None,
            "pattern straddling the 8192 boundary must not match"
        );
    }

    #[test]
    fn test_read_regex_line_based_respects_8192_cap() {
        // Line mode must also respect the 8192-byte cap. A buffer with
        // 9000 bytes of non-terminator content and "needle" past byte
        // 9000 must miss even with Lines(None) (the "no line limit"
        // case).
        let mut buffer = vec![b'a'; 9000];
        buffer.extend_from_slice(b"needle\n");
        let result = read_regex(&buffer, 0, "needle", no_flags(), lines_unbounded()).unwrap();
        assert_eq!(result, None, "line-mode scan must still cap at 8192 bytes");
    }

    #[test]
    fn test_read_regex_lines_none_is_equivalent_to_default_on_buffer_with_terminators() {
        // Regression guard: `RegexCount::Lines(None)` (the `regex/l`
        // shorthand) and `RegexCount::Default` are semantically
        // equivalent -- both produce the full 8192-byte capped window.
        // This test pins that equivalence on a buffer that actually has
        // line terminators, which would exercise the line-walk loop in
        // a pre-simplification implementation. If a future refactor
        // diverges them (e.g., by making `Lines(None)` truncate to the
        // last terminator), this test fires.
        let buffer = b"alpha\nbravo\ncharlie\ndelta";

        // A pattern that matches a byte sequence straddling a `\n`
        // boundary must succeed under BOTH variants because neither
        // truncates the window short. Since multi-line mode is on,
        // `.` does not match `\n`, so we use a class to span it.
        let pattern = "bravo[\\s\\S]*charlie";

        let default_match = read_regex(buffer, 0, pattern, no_flags(), default_count()).unwrap();
        let lines_none_match =
            read_regex(buffer, 0, pattern, no_flags(), lines_unbounded()).unwrap();

        assert!(
            default_match.is_some(),
            "Default window should match the full buffer"
        );
        assert_eq!(
            default_match, lines_none_match,
            "RegexCount::Default and RegexCount::Lines(None) must produce identical matches"
        );
    }

    // ------- bare-CR line terminator (classic Mac) -------

    #[test]
    fn test_read_regex_line_based_bare_cr_terminator() {
        // Classic Mac line endings: `\r` alone counts as a single line
        // terminator.
        let buffer = b"line1\rline2\rline3\r";
        // Line 1 is "line1\r" — "line1" must match.
        let first = read_regex(buffer, 0, "line1", no_flags(), lines(1)).unwrap();
        assert_eq!(first, Some(Value::String("line1".to_string())));
        // "line2" is on line 2 — must NOT match with count=1.
        let second = read_regex(buffer, 0, "line2", no_flags(), lines(1)).unwrap();
        assert_eq!(
            second, None,
            "scan with count=1 must stop after the first bare CR"
        );
    }

    #[test]
    fn test_read_regex_line_based_mixed_terminators() {
        // LF, CRLF, and CR in the same buffer each count as one line.
        // Line 1: "alpha" terminated by LF at index 5.
        // Line 2: "bravo" terminated by CRLF at indices 11..13.
        // Line 3: "charlie" terminated by CR at index 21.
        // Line 4: "delta".
        let buffer = b"alpha\nbravo\r\ncharlie\rdelta";
        // With count=2 the window ends after line 2's CRLF, so
        // "charlie" must miss.
        let charlie = read_regex(buffer, 0, "charlie", no_flags(), lines(2)).unwrap();
        assert_eq!(charlie, None, "scan with count=2 stops after bravo's CRLF");
        // "bravo" is inside the 2-line window and must match.
        let bravo = read_regex(buffer, 0, "bravo", no_flags(), lines(2)).unwrap();
        assert_eq!(bravo, Some(Value::String("bravo".to_string())));
    }

    // ------- regex_bytes_consumed -------

    #[test]
    fn test_regex_bytes_consumed_match_end_by_default() {
        let buffer = b"Hello, World!";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "World", no_flags(), default_count()),
            12
        );
    }

    #[test]
    fn test_regex_bytes_consumed_no_match() {
        let buffer = b"Hello";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "xyz", no_flags(), default_count()),
            0
        );
    }

    #[test]
    fn test_regex_bytes_consumed_bytes_variant_matches_match_end() {
        // RegexCount::Bytes(N) anchor advance: the re-scan should still
        // find the match inside the N-byte window and return match-end.
        let buffer = b"prefix_World_suffix_more_stuff";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "World", no_flags(), bytes(20)),
            12,
            "Bytes(20) window reaches 'World' (ends at byte 12); advance = match-end"
        );
    }

    #[test]
    fn test_regex_bytes_consumed_bytes_variant_narrow_window_misses() {
        // If the byte count stops before the pattern, no match and
        // anchor stays put.
        let buffer = b"prefix_World_suffix";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "World", no_flags(), bytes(5)),
            0,
            "Bytes(5) window is 'prefi' -- 'World' is outside, no anchor advance"
        );
    }

    #[test]
    fn test_regex_bytes_consumed_lines_variant_matches_inside_window() {
        // RegexCount::Lines(Some(2)) anchor advance: pattern on the
        // second line is inside the window; match-end is measured from
        // the scan start (offset 0), so it's the absolute position of
        // the last matched byte within the 2-line window.
        let buffer = b"line1\nline2\nline3\nline4";
        // Match "line2" (6 bytes) starts at byte 6, ends at byte 11.
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "line2", no_flags(), lines(2)),
            11,
            "Lines(Some(2)) window includes line 2; advance = match-end at byte 11"
        );
    }

    #[test]
    fn test_regex_bytes_consumed_lines_variant_misses_past_window() {
        // Pattern on line 3 is outside a 2-line window; no match.
        let buffer = b"line1\nline2\nline3\nline4";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "line3", no_flags(), lines(2)),
            0,
            "Lines(Some(2)) window ends after line 2; line 3 is not scanned"
        );
    }

    #[test]
    fn test_regex_bytes_consumed_lines_none_matches_full_window() {
        // RegexCount::Lines(None) behaves like Default -- scans the
        // full 8192-byte capped window. A pattern anywhere in a small
        // buffer matches and the advance is match-end.
        let buffer = b"line1\nline2\nline3\nline4";
        // "line4" starts at byte 18, ends at byte 23.
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "line4", no_flags(), lines_unbounded()),
            23
        );
    }

    #[test]
    fn test_regex_bytes_consumed_zero_width_match_returns_zero() {
        let buffer = b"hello";
        assert_eq!(
            regex_bytes_consumed(buffer, 0, "^", no_flags(), default_count()),
            0
        );
    }

    // ------- V2: /s flag (start_offset) -------

    #[test]
    fn test_regex_bytes_consumed_start_offset_returns_match_start() {
        // Buffer: "abcWorld", pattern "World" matches at index 3, length
        // 5. Without `/s` the anchor advances by 8 (match-end). With `/s`
        // it advances by 3 (match-start), matching libmagic's
        // REGEX_OFFSET_START / moffset() zero-length path.
        let buffer = b"abcWorld";
        let match_end = regex_bytes_consumed(buffer, 0, "World", no_flags(), default_count());
        let match_start = regex_bytes_consumed(buffer, 0, "World", start_flag(), default_count());
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
            regex_bytes_consumed(buffer, 0, "xyz", start_flag(), default_count()),
            0
        );
    }
}
