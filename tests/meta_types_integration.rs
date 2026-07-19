// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end smoke tests for meta-type directives
//! (name/use/default/clear/indirect/offset).
//!
//! Uses the canonical GNU `file` `searchbug.magic` fixture, which exercises
//! the `name`/`use` subroutine machinery together with `offset`, `search/N`,
//! and relative-offset (`&N`) semantics. All six meta-type variants are fully
//! evaluated; `test_searchbug_matches_full_result_string` verifies the
//! byte-for-byte output against `searchbug.result` including the `offset`
//! pseudo-type's printf-style format substitution.

use std::fs;
use std::io::Write;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::TempDir;

#[test]
fn test_searchbug_magic_loads_end_to_end() {
    // Regression: the canonical GNU `file` testfile `searchbug.magic`
    // exercises the `name`/`use` subroutine machinery together with
    // `offset`, `search/N`, and relative-offset (`&N`) semantics. Before
    // meta-type parsing was wired through, this file failed to load at
    // all (the parser rejected the `offset` and `name`/`use` keywords).
    //
    // The assertion is intentionally loose: evaluation of the top-level
    // `string TEST` rule today returns "data" on buffers that contain no
    // NUL bytes (see GOTCHAS S6.4 -- unanchored string rules without an
    // explicit `/N` length cap read the entire remaining buffer). That is
    // orthogonal to meta-type handling and is tracked separately. The
    // point of this smoke test is to prove that the fixture parses and
    // can be evaluated without panicking or erroring.
    let db = MagicDatabase::load_from_file("third_party/tests/searchbug.magic")
        .expect("searchbug.magic must load end-to-end");
    let bytes = std::fs::read("third_party/tests/searchbug.testfile")
        .expect("searchbug.testfile fixture must exist");

    let result = db
        .evaluate_buffer(&bytes)
        .expect("evaluate_buffer on searchbug.testfile");

    // A non-empty description is the minimum smoke-test bar.
    assert!(
        !result.description.is_empty(),
        "evaluation should produce some description"
    );

    // The top-level `string TEST` rule carries the "Testfmt" message, so
    // any correctly-evaluated run must produce a description that starts
    // with "Testfmt". This prefix guards the primary regression target
    // of this fixture (name/use subroutine dispatch plus continuation
    // rules) -- the weaker non-empty check alone can pass even when
    // `use`-site children are silently skipped.
    assert!(
        result.description.starts_with("Testfmt"),
        "description should start with \"Testfmt\", got: {}",
        result.description
    );
}

/// Synthetic end-to-end coverage of the `default` and `clear` directives:
///
/// - When no sibling rule has matched at the current level, a `default`
///   rule must fire and contribute its message to the description.
/// - When a sibling has matched, a `default` rule must remain silent.
/// - A `clear` directive resets the per-level "sibling matched" flag, so a
///   subsequent `default` sibling at the same level can fire again even
///   after an earlier sibling matched.
///
/// The combined scenario walks the sequence
/// `[match-A, default-skipped, clear, default-fires]` to prove `clear`
/// changes runtime sibling-matched state end-to-end through the full
/// `MagicDatabase` load/evaluate flow.
#[test]
fn test_default_clear_synthetic_scenario() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("default.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    // Real rule fires when first byte is 0xAA. The default fires when
    // nothing else matched at this level. Trailing message fields show up
    // in the concatenated description.
    writeln!(f, r"0 byte 0xAA Real-Match").unwrap();
    writeln!(f, r"0 default x DEFAULT-FALLBACK").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Buffer that does NOT trigger the byte rule -> default must fire.
    let buf_no_match = [0x00u8, 0x01, 0x02, 0x03];
    let result_default = db.evaluate_buffer(&buf_no_match).unwrap();
    assert!(
        result_default.description.contains("DEFAULT-FALLBACK"),
        "default should fire when no sibling matched, got: {}",
        result_default.description
    );

    // Buffer that DOES trigger the byte rule -> default must remain silent.
    let buf_match = [0xAAu8, 0x01, 0x02, 0x03];
    let result_real = db.evaluate_buffer(&buf_match).unwrap();
    assert!(
        !result_real.description.contains("DEFAULT-FALLBACK"),
        "default must not fire when a sibling matched, got: {}",
        result_real.description
    );
    assert!(
        result_real.description.contains("Real-Match"),
        "real byte rule should still match, got: {}",
        result_real.description
    );

    // Now exercise `clear` end-to-end: after a sibling matches (Match-A),
    // the first `default` sibling (DEFAULT-SKIPPED) must stay silent, then
    // `clear` resets the sibling-matched flag so the second `default`
    // sibling (DEFAULT-FIRES) fires despite the earlier match.
    //
    // This walks all top-level siblings, so we must disable
    // `stop_at_first_match` (the default config stops after the first
    // top-level match, which would prevent the later `clear`/`default`
    // siblings from executing).
    let clear_path = temp_dir.path().join("clear.magic");
    let mut cf = fs::File::create(&clear_path).unwrap();
    writeln!(cf, r"0 byte 0xAA Match-A").unwrap();
    writeln!(cf, r"0 default x DEFAULT-SKIPPED").unwrap();
    writeln!(cf, r"0 clear").unwrap();
    writeln!(cf, r"0 default x DEFAULT-FIRES").unwrap();

    let all_matches_config = EvaluationConfig::default().with_stop_at_first_match(false);
    let clear_db =
        MagicDatabase::load_from_file_with_config(&clear_path, all_matches_config).unwrap();

    // Buffer that triggers Match-A. Without `clear`, only Match-A fires
    // and the DEFAULT-SKIPPED is correctly suppressed. With `clear`,
    // Match-A fires, DEFAULT-SKIPPED is suppressed, the clear directive
    // resets sibling_matched, and DEFAULT-FIRES then fires.
    let buf_clear = [0xAAu8, 0x01, 0x02, 0x03];
    let result_clear = clear_db.evaluate_buffer(&buf_clear).unwrap();

    assert!(
        result_clear.description.contains("Match-A"),
        "byte rule should still match before clear, got: {}",
        result_clear.description
    );
    assert!(
        !result_clear.description.contains("DEFAULT-SKIPPED"),
        "default immediately after a sibling match must remain silent, got: {}",
        result_clear.description
    );
    assert!(
        result_clear.description.contains("DEFAULT-FIRES"),
        "clear must reset sibling-matched so a later default can fire, got: {}",
        result_clear.description
    );
}

/// A message-bearing `clear x` directive must EMIT its own message, matching
/// libmagic's `FILE_CLEAR` (the `x` test always succeeds and `mprint` renders
/// a non-empty description). This is the mechanism behind GNU `file`'s
/// `c-lang` chain: for a plain C source file the `>>&0 clear x program text`
/// child appends "program text", producing `c program text` (rmagic
/// previously dropped it and printed only `c`).
///
/// This also pins the coexistence verified against real `file` (file-5.41):
/// the message-bearing clear prints its message AND still resets the per-level
/// sibling-matched flag so a trailing `default` sibling fires. The child-level
/// structure mirrors c-lang -- the directives are children of a matched
/// top-level rule, so each `default`'s sibling-matched check is evaluated at
/// the child level.
#[test]
fn test_clear_with_message_emits_and_still_resets_flag() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("clear_msg.magic");

    // Top-level signature matches, then four children at the same level:
    //   default (fires -- nothing matched at the child level yet),
    //   clear x CLEARED-MSG (emits its message, resets the flag),
    //   default (fires again because clear reset the flag).
    // Verified against real `file`:
    //   MATCH-A DEF-SKIPPED CLEARED-MSG DEF-FIRES
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r"0 string ZQX9 MATCH-A").unwrap();
    writeln!(f, r">4 default x DEF-ONE").unwrap();
    writeln!(f, r">4 clear x CLEARED-MSG").unwrap();
    writeln!(f, r">4 default x DEF-TWO").unwrap();

    // Evaluate every matching sibling (the default config stops at the first
    // top-level match, which would hide the child chain).
    let config = EvaluationConfig::default().with_stop_at_first_match(false);
    let db = MagicDatabase::load_from_file_with_config(&magic_path, config).unwrap();

    let buf = b"ZQX9rest-of-data";
    let result = db.evaluate_buffer(buf).unwrap();

    // Primary regression: the clear's own message text is present.
    assert!(
        result.description.contains("CLEARED-MSG"),
        "message-bearing clear must emit its message, got: {}",
        result.description
    );
    // The first child default fires (no sibling matched at the child level
    // before it).
    assert!(
        result.description.contains("DEF-ONE"),
        "first child default should fire, got: {}",
        result.description
    );
    // Coexistence: clear reset the flag, so the trailing default still fires.
    assert!(
        result.description.contains("DEF-TWO"),
        "clear must reset sibling-matched so the trailing default fires, got: {}",
        result.description
    );
}

/// A bare `clear x` directive with NO message text must remain a pure
/// flag-reset: it emits no description fragment and does not advance the
/// previous-match anchor, exactly as before message-emission was added. This
/// guards the blast radius of `test_clear_with_message_emits_and_still_resets_flag`
/// -- the system magic DB contains ten message-less `clear x` directives
/// (apple, coff, elf, pmem, ...) whose output must be unchanged.
#[test]
fn test_message_less_clear_emits_nothing() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("clear_bare.magic");

    // A matched parent whose only child is a bare `clear x` (no message).
    // The description must be exactly the parent's text with no trailing
    // fragment or stray whitespace from the clear.
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r"0 string ZQX9 PARENT").unwrap();
    writeln!(f, r">4 clear x").unwrap();

    let config = EvaluationConfig::default().with_stop_at_first_match(false);
    let db = MagicDatabase::load_from_file_with_config(&magic_path, config).unwrap();

    let result = db.evaluate_buffer(b"ZQX9rest").unwrap();
    assert_eq!(
        result.description, "PARENT",
        "a message-less clear must contribute no text, got: {}",
        result.description
    );
}

/// Synthetic end-to-end coverage of the `indirect` directive: a rule with
/// `TypeKind::Meta(MetaType::Indirect)` re-applies the loaded magic
/// database starting at the resolved offset. The dispatch is wired
/// through `RuleEnvironment::root_rules`, which `MagicDatabase` populates
/// with the same rule list used at the top level.
#[test]
fn test_indirect_synthetic_scenario() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("indirect.magic");

    // Two rules at the top level:
    //   - At offset 0: byte 0x7F triggers an indirect re-entry at offset 8.
    //     The indirect re-entry then re-applies the root rules against the
    //     sub-buffer starting at byte 8.
    //   - At offset 0: byte 0x42 produces "Inner-Match". When the indirect
    //     fires, the sub-buffer's offset 0 is the outer buffer's offset 8,
    //     so 0x42 there triggers the same rule recursively.
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r"0 byte 0x42 Inner-Match").unwrap();
    writeln!(f, r"8 indirect x").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Build a buffer where:
    //   buf[0]   = 0x00  (no Inner-Match at top level)
    //   buf[8]   = 0x42  (after indirect dispatch, sub-buffer[0] = 0x42)
    let mut buf = vec![0u8; 16];
    buf[8] = 0x42;

    let result = db.evaluate_buffer(&buf).unwrap();
    // The indirect re-entry should produce an Inner-Match for the sub-buffer.
    assert!(
        result.description.contains("Inner-Match"),
        "indirect must dispatch root rules at the resolved offset; got: {}",
        result.description
    );
}

#[test]
fn test_searchbug_matches_full_result_string() {
    // The `searchbug.result` fixture expects the concatenation of every
    // match produced by walking the full rule tree. libmagic-rs's
    // `stop_at_first_match` default is `true`, which causes the
    // evaluator to short-circuit after the first sibling in every
    // nested rule list -- that's the right default for file-type
    // classification but the wrong default for round-tripping magic(5)
    // fixtures that expect every successful rule to surface its
    // message. Disable it here so the fixture's full expected
    // description is produced; GNU `file`'s behavior on this fixture
    // is equivalent to evaluating every branch.
    let config = EvaluationConfig::default().with_stop_at_first_match(false);
    let db = MagicDatabase::load_from_file_with_config("third_party/tests/searchbug.magic", config)
        .expect("searchbug.magic must load end-to-end");
    let bytes = std::fs::read("third_party/tests/searchbug.testfile")
        .expect("searchbug.testfile fixture must exist");
    let expected = std::fs::read_to_string("third_party/tests/searchbug.result")
        .expect("searchbug.result fixture must exist");

    let result = db
        .evaluate_buffer(&bytes)
        .expect("evaluate_buffer on searchbug.testfile");
    assert_eq!(result.description.trim(), expected.trim());
}

/// End-to-end regression for the gzip multi-fragment description, the shape
/// that surfaced four interlocking evaluation bugs (PR #376):
///
/// 1. `stop_at_first_match` must be TOP-LEVEL only -- every matching child
///    sibling under the matched parent must render (not truncate after the
///    first), so `default`, the `use` subroutine detail, and the trailing
///    `offset` size all appear.
/// 2. The `offset` pseudo-type must apply its comparison operator
///    (`offset >48`) against the resolved position, not treat `>48` as
///    message text.
/// 3. `-0` must resolve to the EOF position (`buffer.len()`) via
///    `FromEnd(0)`, so `>>-0 offset >48` gates on file length.
/// 4. Continuation/child rules and `name`-block bodies must stay in FILE
///    order (non-recursive strength sort), so the low-strength `default`
///    is not suppressed by a higher-strength `offset` sibling sorted ahead
///    of it, and the gzip-info detail fragments render in source order.
///
/// Verified byte-for-byte against `file -b` on real gzip files; this test
/// pins a synthetic buffer so the parity survives without the system DB.
/// Uses the default config (`stop_at_first_match: true`) deliberately, to
/// prove children still render fully under first-match short-circuiting.
#[test]
fn test_gzip_multipart_description_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("gzip.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    // Minimal gzip shape mirroring /usr/share/file/magic/compress.
    writeln!(f, r"0	string	\037\213").unwrap();
    writeln!(f, r"# no FNAME/FCOMMENT bit -> binary gzip").unwrap();
    writeln!(
        f,
        r"# (real rule masks with &0x18; a bare `byte 0` suffices here)"
    )
    .unwrap();
    writeln!(f, r"# FLG byte at offset 3").unwrap();
    writeln!(f, r">3	byte	0").unwrap();
    writeln!(f, r">>10	default	x	gzip compressed data").unwrap();
    writeln!(f, r">>>0	use	gzip-info").unwrap();
    // -0 == EOF position; `offset >48` gates the trailing size on file length.
    writeln!(f, r">>-0	offset	>48").unwrap();
    writeln!(f, r">>>-4	ulelong	x	\b, original size modulo 2^32 %u").unwrap();
    writeln!(f, r">>-0	offset	<48	\b, truncated").unwrap();
    writeln!(f, r"0	name	gzip-info").unwrap();
    writeln!(f, r">9	byte	3	\b, from Unix").unwrap();
    drop(f);

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // 64-byte synthetic gzip: magic, CM=deflate, FLG=0, ..., OS=3 (Unix),
    // trailing little-endian original-size = 1000 in the last 4 bytes.
    let mut buf = vec![0u8; 64];
    buf[0] = 0x1f;
    buf[1] = 0x8b;
    buf[2] = 0x08; // CM = deflate
    buf[3] = 0x00; // FLG = 0
    buf[9] = 0x03; // OS = Unix
    buf[60] = 0xE8; // 1000 = 0x000003E8, little-endian in last 4 bytes
    buf[61] = 0x03;

    let result = db.evaluate_buffer(&buf).unwrap();
    assert_eq!(
        result.description, "gzip compressed data, from Unix, original size modulo 2^32 1000",
        "all four fragments must render in file order; got {:?}",
        result.description
    );

    // A short (< 48-byte) buffer must take the `<48` truncated branch, not
    // the size branch -- proving the FromEnd(0)+comparison gate both ways.
    let mut short = vec![0u8; 20];
    short[0] = 0x1f;
    short[1] = 0x8b;
    short[9] = 0x03;
    let short_result = db.evaluate_buffer(&short).unwrap();
    assert!(
        short_result.description.contains("truncated"),
        "a <48-byte gzip must hit the `offset <48` truncated branch; got {:?}",
        short_result.description
    );
    assert!(
        !short_result.description.contains("original size"),
        "the size branch must not fire for a truncated file; got {:?}",
        short_result.description
    );
}
