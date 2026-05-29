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
//! * **2A-H1 / 3A-C2** — `EvaluationConfig::max_string_length` must be
//!   threaded into both the unflagged string dispatcher (`read_typed_value_with_pattern`)
//!   AND the flagged-string dispatcher (`read_pattern_match`). Origin
//!   `.full-review/05-final-report.md` documented the cap as a working
//!   CWE-770 countermeasure that was never wired. These tests pin the
//!   fix on both paths.
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
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Regex {
                flags: RegexFlags::default(),
                count: RegexCount::Default,
            },
            Operator::Equal,
            Value::String((*pat).to_string()),
            "never-matches".to_string(),
        );

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

// =============================================================================
// 2A-H1 / 3A-C2: max_string_length must be honored on both string-read paths
// =============================================================================

/// Helper: build a 1 MiB NUL-free buffer to stress the cap.
fn one_mib_nul_free() -> Vec<u8> {
    vec![b'A'; 1_048_576]
}

/// Build a `string x` rule (unflagged any-value) with no AST max_length.
/// Exercises the `read_typed_value_with_pattern` `(None, _)` arm. The
/// rule's `value` field is `Value::Uint(0)` (sentinel for `x`); using
/// `Value::String("")` would route through the `(None, Some(Value::String(p)))`
/// arm which reads `p.len() == 0` bytes — not the path we want to exercise.
fn unflagged_string_x_rule() -> libmagic_rs::MagicRule {
    use libmagic_rs::parser::ast::StringFlags;
    use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};
    MagicRule::new(
        OffsetSpec::Absolute(0),
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        Operator::AnyValue,
        Value::Uint(0),
        "captured: %s".to_string(),
    )
}

/// Build a `string/<flag> "<pattern>"` rule for testing the flagged-string
/// scan window. Flagged strings reject `Operator::AnyValue` (the engine
/// requires `Equal`/`NotEqual` for pattern-bearing types), so we use a
/// concrete pattern and verify match-vs-no-match based on whether the
/// scan window covers the pattern position.
fn flagged_string_equal_rule(
    flags: libmagic_rs::parser::ast::StringFlags,
    pattern: &str,
) -> libmagic_rs::MagicRule {
    use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};
    MagicRule::new(
        OffsetSpec::Absolute(0),
        TypeKind::String {
            max_length: None,
            flags,
        },
        Operator::Equal,
        Value::String(pattern.to_string()),
        "flagged hit".to_string(),
    )
}

/// Run an evaluation against the rule and return the first match's `Value`
/// if the rule matched, or `None` on no-match.
fn captured_value(
    rule: &libmagic_rs::MagicRule,
    buffer: &[u8],
    cap: usize,
) -> Option<libmagic_rs::parser::ast::Value> {
    use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
    let config = EvaluationConfig::default()
        .with_max_string_length(cap)
        .with_timeout_ms(Some(5_000));
    let mut ctx = EvaluationContext::new(config);
    let matches = evaluate_rules(std::slice::from_ref(rule), buffer, &mut ctx)
        .expect("evaluate_rules should not error for these simple rules");
    matches.into_iter().next().map(|m| m.value)
}

/// Extract the byte length of a captured string-shaped `Value`.
fn captured_len(v: &libmagic_rs::parser::ast::Value) -> usize {
    use libmagic_rs::parser::ast::Value;
    match v {
        Value::String(s) => s.len(),
        Value::Bytes(b) => b.len(),
        other => panic!("expected string/bytes capture, got {other:?}"),
    }
}

/// Unflagged path: `0 string x` against a 1 MiB NUL-free buffer with
/// `max_string_length = 64` must produce a capture bounded to 64 bytes.
/// Pre-fix this allocates the full buffer (~1 MiB) because the dispatcher
/// passes `None` to `read_string`. Origin finding 2A-H1.
#[test]
fn test_max_string_length_caps_unflagged_string_x() {
    let buf = one_mib_nul_free();
    let rule = unflagged_string_x_rule();
    let captured =
        captured_value(&rule, &buf, 64).expect("unflagged `string x` should match any buffer");
    let len = captured_len(&captured);
    assert_eq!(
        len, 64,
        "unflagged string x must cap at max_string_length=64; got {len} bytes \
         (2A-H1 regression: dispatcher dropped the cap)"
    );
}

/// Flagged path with non-zero offset: pins that the `scan_buffer`
/// construction caps the buffer's UPPER bound rather than pre-slicing
/// from `offset`. A future "simplification" that swaps
/// `&buffer[..end]` for `&buffer[offset..end]` would silently double-
/// offset the comparator (which slices `buffer.get(offset..)?`
/// internally) and break every flagged-string rule at non-zero offset.
///
/// This test was added in response to PR #304 review finding TS-1 from
/// `pr-review-toolkit:review-pr`.
#[test]
fn test_max_string_length_flagged_path_works_at_non_zero_offset() {
    use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
    use libmagic_rs::parser::ast::StringFlags;
    use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};

    // Buffer: 50 bytes of 'A', then the literal pattern "hit" at offset 50.
    let mut buf = vec![b'A'; 50];
    buf.extend_from_slice(b"hit");

    // Flagged-`/c` rule at offset 50, looking for "hit". With a cap of
    // 1024 (well above the offset + pattern), the comparator must find
    // the match. If the scan_buffer construction were pre-sliced from
    // `offset`, the comparator's internal `get(offset..)` would skip
    // past the pattern and find nothing.
    let rule = MagicRule::new(
        OffsetSpec::Absolute(50),
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default().with_ignore_lowercase(true),
        },
        Operator::Equal,
        Value::String("hit".to_string()),
        "found at offset".to_string(),
    );
    let config = EvaluationConfig::default().with_max_string_length(1024);
    let mut ctx = EvaluationContext::new(config);
    let matches =
        evaluate_rules(std::slice::from_ref(&rule), &buf, &mut ctx).expect("must not error");
    assert_eq!(
        matches.len(),
        1,
        "flagged string/c at offset 50 must match `hit` with cap=1024; \
         a regression to pre-slice from offset would break this"
    );
}

/// Flagged-`/W` path: a `string/W " X"` rule against a buffer of all
/// whitespace must walk only `max_string_length` bytes before giving up,
/// not the full buffer. Origin 2A-H1 (flagged-string scan-window variant)
/// — the `/W` operator consumes greedy whitespace which without a cap
/// could walk an arbitrarily large buffer. The test uses a buffer too
/// large to consume completely in any reasonable time bound; the U1 cap
/// prevents the runaway walk.
///
/// We assert no-match (the pattern ` X` requires a literal `X` after the
/// leading whitespace; the buffer is all spaces with no `X`), and that
/// the evaluation completes well under the cap-implied wall-clock bound.
/// A correctly capped walk completes in microseconds; an uncapped walk
/// through 16 MiB takes meaningfully longer.
#[test]
fn test_max_string_length_caps_flagged_w_whitespace_walk() {
    use libmagic_rs::parser::ast::StringFlags;
    use std::time::Instant;

    // 16 MiB of whitespace — large enough that an uncapped walk is
    // observably slower than a capped one, but not so large that test
    // setup dominates the run time.
    let buf = vec![b' '; 16 * 1024 * 1024];
    let rule =
        flagged_string_equal_rule(StringFlags::default().with_compact_whitespace(true), " X");

    let cap = 1024usize;
    let start = Instant::now();
    let result = captured_value(&rule, &buf, cap);
    let elapsed = start.elapsed();

    assert!(
        result.is_none(),
        "flagged string/W ' X' must NOT match an all-whitespace buffer; got {result:?}"
    );
    // With a 1024-byte cap the comparator walks at most ~1024 bytes.
    // A pessimistic bound of 100 ms covers any reasonable CI environment;
    // an uncapped walk through 16 MiB takes substantially longer.
    assert!(
        elapsed.as_millis() < 100,
        "flagged string/W against 16 MiB whitespace ran for {elapsed:?} \
         (2A-H1 regression: flagged-string scan_buffer ignored max_string_length=1024)"
    );
}

/// Minimum valid cap (cap = 1) must produce a 1-byte result on the
/// unflagged path. `EvaluationConfig::with_max_string_length` is a pure
/// setter and does not validate; `EvaluationConfig::validate()` (called
/// explicitly by validated entry points such as `MagicDatabase`) rejects
/// `max_string_length == 0`. As defense-in-depth, `EvaluationContext::new`
/// clamps `max_string_length == 0` to `DEFAULT_MAX_STRING_LENGTH` so
/// even low-level callers that bypass `validate()` cannot reach a
/// 0-byte cap at evaluation time (SF-1).
#[test]
fn test_max_string_length_minimum_cap_returns_one_byte() {
    let buf = one_mib_nul_free();
    let v = captured_value(&unflagged_string_x_rule(), &buf, 1).expect("must match");
    assert_eq!(
        captured_len(&v),
        1,
        "unflagged: cap=1 must yield 1-byte capture; got {} bytes",
        captured_len(&v)
    );
}

/// NUL before cap: the unflagged path stops at the first NUL even when
/// the configured cap would allow reading further. Confirms the cap is
/// an upper bound, not a target.
#[test]
fn test_max_string_length_unflagged_stops_at_nul_before_cap() {
    let mut buf = b"hello\0".to_vec();
    buf.extend(std::iter::repeat_n(b'A', 1_048_576));
    let v = captured_value(&unflagged_string_x_rule(), &buf, 64).expect("must match");
    assert_eq!(
        captured_len(&v),
        5,
        "unflagged path must stop at NUL even when cap is larger; \
         got {} bytes",
        captured_len(&v)
    );
}

/// SF-1 defense: `EvaluationContext::new` must clamp `max_string_length = 0`
/// to a safe default. The validator at `EvaluationConfig::validate()` rejects
/// 0, but struct-literal construction and the `with_max_string_length`
/// builder can bypass it. Without the clamp, an invalid config silently
/// disables the CWE-770 control.
#[test]
fn test_evaluation_context_clamps_invalid_max_string_length() {
    use libmagic_rs::evaluator::EvaluationContext;
    let invalid = EvaluationConfig::default().with_max_string_length(0);
    let ctx = EvaluationContext::new(invalid);
    assert!(
        ctx.max_string_length() >= 1,
        "EvaluationContext::new must clamp max_string_length=0 to a safe default; \
         got {} (SF-1 regression)",
        ctx.max_string_length()
    );
}

/// Cap larger than remaining buffer must return the entire buffer (or up
/// to the first NUL, whichever comes first). The cap is an upper bound.
#[test]
fn test_max_string_length_cap_larger_than_buffer_returns_full_buffer() {
    let buf = vec![b'A'; 100];
    let v = captured_value(&unflagged_string_x_rule(), &buf, 1_000_000).expect("must match");
    assert_eq!(
        captured_len(&v),
        100,
        "cap larger than buffer should return full buffer; got {} bytes",
        captured_len(&v)
    );
}
