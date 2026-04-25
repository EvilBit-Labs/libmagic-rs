// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Security regression tests for the review findings in `.full-review/`.
//!
//! Each test here pins a specific security property so that a future
//! refactor cannot silently reintroduce the vulnerability.
//!
//! Covered findings:
//!
//! * **S-H1** — `default_magic_file_path` must not resolve relative-path
//!   fallbacks against the process cwd (CWE-426, untrusted search path).
//! * **S-H2** — `FileBuffer::new` must use `fstat` on the open descriptor
//!   rather than re-resolving the path for metadata validation
//!   (CWE-367, TOCTOU).
//! * **S-M2** — `build_regex` must reject compile-time-DoS patterns via
//!   `size_limit` / `dfa_size_limit` (CWE-1333).
//! * **T-M2 (S13.1)** — `EvaluationConfig::default()` has no timeout;
//!   this test pins the invariant so a change is a deliberate choice.
//!
//! Tests that require private-module access (codegen round-trip for
//! S-L2, `concatenate_messages` backspace edges for S14.1) live inline
//! in `src/parser/codegen.rs` and `src/lib.rs` respectively.

use assert_cmd::Command;
use libmagic_rs::EvaluationConfig;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// S-H1: Untrusted search path
// =============================================================================

/// A planted `./missing.magic` in the process cwd must not be picked up by
/// the rmagic CLI's default-path fallback chain.
#[test]
fn test_cli_rejects_planted_missing_magic_in_cwd() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("missing.magic"),
        "0 string TEST planted-magic-pwn\n",
    )
    .unwrap();
    let target = dir.path().join("target.bin");
    fs::write(&target, b"TEST").unwrap();

    let out = Command::cargo_bin("rmagic")
        .unwrap()
        .current_dir(dir.path())
        .arg(target.file_name().unwrap())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains("planted-magic-pwn"),
        "CLI resolved planted ./missing.magic from cwd (S-H1 regression)\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

/// A planted `./third_party/magic.mgc` in the process cwd must not be
/// picked up even when `CI` or `GITHUB_ACTIONS` env vars are set.
#[test]
fn test_cli_rejects_planted_third_party_magic_in_ci_env() {
    let dir = TempDir::new().unwrap();
    let tp = dir.path().join("third_party");
    fs::create_dir_all(&tp).unwrap();
    fs::write(tp.join("magic.mgc"), "0 string EVIL planted-ci-magic-pwn\n").unwrap();
    let target = dir.path().join("target.bin");
    fs::write(&target, b"EVIL").unwrap();

    let out = Command::cargo_bin("rmagic")
        .unwrap()
        .current_dir(dir.path())
        .env("CI", "true")
        .env("GITHUB_ACTIONS", "true")
        .arg(target.file_name().unwrap())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains("planted-ci-magic-pwn"),
        "CLI resolved planted third_party/magic.mgc under CI env (S-H1 regression)\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

// =============================================================================
// S-H2: TOCTOU race
// =============================================================================

/// `FileBuffer::new` must read metadata via `fstat` on the already-open
/// file, so a symlink swap after `open_file` cannot influence the
/// validated metadata. We cannot reliably race a real TOCTOU in a unit
/// test, but we can assert the *contract* that the error path reports the
/// caller-supplied path (rather than a canonicalized variant), which is
/// only possible if the path is not re-resolved.
#[test]
fn test_file_buffer_error_uses_caller_path_not_canonical() {
    use libmagic_rs::io::{FileBuffer, IoError};
    use std::path::PathBuf;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.bin");
    fs::write(&path, b"").unwrap();

    let err = FileBuffer::new(&path).unwrap_err();
    match err {
        IoError::EmptyFile { path: reported } => {
            assert_eq!(
                reported,
                PathBuf::from(&path),
                "EmptyFile error should report caller-supplied path, not canonicalized"
            );
        }
        other => panic!("Expected EmptyFile, got {other:?}"),
    }
}

// =============================================================================
// S-M2: Regex compile-time DoS
// =============================================================================

/// Pathological patterns that would otherwise consume hundreds of MB of
/// NFA/DFA state must be rejected by the regex compiler's `size_limit`.
#[test]
fn test_regex_compile_bounded_for_pathological_patterns() {
    use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
    use libmagic_rs::parser::ast::{RegexCount, RegexFlags};
    use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};
    use std::time::Instant;

    let cases: &[(&str, &str)] = &[
        ("[a-z]{1000000}", "huge character-class repetition"),
        ("a{1000000}", "huge literal repetition"),
        (".{1000000}", "huge any-char repetition"),
    ];
    let buf = vec![b'a'; 128];
    let config = EvaluationConfig::default().with_timeout_ms(Some(1000));

    for (pat, label) in cases {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Regex {
                flags: RegexFlags::default(),
                count: RegexCount::Default,
            },
            op: Operator::Equal,
            value: Value::String((*pat).to_string()),
            message: "never-matches".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        };

        let mut ctx = EvaluationContext::new(config.clone());
        let start = Instant::now();
        let _ = evaluate_rules(&[rule], &buf, &mut ctx);
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 500,
            "{label}: pathological regex ran for {elapsed:?} (S-M2 regression)"
        );
    }
}

// =============================================================================
// T-M2 / GOTCHAS S13.1: `EvaluationConfig::default()` has no timeout
// =============================================================================

/// Pin the invariant that `EvaluationConfig::default()` leaves `timeout_ms`
/// unset (unbounded). GOTCHAS S13.1 documents this as intentional but warns
/// downstream consumers. If this test fails, either update GOTCHAS S13.1
/// and the rustdoc `# Security` sections on the `MagicDatabase`
/// constructors, or revert the `Default` change.
#[test]
fn test_evaluation_config_default_is_unbounded() {
    let cfg = EvaluationConfig::default();
    assert_eq!(
        cfg.timeout_ms, None,
        "EvaluationConfig::default() is expected to leave timeout_ms unset. \
         If you are intentionally changing this behavior, update GOTCHAS S13.1 \
         and the rustdoc `# Security` section on the MagicDatabase constructors."
    );
}
