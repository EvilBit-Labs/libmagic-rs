// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for loading text magic rules from readers and owned bytes.

#![allow(clippy::unwrap_used)]

use std::io::{Cursor, Read};

use libmagic_rs::{EvaluationConfig, LibmagicError, MagicDatabase, ParseError};

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("reader failed"))
    }
}

#[test]
fn test_load_from_bytes_and_evaluate() {
    let rules = b"0 string OWNED Owned-byte file\n".to_vec();
    let db = MagicDatabase::load_from_bytes(rules).unwrap();

    let result = db.evaluate_buffer(b"OWNED payload").unwrap();

    assert!(result.description.contains("Owned-byte file"));
    assert!(db.source_path().is_none());
}

#[test]
fn test_load_from_bytes_with_config() {
    let rules = b"0 string OWNED Owned-byte file\n".to_vec();
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::load_from_bytes_with_config(rules, config).unwrap();

    assert!(db.config().enable_mime_types);
}

#[test]
fn test_load_from_bytes_rejects_binary_mgc() {
    let headers = [
        ("little-endian", 0xF11E_041Cu32.to_le_bytes()),
        ("big-endian", 0xF11E_041Cu32.to_be_bytes()),
    ];

    for (byte_order, header) in headers {
        let error = MagicDatabase::load_from_bytes(header.to_vec()).unwrap_err();

        assert!(
            matches!(
                error,
                LibmagicError::ParseError(ParseError::UnsupportedFormat { .. })
            ),
            "expected {byte_order} .mgc input to be rejected"
        );
    }
}

#[test]
fn test_load_from_reader_and_evaluate() {
    let rules = b"0 string READER Reader-backed file\n";
    let db = MagicDatabase::load_from_reader(Cursor::new(rules)).unwrap();

    let result = db.evaluate_buffer(b"READER payload").unwrap();

    assert!(result.description.contains("Reader-backed file"));
    assert!(db.source_path().is_none());
}

#[test]
fn test_load_from_reader_with_config() {
    let rules = b"0 string READER Reader-backed file\n";
    let config = EvaluationConfig::default().with_mime_types(true);
    let db = MagicDatabase::load_from_reader_with_config(rules.as_slice(), config).unwrap();

    assert!(db.config().enable_mime_types);
}

#[test]
fn test_load_from_reader_rejects_binary_mgc() {
    let headers = [
        ("little-endian", 0xF11E_041Cu32.to_le_bytes()),
        ("big-endian", 0xF11E_041Cu32.to_be_bytes()),
    ];

    for (byte_order, header) in headers {
        let reader = Cursor::new(header).chain(FailingReader);
        let error = MagicDatabase::load_from_reader(reader).unwrap_err();

        assert!(
            matches!(
                error,
                LibmagicError::ParseError(ParseError::UnsupportedFormat { .. })
            ),
            "expected {byte_order} .mgc input to be rejected"
        );
    }
}

#[test]
fn test_load_from_reader_preserves_io_errors() {
    let error = MagicDatabase::load_from_reader(FailingReader).unwrap_err();

    assert!(matches!(error, LibmagicError::IoError(_)));
}
