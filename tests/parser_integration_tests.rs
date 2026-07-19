// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end integration tests for magic file parser and database integration.
//!
//! These tests validate the complete flow from file/directory loading through
//! rule evaluation, ensuring all components work together correctly.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(clippy::expect_used, clippy::create_dir)]

use libmagic_rs::MagicDatabase;
use libmagic_rs::parser::{ParsedMagic, load_magic_file};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ============================================================
// Test Helper Functions
// ============================================================

/// Creates a test magic file with the given content in the specified directory.
fn create_test_magic_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let file_path = dir.join(name);
    let mut file = fs::File::create(&file_path).expect("Failed to create test magic file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test magic file");
    file_path
}

/// Creates a test binary file with the given magic bytes.
fn create_test_binary_file(dir: &Path, name: &str, magic_bytes: &[u8]) -> PathBuf {
    let file_path = dir.join(name);
    let mut file = fs::File::create(&file_path).expect("Failed to create test binary file");
    file.write_all(magic_bytes)
        .expect("Failed to write test binary file");
    file_path
}

/// Creates a test file with ELF magic bytes.
fn create_elf_test_file(dir: &Path) -> PathBuf {
    create_test_binary_file(dir, "test.elf", b"\x7fELF\x02\x01\x01\x00")
}

/// Creates a test file with ZIP magic bytes.
fn create_zip_test_file(dir: &Path) -> PathBuf {
    create_test_binary_file(dir, "test.zip", b"PK\x03\x04")
}

/// Creates a test file with PDF magic bytes.
fn create_pdf_test_file(dir: &Path) -> PathBuf {
    create_test_binary_file(dir, "test.pdf", b"%PDF-1.4")
}

// ============================================================
// Tests for load_magic_file() Function
// ============================================================

#[test]
fn test_load_text_magic_file_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_content = "\
# Test magic file
0 string \\x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
0 string \\x50\\x4b\\x03\\x04 ZIP archive
";
    let magic_file = create_test_magic_file(temp_dir.path(), "magic", magic_content);

    let ParsedMagic { rules, .. } =
        load_magic_file(&magic_file).expect("Failed to load magic file");

    // Verify rules loaded correctly - should have 2 top-level rules
    assert_eq!(rules.len(), 2, "Should have 2 top-level rules");

    // Check first rule (ELF) and its children
    assert_eq!(rules[0].level, 0);
    assert_eq!(rules[0].message, "ELF executable");
    assert_eq!(
        rules[0].children.len(),
        2,
        "ELF rule should have 2 children"
    );
    assert_eq!(rules[0].children[0].message, "32-bit");
    assert_eq!(rules[0].children[1].message, "64-bit");

    // Check second top-level rule (ZIP)
    assert_eq!(rules[1].level, 0);
    assert_eq!(rules[1].message, "ZIP archive");
    assert_eq!(
        rules[1].children.len(),
        0,
        "ZIP rule should have no children"
    );
}

#[test]
fn test_load_directory_magic_file_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create magic directory with multiple files
    let magic_dir = temp_dir.path().join("magic.d");
    fs::create_dir(&magic_dir).expect("Failed to create magic directory");

    // Create multiple magic files (should be loaded alphabetically)
    create_test_magic_file(&magic_dir, "00_elf", "0 string \\x7fELF ELF executable\n");
    create_test_magic_file(
        &magic_dir,
        "01_zip",
        "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
    );
    create_test_magic_file(&magic_dir, "02_pdf", "0 string \\x25PDF- PDF document\n");

    let ParsedMagic { rules, .. } = load_magic_file(&magic_dir).expect("Failed to load directory");

    // Verify all files merged correctly in alphabetical order
    assert_eq!(rules.len(), 3, "Should have 3 rules from 3 files");
    assert_eq!(rules[0].message, "ELF executable");
    assert_eq!(rules[1].message, "ZIP archive");
    assert_eq!(rules[2].message, "PDF document");
}

#[test]
fn test_load_binary_magic_file_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a binary .mgc file with magic number
    let binary_magic_number: [u8; 4] = [0x1C, 0x04, 0x1E, 0xF1]; // Little-endian 0xF11E041C
    let mgc_file = create_test_binary_file(temp_dir.path(), "magic.mgc", &binary_magic_number);

    let result = load_magic_file(&mgc_file);

    // Should return UnsupportedFormat error
    assert!(result.is_err(), "Should fail to load binary magic file");

    let error = result.unwrap_err();
    let error_message = error.to_string();

    // Verify error message contains --use-builtin guidance
    assert!(
        error_message.contains("--use-builtin") || error_message.contains("Binary"),
        "Error message should mention binary format or --use-builtin option: {error_message}"
    );
}

#[test]
fn test_load_nonexistent_file_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nonexistent_file = temp_dir.path().join("does_not_exist.magic");

    let result = load_magic_file(&nonexistent_file);

    // Should return error for nonexistent file
    assert!(result.is_err(), "Should fail to load nonexistent file");
}

#[test]
fn test_load_empty_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let empty_dir = temp_dir.path().join("empty_magic.d");
    fs::create_dir(&empty_dir).expect("Failed to create empty directory");

    let ParsedMagic { rules, .. } =
        load_magic_file(&empty_dir).expect("Failed to load empty directory");

    // Should return empty rules vector (not error)
    assert_eq!(rules.len(), 0, "Empty directory should return empty rules");
}

// ============================================================
// Tests for name/use subroutine round-trip
// ============================================================

#[test]
fn test_name_use_round_trip() {
    use libmagic_rs::parser::ast::{MetaType, TypeKind};

    // A `name` declaration + a `use` invocation at the top level. The
    // name rule should be hoisted into the name table; the use rule
    // should survive in the rules list. Evaluating the file against a
    // matching buffer should surface the subroutine's message.
    let magic = "\
0 name part2
>3 byte 0x42 sub-match

0 use part2
";
    let parsed = libmagic_rs::parser::parse_text_magic_file(magic).expect("parse meta round-trip");

    // The name rule should be hoisted; only the `use` remains at the top.
    assert_eq!(parsed.rules.len(), 1, "name rule must be hoisted out");
    assert!(
        matches!(
            parsed.rules[0].typ,
            TypeKind::Meta(MetaType::Use(ref n)) if n == "part2"
        ),
        "remaining top-level rule must be the use invocation"
    );

    // End-to-end evaluation via MagicDatabase.
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_test_magic_file(temp_dir.path(), "meta.magic", magic);
    let db = MagicDatabase::load_from_file(&magic_file)
        .expect("load meta-type magic file into MagicDatabase");

    let buffer = b"\x00\x00\x00\x42\x00";
    let result = db.evaluate_buffer(buffer).expect("evaluate meta buffer");
    assert!(
        result.description.contains("sub-match"),
        "description should contain subroutine message, got '{}'",
        result.description
    );
}

// ============================================================
// Tests for MagicDatabase Integration
// ============================================================

#[test]
fn test_magic_database_load_text_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_content = "0 string \\x7fELF ELF executable\n";
    let magic_file = create_test_magic_file(temp_dir.path(), "magic", magic_content);

    let db =
        MagicDatabase::load_from_file(&magic_file).expect("Failed to load database from text file");

    // Verify database contains rules
    // Note: We can't directly inspect rules as they're private, but we can check source_path
    assert!(
        db.source_path().is_some(),
        "Database should have source path"
    );
    assert_eq!(
        db.source_path().unwrap(),
        magic_file.as_path(),
        "Source path should match loaded file"
    );
}

#[test]
fn test_magic_database_load_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_dir = temp_dir.path().join("magic.d");
    fs::create_dir(&magic_dir).expect("Failed to create magic directory");

    create_test_magic_file(&magic_dir, "elf", "0 string \\x7fELF ELF executable\n");
    create_test_magic_file(
        &magic_dir,
        "zip",
        "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
    );

    let db =
        MagicDatabase::load_from_file(&magic_dir).expect("Failed to load database from directory");

    // Verify source path stored correctly
    assert_eq!(
        db.source_path().unwrap(),
        magic_dir.as_path(),
        "Source path should match loaded directory"
    );
}

#[test]
#[ignore = "Parser does not decode \\xNN escape sequences inside string values yet; rule matches 'data' instead of 'ELF'. Re-enable once grammar supports hex escapes in parse_value()."]
fn test_magic_database_evaluate_after_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create magic file with ELF detection rule
    let magic_content = "0 string \\x7fELF ELF executable\n";
    let magic_file = create_test_magic_file(temp_dir.path(), "magic", magic_content);

    // Create test file with ELF magic bytes
    let elf_file = create_elf_test_file(temp_dir.path());

    // Load database and evaluate
    let db = MagicDatabase::load_from_file(&magic_file).expect("Failed to load database");
    let result = db
        .evaluate_file(&elf_file)
        .expect("Failed to evaluate file");

    // Verify correct rule evaluation
    assert!(
        result.description.contains("ELF"),
        "Should detect ELF file, got: {}",
        result.description
    );
}

#[test]
fn test_magic_database_source_path_metadata() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_content = "0 string \\x48\\x65\\x6c\\x6c\\x6f Hello file\n";
    let magic_file = create_test_magic_file(temp_dir.path(), "magic", magic_content);

    let db = MagicDatabase::load_from_file(&magic_file).expect("Failed to load database");

    // Verify source_path metadata is preserved
    assert!(db.source_path().is_some());
    assert_eq!(db.source_path().unwrap(), magic_file.as_path());

    // Verify path persists across operations (evaluate_file doesn't clear it)
    let test_file = create_test_binary_file(temp_dir.path(), "test.bin", b"test data");
    let _result = db.evaluate_file(&test_file);

    assert!(
        db.source_path().is_some(),
        "Source path should persist after evaluation"
    );
}

#[test]
fn test_binary_format_error_message_quality() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create binary .mgc file
    let binary_magic_number: [u8; 4] = [0x1C, 0x04, 0x1E, 0xF1];
    let mgc_file = create_test_binary_file(temp_dir.path(), "magic.mgc", &binary_magic_number);

    let result = MagicDatabase::load_from_file(&mgc_file);

    assert!(result.is_err(), "Should fail to load binary file");

    let error = result.unwrap_err();
    let error_message = error.to_string();

    // Verify error message is user-friendly and actionable
    assert!(
        error_message.contains("Binary") || error_message.contains("binary"),
        "Error should mention binary format: {error_message}"
    );

    // Should suggest using built-in rules
    assert!(
        error_message.contains("--use-builtin") || error_message.contains("built-in"),
        "Error should suggest --use-builtin option: {error_message}"
    );
}

// ============================================================
// End-to-End Integration Tests
// ============================================================

#[test]
#[ignore = "Parser does not decode \\xNN escape sequences inside string values yet; rule matches 'data' instead of 'ZIP'. Re-enable once grammar supports hex escapes in parse_value()."]
fn test_end_to_end_text_file_to_evaluation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create magic file with ZIP detection
    let magic_content = "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n";
    let magic_file = create_test_magic_file(temp_dir.path(), "magic", magic_content);

    // Create test file with ZIP magic bytes
    let zip_file = create_zip_test_file(temp_dir.path());

    // Complete workflow: load → evaluate → output
    let db = MagicDatabase::load_from_file(&magic_file).expect("Failed to load database");
    let result = db
        .evaluate_file(&zip_file)
        .expect("Failed to evaluate file");

    // Verify correct rule evaluation
    assert!(
        result.description.contains("ZIP"),
        "Should detect ZIP archive, got: {}",
        result.description
    );
}

/// End-to-end proof that the string `>NUMERIC` parse fix (task #19) and the
/// string ordering-op full-field render / prefix-limited compare (task #18)
/// compose through real magic-file syntax.
///
/// `0 string >0.6.1 version %s` is parsed straight from text (so the value
/// must survive as `Value::String("0.6.1")`, not a number), then evaluated:
/// - `0.6.2` matches (`0.6.2` > `0.6.1`) and renders the FULL field
///   (`version 0.6.2 release`), exercising the #18 full-field display read.
/// - `0.6.10` does NOT match: the comparison is prefix-limited to
///   `pattern.len()`, so the compared prefix `0.6.1` equals the pattern and
///   `>` is false -- matching real `file` (file-5.41). It falls through to
///   the ascmagic text fallback instead of a spurious `version ...`.
#[test]
fn test_string_numeric_ordering_end_to_end_composes_with_full_field_render() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_test_magic_file(temp_dir.path(), "ver", "0 string >0.6.1 version %s\n");
    let db = MagicDatabase::load_from_file(&magic_file).expect("Failed to load database");

    // 0.6.2 > 0.6.1 -> matches, renders the full string field.
    let match_file = create_test_binary_file(temp_dir.path(), "v62", b"0.6.2 release");
    let matched = db
        .evaluate_file(&match_file)
        .expect("Failed to evaluate matching file");
    assert_eq!(
        matched.description, "version 0.6.2 release",
        "0.6.2 must match >0.6.1 and render the full field, got: {}",
        matched.description
    );

    // 0.6.10: compared prefix `0.6.1` == pattern, so `>` is false. No match.
    let nomatch_file = create_test_binary_file(temp_dir.path(), "v610", b"0.6.10 release");
    let unmatched = db
        .evaluate_file(&nomatch_file)
        .expect("Failed to evaluate non-matching file");
    assert!(
        !unmatched.description.contains("version"),
        "0.6.10 must NOT match >0.6.1 (prefix-limited compare), got: {}",
        unmatched.description
    );
}

/// GOTCHAS S6.7: a top-level `string` signature whose bareword value carries
/// a non-UTF-8 high byte (OS/2 INF's `HSP\x01\x9b\x00`, where `0x9b` is
/// invalid UTF-8) must match. Before the fix, `parse_bare_string_value`
/// lossy-decoded the value to a `Value::String` (0x9b -> U+FFFD), which both
/// inflated the pattern length (6 -> 8) and changed the byte, so the rule
/// silently never matched and the file classified as `data`. Real `file`
/// prints `OS/2 INF (My Help File)`; this pins the full output including the
/// `>107 string >0 (%s)` title child.
#[test]
fn os2_inf_high_byte_signature_matches() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = create_test_magic_file(
        temp_dir.path(),
        "os2",
        "0 string HSP\\x01\\x9b\\x00 OS/2 INF\n>107 string >0 (%s)\n",
    );
    let db = MagicDatabase::load_from_file(&magic_file).expect("Failed to load database");

    // Signature (6 bytes) + filler to offset 107 + NUL-terminated title.
    let mut bytes = vec![0x48, 0x53, 0x50, 0x01, 0x9b, 0x00];
    bytes.resize(107, 0x00);
    bytes.extend_from_slice(b"My Help File\x00");
    let file = create_test_binary_file(temp_dir.path(), "os2inf.bin", &bytes);

    let result = db
        .evaluate_file(&file)
        .expect("Failed to evaluate OS/2 INF file");
    assert_eq!(
        result.description, "OS/2 INF (My Help File)",
        "high-byte signature must match and render the title child, got: {}",
        result.description
    );
}

#[test]
#[ignore = "Parser does not decode \\xNN escape sequences inside string values yet; rules match 'data' instead of the expected magic type. Re-enable once grammar supports hex escapes in parse_value()."]
fn test_end_to_end_directory_to_evaluation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create directory with multiple magic files
    let magic_dir = temp_dir.path().join("magic.d");
    fs::create_dir(&magic_dir).expect("Failed to create magic directory");

    create_test_magic_file(&magic_dir, "elf", "0 string \\x7fELF ELF executable\n");
    create_test_magic_file(
        &magic_dir,
        "zip",
        "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
    );
    create_test_magic_file(&magic_dir, "pdf", "0 string \\x25PDF- PDF document\n");

    // Create test files for each format
    let elf_file = create_elf_test_file(temp_dir.path());
    let zip_file = create_zip_test_file(temp_dir.path());
    let pdf_file = create_pdf_test_file(temp_dir.path());

    // Load database from directory
    let db =
        MagicDatabase::load_from_file(&magic_dir).expect("Failed to load database from directory");

    // Evaluate each file and verify correct detection
    let elf_result = db
        .evaluate_file(&elf_file)
        .expect("Failed to evaluate ELF file");
    assert!(
        elf_result.description.contains("ELF"),
        "Should detect ELF executable, got: {}",
        elf_result.description
    );

    let zip_result = db
        .evaluate_file(&zip_file)
        .expect("Failed to evaluate ZIP file");
    assert!(
        zip_result.description.contains("ZIP"),
        "Should detect ZIP archive, got: {}",
        zip_result.description
    );

    let pdf_result = db
        .evaluate_file(&pdf_file)
        .expect("Failed to evaluate PDF file");
    assert!(
        pdf_result.description.contains("PDF"),
        "Should detect PDF document, got: {}",
        pdf_result.description
    );
}
