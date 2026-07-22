// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! File and directory loading for magic files.
//!
//! Provides functions for loading magic rules from individual files and
//! directories, with automatic format detection and error handling.

use log::warn;

use crate::error::ParseError;
use crate::parser::ParsedMagic;
use crate::parser::name_table::NameTable;
use std::path::{Path, PathBuf};

use super::format::{MagicFileFormat, detect_format};

/// Maximum magic file size (1 GB).
///
/// Applied before loading a magic file (or any file within a magic directory)
/// into memory to prevent memory-exhaustion `DoS` from maliciously oversized
/// inputs.
///
/// This value is kept in sync with `crate::io::FileBuffer::MAX_FILE_SIZE`.
/// The constant is duplicated (rather than imported) because this module is
/// also pulled in by `build.rs` via `#[path]` and the build script cannot
/// reference lib-only modules such as `crate::io`. A unit test below asserts
/// the two constants remain equal.
pub const MAX_MAGIC_FILE_SIZE: u64 = 1024 * 1024 * 1024;

/// Reads a magic file into a `String` after verifying its size does not
/// exceed [`MAX_MAGIC_FILE_SIZE`].
///
/// Returns a `ParseError` if metadata cannot be read, the file exceeds the
/// size limit, or the file contents cannot be read.
///
/// # Encoding
///
/// Magic files are parsed as byte streams (matching GNU `file`/libmagic
/// behavior). Real-world magic files frequently contain non-UTF-8 bytes in
/// comments and attribution lines (e.g., Latin-1 author names). Rather than
/// rejecting such files, invalid UTF-8 sequences are replaced with U+FFFD
/// via [`String::from_utf8_lossy`] and a warning is logged. ASCII rule
/// syntax is preserved byte-for-byte; replacements only affect non-ASCII
/// text which, in practice, appears almost exclusively inside comments
/// that are stripped before tokenization.
fn read_magic_file_bounded(path: &Path) -> Result<String, ParseError> {
    let metadata = std::fs::metadata(path).map_err(|e| {
        ParseError::IoError(std::io::Error::new(
            e.kind(),
            format!("Failed to read metadata for '{}': {}", path.display(), e),
        ))
    })?;

    if metadata.len() > MAX_MAGIC_FILE_SIZE {
        return Err(ParseError::invalid_syntax(
            0,
            format!(
                "Magic file '{}' is too large: {} bytes (maximum allowed: {} bytes)",
                path.display(),
                metadata.len(),
                MAX_MAGIC_FILE_SIZE
            ),
        ));
    }

    let bytes = std::fs::read(path).map_err(ParseError::from)?;

    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(e) => {
            warn!(
                "Magic file '{}' contains non-UTF-8 bytes; they were replaced with U+FFFD. \
                 Rule parsing proceeds, but replacements inside rule bodies may alter matching.",
                path.display()
            );
            Ok(String::from_utf8_lossy(&e.into_bytes()).into_owned())
        }
    }
}

/// Whether `contents` contains at least one actual rule line -- a non-blank
/// line that is neither a comment (`#`) nor a `!:` metadata directive
/// (`!:mime`, `!:strength`, ...). Used by [`load_magic_directory`] to tell a
/// file whose rules were all skipped as unparseable (unusable) apart from a
/// genuinely empty, comment-only, or directive-only file (valid, contributes
/// no rules). `!:` directives are stripped during preprocessing and are not
/// rules, so counting them here would misclassify a directive-only file as
/// "had rules but all were skipped".
fn has_rule_lines(contents: &str) -> bool {
    contents.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("!:")
    })
}

/// Loads and parses all magic files from a directory, merging them into a single rule set.
///
/// This function reads all regular files in the specified directory, parses each as a magic file,
/// and combines the resulting rules into a single `Vec<MagicRule>`. Files are processed in
/// alphabetical order by filename to ensure deterministic results.
///
/// # Error Handling Strategy
///
/// This function distinguishes between critical and non-critical errors:
///
/// - **Critical errors** (I/O failures, directory access issues, encoding problems):
///   These cause immediate failure and return a `ParseError`. The function stops processing
///   and propagates the error to the caller.
///
/// - **Non-critical errors** (individual file parse failures):
///   These are logged at warn level and the file is skipped. Processing
///   continues with remaining files.
///
/// # Behavior
///
/// - Subdirectories are skipped (not recursively processed)
/// - Symbolic links are skipped
/// - Empty directories return an empty rules vector
/// - Files are processed in alphabetical order by filename
/// - All successfully parsed rules are merged in order
///
/// # Examples
///
/// Loading a directory of magic files:
///
/// ```rust,no_run
/// use libmagic_rs::parser::load_magic_directory;
/// use std::path::Path;
///
/// let parsed = load_magic_directory(Path::new("/usr/share/file/magic.d"))?;
/// println!("Loaded {} rules from directory", parsed.rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// Creating a Magdir-style directory structure:
///
/// ```rust,no_run
/// use libmagic_rs::parser::load_magic_directory;
/// use std::path::Path;
///
/// // Directory structure:
/// // magic.d/
/// //   ├── 01-elf
/// //   ├── 02-archive
/// //   └── 03-text
///
/// let parsed = load_magic_directory(Path::new("./magic.d"))?;
/// // Rules from all three files are merged in alphabetical order
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The directory does not exist or cannot be accessed
/// - Directory entries cannot be read
/// - A file cannot be read due to I/O errors
/// - A file contains invalid UTF-8 encoding
///
/// # Panics
///
/// This function does not panic under normal operation.
// Directory iteration + per-file parse + error aggregation runs slightly over
// the 100-line lint; splitting this module is tracked in #391.
#[allow(clippy::too_many_lines)]
pub fn load_magic_directory(dir_path: &Path) -> Result<ParsedMagic, ParseError> {
    use std::fs;

    // Read directory entries
    let entries = fs::read_dir(dir_path).map_err(|e| {
        ParseError::invalid_syntax(
            0,
            format!("Failed to read directory '{}': {}", dir_path.display(), e),
        )
    })?;

    // Collect and sort entries by filename for deterministic ordering
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            ParseError::invalid_syntax(
                0,
                format!(
                    "Failed to read directory entry in '{}': {}",
                    dir_path.display(),
                    e
                ),
            )
        })?;

        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            ParseError::invalid_syntax(
                0,
                format!("Failed to read file type for '{}': {}", path.display(), e),
            )
        })?;

        // Only process regular files, skip directories and symlinks.
        // `is_file()` (not `!is_dir()`) is deliberate: sockets, FIFOs, and
        // device nodes must also be excluded from magic-file discovery.
        #[allow(clippy::filetype_is_file)]
        if file_type.is_file() && !file_type.is_symlink() {
            file_paths.push(path);
        }
    }

    // Sort by filename for deterministic ordering
    file_paths.sort_by_key(|path| path.file_name().map(std::ffi::OsStr::to_os_string));

    // Accumulate rules and name tables from all files
    let mut all_rules = Vec::new();
    let mut merged_table = NameTable::empty();
    let mut parse_failures: Vec<(PathBuf, ParseError)> = Vec::new();
    // Files that parsed (line-tolerantly) but contributed no usable rule or
    // name despite having non-empty content -- i.e. every rule was skipped as
    // unparseable. Tracked so an all-unusable directory is still reported as a
    // failure even though tolerant parsing returns Ok for each such file.
    let mut empty_files: Vec<PathBuf> = Vec::new();
    let mut any_success = false;
    let file_count = file_paths.len();

    for path in file_paths {
        // Read file contents (size-bounded to prevent memory exhaustion)
        let contents = match read_magic_file_bounded(&path) {
            Ok(contents) => contents,
            Err(e) => {
                // I/O errors (including oversized files) are critical
                return Err(ParseError::invalid_syntax(
                    0,
                    format!("Failed to read file '{}': {}", path.display(), e),
                ));
            }
        };

        // Parse the file (line-tolerant: unparseable rules are skipped with a
        // warning rather than dropping the whole file).
        match super::parse_text_magic_file_tolerant(&contents, Some(&path)) {
            Ok(parsed) => {
                if parsed.rules.is_empty() && parsed.name_table.is_empty() {
                    // Contributed nothing usable. If the file had actual rule
                    // lines (not just comments/blank lines), they were all
                    // skipped as unparseable -- record it so an entirely-
                    // unusable directory is still reported as a failure. A
                    // genuinely empty or comment-only file is valid and is NOT
                    // recorded (it simply contributes no rules).
                    if has_rule_lines(&contents) {
                        empty_files.push(path);
                    }
                } else {
                    any_success = true;
                    all_rules.extend(parsed.rules);
                    merged_table.merge(parsed.name_table);
                }
            }
            Err(e) => {
                // Hard (preprocess-level) failure -- track for reporting.
                parse_failures.push((path, e));
            }
        }
    }

    // A directory has loaded only if some file contributed a usable rule or
    // name-table entry. `any_success` (not `all_rules.is_empty()`) so that a
    // directory of pure `name`-subroutine files is not mistaken for failure.
    // With line-tolerant parsing, a file whose rules were all skipped returns
    // Ok with nothing; a directory where every content-bearing file is like
    // that (or fails preprocessing) has loaded nothing and is reported as a
    // failure -- preserving the pre-tolerance contract for a wholly-unusable
    // directory.
    if !any_success && (!parse_failures.is_empty() || !empty_files.is_empty()) {
        use std::fmt::Write;

        let mut problems: Vec<String> = parse_failures
            .iter()
            .map(|(path, e)| format!("  - {}: {}", path.display(), e))
            .collect();
        problems.extend(
            empty_files
                .iter()
                .map(|path| format!("  - {}: no usable rules (all skipped)", path.display())),
        );

        let mut message = format!(
            "All {file_count} magic file(s) in directory failed to parse or produced no usable rules"
        );
        let shown = problems.iter().take(3).cloned().collect::<Vec<_>>();
        if !shown.is_empty() {
            message.push_str(":\n");
            message.push_str(&shown.join("\n"));
            if problems.len() > 3 {
                // fmt::Write to a String is infallible; discard the Result
                // rather than unwrap so the no-panic policy holds regardless.
                #[allow(clippy::let_underscore_must_use)]
                let _ = write!(
                    message,
                    "\n  ... and {} more",
                    problems.len().saturating_sub(3)
                );
            }
        }

        return Err(ParseError::invalid_syntax(0, message));
    }

    // Log warnings for partial failures (some files parsed, some failed)
    for (path, e) in &parse_failures {
        warn!("Failed to parse '{}': {}", path.display(), e);
    }

    Ok(ParsedMagic {
        rules: all_rules,
        name_table: merged_table,
    })
}

/// Loads magic rules from a file or directory, automatically detecting the format.
///
/// This is the unified entry point for loading magic rules from the filesystem. It
/// automatically detects whether the path points to a text magic file, a directory
/// containing magic files, or a binary compiled magic file, and dispatches to the
/// appropriate handler.
///
/// # Format Detection and Handling
///
/// The function uses [`detect_format()`] to determine the file type and handles each
/// format as follows:
///
/// - **Text format**: Reads the file contents and parses using [`super::parse_text_magic_file()`]
/// - **Directory format**: Loads all magic files from the directory using [`load_magic_directory()`]
/// - **Binary format**: Returns an error with guidance to use the `--use-builtin` option
///
/// # Arguments
///
/// * `path` - Path to a magic file or directory. Can be absolute or relative.
///
/// # Returns
///
/// Returns `Ok(Vec<MagicRule>)` containing all successfully parsed magic rules. For
/// directories, rules from all files are merged in alphabetical order by filename.
///
/// # Errors
///
/// This function returns a [`ParseError`] in the following cases:
///
/// - **File not found**: The specified path does not exist
/// - **Unsupported format**: The file is a binary compiled magic file (`.mgc`)
/// - **Parse errors**: The magic file contains syntax errors or invalid rules
/// - **I/O errors**: File system errors during reading (permissions, disk errors, etc.)
///
/// # Examples
///
/// ## Loading a text magic file
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// let parsed = load_magic_file(Path::new("/usr/share/misc/magic"))?;
/// println!("Loaded {} magic rules", parsed.rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// ## Loading a directory of magic files
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// let parsed = load_magic_file(Path::new("/usr/share/misc/magic.d"))?;
/// println!("Loaded {} rules from directory", parsed.rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// ## Handling binary format errors
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// match load_magic_file(Path::new("/usr/share/misc/magic.mgc")) {
///     Ok(parsed) => println!("Loaded {} rules", parsed.rules.len()),
///     Err(e) => {
///         eprintln!("Error loading magic file: {}", e);
///         eprintln!("Hint: Use --use-builtin for binary files");
///     }
/// }
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// # Security
///
/// This function delegates to [`super::parse_text_magic_file()`] or [`load_magic_directory()`]
/// based on format detection. Security considerations are handled by those functions:
///
/// - Rule hierarchy depth is bounded during parsing
/// - Invalid syntax is rejected with descriptive errors
/// - Binary `.mgc` files are rejected (not parsed)
///
/// A 1 GB size limit (`MAX_MAGIC_FILE_SIZE`, module-internal) is enforced on each file loaded
/// (both standalone files and files within a directory) to prevent memory
/// exhaustion from maliciously oversized inputs. Files exceeding the limit are
/// rejected with a `ParseError` before their contents are read.
///
/// # See Also
///
/// - [`detect_format()`] - Format detection logic
/// - [`super::parse_text_magic_file()`] - Text file parser
/// - [`load_magic_directory()`] - Directory loader
pub fn load_magic_file(path: &Path) -> Result<ParsedMagic, ParseError> {
    // Detect the magic file format
    let format = detect_format(path)?;

    // Dispatch to appropriate handler based on format
    match format {
        MagicFileFormat::Text => {
            // Read file contents (size-bounded) and parse as text magic file
            let content = read_magic_file_bounded(path)?;
            super::parse_text_magic_file_tolerant(&content, Some(path))
        }
        MagicFileFormat::Directory => {
            // Load all magic files from directory
            load_magic_directory(path)
        }
        MagicFileFormat::Binary => {
            // Binary compiled magic files are not supported
            Err(ParseError::unsupported_format(
                0,
                "binary .mgc file",
                "Binary compiled magic files (.mgc) are not supported for parsing.\n\
                 Use the --use-builtin option to use the built-in magic rules instead,\n\
                 or provide a text-based magic file or directory.",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    // Restriction lints without an allow-*-in-tests config option;
    // tests create exactly one level under a fresh TempDir.
    #![allow(clippy::create_dir)]

    use super::*;

    // ============================================================
    // Tests for load_magic_directory (6+ test cases)
    // ============================================================

    #[test]
    fn test_load_directory_critical_error_io() {
        use std::path::Path;

        let non_existent = Path::new("/this/should/not/exist/anywhere/at/all");
        let result = load_magic_directory(non_existent);

        assert!(
            result.is_err(),
            "Should return error for non-existent directory"
        );
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to read directory"));
    }

    #[test]
    fn test_load_directory_non_critical_error_parse() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a valid file
        let valid_path = temp_dir.path().join("valid.magic");
        fs::write(&valid_path, "0 string \\x01\\x02 valid\n").expect("Failed to write valid file");

        // Create an invalid file
        let invalid_path = temp_dir.path().join("invalid.magic");
        fs::write(&invalid_path, "this is invalid syntax\n").expect("Failed to write invalid file");

        // Should succeed, loading only the valid file
        let parsed = load_magic_directory(temp_dir.path()).expect("Should load valid files");

        assert_eq!(parsed.rules.len(), 1, "Should load only valid file");
        assert_eq!(parsed.rules[0].message, "valid");
    }

    #[test]
    fn test_load_directory_empty_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create an empty file
        let empty_path = temp_dir.path().join("empty.magic");
        fs::write(&empty_path, "").expect("Failed to write empty file");

        // Create a file with only comments
        let comments_path = temp_dir.path().join("comments.magic");
        fs::write(&comments_path, "# Just comments\n# Nothing else\n")
            .expect("Failed to write comments file");

        // Should succeed with no rules
        let parsed = load_magic_directory(temp_dir.path()).expect("Should handle empty files");

        assert_eq!(
            parsed.rules.len(),
            0,
            "Empty files should contribute no rules"
        );
    }

    #[test]
    fn test_load_directory_all_content_bearing_but_all_rules_skipped_errors() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Two files that BOTH have content (non-comment rule lines) but whose
        // every rule is unparseable. Under line-tolerant parsing (GOTCHAS S3.11)
        // each file returns Ok with zero rules, but the directory contributed
        // nothing usable, so `load_magic_directory` must still Err -- preserving
        // the pre-tolerance "all failed" contract via the `empty_files` /
        // `has_rule_lines` tracking. This is distinct from a directory of
        // genuinely-empty/comment-only files (test_load_directory_empty_files),
        // which is a valid no-op success, and from the mixed valid+invalid case
        // (test_load_directory_non_critical_error_parse), which succeeds because
        // one file contributed a rule.
        fs::write(
            temp_dir.path().join("bad1.magic"),
            "notanoffset badtype whatever\nalso not a valid rule line\n",
        )
        .expect("Failed to write bad1");
        fs::write(
            temp_dir.path().join("bad2.magic"),
            "still invalid syntax here\n",
        )
        .expect("Failed to write bad2");

        let err = load_magic_directory(temp_dir.path()).expect_err(
            "a directory whose content-bearing files all parse to zero rules must fail",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("failed to parse"),
            "error must report the all-failed contract: {msg}"
        );
        assert!(
            msg.contains("no usable rules (all skipped)"),
            "error must attribute the content-bearing-but-all-skipped files: {msg}"
        );
    }

    #[test]
    fn test_load_directory_binary_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a binary file (invalid UTF-8). Lossy conversion turns this
        // into U+FFFD characters that the grammar parser cannot interpret as
        // a rule; the directory loader treats that as a non-critical parse
        // failure and skips the file.
        let binary_path = temp_dir.path().join("binary.dat");
        fs::write(&binary_path, [0xFF, 0xFE, 0xFF, 0xFE]).expect("Failed to write binary file");

        // Create a valid text file
        let valid_path = temp_dir.path().join("valid.magic");
        fs::write(&valid_path, "0 string \\x01\\x02 valid\n").expect("Failed to write valid file");

        let parsed = load_magic_directory(temp_dir.path())
            .expect("Directory with a binary file alongside a valid file should still load");

        assert_eq!(
            parsed.rules.len(),
            1,
            "Only the valid magic file should contribute rules"
        );
        assert_eq!(parsed.rules[0].message, "valid");
    }

    #[test]
    fn test_load_directory_mixed_extensions() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create files with different extensions
        fs::write(
            temp_dir.path().join("file.magic"),
            "0 string \\x01\\x02 magic\n",
        )
        .expect("Failed to write .magic file");
        fs::write(
            temp_dir.path().join("file.txt"),
            "0 string \\x03\\x04 txt\n",
        )
        .expect("Failed to write .txt file");
        fs::write(temp_dir.path().join("noext"), "0 string \\x05\\x06 noext\n")
            .expect("Failed to write no-ext file");

        let parsed = load_magic_directory(temp_dir.path())
            .expect("Should load all files regardless of extension");

        assert_eq!(
            parsed.rules.len(),
            3,
            "Should process all files regardless of extension"
        );

        let messages: Vec<&str> = parsed.rules.iter().map(|r| r.message.as_str()).collect();
        assert!(messages.contains(&"magic"));
        assert!(messages.contains(&"txt"));
        assert!(messages.contains(&"noext"));
    }

    #[test]
    fn test_load_directory_alphabetical_ordering() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create files in non-alphabetical order - using valid magic syntax with hex escapes
        fs::write(
            temp_dir.path().join("03-third"),
            "0 string \\x07\\x08\\x09 third\n",
        )
        .expect("Failed to write third file");
        fs::write(
            temp_dir.path().join("01-first"),
            "0 string \\x01\\x02\\x03 first\n",
        )
        .expect("Failed to write first file");
        fs::write(
            temp_dir.path().join("02-second"),
            "0 string \\x04\\x05\\x06 second\n",
        )
        .expect("Failed to write second file");

        let parsed = load_magic_directory(temp_dir.path()).expect("Should load directory in order");

        assert_eq!(parsed.rules.len(), 3);
        // Should be sorted alphabetically by filename
        assert_eq!(parsed.rules[0].message, "first");
        assert_eq!(parsed.rules[1].message, "second");
        assert_eq!(parsed.rules[2].message, "third");
    }

    // ============================================================
    // Tests for load_magic_file (5+ test cases)
    // ============================================================

    #[test]
    fn test_load_magic_file_text_format() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let magic_file = temp_dir.path().join("magic.txt");

        // Create text magic file with valid content
        fs::write(&magic_file, "0 string \\x7fELF ELF executable\n")
            .expect("Failed to write magic file");

        // Load using load_magic_file
        let parsed = load_magic_file(&magic_file).expect("Failed to load text magic file");

        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].message, "ELF executable");
    }

    #[test]
    fn test_load_magic_file_directory_format() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let magic_dir = temp_dir.path().join("magic.d");
        fs::create_dir(&magic_dir).expect("Failed to create magic directory");

        // Create multiple files in directory
        fs::write(
            magic_dir.join("00_elf"),
            "0 string \\x7fELF ELF executable\n",
        )
        .expect("Failed to write elf file");
        fs::write(
            magic_dir.join("01_zip"),
            "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
        )
        .expect("Failed to write zip file");

        // Load using load_magic_file
        let parsed = load_magic_file(&magic_dir).expect("Failed to load directory");

        assert_eq!(parsed.rules.len(), 2);
        assert_eq!(parsed.rules[0].message, "ELF executable");
        assert_eq!(parsed.rules[1].message, "ZIP archive");
    }

    #[test]
    fn test_load_magic_file_binary_format_error() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let binary_file = temp_dir.path().join("magic.mgc");

        // Create binary file with .mgc magic number
        let mut file = File::create(&binary_file).expect("Failed to create binary file");
        let magic_number: [u8; 4] = [0x1C, 0x04, 0x1E, 0xF1]; // Little-endian 0xF11E041C
        file.write_all(&magic_number)
            .expect("Failed to write magic number");

        // Attempt to load binary file
        let result = load_magic_file(&binary_file);

        assert!(result.is_err(), "Should fail to load binary .mgc file");

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Verify error mentions unsupported format and --use-builtin
        assert!(
            error_msg.contains("Binary") || error_msg.contains("binary"),
            "Error should mention binary format: {error_msg}",
        );
        assert!(
            error_msg.contains("--use-builtin") || error_msg.contains("built-in"),
            "Error should mention --use-builtin option: {error_msg}",
        );
    }

    #[test]
    fn test_load_magic_file_io_error() {
        use std::path::Path;

        // Try to load non-existent file
        let non_existent = Path::new("/this/path/should/not/exist/magic.txt");
        let result = load_magic_file(non_existent);

        assert!(result.is_err(), "Should fail for non-existent file");
    }

    #[test]
    fn test_load_magic_file_tolerates_unparseable_rule_and_keeps_valid_ones() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let mixed_file = temp_dir.path().join("mixed.magic");

        // A valid rule, an unparseable rule (missing offset), then another valid
        // rule. Runtime loading is line-tolerant (GNU `file` semantics, GOTCHAS
        // S3.11): the bad rule is skipped with a warning and the valid rules on
        // either side survive, instead of the whole file being dropped. (This
        // is what lets real system magic files keep their common-format
        // detection despite a stray construct this parser cannot yet handle.)
        fs::write(
            &mixed_file,
            "0 string GOOD1 first good rule\nstring test invalid\n0 string GOOD2 second good rule\n",
        )
        .expect("Failed to write file");

        let parsed = load_magic_file(&mixed_file)
            .expect("runtime load must tolerate an unparseable rule, not abort the whole file");
        let msgs: Vec<&str> = parsed.rules.iter().map(|r| r.message.as_str()).collect();
        assert!(
            msgs.contains(&"first good rule"),
            "a valid rule before the bad one must survive: {msgs:?}"
        );
        assert!(
            msgs.contains(&"second good rule"),
            "a valid rule after the bad one must survive: {msgs:?}"
        );
        assert_eq!(
            parsed.rules.len(),
            2,
            "the unparseable rule must be dropped, keeping exactly the two valid ones: {msgs:?}"
        );
    }

    #[test]
    fn test_tolerant_skip_warning_includes_source_file_path() {
        use std::fs;
        use tempfile::TempDir;

        // The skipped-rule warning must carry the source file path so that a
        // directory load spanning many files can locate the offending rule
        // (issue #391 item 3). The loader threads the path through
        // `parse_text_magic_file_tolerant` -> `build_rule_hierarchy_tolerant`.
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let bad_file = temp_dir.path().join("has_bad_rule.magic");
        fs::write(&bad_file, "0 string GOOD good rule\nstring test invalid\n")
            .expect("Failed to write file");

        testing_logger::setup();
        let _ = load_magic_file(&bad_file).expect("tolerant load must not abort");
        let path_str = bad_file.display().to_string();
        testing_logger::validate(|captured_logs| {
            let skip_warns: Vec<_> = captured_logs
                .iter()
                .filter(|l| l.body.contains("skipping unparseable magic rule"))
                .collect();
            assert_eq!(
                skip_warns.len(),
                1,
                "expected exactly one skip warning, got: {:?}",
                captured_logs.iter().map(|l| &l.body).collect::<Vec<_>>()
            );
            assert_eq!(skip_warns[0].level, log::Level::Warn);
            assert!(
                skip_warns[0].body.contains(&path_str),
                "skip warning must include the source file path '{path_str}', got: {}",
                skip_warns[0].body
            );
        });
    }

    #[test]
    fn test_load_magic_file_drops_subtree_of_unparseable_rule_without_reattaching() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file = temp_dir.path().join("subtree.magic");

        // A valid parent, then an UNPARSEABLE rule (no valid offset) that owns a
        // `>`-indented child, then a valid sibling back at the original level.
        // GOTCHAS S3.11: when the bad rule is skipped, its deeper-indented
        // subtree must be dropped WITH it (`skip_subtree_deeper_than` in
        // `hierarchy.rs`). The orphaned child must NOT silently re-attach to the
        // previous level-0 rule (`GOOD1`) nor survive as a stray top-level rule,
        // and the trailing `GOOD2` proves the skip threshold resets so a later
        // sibling at the original indent still parses.
        fs::write(
            &file,
            "0 string GOOD1 parent rule\n\
             notanoffset badtype orphan parent\n\
             >0 byte x orphaned child that must be dropped\n\
             0 string GOOD2 sibling after the dropped subtree\n",
        )
        .expect("Failed to write file");

        let parsed = load_magic_file(&file)
            .expect("runtime load must tolerate the unparseable rule and its subtree");

        let top_msgs: Vec<&str> = parsed.rules.iter().map(|r| r.message.as_str()).collect();
        assert_eq!(
            parsed.rules.len(),
            2,
            "exactly the two valid top-level rules survive: {top_msgs:?}"
        );
        assert!(
            top_msgs.contains(&"parent rule"),
            "the valid rule before the bad one must survive: {top_msgs:?}"
        );
        assert!(
            top_msgs.contains(&"sibling after the dropped subtree"),
            "the sibling after the dropped subtree must parse (threshold reset): {top_msgs:?}"
        );

        // The orphaned child must appear NOWHERE: not re-attached to the
        // preceding level-0 rule, and not as any surviving rule's child.
        let good1 = parsed
            .rules
            .iter()
            .find(|r| r.message == "parent rule")
            .expect("GOOD1 must be present");
        assert!(
            good1.children.is_empty(),
            "the dropped child must not re-attach to the previous level-0 rule: {:?}",
            good1
                .children
                .iter()
                .map(|c| c.message.as_str())
                .collect::<Vec<_>>()
        );
        let orphan_reattached = parsed
            .rules
            .iter()
            .flat_map(|r| r.children.iter())
            .any(|c| c.message.contains("orphaned child"));
        assert!(
            !orphan_reattached,
            "the orphaned child of the unparseable rule must be dropped entirely"
        );
    }

    #[test]
    fn test_max_magic_file_size_matches_file_buffer_limit() {
        // Ensure the duplicated limit stays in sync with FileBuffer::MAX_FILE_SIZE.
        // loader.rs cannot `use crate::io::FileBuffer` at module scope because
        // build.rs pulls this file in via `#[path]`, but tests compile as part
        // of the library and can reach it fine.
        assert_eq!(
            MAX_MAGIC_FILE_SIZE,
            crate::io::FileBuffer::MAX_FILE_SIZE,
            "MAX_MAGIC_FILE_SIZE must match FileBuffer::MAX_FILE_SIZE"
        );
    }

    #[test]
    fn test_load_magic_file_rejects_oversized_file() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let oversized = temp_dir.path().join("huge.magic");

        // Create a sparse file whose reported size exceeds MAX_MAGIC_FILE_SIZE
        // without actually consuming that much disk space.
        let file = File::create(&oversized).expect("Failed to create oversized file");
        file.set_len(MAX_MAGIC_FILE_SIZE + 1)
            .expect("Failed to set sparse file length");
        drop(file);

        let result = load_magic_file(&oversized);

        assert!(
            result.is_err(),
            "Loading a file larger than MAX_MAGIC_FILE_SIZE must fail"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("too large"),
            "Error should indicate size limit violation, got: {err_msg}"
        );
        assert!(
            err_msg.contains(&MAX_MAGIC_FILE_SIZE.to_string()),
            "Error should mention the maximum allowed size, got: {err_msg}"
        );
    }

    #[test]
    fn test_load_magic_file_tolerates_non_utf8_in_comment() {
        // Regression: /usr/share/file/magic/filesystems on macOS contains a
        // Latin-1 `ß` (0xdf) in a contributor attribution comment. Previously
        // this was rejected by `fs::read_to_string` with an opaque "stream
        // did not contain valid UTF-8" error. The loader must now tolerate
        // non-UTF-8 bytes in comments (and anywhere else they appear) by
        // lossily replacing them.
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let magic_path = temp_dir.path().join("with-latin1-comment.magic");

        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"# From: Thomas Wei");
        bytes.push(0xdf); // invalid UTF-8 (Latin-1 encoding of `ß`)
        bytes.extend_from_slice(b"schuh <thomas@example.invalid>\n");
        bytes.extend_from_slice(b"0 string \\x7fELF ELF executable\n");
        fs::write(&magic_path, &bytes).expect("Failed to write magic file with non-UTF-8 byte");

        let parsed = load_magic_file(&magic_path)
            .expect("Magic file with non-UTF-8 bytes in a comment must still load");

        assert_eq!(
            parsed.rules.len(),
            1,
            "The ELF rule should be parsed; the comment is stripped"
        );
        assert_eq!(parsed.rules[0].message, "ELF executable");
    }

    #[test]
    fn test_load_directory_merges_name_tables() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Each file defines a different named subroutine.
        fs::write(
            temp_dir.path().join("00_first"),
            "0 name sub_a\n>0 byte 1 a-body\n",
        )
        .expect("Failed to write sub_a file");
        fs::write(
            temp_dir.path().join("01_second"),
            "0 name sub_b\n>0 byte 2 b-body\n",
        )
        .expect("Failed to write sub_b file");

        let parsed =
            load_magic_directory(temp_dir.path()).expect("Should load both name subroutines");

        // Both `name` rules are hoisted out, so top-level rules list is empty.
        assert_eq!(parsed.rules.len(), 0);
        assert!(parsed.name_table.get("sub_a").is_some());
        assert!(parsed.name_table.get("sub_b").is_some());
    }
}
