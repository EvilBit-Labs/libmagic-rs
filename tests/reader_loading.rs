// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for loading text magic rules from readers and owned bytes.

#![allow(clippy::unwrap_used)]

use std::io::{Cursor, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

// ============================================================
// Additional coverage: readers with awkward behavior, lossy
// decoding, tolerant parsing, and config-validation ordering.
// ============================================================

const RULES: &[u8] = b"0 string OWNED Owned-byte file\n";

/// Reader that records whether it was ever read, so a test can prove a code
/// path returns before touching the caller's stream.
struct TrackingReader {
    was_read: Arc<AtomicBool>,
}

impl Read for TrackingReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.was_read.store(true, Ordering::SeqCst);
        let _ = buffer;
        Ok(0)
    }
}

/// Reader that yields at most one byte per `read` call, exercising the
/// multi-call accumulation `read_to_end` performs for the 4-byte `.mgc` sniff.
struct DribbleReader {
    data: Vec<u8>,
    position: usize,
}

impl DribbleReader {
    fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            position: 0,
        }
    }
}

impl Read for DribbleReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let (Some(next), Some(slot)) = (self.data.get(self.position), buffer.first_mut()) else {
            return Ok(0);
        };
        *slot = *next;
        self.position += 1;
        Ok(1)
    }
}

/// A reader handing back one byte at a time must still assemble the 4-byte
/// signature -- the sniff relies on `read_to_end` looping, not on a single read.
#[test]
fn test_binary_mgc_detected_through_one_byte_reads() {
    for signature in [0xF11E_041Cu32.to_le_bytes(), 0xF11E_041Cu32.to_be_bytes()] {
        let mut payload = signature.to_vec();
        payload.extend_from_slice(b"trailing compiled data");

        let error = MagicDatabase::load_from_reader(DribbleReader::new(&payload)).unwrap_err();

        assert!(
            matches!(
                error,
                LibmagicError::ParseError(ParseError::UnsupportedFormat { .. })
            ),
            "one-byte reads missed signature {signature:02x?}"
        );
    }
}

#[test]
fn test_empty_and_short_input_loads_without_panicking() {
    // Fewer than four bytes means the signature check runs on a short buffer.
    for payload in [&b""[..], &b"0"[..], &b"0 "[..], &b"0 s"[..]] {
        assert!(
            MagicDatabase::load_from_bytes(payload.to_vec()).is_ok(),
            "load_from_bytes failed on {payload:?}"
        );
        assert!(
            MagicDatabase::load_from_reader(payload).is_ok(),
            "load_from_reader failed on {payload:?}"
        );
        assert!(
            MagicDatabase::load_from_reader(DribbleReader::new(payload)).is_ok(),
            "dribbled load_from_reader failed on {payload:?}"
        );
    }
}

/// Non-UTF-8 bytes are lossily replaced rather than rejected, matching the
/// file path. Rules on clean lines must still parse and match.
#[test]
fn test_non_utf8_input_is_replaced_and_rules_still_match() {
    let mut source = b"# author name: Fran\xe7ois\n".to_vec();
    source.extend_from_slice(RULES);

    let from_bytes = MagicDatabase::load_from_bytes(source.clone()).unwrap();
    let from_reader = MagicDatabase::load_from_reader(source.as_slice()).unwrap();

    for db in [&from_bytes, &from_reader] {
        let result = db.evaluate_buffer(b"OWNED payload").unwrap();
        assert!(
            result.description.contains("Owned-byte file"),
            "lossy replacement lost the rule: {}",
            result.description
        );
    }
}

/// An unparseable rule is skipped with a warning; sibling rules survive.
#[test]
fn test_unparseable_rule_is_skipped_and_valid_rules_survive() {
    let source = b"0 nosuchtype 1 Bogus rule\n0 string OWNED Owned-byte file\n".to_vec();

    let from_bytes = MagicDatabase::load_from_bytes(source.clone()).unwrap();
    let from_reader = MagicDatabase::load_from_reader(source.as_slice()).unwrap();

    for db in [&from_bytes, &from_reader] {
        let result = db.evaluate_buffer(b"OWNED payload").unwrap();
        assert!(
            result.description.contains("Owned-byte file"),
            "tolerant parsing dropped the valid rule: {}",
            result.description
        );
    }
}

/// The same source text must classify identically however it is supplied.
#[test]
fn test_file_bytes_and_reader_paths_agree() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parity.magic");
    std::fs::write(&path, RULES).unwrap();

    let from_file = MagicDatabase::load_from_file(&path).unwrap();
    let from_bytes = MagicDatabase::load_from_bytes(RULES.to_vec()).unwrap();
    let from_reader = MagicDatabase::load_from_reader(RULES).unwrap();

    let file_result = from_file.evaluate_buffer(b"OWNED payload").unwrap();
    let bytes_result = from_bytes.evaluate_buffer(b"OWNED payload").unwrap();
    let reader_result = from_reader.evaluate_buffer(b"OWNED payload").unwrap();

    assert_eq!(file_result.description, bytes_result.description);
    assert_eq!(file_result.description, reader_result.description);
    // Only the file path records a filesystem source.
    assert!(from_file.source_path().is_some());
    assert!(from_bytes.source_path().is_none());
    assert!(from_reader.source_path().is_none());
}

/// I/O failures should name the operation, not surface a bare errno string.
#[test]
fn test_reader_io_error_carries_context() {
    let error = MagicDatabase::load_from_reader(FailingReader).unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("magic database from reader"),
        "I/O error lost its operation context: {message}"
    );
}

#[test]
fn test_invalid_config_is_rejected_for_bytes_and_reader() {
    let invalid = EvaluationConfig::default().with_max_recursion_depth(0);

    let bytes_error =
        MagicDatabase::load_from_bytes_with_config(RULES.to_vec(), invalid.clone()).unwrap_err();
    assert!(matches!(bytes_error, LibmagicError::ConfigError { .. }));

    let reader_error = MagicDatabase::load_from_reader_with_config(RULES, invalid).unwrap_err();
    assert!(matches!(reader_error, LibmagicError::ConfigError { .. }));
}

/// Config validation must happen before the reader is touched, so a caller
/// passing a bad config does not consume a single-use stream.
#[test]
fn test_config_is_validated_before_the_reader_is_read() {
    let invalid = EvaluationConfig::default().with_max_recursion_depth(0);
    let was_read = Arc::new(AtomicBool::new(false));
    let reader = TrackingReader {
        was_read: Arc::clone(&was_read),
    };

    let error = MagicDatabase::load_from_reader_with_config(reader, invalid).unwrap_err();

    assert!(matches!(error, LibmagicError::ConfigError { .. }));
    assert!(
        !was_read.load(Ordering::SeqCst),
        "a rejected config must not consume the caller's reader"
    );
}
