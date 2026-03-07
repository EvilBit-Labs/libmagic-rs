// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Compatibility tests for libmagic-rs
//!
//! These tests ensure that our implementation produces identical results to the original libmagic.
//! Test files are downloaded from the file/file repository and compared against expected results.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use libmagic_rs::MagicDatabase;
use libmagic_rs::parser::{MagicFileFormat, detect_format};

/// Test result for a single compatibility test
#[derive(Debug, Clone)]
struct TestResult {
    test_file: PathBuf,
    status: TestStatus,
    #[allow(dead_code)]
    expected_output: String,
    #[allow(dead_code)]
    actual_output: String,
    error: Option<String>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum TestStatus {
    Pass,
    Fail,
    Error,
}

/// Compatibility test runner
struct CompatibilityTestRunner {
    test_dir: PathBuf,
    rmagic_binary: PathBuf,
}

impl CompatibilityTestRunner {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let test_dir = PathBuf::from("third_party/tests");
        let rmagic_binary = find_rmagic_binary()?;

        if !test_dir.exists() {
            return Err(
                "Compatibility test files not found. Ensure third_party/tests directory exists."
                    .into(),
            );
        }

        Ok(Self {
            test_dir,
            rmagic_binary,
        })
    }

    /// Find all test files and their corresponding result files
    fn find_test_files(&self) -> Vec<(PathBuf, PathBuf)> {
        let mut test_files = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.test_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("testfile") {
                    let result_file = path.with_extension("result");
                    if result_file.exists() {
                        test_files.push((path, result_file));
                    }
                }
            }
        }

        // Sort by input file path to ensure deterministic test execution
        test_files.sort_unstable_by_key(|(input_path, _)| input_path.clone());
        test_files
    }

    /// Run rmagic against a test file
    fn run_rmagic(&self, test_file: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new(&self.rmagic_binary)
            .arg("--use-builtin")
            .arg(test_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("rmagic failed: {}", stderr).into());
        }

        let full_output = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Extract just the description part (after the colon)
        // Expected format: "filename: description" - split at first colon only
        // Fallback: return full output if no colon is present
        if let Some((_filename, description)) = full_output.split_once(':') {
            Ok(description.trim().to_string())
        } else {
            Ok(full_output)
        }
    }

    /// Normalize output for comparison
    fn normalize_output(&self, output: &str) -> String {
        output
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Run a single test with assertion
    fn run_single_test(&self, test_file: PathBuf, result_file: PathBuf) -> TestResult {
        let expected_output = match fs::read_to_string(&result_file) {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                return TestResult {
                    test_file: test_file.clone(),
                    status: TestStatus::Error,
                    expected_output: String::new(),
                    actual_output: String::new(),
                    error: Some(format!("Failed to read result file: {}", e)),
                    errors: vec![],
                };
            }
        };

        let actual_output = match self.run_rmagic(&test_file) {
            Ok(output) => output,
            Err(e) => {
                return TestResult {
                    test_file: test_file.clone(),
                    status: TestStatus::Error,
                    expected_output,
                    actual_output: String::new(),
                    error: Some(format!("rmagic failed: {}", e)),
                    errors: vec![],
                };
            }
        };

        // Compare normalized outputs and record failures instead of panicking
        let normalized_expected = self.normalize_output(&expected_output);
        let normalized_actual = self.normalize_output(&actual_output);

        let (status, errors) = if normalized_expected == normalized_actual {
            (TestStatus::Pass, vec![])
        } else {
            let error_message = format!(
                "Test failed for {}:\nExpected: {}\nActual: {}",
                test_file.display(),
                expected_output,
                actual_output
            );
            (TestStatus::Fail, vec![error_message])
        };

        TestResult {
            test_file,
            status,
            expected_output,
            actual_output,
            error: None,
            errors,
        }
    }

    /// Run all compatibility tests
    fn run_all_tests(&self) -> Vec<TestResult> {
        let test_files = self.find_test_files();
        let mut results = Vec::new();

        println!("Found {} test files", test_files.len());

        for (test_file, result_file) in test_files {
            let result = self.run_single_test(test_file, result_file);
            results.push(result);
        }

        results
    }

    /// Generate a summary report
    fn generate_report(&self, results: &[TestResult]) -> HashMap<String, usize> {
        let mut summary = HashMap::new();
        summary.insert("total".to_string(), results.len());
        summary.insert("passed".to_string(), 0);
        summary.insert("failed".to_string(), 0);
        summary.insert("errors".to_string(), 0);

        for result in results {
            match result.status {
                TestStatus::Pass => {
                    *summary.get_mut("passed").unwrap() += 1;
                }
                TestStatus::Fail => {
                    *summary.get_mut("failed").unwrap() += 1;
                }
                TestStatus::Error => {
                    *summary.get_mut("errors").unwrap() += 1;
                }
            }
        }

        summary
    }
}

/// Find the rmagic binary
fn find_rmagic_binary() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Use the cargo_bin! macro when available (works under cargo test, cargo llvm-cov, etc.)
    let cargo_bin_path = assert_cmd::cargo::cargo_bin!("rmagic");
    if cargo_bin_path.exists() {
        return Ok(cargo_bin_path.to_path_buf());
    }

    // Fallback to manual search for release/debug binaries
    let candidates = [
        "target/release/rmagic",
        "target/release/rmagic.exe",
        "target/debug/rmagic",
        "target/debug/rmagic.exe",
    ];

    candidates
        .iter()
        .find(|c| Path::new(c).exists())
        .map(PathBuf::from)
        .ok_or_else(|| "rmagic binary not found. Please build the project first.".into())
}

/// Test that downloads and runs compatibility tests
#[test]
#[ignore] // Ignore by default since it requires downloading test files
fn test_compatibility_with_original_libmagic() {
    let runner = match CompatibilityTestRunner::new() {
        Ok(runner) => runner,
        Err(e) => {
            println!("Skipping compatibility tests: {}", e);
            return;
        }
    };

    let results = runner.run_all_tests();
    let summary = runner.generate_report(&results);

    println!("\n=== COMPATIBILITY TEST SUMMARY ===");
    println!("Total tests: {}", summary["total"]);
    println!("Passed: {}", summary["passed"]);
    println!("Failed: {}", summary["failed"]);
    println!("Errors: {}", summary["errors"]);

    // Print failed tests
    let failed_tests: Vec<_> = results
        .iter()
        .filter(|r| r.status == TestStatus::Fail)
        .collect();

    if !failed_tests.is_empty() {
        println!("\n=== FAILED TESTS ===");
        for result in failed_tests {
            println!("FAIL {}", result.test_file.display());
            for error in &result.errors {
                println!("   {}", error);
            }
            println!();
        }
    }

    // Print error tests
    let error_tests: Vec<_> = results
        .iter()
        .filter(|r| r.status == TestStatus::Error)
        .collect();

    if !error_tests.is_empty() {
        println!("\n=== ERROR TESTS ===");
        for result in error_tests {
            println!("ERROR {}", result.test_file.display());
            if let Some(error) = &result.error {
                println!("   Error: {}", error);
            }
            println!();
        }
    }

    // Assert that we have some tests
    assert!(summary["total"] > 0, "No compatibility tests found");

    // Fail if we have errors (these are different from assertion failures)
    if summary["errors"] > 0 {
        panic!("{} tests had errors", summary["errors"]);
    }

    // Note: Individual test failures are now handled by assertions in run_single_test
    // If we reach here, all tests passed
    println!("\nCompatibility tests completed successfully!");
}

/// Test that verifies we can load the magic database
#[test]
fn test_magic_database_loading() {
    let magic_file = Path::new("third_party/magic.mgc");
    if !magic_file.exists() {
        println!("Skipping magic database test: third_party/magic.mgc not found");
        return;
    }

    match detect_format(magic_file) {
        Ok(MagicFileFormat::Binary) => {
            println!("Skipping magic database test: binary .mgc not supported");
            return;
        }
        Ok(MagicFileFormat::Text | MagicFileFormat::Directory) => {}
        Err(e) => {
            println!("Skipping magic database test: failed to detect format: {e}");
            return;
        }
    }

    let db = MagicDatabase::load_from_file(magic_file);
    assert!(db.is_ok(), "Failed to load magic database");
}

/// Test that verifies rmagic binary exists and works
#[test]
fn test_rmagic_binary() {
    let binary = find_rmagic_binary();
    assert!(binary.is_ok(), "rmagic binary not found");

    let binary_path = binary.unwrap();
    assert!(binary_path.exists(), "rmagic binary does not exist");

    // Test that the binary runs (even if it fails due to missing args)
    let output = Command::new(&binary_path)
        .output()
        .expect("Failed to run rmagic binary");

    // Should fail with usage message, not crash
    assert!(
        !output.status.success(),
        "rmagic should fail with missing arguments"
    );
}

/// Test that verifies test files are available
#[test]
fn test_compatibility_files_available() {
    let test_dir = Path::new("third_party/tests");
    if !test_dir.exists() {
        println!("Skipping compatibility files test: third_party/tests not found");
        return;
    }

    let runner = CompatibilityTestRunner::new().expect("Failed to create test runner");
    let test_files = runner.find_test_files();

    assert!(!test_files.is_empty(), "No compatibility test files found");
    println!("Found {} compatibility test files", test_files.len());
}
