//! Common test utilities for cross-platform compatibility
//!
//! This module provides helpers for normalizing test outputs to ensure
//! consistent snapshot testing across different operating systems.

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
