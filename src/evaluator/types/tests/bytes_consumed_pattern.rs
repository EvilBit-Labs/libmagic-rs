// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `bytes_consumed_with_pattern` tests for the pattern-bearing types
//! (`Regex`, `Search`) and `String` compared against a `Value::Bytes`
//! operand -- match-end vs. window-end anchor advance, the `/s`
//! start-offset flag, zero-width and no-match cases, and `Value::Bytes`
//! pattern acceptance mirroring the read-side contract in `regex_decode`.

use super::*;

#[test]
fn test_bytes_consumed_regex_with_string_pattern() {
    // Regression guard for GOTCHAS 2.1: variable-width variants must be
    // matched explicitly in `bytes_consumed_with_pattern` or relative
    // offsets silently corrupt. This test exercises the dispatch path
    // and verifies the match-end byte count matches the reader's view.
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("World".to_string());
    // "World" starts at index 7 in the buffer, length 5, so a scan from
    // offset 0 consumes 7+5=12 bytes.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        12
    );
}

/// Regression guard: `bytes_consumed_with_pattern`'s `Regex` arm must
/// accept a `Value::Bytes` pattern, mirroring U1's read-side acceptance
/// (`read_pattern_match` / `read_typed_value_with_pattern`). This was
/// caught by `prop_arbitrary_rule_evaluation_never_panics` firing the
/// `debug_assert` in the pre-fix `other => { debug_assert!(false, ...) }`
/// arm for a `NotEqual` regex rule with a `Value::Bytes` pattern -- a
/// successful Bytes-pattern regex match would advance the anchor by 0
/// (silently stalling) instead of the correct match-end distance, and the
/// property test additionally caught the `debug_assert!(false, ...)`
/// firing as a panic in debug builds.
#[test]
fn test_bytes_consumed_regex_with_bytes_pattern() {
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::Bytes(b"World".to_vec());
    // "World" starts at index 7, length 5, so a scan from offset 0
    // consumes 7+5=12 bytes -- matching the Value::String equivalent in
    // `test_bytes_consumed_regex_with_string_pattern`.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        12
    );
}

#[test]
fn test_bytes_consumed_regex_no_match_returns_zero() {
    let buf = b"abcdef";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("xyz".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_regex_zero_width_match_returns_zero() {
    // Zero-width match at position 0 means match_end=0 so the anchor
    // stays put. Cross-check with the direct reader in regex.rs.
    let buf = b"hello";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags::default(),
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("^".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_regex_start_offset_flag_uses_match_start() {
    // /s flag changes the anchor advance to match-start instead of
    // match-end. Regression guard for V2.
    let buf = b"prefix_World_suffix";
    let typ = TypeKind::Regex {
        flags: crate::parser::ast::RegexFlags {
            start_offset: true,
            ..crate::parser::ast::RegexFlags::default()
        },
        count: crate::parser::ast::RegexCount::Default,
    };
    let pattern = Value::String("World".to_string());
    // Match-start for "World" at index 7 is 7, not 12.
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 7);
}

#[test]
fn test_bytes_consumed_search_with_pattern_is_match_end() {
    // Regression guard for the pre-fix behavior that returned the
    // entire window size instead of match-end. Per GNU `file` softmagic.c
    // FILE_SEARCH, the anchor advances to `base + match_idx + pattern.len()`.
    //
    // R6 sanity control: this is an unflagged pattern, so the declared
    // pattern length and the walked-byte count coincide -- match-end here
    // is unaffected by the R6 fix (U4). See
    // `test_bytes_consumed_search_flagged_w_declared_length_not_walked_length`
    // below for the case where they diverge.
    let buf = b"abcWorld_xyz";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(10),
        flags: SearchFlags::default(),
    };
    let pattern = Value::String("World".to_string());
    // "World" is at index 3, length 5, match-end = 8.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        8,
        "expected match-end (8), not window-end (10)"
    );
}

/// R6 (U4), measured against real `file`-5.41: `search/32/w` pattern
/// `A\ B` (declared length 3) over buffer `A   Bqz` (a 3-space run where
/// the pattern has one optional space) resolves the anchor to index 3 --
/// match position (0) plus DECLARED pattern length (3) -- not the walked
/// match-end (5). Exercised through the public `bytes_consumed_with_pattern`
/// dispatch, mirroring `search_bytes_consumed`'s own unit coverage in
/// `search.rs`. See GOTCHAS S2.6.
#[test]
fn test_bytes_consumed_search_flagged_w_declared_length_not_walked_length() {
    let buf = b"A   Bqz";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(32),
        flags: SearchFlags::default().with_compact_optional_whitespace(true),
    };
    let pattern = Value::String("A B".to_string());
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        3,
        "anchor must land at declared-length index 3, matching real `file`, not walked index 5"
    );
}

/// R6/R8: the `/s` start-anchor flag still resolves the child to the
/// match POSITION, orthogonal to the declared-length fix above. Combined
/// with `/w` so the test also confirms `/s` takes priority over the
/// declared-length branch rather than the two interacting incorrectly.
#[test]
fn test_bytes_consumed_search_start_anchor_resolves_to_match_position() {
    let buf = b"junkA   Bqz";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(32),
        flags: SearchFlags::default()
            .with_compact_optional_whitespace(true)
            .with_start_anchor(true),
    };
    let pattern = Value::String("A B".to_string());
    // Match starts at index 4 ("junk" is 4 bytes); /s anchors there
    // regardless of declared length or walked length.
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        4,
        "/s must resolve to match-start (4), not a declared- or walked-length advance"
    );
}

/// A bare `search` (range `None`, scan-to-EOF) behaves as the ranged
/// form for anchor purposes: the advance is still the declared pattern
/// length under `/w`, not the walked byte count.
#[test]
fn test_bytes_consumed_search_bare_range_behaves_as_ranged_form_for_anchor() {
    let buf = b"A   Bqz_trailing_data_past_the_match";
    let typ = TypeKind::Search {
        range: None,
        flags: SearchFlags::default().with_compact_optional_whitespace(true),
    };
    let pattern = Value::String("A B".to_string());
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        3,
        "bare search (range None) must use declared length (3), matching the ranged form"
    );
}

/// R8: a `/w` search whose declared-length advance exceeds the remaining
/// buffer must not panic. `bytes_consumed_with_pattern` simply reports
/// the declared length unclamped; a downstream relative-offset child at
/// that out-of-bounds offset fails to match (GOTCHAS S15.1), which is
/// the correct, deliberately out-of-scope-here outcome.
#[test]
fn test_bytes_consumed_search_flagged_declared_length_can_exceed_buffer() {
    let buf = b"ab";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(20),
        flags: SearchFlags::default().with_compact_optional_whitespace(true),
    };
    let pattern = Value::String("a b".to_string());
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        3,
        "declared length (3) must be returned unclamped, even past the 2-byte buffer"
    );
}

#[test]
fn test_bytes_consumed_search_no_match_returns_zero() {
    let buf = b"abcdefghij";
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(10),
        flags: SearchFlags::default(),
    };
    let pattern = Value::String("XYZ".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 0);
}

#[test]
fn test_bytes_consumed_search_bytes_pattern_works() {
    // Value::Bytes is an alternative pattern shape for search -- verify
    // the dispatch path accepts it and computes the same match-end as a
    // Value::String pattern would.
    let buf = &[0x00, 0xff, 0xde, 0xad, 0xbe, 0xef, 0x11];
    let typ = TypeKind::Search {
        range: ::std::num::NonZeroUsize::new(7),
        flags: SearchFlags::default(),
    };
    let pattern = Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
    // 0xde at index 2, length 4, match-end = 6.
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 6);
}

/// Regression: when a `TypeKind::String` rule's comparison value is a
/// `Value::Bytes` (e.g., parser produces `Value::Bytes([0x7f, 'E', 'L',
/// 'F'])` for the `\177ELF` ELF magic via `parse_mixed_hex_ascii`), the
/// read path uses `read_string_exact(buffer, offset, b.len())` and so
/// the consume path must agree -- otherwise the relative-offset anchor
/// mis-advances by the NUL-scan length on a NUL-free ELF header. This
/// is the same dual-purpose-helper-sync rule documented in GOTCHAS S6.4
/// for `read_string` <-> `read_string_exact`. The bug pattern is the
/// same class as the original 3-bug fix this PR addresses; the fix here
/// closes the consume-side gap that comment-analyzer (PR #233 review)
/// flagged as load-bearing for ELF-style rules.
#[test]
fn test_bytes_consumed_string_with_bytes_pattern_is_exact_length() {
    use crate::parser::ast::Value;

    // Buffer with no NUL anywhere -- typical ELF header. If the consume
    // path had fallen through to the NUL-scan branch, this would return
    // the full buffer length (16) instead of the pattern length (4).
    let buf: &[u8] = &[
        0x7f, 0x45, 0x4c, 0x46, // \x7fELF
        0x02, 0x01, 0x01, 0x00, // ELF metadata
        0x00, 0x00, 0x00, 0x00, // padding
        0x00, 0x00, 0x00, 0x00, // padding
    ];
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    let pattern = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);

    let consumed = bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern));
    assert_eq!(
        consumed, 4,
        "Bytes pattern of length 4 must consume exactly 4 bytes, not the NUL-scan length"
    );

    // Buffer-overrun case: pattern longer than remaining buffer -> 0.
    let short_buf: &[u8] = &[0x7f, 0x45];
    assert_eq!(
        bytes_consumed_with_pattern(short_buf, 0, &typ, Some(&pattern)),
        0,
        "Bytes pattern longer than buffer must return 0 (overrun)"
    );

    // Offset overflow case.
    assert_eq!(
        bytes_consumed_with_pattern(buf, usize::MAX, &typ, Some(&pattern)),
        0,
        "usize::MAX offset must return 0 via checked_add"
    );
}

/// R6 sanity control: an unflagged `string` match has no flag-walk
/// divergence, so the pattern's declared length and the file bytes
/// consumed always coincide. This is unaffected by the R6 fix -- listed
/// here so the invariant is explicit, not merely implied by other tests.
#[test]
fn test_bytes_consumed_unflagged_string_pattern_walk_equals_declared_length() {
    let buf = b"HELLOxyz";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    let pattern = Value::String("HELLO".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 5);
}

/// R7 (measured against real `file`-5.41; `moffset()` in `softmagic.c`
/// never adds a byte for a trailing NUL): the anchor lands ON a NUL that
/// immediately follows an unflagged `string` match, not past it.
#[test]
fn test_bytes_consumed_unflagged_string_pattern_lands_on_immediate_nul() {
    let buf = b"HELLO\0zz";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    };
    let pattern = Value::String("HELLO".to_string());
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        5,
        "anchor must land ON the NUL at index 5, not past it at index 6"
    );
}

/// R6: a flagged `string` match's anchor advance is the pattern's
/// declared length, not `compare_string_with_flags`'s walked-byte
/// count -- even when that walk is shorter than the declared length.
/// Regression guard for the `consumed == 0` shortcut the pre-fix code
/// used to (incorrectly) treat as "no advance": here the /w-optional
/// whitespace character in the pattern walks zero file bytes, so the
/// OLD walked-byte count was 0 even though the rule matched and the
/// pattern's declared length is 1.
#[test]
fn test_bytes_consumed_flagged_string_declared_length_used_even_when_walk_is_shorter() {
    // Pattern " " (a single optional-whitespace char) against a file
    // byte that is NOT whitespace: /w matches trivially (zero file
    // bytes consumed), but the anchor must still advance by the
    // pattern's declared length (1), landing at offset 1 -- not stay at
    // offset 0 as the old walked-byte-count-driven code would.
    let buf = b"X";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default().with_compact_optional_whitespace(true),
    };
    let pattern = Value::String(" ".to_string());
    assert_eq!(
        bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)),
        1,
        "declared pattern length (1) must drive the advance, not the walked count (0)"
    );
}

/// R6/R8: a `/w`-flagged pattern whose declared length exceeds the
/// buffer's remaining bytes must not panic. `bytes_consumed_with_pattern`
/// simply reports the declared length; the caller's `saturating_add`
/// produces an anchor past EOF, and the downstream child read fails
/// gracefully (see the engine-level
/// `test_flagged_string_w_near_end_of_buffer_anchor_does_not_panic_and_child_is_non_match`
/// test for the end-to-end non-match behavior).
#[test]
fn test_bytes_consumed_flagged_string_declared_length_can_exceed_remaining_buffer() {
    // Pattern "#! X" (4 bytes) matches "#!X" (3 bytes) via /w consuming
    // zero optional whitespace. The declared length (4) exceeds the
    // 3-byte buffer, but the function must still return it, not panic
    // or silently clamp to 0.
    let buf = b"#!X";
    let typ = TypeKind::String {
        max_length: None,
        flags: StringFlags::default().with_compact_optional_whitespace(true),
    };
    let pattern = Value::String("#! X".to_string());
    assert_eq!(bytes_consumed_with_pattern(buf, 0, &typ, Some(&pattern)), 4);
}
