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
//! That full string is asserted only by the differential test, which compares
//! against whatever `file` the host actually has. The hermetic test asserts the
//! segment-walk tail, which is the part this issue is about.

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
    std::fs::read(FIXTURE)
        .expect("committed fixture tests/fixtures/jpeg_segment_walk.jpg must exist")
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

    assert!(
        description.contains("JFIF segment"),
        "the walk must reach APP0 at position 2; got {description:?}"
    );
    assert!(
        description.contains("Exif segment"),
        "the walk must reach APP1 at position 20, which needs the first \
         rebase (use-site 2 + segment length); got {description:?}"
    );
    assert!(
        description.contains("320x200"),
        "the walk must reach SOF0 at its fixture position, which needs the rebase to \
         hold across a second hop (GOTCHAS S3.10); a walk that drops the \
         subroutine base stops earlier. got {description:?}"
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

    assert!(
        description.contains("Exif segment"),
        "truncating at SOF0 must leave the first two segments reachable; \
         got {description:?}"
    );
    assert!(
        !description.contains("320x200"),
        "a segment that is not present must not be rendered -- otherwise the \
         positive test proves nothing about walking. got {description:?}"
    );
}

/// Parity against real `file` on the same magic database.
///
/// Skips cleanly when the system magic directory or the `file` binary is
/// absent, matching `tests/system_magic_dir.rs`. Parity on the same database
/// is asserted rather than a frozen expected string: a committed string goes
/// stale when the host database changes, while divergence from the oracle is
/// always a genuine signal.
#[test]
fn differential_parity_against_gnu_file_on_the_committed_fixture() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !system_dir.is_dir() {
        eprintln!("SKIP: {SYSTEM_MAGIC_DIR} not present -- parity test skipped cleanly");
        return;
    }
    // The directory existing is not enough. Debian and Ubuntu ship only the
    // compiled `magic.mgc` and leave this source directory empty; `file
    // --magic-file <empty dir>` then silently falls back to its built-in
    // database while rmagic loads the empty directory and classifies nothing.
    // Comparing those two is not a parity check -- the sides are reading
    // different databases -- so require at least one source file first.
    let source_files = std::fs::read_dir(system_dir).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count()
    });
    if source_files == 0 {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} holds no source magic files (compiled-only \
             install) -- `file` would fall back to its built-in DB, so there is \
             no shared database to compare against"
        );
        return;
    }
    if !has_file_binary() {
        eprintln!("SKIP: `file` not on PATH -- parity test skipped cleanly");
        return;
    }

    let db = MagicDatabase::load_from_file_with_config(system_dir, EvaluationConfig::default())
        .expect("loading the system magic directory must not fail");
    let ours = db
        .evaluate_buffer(&fixture_bytes())
        .expect("evaluate committed fixture")
        .description;

    let output = Command::new("file")
        .arg("-b")
        .arg("--magic-file")
        .arg(SYSTEM_MAGIC_DIR)
        .arg(FIXTURE)
        .output()
        .expect("invoking `file` must not fail once it is known present");
    // A nonzero exit means `file` started but could not use the database or
    // arguments. stdout is then empty, which would fall through the skip branch
    // below and report success with no oracle -- the exact silent pass this
    // test exists to prevent.
    assert!(
        output.status.success(),
        "`file` exited {:?} rather than classifying the fixture; the oracle did \
         not run. stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).trim()
    );

    let theirs = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !theirs.contains("JPEG") {
        eprintln!(
            "SKIP: this host's --magic-file DB does not classify JPEG \
             (got {theirs:?}) -- parity test skipped cleanly"
        );
        return;
    }

    assert_eq!(
        ours, theirs,
        "rmagic must match real `file` on the same magic database for the \
         committed JPEG fixture; a divergence here is a genuine regression to \
         diagnose in the evaluator, not to defer"
    );
}

/// The parity test above skips when the system magic directory is absent.
/// A skip gate that silently stopped being reachable would turn that test into
/// a no-op without any signal, so pin the predicate itself -- the same guard
/// `tests/system_magic_dir.rs` keeps for its own skip.
///
/// The absent path is a fresh temp directory's unborn child rather than a
/// hard-coded absolute path: a literal path could exist on some host or
/// container image, which would silently invert what this test proves.
#[test]
fn parity_skip_gate_is_reachable_for_a_missing_directory() {
    let temp = tempfile::TempDir::new().expect("temp dir for skip-gate probe");
    let absent = temp.path().join("no-such-magic-dir");

    assert!(
        !absent.is_dir(),
        "the parity test's directory predicate must still be able to report \
         absent, or its skip branch is unreachable and the test is a no-op"
    );
}
