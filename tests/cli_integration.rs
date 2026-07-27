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

use assert_cmd::Command;
use libmagic_rs::EvaluationConfig;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// Magic byte constants for test file creation
const ELF_HEADER: &[u8] = b"\x7fELF\x02\x01\x01\x00";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SOI: &[u8] = b"\xff\xd8\xff\xe0";
const PDF_HEADER: &[u8] = b"%PDF-1.4";
const ZIP_HEADER: &[u8] = b"PK\x03\x04";
const GIF_HEADER: &[u8] = b"GIF89a";

/// Helper to create a Command for the rmagic binary
fn rmagic_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("rmagic"))
}

/// Helper to create a temporary data file for testing
fn create_data_file(dir: &TempDir, filename: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(filename);
    fs::write(&path, content).expect("Failed to create data file");
    path
}

/// Helper to create a temporary magic file for testing
fn create_magic_file(dir: &TempDir, content: &str) -> std::path::PathBuf {
    let path = dir.path().join("test.magic");
    fs::write(&path, content).expect("Failed to create magic file");
    path
}

/// Convert a path to a string, panicking with context on failure
fn path_str(path: &std::path::Path) -> &str {
    path.to_str().expect("Invalid path")
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

// =============================================================================
// Symlink Test Helpers
// =============================================================================

/// Create a symlink at `link` pointing at `target`.
///
/// `FileBuffer::create_symlink` is `pub(crate)` and so is not reachable from an
/// integration test; this mirrors its three-arm platform dispatch. The `#[cfg]`
/// blocks sit inside the body so the function itself compiles everywhere.
fn try_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlink creation is not supported on this platform",
        ))
    }
}

/// Create a symlink, or report that the caller should skip.
///
/// Returns `false` after printing a skip message when symlink creation is not
/// permitted (Windows without developer mode, restricted CI sandboxes). Tests
/// skip at runtime rather than compiling out under `#[cfg(unix)]`, so the
/// symlink suite still runs wherever symlinks happen to be available. Mirrors
/// `src/io/mod.rs`'s `test_file_buffer_symlink_to_directory_rejection`.
#[must_use]
fn symlink_or_skip(target: &std::path::Path, link: &std::path::Path, test_name: &str) -> bool {
    match try_symlink(target, link) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("Skipping {test_name}: cannot create symlink ({e})");
            false
        }
    }
}

#[test]
fn test_symlink_helper_creates_resolvable_file_link() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "target.txt", b"hello");
    let link = temp_dir.path().join("file.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_symlink_helper_creates_resolvable_file_link",
    ) {
        return;
    }

    assert!(
        fs::symlink_metadata(&link)
            .expect("lstat on link")
            .file_type()
            .is_symlink(),
        "helper must create an actual symlink, not a copy"
    );
    assert_eq!(
        fs::read(&link).expect("read through link"),
        b"hello",
        "link must resolve to the target's content"
    );
}

#[test]
fn test_symlink_helper_creates_directory_link() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = temp_dir.path().join("realdir");
    fs::create_dir_all(&target).expect("Failed to create dir");
    let link = temp_dir.path().join("dir.link");

    if !symlink_or_skip(&target, &link, "test_symlink_helper_creates_directory_link") {
        return;
    }

    // `is_dir()` follows symlinks -- this is exactly why the CLI symlink
    // precheck must run before the `is_dir()` branch in `process_file`.
    assert!(
        link.is_dir(),
        "is_dir() must report true through a dir link"
    );
}

#[test]
fn test_symlink_helper_error_arm_is_reachable_and_does_not_panic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "target.txt", b"x");
    let occupied = create_data_file(&temp_dir, "occupied.txt", b"y");

    // Creating a link where a file already exists fails with EEXIST. This
    // exercises the `Err` arm without revoking privileges, proving the
    // runtime-skip path returns cleanly instead of panicking.
    let result = try_symlink(&target, &occupied);
    assert!(
        result.is_err(),
        "creating a link over an existing file must fail"
    );
    assert!(
        !symlink_or_skip(&target, &occupied, "reachability probe"),
        "skip helper must report false rather than panic on the Err arm"
    );
}

// =============================================================================
// Symlink Classification Tests (issue #383)
// =============================================================================

#[test]
fn test_broken_symlink_reports_to_stdout_and_exits_zero() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_reports_to_stdout_and_exits_zero",
    ) {
        return;
    }

    // GNU `file` prints this to stdout and exits 0. Before this change rmagic
    // printed nothing to stdout and an I/O error to stderr (issue #383).
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_symlink_cycle_reports_broken_not_an_eloop_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("cycle.link");

    // A link pointing at itself: `fs::metadata` fails with ELOOP rather than
    // ENOENT. One reachability probe collapses both into the same output.
    if !symlink_or_skip(
        std::path::Path::new("cycle.link"),
        &link,
        "test_symlink_cycle_reports_broken_not_an_eloop_error",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to cycle.link",
        ))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_symlink_relative_and_absolute_targets_render_verbatim() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let cases = [
        ("relative.link", "../../elsewhere/x/y.txt"),
        ("absolute.link", "/nonexistent/absolute/target.txt"),
    ];

    for (link_name, target) in cases {
        let link = temp_dir.path().join(link_name);
        if !symlink_or_skip(
            std::path::Path::new(target),
            &link,
            "test_symlink_relative_and_absolute_targets_render_verbatim",
        ) {
            return;
        }

        // No canonicalization and no parent-joining: the stored target is
        // reproduced exactly as `readlink` would report it.
        rmagic_cmd()
            .args(["--use-builtin", path_str(&link)])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "broken symbolic link to {target}"
            )));
    }
}

#[test]
fn test_empty_target_symlink_is_not_reported_as_broken_with_a_blank_target() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("empty.link");

    // `ln -s "" x` is creatable and `read_link` succeeds on it, so an empty
    // target reaches the classifier rather than the fall-through path.
    if !symlink_or_skip(
        std::path::Path::new(""),
        &link,
        "test_empty_target_symlink_is_not_reported_as_broken_with_a_blank_target",
    ) {
        return;
    }

    // The negative is the requirement: `broken symbolic link to ` with a
    // dangling trailing space is a wrong detection result. The replacement
    // diagnostic's wording is ours to choose.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("broken symbolic link to \n").not())
        .stdout(predicate::str::contains("unreadable symlink"));
}

#[test]
fn test_valid_symlink_still_classifies_its_target() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("valid.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_valid_symlink_still_classifies_its_target",
    ) {
        return;
    }

    // Regression guard: a reachable link must keep resolving through to the
    // target, exactly as before this change.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("symbolic link").not());
}

#[test]
fn test_broken_symlink_alongside_regular_file_reports_both() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_alongside_regular_file_reports_both",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link), path_str(&regular)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        .stdout(predicate::str::contains("ELF"))
        // The `path: ` prefix must survive multi-file output.
        .stdout(predicate::str::contains("dangling.link:"));
}

#[test]
fn test_broken_symlink_json_output_is_coherent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_json_output_is_coherent",
    ) {
        return;
    }

    // The JSON arm builds from `matches`, not `description`, so an empty
    // `matches` would silently produce a valid-but-empty object.
    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ));
}

#[test]
fn test_broken_symlink_with_strict_exits_non_zero_and_still_prints_to_stdout() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_with_strict_exits_non_zero_and_still_prints_to_stdout",
    ) {
        return;
    }

    // `--strict` treats an unreadable path as a failure while the
    // classification still reaches stdout -- the distinction a plain `Err`
    // return cannot express.
    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&link)])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        // The per-file loop stays silent. The only stderr text is the
        // exit-code explanation, and it names the real cause rather than the
        // misleading "check the file path" advice NotFound would produce.
        .stderr(predicate::str::contains("Error processing").not())
        .stderr(predicate::str::contains("unreadable symlink"));
}

#[test]
fn test_regular_file_with_strict_is_unaffected() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "real.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_nonexistent_non_symlink_path_keeps_the_existing_error_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let missing = temp_dir.path().join("no-such-file.bin");

    // Proves the `Err` arm is untouched: a plain missing path still reports
    // to stderr and produces no stdout line.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&missing)])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Error processing"));
}

#[test]
fn test_multi_file_strict_exit_depends_only_on_the_strict_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_multi_file_strict_exit_depends_only_on_the_strict_flag",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link), path_str(&regular)])
        .assert()
        .success();

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            path_str(&link),
            path_str(&regular),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_piped_stdin_still_classifies_with_and_without_strict() {
    // The stdin branch returns before the symlink precheck, making it the
    // easiest outcome conversion to miss.
    for extra in [&[][..], &["--strict"][..]] {
        let mut cmd = rmagic_cmd();
        cmd.arg("--use-builtin");
        for flag in extra {
            cmd.arg(flag);
        }
        cmd.arg("-")
            .write_stdin(ELF_HEADER)
            .assert()
            .success()
            .stdout(predicate::str::contains("ELF"));
    }
}

/// Run `file -b <path>`, or return `None` when `file` is unavailable.
#[cfg(unix)]
fn file_binary_description(path: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("file")
        .arg("-b")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
    )
}

#[cfg(unix)]
#[test]
fn test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("ctrl.link");

    // A raw ESC in the target is what a planted link would use to open an
    // OSC/CSI sequence. Captured output must still match `file` exactly --
    // escaping is gated on stdout being a terminal, and `assert_cmd`
    // captures, so this exercises the pass-through branch.
    let target = "esc\u{1b}[2Jx";
    if !symlink_or_skip(
        std::path::Path::new(target),
        &link,
        "test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured",
    ) {
        return;
    }

    let Some(expected) = file_binary_description(&link) else {
        eprintln!(
            "Skipping test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured: \
             the `file` binary is unavailable"
        );
        return;
    };

    // Guard the oracle itself: if `file` ever starts escaping, the parity
    // claim this test defends has changed and the assertion below is no
    // longer meaningful.
    assert!(
        expected.contains('\u{1b}'),
        "expected GNU `file` to emit the ESC byte unescaped, got {expected:?}"
    );

    let output = rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .output()
        .expect("Failed to run rmagic");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let description = stdout
        .trim_end()
        .rsplit_once(": ")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default();

    assert_eq!(
        description, expected,
        "captured output must match GNU `file` byte-for-byte (ADR-0001)"
    );
}

#[cfg(unix)]
#[test]
fn test_symlink_into_unreadable_directory_reports_broken() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let locked = temp_dir.path().join("locked");
    fs::create_dir_all(&locked).expect("Failed to create dir");
    let target = locked.join("hidden.txt");
    fs::write(&target, ELF_HEADER).expect("Failed to write target");

    let link = temp_dir.path().join("eacces.link");
    if !symlink_or_skip(
        &target,
        &link,
        "test_symlink_into_unreadable_directory_reports_broken",
    ) {
        return;
    }

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
        .expect("Failed to lock directory");

    // Root ignores directory permissions, so the EACCES scenario cannot be
    // constructed there. Probe rather than checking the uid, which would
    // need an unsafe libc call this crate forbids.
    let is_effective = fs::read(&target).is_err();

    if is_effective {
        rmagic_cmd()
            .args(["--use-builtin", path_str(&link)])
            .assert()
            .success()
            // EACCES collapses into the same output as ENOENT and ELOOP.
            .stdout(predicate::str::contains("broken symbolic link to"));
    } else {
        eprintln!(
            "Skipping test_symlink_into_unreadable_directory_reports_broken: \
             directory permissions are not enforced for this user"
        );
    }

    // Restore before the TempDir drop, or cleanup fails.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("Failed to restore directory permissions");
}

// =============================================================================
// Directory Classification Tests (ADR-0001 detection gap, issue #383)
// =============================================================================

#[test]
fn test_directory_is_classified_not_reported_as_an_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    // `file <dir>` prints `<dir>: directory` and exits 0. rmagic previously
    // returned an I/O error with no stdout line.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_directory_with_strict_exits_zero() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    // A directory is a successful detection, not an I/O failure, so
    // `--strict` has nothing to flag.
    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"));
}

#[test]
fn test_symlink_to_directory_classifies_as_directory_by_default() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");
    let link = temp_dir.path().join("dir.link");

    if !symlink_or_skip(
        &dir,
        &link,
        "test_symlink_to_directory_classifies_as_directory_by_default",
    ) {
        return;
    }

    // Default flags follow the link, so the precheck falls through to the
    // directory branch -- matching `file dir.link` -> `directory`.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stdout(predicate::str::contains("symbolic link").not());
}

#[test]
fn test_directory_and_regular_file_in_one_invocation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", path_str(&dir), path_str(&regular)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("realdir:"));
}

#[test]
fn test_directory_json_output_is_coherent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("directory"));
}
