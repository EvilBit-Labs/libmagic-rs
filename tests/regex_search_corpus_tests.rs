// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Corpus integration tests for issue #39 — regex and search types
//!
//! This file exercises the regex and search TypeKind variants end-to-end
//! against the test corpus files listed as "blocked" in issue #39:
//!
//! * `searchbug` — exercises `search/N` against a two-match binary buffer
//! * `json1`, `jsonlines1` — JSON text detection via regex
//! * `cmd1` — shell script detection via regex
//! * `gedcom` — GEDCOM genealogy file detection via regex
//!
//! Where a corpus file depends on magic-file features we do not yet
//! support (`use`/`name` directives, `offset` type, the `&+N`/`&-N`
//! parser for relative offsets), the test bypasses `parse_text_magic_file`
//! and builds the equivalent rule tree programmatically via the AST.
//! This pattern is documented in GOTCHAS 3.9.

use libmagic_rs::evaluator::evaluate_rules;
use libmagic_rs::parser::ast::RegexFlags;
use libmagic_rs::{
    EvaluationConfig, EvaluationContext, MagicRule, OffsetSpec, Operator, TypeKind, Value,
};
use std::num::NonZeroUsize;

const CORPUS_DIR: &str = "third_party/tests";

fn load_corpus_file(name: &str) -> Vec<u8> {
    let path = format!("{CORPUS_DIR}/{name}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

/// Run a flat list of rules against a buffer with a permissive config
/// and return the vector of matches for assertion.
fn run_rules(rules: &[MagicRule], buffer: &[u8]) -> Vec<libmagic_rs::evaluator::RuleMatch> {
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    evaluate_rules(rules, buffer, &mut context).expect("evaluation should not fail")
}

fn regex_rule(
    offset: OffsetSpec,
    pattern: &str,
    flags: RegexFlags,
    count: Option<u32>,
    message: &str,
    children: Vec<MagicRule>,
    level: u32,
) -> MagicRule {
    MagicRule {
        offset,
        typ: TypeKind::Regex {
            flags,
            count: count.and_then(std::num::NonZeroU32::new),
        },
        op: Operator::Equal,
        value: Value::String(pattern.to_string()),
        message: message.to_string(),
        children,
        level,
        strength_modifier: None,
    }
}

fn search_rule(
    offset: OffsetSpec,
    pattern: &str,
    range: usize,
    message: &str,
    children: Vec<MagicRule>,
    level: u32,
) -> MagicRule {
    MagicRule {
        offset,
        typ: TypeKind::Search {
            range: NonZeroUsize::new(range).expect("range must be non-zero"),
        },
        op: Operator::Equal,
        value: Value::String(pattern.to_string()),
        message: message.to_string(),
        children,
        level,
        strength_modifier: None,
    }
}

// =====================================================================
// searchbug — search type hierarchical scan
// =====================================================================

/// `searchbug.magic` uses `use`/`name`/`offset`/`&0` features we do not
/// yet parse. The programmatic equivalent here models the same behavior:
/// a `TEST` header at offset 0 triggers a `search/12 "ABC"` scan, and
/// a byte rule reads the character immediately after the `ABC` match
/// (exercising the `Relative(N)` anchor advance after a search).
#[test]
fn test_searchbug_corpus_search_with_relative_child() {
    let buffer = load_corpus_file("searchbug.testfile");
    assert!(buffer.starts_with(b"TEST"), "corpus should begin with TEST");

    // Byte child reading the character immediately after "ABC". In the
    // corpus file the first ABC is `ABC1` at offset 8, so after "ABC"
    // (match-end at 11) the byte at offset 11 is '1' (0x31).
    let after_abc = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'1')),
        message: "followed by 1".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
    };

    // search/12 "ABC" with Relative(0) child.
    let search_abc = search_rule(
        OffsetSpec::Relative(0),
        "ABC",
        12,
        "found ABC",
        vec![after_abc],
        1,
    );

    // Parent: TEST header at offset 0.
    let root = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: Some(4),
        },
        op: Operator::Equal,
        value: Value::String("TEST".to_string()),
        message: "Testfmt".to_string(),
        children: vec![search_abc],
        level: 0,
        strength_modifier: None,
    };

    let matches = run_rules(&[root], &buffer);

    // Expected chain: TEST header -> found ABC -> followed by 1.
    assert_eq!(
        matches.len(),
        3,
        "expected 3 matches (header + search + byte child), got {matches:#?}"
    );
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(messages, ["Testfmt", "found ABC", "followed by 1"]);
}

#[test]
fn test_searchbug_search_anchor_advance_not_window_end() {
    // Regression guard: the search anchor must advance to match-end
    // (8 + 3 = 11), NOT to the window end (first search starts at
    // offset 0, window 12 would land at offset 12). If it advanced to
    // window-end, the Relative(0) child would read byte 12 which is
    // 'x' (0x78), not '1' (0x31).
    let buffer = load_corpus_file("searchbug.testfile");

    let wrong_byte = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'x')),
        message: "window-end bug -- must NOT match".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
    };

    let search_abc = search_rule(
        OffsetSpec::Relative(0),
        "ABC",
        12,
        "found ABC",
        vec![wrong_byte],
        1,
    );

    let root = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: Some(4),
        },
        op: Operator::Equal,
        value: Value::String("TEST".to_string()),
        message: "Testfmt".to_string(),
        children: vec![search_abc],
        level: 0,
        strength_modifier: None,
    };

    let matches = run_rules(&[root], &buffer);
    // Should see Testfmt + found ABC but NOT the wrong_byte child.
    assert_eq!(
        matches.len(),
        2,
        "wrong_byte should not match: {matches:#?}"
    );
    assert_eq!(matches[1].message, "found ABC");
}

// =====================================================================
// json1 / jsonlines1 — JSON text detection via regex
// =====================================================================

/// JSON detection: a buffer starting with `{` or `[` (after optional
/// whitespace) is a JSON document. This is the simplified detection
/// pattern used by libmagic's json.magic for the fast path.
#[test]
fn test_json1_corpus_detected_by_regex() {
    let buffer = load_corpus_file("json1.testfile");

    // `^\s*[\{\[]` — optional leading whitespace followed by an object
    // or array opener. Multi-line mode is always on, so `^` matches the
    // buffer start.
    let json_rule = regex_rule(
        OffsetSpec::Absolute(0),
        r"^\s*[\{\[]",
        RegexFlags::default(),
        None,
        "JSON text data",
        vec![],
        0,
    );

    let matches = run_rules(&[json_rule], &buffer);
    assert_eq!(matches.len(), 1, "json1 should match: {matches:#?}");
    assert_eq!(matches[0].message, "JSON text data");
}

#[test]
fn test_jsonlines1_corpus_detected_by_regex() {
    let buffer = load_corpus_file("jsonlines1.testfile");

    // JSON Lines detection: each line is an independent JSON document
    // so we can reuse the same opener check on the first line.
    let jsonlines_rule = regex_rule(
        OffsetSpec::Absolute(0),
        r"^\s*[\{\[]",
        RegexFlags::default(),
        None,
        "JSON Lines text",
        vec![],
        0,
    );

    let matches = run_rules(&[jsonlines_rule], &buffer);
    assert_eq!(matches.len(), 1, "jsonlines1 should match: {matches:#?}");
}

// =====================================================================
// cmd1 — shell script detection via regex
// =====================================================================

/// Shell script detection: a buffer starting with `#!` is a script. We
/// use a regex anchored at offset 0 to verify the shebang and capture
/// the interpreter path for a stronger match.
#[test]
fn test_cmd1_corpus_detected_by_regex() {
    let buffer = load_corpus_file("cmd1.testfile");

    let shebang_rule = regex_rule(
        OffsetSpec::Absolute(0),
        r"^#![ \t]*/\S+",
        RegexFlags::default(),
        None,
        "a shell script",
        vec![],
        0,
    );

    let matches = run_rules(&[shebang_rule], &buffer);
    assert!(!matches.is_empty(), "cmd1 should match: {matches:#?}");
    assert_eq!(matches[0].message, "a shell script");
}

// =====================================================================
// gedcom — genealogy file detection via regex
// =====================================================================

/// GEDCOM files begin with `0 HEAD` on the first line followed by
/// `1 SOUR <something>` and `2 VERS <version>`. A simple regex on the
/// head line (with the `/l` line limit) is enough to detect the format.
#[test]
fn test_gedcom_corpus_detected_by_line_based_regex() {
    let buffer = load_corpus_file("gedcom.testfile");

    // `regex/1l "^0 HEAD"` — scan only the first line for the header.
    let head_line_flags = RegexFlags {
        line_based: true,
        ..RegexFlags::default()
    };

    let gedcom_rule = regex_rule(
        OffsetSpec::Absolute(0),
        r"^0 HEAD",
        head_line_flags,
        Some(1),
        "GEDCOM genealogy data",
        vec![],
        0,
    );

    let matches = run_rules(&[gedcom_rule], &buffer);
    assert_eq!(matches.len(), 1, "gedcom should match: {matches:#?}");
    assert_eq!(matches[0].message, "GEDCOM genealogy data");
}

// =====================================================================
// regex-eol — simplified version extraction smoke test
// =====================================================================

/// Smoke test that the simpler non-hierarchical part of the regex-eol
/// scenario still works after the flag semantic change. Full
/// hierarchical coverage lives in the `test_regex_eol_corpus` test in
/// `tests/evaluator_tests.rs`.
#[test]
fn test_regex_eol_version_extraction() {
    let buffer = load_corpus_file("regex-eol.testfile");

    // Match a version number anywhere in the first line.
    let version_rule = regex_rule(
        OffsetSpec::Absolute(0),
        r"[0-9]+(\.[0-9]+)+",
        RegexFlags {
            line_based: true,
            ..RegexFlags::default()
        },
        Some(1),
        "version found",
        vec![],
        0,
    );

    let matches = run_rules(&[version_rule], &buffer);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "version found");
    // The matched value should look like a version number.
    match &matches[0].value {
        Value::String(s) => assert!(
            s.chars().all(|c| c.is_ascii_digit() || c == '.'),
            "matched text should be a version number, got {s:?}"
        ),
        other => panic!("expected Value::String, got {other:?}"),
    }
}

// =====================================================================
// Meta: corpus files exist
// =====================================================================

#[test]
fn test_corpus_files_exist() {
    for name in [
        "searchbug.testfile",
        "json1.testfile",
        "jsonlines1.testfile",
        "cmd1.testfile",
        "gedcom.testfile",
        "regex-eol.testfile",
    ] {
        let path = format!("{CORPUS_DIR}/{name}");
        assert!(
            std::path::Path::new(&path).exists(),
            "corpus file missing: {path}"
        );
    }
}
