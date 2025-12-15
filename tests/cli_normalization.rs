//! Tests for CLI output normalization functionality
//!
//! These tests ensure that the cross-platform normalization helpers work correctly
//! and remain stable across different environments.

use insta::assert_snapshot;

mod common;

#[test]
fn normalizes_executable_suffix_in_snapshots() {
    // Test that the normalization function works correctly for Windows executable names
    let input = "Usage: rmagic.exe [OPTIONS] <FILE>\n\nArguments:\n  <FILE>  File to analyze";
    let normalized = common::normalize_cli_output(input);
    assert_snapshot!("normalize_exe_suffix", normalized);
}

#[test]
fn normalizes_windows_path_prefixes() {
    // Test that Windows path prefixes are normalized correctly
    let input = "Failed to access file: File '\\\\?\\C:\\Users\\test\\file.bin' is empty";
    let normalized = common::normalize_cli_output(input);
    assert_snapshot!("normalize_path_prefix", normalized);
}

#[test]
fn filters_cargo_error_messages() {
    // Test that cargo error messages are filtered out
    let input = "Error: File not found\nThe specified file does not exist.\nerror: process didn't exit successfully: `target\\debug\\rmagic.exe file.bin` (exit code: 3)";
    let normalized = common::normalize_cli_output(input);
    assert_snapshot!("filter_cargo_errors", normalized);
}

#[test]
fn combines_all_normalization_features() {
    // Test that all normalization features work together
    let input = r#"Usage: rmagic.exe [OPTIONS]
Error: File access failed
Failed to access file: File '\\?\D:\test\file.txt' is empty
Please check the file path and permissions.
error: process didn't exit successfully: `target\debug\rmagic.exe test.bin` (exit code: 3)"#;

    let normalized = common::normalize_cli_output(input);
    assert_snapshot!("combined_normalization", normalized);
}
