// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Message-bearing-match predicates used by the `stop_at_first_match` dispatch
//! in [`super::evaluate_rules`].
//!
//! Extracted from `engine/mod.rs` as a pure code-motion split (issue #391 item
//! 1, Unit U3): these two functions decide whether a rule's match (and any
//! descendant matches) actually contributed usable description text, so that
//! a message-less gating rule cannot shadow a later, more specific rule under
//! `stop_at_first_match: true` (see GOTCHAS S13.2).

use super::RuleMatch;

/// Whether `message` carries any usable description text.
///
/// A message is considered message-less (and thus does not count as
/// "producing output") if, after trimming ASCII/Unicode whitespace and
/// stripping a leading GNU `file` no-separator marker (see GOTCHAS S14.1),
/// nothing remains. This covers three shapes GNU `file` magic files use
/// for structural/gating rules that carry no description of their own:
/// a genuinely empty message (`""`), a whitespace-only message, and a
/// `\b`-only message (used purely to suppress a separator when appended
/// to a sibling's text -- with nothing else to append, it contributes no
/// content either).
///
/// The marker is recognized in BOTH forms -- the raw byte `U+0008` and the
/// literal `\b` (backslash + `'b'`) -- via the shared
/// [`crate::evaluator::strip_no_separator_marker`], so this predicate agrees
/// with `concatenate_messages`: a message that renders to empty there (e.g.
/// exactly `"\b"`, the literal marker) is classified message-less here and
/// therefore cannot win the `stop_at_first_match` race and shadow a later,
/// more specific rule that would produce real output (the S13.2 bug class).
pub(crate) fn is_message_bearing(message: &str) -> bool {
    let trimmed = message.trim_matches(|c: char| c.is_whitespace() || c == '\u{8}');
    let stripped = crate::evaluator::strip_no_separator_marker(trimmed).unwrap_or(trimmed);
    !stripped
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{8}')
        .is_empty()
}

/// Whether any match in `matches[from..]` carries usable description text
/// (see [`is_message_bearing`]).
///
/// Used to decide whether a top-level rule's match -- together with any
/// descendant matches produced by its children -- should be treated as
/// the "winning" match for `stop_at_first_match` purposes. GNU `file`
/// magic files commonly use message-less top-level rules purely as
/// gating conditions for child rules (for example the `c-lang` search
/// rules that test for `#include`/`pragma`/etc. before dispatching to a
/// message-bearing regex child); under the old all-or-nothing contract, a
/// message-less rule matching first under `stop_at_first_match: true`
/// would silently shadow a later, more specific rule that actually
/// produces a description (GOTCHAS S13.2, the assembler-source-text /
/// plain-ASCII-text blank-output bug). A rule only "wins" the race if it
/// (or a descendant) contributes real output text; otherwise evaluation
/// continues to the next top-level sibling.
///
/// Takes `from` (the length of `matches` before this rule's dispatch) and
/// slices via `.get()` (rather than the caller indexing `matches[from..]`
/// directly) so this is panic-free per the project's bounds-checking
/// discipline; `from` is always `<= matches.len()` by construction (it is
/// captured from `matches.len()` earlier in the same call), so `.get()`
/// always returns `Some`, but the panic-free form is required regardless.
pub(crate) fn has_message_bearing_match(matches: &[RuleMatch], from: usize) -> bool {
    matches
        .get(from..)
        .is_some_and(|tail| tail.iter().any(|m| is_message_bearing(&m.message)))
}
