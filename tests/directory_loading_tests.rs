// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for directory loading functionality.
//!
//! These tests validate the `load_magic_directory()` function's behavior
//! with various directory structures and content scenarios.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(clippy::expect_used, clippy::create_dir)]

use libmagic_rs::parser::{ParsedMagic, load_magic_directory};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper function to create a test magic file in a directory.
fn create_test_magic_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let file_path = dir.join(name);
    fs::write(&file_path, content).expect("Failed to write test magic file");
    file_path
}

/// Helper function to create a realistic Magdir-style directory structure.
fn create_magdir_structure(dir: &Path) -> Vec<PathBuf> {
    vec![
        // ELF file detection
        create_test_magic_file(
            dir,
            "01-elf",
            "# ELF executables\n\
             0 string \\x7fELF ELF executable\n\
             >4 byte 1 32-bit\n\
             >4 byte 2 64-bit\n",
        ),
        // Archive formats
        create_test_magic_file(
            dir,
            "02-archive",
            "# Archive formats\n\
             0 string \\x21\\x3c ar archive\n\
             0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
        ),
        // Text files
        create_test_magic_file(
            dir,
            "03-text",
            "# Text files\n\
             0 string \\x23\\x21 shell script\n\
             0 string \\x23\\x21 bash script\n",
        ),
    ]
}

#[test]
fn test_load_empty_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load empty directory");

    assert_eq!(rules.len(), 0, "Empty directory should return no rules");
}

#[test]
fn test_load_directory_single_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    create_test_magic_file(
        temp_dir.path(),
        "test.magic",
        "0 string \\x7fELF ELF executable\n\
         >4 byte 1 32-bit\n\
         >4 byte 2 64-bit\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(rules.len(), 1, "Should load one top-level rule");
    assert_eq!(rules[0].message, "ELF executable");
    assert_eq!(rules[0].children.len(), 2, "Should have 2 child rules");
}

#[test]
fn test_load_directory_multiple_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    create_test_magic_file(
        temp_dir.path(),
        "elf.magic",
        "0 string \\x7fELF ELF executable\n",
    );

    create_test_magic_file(
        temp_dir.path(),
        "archive.magic",
        "0 string \\x21\\x3c ar archive\n\
         0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
    );

    create_test_magic_file(
        temp_dir.path(),
        "script.magic",
        "0 string \\x23\\x21 shell script\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(rules.len(), 4, "Should load all rules from all files");

    // Verify we got rules from all three files
    let messages: Vec<&str> = rules.iter().map(|r| r.message.as_str()).collect();
    assert!(messages.contains(&"ar archive"));
    assert!(messages.contains(&"ZIP archive"));
    assert!(messages.contains(&"ELF executable"));
    assert!(messages.contains(&"shell script"));
}

#[test]
fn test_load_directory_preserves_order() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create files with specific ordering - using valid magic syntax with hex escapes
    create_test_magic_file(
        temp_dir.path(),
        "01-first.magic",
        "0 string \\x01\\x02\\x03 first file\n",
    );

    create_test_magic_file(
        temp_dir.path(),
        "02-second.magic",
        "0 string \\x04\\x05\\x06 second file\n",
    );

    create_test_magic_file(
        temp_dir.path(),
        "03-third.magic",
        "0 string \\x07\\x08\\x09 third file\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(rules.len(), 3);
    // Files should be processed in alphabetical order
    assert_eq!(rules[0].message, "first file");
    assert_eq!(rules[1].message, "second file");
    assert_eq!(rules[2].message, "third file");
}

#[test]
fn test_load_directory_skips_subdirectories() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a file in the main directory
    create_test_magic_file(
        temp_dir.path(),
        "main.magic",
        "0 string \\x01\\x02 main file\n",
    );

    // Create a subdirectory with a magic file
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).expect("Failed to create subdirectory");
    create_test_magic_file(&subdir, "sub.magic", "0 string \\x03\\x04 sub file\n");

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    // Should only load the main file, not the one in subdirectory
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].message, "main file");
}

#[test]
#[cfg(unix)] // Symlink creation is platform-specific
fn test_load_directory_skips_symlinks() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a regular file
    let _regular_file = create_test_magic_file(
        temp_dir.path(),
        "regular.magic",
        "0 string \\x01\\x02 regular file\n",
    );

    // Create another file outside the directory
    let external_dir = TempDir::new().expect("Failed to create external temp dir");
    let external_file = create_test_magic_file(
        external_dir.path(),
        "external.magic",
        "0 string \\x03\\x04 external file\n",
    );

    // Create a symlink to the external file
    let symlink_path = temp_dir.path().join("symlink.magic");
    symlink(&external_file, &symlink_path).expect("Failed to create symlink");

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    // Should only load the regular file, not the symlinked one
    assert_eq!(rules.len(), 1, "Should skip symlinks");
    assert_eq!(rules[0].message, "regular file");
}

#[test]
fn test_load_directory_with_parse_errors() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a valid file
    create_test_magic_file(
        temp_dir.path(),
        "01-valid.magic",
        "0 string \\x01\\x02 valid file\n",
    );

    // Create an invalid file (malformed syntax)
    create_test_magic_file(
        temp_dir.path(),
        "02-invalid.magic",
        "this is not valid magic file syntax\n\
         completely broken\n",
    );

    // Create another valid file
    create_test_magic_file(
        temp_dir.path(),
        "03-valid.magic",
        "0 string \\x03\\x04 another valid file\n",
    );

    // Should succeed and load only the valid files
    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(
        rules.len(),
        2,
        "Should load only valid files, skipping invalid ones"
    );
    assert_eq!(rules[0].message, "valid file");
    assert_eq!(rules[1].message, "another valid file");
}

#[test]
fn test_load_directory_io_error() {
    let non_existent_path = Path::new("/this/path/should/not/exist/anywhere");

    let result = load_magic_directory(non_existent_path);

    assert!(
        result.is_err(),
        "Should return error for non-existent directory"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to read directory"),
        "Error should mention directory read failure"
    );
}

#[test]
fn test_load_directory_with_comments() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    create_test_magic_file(
        temp_dir.path(),
        "commented.magic",
        "# This is a comment\n\
         # Another comment\n\
         0 string \\x01\\x02 test file\n\
         # Inline comment\n\
         >4 byte 1 version 1\n\
         \n\
         # Empty lines above\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].message, "test file");
    assert_eq!(rules[0].children.len(), 1);
}

#[test]
fn test_load_directory_with_nested_rules() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    create_test_magic_file(
        temp_dir.path(),
        "nested.magic",
        "0 string \\x7fELF ELF executable\n\
         >4 byte 1 32-bit\n\
         >>16 short 2 executable\n\
         >>16 short 3 shared object\n\
         >4 byte 2 64-bit\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    assert_eq!(rules.len(), 1, "Should have one top-level rule");
    assert_eq!(rules[0].children.len(), 2, "Should have two child rules");

    // Check first child has nested children
    assert_eq!(
        rules[0].children[0].children.len(),
        2,
        "First child should have 2 nested children"
    );
}

#[test]
fn test_load_directory_rule_count() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    create_magdir_structure(temp_dir.path());

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    // Count total rules from create_magdir_structure:
    // 01-elf: 1 top-level (ELF executable) with 2 children = 1 top-level rule
    // 02-archive: 2 top-level (ar archive, ZIP archive) = 2 top-level rules
    // 03-text: 2 top-level (shell script, bash script) = 2 top-level rules
    // Total: 5 top-level rules
    assert_eq!(
        rules.len(),
        5,
        "Should have 5 top-level rules from Magdir structure"
    );
}

#[test]
fn test_load_directory_empty_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create an empty file
    create_test_magic_file(temp_dir.path(), "empty.magic", "");

    // Create a file with only whitespace
    create_test_magic_file(temp_dir.path(), "whitespace.magic", "   \n\n  \n");

    // Create a valid file
    create_test_magic_file(
        temp_dir.path(),
        "valid.magic",
        "0 string \\x01\\x02 valid file\n",
    );

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    // Empty files should be handled gracefully
    assert_eq!(
        rules.len(),
        1,
        "Should load only the valid file, empty files contribute no rules"
    );
    assert_eq!(rules[0].message, "valid file");
}

#[test]
fn test_load_directory_mixed_extensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Various file extensions
    create_test_magic_file(
        temp_dir.path(),
        "file.magic",
        "0 string \\x01\\x02 magic ext\n",
    );

    create_test_magic_file(temp_dir.path(), "file.txt", "0 string \\x03\\x04 txt ext\n");

    create_test_magic_file(temp_dir.path(), "noext", "0 string \\x05\\x06 no ext\n");

    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Failed to load directory");

    // All files should be processed regardless of extension
    assert_eq!(
        rules.len(),
        3,
        "Should process all files regardless of extension"
    );

    let messages: Vec<&str> = rules.iter().map(|r| r.message.as_str()).collect();
    assert!(messages.contains(&"magic ext"));
    assert!(messages.contains(&"txt ext"));
    assert!(messages.contains(&"no ext"));
}

#[test]
fn test_load_directory_all_files_fail_to_parse() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create files with invalid magic syntax
    create_test_magic_file(
        temp_dir.path(),
        "bad1",
        "this is not valid magic file syntax at all",
    );

    create_test_magic_file(temp_dir.path(), "bad2", "also invalid\nno proper format\n");

    // When all files fail to parse, we should get an error
    let result = load_magic_directory(temp_dir.path());

    assert!(
        result.is_err(),
        "Should return error when all files fail to parse"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("All") && err_msg.contains("failed to parse"),
        "Error message should indicate all files failed: {err_msg}"
    );
}

#[test]
fn test_load_directory_partial_failure_succeeds() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // One valid file
    create_test_magic_file(temp_dir.path(), "good", "0 string \\x00 valid rule\n");

    // One invalid file
    create_test_magic_file(temp_dir.path(), "bad", "not valid magic syntax");

    // Should succeed because at least one file parsed
    let ParsedMagic { rules, .. } =
        load_magic_directory(temp_dir.path()).expect("Should succeed with partial failure");

    assert_eq!(rules.len(), 1, "Should have one rule from the valid file");
    assert_eq!(rules[0].message, "valid rule");
}
