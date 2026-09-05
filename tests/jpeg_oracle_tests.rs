// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Oracle-backed coverage for the JPEG segment walk (issue #471).
//!
//! The unit and end-to-end tests for `apply_use_result_base` assert against
//! hand-authored offsets. They prove the rebasing contract, but every value in
//! them is one this repository chose, so on their own they cannot detect a
//! regression that keeps the contract and still changes what a user sees. This
//! file closes that gap from the other end: it asserts on a rendered
//! description for a committed fixture.
//!
//! Two layers, deliberately:
//!
//! 1. `hermetic_segment_walk_renders_every_segment` always runs. It builds a
//!    reduced jpeg magic chain inline and asserts the full description, so CI
//!    needs no `file` binary and no system magic database. This follows
//!    `tests/macho_universal_tests.rs`, which builds its magic inline for the
//!    same reason.
//! 2. `differential_parity_against_gnu_file_on_the_committed_fixture` asserts
//!    rmagic and real `file` agree on the same magic database, skipping
//!    cleanly when either is absent. This follows
//!    `tests/system_magic_dir.rs`. Parity against the same database is a
//!    stronger invariant than a frozen string, which goes stale the moment the
//!    host database changes.
//!
//! ## Fixture and oracle provenance
//!
//! `tests/fixtures/jpeg_segment_walk.jpg` is 85 bytes, generated for this test
//! rather than taken from a system path, and carries three segments so the
//! walk has to traverse all of them:
//!
//! | position | marker        | length | next |
//! | -------- | ------------- | ------ | ---- |
//! | 2        | `FFE0` APP0   | 16     | 20   |
//! | 20       | `FFE1` APP1   | 42     | 64   |
//! | 64       | `FFC0` SOF0   | 17     | 83   |
//!
//! Reaching SOF0 at 64 requires the `use` result framing in GOTCHAS S3.10: the
//! walk arrives there only by rebasing each dereferenced segment length onto
//! the current use-site. A walk that drops the base stops after APP0.
//!
//! Oracle recorded 2026-09-02 with `file-5.41` against
//! `/usr/share/file/magic/` on macOS:
//!
//! ```text
//! JPEG image data, JFIF standard 1.01, aspect ratio, density 72x72,
//! segment length 16, Exif Standard: [TIFF image data, big-endian,
//! direntries=1, xresolution=26], baseline, precision 8, 320x200, components 3
//! ```
//!
//! That string is recorded for provenance only; no test asserts it. The
//! differential test instead asserts that rmagic and `file` agree on a staged
//! copy of the host's own database, so it survives a magic-DB or `file` upgrade
//! that a frozen string would not.

#![allow(clippy::expect_used)]

use std::io::Write;
use std::path::Path;
use std::process::Command;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::NamedTempFile;

const FIXTURE: &str = "tests/fixtures/jpeg_segment_walk.jpg";
const SYSTEM_MAGIC_DIR: &str = "/usr/share/file/magic";

/// SOF0's position in the committed fixture -- the third segment, and the one
/// the walk reaches only by rebasing twice. Named so the truncation slice below
/// stays tied to the segment table in the module doc; regenerating the fixture
/// with a different layout must update both together.
const SOF0_OFFSET: usize = 64;

/// A reduced `jpeg` chain: enough to walk APP0 -> APP1 -> SOF0 through the
/// recursive `use jpeg_segment`, and nothing else. The shape mirrors the real
/// `jpeg` magic file's `>>(2.S+2) use jpeg_segment` recursion, which is the
/// construct issue #471 is about.
const JPEG_MAGIC: &str = "0\tbeshort\t0xffd8\tJPEG image data\n\
     >2\tuse\tjpeg_segment\n\
     0\tname\tjpeg_segment\n\
     >0\tbeshort\t0xffe0\t\\b, JFIF segment\n\
     >0\tbeshort\t0xffe1\t\\b, Exif segment\n\
     >0\tbeshort\t0xffc0\t\\b, baseline\n\
     >>7\tbeshort\tx\t\\b, %dx\n\
     >>5\tbeshort\tx\t\\b%d\n\
     >0\tbeshort\t!0xffc0\n\
     >>(2.S+2)\tuse\tjpeg_segment\n";

fn hermetic_db() -> MagicDatabase {
    let mut f = NamedTempFile::new().expect("temp magic file");
    f.write_all(JPEG_MAGIC.as_bytes())
        .expect("write temp magic");
    f.flush().expect("flush temp magic");
    MagicDatabase::load_from_file_with_config(
        f.path(),
        EvaluationConfig::default().with_stop_at_first_match(false),
    )
    .expect("reduced jpeg magic must load")
}

fn fixture_bytes() -> Vec<u8> {
    let bytes = std::fs::read(FIXTURE)
        .expect("committed fixture tests/fixtures/jpeg_segment_walk.jpg must exist");

    // Tie the committed bytes to the segment table in the module doc. Several
    // tests hardcode offsets derived from that table and there is no generator
    // script, so a regenerated fixture would otherwise drift silently until an
    // opaque "description lacked 320x200" failure.
    assert_eq!(
        bytes.len(),
        85,
        "fixture size drifted from the documented segment table"
    );
    for (pos, marker, label) in [
        (2usize, 0xE0u8, "APP0"),
        (20, 0xE1, "APP1"),
        (SOF0_OFFSET, 0xC0, "SOF0"),
    ] {
        let seen = bytes.get(pos..=pos + 1);
        assert_eq!(
            seen,
            Some([0xFFu8, marker].as_slice()),
            "{label} is no longer at offset {pos}; the module doc's segment table \
             and every offset derived from it are stale"
        );
    }
    bytes
}

fn has_file_binary() -> bool {
    Command::new("file")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The committed fixture's walk must reach the third segment.
///
/// `320x200` comes from SOF0 at position 64, which the walk reaches only by
/// rebasing each dereferenced length onto the use-site. Stub the rebase in
/// `apply_use_result_base` and this assertion fails -- that is what makes this
/// a regression gate rather than a snapshot.
#[test]
fn hermetic_segment_walk_renders_every_segment() {
    let description = hermetic_db()
        .evaluate_buffer(&fixture_bytes())
        .expect("evaluate committed fixture")
        .description;

    assert_eq!(
        description, "JPEG image data, JFIF segment, Exif segment, baseline, 320x200",
        "the walk must reach all three segments. Exact equality rather than \
         substring checks: a recursion regression (GOTCHAS S14.7) duplicates \
         fragments and degrades at the depth limit, which leaves every substring \
         present and would pass a `contains` assertion on visibly wrong output"
    );
}

/// Negative control: a fixture truncated before SOF0 must NOT render the
/// dimensions. Without this, a bug that renders the tail unconditionally --
/// rather than by actually walking to it -- would pass the test above.
#[test]
fn truncated_fixture_does_not_render_the_unreached_segment() {
    let full = fixture_bytes();
    // Cutting at SOF0 leaves APP0 and APP1 intact.
    let truncated = &full[..SOF0_OFFSET];

    let description = hermetic_db()
        .evaluate_buffer(truncated)
        .expect("evaluate truncated fixture")
        .description;

    assert_eq!(
        description, "JPEG image data, JFIF segment, Exif segment",
        "a segment that is not present must not be rendered -- otherwise the \
         positive test proves nothing about walking rather than about rendering \
         a tail unconditionally"
    );
}

/// Why the oracle cannot simply be `file --magic-file <system dir>`.
///
/// Measured on this host with `file-5.41`: `--magic-file <dir>` prefers a
/// sibling compiled `<dir>.mgc` over the directory itself, and
/// `/usr/share/file/magic.mgc` exists next to `/usr/share/file/magic`. A
/// directory holding only a sentinel rule still yields the built-in
/// classification. The naive form therefore compares rmagic-on-source-files
/// against file-on-compiled-database -- two different databases, which is not a
/// parity check and cannot notice the two drifting apart.
enum OracleReadiness {
    /// A staged copy whose magic directory `file` provably reads.
    Ready(tempfile::TempDir),
    /// The environment cannot support a like-for-like comparison.
    Skip(String),
}

/// Count the plain files in `dir`.
///
/// Named and extracted so the skip decision rests on testable logic rather than
/// an inline closure. Debian and Ubuntu ship only the compiled `magic.mgc` and
/// leave the source directory empty; that is the case this detects.
fn magic_source_file_count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count()
    })
}

/// Ask `file` to classify `target` using only `magic_dir`.
///
/// `MAGIC=` rather than `--magic-file`: measured on this host, the flag did not
/// restrict the database while the environment variable did.
fn file_says(magic_dir: &Path, target: &str) -> String {
    let output = Command::new("file")
        .env("MAGIC", magic_dir)
        .arg("-b")
        .arg(target)
        .output()
        .expect("invoking `file` must not fail once it is known present");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Stage a magic directory `file` demonstrably reads, or explain why not.
///
/// The canary is the load-bearing step. Every earlier version of this test gated
/// on a symptom -- whether `file`'s answer looked right -- which passes exactly
/// when a silent fallback is masking the mismatch. This proves the negative
/// instead: point `file` at a database knowing only a sentinel type and require
/// the fixture NOT to classify as JPEG. If it still does, `file` is answering
/// from elsewhere and no comparison here would mean anything.
fn stage_oracle() -> OracleReadiness {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !system_dir.is_dir() {
        return OracleReadiness::Skip(format!("{SYSTEM_MAGIC_DIR} is not present"));
    }
    if magic_source_file_count(system_dir) == 0 {
        return OracleReadiness::Skip(format!(
            "{SYSTEM_MAGIC_DIR} holds no source magic files (compiled-only install)"
        ));
    }
    if !has_file_binary() {
        return OracleReadiness::Skip("`file` is not on PATH".to_string());
    }

    let staged = tempfile::TempDir::new().expect("temp dir for the staged magic copy");

    let canary_dir = staged.path().join("canary");
    std::fs::create_dir_all(&canary_dir).expect("create canary dir");
    std::fs::write(
        canary_dir.join("sentinel"),
        "0\tstring\tZZ-SENTINEL-NEVER-MATCHES\tsentinel\n",
    )
    .expect("write sentinel rule");
    let canary = file_says(&canary_dir, FIXTURE);
    assert!(
        !canary.contains("JPEG"),
        "`file` classified the fixture as JPEG from a database holding only a \
         sentinel rule, so it is answering from some other database and this \
         comparison would be meaningless. Got: {canary:?}"
    );

    let magic_copy = staged.path().join("magic");
    std::fs::create_dir_all(&magic_copy).expect("create staged magic dir");
    for entry in std::fs::read_dir(system_dir).expect("read system magic dir") {
        let entry = entry.expect("read system magic entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), magic_copy.join(entry.file_name()))
                .expect("copy magic source file");
        }
    }
    OracleReadiness::Ready(staged)
}

/// Parity against real `file` on a magic database both sides provably share.
///
/// A divergence is not automatically an evaluator regression: the runtime loader
/// is line-tolerant (GOTCHAS S3.11) and silently drops a rule -- and its subtree
/// -- that it cannot parse, while `file` accepts it. Establish which before
/// concluding anything.
#[test]
fn differential_parity_against_gnu_file_on_the_committed_fixture() {
    let staged = match stage_oracle() {
        OracleReadiness::Ready(dir) => dir,
        OracleReadiness::Skip(reason) => {
            eprintln!("SKIP: {reason} -- parity test skipped cleanly");
            return;
        }
    };
    let magic_dir = staged.path().join("magic");

    let db = MagicDatabase::load_from_file_with_config(&magic_dir, EvaluationConfig::default())
        .expect("loading the staged magic directory must not fail");
    let ours = db
        .evaluate_buffer(&fixture_bytes())
        .expect("evaluate committed fixture")
        .description;
    let theirs = file_says(&magic_dir, FIXTURE);

    assert_eq!(
        ours, theirs,
        "rmagic and `file` disagree on a database both provably read. Either the \
         evaluator regressed, or the tolerant loader dropped a rule `file` honored \
         (GOTCHAS S3.11) -- determine which rather than deferring"
    );
}

/// Pin the skip decision's own logic.
///
/// The previous version of this test asserted that a nonexistent path reports
/// `is_dir() == false`, which is standard-library behavior that nothing in this
/// repository can break -- it was the only test here that survived a mutation
/// disabling the rebase. `magic_source_file_count` is the half that can
/// actually regress, and it is what decides whether a compiled-only install
/// skips or produces a bogus comparison.
#[test]
fn magic_source_file_count_distinguishes_empty_from_populated() {
    let temp = tempfile::TempDir::new().expect("temp dir for the count probe");

    assert_eq!(
        magic_source_file_count(temp.path()),
        0,
        "an empty directory must count as no source magic, or a compiled-only \
         install would compare rmagic against `file`'s built-in database"
    );

    std::fs::write(temp.path().join("jpeg"), "0\tbeshort\t0xffd8\tJPEG\n")
        .expect("write a magic source file");
    assert_eq!(
        magic_source_file_count(temp.path()),
        1,
        "a directory holding a magic source file must count it, or the parity \
         test would skip on every host and never compare anything"
    );

    // A subdirectory is not a source file and must not make an otherwise-empty
    // directory look populated.
    let nested = tempfile::TempDir::new().expect("second temp dir");
    std::fs::create_dir_all(nested.path().join("subdir")).expect("create subdir");
    assert_eq!(
        magic_source_file_count(nested.path()),
        0,
        "directories must not be counted as source magic files"
    );
}
