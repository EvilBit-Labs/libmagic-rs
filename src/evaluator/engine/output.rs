// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Output-shaping helpers for [`super::evaluate_rules`].
//!
//! The message-bearing predicates decide whether a rule's match (and any
//! descendant matches) actually contributed usable description text, so that
//! a message-less gating rule cannot shadow a later, more specific rule under
//! `stop_at_first_match: true` (see GOTCHAS S13.2). The no-separator helpers
//! attach GNU `file`'s `\b` marker where a `use` or `indirect` re-entry
//! continues the preceding fragment (GOTCHAS S14.4, S14.5).

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

/// The first match that actually renders text, skipping message-less ones.
fn first_rendering_match(matches: &mut [RuleMatch]) -> Option<&mut RuleMatch> {
    matches.iter_mut().find(|m| is_message_bearing(&m.message))
}

/// Prefix the no-separator marker unless the message already carries one.
fn mark_no_separator(target: &mut RuleMatch) {
    if crate::evaluator::strip_no_separator_marker(&target.message).is_none() {
        target.message = format!("\\b{}", target.message);
    }
}

/// Prepend the GNU `file` no-separator marker to the first message-bearing
/// match, returning a new vector.
///
/// Used by a `use` site whose own message carries the marker
/// (`>0 use mach-o-cpu \b`), so `[` + `x86_64` renders `[x86_64` (GOTCHAS
/// S14.4). `indirect` re-entries use the level-gated
/// [`attach_no_separator_if_top_level`] instead.
///
/// The marker is applied to the first match that actually renders text, so a
/// leading message-less match cannot swallow it and leave the separator in
/// place. A match already carrying a marker is left untouched rather than
/// double-marked.
pub(crate) fn attach_no_separator_to_first(mut matches: Vec<RuleMatch>) -> Vec<RuleMatch> {
    if let Some(target) = first_rendering_match(&mut matches) {
        mark_no_separator(target);
    }
    matches
}

/// [`attach_no_separator_to_first`], applied only when the first match that
/// renders text came from a top-level (level-0) rule.
///
/// libmagic's `match()` prints a top-level description with no leading space
/// but spaces a continuation description when `need_separator` is set, and an
/// `indirect` re-entry inherits the caller's flag. So an ID3 tag's
/// `\b, contains:` followed by MPEG ADTS (description on a continuation)
/// renders `contains: MPEG`, while Mach-O's inner top-level rule renders
/// `:Mach-O` (GOTCHAS S14.5).
pub(crate) fn attach_no_separator_if_top_level(mut matches: Vec<RuleMatch>) -> Vec<RuleMatch> {
    if let Some(target) = first_rendering_match(&mut matches).filter(|m| m.level == 0) {
        mark_no_separator(target);
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{TypeKind, Value};

    fn match_with(message: &str) -> RuleMatch {
        RuleMatch::new(
            message.to_string(),
            0,
            0,
            Value::Uint(0),
            TypeKind::Byte { signed: false },
            1.0,
        )
    }

    fn messages(matches: &[RuleMatch]) -> Vec<&str> {
        matches.iter().map(|m| m.message.as_str()).collect()
    }

    #[test]
    fn attach_no_separator_marks_first_message_bearing_match() {
        let out = attach_no_separator_to_first(vec![match_with("MachO"), match_with("x86_64")]);
        assert_eq!(
            messages(&out),
            vec!["\\bMachO", "x86_64"],
            "only the first message-bearing match takes the marker"
        );
    }

    #[test]
    fn attach_no_separator_skips_message_less_leading_matches() {
        // A leading empty / whitespace / marker-only match renders nothing, so
        // it must not swallow the marker and leave the separator in place.
        let out = attach_no_separator_to_first(vec![
            match_with(""),
            match_with("   "),
            match_with("\\b"),
            match_with("MachO"),
        ]);
        assert_eq!(
            messages(&out),
            vec!["", "   ", "\\b", "\\bMachO"],
            "the marker must land on the first match that actually renders text"
        );
    }

    #[test]
    fn attach_no_separator_does_not_double_mark() {
        for already in ["\\b, contains ", "\u{0008}already"] {
            let out = attach_no_separator_to_first(vec![match_with(already)]);
            assert_eq!(
                messages(&out),
                vec![already],
                "a match already carrying a marker must be left untouched"
            );
        }
    }

    #[test]
    fn attach_no_separator_is_a_noop_without_a_message_bearing_match() {
        for input in [vec![], vec![match_with("")], vec![match_with("\\b")]] {
            let expected = messages(&input)
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>();
            let out = attach_no_separator_to_first(input);
            assert_eq!(
                messages(&out),
                expected,
                "nothing to mark leaves the vector unchanged"
            );
        }
    }

    fn match_at_level(message: &str, level: u32) -> RuleMatch {
        RuleMatch {
            level,
            ..match_with(message)
        }
    }

    #[test]
    fn attach_no_separator_if_top_level_is_gated_on_the_first_rendering_match() {
        let cases: &[(&str, Vec<RuleMatch>, Vec<&str>)] = &[
            (
                "level-0 first fragment takes the marker",
                vec![match_at_level("MachO", 0), match_at_level("x86_64", 1)],
                vec!["\\bMachO", "x86_64"],
            ),
            (
                "continuation first fragment stays spaced",
                vec![match_at_level("MPEG ADTS", 1)],
                vec!["MPEG ADTS"],
            ),
            (
                "message-less level-0 match defers to the level-1 fragment",
                vec![match_at_level("", 0), match_at_level("MPEG ADTS", 1)],
                vec!["", "MPEG ADTS"],
            ),
            (
                "level-0 fragment already marked is not doubled",
                vec![match_at_level("\\b[TIFF", 0)],
                vec!["\\b[TIFF"],
            ),
            ("nothing renders", vec![match_at_level("", 0)], vec![""]),
        ];

        for (name, input, expected) in cases {
            let out = attach_no_separator_if_top_level(input.clone());
            assert_eq!(messages(&out), *expected, "case: {name}");
        }
    }
}
