// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Oracle-backed coverage for ID3 synchsafe indirect pointers (issue #237).
//!
//! `audio:308` is `>(6.I+10) indirect x \b, contains:`: the `I` pointer reads
//! the ID3 tag size as a synchsafe integer and re-enters the database past the
//! tag. Two layers, following `tests/jpeg_oracle_tests.rs`:
//!
//! 1. Hermetic tests build a reduced ID3 + MPEG chain inline, so CI needs no
//!    `file` binary.
//! 2. A differential test asserts rmagic and real `file` agree on the committed
//!    fixture using the same staged magic database, skipping cleanly when
//!    either is absent.
//!
//! ## Fixture and oracle provenance
//!
//! `third_party/tests/JW07022A.mp3.testfile` comes from the upstream `file`
//! test suite. Its size field at offset 6 is `00 00 10 24`: 2084 as synchsafe,
//! 4132 as a plain long. The MPEG frame sync `ff fb` sits at 2094 (2084 + 10);
//! offset 4142 holds unrelated audio data. Its `.result` file, which matches
//! `file-5.41` on macOS, is:
//!
//! ```text
//! Audio file with ID3 version 2.2.0, contains: MPEG ADTS, layer III, v1, 96 kbps, 44.1 kHz, Monaural
//! ```

#![allow(clippy::expect_used)]

mod common;

use std::io::Write;

use common::magic_oracle::{OracleReadiness, file_says, skip, stage_system_magic};
use libmagic_rs::MagicDatabase;
use tempfile::NamedTempFile;

const FIXTURE: &str = "third_party/tests/JW07022A.mp3.testfile";

/// The `audio` ID3 entry plus the first three lines of `animation`'s MPEG ADTS
/// entry, whose top-level rule carries no description. `{pointer}` is the
/// indirect pointer specifier under test.
const ID3_MAGIC_TEMPLATE: &str = "0\tstring\tID3\tAudio file with ID3 version 2\n\
     >3\tbyte\tx\t\\b.%d\n\
     >4\tbyte\tx\t\\b.%d\n\
     >(6.{pointer}+10)\tindirect\tx\t\\b, contains:\n\
     0\tbeshort&0xFFFE\t0xFFFA\n\
     >2\tbyte&0xF0\t!0\n\
     >>2\tbyte&0xF0\t!0xF0\tMPEG ADTS, layer III, v1\n";

fn describe_fixture_with_pointer(pointer: &str) -> String {
    let mut f = NamedTempFile::new().expect("temp magic file");
    f.write_all(ID3_MAGIC_TEMPLATE.replace("{pointer}", pointer).as_bytes())
        .expect("write temp magic");
    f.flush().expect("flush temp magic");
    let db = MagicDatabase::load_from_file(f.path()).expect("reduced ID3 magic must load");
    let bytes = std::fs::read(FIXTURE).expect("committed ID3 fixture must exist");
    db.evaluate_buffer(&bytes)
        .expect("evaluate ID3 fixture")
        .description
}

/// The synchsafe `I` pointer lands on the MPEG frame, and the continuation's
/// description gets one space after `contains:` (GOTCHAS S14.5).
#[test]
fn hermetic_synchsafe_pointer_reaches_the_inner_mpeg_frame() {
    assert_eq!(
        describe_fixture_with_pointer("I"),
        "Audio file with ID3 version 2.2.0, contains: MPEG ADTS, layer III, v1"
    );
}

/// Negative control: the same chain read as a plain big-endian long lands at
/// 4142, where no MPEG frame exists, so the re-entry renders nothing and the
/// `indirect` is a non-match (its `contains:` is dropped). Without this, the
/// positive test could pass with the decode removed.
#[test]
fn hermetic_plain_long_pointer_misses_the_inner_frame() {
    assert_eq!(
        describe_fixture_with_pointer("L"),
        "Audio file with ID3 version 2.2.0"
    );
}

/// `beid3`/`leid3` read the same synchsafe value as a rule type: the fixture's
/// size field at offset 6 renders as 2084, not the plain-long 4132.
#[test]
fn hermetic_id3_keywords_read_synchsafe_values() {
    let bytes = std::fs::read(FIXTURE).expect("committed ID3 fixture must exist");
    let cases = [
        ("6 beid3 x size %d", "size 2084"),
        ("6 leid3 x size %d", "size 75759616"),
        ("6 beid3 2084 exact", "exact"),
    ];
    for (rule, expected) in cases {
        let mut f = NamedTempFile::new().expect("temp magic file");
        writeln!(f, "{rule}").expect("write temp magic");
        f.flush().expect("flush temp magic");
        let db = MagicDatabase::load_from_file(f.path()).expect("id3 keyword rule must load");
        let got = db.evaluate_buffer(&bytes).expect("evaluate").description;
        assert_eq!(got, expected, "rule: {rule}");
    }
}

/// Parity against real `file` on a magic database both sides provably share.
#[test]
fn differential_parity_against_gnu_file_on_the_id3_fixture() {
    let staged = match stage_system_magic(FIXTURE, "ID3") {
        OracleReadiness::Ready(dir) => dir,
        OracleReadiness::Skip(reason) => {
            skip(&reason);
            return;
        }
    };
    let magic_dir = staged.path().join("magic");

    let db = MagicDatabase::load_from_file(&magic_dir)
        .expect("loading the staged magic directory must not fail");
    let bytes = std::fs::read(FIXTURE).expect("committed ID3 fixture must exist");
    let ours = db
        .evaluate_buffer(&bytes)
        .expect("evaluate ID3 fixture")
        .description;
    let theirs = file_says(&magic_dir, FIXTURE);

    assert_eq!(
        ours, theirs,
        "rmagic and `file` disagree on a database both provably read. Either the \
         evaluator regressed, or the tolerant loader dropped a rule `file` honored \
         (GOTCHAS S3.11) -- determine which rather than deferring"
    );
}
