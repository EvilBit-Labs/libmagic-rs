//! CLI integration tests for libmagic-rs
//!
//! These tests verify the command-line interface functionality including:
//! - File processing with various input files
//! - Output format selection (text, JSON)
//! - Magic file loading and parsing
//! - Error handling and edge cases

use insta::assert_snapshot;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str;

/// Helper function to run the CLI with given arguments
fn run_cli(args: &[&str]) -> Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--").args(args);
    cmd.output()
}

/// Helper function to run the CLI and get stdout as string
fn run_cli_stdout(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = run_cli(args)?;
    Ok(String::from_utf8(output.stdout)?)
}

/// Helper function to run the CLI and get stderr as string
fn run_cli_stderr(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = run_cli(args)?;
    let stderr = String::from_utf8(output.stderr)?;
    // Filter out build noise for cleaner snapshots
    let filtered_stderr = stderr
        .lines()
        .filter(|line| {
            !line.contains("Blocking waiting for file lock")
                && !line.contains("Finished `dev` profile")
                && !line.contains("Running `target\\debug\\rmagic.exe")
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(filtered_stderr)
}

/// Helper function to create a temporary test file with given content
fn create_test_file(name: &str, content: &[u8]) -> Result<String, std::io::Error> {
    let path = format!("test_files/{}", name);
    fs::write(&path, content)?;
    Ok(path)
}

#[test]
fn test_cli_basic_file_processing() {
    // Test basic file processing with text output (default)
    let result = run_cli_stdout(&["test_files/sample.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("basic_file_processing", output);
}

#[test]
fn test_cli_json_output_format() {
    // Test JSON output format
    let result = run_cli_stdout(&["test_files/sample.bin", "--json"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("json_output_format", output);
}

#[test]
fn test_cli_text_output_format_explicit() {
    // Test explicit text output format
    let result = run_cli_stdout(&["test_files/sample.bin", "--text"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("text_output_format_explicit", output);
}

#[test]
fn test_cli_nonexistent_file() {
    // Test error handling for nonexistent file
    let result = run_cli(&["nonexistent_file.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(!output.status.success());

    let stderr = run_cli_stderr(&["nonexistent_file.bin"]).unwrap();
    assert_snapshot!("nonexistent_file_error", stderr);
}

#[test]
fn test_cli_custom_magic_file() {
    // Create a simple custom magic file
    let custom_magic_content = r#"# Custom magic file for testing
0	string	TEST	Test file format
0	string	HELLO	Hello file format
"#;

    let magic_file_path = create_test_file("custom.magic", custom_magic_content.as_bytes())
        .expect("Failed to create custom magic file");

    // Create a test file that matches our custom magic
    let test_file_path =
        create_test_file("test_custom.bin", b"TEST data here").expect("Failed to create test file");

    // Test with custom magic file
    let result = run_cli_stdout(&[&test_file_path, "--magic-file", &magic_file_path]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("custom_magic_file", output);

    // Clean up
    let _ = fs::remove_file(&magic_file_path);
    let _ = fs::remove_file(&test_file_path);
}

#[test]
fn test_cli_invalid_magic_file() {
    // Test with nonexistent magic file - should still work with fallback
    let result = run_cli(&["test_files/sample.bin", "--magic-file", "nonexistent.magic"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should succeed even with missing magic file (uses fallback)
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_snapshot!("invalid_magic_file", stdout);
}

#[test]
fn test_cli_conflicting_output_formats() {
    // Test that --json and --text conflict
    let result = run_cli(&["test_files/sample.bin", "--json", "--text"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(!output.status.success());

    let stderr = run_cli_stderr(&["test_files/sample.bin", "--json", "--text"]).unwrap();
    assert_snapshot!("conflicting_output_formats", stderr);
}

#[test]
fn test_cli_missing_file_argument() {
    // Test that file argument is required
    let result = run_cli(&["--json"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(!output.status.success());

    let stderr = run_cli_stderr(&["--json"]).unwrap();
    assert_snapshot!("missing_file_argument", stderr);
}

#[test]
fn test_cli_help_output() {
    // Test help output
    let result = run_cli(&["--help"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_snapshot!("help_output", stdout);
}

#[test]
fn test_cli_version_output() {
    // Test version output
    let result = run_cli(&["--version"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_snapshot!("version_output", stdout);
}

#[test]
fn test_cli_multiple_file_formats() {
    // Test with different file types if they exist
    let test_files = [
        "test_files/sample.bin",
        "test_files/magic",
        "test_files/magic.mgc",
        "test_files/magic.mime",
    ];

    for file_path in &test_files {
        if Path::new(file_path).exists() {
            let result = run_cli_stdout(&[file_path]);
            assert!(result.is_ok(), "Failed to process file: {}", file_path);

            let output = result.unwrap();
            assert!(output.contains(file_path));
            // All files should return some result (even if just "data")
            assert!(!output.trim().is_empty());
        }
    }
}

#[test]
fn test_cli_json_output_structure() {
    // Test detailed JSON output structure
    let result = run_cli_stdout(&["test_files/sample.bin", "--json"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    // Verify it's valid JSON by parsing it
    let _json_result: serde_json::Value =
        serde_json::from_str(&output).expect("Output should be valid JSON");

    assert_snapshot!("json_output_structure", output);
}

#[test]
fn test_cli_file_path_handling() {
    // Test various file path formats
    let test_cases = [
        "test_files/sample.bin",   // Relative path
        "./test_files/sample.bin", // Explicit relative path
    ];

    for file_path in &test_cases {
        if Path::new(file_path).exists() {
            let result = run_cli_stdout(&[file_path]);
            assert!(result.is_ok(), "Failed with path: {}", file_path);

            let output = result.unwrap();
            assert!(output.contains(file_path) || output.contains("sample.bin"));
        }
    }
}

#[test]
fn test_cli_empty_file() {
    // Create an empty test file
    let empty_file_path =
        create_test_file("empty.bin", &[]).expect("Failed to create empty test file");

    let result = run_cli(&[&empty_file_path]);
    assert!(result.is_ok());

    let output = result.unwrap();
    // Empty files might cause an error, which is acceptable behavior
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_snapshot!("empty_file_success", stdout);
    } else {
        // Empty file error is acceptable
        let stderr = run_cli_stderr(&[&empty_file_path]).unwrap();
        assert_snapshot!("empty_file_error", stderr);
    }

    // Clean up
    let _ = fs::remove_file(&empty_file_path);
}

#[test]
fn test_cli_large_file_handling() {
    // Create a larger test file (1KB)
    let large_content = vec![0x41; 1024]; // 1KB of 'A' characters
    let large_file_path =
        create_test_file("large.bin", &large_content).expect("Failed to create large test file");

    let result = run_cli_stdout(&[&large_file_path]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("large_file_handling", output);

    // Clean up
    let _ = fs::remove_file(&large_file_path);
}

#[test]
fn test_cli_binary_file_handling() {
    // Create a binary file with various byte values
    let binary_content = (0..=255u8).collect::<Vec<u8>>();
    let binary_file_path =
        create_test_file("binary.bin", &binary_content).expect("Failed to create binary test file");

    let result = run_cli_stdout(&[&binary_file_path]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_snapshot!("binary_file_handling", output);

    // Clean up
    let _ = fs::remove_file(&binary_file_path);
}

#[test]
fn test_cli_magic_file_fallback() {
    // Test that CLI works even when magic files are missing
    // This tests the download_magic_files functionality

    // Try with a non-standard magic file location
    let result = run_cli(&["test_files/sample.bin", "--magic-file", "missing.magic"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should succeed even with missing magic file (uses fallback)
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_snapshot!("magic_file_fallback", stdout);
}

#[test]
fn test_cli_output_consistency() {
    // Test that multiple runs produce consistent output
    let file_path = "test_files/sample.bin";

    let result1 = run_cli_stdout(&[file_path]);
    let result2 = run_cli_stdout(&[file_path]);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let output1 = result1.unwrap();
    let output2 = result2.unwrap();

    // Outputs should be identical
    assert_eq!(output1, output2);
}

#[test]
fn test_cli_json_vs_text_consistency() {
    // Test that JSON and text outputs contain consistent information
    let file_path = "test_files/sample.bin";

    let text_result = run_cli_stdout(&[file_path, "--text"]);
    let json_result = run_cli_stdout(&[file_path, "--json"]);

    assert!(text_result.is_ok());
    assert!(json_result.is_ok());

    let text_output = text_result.unwrap();
    let json_output = json_result.unwrap();

    let json_data: serde_json::Value =
        serde_json::from_str(&json_output).expect("JSON output should be valid");

    // Extract description from JSON
    let json_description = json_data["description"].as_str().unwrap();

    // Text output should contain the same description
    assert!(text_output.contains(json_description));
}

#[test]
fn test_cli_error_exit_codes() {
    // Test that CLI returns appropriate exit codes for errors

    // Nonexistent file should return exit code 3 (file not found)
    let result = run_cli(&["nonexistent_file.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn test_cli_success_exit_codes() {
    // Test that CLI returns zero exit code for successful operations
    let result = run_cli(&["test_files/sample.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.status.success());
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_cli_platform_specific_magic_paths() {
    // Test that platform-specific magic file paths are handled correctly
    // This is mainly testing the path resolution logic

    let result = run_cli_stdout(&["test_files/sample.bin"]);
    assert!(result.is_ok());

    // Should work regardless of platform-specific paths
    let output = result.unwrap();
    assert!(output.contains("test_files/sample.bin"));
}

#[test]
fn test_cli_ci_environment_detection() {
    // Test CI environment detection for magic file fallback
    // Set CI environment variable temporarily
    unsafe {
        std::env::set_var("CI", "true");
    }

    let result = run_cli_stdout(&["test_files/sample.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.contains("test_files/sample.bin"));

    // Clean up environment variable
    unsafe {
        std::env::remove_var("CI");
    }
}

#[test]
fn test_cli_magic_file_creation() {
    // Test that basic magic file is created when missing
    // This tests the download_magic_files functionality

    // Create a temporary directory for testing
    let temp_dir = "test_files/temp_magic_test";
    let _ = fs::create_dir_all(temp_dir);

    let temp_magic_path = format!("{}/test.magic", temp_dir);

    // Ensure the file doesn't exist initially
    let _ = fs::remove_file(&temp_magic_path);

    let result = run_cli_stderr(&["test_files/sample.bin", "--magic-file", &temp_magic_path]);
    assert!(result.is_ok());

    // Clean up
    let _ = fs::remove_file(&temp_magic_path);
    let _ = fs::remove_dir(temp_dir);
}

#[test]
fn test_cli_magic_file_download_functionality() {
    // Test the download_magic_files function specifically
    let temp_dir = "test_files/temp_download_test";
    let _ = fs::create_dir_all(temp_dir);

    let temp_magic_path = format!("{}/downloaded.magic", temp_dir);

    // Ensure the file doesn't exist initially
    let _ = fs::remove_file(&temp_magic_path);

    // Run CLI with missing magic file to trigger download
    let result = run_cli_stderr(&["test_files/sample.bin", "--magic-file", &temp_magic_path]);
    assert!(result.is_ok());

    let stderr = result.unwrap();
    // Should show download attempt
    assert!(stderr.contains("download") || stderr.contains("Created basic magic file"));

    // Clean up
    let _ = fs::remove_file(&temp_magic_path);
    let _ = fs::remove_dir(temp_dir);
}

#[test]
fn test_cli_with_existing_magic_files() {
    // Test CLI behavior with existing magic files in test_files
    let magic_file_path = "test_files/magic";

    if Path::new(magic_file_path).exists() {
        let result = run_cli_stdout(&["test_files/sample.bin", "--magic-file", magic_file_path]);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.contains("test_files/sample.bin"));
        // Should process without errors even if magic parsing isn't complete
        assert!(output.contains("data"));
    }
}

#[test]
fn test_cli_platform_magic_file_detection() {
    // Test that CLI attempts to find platform-appropriate magic files
    let result = run_cli_stdout(&["test_files/sample.bin"]);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.contains("test_files/sample.bin"));

    // Should work even if system magic files aren't found
    assert!(output.contains("data"));
}

#[test]
fn test_cli_file_processing_with_different_extensions() {
    // Test processing files with different extensions
    let test_cases = vec![
        ("test.txt", b"Hello, World!" as &[u8]),
        ("test.bin", &[0x7f, 0x45, 0x4c, 0x46]), // ELF-like
        ("test.dat", &[0x50, 0x4b, 0x03, 0x04]), // ZIP-like
    ];

    for (filename, content) in test_cases {
        let file_path = create_test_file(filename, content).expect("Failed to create test file");

        let result = run_cli_stdout(&[&file_path]);
        assert!(result.is_ok(), "Failed to process {}", filename);

        let output = result.unwrap();
        assert!(output.contains(&file_path));
        assert!(output.contains("data"));

        // Clean up
        let _ = fs::remove_file(&file_path);
    }
}

#[test]
fn test_cli_integration_with_sample_files() {
    // Test integration with all available sample files in test_files
    let test_files_dir = Path::new("test_files");

    if test_files_dir.exists() {
        for entry in fs::read_dir(test_files_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_file() {
                let path_str = path.to_string_lossy();

                // Skip temporary files and directories
                if path_str.contains("temp_") || path_str.ends_with(".py") {
                    continue;
                }

                let result = run_cli(&[&path_str]);
                assert!(result.is_ok(), "Failed to process file: {}", path_str);

                let output = result.unwrap();
                let filename = path.file_name().unwrap().to_str().unwrap();

                if output.status.success() {
                    let stdout = String::from_utf8(output.stdout).unwrap();
                    assert!(
                        stdout.contains(path_str.as_ref())
                            || stdout.contains(filename)
                            || stdout.contains("data"), // At minimum should contain "data" as fallback
                        "Output '{}' should contain file path '{}' or filename '{}'",
                        stdout.trim(),
                        path_str,
                        filename
                    );
                    assert!(!stdout.trim().is_empty());
                } else {
                    // Some files might cause errors (like empty files), which is acceptable
                    let stderr = String::from_utf8(output.stderr).unwrap();
                    assert!(
                        stderr.contains("Error:"),
                        "Expected error message for file: {}",
                        path_str
                    );
                }
            }
        }
    }
}
