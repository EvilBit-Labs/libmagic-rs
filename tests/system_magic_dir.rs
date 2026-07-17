// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

// Helper functions in this file (not themselves `#[test]` fns) call
// `.expect()` on I/O and evaluation results; clippy's `allow-expect-in-tests`
// (clippy.toml) only recognizes code directly inside `#[test]` bodies as
// test code, not helpers called from them. See `tests/integration_tests.rs`
// and `tests/directory_loading_tests.rs` for the same established pattern.
#![allow(clippy::expect_used)]

//! Gated system-DB load test + differential parity against GNU `file`
//! (U6 of the `fix/system-magic-regex-graceful` plan).
//!
//! # Gates
//!
//! **GATE 1 (system DB present):** runs only if `/usr/share/file/magic/`
//! exists on the host. If not, the test skips cleanly via an early
//! `eprintln!` + `return` -- it does not fail, so CI hosts without the
//! macOS/Linux `file` package installed stay green. This is the floor
//! proof required by R1: loading the real system magic DB and
//! evaluating real targets must never fatally abort.
//!
//! **GATE 2 (`file` binary present):** additionally gated on the `file`
//! binary being on `PATH` (checked via `file --version`). If absent, the
//! differential-parity tests skip cleanly the same way. Where both gates
//! are satisfied, this is the authoritative arbiter of correctness (R6):
//! it compares this crate's evaluation of curated samples against GNU
//! `file`'s own output on the *specific dimension* the regex fix targets
//! (whether the `assembler` source signal is detected), not a
//! byte-for-byte whole-line comparison -- `rmagic` and GNU `file` cover
//! very different total rule sets, so unrelated-rule divergence is
//! expected and out of scope.
//!
//! # Why the library API, not the `rmagic` binary
//!
//! The differential-parity comparisons call [`libmagic_rs::MagicDatabase`]
//! directly rather than shelling out to `target/debug/rmagic`, so this
//! test has no build-order dependency on the CLI binary. `rmagic` itself
//! is just a thin wrapper over this same API (see `src/main.rs`), so
//! going through the library is equivalent for the purpose of proving
//! the regex fix's correctness, and is more robust in `cargo test`
//! invocations that do not also build binaries first.
//!
//! # Why `stop_at_first_match(false)` for the parity checks
//!
//! `EvaluationConfig::default()` sets `stop_at_first_match: true`
//! (GOTCHAS S13.2): the evaluator halts at the *first* top-level rule
//! that matches, in strength-sorted order. The full system DB contains
//! thousands of rules across hundreds of files, and several unrelated
//! top-level rules (for example the message-less `c-lang` gating
//! `search` rules used purely to trigger child regex rules, see
//! `/usr/share/file/magic/c-lang`) legitimately match with an *empty*
//! message before the `assembler` rules are ever tried alphabetically.
//! With the default config, this can shadow the assembler detection
//! entirely -- not because the regex fix is wrong (the isolated-file
//! evaluation with only `assembler` loaded proves it fires correctly),
//! but because an unrelated rule earlier in strength order wins the
//! first-match race. The parity checks therefore load with
//! `with_stop_at_first_match(false)` and search the full match list for
//! the `"assembler source text"` message, which is the behaviorally
//! correct way to ask "did the assembler signal fire anywhere," matching
//! what GNU `file`'s own multi-entry evaluation effectively does when it
//! prints `"assembler source text, ASCII text"` (two independent magic
//! entries, concatenated).

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use std::path::Path;
use std::process::Command;

/// GNU `file`'s macOS/Linux-packaged system magic directory. Hard-coded
/// per the plan (this exact path is the one the maintainer verified the
/// bug against), not read from an environment variable, since the intent
/// is to test against the specific host DB the bug report was filed
/// against.
const SYSTEM_MAGIC_DIR: &str = "/usr/share/file/magic/";

/// Whether `path` (a system magic directory candidate) exists. Factored
/// out into its own function so the "clean skip" branch is exercised by
/// an isolated unit test below, independent of whether the actual host
/// happens to have the system DB installed.
fn has_system_magic_dir(path: &Path) -> bool {
    path.exists()
}

/// Whether the `file` binary is available on `PATH`, checked via
/// `file --version` rather than `which file` so the check works
/// identically on hosts where `which` itself might not be installed
/// (e.g., some minimal container images).
fn has_file_binary() -> bool {
    Command::new("file")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The "clean skip" path itself must be reachable and correct, verified
/// against a path that is guaranteed not to exist -- this does not
/// depend on the actual host's `/usr/share/file/magic/` state, so it
/// always runs (never gated) and always passes.
#[test]
fn test_skip_gate_is_reachable_for_a_missing_directory() {
    let fake_path = Path::new("/definitely/does/not/exist/on/any/host/libmagic-rs-test");
    assert!(
        !has_system_magic_dir(fake_path),
        "the gate helper must report false for a path that does not exist"
    );
}

/// GATE 1: the system DB never fatally aborts evaluation of a real
/// target (R1, the plan's floor requirement). Per-file parse warnings
/// and non-matches are fine -- only a hard `Err` is a failure. This
/// mirrors the plan's exact repro:
/// `./target/debug/rmagic --magic-file /usr/share/file/magic/ ./Cargo.toml`
#[test]
fn test_system_magic_dir_loads_and_evaluates_cargo_toml_without_fatal_error() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             gated system-DB load test skipped cleanly"
        );
        return;
    }

    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let cargo_toml = std::fs::read("Cargo.toml").expect("Cargo.toml must be readable in CI/dev");
    let result = db
        .evaluate_buffer(&cargo_toml)
        .expect("evaluating Cargo.toml against the system DB must not fatally error");

    // Not asserting on `description` content here -- Cargo.toml is a
    // plain-text TOML file with no dedicated magic entry in the system
    // DB, so "no confident match" is an entirely valid outcome. The
    // floor requirement is solely that evaluation completes.
    let _ = result;
}

/// GATE 1 (binary target variant): if the crate's own compiled binary is
/// present (built by a prior `cargo build` in this workspace), evaluate
/// it too -- a real ELF/Mach-O binary exercises a very different set of
/// system-DB rules than a text file and is a stronger floor proof.
/// Skipped (not failed) if the binary has not been built, since this
/// test does not build it itself (see the module doc for why the
/// parity tests avoid a build-order dependency).
#[test]
fn test_system_magic_dir_loads_and_evaluates_rmagic_binary_if_present() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             gated system-DB load test skipped cleanly"
        );
        return;
    }

    let binary_path = Path::new("target/debug/rmagic");
    if !binary_path.exists() {
        eprintln!("SKIP: target/debug/rmagic not built -- binary-target load test skipped");
        return;
    }

    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");
    let buffer = std::fs::read(binary_path).expect("target/debug/rmagic must be readable");
    let result = db
        .evaluate_buffer(&buffer)
        .expect("evaluating the rmagic binary against the system DB must not fatally error");
    let _ = result;
}

/// A curated sample and its expected assembler-detection outcome, used
/// by both the differential-parity test (against GNU `file`) and (via
/// its own positive/negative shape) as a second, real-file-backed cross
/// check on top of the AST-built fixtures in
/// `tests/regex_getstr_fixtures.rs`.
struct Sample {
    filename: &'static str,
    content: &'static [u8],
    /// Whether both `rmagic` and GNU `file` are expected to detect the
    /// `assembler` source signal for this sample.
    expect_assembler_detected: bool,
    description: &'static str,
}

const SAMPLES: &[Sample] = &[
    Sample {
        filename: "leading_tab_asciiz.s",
        content: b"\t.asciiz \"hi\"\n",
        expect_assembler_detected: true,
        description: "leading tab then .asciiz -- the plan's worked-example positive case",
    },
    Sample {
        filename: "column_zero_globl.s",
        content: b".globl main\n",
        expect_assembler_detected: true,
        description: "a different affected assembler keyword (.globl) at column 0",
    },
    Sample {
        filename: "plain_text.txt",
        content: b"hello world, this is not assembler\n",
        expect_assembler_detected: false,
        description: "ordinary prose text with no assembler directive anywhere",
    },
    Sample {
        filename: "non_whitespace_prefix_asciiz.s",
        content: b"xyz .asciiz\n",
        expect_assembler_detected: false,
        description: "non-whitespace prefix before .asciiz -- must not match (anchor requires \
                       0-50 leading whitespace chars, not arbitrary prefix text)",
    },
];

/// Does `message` (GNU `file`'s stdout, or one of our own rule
/// messages) carry the assembler-source signal this fix restores?
fn contains_assembler_signal(text: &str) -> bool {
    text.contains("assembler source text")
}

/// Run this crate's evaluator (via the library API, `stop_at_first_match`
/// disabled per the module doc) against `content` and report whether the
/// assembler signal fired anywhere in the match list.
fn rmagic_detects_assembler(db: &MagicDatabase, content: &[u8]) -> bool {
    let result = db
        .evaluate_buffer(content)
        .expect("evaluation must not fatally error for a curated sample");
    result
        .matches
        .iter()
        .any(|m| contains_assembler_signal(&m.message))
}

/// Run GNU `file` against a real file on disk and report whether its
/// output carries the assembler signal.
fn file_binary_detects_assembler(path: &Path) -> bool {
    let output = Command::new("file")
        .arg("--magic-file")
        .arg(SYSTEM_MAGIC_DIR)
        .arg(path)
        .output()
        .expect("failed to invoke the `file` binary");
    assert!(
        output.status.success(),
        "`file` exited non-zero for {}: stderr={}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    contains_assembler_signal(&String::from_utf8_lossy(&output.stdout))
}

/// GATE 2: differential parity against GNU `file` on the specific
/// dimension this fix restores -- assembler-source detection -- for a
/// small curated sample set covering both a positive and a negative
/// case, and both the plan's worked example (`.asciiz`) and a second
/// affected keyword (`.globl`) to guard against a resolver bug that
/// happens to work for one keyword but not others.
///
/// Per the plan's "don't leave residuals" directive: if this test ever
/// finds a genuine divergence between `rmagic` and `file`, the fix
/// belongs in `src/parser/grammar/getstr/mod.rs` (or, as this session's
/// U6 pass discovered, in `src/evaluator/types/regex.rs`'s regex-compile
/// setup -- see that file's `build_regex` doc comment for the
/// `unicode(false)` fix this test's `>= 0x80` cousin in
/// `tests/regex_getstr_fixtures.rs` is guarding), not a follow-up issue.
/// On this host (verified during implementation), parity holds for all
/// four samples with no divergence to diagnose.
#[test]
fn test_differential_parity_against_gnu_file_for_assembler_detection() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             differential parity test skipped cleanly"
        );
        return;
    }
    if !has_file_binary() {
        eprintln!("SKIP: `file` binary not on PATH -- differential parity test skipped cleanly");
        return;
    }

    // See the module doc: `stop_at_first_match(false)` is required so an
    // unrelated earlier top-level match (e.g. a message-less `c-lang`
    // gating `search` rule) cannot shadow the assembler rule out of the
    // match list before we get to inspect it.
    let config = EvaluationConfig::default().with_stop_at_first_match(false);
    let db = MagicDatabase::load_from_file_with_config(system_dir, config)
        .expect("loading the system magic directory must not fail");

    let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir for samples");

    for sample in SAMPLES {
        let sample_path = temp_dir.path().join(sample.filename);
        std::fs::write(&sample_path, sample.content).expect("failed to write sample file");

        let rmagic_result = rmagic_detects_assembler(&db, sample.content);
        let file_result = file_binary_detects_assembler(&sample_path);

        assert_eq!(
            rmagic_result, sample.expect_assembler_detected,
            "rmagic detection mismatch for {:?} ({}): expected {}, got {}",
            sample.filename, sample.description, sample.expect_assembler_detected, rmagic_result
        );
        assert_eq!(
            file_result, sample.expect_assembler_detected,
            "GNU `file` detection mismatch for {:?} ({}): expected {}, got {} -- \
             this would mean the fixture's expectation itself is wrong, not rmagic",
            sample.filename, sample.description, sample.expect_assembler_detected, file_result
        );
        assert_eq!(
            rmagic_result, file_result,
            "PARITY FAILURE for {:?} ({}): rmagic={rmagic_result} file={file_result} -- \
             a genuine divergence must be diagnosed and fixed in-band, not deferred \
             (see the plan's U6 section and this test's doc comment)",
            sample.filename, sample.description
        );
    }
}
