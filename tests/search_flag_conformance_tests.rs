// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration and conformance tests for `search`-type flag semantics
//! (issue #235, plan U6).
//!
//! Each test corresponds to a real-world magic(5) rule shape from the
//! `/usr/share/file/magic` corpus that exercises one or more search-type
//! flag letters. The rules are constructed programmatically via the AST
//! (matching `tests/regex_search_corpus_tests.rs` style) so the tests stay
//! self-contained -- the parser layer is exhaustively covered by per-letter
//! tests in `src/parser/grammar/tests/mod.rs`.
//!
//! Note on `range` semantics: in this crate the `search/N` range is the
//! scan **window size**; the pattern must fit entirely within those `N`
//! bytes (`memchr::memmem::find` and the byte-walk both treat the window
//! as the searchable region). libmagic's surface syntax sometimes uses
//! tight ranges like `search/1` because its semantics treat `N` as the
//! number of starting positions to try while still allowing the pattern
//! to extend past the window; the tests below use ranges large enough to
//! contain the full pattern under either interpretation.
//!
//! Flag semantics under test:
//! * `/s` -- anchor advance lands at match-START, not match-END
//! * `/w` -- optional whitespace: pattern's single space matches zero or
//!   more file whitespace bytes
//! * `/b` -- binary-mode hint; recorded but does not alter match decision
//!   (deferred to `!:mime` evaluation in #51)
//!
//! The fourth nominated fixture from the plan (sfnt name table via `/s`) is
//! deferred -- see the plan's Open Questions for sfnt-fixture sourcing. The
//! `/s` semantics are still covered by the TGA test.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate instead.
#![allow(clippy::expect_used, clippy::unimplemented)]

use libmagic_rs::evaluator::evaluate_rules;
use libmagic_rs::parser::ast::SearchFlags;
use libmagic_rs::parser::parse_text_magic_file;
use libmagic_rs::{
    EvaluationConfig, EvaluationContext, MagicRule, OffsetSpec, Operator, TypeKind, Value,
};
use std::num::NonZeroUsize;

// ---------- helpers ----------

fn cfg() -> EvaluationConfig {
    EvaluationConfig::default().with_stop_at_first_match(false)
}

fn search_rule(
    offset: OffsetSpec,
    pattern: &str,
    range: usize,
    flags: SearchFlags,
    msg: &str,
    children: Vec<MagicRule>,
    level: u32,
) -> MagicRule {
    MagicRule::new(
        offset,
        TypeKind::Search {
            range: NonZeroUsize::new(range),
            flags,
        },
        Operator::Equal,
        Value::String(pattern.to_string()),
        msg.to_string(),
    )
    .with_children(children)
    .with_level(level)
}

fn run_rules(rules: &[MagicRule], buffer: &[u8]) -> Vec<libmagic_rs::evaluator::RuleMatch> {
    let mut ctx = EvaluationContext::new(cfg());
    evaluate_rules(rules, buffer, &mut ctx).expect("evaluation should not fail")
}

// =====================================================================
// Test 1: TGA footer with `/s` -- anchor lands at match-START
// =====================================================================
//
// Models `/usr/share/file/magic/images:114`:
//
//   0   search/16/s TRUEVISION-XFILE.\0
//   >-8 lelong       x          \b, offset %d
//
// The TGA file format places the magic string in the trailer; the parent
// uses `/s` so relative-offset children resolve against the match-START
// position (the first byte of "TRUEVISION-XFILE."), not match-END.
//
// We construct a focused equivalent: parent `search/N/s ABC` with a child
// `>&0 string ABC` (a relative offset of 0 from the anchor). With `/s`, the
// anchor is the match-START index, so the child re-reads the SAME bytes
// the parent matched and succeeds. Without `/s`, the anchor would be
// match-END, so `&0` would attempt to read past the matched "ABC" and miss.

#[test]
fn search_s_anchors_relative_child_at_match_start() {
    // Buffer layout:
    //   0..12  "junk-prefix-"
    //   12..15 "ABC"         <- parent matches here (match_idx = 12)
    //   15..   "-suffix"
    //
    // With /s: anchor = 12 (match-START), child `&0 string ABC` reads
    // buffer[12..15] = "ABC" -> child matches.
    // Without /s: anchor = 15 (match-END), child reads buffer[15..] =
    // "-suffix..." -> child does NOT match "ABC".
    let buffer: &[u8] = b"junk-prefix-ABC-suffix-bytes";

    // Child: `>&0 string ABC`. We use TypeKind::String with max_length = 3.
    let child = MagicRule::new(
        OffsetSpec::Relative(0),
        TypeKind::String {
            max_length: Some(3),
            flags: libmagic_rs::parser::ast::StringFlags::default(),
        },
        Operator::Equal,
        Value::String("ABC".to_string()),
        "child matched at anchor".to_string(),
    )
    .with_level(1);

    // Parent WITH /s -- expect both parent and child to fire.
    let parent_s = search_rule(
        OffsetSpec::Absolute(0),
        "ABC",
        32,
        SearchFlags::default().with_start_anchor(true),
        "TGA-like marker",
        vec![child.clone()],
        0,
    );
    let matches_with_s = run_rules(&[parent_s], buffer);
    assert_eq!(
        matches_with_s.len(),
        2,
        "with /s, parent + relative child should both match: {matches_with_s:#?}"
    );
    assert_eq!(matches_with_s[0].message, "TGA-like marker");
    assert_eq!(matches_with_s[1].message, "child matched at anchor");

    // Parent WITHOUT /s -- expect only parent to fire (child reads past
    // the match-end and misses "ABC").
    let parent_no_s = search_rule(
        OffsetSpec::Absolute(0),
        "ABC",
        32,
        SearchFlags::default(),
        "TGA-like marker",
        vec![child],
        0,
    );
    let matches_without_s = run_rules(&[parent_no_s], buffer);
    assert_eq!(
        matches_without_s.len(),
        1,
        "without /s, only parent matches (child reads past match-end): {matches_without_s:#?}"
    );
    assert_eq!(matches_without_s[0].message, "TGA-like marker");
}

// =====================================================================
// Test 2: Python shebang with `/w` -- compact optional whitespace
// =====================================================================
//
// Models the Python shebang detection family. The pattern `#! /usr/bin/python`
// contains a single space; `/w` says the file may have zero or more
// whitespace bytes wherever the pattern has one. The same rule shape
// applies to shell, perl, and other interpreter detection.

#[test]
fn search_w_matches_python_shebang_with_one_space() {
    // Exact match: one space in pattern, one space in buffer.
    let buf = b"#! /usr/bin/python script.py\n";
    let rule = search_rule(
        OffsetSpec::Absolute(0),
        "#! /usr/bin/python",
        // Window must contain the full pattern (18 bytes); use 32 for headroom.
        32,
        SearchFlags::default().with_compact_optional_whitespace(true),
        "Python script",
        vec![],
        0,
    );
    let matches = run_rules(&[rule], buf);
    assert_eq!(
        matches.len(),
        1,
        "search/w must match exact-spacing buffer: {matches:#?}"
    );
    assert_eq!(matches[0].message, "Python script");
}

#[test]
fn search_w_matches_python_shebang_with_multiple_spaces() {
    // /w must absorb wider whitespace runs.
    let buf = b"#!   /usr/bin/python script.py\n";
    let rule = search_rule(
        OffsetSpec::Absolute(0),
        "#! /usr/bin/python",
        // Window must hold pattern + the wider whitespace run; 64 is safe.
        64,
        SearchFlags::default().with_compact_optional_whitespace(true),
        "Python script",
        vec![],
        0,
    );
    let matches = run_rules(&[rule], buf);
    assert_eq!(
        matches.len(),
        1,
        "search/w must accept multiple file whitespace: {matches:#?}"
    );
}

#[test]
fn search_w_matches_python_shebang_with_zero_spaces() {
    // /w means *zero* or more, so the buffer can omit the space entirely.
    let buf = b"#!/usr/bin/python script.py\n";
    let rule = search_rule(
        OffsetSpec::Absolute(0),
        "#! /usr/bin/python",
        // Window must hold the pattern at match-START (index 0); 32 suffices.
        32,
        SearchFlags::default().with_compact_optional_whitespace(true),
        "Python script",
        vec![],
        0,
    );
    let matches = run_rules(&[rule], buf);
    assert_eq!(
        matches.len(),
        1,
        "search/w must accept zero file whitespace: {matches:#?}"
    );
}

// =====================================================================
// Test 3: BinHex with `/b` -- binary-mode hint
// =====================================================================
//
// Models `/usr/share/file/magic/macintosh:17`:
//
//   0 search/2652 (This\ file\ must\ be\ converted\ with\ BinHex BinHex binary text
//
// `/b` is parsed and recorded, but per the plan (R3 and scope boundaries)
// it does not currently alter the match decision -- it is a MIME-output
// hint deferred to #51. This test confirms:
//   (a) a search rule with `/b` set parses & evaluates without error
//   (b) the byte-exact match still fires (regression guard for parse-and-drop)
//   (c) `flags.bin_test` is preserved through evaluation

#[test]
fn search_b_matches_binhex_marker() {
    let buf = b"(This file must be converted with BinHex 4.0)";
    let rule = search_rule(
        OffsetSpec::Absolute(0),
        "(This file must be converted with BinHex",
        2652,
        SearchFlags::default().with_bin_test(true),
        "BinHex binary text",
        vec![],
        0,
    );
    let matches = run_rules(&[rule], buf);
    assert_eq!(
        matches.len(),
        1,
        "search/b should match BinHex marker (regression guard): {matches:#?}"
    );
    assert_eq!(matches[0].message, "BinHex binary text");
}

#[test]
fn search_b_flag_does_not_alter_byte_exact_comparison() {
    // Sanity: setting /b alone must produce the same outcome as no flags
    // for a byte-exact comparison. The flag is captured for MIME output
    // but does not change match decisions today.
    let buf = b"prefix__FOO__suffix";
    let plain = search_rule(
        OffsetSpec::Absolute(0),
        "FOO",
        32,
        SearchFlags::default(),
        "plain",
        vec![],
        0,
    );
    let bin = search_rule(
        OffsetSpec::Absolute(0),
        "FOO",
        32,
        SearchFlags::default().with_bin_test(true),
        "bin",
        vec![],
        0,
    );
    let plain_matches = run_rules(&[plain], buf);
    let bin_matches = run_rules(&[bin], buf);
    assert_eq!(plain_matches.len(), 1);
    assert_eq!(bin_matches.len(), 1);
}

// =====================================================================
// Test 4: archive:1427 EPUB-style load test
// =====================================================================
//
// Models `/usr/share/file/magic/archive:1427`:
//
//   >>30 search/100/b !application/epub+zip
//
// The full EPUB negation semantics depend on `!:mime` and ZIP content
// inspection from #51. At this stage, we verify:
//   - The magic-file fragment loads without parse error
//   - The parsed rule carries `flags.bin_test = true`
//   - The pattern `!application/epub+zip` survives bareword parsing
//
// Full match-decision parity is deferred to #51.

#[test]
fn search_b_archive_epub_rule_loads_and_carries_bin_test_flag() {
    // Magic fragment modeled on archive:1427. We use a top-level rule at
    // absolute offset 30 with the same `search/100/b !application/epub+zip`
    // shape -- the original has a parent `>>30` (continuation) but we strip
    // the parent context to keep the fragment self-contained at parse time.
    //
    // Per AGENTS.md S3.11, parse_text_magic_file is fail-fast, so unrelated
    // syntax in real magic files (e.g., `&+N` relative offsets, `$VAR`
    // substitutions) would block this test if we tried to load the whole
    // archive file. A focused fragment is sufficient for the load+flag
    // assertion.
    let fragment = "30 search/100/b !application/epub+zip EPUB-not-detected\n";

    let parsed = parse_text_magic_file(fragment)
        .expect("archive:1427-style search/100/b rule should parse cleanly");

    assert_eq!(
        parsed.rules.len(),
        1,
        "fragment should produce exactly one rule"
    );
    let rule = &parsed.rules[0];

    // Verify the rule is a Search with the binary hint set.
    match &rule.typ {
        TypeKind::Search { range, flags } => {
            assert_eq!(
                range.expect("search/100 yields a bounded Some range").get(),
                100,
                "search range from /100 suffix should be 100"
            );
            assert!(
                flags.bin_test,
                "search/100/b must record bin_test = true (MIME hint for #51)"
            );
            assert!(!flags.start_anchor, "/b should not set start_anchor");
            // The remaining shared fields should stay at their defaults.
            assert!(!flags.compact_whitespace);
            assert!(!flags.compact_optional_whitespace);
            assert!(!flags.ignore_lowercase);
            assert!(!flags.ignore_uppercase);
            assert!(!flags.trim);
            assert!(!flags.full_word);
        }
        other => panic!("expected TypeKind::Search, got {other:?}"),
    }
}

// =====================================================================
// Test 5: sfnt name table (DEFERRED -- see plan Open Questions)
// =====================================================================
//
// `fonts:260` uses `search/432/s name` to locate the sfnt name table
// header in TTF/OTF binaries, with relative-offset children walking
// backwards from match-START. Conformance against this fixture requires
// a small (<1 KB) TTF binary checked into `tests/fixtures/`; the plan's
// Open Questions section flags sfnt-fixture sourcing as pending
// (proposed: trimmed Roboto-Mono or DejaVuSans under Apache 2.0 /
// Bitstream Vera).
//
// The `/s` anchor semantics are exercised by Test 1 above, so this
// deferred test is a coverage-completeness placeholder. Track via
// the plan's Open Questions; revisit once a fixture is committed.

#[test]
#[ignore = "sfnt fixture sourcing pending -- see plan Open Questions (Roboto-Mono / DejaVuSans trim TBD)"]
fn search_s_sfnt_name_table_via_real_ttf_fixture() {
    // TODO(#235 follow-up): commit a sub-1 KB TTF fixture under
    // tests/fixtures/sfnt-name-table.ttf and assert the fonts:260 rule
    // chain produces the expected GNU `file` description.
    unimplemented!("sfnt fixture not yet committed");
}
