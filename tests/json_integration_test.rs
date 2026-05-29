// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for JSON output functionality
//!
//! These tests verify that the CLI correctly integrates the JSON output formatter
//! and produces the expected JSON structure when the --json flag is used.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test that the CLI produces valid JSON output when --json flag is used
#[test]
fn test_cli_json_output_format() {
    // Create a temporary test file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.bin");
    fs::write(&test_file, b"Hello, World!").expect("Failed to write test file");

    // Create a basic magic file
    let magic_file = temp_dir.path().join("test.magic");
    fs::write(&magic_file, "0 byte 72 Hello file\n").expect("Failed to write magic file");

    // Run the CLI with --json flag
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rmagic",
            "--",
            test_file.to_str().unwrap(),
            "--json",
            "--magic-file",
            magic_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    if !output.status.success() {
        eprintln!(
            "Command failed with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!("Command failed with exit code: {:?}", output.status.code());
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");

    // Parse the JSON output
    let json_value: serde_json::Value =
        serde_json::from_str(&stdout).expect("Failed to parse JSON output");

    // Verify the JSON structure matches the expected format
    assert!(json_value.is_object(), "Output should be a JSON object");

    let json_obj = json_value.as_object().unwrap();
    assert!(
        json_obj.contains_key("matches"),
        "JSON should contain 'matches' field"
    );

    let matches = json_obj["matches"]
        .as_array()
        .expect("'matches' should be an array");

    if !matches.is_empty() {
        let first_match = &matches[0];
        assert!(first_match.is_object(), "Match should be a JSON object");

        let match_obj = first_match.as_object().unwrap();

        // Verify required fields are present
        assert!(
            match_obj.contains_key("text"),
            "Match should contain 'text' field"
        );
        assert!(
            match_obj.contains_key("offset"),
            "Match should contain 'offset' field"
        );
        assert!(
            match_obj.contains_key("value"),
            "Match should contain 'value' field"
        );
        assert!(
            match_obj.contains_key("tags"),
            "Match should contain 'tags' field"
        );
        assert!(
            match_obj.contains_key("score"),
            "Match should contain 'score' field"
        );

        // Verify field types
        assert!(match_obj["text"].is_string(), "'text' should be a string");
        assert!(
            match_obj["offset"].is_number(),
            "'offset' should be a number"
        );
        assert!(match_obj["value"].is_string(), "'value' should be a string");
        assert!(match_obj["tags"].is_array(), "'tags' should be an array");
        assert!(match_obj["score"].is_number(), "'score' should be a number");
    }
}

/// Test that the CLI produces empty matches array when no rules match
#[test]
fn test_cli_json_output_no_matches() {
    // Create a temporary test file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.bin");
    fs::write(&test_file, b"Random binary data").expect("Failed to write test file");

    // Create a magic file that won't match
    let magic_file = temp_dir.path().join("test.magic");
    fs::write(&magic_file, "0 byte 255 No match file\n").expect("Failed to write magic file");

    // Run the CLI with --json flag
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rmagic",
            "--",
            test_file.to_str().unwrap(),
            "--json",
            "--magic-file",
            magic_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    if !output.status.success() {
        eprintln!(
            "Command failed with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!("Command failed with exit code: {:?}", output.status.code());
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");

    // Parse the JSON output
    let json_value: serde_json::Value =
        serde_json::from_str(&stdout).expect("Failed to parse JSON output");

    // Verify the JSON structure
    assert!(json_value.is_object(), "Output should be a JSON object");

    let json_obj = json_value.as_object().unwrap();
    assert!(
        json_obj.contains_key("matches"),
        "JSON should contain 'matches' field"
    );

    let matches = json_obj["matches"]
        .as_array()
        .expect("'matches' should be an array");

    // When no rules match, we should get an empty matches array or a single "data" match
    // depending on the implementation
    if !matches.is_empty() {
        // If there's a match, it should be the fallback "data" match
        assert_eq!(matches.len(), 1, "Should have exactly one fallback match");
        let match_obj = matches[0].as_object().unwrap();
        // The fallback match might be "data" or similar generic description
        assert!(match_obj["text"].is_string(), "'text' should be a string");
    }
}

/// Test that JSON output is valid and well-formed
#[test]
fn test_cli_json_output_validity() {
    // Create a temporary test file with known content
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "#!/bin/bash\necho 'Hello World'\n").expect("Failed to write test file");

    // Create a magic file that should match
    let magic_file = temp_dir.path().join("test.magic");
    fs::write(&magic_file, "0 byte 35 Bash script\n").expect("Failed to write magic file");

    // Run the CLI with --json flag
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rmagic",
            "--",
            test_file.to_str().unwrap(),
            "--json",
            "--magic-file",
            magic_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    if !output.status.success() {
        eprintln!(
            "Command failed with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!("Command failed with exit code: {:?}", output.status.code());
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");

    // Verify that the output is valid JSON
    let json_value: serde_json::Value =
        serde_json::from_str(&stdout).expect("Failed to parse JSON output");

    // Verify the JSON can be serialized back to string (round-trip test)
    let serialized =
        serde_json::to_string(&json_value).expect("Failed to serialize JSON back to string");

    // Verify the serialized JSON can be parsed again
    let _reparsed: serde_json::Value =
        serde_json::from_str(&serialized).expect("Failed to reparse serialized JSON");

    // Verify the output contains the expected structure
    assert!(json_value.is_object(), "Root should be an object");
    let root_obj = json_value.as_object().unwrap();
    assert!(
        root_obj.contains_key("matches"),
        "Should contain matches array"
    );

    let matches = root_obj["matches"]
        .as_array()
        .expect("matches should be an array");

    // If there are matches, verify their structure
    for match_item in matches {
        assert!(match_item.is_object(), "Each match should be an object");
        let match_obj = match_item.as_object().unwrap();

        // Verify all required fields are present and have correct types
        assert!(match_obj.contains_key("text") && match_obj["text"].is_string());
        assert!(match_obj.contains_key("offset") && match_obj["offset"].is_number());
        assert!(match_obj.contains_key("value") && match_obj["value"].is_string());
        assert!(match_obj.contains_key("tags") && match_obj["tags"].is_array());
        assert!(match_obj.contains_key("score") && match_obj["score"].is_number());

        // Verify score is in valid range (0-100)
        let score = match_obj["score"]
            .as_u64()
            .expect("score should be a number");
        assert!(score <= 100, "Score should be <= 100, got {}", score);
    }
}

/// Test that the JSON output differs from text output
#[test]
fn test_cli_json_vs_text_output() {
    // Create a temporary test file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.bin");
    fs::write(&test_file, b"Test content").expect("Failed to write test file");

    // Create a basic magic file
    let magic_file = temp_dir.path().join("test.magic");
    fs::write(&magic_file, "0 byte 84 Test file\n").expect("Failed to write magic file");

    // Run with JSON output
    let json_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rmagic",
            "--",
            test_file.to_str().unwrap(),
            "--json",
            "--magic-file",
            magic_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute JSON command");

    // Run with text output
    let text_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rmagic",
            "--",
            test_file.to_str().unwrap(),
            "--text",
            "--magic-file",
            magic_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute text command");

    // Both commands should succeed
    assert!(json_output.status.success(), "JSON command should succeed");
    assert!(text_output.status.success(), "Text command should succeed");

    let json_stdout = String::from_utf8(json_output.stdout).expect("Invalid UTF-8 in JSON stdout");
    let text_stdout = String::from_utf8(text_output.stdout).expect("Invalid UTF-8 in text stdout");

    // Outputs should be different
    assert_ne!(
        json_stdout, text_stdout,
        "JSON and text outputs should be different"
    );

    // JSON output should be parseable as JSON
    let _json_value: serde_json::Value =
        serde_json::from_str(&json_stdout).expect("JSON output should be valid JSON");

    // Text output should NOT be parseable as JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&text_stdout).is_err(),
        "Text output should not be valid JSON"
    );

    // Text output should contain the filename
    assert!(
        text_stdout.contains(test_file.file_name().unwrap().to_str().unwrap()),
        "Text output should contain filename"
    );
}

// =============================================================================
// 1B-H2 / 2A-M1: RuleMatch.type_kind must not leak into JSON output
// =============================================================================

/// Library-side `EvaluationResult` carries `RuleMatch.type_kind: TypeKind`
/// for runtime needs (width-masking, bit_width derivation). Serializing
/// the result directly via `serde_json::to_string` must NOT expose the
/// parser AST to JSON consumers. The documented JSON contract is
/// `output::json::JsonMatchResult` which omits this field; the library
/// type now matches that contract via `#[serde(skip)]` on the field.
///
/// Origin findings 1B-H2 (CWE-200 information exposure through serialized
/// AST shape) and 2A-M1 (security-lens variant).
#[test]
fn test_rule_match_type_kind_not_serialized_in_evaluation_result() {
    use libmagic_rs::parser::ast::{TypeKind, Value};
    use libmagic_rs::{EvaluationMetadata, EvaluationResult, evaluator::RuleMatch};

    // Construct an EvaluationResult containing a RuleMatch whose
    // `type_kind` is a fully-qualified TypeKind variant. Then serialize
    // it and assert the rendered JSON contains no `type_kind` key.
    let rule_match = RuleMatch::new(
        "ELF executable".to_string(),
        0,
        0,
        Value::Uint(0x7f),
        TypeKind::Byte { signed: false },
        0.9,
    );
    let metadata = EvaluationMetadata::default();
    let result = EvaluationResult::new(
        "ELF executable".to_string(),
        None,
        0.9,
        vec![rule_match],
        metadata,
    );

    let json = serde_json::to_string(&result).expect("must serialize");
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("emitted JSON must round-trip through serde_json");

    // Walk the structure and assert `type_kind` is absent from every
    // RuleMatch entry under `matches[*]`. Substring checks would false-
    // fail if any user-visible string happened to contain `type_kind`,
    // and -- more importantly -- they cannot prove key absence; only
    // structural inspection can.
    let matches = parsed
        .get("matches")
        .and_then(|v| v.as_array())
        .expect("EvaluationResult JSON must have a `matches` array");
    assert!(
        !matches.is_empty(),
        "EvaluationResult JSON must include at least one rule match for this assertion to be meaningful"
    );
    for (i, m) in matches.iter().enumerate() {
        let obj = m.as_object().unwrap_or_else(|| {
            panic!("matches[{i}] must be a JSON object, got {m}");
        });
        assert!(
            !obj.contains_key("type_kind"),
            "matches[{i}] must not contain `type_kind` key \
             (1B-H2 / 2A-M1 regression: parser AST leaking into JSON output). \
             Got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj.contains_key("value"),
            "matches[{i}] must still include `value` -- #[serde(skip)] should \
             only suppress type_kind, not the surrounding fields. Got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj.contains_key("confidence"),
            "matches[{i}] must still include `confidence`. Got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }
}

/// Sanity check: the `type_kind` field is still accessible from Rust
/// code after `#[serde(skip)]`. The attribute affects serde only; field
/// access for runtime consumers (`format_magic_message`, `bit_width()`
/// derivation) is unchanged.
#[test]
fn test_rule_match_type_kind_still_accessible_in_rust() {
    use libmagic_rs::evaluator::RuleMatch;
    use libmagic_rs::parser::ast::{Endianness, TypeKind, Value};

    let m = RuleMatch::new(
        "test".to_string(),
        0,
        0,
        Value::Uint(0),
        TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        1.0,
    );

    // Field access still works
    match m.type_kind {
        TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        } => {}
        ref other => panic!("type_kind field access broke: got {other:?}"),
    }

    // bit_width() derivation still works (relied on by format_magic_message)
    assert_eq!(m.type_kind.bit_width(), Some(32));
}
