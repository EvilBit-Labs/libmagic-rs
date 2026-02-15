// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Common test utilities for cross-platform compatibility
//!
//! This module provides helpers for normalizing test outputs to ensure
//! consistent snapshot testing across different operating systems.

#![allow(dead_code)]

/// Normalize CLI output for cross-platform snapshot consistency
///
/// This function normalizes executable names like "rmagic.exe" to "rmagic"
/// and removes Windows-style path prefixes for consistent snapshots.
///
/// # Example
///
/// ```rust
/// let output = get_cli_output();
/// let normalized = normalize_cli_output(&output);
/// assert_snapshot!("help_output", normalized);
/// ```
pub fn normalize_cli_output(input: &str) -> String {
    input
        .replace("rmagic.exe", "rmagic")
        .replace("\\\\?\\", "")
        // Also filter out full cargo stderr messages that might leak through
        .lines()
        .filter(|line| !line.contains("error: process didn't exit successfully:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Extract just the filename from a path that may contain `third_party/tests/`
///
/// This normalizes absolute paths to just show the relative portion after
/// `third_party/tests/` to make snapshots portable across different machines.
///
/// # Examples
///
/// ```rust
/// use crate::common::normalize_testfile_path;
///
/// assert_eq!(
///     normalize_testfile_path("/home/user/project/third_party/tests/file.testfile"),
///     "file.testfile"
/// );
/// assert_eq!(
///     normalize_testfile_path("C:\\Users\\me\\project\\third_party\\tests\\file.testfile"),
///     "file.testfile"
/// );
/// ```
pub fn normalize_testfile_path(path: &str) -> String {
    // Look for third_party/tests in the path and take everything after it
    if let Some(pos) = path.find("third_party/tests/") {
        return path[pos + "third_party/tests/".len()..].to_string();
    }

    // Also handle Windows-style paths
    if let Some(pos) = path.find("third_party\\tests\\") {
        return path[pos + "third_party\\tests\\".len()..].replace('\\', "/");
    }

    // If no third_party/tests found, just return the filename
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Normalize all paths in text output that reference third_party/tests files
///
/// This function scans through text and replaces any absolute paths that contain
/// `third_party/tests/` with just the relative filename portion, making snapshots
/// portable across different machines and operating systems.
///
/// # Examples
///
/// ```rust
/// use crate::common::normalize_paths_in_text;
///
/// let output = "/home/user/project/third_party/tests/file.testfile: data";
/// assert_eq!(normalize_paths_in_text(output), "file.testfile: data");
/// ```
pub fn normalize_paths_in_text(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static UNIX_PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    static WINDOWS_PATH_REGEX: OnceLock<Regex> = OnceLock::new();

    let unix_re = UNIX_PATH_REGEX.get_or_init(|| {
        Regex::new(r"(?m)([^\s]*)/third_party/tests/([^\s:]+)").expect("valid regex")
    });

    let windows_re = WINDOWS_PATH_REGEX.get_or_init(|| {
        Regex::new(r"(?m)([^\s]*)\\third_party\\tests\\([^\s:]+)").expect("valid regex")
    });

    // First handle Unix-style paths
    let text = unix_re.replace_all(text, "$2");

    // Then handle Windows-style paths
    let text = windows_re.replace_all(&text, "$2");

    // For now, just preserve the text as-is since the main issue was absolute paths
    // which are already handled by the path regex patterns above.
    // We can add more sophisticated backslash handling later if needed.
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_testfile_path_unix() {
        assert_eq!(
            normalize_testfile_path("/home/user/project/third_party/tests/file.testfile"),
            "file.testfile"
        );

        assert_eq!(
            normalize_testfile_path("/long/nested/path/third_party/tests/subfolder/test.result"),
            "subfolder/test.result"
        );
    }

    #[test]
    fn test_normalize_testfile_path_windows() {
        assert_eq!(
            normalize_testfile_path("C:\\Users\\me\\project\\third_party\\tests\\file.testfile"),
            "file.testfile"
        );

        assert_eq!(
            normalize_testfile_path("D:\\workspace\\proj\\third_party\\tests\\sub\\test.result"),
            "sub/test.result"
        );
    }

    #[test]
    fn test_normalize_testfile_path_no_third_party() {
        assert_eq!(
            normalize_testfile_path("/some/random/path/file.txt"),
            "file.txt"
        );

        assert_eq!(
            normalize_testfile_path("just_a_filename.test"),
            "just_a_filename.test"
        );
    }

    #[test]
    fn test_normalize_paths_in_text_unix() {
        let input = "/home/user/project/third_party/tests/android-vdex-1.testfile\n  got: 'data'";
        let expected = "android-vdex-1.testfile\n  got: 'data'";
        assert_eq!(normalize_paths_in_text(input), expected);
    }

    #[test]
    fn test_normalize_paths_in_text_windows() {
        let input = "C:\\Users\\me\\project\\third_party\\tests\\file.testfile: data";
        let expected = "file.testfile: data";
        assert_eq!(normalize_paths_in_text(input), expected);
    }

    #[test]
    fn test_normalize_paths_in_text_mixed() {
        let input = "Multiple paths:\n/unix/path/third_party/tests/file1.test\nC:\\Windows\\path\\third_party\\tests\\file2.test";
        let expected = "Multiple paths:\nfile1.test\nfile2.test";
        assert_eq!(normalize_paths_in_text(input), expected);
    }

    #[test]
    fn test_normalize_paths_in_text_no_change() {
        let input = "No paths to normalize here";
        assert_eq!(normalize_paths_in_text(input), input);
    }
}
