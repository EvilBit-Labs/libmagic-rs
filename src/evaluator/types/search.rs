// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Bounded literal pattern search for magic rule evaluation.
//!
//! Implements the `search` `TypeKind` as a forward scan for a literal byte
//! pattern within a bounded window. Unlike `TypeKind::String`, which only
//! matches at the exact offset, `search` advances through the buffer looking
//! for the first occurrence of the pattern anywhere in the window. The
//! search window is `buffer[offset..]` capped by the optional `range`.

use super::TypeReadError;
use crate::parser::ast::Value;

/// Scan a bounded window of `buffer` for the first occurrence of `pattern`.
///
/// # Arguments
///
/// * `buffer` - File buffer to scan
/// * `offset` - Starting position within the buffer
/// * `pattern` - Literal bytes to search for (from the rule's value operand)
/// * `range` - Optional byte range to scan starting at `offset`. When
///   `Some(n)`, the window is the smaller of `n` and the buffer remainder.
///   When `None`, the window extends to the end of the buffer.
///
/// # Returns
///
/// * `Ok(Some(Value::String(pattern_text)))` on a successful match -- the
///   matched text is the literal pattern (search is a locate, not a
///   capture), with invalid UTF-8 replaced via `from_utf8_lossy`.
/// * `Ok(None)` when the pattern is not found in the window. `None` is the
///   structured "no match" signal; callers that need a compatibility
///   `Value::String(String::new())` should convert at the call site.
///
/// # Errors
///
/// * `TypeReadError::BufferOverrun` if `offset >= buffer.len()`.
pub fn read_search(
    buffer: &[u8],
    offset: usize,
    pattern: &[u8],
    range: Option<usize>,
) -> Result<Option<Value>, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let remaining = &buffer[offset..];

    let window_len = match range {
        Some(n) => n.min(remaining.len()),
        None => remaining.len(),
    };
    let window = &remaining[..window_len];

    match memchr::memmem::find(window, pattern) {
        Some(_) => Ok(Some(Value::String(
            String::from_utf8_lossy(pattern).into_owned(),
        ))),
        None => Ok(None),
    }
}

/// Compute the anchor-advance distance for a successful search match.
///
/// GNU `file` advances its previous-match anchor to the byte just past the
/// matched pattern -- `base_offset + match_index + pattern.len()`, not past
/// the full search window. See `src/softmagic.c` `moffset()` / `FILE_SEARCH`
/// branch (`vlen = m->vallen; o = ms->search.offset + vlen - offset;`) where
/// `ms->search.offset` has already been advanced by `idx` (the match index
/// within the window).
///
/// This function re-runs the same `memchr::memmem::find` scan as
/// [`read_search`] and returns `match_index + pattern.len()`. On miss or
/// invalid state it returns `0`; the engine only calls it after a successful
/// read so the defensive paths are belt-and-braces.
///
/// Note: like [`crate::evaluator::types::regex::regex_bytes_consumed`], this
/// pays the cost of a second scan rather than threading the match position
/// back through the reader API. Caching would require a second return
/// channel that complicates every non-pattern type.
#[must_use]
pub(super) fn search_bytes_consumed(
    buffer: &[u8],
    offset: usize,
    pattern: &[u8],
    range: Option<usize>,
) -> usize {
    let Some(remaining) = buffer.get(offset..) else {
        debug_assert!(
            false,
            "search_bytes_consumed: offset {offset} > buffer.len() {} -- engine invariant violated (called without a preceding successful read_search)",
            buffer.len()
        );
        return 0;
    };
    let window_len = match range {
        Some(n) => n.min(remaining.len()),
        None => remaining.len(),
    };
    let window = &remaining[..window_len];
    memchr::memmem::find(window, pattern).map_or(0, |idx| idx + pattern.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_search_basic_match() {
        let buffer = b"Hello, World!";
        let result = read_search(buffer, 0, b"World", None).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_no_match_returns_none() {
        let buffer = b"Hello, World!";
        let result = read_search(buffer, 0, b"xyz", None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_search_bounded_range_finds() {
        let buffer = b"abcdefWorldxyz";
        let result = read_search(buffer, 0, b"World", Some(14)).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_bounded_range_too_small() {
        let buffer = b"abcdefWorldxyz";
        // Range only covers "abcde" -- World is past the window
        let result = read_search(buffer, 0, b"World", Some(5)).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_search_range_larger_than_buffer() {
        let buffer = b"Hello";
        let result = read_search(buffer, 0, b"lo", Some(1000)).unwrap();
        assert_eq!(result, Some(Value::String("lo".to_string())));
    }

    #[test]
    fn test_read_search_at_offset() {
        let buffer = b"junk_prefix_World!";
        let result = read_search(buffer, 12, b"World", None).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_offset_past_end() {
        let buffer = b"Hello";
        let result = read_search(buffer, 10, b"x", None);
        assert!(matches!(
            result,
            Err(TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 5
            })
        ));
    }

    #[test]
    fn test_read_search_binary_pattern() {
        let buffer = &[0x00, 0xff, 0xfe, 0xaa, 0xbb, 0xcc];
        let result = read_search(buffer, 0, &[0xaa, 0xbb], None).unwrap();
        // Invalid UTF-8 gets replaced with U+FFFD, but the match is still Some
        match result {
            Some(Value::String(s)) => assert!(!s.is_empty()),
            other => panic!("Expected Some(Value::String), got {other:?}"),
        }
    }

    #[test]
    fn test_read_search_empty_pattern_matches_at_offset() {
        // memmem finds an empty pattern at position 0 in any non-empty
        // window. This is a degenerate but well-defined outcome: the
        // reader reports a match with an empty matched text. Magic files
        // using `search` with an empty pattern are nonsensical; the
        // grammar layer should reject them, not the reader.
        let buffer = b"Hello";
        let result = read_search(buffer, 0, b"", None).unwrap();
        assert_eq!(result, Some(Value::String(String::new())));
    }

    #[test]
    fn test_read_search_multi_char_pattern() {
        let buffer = b"The quick brown fox jumps over the lazy dog";
        let result = read_search(buffer, 0, b"brown", Some(50)).unwrap();
        assert_eq!(result, Some(Value::String("brown".to_string())));
    }

    #[test]
    fn test_search_bytes_consumed_matches_match_end_not_window_end() {
        // GNU `file` advances the anchor past the matched pattern, not
        // past the full search window. Regression guard for the pre-fix
        // behavior which returned the whole window size.
        let buffer = b"abcWorldxyz___more_data";
        // Window size 10 (`abcWorldxy`), pattern "World" at index 3,
        // length 5, so match-end = 3 + 5 = 8.
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"World", Some(10)),
            8,
            "expected match-end (8), not window-end (10)"
        );
    }

    #[test]
    fn test_search_bytes_consumed_no_match_returns_zero() {
        let buffer = b"abcdefghij";
        assert_eq!(search_bytes_consumed(buffer, 0, b"XYZ", Some(10)), 0);
    }

    #[test]
    fn test_search_bytes_consumed_unbounded_range() {
        let buffer = b"prefix_World_suffix";
        // No range cap, so the scan looks through the entire buffer tail;
        // match "World" starts at index 7, length 5, match-end = 12.
        assert_eq!(search_bytes_consumed(buffer, 0, b"World", None), 12);
    }

    #[test]
    fn test_search_bytes_consumed_range_caps_match() {
        // Match exists past the window; bytes_consumed reports 0 because
        // the scan only sees the window.
        let buffer = b"abcdefWorldxyz";
        // Range 5 means window is "abcde" -- no "World" inside it.
        assert_eq!(search_bytes_consumed(buffer, 0, b"World", Some(5)), 0);
    }

    #[test]
    fn test_search_bytes_consumed_match_at_window_end() {
        // Pattern lands exactly at the window boundary: window is 8
        // bytes, pattern "def" occupies indices 3..6, match-end = 6.
        let buffer = b"abcdefgh_ignored";
        assert_eq!(search_bytes_consumed(buffer, 0, b"def", Some(8)), 6);
    }
}
