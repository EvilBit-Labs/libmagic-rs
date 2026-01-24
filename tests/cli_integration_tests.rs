//! CLI integration tests for libmagic-rs using canonical libmagic test suite
//!
//! These tests verify the command-line interface functionality by running against
//! the canonical libmagic test suite from third_party/tests/.
//! Each test consists of a .testfile (input) and .result (expected output) pair.

use insta::assert_snapshot;
use libmagic_rs::EvaluationConfig;
use libmagic_rs::parser::load_magic_file;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;

mod common;
use common::{normalize_paths_in_text, normalize_testfile_path};

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
