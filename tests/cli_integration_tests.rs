// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! CLI integration tests for libmagic-rs using canonical libmagic test suite
//!
//! These tests verify the command-line interface functionality by running against
//! the canonical libmagic test suite from third_party/tests/.
//! Each test consists of a .testfile (input) and .result (expected output) pair.
//!
//! # Test Categories
//!
//! ## Canonical Test Suite
//! - Tests that run against the official libmagic test files
//! - Validates compatibility with the C libmagic implementation
//!
//! ## Multiple File Processing
//! - Tests for sequential processing of multiple files
//! - Validates output order matches input argument order
//!
//! ## Strict Mode (`--strict`)
//! - Tests exit code behavior with and without strict mode
//! - Validates error handling continues processing in non-strict mode
//!
//! ## Built-in Rules (`--use-builtin`)
//! - Tests built-in rules for common file type detection
//! - Validates flag precedence over `--magic-file`
//! - Tests detection of ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF
//!
//! ## JSON Lines Output
//! - Tests JSON format output for multiple files
//! - Validates compact JSON Lines format vs pretty-printed single file
//!
//! ## Error Handling
//! - Tests per-file error handling and continuation
//! - Validates error messages include filename context
//!
//! ## Edge Cases
//! - Empty files, large files, directories as input
//! - Permission errors (Unix only)
//! - Mixed stdin and file arguments

use insta::assert_snapshot;
use libmagic_rs::EvaluationConfig;
use libmagic_rs::parser::load_magic_file;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

mod common;
use common::{normalize_paths_in_text, normalize_testfile_path};

// =============================================================================
// Test Helper Functions
// =============================================================================

/// Creates a file in the given directory with specified content.
/// Returns the full path to the created file.
fn create_test_file_with_content(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).expect("Failed to create test file");
    path
}

/// Runs the CLI with given arguments and returns the full output.
/// Uses the already-built test binary for better performance in parallel tests.
fn run_cli_with_args(args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_rmagic"))
        .args(args)
        .output()?;
    Ok(output)
}

/// Parses JSON Lines output into a vector of JSON values.
/// Each line is expected to be valid JSON.
fn parse_json_lines(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("Invalid JSON line"))
        .collect()
}

/// Asserts the exit code matches expected value with a clear error message.
fn assert_exit_code(output: &Output, expected: i32, message: &str) {
    let actual = output.status.code().unwrap_or(-1);
    assert_eq!(
        actual,
        expected,
        "{}: expected exit code {}, got {}.\nstdout: {}\nstderr: {}",
        message,
        expected,
        actual,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Get the root directory for canonical libmagic tests
fn canonical_tests_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("third_party")
        .join("tests")
}

/// Find all test file pairs (.testfile + .result) from the canonical test suite
fn canonical_test_pairs() -> Vec<(PathBuf, PathBuf)> {
    let root = canonical_tests_root();
    let mut pairs = Vec::new();

    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some(OsStr::new("testfile")) {
                let result = path.with_extension("result");
                if result.exists() {
                    pairs.push((path, result));
                }
            }
        }
    }

    pairs.sort();
    pairs
}

/// Parse expected results from a .result file
/// Ignores blank lines and comment lines starting with '#'
fn parse_expected(result_path: &Path) -> Vec<String> {
    let raw = fs::read_to_string(result_path).unwrap_or_default();
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|s| s.to_string())
        .collect()
}

/// Normalize CLI output for comparison
/// - Convert CRLF to LF
/// - Trim whitespace
/// - Strip "filename:" prefix if present
fn normalize_cli_output(out: &str, file_name: &str) -> String {
    let s = out.replace("\r\n", "\n").trim().to_string();

    // Look for the pattern "filename: description" and extract just the description
    // We need to handle paths that might contain colons (like Windows drive letters C:)
    // so we search for the filename followed by a colon and space
    let search_pattern = format!("{}: ", file_name);
    if let Some(pos) = s.find(&search_pattern) {
        return s[pos + search_pattern.len()..].trim().to_string();
    }

    // Fallback: try to find just "filename:" without the space
    let search_pattern_no_space = format!("{}:", file_name);
    if let Some(pos) = s.find(&search_pattern_no_space) {
        return s[pos + search_pattern_no_space.len()..].trim().to_string();
    }

    s
}

/// Run CLI with the given test file and return normalized output
fn run_cli_on_testfile(
    testfile: &Path,
    magic_file: Option<&Path>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut args = vec!["run", "--"];
    if let Some(magic_file) = magic_file {
        args.push("--magic-file");
        args.push(magic_file.to_str().unwrap());
    }
    args.push(testfile.to_str().unwrap());

    let output = Command::new("cargo").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("CLI failed: {}", stderr).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let file_name = testfile.file_name().unwrap().to_str().unwrap();
    Ok(normalize_cli_output(&stdout, file_name))
}

/// Main test function that runs all canonical libmagic tests
#[test]
fn cli_matches_canonical_libmagic_tests() {
    let magic_file = match resolve_magic_file_for_cli() {
        Some(path) => path,
        None => {
            eprintln!("Skipping canonical CLI tests: no compatible text magic file available");
            return;
        }
    };

    let mut failures = Vec::new();
    let test_pairs = canonical_test_pairs();

    println!("Running {} canonical libmagic test pairs", test_pairs.len());

    for (testfile, resultfile) in test_pairs {
        let expected_variants = parse_expected(&resultfile);

        // Skip tests with no expected output
        if expected_variants.is_empty() {
            continue;
        }

        // Run CLI on the test file
        let actual_output = match run_cli_on_testfile(&testfile, Some(&magic_file)) {
            Ok(output) => output,
            Err(e) => {
                failures.push(format!(
                    "{}\n  CLI error: {}",
                    normalize_testfile_path(&testfile.to_string_lossy()),
                    e
                ));
                continue;
            }
        };

        // Check if actual output matches any expected variant
        let matched = expected_variants
            .iter()
            .any(|expected| actual_output.contains(expected) || expected.contains(&actual_output));

        if !matched {
            failures.push(format!(
                "{}\n  got:      '{}'\n  expected: {:?}",
                normalize_testfile_path(&testfile.to_string_lossy()),
                actual_output,
                expected_variants
            ));
        }
    }

    // If there are failures, create a snapshot for debugging
    if !failures.is_empty() {
        let failure_summary = format!(
            "Found {} test failures out of {} canonical tests:\n\n{}",
            failures.len(),
            canonical_test_pairs().len(),
            failures.join("\n\n")
        );
        // Normalize any remaining paths in the summary before snapshotting
        let normalized_summary = normalize_paths_in_text(&failure_summary);
        assert_snapshot!("canonical_cli_test_failures", normalized_summary);
    }
}

/// Resolve a usable text-based magic file for CLI tests.
///
/// Returns `None` if no compatible text magic file can be found and parsed.
fn resolve_magic_file_for_cli() -> Option<PathBuf> {
    let repo_magic = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing.magic");
    let candidates = [
        "/usr/share/misc/magic",
        "/etc/magic",
        "/usr/local/share/misc/magic",
        "/opt/local/share/file/magic",
        "/usr/share/file/magic",
        repo_magic.to_str().unwrap(),
    ];

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if !path.exists() || path.is_dir() {
            continue;
        }

        if load_magic_file(&path).is_ok() {
            return Some(path);
        }
    }

    None
}

fn resolve_magic_file_for_stdin_tests() -> Option<PathBuf> {
    resolve_magic_file_for_cli()
}

fn run_cli_with_stdin(
    args: &[&str],
    input: &[u8],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command.args(["run", "--quiet", "--"]);
    command.args(args);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input)?;
    }

    let output = child.wait_with_output()?;
    Ok(output)
}

/// Test that we can discover canonical test files
#[test]
fn test_canonical_test_discovery() {
    let pairs = canonical_test_pairs();

    // Should find at least some test pairs
    assert!(
        pairs.len() > 10,
        "Expected to find more than 10 test pairs, found: {}",
        pairs.len()
    );

    // Verify each pair has both testfile and result
    for (testfile, resultfile) in &pairs {
        assert!(
            testfile.exists(),
            "Test file should exist: {}",
            testfile.display()
        );
        assert!(
            resultfile.exists(),
            "Result file should exist: {}",
            resultfile.display()
        );
        assert_eq!(
            testfile.extension(),
            Some(OsStr::new("testfile")),
            "Test file should have .testfile extension"
        );
        assert_eq!(
            resultfile.extension(),
            Some(OsStr::new("result")),
            "Result file should have .result extension"
        );
    }
}

#[test]
fn test_basic_stdin_input() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let output =
        run_cli_with_stdin(&["--magic-file", magic_file.to_str().unwrap(), "-"], b"").unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stdin: data"));
}

#[test]
fn test_stdin_dash_argument() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let output = run_cli_with_stdin(
        &["--magic-file", magic_file.to_str().unwrap(), "-"],
        b"test",
    )
    .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stdin:"));
}

#[test]
fn test_stdin_with_multiple_files() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let temp_dir = tempfile::tempdir().unwrap();
    let file1_path = temp_dir.path().join("file1.bin");
    let file2_path = temp_dir.path().join("file2.bin");

    fs::write(&file1_path, b"file-one").unwrap();
    fs::write(&file2_path, b"file-two").unwrap();

    let output = run_cli_with_stdin(
        &[
            "--magic-file",
            magic_file.to_str().unwrap(),
            file1_path.to_str().unwrap(),
            "-",
            file2_path.to_str().unwrap(),
        ],
        b"stdin-input",
    )
    .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 3);
    assert!(stdout.contains(file1_path.to_string_lossy().as_ref()));
    assert!(stdout.contains("stdin:"));
    assert!(stdout.contains(file2_path.to_string_lossy().as_ref()));
}

#[test]
fn test_stdin_truncation_warning() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let max_string_length = EvaluationConfig::default().max_string_length;
    let input = vec![b'a'; max_string_length + 10];

    let output =
        run_cli_with_stdin(&["--magic-file", magic_file.to_str().unwrap(), "-"], &input).unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!(
        "Warning: stdin input truncated to {} bytes",
        max_string_length
    )));
}

#[test]
fn test_stdin_no_false_truncation_warning() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let max_string_length = EvaluationConfig::default().max_string_length;
    // Input is exactly max_string_length bytes - should NOT trigger warning
    let input = vec![b'a'; max_string_length];

    let output =
        run_cli_with_stdin(&["--magic-file", magic_file.to_str().unwrap(), "-"], &input).unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Warning: stdin input truncated"),
        "Should not show truncation warning when input equals max_string_length"
    );
}

#[test]
fn test_stdin_json_output() {
    let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
        eprintln!("Skipping stdin test: no compatible text magic file available");
        return;
    };
    let output = run_cli_with_stdin(
        &["--magic-file", magic_file.to_str().unwrap(), "--json", "-"],
        b"",
    )
    .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.get("matches").is_some());
}

// =============================================================================
// Multiple File Processing Tests
// =============================================================================

/// Test that multiple files are processed sequentially with proper text output format.
/// Each file should produce one line of output in "filename: description" format.
#[test]
fn test_multiple_files_text_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"Hello World");
    let file2 =
        create_test_file_with_content(temp_dir.path(), "file2.bin", &[0x7f, 0x45, 0x4c, 0x46]);
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.dat", b"random data here");

    let output = run_cli_with_args(&[
        "--use-builtin",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(&output, 0, "Multiple files should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    // Should have exactly 3 lines, one per file
    assert_eq!(lines.len(), 3, "Should have one output line per file");

    // Each line should contain the filename
    assert!(
        lines[0].contains("file1.txt"),
        "First line should reference file1.txt"
    );
    assert!(
        lines[1].contains("file2.bin"),
        "Second line should reference file2.bin"
    );
    assert!(
        lines[2].contains("file3.dat"),
        "Third line should reference file3.dat"
    );
}

/// Test that output order matches input argument order.
/// Files should be processed sequentially in the order specified.
#[test]
fn test_multiple_files_sequential_processing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_a = create_test_file_with_content(temp_dir.path(), "aaa.txt", b"first file content");
    let file_b = create_test_file_with_content(temp_dir.path(), "bbb.txt", b"second file content");
    let file_c = create_test_file_with_content(temp_dir.path(), "ccc.txt", b"third file content");

    // Pass files in specific order: b, c, a
    let output = run_cli_with_args(&[
        "--use-builtin",
        file_b.to_str().unwrap(),
        file_c.to_str().unwrap(),
        file_a.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(&output, 0, "Sequential processing should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    assert_eq!(lines.len(), 3, "Should have 3 output lines");

    // Verify order matches argument order (b, c, a)
    assert!(
        lines[0].contains("bbb.txt"),
        "First output should be bbb.txt"
    );
    assert!(
        lines[1].contains("ccc.txt"),
        "Second output should be ccc.txt"
    );
    assert!(
        lines[2].contains("aaa.txt"),
        "Third output should be aaa.txt"
    );
}

// =============================================================================
// Strict Mode (`--strict`) Tests
// =============================================================================

/// Test that `--strict` mode returns non-zero exit code on file not found error.
#[test]
fn test_strict_mode_exit_on_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let valid_file = create_test_file_with_content(temp_dir.path(), "valid.txt", b"valid content");
    let nonexistent = temp_dir.path().join("nonexistent.txt");

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--strict",
        valid_file.to_str().unwrap(),
        nonexistent.to_str().unwrap(),
    ])
    .unwrap();

    // Exit code should be non-zero (3 for I/O error)
    assert!(
        !output.status.success(),
        "Strict mode should return non-zero exit code on failure"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent.txt") || stderr.contains("Error"),
        "Stderr should contain error message for missing file"
    );
}

/// Test that non-strict mode returns success even when some files fail.
#[test]
fn test_non_strict_mode_continues_on_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let valid_file = create_test_file_with_content(temp_dir.path(), "valid.txt", b"valid content");
    let nonexistent = temp_dir.path().join("nonexistent.txt");

    let output = run_cli_with_args(&[
        "--use-builtin",
        valid_file.to_str().unwrap(),
        nonexistent.to_str().unwrap(),
    ])
    .unwrap();

    // Exit code should be 0 (success despite error)
    assert_exit_code(
        &output,
        0,
        "Non-strict mode should return success despite errors",
    );

    // Valid file should still produce output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("valid.txt"),
        "Valid file should still produce output"
    );

    // Error message should be in stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent.txt") || stderr.contains("Error"),
        "Stderr should contain error message for missing file"
    );
}

/// Test that `--strict` mode returns success when all files are valid.
#[test]
fn test_strict_mode_success_all_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"content 1");
    let file2 = create_test_file_with_content(temp_dir.path(), "file2.txt", b"content 2");
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.txt", b"content 3");

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--strict",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(
        &output,
        0,
        "Strict mode should succeed when all files are valid",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "All files should produce output");
}

/// Test that "data" result for unknown files is not considered an error in strict mode.
/// Files with random bytes that don't match any rule should return "data" as success.
#[test]
fn test_strict_mode_unknown_file_not_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Create file with random bytes that won't match any built-in rule
    let random_bytes = b"\xAB\xCD\xEF\x12\x34\x56\x78\x90random binary content here";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.bin", random_bytes);

    let output =
        run_cli_with_args(&["--use-builtin", "--strict", test_file.to_str().unwrap()]).unwrap();

    assert_exit_code(
        &output,
        0,
        "Unknown file (data result) should not be an error in strict mode",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("data"),
        "Unknown file should return 'data', got: {}",
        stdout
    );
}

// =============================================================================
// Built-in Rules (`--use-builtin`) Tests
// =============================================================================

/// Test that `--use-builtin` flag works and detects ELF files.
#[test]
fn test_use_builtin_flag() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Create a test file with ELF magic bytes (64-bit LSB)
    let elf_header = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.elf", elf_header);

    let output = run_cli_with_args(&["--use-builtin", test_file.to_str().unwrap()]).unwrap();

    assert_exit_code(&output, 0, "Built-in rules should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ELF"),
        "Built-in rules should detect ELF file, got: {}",
        stdout
    );
}

/// Test that `--use-builtin` and `--magic-file` conflict.
#[test]
fn test_use_builtin_conflicts_with_magic_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = create_test_file_with_content(temp_dir.path(), "test.txt", b"test content");

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--magic-file",
        "/nonexistent/magic/file",
        test_file.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(&output, 2, "--use-builtin and --magic-file should conflict");
}

/// Test that `--use-builtin` works with multiple files and detects different file types.
#[test]
fn test_use_builtin_with_multiple_files() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create ELF file (magic: 0x7f454c46)
    let elf_header = b"\x7fELF\x00\x00\x00\x00";
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.elf", elf_header);

    // Create ZIP file (magic: 0x504b0304)
    let zip_header = b"PK\x03\x04\x00\x00\x00\x00";
    let file2 = create_test_file_with_content(temp_dir.path(), "file2.zip", zip_header);

    // Create PNG file (magic: 0x89504e47)
    let png_header = b"\x89PNG\x00\x00\x00\x00";
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.png", png_header);

    let output = run_cli_with_args(&[
        "--use-builtin",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(
        &output,
        0,
        "Built-in rules with multiple files should succeed",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    assert_eq!(lines.len(), 3, "Should have one line per file");

    // Verify each file is correctly identified
    assert!(
        stdout.contains("ELF"),
        "Should detect ELF file, got: {}",
        stdout
    );
    assert!(
        stdout.contains("ZIP"),
        "Should detect ZIP file, got: {}",
        stdout
    );
    assert!(
        stdout.contains("PNG"),
        "Should detect PNG file, got: {}",
        stdout
    );
}

/// Test that `--use-builtin --json` produces valid JSON output with JPEG detection.
/// Note: Single file JSON output only has "matches" field, not "filename".
#[test]
fn test_use_builtin_json_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Create JPEG file (magic: 0xffd8)
    let jpeg_header = b"\xff\xd8\x00\x00\x00\x00\x00\x00";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.jpg", jpeg_header);

    let output =
        run_cli_with_args(&["--use-builtin", "--json", test_file.to_str().unwrap()]).unwrap();

    assert_exit_code(&output, 0, "Built-in JSON output should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Verify JSON structure - single file mode only has "matches", not "filename"
    assert!(
        parsed.get("matches").is_some(),
        "JSON should have matches array"
    );

    // Verify JPEG detection in matches
    let matches = parsed.get("matches").unwrap().as_array().unwrap();
    assert!(!matches.is_empty(), "Should have at least one match");

    let first_match = &matches[0];
    let text = first_match
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        text.contains("JPEG") || text.contains("image"),
        "Should detect JPEG file, got: {text}"
    );
}

/// Test that built-in rules correctly detect ELF files.
/// Note: Currently only tests basic ELF detection. Nested rule output
/// (architecture/endianness) is a feature for future enhancement.
#[test]
fn test_builtin_detect_elf_files() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create ELF 32-bit LSB file
    let elf32_lsb = b"\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let file1 = create_test_file_with_content(temp_dir.path(), "elf32lsb.bin", elf32_lsb);

    // Create ELF 64-bit MSB file
    let elf64_msb = b"\x7fELF\x02\x02\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let file2 = create_test_file_with_content(temp_dir.path(), "elf64msb.bin", elf64_msb);

    // Test 32-bit LSB - verify ELF is detected
    let output1 = run_cli_with_args(&["--use-builtin", file1.to_str().unwrap()]).unwrap();
    assert_exit_code(&output1, 0, "ELF 32-bit detection should succeed");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(
        stdout1.contains("ELF"),
        "Should detect ELF in 32-bit file, got: {stdout1}"
    );

    // Test 64-bit MSB - verify ELF is detected
    let output2 = run_cli_with_args(&["--use-builtin", file2.to_str().unwrap()]).unwrap();
    assert_exit_code(&output2, 0, "ELF 64-bit detection should succeed");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("ELF"),
        "Should detect ELF in 64-bit file, got: {stdout2}"
    );
}

/// Test that built-in rules correctly detect PE/DOS executable files.
#[test]
fn test_builtin_detect_pe_dos_files() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create DOS/PE file (magic: "MZ")
    let dos_header = b"MZ\x00\x00\x00\x00";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.exe", dos_header);

    let output = run_cli_with_args(&["--use-builtin", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 0, "PE/DOS detection should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("MS-DOS") || stdout.contains("executable"),
        "Should detect MS-DOS executable, got: {}",
        stdout
    );
}

/// Test that built-in rules correctly detect archive formats.
#[test]
fn test_builtin_detect_archive_formats() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create ZIP file
    let zip_header = b"PK\x03\x04\x14\x00\x00\x00\x08\x00";
    let zip_file = create_test_file_with_content(temp_dir.path(), "test.zip", zip_header);

    // Create TAR file (512 bytes with "ustar" at offset 257)
    let mut tar_data = vec![0u8; 512];
    tar_data[257..262].copy_from_slice(b"ustar");
    let tar_file = create_test_file_with_content(temp_dir.path(), "test.tar", &tar_data);

    // Create GZIP file
    let gzip_header = b"\x1f\x8b\x08\x00\x00\x00\x00\x00";
    let gzip_file = create_test_file_with_content(temp_dir.path(), "test.gz", gzip_header);

    // Test ZIP
    let output1 = run_cli_with_args(&["--use-builtin", zip_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output1, 0, "ZIP detection should succeed");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(
        stdout1.contains("ZIP"),
        "Should detect ZIP archive, got: {}",
        stdout1
    );

    // Test TAR
    let output2 = run_cli_with_args(&["--use-builtin", tar_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output2, 0, "TAR detection should succeed");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("tar"),
        "Should detect TAR archive, got: {}",
        stdout2
    );

    // Test GZIP
    let output3 = run_cli_with_args(&["--use-builtin", gzip_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output3, 0, "GZIP detection should succeed");
    let stdout3 = String::from_utf8_lossy(&output3.stdout);
    assert!(
        stdout3.contains("gzip"),
        "Should detect GZIP archive, got: {}",
        stdout3
    );
}

/// Test that built-in rules correctly detect image formats.
/// Note: Uses simplified headers that match the exact patterns in built-in rules.
#[test]
fn test_builtin_detect_image_formats() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create JPEG file (magic: 0xffd8)
    let jpeg_header = b"\xff\xd8\x00\x00\x00\x00\x00\x00";
    let jpeg_file = create_test_file_with_content(temp_dir.path(), "test.jpg", jpeg_header);

    // Create PNG file (magic: 0x89504e47)
    let png_header = b"\x89PNG\x00\x00\x00\x00";
    let png_file = create_test_file_with_content(temp_dir.path(), "test.png", png_header);

    // Create GIF file (magic: "GIF8")
    let gif_header = b"GIF8\x00\x00\x00\x00";
    let gif_file = create_test_file_with_content(temp_dir.path(), "test.gif", gif_header);

    // Create BMP file (magic: "BM")
    let bmp_header = b"BM\x00\x00\x00\x00\x00\x00";
    let bmp_file = create_test_file_with_content(temp_dir.path(), "test.bmp", bmp_header);

    // Test JPEG
    let output1 = run_cli_with_args(&["--use-builtin", jpeg_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output1, 0, "JPEG detection should succeed");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(
        stdout1.contains("JPEG") || stdout1.contains("JFIF"),
        "Should detect JPEG image, got: {}",
        stdout1
    );

    // Test PNG
    let output2 = run_cli_with_args(&["--use-builtin", png_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output2, 0, "PNG detection should succeed");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("PNG"),
        "Should detect PNG image, got: {}",
        stdout2
    );

    // Test GIF
    let output3 = run_cli_with_args(&["--use-builtin", gif_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output3, 0, "GIF detection should succeed");
    let stdout3 = String::from_utf8_lossy(&output3.stdout);
    assert!(
        stdout3.contains("GIF"),
        "Should detect GIF image, got: {}",
        stdout3
    );

    // Test BMP
    let output4 = run_cli_with_args(&["--use-builtin", bmp_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output4, 0, "BMP detection should succeed");
    let stdout4 = String::from_utf8_lossy(&output4.stdout);
    assert!(
        stdout4.contains("BMP") || stdout4.contains("bitmap"),
        "Should detect BMP image, got: {}",
        stdout4
    );
}

/// Test that built-in rules correctly detect PDF documents.
#[test]
fn test_builtin_detect_pdf_documents() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create PDF file (magic: "%PDF-")
    let pdf_header = b"%PDF-\x00\x00\x00";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.pdf", pdf_header);

    let output = run_cli_with_args(&["--use-builtin", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 0, "PDF detection should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PDF"),
        "Should detect PDF document, got: {}",
        stdout
    );
}

/// Test that built-in rules return "data" for unknown file types.
#[test]
fn test_builtin_unknown_file_returns_data() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create file with random bytes that don't match any pattern
    let random_bytes = b"\xDE\xAD\xBE\xEF\x12\x34\x56\x78\x9A\xBC\xDE\xF0random content";
    let test_file = create_test_file_with_content(temp_dir.path(), "unknown.bin", random_bytes);

    let output = run_cli_with_args(&["--use-builtin", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 0, "Unknown file should not cause error");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("data"),
        "Unknown file should return 'data', got: {}",
        stdout
    );
}

// =============================================================================
// JSON Lines Output Tests
// =============================================================================

/// Test that JSON output with multiple files uses JSON Lines format (one JSON per line).
#[test]
fn test_json_lines_multiple_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"content 1");
    let file2 = create_test_file_with_content(temp_dir.path(), "file2.txt", b"content 2");
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.txt", b"content 3");

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--json",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(&output, 0, "JSON Lines output should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_objects = parse_json_lines(&stdout);

    assert_eq!(
        json_objects.len(),
        3,
        "Should have one JSON object per file"
    );

    // Verify each JSON object has required fields
    for (i, obj) in json_objects.iter().enumerate() {
        assert!(
            obj.get("filename").is_some(),
            "JSON object {} should have filename",
            i
        );
        assert!(
            obj.get("matches").is_some(),
            "JSON object {} should have matches",
            i
        );
    }
}

/// Test that single file JSON output is pretty-printed.
/// Note: Single file JSON output uses JsonOutput struct which only has "matches",
/// not "filename" (which is only in JsonLineOutput for multi-file mode).
#[test]
fn test_json_single_file_pretty_print() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = create_test_file_with_content(temp_dir.path(), "test.txt", b"test content");

    let output =
        run_cli_with_args(&["--use-builtin", "--json", test_file.to_str().unwrap()]).unwrap();

    assert_exit_code(&output, 0, "Single file JSON should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Pretty-printed JSON should contain newlines and indentation
    assert!(
        stdout.contains('\n'),
        "Single file JSON should be pretty-printed with newlines"
    );

    // Verify it's still valid JSON with matches array
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");
    // Single file JSON has "matches" but not "filename" (that's only in multi-file mode)
    assert!(
        parsed.get("matches").is_some(),
        "Single file JSON should have 'matches' field"
    );
}

/// Test JSON Lines output with stdin included.
#[test]
fn test_json_lines_with_stdin() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"file content");
    let file2 = create_test_file_with_content(temp_dir.path(), "file2.txt", b"file content");

    let output = run_cli_with_stdin(
        &[
            "--use-builtin",
            "--json",
            file1.to_str().unwrap(),
            "-",
            file2.to_str().unwrap(),
        ],
        b"stdin content",
    )
    .unwrap();

    assert_exit_code(&output, 0, "JSON Lines with stdin should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Filter out empty lines and parse remaining JSON
    let non_empty_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    assert_eq!(
        non_empty_lines.len(),
        3,
        "Should have 3 JSON lines, got: {:?}",
        non_empty_lines
    );

    let json_objects = parse_json_lines(&stdout);
    assert_eq!(json_objects.len(), 3, "Should have 3 JSON objects");

    // Find the stdin entry
    let stdin_entry = json_objects
        .iter()
        .find(|obj| {
            obj.get("filename")
                .and_then(|f| f.as_str())
                .map(|s| s == "stdin")
                .unwrap_or(false)
        })
        .expect("Should have stdin entry");

    assert_eq!(
        stdin_entry.get("filename").and_then(|f| f.as_str()),
        Some("stdin"),
        "Stdin entry should have filename 'stdin'"
    );
}

// =============================================================================
// Per-File Error Handling Tests
// =============================================================================

/// Test that processing continues even when one file fails (non-strict mode).
#[test]
fn test_per_file_error_handling_continues() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"content 1");
    let invalid_dir = temp_dir.path().join("directory");
    fs::create_dir(&invalid_dir).unwrap();
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.txt", b"content 3");

    let output = run_cli_with_args(&[
        "--use-builtin",
        file1.to_str().unwrap(),
        invalid_dir.to_str().unwrap(), // Directory, should fail
        file3.to_str().unwrap(),
    ])
    .unwrap();

    // Non-strict mode should still succeed
    assert_exit_code(
        &output,
        0,
        "Non-strict should succeed despite directory error",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // file1 and file3 should produce output
    assert!(stdout.contains("file1.txt"), "file1 should produce output");
    assert!(stdout.contains("file3.txt"), "file3 should produce output");

    // Directory error should be in stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("directory") || stderr.contains("Error"),
        "Stderr should contain error for directory"
    );
}

/// Test that strict mode sets non-zero exit code but still processes all files.
#[test]
fn test_per_file_error_with_strict_stops_exit_code() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "file1.txt", b"content 1");
    let nonexistent = temp_dir.path().join("nonexistent.txt");
    let file3 = create_test_file_with_content(temp_dir.path(), "file3.txt", b"content 3");

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--strict",
        file1.to_str().unwrap(),
        nonexistent.to_str().unwrap(),
        file3.to_str().unwrap(),
    ])
    .unwrap();

    // Strict mode should return non-zero
    assert!(
        !output.status.success(),
        "Strict mode should return non-zero exit code"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // All valid files should still be processed
    assert!(
        stdout.contains("file1.txt"),
        "file1 should produce output in strict mode"
    );
    assert!(
        stdout.contains("file3.txt"),
        "file3 should produce output in strict mode"
    );

    // Error should be in stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent") || stderr.contains("Error"),
        "Stderr should contain error for nonexistent file"
    );
}

/// Test that error messages include filename context.
#[test]
fn test_mixed_success_failure_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let valid_file = create_test_file_with_content(temp_dir.path(), "valid.txt", b"valid content");
    let nonexistent = temp_dir.path().join("missing_file.txt");

    let output = run_cli_with_args(&[
        "--use-builtin",
        valid_file.to_str().unwrap(),
        nonexistent.to_str().unwrap(),
    ])
    .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Valid file produces output
    assert!(
        stdout.contains("valid.txt"),
        "Valid file should have output"
    );

    // Error message should contain filename context
    assert!(
        stderr.contains("missing_file.txt") || stderr.contains("Error"),
        "Error message should include filename: {}",
        stderr
    );
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test handling of empty files (0 bytes).
/// Empty files are accepted and evaluated like any other file.
/// They produce output with the filename and description (typically "data").
#[test]
fn test_empty_file_handling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let empty_file = create_test_file_with_content(temp_dir.path(), "empty.txt", b"");

    // Non-strict mode: should succeed and produce output
    let output = run_cli_with_args(&["--use-builtin", empty_file.to_str().unwrap()]).unwrap();

    assert_exit_code(&output, 0, "Non-strict mode should succeed with empty file");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Empty file should produce output with filename
    assert!(
        stdout.contains("empty.txt"),
        "Output should contain filename: {}",
        stdout
    );
    // When using --use-builtin, expect "data" as the description
    assert!(
        stdout.contains("data"),
        "Output should contain 'data' description: {}",
        stdout
    );

    // Strict mode should also succeed for empty files
    let strict_output =
        run_cli_with_args(&["--use-builtin", "--strict", empty_file.to_str().unwrap()]).unwrap();

    assert_exit_code(
        &strict_output,
        0,
        "Strict mode should succeed with empty file",
    );

    let strict_stdout = String::from_utf8_lossy(&strict_output.stdout);
    assert!(
        strict_stdout.contains("empty.txt"),
        "Strict mode output should contain filename: {}",
        strict_stdout
    );
    assert!(
        strict_stdout.contains("data"),
        "Strict mode output should contain 'data' description: {}",
        strict_stdout
    );
}

/// Test handling of large files.
#[test]
fn test_large_file_handling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let max_len = EvaluationConfig::default().max_string_length;
    let large_content = vec![b'X'; max_len + 1024];
    let large_file = create_test_file_with_content(temp_dir.path(), "large.bin", &large_content);

    let output = run_cli_with_args(&["--use-builtin", large_file.to_str().unwrap()]).unwrap();

    assert_exit_code(&output, 0, "Large file should be handled without error");

    // For files (not stdin), there should be no truncation warning
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("truncated"),
        "File input should not show truncation warning (only stdin)"
    );
}

/// Test that directories as input produce an error.
#[test]
fn test_directory_as_input_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_dir = temp_dir.path().join("test_directory");
    fs::create_dir(&test_dir).unwrap();

    let output = run_cli_with_args(&["--use-builtin", test_dir.to_str().unwrap()]).unwrap();

    // Directory input should produce an error in strict mode
    let output_strict =
        run_cli_with_args(&["--use-builtin", "--strict", test_dir.to_str().unwrap()]).unwrap();

    // In strict mode, should have non-zero exit code
    assert!(
        !output_strict.status.success(),
        "Directory input should fail in strict mode"
    );

    let stderr = String::from_utf8_lossy(&output_strict.stderr);
    assert!(
        stderr.contains("directory")
            || stderr.contains("Error")
            || stderr.contains("Is a directory"),
        "Error message should indicate directory issue: {}",
        stderr
    );

    // In non-strict mode, should still succeed overall
    assert_exit_code(
        &output,
        0,
        "Directory error should not fail in non-strict mode",
    );
}

/// Test error message for non-existent file.
#[test]
fn test_nonexistent_file_error_message() {
    let nonexistent = PathBuf::from("/nonexistent/path/to/file.txt");

    let output =
        run_cli_with_args(&["--use-builtin", "--strict", nonexistent.to_str().unwrap()]).unwrap();

    // Should have non-zero exit code
    assert!(
        !output.status.success(),
        "Nonexistent file should fail in strict mode"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("file.txt") || stderr.contains("Error") || stderr.contains("No such file"),
        "Error message should be clear about missing file: {}",
        stderr
    );
}

/// Test permission denied handling (Unix only).
#[cfg(unix)]
#[test]
fn test_permission_denied_handling() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let restricted_file =
        create_test_file_with_content(temp_dir.path(), "restricted.txt", b"secret content");

    // Remove all permissions
    let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&restricted_file, perms).unwrap();

    let output = run_cli_with_args(&[
        "--use-builtin",
        "--strict",
        restricted_file.to_str().unwrap(),
    ])
    .unwrap();

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&restricted_file, perms).unwrap();

    // Should have non-zero exit code
    assert!(
        !output.status.success(),
        "Permission denied should fail in strict mode"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Permission") || stderr.contains("Error") || stderr.contains("denied"),
        "Error message should indicate permission issue: {}",
        stderr
    );
}

/// Test mixed stdin and file arguments in correct order.
#[test]
fn test_mixed_stdin_and_files_order() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = create_test_file_with_content(temp_dir.path(), "first.txt", b"first content");
    let file2 = create_test_file_with_content(temp_dir.path(), "third.txt", b"third content");

    // Order: file1, stdin, file2
    let output = run_cli_with_stdin(
        &[
            "--use-builtin",
            file1.to_str().unwrap(),
            "-",
            file2.to_str().unwrap(),
        ],
        b"stdin content",
    )
    .unwrap();

    assert_exit_code(&output, 0, "Mixed stdin and files should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();

    assert_eq!(
        lines.len(),
        3,
        "Should have 3 output lines, got: {:?}",
        lines
    );

    // Verify order: first.txt, stdin, third.txt
    assert!(
        lines[0].contains("first.txt"),
        "First output should be first.txt, got: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("stdin"),
        "Second output should be stdin, got: {}",
        lines[1]
    );
    assert!(
        lines[2].contains("third.txt"),
        "Third output should be third.txt, got: {}",
        lines[2]
    );
}

// =============================================================================
// Timeout Behavior Tests
// =============================================================================

/// Test timeout behavior and per-file independence with a slow magic file.
///
/// This creates a magic file with repeated string rules that force full-buffer
/// reads. A large input triggers the timeout while small inputs complete.
#[test]
fn test_timeout_per_file_independent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let slow_magic_path = temp_dir.path().join("slow.magic");

    let mut slow_rules = String::new();
    for _ in 0..25 {
        slow_rules.push_str("0 string \"b\" data slow\n");
    }
    fs::write(&slow_magic_path, slow_rules).unwrap();

    let fast1 = create_test_file_with_content(temp_dir.path(), "fast1.txt", b"fast content");
    let slow_trigger =
        create_test_file_with_content(temp_dir.path(), "slow_trigger.txt", &vec![b'a'; 5_000_000]);
    let fast2 = create_test_file_with_content(temp_dir.path(), "fast2.txt", b"fast content");

    let output = run_cli_with_args(&[
        "--timeout-ms",
        "50",
        "--magic-file",
        slow_magic_path.to_str().unwrap(),
        fast1.to_str().unwrap(),
        slow_trigger.to_str().unwrap(),
        fast2.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(
        &output,
        0,
        "Non-strict timeout run should exit successfully",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "Only fast files should produce output");
    assert!(
        lines[0].contains("fast1.txt"),
        "Output should start with fast1"
    );
    assert!(
        lines[1].contains("fast2.txt"),
        "Output should include fast2"
    );
    assert!(
        !stdout.contains("slow_trigger.txt"),
        "Timeout file should not produce stdout output"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("slow_trigger.txt"),
        "Timeout error should include filename"
    );
    assert!(
        stderr.contains("timeout") || stderr.contains("Timeout"),
        "Timeout error should mention timeout"
    );
    assert!(
        stderr.contains("50ms"),
        "Timeout error should include 50ms (non-strict)"
    );
}

/// Test that strict mode returns exit code 5 on timeout while still processing
/// subsequent files.
#[test]
fn test_timeout_per_file_independent_strict() {
    let temp_dir = tempfile::tempdir().unwrap();
    let slow_magic_path = temp_dir.path().join("slow.magic");

    let mut slow_rules = String::new();
    for _ in 0..25 {
        slow_rules.push_str("0 string \"b\" data slow\n");
    }
    fs::write(&slow_magic_path, slow_rules).unwrap();

    let fast1 = create_test_file_with_content(temp_dir.path(), "fast1.txt", b"fast content");
    let slow_trigger =
        create_test_file_with_content(temp_dir.path(), "slow_trigger.txt", &vec![b'a'; 5_000_000]);
    let fast2 = create_test_file_with_content(temp_dir.path(), "fast2.txt", b"fast content");

    let output = run_cli_with_args(&[
        "--timeout-ms",
        "50",
        "--magic-file",
        slow_magic_path.to_str().unwrap(),
        "--strict",
        fast1.to_str().unwrap(),
        slow_trigger.to_str().unwrap(),
        fast2.to_str().unwrap(),
    ])
    .unwrap();

    assert_exit_code(&output, 5, "Strict timeout run should exit with code 5");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "Strict mode should still output fast files");
    assert!(
        lines[0].contains("fast1.txt"),
        "Output should start with fast1"
    );
    assert!(
        lines[1].contains("fast2.txt"),
        "Output should include fast2"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("slow_trigger.txt"),
        "Timeout error should include filename"
    );
    assert!(
        stderr.contains("timeout") || stderr.contains("Timeout"),
        "Timeout error should mention timeout"
    );
    assert!(
        stderr.contains("50ms"),
        "Timeout error should include 50ms (strict)"
    );
}

// =============================================================================
// Help, Version, and Shell Completion Tests
// =============================================================================

/// Test that --help exits 0 and contains expected content.
#[test]
fn test_help_flag() {
    let output = run_cli_with_args(&["--help"]).unwrap();
    assert_exit_code(&output, 0, "--help should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "--help should contain Usage:");
    assert!(stdout.contains("--json"), "--help should mention --json");
    assert!(
        stdout.contains("--magic-file"),
        "--help should mention --magic-file"
    );
    assert!(
        stdout.contains("Examples:"),
        "--help should contain Examples section in after_help"
    );
}

/// Test that -h (short help) exits 0.
#[test]
fn test_short_help_flag() {
    let output = run_cli_with_args(&["-h"]).unwrap();
    assert_exit_code(&output, 0, "-h should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "-h should contain Usage:");
}

/// Test that --version exits 0 and contains a version string.
#[test]
fn test_version_flag() {
    let output = run_cli_with_args(&["--version"]).unwrap();
    assert_exit_code(&output, 0, "--version should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "--version should contain package version, got: {}",
        stdout
    );
}

/// Test that --generate-completion bash produces shell completion output.
#[test]
fn test_generate_completion_bash() {
    let output = run_cli_with_args(&["--generate-completion", "bash"]).unwrap();
    assert_exit_code(&output, 0, "--generate-completion bash should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("rmagic"),
        "Bash completion output should reference rmagic, got: {}",
        stdout
    );
}

/// Test that --json and --text together is rejected.
#[test]
fn test_json_text_conflict_cli() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = create_test_file_with_content(temp_dir.path(), "test.txt", b"test");

    let output = run_cli_with_args(&["--json", "--text", test_file.to_str().unwrap()]).unwrap();

    assert_exit_code(
        &output,
        2,
        "--json and --text should conflict and exit non-zero",
    );
}

/// Test short flags work correctly.
#[test]
fn test_short_flags() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create an ELF file for detection
    let elf_header = b"\x7fELF\x00\x00\x00\x00";
    let test_file = create_test_file_with_content(temp_dir.path(), "test.elf", elf_header);

    // Test -j (json) and -b (use-builtin)
    let output = run_cli_with_args(&["-j", "-b", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 0, "-j -b should work");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // JSON output should be parseable
    assert!(
        stdout.contains('{'),
        "JSON output should contain braces, got: {}",
        stdout
    );
}

/// Test that --timeout-ms validates range.
#[test]
fn test_timeout_ms_range_validation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = create_test_file_with_content(temp_dir.path(), "test.txt", b"test");

    // 0 should be rejected (minimum is 1)
    let output = run_cli_with_args(&["--timeout-ms", "0", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 2, "--timeout-ms 0 should be rejected");

    // 300001 should be rejected (maximum is 300000)
    let output =
        run_cli_with_args(&["--timeout-ms", "300001", test_file.to_str().unwrap()]).unwrap();
    assert_exit_code(&output, 2, "--timeout-ms 300001 should be rejected");
}
