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
//! whose match (or a descendant's) carries usable description text, in
//! strength-sorted order. The full system DB contains thousands of rules
//! across hundreds of files, and an unrelated but *message-bearing*
//! top-level rule can legitimately match a curated sample earlier in
//! strength order than the `assembler` rules, halting evaluation before
//! the assembler signal is reached. (Note: since the S13.2 fix,
//! *message-less* gating rules -- e.g. the `c-lang` `search` rules that
//! merely trigger child regex rules -- no longer shadow anything; they
//! do not stop evaluation. The remaining first-match race is purely
//! between message-bearing rules.) The parity checks therefore load with
//! `with_stop_at_first_match(false)` and search the full match list for
//! the `"assembler source text"` message, which is the behaviorally
//! correct way to ask "did the assembler signal fire anywhere," matching
//! what GNU `file`'s own multi-entry evaluation effectively does when it
//! prints `"assembler source text, ASCII text"` (two independent magic
//! entries, concatenated). The default-config tests further below
//! instead exercise the exact `stop_at_first_match: true` path the CLI
//! uses.

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
/// Parity (rmagic == real `file` on the same `--magic-file` DB) is the
/// host-adaptive invariant asserted on every host; the absolute
/// positive/negative expectations are additionally asserted only where the
/// host DB actually carries the assembler rules (macOS ships them as source
/// magic files; Ubuntu ships only the compiled `magic.mgc`, leaving the
/// `--magic-file` source dir empty -- see the in-body probe).
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

    // Probe whether THIS host's `--magic-file SYSTEM_MAGIC_DIR` DB actually
    // carries the assembler-source rules. This is host-dependent: macOS
    // ships the *source* magic files under `/usr/share/file/magic/`, so both
    // `file` and rmagic can detect assembler there; Ubuntu/Debian ship only
    // the *compiled* `magic.mgc` and leave that source directory empty, so a
    // `--magic-file /usr/share/file/magic/` invocation carries no assembler
    // rule at all (verified: on ubuntu-latest `file --magic-file
    // /usr/share/file/magic/ asm.s` prints "ASCII text", not "assembler
    // source text"). The absolute positive/negative expectations only apply
    // where the rules are present; parity with real `file` is asserted on
    // every host regardless.
    let probe_path = temp_dir.path().join("__probe_leading_tab_asciiz.s");
    std::fs::write(&probe_path, b"\t.asciiz \"hi\"\n").expect("failed to write probe sample");
    let host_db_has_assembler_rules = file_binary_detects_assembler(&probe_path);

    for sample in SAMPLES {
        let sample_path = temp_dir.path().join(sample.filename);
        std::fs::write(&sample_path, sample.content).expect("failed to write sample file");

        let rmagic_result = rmagic_detects_assembler(&db, sample.content);
        let file_result = file_binary_detects_assembler(&sample_path);

        // Trustworthy invariant on every host: rmagic must agree with real
        // `file` on the SAME `--magic-file` DB, whatever that host DB holds.
        // A genuine divergence must be diagnosed and fixed in-band (in
        // `src/parser/grammar/getstr/mod.rs` or `src/evaluator/types/regex.rs`),
        // not deferred.
        assert_eq!(
            rmagic_result, file_result,
            "PARITY FAILURE for {:?} ({}): rmagic={rmagic_result} file={file_result}",
            sample.filename, sample.description
        );

        // Absolute semantic check -- applicable only where the host DB
        // actually carries the assembler rules (see the probe above).
        if host_db_has_assembler_rules {
            assert_eq!(
                rmagic_result,
                sample.expect_assembler_detected,
                "rmagic detection mismatch for {:?} ({}): expected {}, got {}",
                sample.filename,
                sample.description,
                sample.expect_assembler_detected,
                rmagic_result
            );
        }
    }
}

// =======================================================================
// Default-config (`stop_at_first_match: true`) end-to-end coverage
//
// The tests above deliberately use `with_stop_at_first_match(false)` so
// the assembler signal can be found anywhere in the full match list
// (see the module doc). The tests below instead use
// `EvaluationConfig::default()` unmodified -- the exact configuration
// `MagicDatabase::load_from_file` uses and the CLI's default behavior --
// to prove the actual maintainer-reported bug is fixed: a message-less
// top-level rule (e.g. a `c-lang` gating `search` rule) matching before
// the assembler/text rules used to terminate evaluation via
// `stop_at_first_match` and leave the CLI's description blank. See
// GOTCHAS S13.2 for the refined stop-at-first-match semantics
// (`has_message_bearing_match` in `src/evaluator/engine/mod.rs`) and
// `src/output/ascmagic.rs` for the text/data fallback that also
// contributes here.
// =======================================================================

/// A fixed, reproducible "binary blob" (not literal `/dev/urandom`, which
/// would make the test's outcome host- and run-dependent) verified against
/// the real `file` binary to produce `"data"` on this host's full system
/// magic DB.
///
/// This is a plain linear-congruential byte stream with no special
/// constraints on any byte range.
///
/// History: during initial implementation of the text/data fallback, this
/// exact byte stream spuriously matched `/usr/share/file/magic/ibm6000`'s
/// `AIX core file` entry -- `4 belong &0x0feeddb0` with no explicit
/// comparison and no message. That entry is GNU `file`'s magic(5) shorthand
/// for `belong &0x0feeddb0 =0x0feeddb0` ("all of the masked bits must be
/// set"), but `apply_bitwise_and` (`src/evaluator/operators/bitwise.rs`)
/// at the time implemented the bare `&` relational operator as
/// `(value & operand) != 0` ("any masked bit set") instead -- confirmed by
/// direct A/B testing against `file --magic-file .../ibm6000` on crafted
/// buffers. That operator-semantics bug has since been fixed (see GOTCHAS
/// S13.3 and S2.3): `apply_bitwise_and` now implements `(value & operand)
/// == operand`, matching libmagic's `magiccheck()`. This blob is kept
/// exactly as originally generated (rather than perturbed to dodge the old
/// bug) so this test doubles as a regression guard for that fix.
fn deterministic_binary_blob() -> Vec<u8> {
    (0..64u32)
        .map(|i| u8::try_from((i * 37 + 11) % 256).expect("modulo 256 always fits in u8"))
        .collect()
}

/// GOTCHAS S13.2 (the exact repro from the bug report): with the real
/// system magic DB and the library's default configuration
/// (`stop_at_first_match: true`, matching what `MagicDatabase::load_from_file`
/// and the CLI both use), an assembler-source file must no longer produce
/// a blank description.
#[test]
fn test_default_config_assembler_source_no_longer_blank() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             default-config assembler test skipped cleanly"
        );
        return;
    }

    // Default config: do NOT override stop_at_first_match.
    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let result = db
        .evaluate_buffer(b"\t.asciiz \"hi\"\n")
        .expect("evaluation must not fatally error");

    // The ORIGINAL bug (blank output) is host-independent -- it must never
    // recur, whatever the host DB contains. This is the core regression
    // guard for the maintainer-reported bug.
    assert!(
        !result.description.trim().is_empty(),
        "description must not be blank for an assembler-source buffer, got: {:?}",
        result.description
    );

    // The assembler *classification* is host-dependent (macOS ships the
    // source magic rules under `/usr/share/file/magic/`; Ubuntu ships only
    // the compiled `magic.mgc`, leaving that `--magic-file` dir empty).
    // Assert it against what real `file` reports on the SAME DB rather than
    // hardcoding a host-specific outcome. See
    // `test_default_config_messageless_gate_does_not_blank_output_end_to_end`
    // for the host-INDEPENDENT proof that the message-shadowing fix works.
    if has_file_binary() {
        let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let sample_path = temp_dir.path().join("leading_tab_asciiz.s");
        std::fs::write(&sample_path, b"\t.asciiz \"hi\"\n").expect("failed to write sample");
        let file_says_assembler = file_binary_detects_assembler(&sample_path);
        assert_eq!(
            contains_assembler_signal(&result.description),
            file_says_assembler,
            "assembler-detection parity failure under default config: rmagic description={:?}, \
             GNU `file` detected assembler={file_says_assembler}",
            result.description
        );
    }
}

/// Companion to the assembler case: plain ASCII text with no magic-file
/// signature anywhere must fall back to the text/data classifier's
/// `"ASCII text"` (GOTCHAS S13.2) under the default config, not a blank
/// description.
#[test]
fn test_default_config_plain_ascii_text_no_longer_blank() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             default-config plain-text test skipped cleanly"
        );
        return;
    }

    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let result = db
        .evaluate_buffer(b"hello world, this is not assembler\n")
        .expect("evaluation must not fatally error");

    assert_eq!(result.description, "ASCII text");
}

/// Default-config regression for the message-bearing `clear` fix
/// (GOTCHAS S13.6): a C source file with `#include` must classify as
/// `c program text` -- the trailing `program text` fragment comes from
/// c-lang's `>>&0 clear x program text` child, which rmagic previously
/// dropped (printing only `c`).
///
/// This is the CRITICAL companion to the synthetic
/// `test_clear_with_message_emits_and_still_resets_flag` in
/// `tests/meta_types_integration.rs`: that test runs with
/// `stop_at_first_match(false)` on a hand-built chain, whereas the real
/// bug -- and the CLI -- runs under the DEFAULT config
/// (`stop_at_first_match: true`) against the full c-lang chain with
/// competing top-level `pragma`/`endif` rules. Without this test a
/// refactor of stop-at-first-match could silently regress c.c back to
/// `c` while the synthetic test keeps passing.
///
/// Host-gated on real `file` parity over the SAME `--magic-file` dir
/// (macOS ships the c-lang source rules there; a host whose
/// `/usr/share/file/magic/` lacks them makes `file` not say
/// "c program text" either, so the assertion is vacuous rather than a
/// false failure). Asserts `contains`, not `==`: real `file` also
/// appends `, ASCII text` (the deferred combined-classification garnish,
/// intentionally absent in rmagic).
#[test]
fn test_default_config_c_source_is_c_program_text() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             default-config c-source test skipped cleanly"
        );
        return;
    }

    // Default config: do NOT override stop_at_first_match -- this is the
    // exact path the CLI and the original bug exercise.
    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let c_source = b"#include <stdio.h>\nint main(void){return 0;}\n";
    let result = db
        .evaluate_buffer(c_source)
        .expect("evaluation must not fatally error");

    if has_file_binary() {
        let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let sample_path = temp_dir.path().join("sample.c");
        std::fs::write(&sample_path, c_source).expect("failed to write sample");

        let output = Command::new("file")
            .arg("--magic-file")
            .arg(SYSTEM_MAGIC_DIR)
            .arg(&sample_path)
            .output()
            .expect("failed to invoke the `file` binary");
        let file_desc = String::from_utf8_lossy(&output.stdout);

        // Only assert parity when the host's own `file` (using the SAME
        // magic dir) detects the c-lang chain. This keeps the test
        // host-independent: where the c-lang rules are absent, `file`
        // won't say "c program text" and neither must rmagic.
        if file_desc.contains("c program text") {
            assert!(
                result.description.contains("c program text"),
                "message-bearing `clear` regression: rmagic must emit \
                 \"c program text\" for a C source under the default config, \
                 got: {:?} (file said: {:?})",
                result.description,
                file_desc.trim()
            );
        }
    }
}

/// Binary content must still fall back to `"data"` under the default
/// config -- the text/data fallback must not misclassify binary content
/// as text.
#[test]
fn test_default_config_binary_blob_is_data() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             default-config binary-blob test skipped cleanly"
        );
        return;
    }

    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let result = db
        .evaluate_buffer(&deterministic_binary_blob())
        .expect("evaluation must not fatally error");

    assert_eq!(result.description, "data");
}

/// GATE 2 (full differential parity, default config): the exact
/// three-case set the fix's acceptance criteria call out -- an
/// assembler-source file, a plain ASCII text file, and a binary blob --
/// each run through both `rmagic` (via the library API, default config)
/// and the real `file` binary, on the same system magic DB. Unlike
/// `test_differential_parity_against_gnu_file_for_assembler_detection`
/// above, this does NOT disable `stop_at_first_match`: it is the
/// end-to-end proof that the default configuration (what the CLI and
/// `MagicDatabase::load_from_file` actually use) matches GNU `file`,
/// not just that the assembler signal is present somewhere in an
/// exhaustive match list.
#[test]
fn test_default_config_differential_parity_three_cases() {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !has_system_magic_dir(system_dir) {
        eprintln!(
            "SKIP: {SYSTEM_MAGIC_DIR} not present on this host -- \
             default-config differential parity test skipped cleanly"
        );
        return;
    }
    if !has_file_binary() {
        eprintln!("SKIP: `file` binary not on PATH -- differential parity test skipped cleanly");
        return;
    }

    let db = MagicDatabase::load_from_file(system_dir)
        .expect("loading the system magic directory must not fail");

    let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir for samples");

    // (filename, content, a predicate over (rmagic_description, file_stdout)
    // proving parity on the dimension that matters for that case).
    let cases: &[(&str, &[u8])] = &[
        ("assembler.s", b"\t.asciiz \"hi\"\n"),
        ("plain.txt", b"hello world, this is not assembler\n"),
    ];

    for (filename, content) in cases {
        let path = temp_dir.path().join(filename);
        std::fs::write(&path, content).expect("failed to write sample file");

        let rmagic_result = db
            .evaluate_buffer(content)
            .expect("evaluation must not fatally error");
        let file_output = Command::new("file")
            .arg("--magic-file")
            .arg(SYSTEM_MAGIC_DIR)
            .arg(&path)
            .output()
            .expect("failed to invoke the `file` binary");
        assert!(file_output.status.success());
        let file_stdout = String::from_utf8_lossy(&file_output.stdout);

        assert!(
            !rmagic_result.description.trim().is_empty(),
            "rmagic must not produce a blank description for {filename:?}"
        );

        if *filename == "assembler.s" {
            // Host-adaptive: assert rmagic AGREES with real `file` on whether
            // this host's DB classifies the sample as assembler, not that it
            // unconditionally does. Ubuntu's `--magic-file /usr/share/file/magic/`
            // dir is empty (only compiled `magic.mgc` is shipped), so neither
            // side detects assembler there; macOS's dir carries the rule, so
            // both do. Either way rmagic must match `file`.
            // Use the full `contains_assembler_signal` phrase, NOT a bare
            // "assembler" substring -- `file`'s stdout includes the file
            // PATH ("/tmp/.../assembler.s: ..."), so a bare substring would
            // spuriously match the filename rather than the classification.
            assert_eq!(
                contains_assembler_signal(&rmagic_result.description),
                contains_assembler_signal(&file_stdout),
                "assembler-detection parity failure for {filename:?}: \
                 rmagic={:?}, file={file_stdout:?}",
                rmagic_result.description
            );
        } else {
            assert_eq!(rmagic_result.description, "ASCII text");
            assert!(
                file_stdout.contains("ASCII text"),
                "GNU `file` should also report ASCII text for {filename:?}, got: {file_stdout:?}"
            );
        }
    }

    // Binary blob: both must agree on "data".
    let binary_path = temp_dir.path().join("blob.bin");
    let blob = deterministic_binary_blob();
    std::fs::write(&binary_path, &blob).expect("failed to write binary blob");

    let rmagic_blob_result = db
        .evaluate_buffer(&blob)
        .expect("evaluation must not fatally error");
    let file_blob_output = Command::new("file")
        .arg("--magic-file")
        .arg(SYSTEM_MAGIC_DIR)
        .arg(&binary_path)
        .output()
        .expect("failed to invoke the `file` binary");
    assert!(file_blob_output.status.success());
    let file_blob_stdout = String::from_utf8_lossy(&file_blob_output.stdout);

    assert_eq!(rmagic_blob_result.description, "data");
    assert!(
        file_blob_stdout.contains("data"),
        "GNU `file` should also report \"data\" for the binary blob, got: {file_blob_stdout:?}"
    );
}

// =======================================================================
// Portable, host-INDEPENDENT end-to-end guards for the stop_at_first_match
// message-shadowing fix (GOTCHAS S13.2) and the text/data fallback (S13.3).
//
// Unlike every test above, these do NOT depend on the host system magic DB
// (which differs across platforms -- macOS ships source magic files under
// `/usr/share/file/magic/`; Ubuntu ships only the compiled `magic.mgc`,
// leaving that directory empty). They build self-contained temp magic files
// so they exercise the full default-config pipeline (load -> evaluate ->
// description assembly -> message-bearing stop gate -> text/data fallback)
// identically on every CI host. These are the authoritative regression
// guards for the fix; the system-DB tests above are supplementary parity
// checks that only exercise the assembler path where the host DB carries
// the rule.
// =======================================================================

/// S13.3 fallback wiring: when no rule produces a description (here, a
/// single message-LESS rule matches but carries no text), the default
/// config must fall back to the text/data classifier rather than emitting
/// a blank description. Host-independent.
#[test]
fn test_default_config_no_match_falls_back_to_text_classification_end_to_end() {
    let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
    let magic_path = temp_dir.path().join("messageless_only.magic");
    // A single message-LESS rule: matches any buffer whose first byte is a
    // tab (0x09), carries no description of its own.
    std::fs::write(&magic_path, "0\tbyte\t0x09\n").expect("failed to write magic file");

    let db = MagicDatabase::load_from_file(&magic_path)
        .expect("loading a single-rule magic file must not fail");
    let result = db
        .evaluate_buffer(b"\t.asciiz \"hi\"\n")
        .expect("evaluation must not fatally error");

    assert_eq!(
        result.description, "ASCII text",
        "a match with no usable description text must fall back to the text \
         classifier, not a blank description; got: {:?}",
        result.description
    );
}

/// S13.2 shadow gate (the exact shape of the maintainer-reported bug): a
/// message-LESS gating rule that matches and sorts FIRST must NOT shadow a
/// later message-BEARING rule under the default config. Two `string` rules
/// with identical strength are used so the STABLE strength sort
/// (`sort_by_cached_key`) preserves source order -- the message-less rule
/// is evaluated first. Pre-fix this halted evaluation and produced a blank
/// description (then the text fallback); post-fix evaluation continues to
/// the message-bearing rule and surfaces its text. Host-independent, so it
/// proves the fix on Ubuntu/Windows where the system DB has no assembler
/// rule to exercise.
#[test]
fn test_default_config_messageless_gate_does_not_blank_output_end_to_end() {
    let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
    let magic_path = temp_dir.path().join("gate_then_message.magic");
    // Rule 1 (message-less) and Rule 2 (message-bearing) have identical
    // type/offset/operator/value, hence identical strength; the stable sort
    // keeps Rule 1 first. Both match `\t.asciiz` at offset 0.
    std::fs::write(
        &magic_path,
        "0\tstring\t\\t.asciiz\n0\tstring\t\\t.asciiz\tassembler source text\n",
    )
    .expect("failed to write magic file");

    let db = MagicDatabase::load_from_file(&magic_path)
        .expect("loading the two-rule magic file must not fail");
    let result = db
        .evaluate_buffer(b"\t.asciiz \"hi\"\n")
        .expect("evaluation must not fatally error");

    assert!(
        result.description.contains("assembler source text"),
        "the message-bearing rule's text must survive a preceding message-less \
         match under stop_at_first_match; got: {:?}",
        result.description
    );
}
