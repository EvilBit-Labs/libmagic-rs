// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! MIME type mapping integration tests
//!
//! Tests for MIME type detection through the public API,
//! including hardcoded mappings, prefix matching, and case insensitivity.

use libmagic_rs::mime::MimeMapper;
use libmagic_rs::{EvaluationConfig, MagicDatabase};

// ============================================================
// Integration: MIME via MagicDatabase
// ============================================================

#[test]
fn test_mime_enabled_returns_type_for_elf() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert_eq!(
        result.mime_type.as_deref(),
        Some("application/x-executable")
    );
}

#[test]
fn test_mime_enabled_returns_type_for_pdf() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"%PDF-\x00\x00\x00").unwrap();
    assert_eq!(result.mime_type.as_deref(), Some("application/pdf"));
}

#[test]
fn test_mime_enabled_returns_type_for_png() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db
        .evaluate_buffer(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
        .unwrap();
    assert_eq!(result.mime_type.as_deref(), Some("image/png"));
}

#[test]
fn test_mime_enabled_returns_type_for_jpeg() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db
        .evaluate_buffer(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
        .unwrap();
    assert_eq!(result.mime_type.as_deref(), Some("image/jpeg"));
}

#[test]
fn test_mime_enabled_returns_type_for_zip() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"PK\x03\x04rest of zip").unwrap();
    assert_eq!(result.mime_type.as_deref(), Some("application/zip"));
}

#[test]
fn test_mime_disabled_returns_none() {
    let config = EvaluationConfig::default().with_mime_types(false);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00").unwrap();
    assert_eq!(result.mime_type, None);
}

#[test]
fn test_mime_unknown_data_returns_none() {
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::with_builtin_rules_and_config(config).unwrap();
    let result = db.evaluate_buffer(b"random unknown content").unwrap();
    assert_eq!(result.mime_type, None);
}

// ============================================================
// MimeMapper Direct Tests
// ============================================================

#[test]
fn test_mapper_hardcoded_executables() {
    let mapper = MimeMapper::new();
    assert_eq!(
        mapper.get_mime_type("ELF 64-bit LSB executable"),
        Some("application/x-executable")
    );
    assert_eq!(
        mapper.get_mime_type("PE32 executable (DLL)"),
        Some("application/vnd.microsoft.portable-executable")
    );
    assert_eq!(
        mapper.get_mime_type("Mach-O 64-bit executable"),
        Some("application/x-mach-binary")
    );
}

#[test]
fn test_mapper_hardcoded_archives() {
    let mapper = MimeMapper::new();
    assert_eq!(
        mapper.get_mime_type("Zip archive data"),
        Some("application/zip")
    );
    assert_eq!(
        mapper.get_mime_type("gzip compressed data"),
        Some("application/gzip")
    );
    assert_eq!(
        mapper.get_mime_type("POSIX tar archive"),
        Some("application/x-tar")
    );
    assert_eq!(
        mapper.get_mime_type("RAR archive data"),
        Some("application/vnd.rar")
    );
    assert_eq!(
        mapper.get_mime_type("7-zip archive"),
        Some("application/x-7z-compressed")
    );
}

#[test]
fn test_mapper_hardcoded_images() {
    let mapper = MimeMapper::new();
    assert_eq!(mapper.get_mime_type("JPEG image data"), Some("image/jpeg"));
    assert_eq!(
        mapper.get_mime_type("PNG image data, 800x600"),
        Some("image/png")
    );
    assert_eq!(
        mapper.get_mime_type("GIF image data, version 89a"),
        Some("image/gif")
    );
    assert_eq!(mapper.get_mime_type("BMP image data"), Some("image/bmp"));
    assert_eq!(mapper.get_mime_type("WebP image data"), Some("image/webp"));
}

#[test]
fn test_mapper_hardcoded_documents() {
    let mapper = MimeMapper::new();
    assert_eq!(
        mapper.get_mime_type("PDF document, version 1.4"),
        Some("application/pdf")
    );
    assert_eq!(
        mapper.get_mime_type("PostScript document"),
        Some("application/postscript")
    );
}

#[test]
fn test_mapper_hardcoded_web() {
    let mapper = MimeMapper::new();
    assert_eq!(mapper.get_mime_type("HTML document"), Some("text/html"));
    assert_eq!(
        mapper.get_mime_type("XML document"),
        Some("application/xml")
    );
    assert_eq!(mapper.get_mime_type("JSON data"), Some("application/json"));
}

#[test]
fn test_mapper_case_insensitive() {
    let mapper = MimeMapper::new();
    assert_eq!(
        mapper.get_mime_type("elf executable"),
        mapper.get_mime_type("ELF EXECUTABLE")
    );
    assert_eq!(
        mapper.get_mime_type("png image"),
        mapper.get_mime_type("PNG IMAGE")
    );
}

#[test]
fn test_mapper_prefers_longer_match() {
    let mapper = MimeMapper::new();
    // "gzip" should match before "zip" for "gzip compressed"
    assert_eq!(
        mapper.get_mime_type("gzip compressed data"),
        Some("application/gzip")
    );
}

#[test]
fn test_mapper_no_match_returns_none() {
    let mapper = MimeMapper::new();
    assert_eq!(mapper.get_mime_type("data"), None);
    assert_eq!(mapper.get_mime_type("unknown binary format"), None);
}
