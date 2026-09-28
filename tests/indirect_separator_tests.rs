// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! How an `indirect` re-entry's first fragment joins the text before it
//! (GOTCHAS S14.5).
//!
//! libmagic never spaces a top-level description, but spaces a continuation
//! description whenever the shared `need_separator` flag is already set -- and
//! the outer rule that dispatched the `indirect` has set it. So the join
//! depends on the level of the inner rule that produced the first fragment.
//! An ID3 tag's `\b, contains:` followed by MPEG ADTS (whose top-level rule has no
//! description) is the real case that needs the space.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::Write;

use libmagic_rs::MagicDatabase;
use tempfile::TempDir;

/// Outer rule matches `OUT` and re-enters the whole database at offset 4,
/// with `indirect_message` as the directive's own description.
fn describe_with(indirect_message: &str, inner_rules: &str, inner_bytes: &[u8]) -> String {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("indirect_sep.magic");
    let mut f = fs::File::create(&magic_path).unwrap();
    write!(
        f,
        "0 string OUT Outer\n>4 indirect x {indirect_message}\n{inner_rules}"
    )
    .unwrap();
    drop(f);

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    let mut buffer = b"OUT\0".to_vec();
    buffer.extend_from_slice(inner_bytes);
    db.evaluate_buffer(&buffer).unwrap().description
}

fn describe(inner_rules: &str, inner_bytes: &[u8]) -> String {
    describe_with("\\b, contains:", inner_rules, inner_bytes)
}

#[test]
fn indirect_first_fragment_joins_by_inner_rule_level() {
    let cases: &[(&str, &str, &[u8], &str)] = &[
        (
            "top-level inner description attaches with no space",
            "0 byte 0xFB TOPINNER\n",
            &[0xFB, 0x00],
            "Outer, contains:TOPINNER",
        ),
        (
            "continuation inner description gets one space",
            "0 byte&0xFE 0xFA\n>1 byte x INNER\n",
            &[0xFB, 0x00],
            "Outer, contains: INNER",
        ),
        (
            "continuation carrying its own \\b marker stays unspaced",
            "0 byte&0xFE 0xFA\n>1 byte x \\bNOSP\n",
            &[0xFB, 0x00],
            "Outer, contains:NOSP",
        ),
        (
            "re-entry that renders nothing drops the indirect's own message",
            "0 byte 0xFB TOPINNER\n",
            &[0x00, 0x00],
            "Outer",
        ),
    ];

    for (name, inner_rules, inner_bytes, expected) in cases {
        let got = describe(inner_rules, inner_bytes);
        assert_eq!(&got, expected, "case: {name}");
    }
}

/// `archive`'s XAR rules end their message with a space (`\b, contains `).
/// Measured against file-5.41: the trailing space is kept, and a continuation
/// first fragment still gets its own separator on top of it.
#[test]
fn indirect_message_trailing_space_is_preserved() {
    let cases: &[(&str, &str, &[u8], &str)] = &[
        (
            "top-level inner description follows the trailing space directly",
            "0 byte 0xFC TOP\n",
            &[0xFC, 0x00],
            "Outer, contains TOP",
        ),
        (
            "continuation inner description adds a separator after it",
            "0 byte&0xFE 0xFA\n>1 byte x INNER\n",
            &[0xFB, 0x00],
            "Outer, contains  INNER",
        ),
    ];

    for (name, inner_rules, inner_bytes, expected) in cases {
        let got = describe_with("\\b, contains ", inner_rules, inner_bytes);
        assert_eq!(&got, expected, "case: {name}");
    }
}
