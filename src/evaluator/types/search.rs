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
use super::string::compare_string_with_flags;
use crate::parser::ast::{SearchFlags, Value};
use std::num::NonZeroUsize;

/// ASCII whitespace bytes per libmagic's `isspace`-equivalent contract.
///
/// Mirrors [`super::trim_ascii_whitespace`] (which is module-private) so
/// search-side trimming for `/T` can be done without crossing module
/// boundaries. ASCII-only is intentional: libmagic's `STRING_TRIM` uses C
/// `isspace`, not full Unicode whitespace.
// Slicing is invariant-safe: `start <= end <= s.len()` by construction
// (`position`/`rposition` results).
#[allow(clippy::indexing_slicing)]
fn trim_ascii_whitespace(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    &s[start..end]
}

/// Scan result: the byte index of the match within the window, and the
/// number of buffer bytes consumed by the match (which may exceed
/// `pattern.len()` under `/w`/`/W` whitespace flags or fall short of it
/// under `/T` trim).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScanHit {
    match_idx: usize,
    matched_len: usize,
}

/// Locate `pattern` in `buffer[offset..]` within `range` bytes, honoring
/// `flags`.
///
/// Returns `None` when the pattern is not found in the window. Returns
/// `Err(BufferOverrun)` when `offset >= buffer.len()`. The fast path uses
/// `memchr::memmem::find` for byte-exact patterns; when any of
/// `/c`/`/C`/`/w`/`/W`/`/T`/`/f` is set the comparator-driven slow path
/// walks the window byte-by-byte and recovers the post-comparator
/// consumed-byte count.
///
/// The `flags.start_anchor` and `flags.text_test`/`flags.bin_test` fields
/// have no effect on whether a match is found -- they only matter to
/// [`search_bytes_consumed`] and to future MIME-output wiring respectively.
// Slicing is invariant-safe: `offset < buffer.len()` is checked at entry
// and `window_len` is clamped to `remaining.len()`.
#[allow(clippy::indexing_slicing)]
fn find_match(
    buffer: &[u8],
    offset: usize,
    pattern: &[u8],
    range: NonZeroUsize,
    flags: SearchFlags,
) -> Result<Option<ScanHit>, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let remaining = &buffer[offset..];
    let window_len = range.get().min(remaining.len());
    let window = &remaining[..window_len];

    if flags.needs_byte_compare() {
        // Slow path: byte-walk with `compare_string_with_flags`. Trim the
        // pattern up front when `/T` is set so we don't pay the trim cost
        // at every candidate offset; the comparator itself ignores the
        // `trim` field (see its docs at GOTCHAS S6 and the per-flag note
        // on `compare_string_with_flags`).
        let string_flags = flags.to_string_flags();
        let effective_pattern: &[u8] = if flags.trim {
            trim_ascii_whitespace(pattern)
        } else {
            pattern
        };
        // An empty post-trim pattern would silently match every offset
        // because `compare_string_with_flags(b"", ...)` returns `Some(0)`.
        // Treat it as "no match" with a `warn!` so the malformed rule
        // surfaces in logs without poisoning subsequent rules. Mirrors
        // the flagged-string path in
        // `src/evaluator/types/mod.rs::read_pattern_match`.
        if effective_pattern.is_empty() {
            log::warn!(
                "search rule has empty pattern (after /T trim); treating as no-match to avoid catastrophic over-matching"
            );
            return Ok(None);
        }

        for i in 0..window_len {
            if let Some(consumed) =
                compare_string_with_flags(effective_pattern, window, i, string_flags)
            {
                return Ok(Some(ScanHit {
                    match_idx: i,
                    matched_len: consumed,
                }));
            }
        }
        Ok(None)
    } else {
        // Fast path: SIMD-accelerated literal scan. memmem is byte-exact,
        // so the consumed length equals the pattern length on hit.
        Ok(memchr::memmem::find(window, pattern).map(|idx| ScanHit {
            match_idx: idx,
            matched_len: pattern.len(),
        }))
    }
}

/// Scan a bounded window of `buffer` for the first occurrence of `pattern`.
///
/// # Arguments
///
/// * `buffer` - File buffer to scan
/// * `offset` - Starting position within the buffer
/// * `pattern` - Literal bytes to search for (from the rule's value operand)
/// * `range` - Byte range to scan starting at `offset`. The window is the
///   smaller of `range` and the buffer remainder. Per GNU `file`'s
///   magic(5), the range is mandatory and is therefore a [`NonZeroUsize`]
///   in the type signature.
/// * `flags` - Parsed flag set from the magic rule. `/c`/`/C`/`/w`/`/W`/`/T`/`/f`
///   force the byte-walk slow path through
///   [`compare_string_with_flags`]; `/s`/`/t`/`/b` keep the SIMD fast
///   path. See [`SearchFlags`] for the per-letter semantics.
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
    range: NonZeroUsize,
    flags: SearchFlags,
) -> Result<Option<Value>, TypeReadError> {
    match find_match(buffer, offset, pattern, range, flags)? {
        Some(_) => Ok(Some(Value::String(
            String::from_utf8_lossy(pattern).into_owned(),
        ))),
        None => Ok(None),
    }
}

/// Compute the anchor-advance distance for a successful search match.
///
/// GNU `file` advances its previous-match anchor to the byte just past the
/// matched pattern -- `base_offset + match_index + matched_len`, not past
/// the full search window. See `src/softmagic.c` `moffset()` / `FILE_SEARCH`
/// branch (`vlen = m->vallen; o = ms->search.offset + vlen - offset;`) where
/// `ms->search.offset` has already been advanced by `idx` (the match index
/// within the window).
///
/// When `flags.start_anchor` is set (the `/s` modifier), the anchor lands
/// on `match_index` instead of past-end. This mirrors libmagic's
/// `FILE_SEARCH` / search-start handling in `softmagic.c`'s `moffset`.
///
/// `matched_len` is the source of truth for the past-end branch: under
/// `/T` trim the comparator inspects fewer bytes than `pattern.len()`,
/// and under `/w`/`/W` the comparator can consume more. The
/// [`compare_string_with_flags`] return value carries the actual count.
/// On the fast path (no comparison-altering flags) `matched_len ==
/// pattern.len()` because `memmem` is byte-exact.
///
/// This function re-runs the same scan as [`read_search`] and returns the
/// advance. On miss or invalid state it returns `0`; the engine only
/// calls it after a successful read so the defensive paths are
/// belt-and-braces.
///
/// The result is clamped against `buffer.len().saturating_sub(offset)`
/// (the remaining-buffer length) to defend against any pattern-length math
/// that could overflow on adversarial input -- mirroring the pstring
/// anchor clamp at `docs/solutions/security-issues/pstring-anchor-poisoning.md`.
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
    range: NonZeroUsize,
    flags: SearchFlags,
) -> usize {
    if buffer.get(offset..).is_none() {
        debug_assert!(
            false,
            "search_bytes_consumed: offset {offset} > buffer.len() {} -- engine invariant violated (called without a preceding successful read_search)",
            buffer.len()
        );
        return 0;
    }

    let Ok(Some(hit)) = find_match(buffer, offset, pattern, range, flags) else {
        return 0;
    };

    let raw = if flags.start_anchor {
        hit.match_idx
    } else {
        // Use saturating_add so we never panic; the clamp below converts
        // any saturation into a buffer-bounded value.
        hit.match_idx.saturating_add(hit.matched_len)
    };
    let remaining = buffer.len().saturating_sub(offset);
    raw.min(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("non-zero in test")
    }

    fn default_flags() -> SearchFlags {
        SearchFlags::default()
    }

    #[test]
    fn test_read_search_basic_match() {
        let buffer = b"Hello, World!";
        let result = read_search(buffer, 0, b"World", nz(100), default_flags()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_no_match_returns_none() {
        let buffer = b"Hello, World!";
        let result = read_search(buffer, 0, b"xyz", nz(100), default_flags()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_search_bounded_range_finds() {
        let buffer = b"abcdefWorldxyz";
        let result = read_search(buffer, 0, b"World", nz(14), default_flags()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_bounded_range_too_small() {
        let buffer = b"abcdefWorldxyz";
        // Range only covers "abcde" -- World is past the window
        let result = read_search(buffer, 0, b"World", nz(5), default_flags()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_search_range_larger_than_buffer() {
        let buffer = b"Hello";
        let result = read_search(buffer, 0, b"lo", nz(1000), default_flags()).unwrap();
        assert_eq!(result, Some(Value::String("lo".to_string())));
    }

    #[test]
    fn test_read_search_at_offset() {
        let buffer = b"junk_prefix_World!";
        let result = read_search(buffer, 12, b"World", nz(100), default_flags()).unwrap();
        assert_eq!(result, Some(Value::String("World".to_string())));
    }

    #[test]
    fn test_read_search_offset_past_end() {
        let buffer = b"Hello";
        let result = read_search(buffer, 10, b"x", nz(100), default_flags());
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
        let result = read_search(buffer, 0, &[0xaa, 0xbb], nz(100), default_flags()).unwrap();
        // Invalid UTF-8 gets replaced with U+FFFD, but the match is still Some
        match result {
            Some(Value::String(s)) => assert!(!s.is_empty()),
            other => panic!("Expected Some(Value::String), got {other:?}"),
        }
    }

    #[test]
    fn test_read_search_empty_pattern_matches_at_offset() {
        // memmem finds an empty pattern at position 0 in any non-empty
        // window. This is a degenerate but well-defined outcome on the
        // fast path: the reader reports a match with an empty matched
        // text. Magic files using `search` with an empty pattern are
        // nonsensical; the grammar layer should reject them, not the
        // reader.
        let buffer = b"Hello";
        let result = read_search(buffer, 0, b"", nz(100), default_flags()).unwrap();
        assert_eq!(result, Some(Value::String(String::new())));
    }

    #[test]
    fn test_read_search_multi_char_pattern() {
        let buffer = b"The quick brown fox jumps over the lazy dog";
        let result = read_search(buffer, 0, b"brown", nz(50), default_flags()).unwrap();
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
            search_bytes_consumed(buffer, 0, b"World", nz(10), default_flags()),
            8,
            "expected match-end (8), not window-end (10)"
        );
    }

    #[test]
    fn test_search_bytes_consumed_no_match_returns_zero() {
        let buffer = b"abcdefghij";
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"XYZ", nz(10), default_flags()),
            0
        );
    }

    #[test]
    fn test_search_bytes_consumed_range_caps_match() {
        // Match exists past the window; bytes_consumed reports 0 because
        // the scan only sees the window.
        let buffer = b"abcdefWorldxyz";
        // Range 5 means window is "abcde" -- no "World" inside it.
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"World", nz(5), default_flags()),
            0
        );
    }

    #[test]
    fn test_search_bytes_consumed_match_at_window_end() {
        // Pattern lands exactly at the window boundary: window is 8
        // bytes, pattern "def" occupies indices 3..6, match-end = 6.
        let buffer = b"abcdefgh_ignored";
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"def", nz(8), default_flags()),
            6
        );
    }

    // ---- U3 flag-aware behavior ----

    #[test]
    fn test_search_with_start_anchor_returns_match_start_index() {
        // `/s` anchor advance lands on match-START, not match-END.
        let buffer = b"junkABCDEFmore";
        let flags = SearchFlags::default().with_start_anchor(true);
        // Match index 4, pattern length 6 -> with /s the anchor is 4,
        // without /s the anchor is 10.
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"ABCDEF", nz(20), flags),
            4,
            "/s should produce match-start anchor (4), not match-end (10)"
        );
    }

    #[test]
    fn test_search_without_start_anchor_returns_match_end_index() {
        // Same buffer/pattern as above, default flags -> match-end.
        let buffer = b"junkABCDEFmore";
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"ABCDEF", nz(20), default_flags()),
            10,
            "default flags preserve match-end anchor semantics"
        );
    }

    #[test]
    fn test_search_ignore_lowercase_matches_uppercase_buffer() {
        // `/c` -- lowercase pattern matches uppercase file bytes.
        let buffer = b"abcWORLDxyz";
        let flags = SearchFlags::default().with_ignore_lowercase(true);
        let result = read_search(buffer, 0, b"world", nz(20), flags).unwrap();
        assert!(
            result.is_some(),
            "/c should match uppercase WORLD with lowercase pattern"
        );
        // match_idx 3 + matched_len 5 = 8
        assert_eq!(search_bytes_consumed(buffer, 0, b"world", nz(20), flags), 8);
    }

    #[test]
    fn test_search_ignore_uppercase_matches_lowercase_buffer() {
        // `/C` -- uppercase pattern matches lowercase file bytes.
        let buffer = b"abcworldxyz";
        let flags = SearchFlags::default().with_ignore_uppercase(true);
        let result = read_search(buffer, 0, b"WORLD", nz(20), flags).unwrap();
        assert!(
            result.is_some(),
            "/C should match lowercase world with uppercase pattern"
        );
        assert_eq!(search_bytes_consumed(buffer, 0, b"WORLD", nz(20), flags), 8);
    }

    #[test]
    fn test_search_trim_with_pattern_whitespace() {
        // `/T` -- leading/trailing whitespace in the pattern is trimmed
        // before comparison. After trim the comparator sees `foo`, so
        // matched_len = 3, not 7.
        let buffer = b"foobarbaz";
        let flags = SearchFlags::default().with_trim(true);
        let result = read_search(buffer, 0, b"  foo  ", nz(20), flags).unwrap();
        assert!(
            result.is_some(),
            "/T should match after trimming pattern whitespace"
        );
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"  foo  ", nz(20), flags),
            3,
            "matched_len after /T trim is the trimmed-pattern length"
        );
    }

    #[test]
    fn test_search_compact_optional_whitespace() {
        // `/w` -- pattern whitespace matches zero or more whitespace
        // bytes in the file. The comparator consumes the wider whitespace
        // run; matched_len reflects the actual buffer bytes inspected.
        let buffer = b"foo   bar"; // three spaces
        let flags = SearchFlags::default().with_compact_optional_whitespace(true);
        let result = read_search(buffer, 0, b"foo bar", nz(20), flags).unwrap();
        assert!(result.is_some(), "/w should match wider whitespace runs");
        // match_idx 0, matched_len 9 (all 9 buffer bytes consumed) -> 9.
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"foo bar", nz(20), flags),
            9,
            "matched_len under /w reflects the wider whitespace run"
        );
    }

    #[test]
    fn test_search_anchor_only_flags_keep_fast_path() {
        // Flag-shape assertions: `/t`, `/b`, `/s` do not alter the
        // comparison, so they should NOT trigger the byte-walk slow path.
        assert!(
            !SearchFlags::default()
                .with_text_test(true)
                .needs_byte_compare(),
            "/t is metadata-only"
        );
        assert!(
            !SearchFlags::default()
                .with_bin_test(true)
                .needs_byte_compare(),
            "/b is metadata-only"
        );
        assert!(
            !SearchFlags::default()
                .with_start_anchor(true)
                .needs_byte_compare(),
            "/s is anchor-advance only"
        );
    }

    #[test]
    fn test_search_bytes_consumed_clamps_against_buffer_length() {
        // Pattern very close to buffer end. The clamp guarantees the
        // returned advance never exceeds `buffer.len() - offset`,
        // protecting against arithmetic overshoot on adversarial input.
        let buffer = b"hello";
        // Pattern "lo" at index 3, matched_len 2, raw = 5; remaining =
        // buffer.len() - offset = 5 - 0 = 5. Clamp leaves it at 5.
        assert_eq!(
            search_bytes_consumed(buffer, 0, b"lo", nz(100), default_flags()),
            5
        );
        // Pattern at offset 3: remaining = 5 - 3 = 2; match_idx 0 +
        // matched_len 2 = 2, clamp leaves it at 2.
        assert_eq!(
            search_bytes_consumed(buffer, 3, b"lo", nz(100), default_flags()),
            2
        );
    }

    #[test]
    fn test_search_full_word_blocks_in_word_match() {
        // `/f` post-match word-boundary check.
        let buffer = b"caterpillar";
        let flags = SearchFlags::default().with_full_word(true);
        // "cat" at index 0 is followed by 'e' (a word char), so /f rejects.
        let result = read_search(buffer, 0, b"cat", nz(20), flags).unwrap();
        assert!(result.is_none(), "/f should reject in-word match");

        // Now with a non-word boundary after the match.
        let buffer2 = b"a cat sat";
        let result2 = read_search(buffer2, 0, b"cat", nz(20), flags).unwrap();
        assert!(result2.is_some(), "/f should accept word-boundary match");
        // match_idx 2 + matched_len 3 = 5.
        assert_eq!(search_bytes_consumed(buffer2, 0, b"cat", nz(20), flags), 5);
    }

    #[test]
    fn test_search_combined_start_anchor_and_ignore_lowercase() {
        // `/c/s` combined: case-insensitive locate, anchor at match-start.
        let buffer = b"abcWORLDxyz";
        let flags = SearchFlags::default()
            .with_ignore_lowercase(true)
            .with_start_anchor(true);
        let result = read_search(buffer, 0, b"world", nz(20), flags).unwrap();
        assert!(result.is_some());
        // match_idx 3, /s -> anchor = 3 (not 8).
        assert_eq!(search_bytes_consumed(buffer, 0, b"world", nz(20), flags), 3);
    }

    #[test]
    fn test_search_trim_pattern_empty_after_trim_returns_none() {
        // Pure-whitespace pattern with /T trims to empty; we treat this
        // as no-match to avoid silently over-matching every offset.
        let buffer = b"hello";
        let flags = SearchFlags::default().with_trim(true);
        let result = read_search(buffer, 0, b"   ", nz(20), flags).unwrap();
        assert_eq!(
            result, None,
            "/T-emptied pattern should yield no match, not over-match"
        );
    }
}
