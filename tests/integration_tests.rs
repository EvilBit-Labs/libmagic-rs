// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end integration tests for core flows
//!
//! Tests the complete workflow: load database, evaluate files/buffers,
//! verify output formatting and metadata.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use libmagic_rs::{EvaluationConfig, MagicDatabase};
use tempfile::TempDir;

// ============================================================
// Built-in Rules End-to-End
// ============================================================

#[test]
fn test_builtin_rules_detect_elf() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(result.description.contains("ELF"));
    assert!(result.confidence > 0.0);
}

#[test]
fn test_builtin_rules_detect_pdf() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"%PDF-\x00\x00\x00").unwrap();
    assert!(result.description.contains("PDF"));
}

#[test]
fn test_builtin_rules_detect_zip() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"PK\x03\x04").unwrap();
    assert!(
        result.description.contains("ZIP") || result.description.contains("Zip"),
        "Expected ZIP detection, got: {}",
        result.description
    );
}

#[test]
fn test_builtin_rules_detect_png() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db
        .evaluate_buffer(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
        .unwrap();
    assert!(result.description.contains("PNG"));
}

#[test]
fn test_builtin_rules_detect_jpeg() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db
        .evaluate_buffer(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
        .unwrap();
    assert!(
        result.description.contains("JPEG") || result.description.contains("JFIF"),
        "Expected JPEG detection, got: {}",
        result.description
    );
}

#[test]
fn test_builtin_rules_detect_gif() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"GIF8\x00\x00").unwrap();
    assert!(
        result.description.contains("GIF"),
        "Expected GIF detection, got: {}",
        result.description
    );
}

#[test]
fn test_builtin_rules_unknown_fallback() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"random bytes").unwrap();
    assert_eq!(result.description, "data");
}

// ============================================================
// Load from File End-to-End
// ============================================================

#[test]
fn test_load_from_magic_file_and_evaluate() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("test.magic");

    // Write a simple magic file using quoted string value
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, "0 string \"TESTMAGIC\" Test file format").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    let result = db.evaluate_buffer(b"TESTMAGIC\x00some data").unwrap();
    assert!(
        result.description.contains("Test file format"),
        "Expected custom rule match, got: {}",
        result.description
    );
}

#[test]
fn test_load_from_file_with_config() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("test.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, "0 string \"HELLO\" Hello file").unwrap();

    let config = EvaluationConfig {
        enable_mime_types: true,
        ..EvaluationConfig::default()
    };
    let db = MagicDatabase::load_from_file_with_config(&magic_path, config).unwrap();
    let result = db.evaluate_buffer(b"HELLO\x00world").unwrap();
    assert!(result.description.contains("Hello file"));
}

#[test]
fn test_source_path_tracked() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("test.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, "0 string \"X\" Test").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    assert!(db.source_path().is_some());
}

#[test]
fn test_source_path_none_for_builtin() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    assert!(db.source_path().is_none());
}

#[test]
fn test_load_nonexistent_file_errors() {
    let result = MagicDatabase::load_from_file("/nonexistent/magic/file");
    assert!(result.is_err());
}

// ============================================================
// Evaluate File End-to-End
// ============================================================

#[test]
fn test_evaluate_file_with_builtin() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.elf");

    // Write ELF header
    fs::write(
        &test_file,
        b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00",
    )
    .unwrap();

    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_file(&test_file).unwrap();
    assert!(result.description.contains("ELF"));
    assert!(result.metadata.file_size > 0);
}

#[test]
fn test_evaluate_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty.bin");
    fs::write(&test_file, b"").unwrap();

    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_file(&test_file).unwrap();
    assert_eq!(result.metadata.file_size, 0);
}

#[test]
fn test_evaluate_nonexistent_file_errors() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_file("/nonexistent/test/file.bin");
    assert!(result.is_err());
}

// ============================================================
// Multiple Format Detection
// ============================================================

#[test]
fn test_multiple_formats_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();

    let test_cases: Vec<(&[u8], &str)> = vec![
        (b"\x7fELF\x02\x01\x01\x00", "ELF"),
        (b"%PDF-\x00\x00\x00", "PDF"),
        (b"PK\x03\x04rest", "ZIP"),
        (b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR", "PNG"),
        (b"GIF8\x00\x00", "GIF"),
    ];

    for (input, expected) in test_cases {
        let result = db.evaluate_buffer(input).unwrap();
        assert!(
            result.description.contains(expected),
            "Expected '{}' for input {:?}, got: '{}'",
            expected,
            &input[..4.min(input.len())],
            result.description
        );
    }
}

// ============================================================
// Custom Magic Rules End-to-End
// ============================================================

#[test]
fn test_custom_rules_with_hierarchy() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("custom.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, "0 belong 0x7f454c46 ELF").unwrap();
    writeln!(f, ">4 byte 1 32-bit").unwrap();
    writeln!(f, ">4 byte 2 64-bit").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();

    // Test 32-bit ELF (5 bytes: 4-byte magic + class byte)
    let result = db.evaluate_buffer(b"\x7fELF\x01").unwrap();
    assert!(
        result.description.contains("32-bit"),
        "Expected 32-bit, got: {}",
        result.description
    );

    // Test 64-bit ELF
    let result = db.evaluate_buffer(b"\x7fELF\x02").unwrap();
    assert!(
        result.description.contains("64-bit"),
        "Expected 64-bit, got: {}",
        result.description
    );
}

#[test]
fn test_custom_rules_no_match_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("custom.magic");

    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, "0 string \"RARE\" Rare format").unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    let result = db.evaluate_buffer(b"no match here").unwrap();
    assert_eq!(result.description, "data");
}

// ============================================================
// Load from Directory End-to-End
// ============================================================

#[test]
fn test_load_directory_of_magic_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple magic files using belong for binary magic numbers
    let mut f1 = fs::File::create(temp_dir.path().join("images.magic")).unwrap();
    writeln!(f1, "0 belong 0x89504e47 PNG image").unwrap();

    let mut f2 = fs::File::create(temp_dir.path().join("docs.magic")).unwrap();
    writeln!(f2, "0 string \"%PDF-\" PDF document").unwrap();

    let db = MagicDatabase::load_from_file(temp_dir.path()).unwrap();

    let png_result = db
        .evaluate_buffer(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
        .unwrap();
    assert!(
        png_result.description.contains("PNG"),
        "Expected PNG from directory load, got: {}",
        png_result.description
    );

    let pdf_result = db.evaluate_buffer(b"%PDF-\x001.4").unwrap();
    assert!(
        pdf_result.description.contains("PDF"),
        "Expected PDF from directory load, got: {}",
        pdf_result.description
    );
}

// ============================================================
// Config Accessor
// ============================================================

#[test]
fn test_config_accessor() {
    let config = EvaluationConfig {
        max_recursion_depth: 15,
        timeout_ms: Some(5000),
        ..EvaluationConfig::default()
    };
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    assert_eq!(db.config().max_recursion_depth, 15);
    assert_eq!(db.config().timeout_ms, Some(5000));
}

// ============================================================
// Metadata in File Evaluation
// ============================================================

#[test]
fn test_file_evaluation_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.pdf");
    let content = b"%PDF-\x00some pdf content here for testing";
    fs::write(&test_file, content).unwrap();

    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_file(&test_file).unwrap();

    assert_eq!(result.metadata.file_size, content.len() as u64);
    assert!(result.metadata.evaluation_time_ms >= 0.0);
    assert!(result.metadata.rules_evaluated > 0);
    assert!(!result.metadata.timed_out);
}

// ============================================================
// Multiple Evaluations on Same Database
// ============================================================

#[test]
fn test_reuse_database_multiple_evaluations() {
    let db = MagicDatabase::with_builtin_rules().unwrap();

    let elf = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    let pdf = db.evaluate_buffer(b"%PDF-\x001.4").unwrap();
    let unknown = db.evaluate_buffer(b"nothing here").unwrap();

    assert!(elf.description.contains("ELF"));
    assert!(pdf.description.contains("PDF"));
    assert_eq!(unknown.description, "data");
}

// ============================================================
// Test Fixture Helper
// ============================================================

fn create_test_file(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_evaluate_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let db = MagicDatabase::with_builtin_rules().unwrap();

    let elf_file = create_test_file(
        temp_dir.path(),
        "test.elf",
        b"\x7fELF\x02\x01\x01\x00\x00\x00",
    );
    let pdf_file = create_test_file(temp_dir.path(), "test.pdf", b"%PDF-\x001.4 content");

    let elf_result = db.evaluate_file(&elf_file).unwrap();
    let pdf_result = db.evaluate_file(&pdf_file).unwrap();

    assert!(elf_result.description.contains("ELF"));
    assert!(pdf_result.description.contains("PDF"));
}
