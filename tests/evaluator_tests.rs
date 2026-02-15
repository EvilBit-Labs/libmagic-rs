// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Evaluator integration tests
//!
//! Tests for confidence calculation, rule ordering, and evaluation behavior
//! through the public `MagicDatabase` API.

use libmagic_rs::{EvaluationConfig, MagicDatabase};

// ============================================================
// Confidence Calculation Tests
// ============================================================

#[test]
fn test_confidence_nonzero_for_known_type() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(
        result.confidence > 0.0,
        "ELF detection should have non-zero confidence, got {}",
        result.confidence
    );
}

#[test]
fn test_confidence_zero_for_unknown_type() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"random unknown content").unwrap();
    assert!(
        (result.confidence - 0.0).abs() < f64::EPSILON,
        "Unknown type should have zero confidence"
    );
}

#[test]
fn test_confidence_matches_first_match() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    if let Some(first) = result.matches.first() {
        assert!(
            (result.confidence - first.confidence).abs() < f64::EPSILON,
            "Result confidence should equal first match confidence"
        );
    }
}

// ============================================================
// Rule Ordering Tests
// ============================================================

#[test]
fn test_elf_detected_before_generic() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(
        result.description.contains("ELF"),
        "ELF should be detected, got: {}",
        result.description
    );
}

#[test]
fn test_pdf_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"%PDF-\x00\x00\x00").unwrap();
    assert!(
        result.description.contains("PDF"),
        "PDF should be detected, got: {}",
        result.description
    );
}

#[test]
fn test_png_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db
        .evaluate_buffer(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
        .unwrap();
    assert!(
        result.description.contains("PNG"),
        "PNG should be detected, got: {}",
        result.description
    );
}

#[test]
fn test_jpeg_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db
        .evaluate_buffer(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
        .unwrap();
    assert!(
        result.description.contains("JPEG") || result.description.contains("JFIF"),
        "JPEG should be detected, got: {}",
        result.description
    );
}

#[test]
fn test_zip_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"PK\x03\x04rest of zip").unwrap();
    assert!(
        result.description.contains("ZIP") || result.description.contains("Zip"),
        "ZIP should be detected, got: {}",
        result.description
    );
}

#[test]
fn test_gzip_detected_correctly() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db
        .evaluate_buffer(b"\x1f\x8b\x08\x00\x00\x00\x00\x00")
        .unwrap();
    assert!(
        result.description.to_lowercase().contains("gzip"),
        "GZIP should be detected, got: {}",
        result.description
    );
}

// ============================================================
// Configuration Tests
// ============================================================

#[test]
fn test_evaluate_with_performance_config() {
    let config = EvaluationConfig::performance();
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(result.description.contains("ELF"));
}

#[test]
fn test_evaluate_with_comprehensive_config() {
    let config = EvaluationConfig::comprehensive();
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(result.description.contains("ELF"));
}

#[test]
fn test_evaluate_with_mime_types_enabled() {
    let config = EvaluationConfig {
        enable_mime_types: true,
        ..EvaluationConfig::default()
    };
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(
        result.mime_type.is_some(),
        "MIME type should be present when enabled"
    );
}

#[test]
fn test_evaluate_without_mime_types() {
    let config = EvaluationConfig {
        enable_mime_types: false,
        ..EvaluationConfig::default()
    };
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert!(
        result.mime_type.is_none(),
        "MIME type should be absent when disabled"
    );
}

#[test]
fn test_invalid_config_rejected() {
    let config = EvaluationConfig {
        max_recursion_depth: 0,
        ..EvaluationConfig::default()
    };
    let result = MagicDatabase::with_builtin_rules_and_config(config);
    assert!(result.is_err(), "Zero recursion depth should be rejected");
}

// ============================================================
// Metadata Tests
// ============================================================

#[test]
fn test_metadata_populated_for_buffer() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();

    assert_eq!(result.metadata.file_size, 8);
    assert!(result.metadata.evaluation_time_ms >= 0.0);
    assert!(result.metadata.rules_evaluated > 0);
    assert!(result.metadata.magic_file.is_none());
    assert!(!result.metadata.timed_out);
}

#[test]
fn test_metadata_for_no_match() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"nothing matches this").unwrap();

    assert_eq!(result.description, "data");
    assert!(result.metadata.rules_evaluated > 0);
}

// ============================================================
// Edge Cases
// ============================================================

#[test]
fn test_evaluate_empty_buffer() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"").unwrap();
    assert_eq!(result.description, "data");
}

#[test]
fn test_evaluate_single_byte_buffer() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(b"\x00").unwrap();
    // Should not panic, may or may not match
    assert!(!result.description.is_empty());
}

#[test]
fn test_evaluate_all_zeros() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(&[0u8; 1024]).unwrap();
    assert!(!result.description.is_empty());
}

#[test]
fn test_evaluate_all_ones() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    let result = db.evaluate_buffer(&[0xFF; 1024]).unwrap();
    assert!(!result.description.is_empty());
}

#[test]
fn test_evaluate_partial_magic_header() {
    let db = MagicDatabase::with_builtin_rules().unwrap();
    // Only first byte of ELF magic
    let result = db.evaluate_buffer(b"\x7f").unwrap();
    // Should not crash, might not match
    assert!(!result.description.is_empty());
}
