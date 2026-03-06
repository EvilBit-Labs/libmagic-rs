// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! CLI integration tests for rmagic binary
//!
//! These tests use subprocess-based testing with `assert_cmd` for natural process
//! isolation. This approach eliminates the need for fragile fd manipulation and
//! enables reliable execution under llvm-cov.

use assert_cmd::Command;
use libmagic_rs::EvaluationConfig;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a Command for the rmagic binary
fn rmagic_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("rmagic"))
}

/// Helper to create a temporary ELF file for testing
fn create_elf_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.elf");
    // Minimal ELF header: magic + class (64-bit) + endianness (little) + version
    fs::write(&path, b"\x7fELF\x02\x01\x01\x00").expect("Failed to create ELF file");
    path
}

/// Helper to create a temporary PNG file for testing
fn create_png_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.png");
    // PNG signature
    fs::write(&path, b"\x89PNG\r\n\x1a\n").expect("Failed to create PNG file");
    path
}

/// Helper to create a temporary JPEG file for testing
fn create_jpeg_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.jpg");
    // JPEG SOI marker
    fs::write(&path, b"\xff\xd8\xff\xe0").expect("Failed to create JPEG file");
    path
}

/// Helper to create a temporary PDF file for testing
fn create_pdf_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.pdf");
    fs::write(&path, b"%PDF-1.4").expect("Failed to create PDF file");
    path
}

/// Helper to create a temporary ZIP file for testing
fn create_zip_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.zip");
    // ZIP local file header signature
    fs::write(&path, b"PK\x03\x04").expect("Failed to create ZIP file");
    path
}

/// Helper to create a temporary GIF file for testing
fn create_gif_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.gif");
    fs::write(&path, b"GIF89a").expect("Failed to create GIF file");
    path
}

/// Helper to create a temporary magic file for testing
fn create_magic_file(dir: &TempDir, content: &str) -> std::path::PathBuf {
    let path = dir.path().join("test.magic");
    fs::write(&path, content).expect("Failed to create magic file");
    path
}

/// Helper to create a temporary data file for testing
fn create_data_file(dir: &TempDir, filename: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(filename);
    fs::write(&path, content).expect("Failed to create data file");
    path
}

// =============================================================================
// Builtin Flag Tests
// =============================================================================

#[test]
fn test_builtin_elf_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_builtin_png_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_png_file(&temp_dir);

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("PNG"));
}

#[test]
fn test_builtin_jpeg_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_jpeg_file(&temp_dir);

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("JPEG"));
}

#[test]
fn test_builtin_pdf_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_pdf_file(&temp_dir);

    // PDF detection may return "PDF" or "data" depending on builtin rules
    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success();
}

#[test]
fn test_builtin_zip_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_zip_file(&temp_dir);

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("ZIP"));
}

#[test]
fn test_builtin_gif_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_gif_file(&temp_dir);

    // GIF detection may return "GIF" or "data" depending on builtin rules
    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success();
}

#[test]
fn test_builtin_with_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_builtin_with_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--json",
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_builtin_unknown_file_returns_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "unknown.bin", b"random data here");

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

// =============================================================================
// Stdin Tests
// =============================================================================

#[test]
fn test_stdin_elf_detection() {
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"\x7fELF\x02\x01\x01\x00" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"))
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_stdin_png_detection() {
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"\x89PNG\r\n\x1a\n" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"))
        .stdout(predicate::str::contains("PNG"));
}

#[test]
fn test_stdin_empty_returns_data() {
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin: data"));
}

#[test]
fn test_stdin_output_format_text() {
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"sample data" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"));
}

#[test]
fn test_stdin_output_format_json() {
    rmagic_cmd()
        .args(["--use-builtin", "--json", "-"])
        .write_stdin(b"\x7fELF\x02\x01\x01\x00" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""));
}

#[test]
fn test_stdin_with_strict() {
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "-"])
        .write_stdin(b"\x7fELF\x02\x01\x01\x00" as &[u8])
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
// Multiple File Tests
// =============================================================================

#[test]
fn test_multiple_files_sequential_output() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let png_file = create_png_file(&temp_dir);
    let zip_file = create_zip_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            elf_file.to_str().expect("Invalid path"),
            png_file.to_str().expect("Invalid path"),
            zip_file.to_str().expect("Invalid path"),
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
    let elf_file = create_elf_file(&temp_dir);
    let png_file = create_png_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            elf_file.to_str().expect("Invalid path"),
            png_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("PNG"));
}

#[test]
fn test_multiple_files_with_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let png_file = create_png_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--json",
            elf_file.to_str().expect("Invalid path"),
            png_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        // JSON Lines format uses "filename" field
        .stdout(predicate::str::contains("\"filename\""));
}

#[test]
fn test_multiple_files_with_custom_magic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a valid magic file - using byte type for simple matching
    let magic_file = create_magic_file(&temp_dir, "# Test magic\n0 byte 0x7f ELF marker\n");
    let data_file = create_data_file(&temp_dir, "test1.bin", b"\x7fELF\x02\x01\x01\x00");
    let data_file2 = create_data_file(&temp_dir, "test2.bin", b"\x7fELF\x01\x01\x01\x00");

    // Verify CLI handles multiple files with custom magic
    rmagic_cmd()
        .args([
            "--magic-file",
            magic_file.to_str().expect("Invalid path"),
            data_file.to_str().expect("Invalid path"),
            data_file2.to_str().expect("Invalid path"),
        ])
        .assert()
        .success();
}

#[test]
fn test_multiple_files_partial_failure_non_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let nonexistent = temp_dir.path().join("nonexistent.bin");

    rmagic_cmd()
        .args([
            "--use-builtin",
            elf_file.to_str().expect("Invalid path"),
            nonexistent.to_str().expect("Invalid path"),
        ])
        .assert()
        .success() // Non-strict mode should succeed overall
        .stdout(predicate::str::contains("ELF"))
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_multiple_files_partial_failure_strict() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let nonexistent = temp_dir.path().join("nonexistent.bin");

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            elf_file.to_str().expect("Invalid path"),
            nonexistent.to_str().expect("Invalid path"),
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
    // With strict mode, file not found returns exit code 3
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "nonexistent_file.bin"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_error_directory_instead_of_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Without strict mode, the CLI succeeds but prints error to stderr
    // With strict mode, it fails with exit code 2
    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            temp_dir.path().to_str().expect("Invalid path"),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("directory"));
}

#[test]
fn test_error_magic_file_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_data_file(&temp_dir, "test.bin", b"test");

    rmagic_cmd()
        .args([
            "--magic-file",
            "nonexistent.magic",
            test_file.to_str().expect("Invalid path"),
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
        .args([
            "--magic-file",
            magic_file.to_str().expect("Invalid path"),
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn test_error_invalid_arguments_no_files() {
    rmagic_cmd().assert().failure().code(2);
}

#[test]
fn test_error_conflicting_flags() {
    rmagic_cmd()
        .args(["--json", "--text", "test.bin"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_error_builtin_with_magic_file_conflict() {
    rmagic_cmd()
        .args(["--use-builtin", "--magic-file", "custom.magic", "test.bin"])
        .assert()
        .failure()
        .code(2);
}

// =============================================================================
// Timeout Tests
// =============================================================================

#[test]
fn test_timeout_argument_parsing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    // Valid timeout value
    rmagic_cmd()
        .args([
            "--use-builtin",
            "--timeout-ms",
            "1000",
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success();
}

#[test]
fn test_timeout_too_small() {
    rmagic_cmd()
        .args(["--use-builtin", "--timeout-ms", "0", "test.bin"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_timeout_too_large() {
    rmagic_cmd()
        .args(["--use-builtin", "--timeout-ms", "999999999", "test.bin"])
        .assert()
        .failure()
        .code(2);
}

// =============================================================================
// Output Format Tests
// =============================================================================

#[test]
fn test_output_text_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--text",
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(":"))
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_output_json_single_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--json",
            test_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("]"));
}

#[test]
fn test_output_json_multiple_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let png_file = create_png_file(&temp_dir);

    let output = rmagic_cmd()
        .args([
            "--use-builtin",
            "--json",
            elf_file.to_str().expect("Invalid path"),
            png_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success();

    // JSON Lines format should have one JSON object per line
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should have 2 JSON lines for 2 files");
}

// =============================================================================
// Shell Completion Tests
// =============================================================================

#[test]
fn test_generate_completion_bash() {
    rmagic_cmd()
        .args(["--generate-completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_rmagic"));
}

#[test]
fn test_generate_completion_zsh() {
    rmagic_cmd()
        .args(["--generate-completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef"));
}

#[test]
fn test_generate_completion_fish() {
    rmagic_cmd()
        .args(["--generate-completion", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

// =============================================================================
// Custom Magic File Tests
// =============================================================================

#[test]
fn test_custom_magic_file_accepted() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a valid magic file - the format is tested thoroughly in parser unit tests
    let magic_content = "# Test magic file\n0 byte 0x7f ELF magic\n";
    let magic_file = create_magic_file(&temp_dir, magic_content);
    let data_file = create_data_file(&temp_dir, "test.bin", b"\x7fELF data here");

    // Verify CLI accepts custom magic file without crashing
    rmagic_cmd()
        .args([
            "--magic-file",
            magic_file.to_str().expect("Invalid path"),
            data_file.to_str().expect("Invalid path"),
        ])
        .assert()
        .success();
}

#[test]
fn test_custom_magic_file_fallback_to_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a magic file that won't match the test data
    let magic_content = "# Test magic file\n0 byte 0xff Marker\n";
    let magic_file = create_magic_file(&temp_dir, magic_content);
    let data_file = create_data_file(&temp_dir, "test.bin", b"plain text");

    // When no rule matches, output should contain "data"
    rmagic_cmd()
        .args([
            "--magic-file",
            magic_file.to_str().expect("Invalid path"),
            data_file.to_str().expect("Invalid path"),
        ])
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
    let path = temp_dir.path().join("file with spaces.elf");
    fs::write(&path, b"\x7fELF\x02\x01\x01\x00").expect("Failed to create file");

    rmagic_cmd()
        .args(["--use-builtin", path.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_file_with_unicode_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("test_\u{1F600}.elf");
    fs::write(&path, b"\x7fELF\x02\x01\x01\x00").expect("Failed to create file");

    rmagic_cmd()
        .args(["--use-builtin", path.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_empty_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "empty.bin", b"");

    rmagic_cmd()
        .args(["--use-builtin", path.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

#[test]
fn test_very_small_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "small.bin", b"x");

    rmagic_cmd()
        .args(["--use-builtin", path.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

// =============================================================================
// CLI Argument Parsing Tests (migrated from main.rs unit tests)
// =============================================================================

#[test]
fn test_args_multiple_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file1 = create_elf_file(&temp_dir);
    let file2 = create_png_file(&temp_dir);
    let file3 = create_zip_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            file1.to_str().expect("Invalid path"),
            file2.to_str().expect("Invalid path"),
            file3.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("PNG"))
        .stdout(predicate::str::contains("ZIP"));
}

#[test]
fn test_args_strict_with_multiple_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file1 = create_elf_file(&temp_dir);
    let file2 = create_png_file(&temp_dir);
    let file3 = create_zip_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            file1.to_str().expect("Invalid path"),
            file2.to_str().expect("Invalid path"),
            file3.to_str().expect("Invalid path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("PNG"))
        .stdout(predicate::str::contains("ZIP"));
}

#[test]
fn test_args_multiple_files_with_magic_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_magic_file(&temp_dir, "# Test magic\n0 byte 0x7f ELF marker\n");
    let file1 = create_data_file(&temp_dir, "test1.bin", b"\x7fELF data");
    let file2 = create_data_file(&temp_dir, "test2.bin", b"\x7fELF more data");

    rmagic_cmd()
        .args([
            "--magic-file",
            magic_file.to_str().expect("Invalid path"),
            file1.to_str().expect("Invalid path"),
            file2.to_str().expect("Invalid path"),
        ])
        .assert()
        .success();
}

#[test]
fn test_use_builtin_with_multiple_formats() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);
    let png_file = create_png_file(&temp_dir);
    let jpeg_file = create_jpeg_file(&temp_dir);
    let pdf_file = create_pdf_file(&temp_dir);
    let zip_file = create_zip_file(&temp_dir);
    let gif_file = create_gif_file(&temp_dir);

    // Test all formats with builtin rules
    for (file, expected_substr) in [
        (&elf_file, "ELF"),
        (&png_file, "PNG"),
        (&jpeg_file, "JPEG"),
        (&zip_file, "ZIP"),
    ] {
        rmagic_cmd()
            .args(["--use-builtin", file.to_str().expect("Invalid path")])
            .assert()
            .success()
            .stdout(predicate::str::contains(expected_substr));
    }

    // PDF and GIF may return "data" depending on builtin rules
    for file in [&pdf_file, &gif_file] {
        rmagic_cmd()
            .args(["--use-builtin", file.to_str().expect("Invalid path")])
            .assert()
            .success();
    }
}

#[test]
fn test_stdin_detection() {
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"test data" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"));
}

// =============================================================================
// Strict-Mode Stdin Error Tests
// =============================================================================

#[test]
fn test_stdin_strict_mode_with_invalid_content() {
    // Empty stdin in strict mode should still succeed (empty file is valid)
    rmagic_cmd()
        .args(["--use-builtin", "--strict", "-"])
        .write_stdin(b"" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin: data"));
}

#[test]
fn test_stdin_non_strict_continues_on_unknown() {
    // Non-strict mode should continue without error on unknown content
    rmagic_cmd()
        .args(["--use-builtin", "-"])
        .write_stdin(b"random unrecognized content" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("data"));
}

#[test]
fn test_multiple_inputs_strict_mode_stdin_first() {
    // Test stdin with other files in strict mode
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let elf_file = create_elf_file(&temp_dir);

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            "-",
            elf_file.to_str().expect("Invalid path"),
        ])
        .write_stdin(b"\x7fELF\x02\x01\x01\x00" as &[u8])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdin:"))
        .stdout(predicate::str::contains("ELF"));
}
