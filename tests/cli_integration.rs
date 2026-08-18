// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! CLI integration tests for rmagic binary
//!
//! These tests use subprocess-based testing with `assert_cmd` for natural process
//! isolation. This approach eliminates the need for fragile fd manipulation and
//! enables reliable execution under llvm-cov.
//!
//! Note: These tests require the `rmagic` binary to be built (handled
//! automatically by `assert_cmd`).

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate instead.
#![allow(clippy::expect_used)]

mod common;

use common::{ELF_HEADER, create_data_file, path_str, rmagic_cmd};
use libmagic_rs::EvaluationConfig;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// Magic byte constants for test file creation
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SOI: &[u8] = b"\xff\xd8\xff\xe0";
const PDF_HEADER: &[u8] = b"%PDF-1.4";
const ZIP_HEADER: &[u8] = b"PK\x03\x04";
const GIF_HEADER: &[u8] = b"GIF89a";

/// Helper to create a temporary magic file for testing
fn create_magic_file(dir: &TempDir, content: &str) -> std::path::PathBuf {
    let path = dir.path().join("test.magic");
    fs::write(&path, content).expect("Failed to create magic file");
    path
}

// =============================================================================
// Builtin Format Detection Tests
// =============================================================================

#[test]
fn test_builtin_format_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Formats with definite builtin detection
    let detected_cases = [
        ("test.elf", ELF_HEADER, "ELF"),
        ("test.png", PNG_SIGNATURE, "PNG"),
        ("test.jpg", JPEG_SOI, "JPEG"),
        ("test.zip", ZIP_HEADER, "ZIP"),
    ];

    for (filename, content, expected) in detected_cases {
        let test_file = create_data_file(&temp_dir, filename, content);
        rmagic_cmd()
            .args(["--use-builtin", path_str(&test_file)])
            .assert()
            .success()
            .stdout(predicate::str::contains(expected));
    }

    // PDF and GIF are not currently detected by builtin rules, so they fall
    // through to the text/data fallback (GOTCHAS S13.2). `PDF_HEADER`
    // (`%PDF-1.4`) and `GIF_HEADER` (`GIF89a`) are both plain ASCII, so the
    // fallback classifies them as "ASCII text" (matching GNU `file`'s
    // ascmagic behavior for readable content), not "data" -- "data" is
    // reserved for genuinely binary content. We verify the CLI runs
    // without error and produces either the format name or the text
    // fallback.
    let fallback_cases = [
        ("test.pdf", PDF_HEADER, "PDF"),
        ("test.gif", GIF_HEADER, "GIF"),
    ];

    for (filename, content, format_name) in fallback_cases {
        let test_file = create_data_file(&temp_dir, filename, content);
        rmagic_cmd()
            .args(["--use-builtin", path_str(&test_file)])
            .assert()
            .success()
            .stdout(
                predicate::str::contains(format_name).or(predicate::str::contains("ASCII text")),
            );
    }
}

#[test]
fn test_builtin_with_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&test_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_builtin_with_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&test_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_builtin_unknown_file_returns_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Genuinely binary (non-ASCII, invalid-UTF-8) content so the text/data
    // fallback (GOTCHAS S13.2) reports "data" -- an ASCII buffer would
    // instead fall back to "ASCII text", which is what this test's name
    // and assertion specifically exercise.
    let test_file = create_data_file(
        &temp_dir,
        "unknown.bin",
        &[0x00, 0x01, 0x02, 0xFF, 0xFE, 0x80, 0x81],
    );

    rmagic_cmd()
        .args(["--use-builtin", path_str(&test_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

// =============================================================================
// Stdin Tests
// =============================================================================

#[test]
fn test_stdin_format_detection() {
    let cases: &[(&str, &[u8], Option<&str>)] = &[
        ("ELF via stdin", ELF_HEADER, Some("ELF")),
        ("PNG via stdin", PNG_SIGNATURE, Some("PNG")),
        // Empty input falls back to "empty" (GOTCHAS S13.2 text/data
        // fallback), matching GNU `file`'s literal output for a
        // zero-byte input -- not the old hardcoded "data".
        ("empty stdin", b"", Some("empty")),
        ("unknown content", b"sample data", None),
    ];

    for (label, input, expected_substr) in cases {
        let assertion = rmagic_cmd()
            .args(["--use-builtin", "-"])
            .write_stdin(*input)
            .assert()
            .success()
            .stdout(predicate::str::contains("stdin:"));

        if let Some(substr) = expected_substr {
            assertion.stdout(predicate::str::contains(*substr));
        }

        // Satisfy the borrow checker - label is used for debugging context
        let _ = label;
    }
}

#[test]
fn test_stdin_output_format_json() {
    rmagic_cmd()
        .args(["--use-builtin", "--json", "-"])
        .write_stdin(ELF_HEADER)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""));
}

#[test]
fn test_stdin_with_strict() {
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "-"])
        .write_stdin(ELF_HEADER)
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_stdin_truncation_warning() {
    // Derive threshold from configuration to avoid hardcoded assumptions
    let max_string_length = EvaluationConfig::default().max_string_length;
    // Create input larger than max_string_length
    let large_input = vec![b'a'; max_string_length + 8];

    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(large_input)
        .assert()
        .success()
        .stderr(predicate::str::contains("Warning: stdin input truncated"));
}

#[test]
fn test_stdin_no_false_truncation_warning() {
    // Derive threshold from configuration to avoid hardcoded assumptions
    let max_string_length = EvaluationConfig::default().max_string_length;
    // Input exactly at max_string_length should NOT trigger warning
    let exact_input = vec![b'a'; max_string_length];

    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(exact_input)
        .assert()
        .success()
        .stderr(predicate::str::contains("truncated").not());
}

// =============================================================================
// Strict-Mode Stdin Error Tests
// =============================================================================

#[test]
fn test_stdin_strict_mode_with_empty_input() {
    // Empty stdin in strict mode should still succeed (empty file is
    // valid). The text/data fallback (GOTCHAS S13.2) reports "empty" for
    // a zero-byte buffer, matching GNU `file` -- not the old hardcoded
    // "data".
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "-"])
        .write_stdin(b"" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin: empty"));
}

#[test]
fn test_stdin_non_strict_continues_on_unknown() {
    // Non-strict mode should continue without error on unknown content.
    // Genuinely binary content so the text/data fallback reports "data"
    // rather than "ASCII text".
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(&[0x00u8, 0x01, 0x02, 0xFF, 0xFE, 0x80, 0x81] as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

#[test]
fn test_multiple_inputs_strict_mode_stdin_first() {
    // Test stdin with other files in strict mode
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--strict", "-", path_str(&elf_file)])
        .write_stdin(ELF_HEADER)
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"))
        .stdout(predicate::str::contains("ELF"));
}

// =============================================================================
// Multiple File Tests
// =============================================================================

#[test]
fn test_multiple_files_sequential_output() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);
    let png_file = create_data_file(&temp_dir, "test.png", PNG_SIGNATURE);
    let zip_file = create_data_file(&temp_dir, "test.zip", ZIP_HEADER);

    rmagic_cmd()
        .args([
            "--use-builtin",
            path_str(&elf_file),
            path_str(&png_file),
            path_str(&zip_file),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("PNG"))
        .stdout(predicate::str::contains("ZIP"));
}

#[test]
fn test_multiple_files_with_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);
    let png_file = create_data_file(&temp_dir, "test.png", PNG_SIGNATURE);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            path_str(&elf_file),
            path_str(&png_file),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("PNG"));
}

#[test]
fn test_multiple_files_with_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);
    let png_file = create_data_file(&temp_dir, "test.png", PNG_SIGNATURE);

    let output = rmagic_cmd()
        .args([
            "--use-builtin",
            "--json",
            path_str(&elf_file),
            path_str(&png_file),
        ])
        .assert()
        .success();

    // JSON Lines format should have one JSON object per line
    let stdout = String::from_utf8(output.get_output().stdout.clone())
        .expect("stdout should be valid UTF-8");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should have 2 JSON lines for 2 files");
}

#[test]
fn test_multiple_files_with_custom_magic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_magic_file(&temp_dir, "# Test magic\n0 byte 0x7f ELF marker\n");
    let data_file = create_data_file(&temp_dir, "test1.bin", b"\x7fELF\x02\x01\x01\x00");
    let data_file2 = create_data_file(&temp_dir, "test2.bin", b"\x7fELF\x01\x01\x01\x00");

    rmagic_cmd()
        .args([
            "--magic-file",
            path_str(&magic_file),
            path_str(&data_file),
            path_str(&data_file2),
        ])
        .assert()
        .success();
}

#[test]
fn test_multiple_files_partial_failure_non_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);
    let nonexistent = temp_dir.path().join("nonexistent.bin");

    rmagic_cmd()
        .args(["--use-builtin", path_str(&elf_file), path_str(&nonexistent)])
        .assert()
        .success() // Non-strict mode should succeed overall
        .stdout(predicate::str::contains("ELF"))
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_multiple_files_partial_failure_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);
    let nonexistent = temp_dir.path().join("nonexistent.bin");

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            path_str(&elf_file),
            path_str(&nonexistent),
        ])
        .assert()
        .failure() // Strict mode should fail
        .code(3); // File not found exit code
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_error_file_not_found() {
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "nonexistent_file.bin"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_directory_instead_of_file_is_classified_not_an_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // This test previously asserted the opposite: a directory under
    // `--strict` failed with "directory" on stderr. GNU `file` classifies
    // directories (`file <dir>` -> `<dir>: directory`, exit 0), and under
    // ADR-0001 that string is a detection result, so the old behavior was a
    // contract gap. Rewritten rather than deleted to keep the coverage.
    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(temp_dir.path())])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_error_magic_file_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.bin", b"test");

    let nonexistent_magic = temp_dir.path().join("nonexistent.magic");
    rmagic_cmd()
        .args([
            "--magic-file",
            path_str(&nonexistent_magic),
            path_str(&test_file),
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("Magic file"));
}

#[test]
fn test_error_empty_magic_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_magic_file(&temp_dir, "");
    let test_file = create_data_file(&temp_dir, "test.bin", b"test");

    rmagic_cmd()
        .args(["--magic-file", path_str(&magic_file), path_str(&test_file)])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn test_error_argument_validation() {
    let cases: &[&[&str]] = &[
        &[],                                                            // no files
        &["--json", "--text", "test.bin"],                              // conflicting flags
        &["--use-builtin", "--magic-file", "custom.magic", "test.bin"], // builtin + magic-file
    ];

    for args in cases {
        rmagic_cmd().args(*args).assert().failure().code(2);
    }
}

// =============================================================================
// Timeout Tests
// =============================================================================

#[test]
fn test_timeout_argument_parsing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    // Valid timeout value
    rmagic_cmd()
        .args([
            "--use-builtin",
            "--timeout-ms",
            "1000",
            path_str(&test_file),
        ])
        .assert()
        .success();
}

#[test]
fn test_timeout_invalid_values() {
    let cases = [
        &["--use-builtin", "--timeout-ms", "0", "test.bin"][..],
        &["--use-builtin", "--timeout-ms", "999999999", "test.bin"][..],
    ];

    for args in cases {
        rmagic_cmd().args(args).assert().failure().code(2);
    }
}

// =============================================================================
// Output Format Tests
// =============================================================================

#[test]
fn test_output_text_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--text", path_str(&test_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains(":"))
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_output_json_single_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&test_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("]"));
}

// =============================================================================
// Shell Completion Tests
// =============================================================================

#[test]
fn test_generate_completions() {
    let cases = [
        ("bash", "_rmagic"),
        ("zsh", "#compdef"),
        ("fish", "complete"),
    ];

    for (shell, expected) in cases {
        rmagic_cmd()
            .args(["--generate-completion", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains(expected));
    }
}

// =============================================================================
// Custom Magic File Tests
// =============================================================================

#[test]
fn test_custom_magic_file_accepted() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_content = "# Test magic file\n0 byte 0x7f ELF magic\n";
    let magic_file = create_magic_file(&temp_dir, magic_content);
    let data_file = create_data_file(&temp_dir, "test.bin", b"\x7fELF data here");

    rmagic_cmd()
        .args(["--magic-file", path_str(&magic_file), path_str(&data_file)])
        .assert()
        .success();
}

#[test]
fn test_custom_magic_file_fallback_to_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_content = "# Test magic file\n0 byte 0xff Marker\n";
    let magic_file = create_magic_file(&temp_dir, magic_content);
    // Genuinely binary content (no custom rule matches, and it is not
    // ASCII/UTF-8 text) so the text/data fallback (GOTCHAS S13.2) reports
    // "data" as this test's name asserts.
    let data_file = create_data_file(
        &temp_dir,
        "test.bin",
        &[0x00u8, 0x01, 0x02, 0xFF, 0xFE, 0x80, 0x81],
    );

    rmagic_cmd()
        .args(["--magic-file", path_str(&magic_file), path_str(&data_file)])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_file_with_spaces_in_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "file with spaces.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_file_with_unicode_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "test_\u{1F600}.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_empty_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "empty.bin", b"");

    // The text/data fallback (GOTCHAS S13.2) reports "empty" for a
    // zero-byte file, matching GNU `file`'s literal output -- not the
    // old hardcoded "data".
    rmagic_cmd()
        .args(["--use-builtin", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("empty"));
}

#[test]
fn test_very_small_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "small.bin", b"x");

    // A single printable ASCII byte falls back to "ASCII text" (GOTCHAS
    // S13.2), matching GNU `file` -- not "data".
    rmagic_cmd()
        .args(["--use-builtin", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ASCII text"));
}
