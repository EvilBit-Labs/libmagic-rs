// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Durable coverage for the description-bound / anchor-advance fix (plan
//! `2026-09-21-1127-fix-bounded-inert-magic-descriptions`, unit U6).
//!
//! Before this plan, an any-value `string`/`pstring`/`string16` field
//! rendered the *entire* remaining buffer (no 127-byte bound, no newline
//! stop), and a flagged `string`/`search` parent's relative-offset child
//! anchored on the walked byte count instead of the declared pattern
//! length. U1 through U5 fixed the mechanics and pinned them with unit
//! tests colocated with the code they touch; this file is the outside-in
//! acceptance net, mirroring `tests/jpeg_oracle_tests.rs` (issue #471).
//!
//! Two layers, following that file's shape:
//!
//! 1. `mod hermetic` always runs: rules built with the public
//!    `MagicRule`/`evaluate_rules` API (mirroring
//!    `tests/relative_offset_evaluation.rs`), no `file` binary needed.
//!    Every value here was independently cross-checked against real
//!    `file`-5.41 while writing this file (see `mod oracle` below).
//! 2. `mod oracle` invokes the real `file` binary against small
//!    self-contained magic-text + buffer pairs (via `tempfile`, never a
//!    committed fixture -- this unit touches only this file). Gated on
//!    `file` being present, a SOURCE (not compiled-only) system magic
//!    directory existing (Debian/Ubuntu ship only `magic.mgc` with no
//!    source dir -- see `magic_source_file_count`, mirroring
//!    `tests/jpeg_oracle_tests.rs`), and a `file` version of 5.39 or
//!    later. Below 5.39 the test FAILS rather than skips: the 127-byte
//!    bound is GNU `file`'s `MAXstring - 1`, and `MAXstring` was 96 (not
//!    128) through `file` 5.38, so a silent skip on an old binary would
//!    let this net report success while verifying nothing.
//!
//! Unlike `tests/jpeg_oracle_tests.rs`, `mod oracle` does not stage the
//! system magic directory as a canary: each scenario's magic text
//! produces a distinctive message (`STR=[...]`, `found, anchor=%d`, ...)
//! that `file`'s silent-fallback behavior (a missing/unparseable `-m`
//! file falls back to `ASCII text`/`data`, not an error -- verified
//! empirically) could never coincidentally reproduce, so a canary is
//! redundant. The directory is still required as a readiness gate,
//! matching this repository's existing "real `file` install, not a
//! minimal one" signal, even though its content is never read here.
//!
//! ## The `search/w` field-level baseline divergence (#382), measured
//!
//! `search/32/w` over `"A   Bqz"` measurably gets a trailing
//! `, ASCII text, with no line terminators` from real `file`-5.41 that
//! an otherwise-identical `string/w` scenario does not -- `file`'s own
//! internal bookkeeping for whether a `search` match suppresses its
//! ascmagic fallback pass, not anything this crate controls. This is a
//! live instance of the #382 trailing text-class fragment named in the
//! plan's Scope Boundaries;
//! `oracle::flagged_search_w_anchor_diverges_only_by_known_text_class_fragment`
//! asserts it classifies as a recorded baseline divergence, not a
//! regression, while still pinning rmagic's own field exactly.
//!
//! A `regex` oracle scenario was considered and dropped: GNU `file`
//! compiles `regex` as POSIX ERE (no special meaning for `\` inside a
//! bracket expression), while rmagic uses the Rust `regex` crate's
//! PCRE-like syntax (GOTCHAS S2.7/S2.8). Measured: `[\s\S]*` matches
//! "everything" in Rust regex but only literal `\`/`s`/`S` in POSIX ERE,
//! so no single pattern is a fair fixture for both engines. R5's regex
//! exemption is covered hermetically instead
//! (`hermetic::regex_type_is_exempt_from_bound_and_newline_stop`).

#![allow(clippy::expect_used, clippy::panic)]

use std::num::NonZeroUsize;

use libmagic_rs::evaluator::evaluate_rules;
use libmagic_rs::output::format::format_magic_message;
use libmagic_rs::parser::ast::{
    PStringLengthWidth, RegexCount, RegexFlags, SearchFlags, StringFlags,
};
use libmagic_rs::{
    Endianness, EvaluationConfig, EvaluationContext, MagicDatabase, MagicRule, OffsetSpec,
    Operator, TypeKind, Value,
};

fn string_type() -> TypeKind {
    TypeKind::String {
        max_length: None,
        flags: StringFlags::default(),
    }
}

fn pstring_type() -> TypeKind {
    TypeKind::PString {
        max_length: None,
        length_width: PStringLengthWidth::OneByte,
        length_includes_itself: false,
    }
}

fn string16_le_type() -> TypeKind {
    TypeKind::String16 {
        endian: Endianness::Little,
    }
}

/// UCS-2LE encoding of `"AB\ncd"` plus its NUL terminator, hand-derived
/// once so both layers can share the exact bytes: `A`=0x0041, `B`=0x0042,
/// `\n`=0x000A, `c`=0x0063, `d`=0x0064, each little-endian, followed by
/// the 2-byte NUL terminator `read_string16_any_value` scans for.
const UCS2LE_AB_NL_CD: &[u8] = &[
    0x41, 0x00, 0x42, 0x00, 0x0A, 0x00, 0x63, 0x00, 0x64, 0x00, 0x00, 0x00,
];

mod hermetic {
    use super::{
        EvaluationConfig, EvaluationContext, MagicRule, NonZeroUsize, OffsetSpec, Operator,
        RegexCount, RegexFlags, SearchFlags, StringFlags, TypeKind, UCS2LE_AB_NL_CD, Value,
        evaluate_rules, format_magic_message, pstring_type, string_type, string16_le_type,
    };

    fn cfg() -> EvaluationConfig {
        EvaluationConfig::default().with_stop_at_first_match(false)
    }

    /// Render a single match's message the way `MagicDatabase::build_result`
    /// would for a one-match description.
    fn render(m: &libmagic_rs::evaluator::RuleMatch) -> String {
        format_magic_message(&m.message, &m.value, &m.type_kind)
    }

    /// AE1/R1: a 304-byte single-line any-value field renders exactly 127
    /// bytes (`MAX_DESCRIPTION_FIELD_LEN`, GNU `file`'s `MAXstring - 1`).
    /// Exact equality, not `contains`: a bound that truncates one byte
    /// short or leaks one byte long would still contain the right
    /// substring and pass a weaker check.
    #[test]
    fn string_any_value_304_bytes_renders_127_byte_field() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            string_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "STR=[%s]".to_string(),
        );
        let buffer = vec![b'Q'; 304];
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("any-value string rule must not error");
        assert_eq!(matches.len(), 1, "the any-value rule must match once");

        let rendered = render(&matches[0]);
        let expected = format!("STR=[{}]", "Q".repeat(127));
        assert_eq!(
            rendered, expected,
            "127 'Q' characters, not 126 or 128 -- the bound must be exact"
        );
    }

    /// AE3/R2: an any-value field over a multi-line buffer stops at the
    /// first `\r`/`\n`, matching `file`-5.41's own single-line rendering
    /// (measured; see the module doc and `oracle::any_value_string_...`
    /// below for the same fixture verified against the real binary).
    #[test]
    fn string_any_value_stops_at_first_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            string_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "STR=[%s]".to_string(),
        );
        let buffer = b"ZZZZABC\nSECOND\n";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("any-value string rule must not error");
        assert_eq!(matches.len(), 1);

        let rendered = render(&matches[0]);
        assert_eq!(
            rendered, "STR=[ZZZZABC]",
            "one line only -- 'SECOND' must not appear"
        );
    }

    /// R5: extends the any-value bound/stop to `pstring`. Payload
    /// `"ABC\ndef"` (7 bytes, 1-byte length prefix `0x07`) renders only
    /// its first line.
    #[test]
    fn pstring_any_value_stops_at_first_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        );
        let buffer = b"\x07ABC\ndef";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("any-value pstring rule must not error");
        assert_eq!(matches.len(), 1);

        let rendered = render(&matches[0]);
        assert_eq!(rendered, "PS=[ABC]");
    }

    /// R5: extends the any-value bound/stop to `lestring16` (UCS-2LE).
    /// Payload decodes to `"AB\ncd"`; only `"AB"` renders.
    #[test]
    fn string16_any_value_stops_at_first_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            string16_le_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "S=[%s]".to_string(),
        );
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), UCS2LE_AB_NL_CD, &mut ctx)
            .expect("any-value string16 rule must not error");
        assert_eq!(matches.len(), 1);

        let rendered = render(&matches[0]);
        assert_eq!(rendered, "S=[AB]");
    }

    /// R5: `regex` is exempt from both the 127-byte bound and the
    /// newline stop -- it has its own 8192-byte scan-window cap
    /// (GOTCHAS S2.8) and is a genuinely different render path
    /// (`is_bounded_string_family` in `src/output/format.rs` excludes
    /// it). `[\s\S]*` is a Rust-regex-only idiom (see the module doc for
    /// why this is not oracle-verified against real `file`).
    #[test]
    fn regex_type_is_exempt_from_bound_and_newline_stop() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Regex {
                flags: RegexFlags::default(),
                count: RegexCount::Default,
            },
            Operator::Equal,
            Value::String(r"[\s\S]*".to_string()),
            "RX=[%s]".to_string(),
        );
        let mut buffer = vec![b'D'; 150];
        buffer.push(b'\n');
        buffer.extend_from_slice(b"more-after-newline");
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("regex rule must not error");
        assert_eq!(matches.len(), 1, "the whole-buffer regex must match once");

        let rendered = render(&matches[0]);
        let expected_body = String::from_utf8(buffer).expect("fixture buffer is valid ASCII/UTF-8");
        assert_eq!(
            rendered,
            format!("RX=[{expected_body}]"),
            "regex must render past 127 bytes and across a newline -- capping it \
             here would be a NEW divergence from `file`, not a fix"
        );
    }

    /// R2: an equality-compared rule never gates the newline stop,
    /// regardless of content -- the gate is scoped to any-value and
    /// null-first-byte-pattern ordering comparisons only
    /// (`newline_stop_gate` in `src/evaluator/engine/value_eval.rs`).
    #[test]
    fn equality_compared_rule_with_embedded_newline_does_not_stop() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            string_type(),
            Operator::Equal,
            Value::String("AB\nCD".to_string()),
            "eq=[%s]".to_string(),
        );
        let buffer = b"AB\nCD";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("equality string rule must not error");
        assert_eq!(matches.len(), 1);

        let rendered = render(&matches[0]);
        assert_eq!(
            rendered, "eq=[AB\nCD]",
            "an equality-compared rule must never stop at an embedded newline"
        );
    }

    /// AE2/R6, measured against `file`-5.41: a `/w` (whitespace-optional)
    /// `string` parent's relative-offset child lands at the pattern's
    /// DECLARED length (3, the byte after `"#! "`), not the walked file
    /// length. Buffer `"#!   /bin/xx"` has three spaces where the
    /// pattern declares one optional space, so declared-length (3) and
    /// walked-length (5) genuinely diverge -- this is the case R6 fixes.
    #[test]
    fn flagged_string_w_relative_child_lands_at_declared_pattern_length() {
        let child = MagicRule::new(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Operator::AnyValue,
            Value::Uint(0),
            "anchor".to_string(),
        )
        .with_level(1);
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default().with_compact_optional_whitespace(true),
            },
            Operator::Equal,
            Value::String("#! ".to_string()),
            "shebang".to_string(),
        )
        .with_children(vec![child]);
        let buffer = b"#!   /bin/xx";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&parent), buffer, &mut ctx)
            .expect("flagged string parent + child must not error");
        assert_eq!(matches.len(), 2, "parent and child must both match");
        assert_eq!(
            matches[1].offset, 3,
            "child anchor must be the pattern's declared length (3), not the \
             walked byte count (5)"
        );
    }

    /// AE2/R6, measured against `file`-5.41: a `/w` `search` parent's
    /// relative-offset child also lands at the declared pattern length
    /// (3, the length of the pattern `"A B"`), not the walked match-end (5, over the 3-space
    /// run in `"A   Bqz"`). Mirrors the `string/w` case above for
    /// `search` (R6 fixes both).
    #[test]
    fn flagged_search_w_relative_child_lands_at_declared_pattern_length() {
        let child = MagicRule::new(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Operator::AnyValue,
            Value::Uint(0),
            "anchor".to_string(),
        )
        .with_level(1);
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Search {
                range: NonZeroUsize::new(32),
                flags: SearchFlags::default().with_compact_optional_whitespace(true),
            },
            Operator::Equal,
            Value::String("A B".to_string()),
            "found".to_string(),
        )
        .with_children(vec![child]);
        let buffer = b"A   Bqz";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&parent), buffer, &mut ctx)
            .expect("flagged search parent + child must not error");
        assert_eq!(matches.len(), 2, "parent and child must both match");
        assert_eq!(
            matches[1].offset, 3,
            "child anchor must be the pattern's declared length (3), not the \
             walked match-end (5)"
        );
    }

    /// AE2/R7, measured against `file`-5.41: a pattern immediately
    /// followed by a NUL lands the relative-offset child ON the NUL
    /// (index 5 of `"HELLO\0zz"`), not past it (index 6). The child
    /// itself asserts the byte is exactly `0` (not just that offset 5
    /// exists), so a regression that overshoots to index 6 (`'z'`, not
    /// zero) would drop the child match entirely rather than passing
    /// with a wrong offset.
    #[test]
    fn pattern_followed_by_null_lands_child_on_null() {
        let child = MagicRule::new(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0),
            "on-null".to_string(),
        )
        .with_level(1);
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            string_type(),
            Operator::Equal,
            Value::String("HELLO".to_string()),
            "found".to_string(),
        )
        .with_children(vec![child]);
        let buffer = b"HELLO\0zz";
        let mut ctx = EvaluationContext::new(cfg());
        let matches = evaluate_rules(std::slice::from_ref(&parent), buffer, &mut ctx)
            .expect("unflagged string parent + child must not error");
        assert_eq!(
            matches.len(),
            2,
            "child must match a zero byte at the anchor -- an overshoot to index \
             6 ('z') would fail the child's own Equal(0) comparison and this \
             assertion would catch it as a missing match, not a silent wrong offset"
        );
        assert_eq!(
            matches[1].offset, 5,
            "anchor must land ON the NUL at index 5"
        );
    }
}

mod oracle {
    use std::io::Write as _;
    use std::path::Path;
    use std::process::Command;

    use super::{EvaluationConfig, MagicDatabase, UCS2LE_AB_NL_CD};
    use tempfile::NamedTempFile;

    const SYSTEM_MAGIC_DIR: &str = "/usr/share/file/magic";

    fn has_file_binary() -> bool {
        Command::new("file")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// Count the plain files in `dir`. Mirrors
    /// `tests/jpeg_oracle_tests.rs::magic_source_file_count`: Debian and
    /// Ubuntu ship only the compiled `magic.mgc` and leave the source
    /// directory empty (or absent) -- that is the case this detects.
    fn magic_source_file_count(dir: &Path) -> usize {
        std::fs::read_dir(dir).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().is_file())
                .count()
        })
    }

    /// Parse `file --version`'s first line (`"file-5.41"`, observed
    /// format) into `(major, minor)`. `None` on anything unparseable --
    /// the caller treats that as a hard failure, not a skip (see
    /// `require_oracle_ready`).
    fn file_version() -> Option<(u32, u32)> {
        let output = Command::new("file").arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next()?;
        let version_str = first_line.strip_prefix("file-")?;
        let mut parts = version_str.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor_raw = parts.next()?;
        // The minor component may carry a non-numeric suffix (e.g. a
        // "-rc1" packaging tag); take only the leading digit run.
        let digit_prefix_len = minor_raw
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(minor_raw.len());
        let minor: u32 = minor_raw.get(..digit_prefix_len)?.parse().ok()?;
        Some((major, minor))
    }

    enum OracleReadiness {
        Ready,
        Skip(String),
    }

    /// Readiness gate mirroring `tests/jpeg_oracle_tests.rs`'s shape,
    /// plus the version check this plan's Dependencies require: the
    /// 127-byte bound is GNU `file`'s `MAXstring - 1`, and `MAXstring`
    /// was 96 (not 128) through `file` 5.38. A version below 5.39 FAILS
    /// here rather than skipping -- skipping would let this test report
    /// success without ever validating the bound it exists to pin.
    fn require_oracle_ready() -> OracleReadiness {
        if !has_file_binary() {
            return OracleReadiness::Skip("`file` is not on PATH".to_string());
        }
        let dir = Path::new(SYSTEM_MAGIC_DIR);
        if !dir.is_dir() {
            return OracleReadiness::Skip(format!("{SYSTEM_MAGIC_DIR} is not present"));
        }
        if magic_source_file_count(dir) == 0 {
            return OracleReadiness::Skip(format!(
                "{SYSTEM_MAGIC_DIR} holds no source magic files (compiled-only \
                 install, e.g. Debian/Ubuntu shipping only magic.mgc)"
            ));
        }
        let version = file_version().unwrap_or_else(|| {
            panic!(
                "could not parse `file --version` output -- refusing to silently \
                 skip. The 127-byte bound assumes `file` 5.39+ (see this module's \
                 doc); an unverifiable version must fail loudly so the assumption \
                 is never silently unvalidated."
            )
        });
        assert!(
            version >= (5, 39),
            "oracle `file` reports version {version:?}, older than 5.39. \
             `MAXstring` was 96 through file 5.38 and became 128 at 5.39 (this \
             crate's MAX_DESCRIPTION_FIELD_LEN = 127 = MAXstring - 1 assumes \
             5.39+). Comparing against an older binary would silently assert the \
             wrong bound -- fix the test environment rather than relaxing this."
        );
        OracleReadiness::Ready
    }

    /// `true` if the oracle is ready; prints a `SKIP:` line and returns
    /// `false` otherwise, for the common `if !gate() { return; }`
    /// early-exit at the top of every test in this module.
    fn gate() -> bool {
        match require_oracle_ready() {
            OracleReadiness::Ready => true,
            OracleReadiness::Skip(reason) => {
                eprintln!("SKIP: {reason} -- oracle test skipped cleanly");
                false
            }
        }
    }

    /// rmagic's own answer for `magic_text` evaluated against `buffer`.
    fn ours_says(magic_text: &str, buffer: &[u8]) -> String {
        let db = MagicDatabase::load_from_bytes_with_config(
            magic_text.as_bytes().to_vec(),
            EvaluationConfig::default(),
        )
        .unwrap_or_else(|e| panic!("magic text must load: {e}\n---\n{magic_text}"));
        db.evaluate_buffer(buffer)
            .unwrap_or_else(|e| panic!("evaluate_buffer must not error: {e}"))
            .description
    }

    /// Real `file`'s answer for the same magic text and buffer, via two
    /// temp files (never a committed fixture -- this unit touches only
    /// this test file). See the module doc for why no directory-staging
    /// canary is needed here: each scenario's message is distinctive
    /// enough that `file`'s silent-fallback behavior (verified
    /// empirically: a missing/unparseable `-m` file falls back to
    /// `ASCII text`/`data`, not an error) could never coincidentally
    /// reproduce it.
    fn file_says(magic_text: &str, buffer: &[u8]) -> String {
        let mut magic_f = NamedTempFile::new().expect("temp magic file");
        magic_f
            .write_all(magic_text.as_bytes())
            .expect("write temp magic");
        magic_f.flush().expect("flush temp magic");
        let mut buf_f = NamedTempFile::new().expect("temp buffer file");
        buf_f.write_all(buffer).expect("write temp buffer");
        buf_f.flush().expect("flush temp buffer");

        let output = Command::new("file")
            .arg("-b")
            .arg("-m")
            .arg(magic_f.path())
            .arg(buf_f.path())
            .output()
            .expect("invoking `file` must not fail once it is known present");
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string()
    }

    /// Suffixes GNU `file`'s ascmagic text-class fallback is measured to
    /// append after an otherwise-complete magic match (issue #382,
    /// recorded out of scope by the plan's Scope Boundaries -- see this
    /// module's doc for the specific `search/32/w` case that surfaces
    /// it). Recorded as a MEASURED baseline, not a theory.
    const KNOWN_TEXT_CLASS_SUFFIXES: &[&str] =
        &[", ASCII text", ", ASCII text, with no line terminators"];

    enum Divergence {
        Unchanged,
        /// `theirs` extends `ours` with one of `KNOWN_TEXT_CLASS_SUFFIXES`.
        KnownTextClassFragment,
        /// Anything else. Never silently accepted as a baseline --
        /// "record the current out-of-scope divergences ... so only a
        /// delta outside that baseline counts as a regression" (plan U6
        /// step 3) means an unrecognized divergence is exactly the thing
        /// this classifier exists to catch, not wave through.
        Unclassified,
    }

    fn classify(ours: &str, theirs: &str) -> Divergence {
        if ours == theirs {
            return Divergence::Unchanged;
        }
        if let Some(rest) = theirs.strip_prefix(ours)
            && KNOWN_TEXT_CLASS_SUFFIXES.contains(&rest)
        {
            return Divergence::KnownTextClassFragment;
        }
        Divergence::Unclassified
    }

    /// AE1/R1, verified against the real oracle: the 304-byte-field
    /// scenario's rendered length is exactly `file`-5.41's own
    /// `MAXstring - 1` bound, not just rmagic's internal notion of it.
    #[test]
    fn any_value_string_304_bytes_matches_gnu_file_127_byte_bound() {
        if !gate() {
            return;
        }
        let magic = "0\tstring\tx\tSTR=[%s]\n";
        let buffer = vec![b'Q'; 304];
        let ours = ours_says(magic, &buffer);
        let theirs = file_says(magic, &buffer);
        assert_eq!(
            ours, theirs,
            "rmagic and `file`-5.41 must agree on the exact 127-byte bound"
        );
        assert_eq!(ours, format!("STR=[{}]", "Q".repeat(127)));
    }

    /// AE3/R2, verified against the real oracle: the newline-stop
    /// scenario renders exactly what `file`-5.41 renders.
    #[test]
    fn any_value_string_renders_single_line_matching_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tstring\tx\tSTR=[%s]\n";
        let buffer = b"ZZZZABC\nSECOND\n";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);
        assert_eq!(ours, theirs);
        assert_eq!(ours, "STR=[ZZZZABC]");
    }

    /// R5, verified against the real oracle: `pstring` any-value
    /// newline stop.
    #[test]
    fn pstring_any_value_matches_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tpstring\tx\tPS=[%s]\n";
        let buffer = b"\x07ABC\ndef";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);
        assert_eq!(ours, theirs);
        assert_eq!(ours, "PS=[ABC]");
    }

    /// R5, verified against the real oracle: `lestring16` any-value
    /// newline stop.
    #[test]
    fn string16_any_value_matches_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tlestring16\tx\tS=[%s]\n";
        let ours = ours_says(magic, UCS2LE_AB_NL_CD);
        let theirs = file_says(magic, UCS2LE_AB_NL_CD);
        assert_eq!(ours, theirs);
        assert_eq!(ours, "S=[AB]");
    }

    /// AE2/R6, verified against the real oracle: a flagged `string/w`
    /// parent's relative child anchors at the declared pattern length
    /// (3), and `file`-5.41 agrees byte for byte -- no #382 tail here
    /// (see the module doc for why `search/w` below differs).
    #[test]
    fn flagged_string_w_anchor_matches_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tstring/w\t#!\\ \tshebang\n>&0\tbyte\tx\t\\b, anchor=%d\n";
        let buffer = b"#!   /bin/xx";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);
        assert_eq!(
            ours, theirs,
            "no known-divergence exception applies to this scenario"
        );
        assert_eq!(
            ours, "shebang, anchor=32",
            "32 is the space byte at index 3 -- the pattern's declared length, \
             not the walked index 5"
        );
    }

    /// AE2/R6, verified against the real oracle, and the measured #382
    /// instance documented in this module's doc: a flagged `search/w`
    /// parent's relative child anchors at the declared pattern length
    /// (3) just like the `string/w` case above, but `file`-5.41 also
    /// appends a trailing ascmagic text-class fragment that an
    /// otherwise-identical `string/w` scenario does not get. That
    /// fragment is the named #382 baseline divergence (plan Scope
    /// Boundaries); this test asserts it is classified as such and NOT
    /// treated as a regression, while still pinning rmagic's own field
    /// exactly.
    #[test]
    fn flagged_search_w_anchor_diverges_only_by_known_text_class_fragment() {
        if !gate() {
            return;
        }
        let magic = "0\tsearch/32/w\tA\\ B\tfound\n>&0\tbyte\tx\t\\b, anchor=%d\n";
        let buffer = b"A   Bqz";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);

        assert_eq!(
            ours, "found, anchor=32",
            "32 is the space byte at index 3 -- the pattern's declared length, \
             not the walked index 5. This is rmagic's own field and must be \
             right regardless of the #382 divergence below."
        );
        match classify(&ours, &theirs) {
            Divergence::Unchanged => panic!(
                "the measured #382 tail is no longer present (`file` says \
                 {theirs:?}) -- if this scenario now matches exactly, tighten \
                 this test to assert plain equality instead of carrying a stale \
                 exception"
            ),
            Divergence::KnownTextClassFragment => {
                // Expected: the recorded #382 baseline, out of scope for this
                // plan.
            }
            Divergence::Unclassified => panic!(
                "regression: `file` says {theirs:?}, rmagic says {ours:?}, and \
                 the divergence does not match the recorded #382 baseline"
            ),
        }
    }

    /// AE2/R7, verified against the real oracle: a pattern immediately
    /// followed by a NUL lands the child ON the NUL (byte value 0), not
    /// past it.
    #[test]
    fn pattern_followed_by_null_matches_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tstring\tHELLO\tfound\n>&0\tbyte\tx\t\\b, anchor=%d\n";
        let buffer = b"HELLO\0zz";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);
        assert_eq!(ours, theirs);
        assert_eq!(
            ours, "found, anchor=0",
            "0 is the NUL byte's own value at index 5, confirming the anchor \
             landed exactly there and not at index 6 ('z' = 122)"
        );
    }

    /// R2, verified against the real oracle: equality comparison never
    /// stops at an embedded newline.
    #[test]
    fn equality_with_embedded_newline_matches_gnu_file() {
        if !gate() {
            return;
        }
        let magic = "0\tstring\tAB\\nCD\teq=[%s]\n";
        let buffer = b"AB\nCD";
        let ours = ours_says(magic, buffer);
        let theirs = file_says(magic, buffer);
        assert_eq!(ours, theirs);
        assert_eq!(ours, "eq=[AB\nCD]");
    }

    /// The defect's direct signature (plan Success Criteria / U6 step
    /// 5): none of this fix's scenarios -- including the one that used
    /// to render the whole remaining multi-line buffer -- produce a
    /// multi-line rmagic description. This is the clean, stable
    /// invariant the plan asks this differential to assert explicitly,
    /// independent of row-level `file` comparison.
    #[test]
    fn no_scenario_renders_a_multiline_description() {
        if !gate() {
            return;
        }
        let scenarios: [(&str, &[u8]); 3] = [
            ("0\tstring\tx\tSTR=[%s]\n", b"ZZZZABC\nSECOND\n"),
            ("0\tpstring\tx\tPS=[%s]\n", b"\x07ABC\ndef"),
            ("0\tlestring16\tx\tS=[%s]\n", UCS2LE_AB_NL_CD),
        ];
        for (magic, buffer) in scenarios {
            let ours = ours_says(magic, buffer);
            assert!(
                !ours.contains('\n') && !ours.contains('\r'),
                "rmagic's own description must never be multi-line for magic \
                 {magic:?} -- this is the defect's direct signature (an \
                 unbounded any-value read rendering the whole multi-line file). \
                 Got: {ours:?}"
            );
        }
    }
}
