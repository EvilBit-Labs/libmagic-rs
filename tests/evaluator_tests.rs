// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Evaluator integration tests
//!
//! Tests for confidence calculation, rule ordering, and evaluation behavior.
//! Uses both the public `MagicDatabase` API and the lower-level `evaluate_rules`
//! function for type-specific evaluation scenarios.

use libmagic_rs::evaluator::evaluate_rules;
use libmagic_rs::parser::ast::PStringLengthWidth;
use libmagic_rs::{
    Endianness, EvaluationConfig, EvaluationContext, MagicDatabase, MagicRule, OffsetSpec,
    Operator, TypeKind, Value,
};

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

// ============================================================
// Float / Double Evaluation Tests
// ============================================================

#[test]
fn test_evaluate_float_rule_equal() {
    // IEEE 754 little-endian 1.0f32 = 0x3f800000 => bytes [0x00, 0x00, 0x80, 0x3f]
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Float {
            endian: Endianness::Little,
        },
        op: Operator::Equal,
        value: Value::Float(1.0),
        message: "float 1.0 detected".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[0x00, 0x00, 0x80, 0x3f];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1, "Float equal rule should match 1.0f32 LE");
}

#[test]
fn test_evaluate_double_rule_equal() {
    // IEEE 754 big-endian 1.0f64 = 0x3ff0000000000000
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Double {
            endian: Endianness::Big,
        },
        op: Operator::Equal,
        value: Value::Float(1.0),
        message: "double 1.0 detected".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1, "Double equal rule should match 1.0f64 BE");
}

#[test]
fn test_evaluate_float_rule_not_equal() {
    // Buffer contains 1.0f32 LE, rule expects != 2.0 -- should match
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Float {
            endian: Endianness::Little,
        },
        op: Operator::NotEqual,
        value: Value::Float(2.0),
        message: "not 2.0".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[0x00, 0x00, 0x80, 0x3f]; // 1.0f32 LE
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "Float not-equal rule should match when value differs"
    );
}

#[test]
fn test_evaluate_float_rule_less_than() {
    // Buffer contains 1.0f32 LE, rule checks < 2.0 -- should match
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Float {
            endian: Endianness::Little,
        },
        op: Operator::LessThan,
        value: Value::Float(2.0),
        message: "less than 2.0".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[0x00, 0x00, 0x80, 0x3f]; // 1.0f32 LE
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "Float less-than rule should match 1.0 < 2.0"
    );
}

#[test]
fn test_evaluate_pstring_rule_match() {
    // Pascal string: length byte (3) followed by "PDF"
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("PDF".to_string()),
        message: "Pascal PDF marker".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[3, b'P', b'D', b'F', 0x00, 0x00];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1, "PString rule should match pascal string");
    assert_eq!(matches[0].message, "Pascal PDF marker");
}

#[test]
fn test_evaluate_pstring_rule_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("ZIP".to_string()),
        message: "Should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[3, b'P', b'D', b'F'];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "PString rule should not match when strings differ"
    );
}

#[test]
fn test_evaluate_pstring_with_max_length() {
    // Pascal string in buffer has length=10, but max_length caps at 3
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: Some(3),
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("Hel".to_string()),
        message: "Truncated pascal string".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[
        10, b'H', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l',
    ];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "PString with max_length should truncate and match"
    );
}

#[test]
fn test_evaluate_pstring_two_byte_be_with_j_flag() {
    // 2-byte big-endian prefix with /J: stored length=7 includes the 2-byte prefix, so string is 5 bytes
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::TwoByteBE,
            length_includes_itself: true,
        },
        op: Operator::Equal,
        value: Value::String("Hello".to_string()),
        message: "BE pstring with /J flag".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    // Big-endian length 7 = [0x00, 0x07], minus 2-byte prefix = 5 bytes of "Hello"
    let buffer: &[u8] = &[0x00, 0x07, b'H', b'e', b'l', b'l', b'o'];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "PString/HJ rule should match 2-byte BE prefix with self-inclusive length"
    );
}

#[test]
fn test_evaluate_float_rule_no_match() {
    // Buffer contains 1.0f32 LE, rule expects == 2.0 -- should NOT match
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Float {
            endian: Endianness::Little,
        },
        op: Operator::Equal,
        value: Value::Float(2.0),
        message: "should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer: &[u8] = &[0x00, 0x00, 0x80, 0x3f]; // 1.0f32 LE
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "Float equal rule should not match when value differs"
    );
}

// ============================================================
// Third-Party Corpus: regex-eol
// ============================================================

/// Integration test for the `regex-eol.magic` corpus test from the upstream
/// `file` project. The magic file itself uses two syntaxes that the text
/// parser does not yet accept -- a bare unquoted `$ANSIBLE_VAULT` string
/// value (see GOTCHAS S3.6) and `>&1` / `>>&1` relative-offset anchors (the
/// `&+N`/`&-N` parsing TODO in AGENTS.md) -- so this test temporarily
/// bypasses `MagicDatabase::load_from_file` and constructs the equivalent
/// rule tree programmatically. The testfile fixture at
/// `third_party/tests/regex-eol.testfile` is still read verbatim, so the
/// runtime evaluation path (string match, `regex/1l` line-anchored matching,
/// and `OffsetSpec::Relative` anchor advancement through
/// `EvaluationContext::last_match_end`) is exercised end-to-end.
///
/// Once the parser learns unquoted string values and `&+N` relative offsets,
/// this test should be rewritten to call `MagicDatabase::load_from_file`
/// against the unmodified `regex-eol.magic` corpus file.
#[test]
fn test_regex_eol_corpus() {
    let buffer = std::fs::read("third_party/tests/regex-eol.testfile")
        .expect("failed to read regex-eol.testfile");

    // Mirror of:
    //   0     string    $ANSIBLE_VAULT     Ansible Vault text
    //   >&1   regex/1l  [0-9]+(\.[0-9]+)+  \b, version %s
    //   >>&1  regex/1l  [^;]+$             \b, using %s encryption
    //
    // Messages hardcode the captured tokens that libmagic's `%s` formatter
    // would substitute (libmagic-rs does not yet implement format
    // substitution), so the final description contains the literal
    // `version`, `1.1`, and `AES256` strings. The match-value assertions
    // below separately verify the regex engine actually captured those
    // tokens from the buffer, so the test still fails if the regex
    // behavior regresses.
    //
    // `max_length: Some(14)` caps read_string at the 14-byte target so the
    // comparison succeeds on a buffer with no NUL terminator. `Relative(1)`
    // on each child matches the `&+1` anchor offset (previous match end + 1,
    // skipping the `;` separator).
    // `regex/1l` == 1-line scan window. In the new RegexCount design
    // this is `RegexCount::Lines(Some(NonZeroU32::new(1)))`. Multi-line
    // regex matching is always on (matching libmagic's unconditional
    // REG_NEWLINE) so `^`/`$` match at line boundaries regardless.
    let one_line_count =
        libmagic_rs::parser::ast::RegexCount::Lines(::std::num::NonZeroU32::new(1));

    let inner_regex = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Regex {
            flags: libmagic_rs::parser::ast::RegexFlags::default(),
            count: one_line_count,
        },
        op: Operator::Equal,
        value: Value::String("[^;]+$".to_string()),
        message: "\u{0008}, using AES256 encryption".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
    };

    let version_regex = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Regex {
            flags: libmagic_rs::parser::ast::RegexFlags::default(),
            count: one_line_count,
        },
        op: Operator::Equal,
        value: Value::String("[0-9]+(\\.[0-9]+)+".to_string()),
        message: "\u{0008}, version 1.1".to_string(),
        children: vec![inner_regex],
        level: 1,
        strength_modifier: None,
    };

    let ansible_vault = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: Some("$ANSIBLE_VAULT".len()),
        },
        op: Operator::Equal,
        value: Value::String("$ANSIBLE_VAULT".to_string()),
        message: "Ansible Vault text".to_string(),
        children: vec![version_regex],
        level: 0,
        strength_modifier: None,
    };

    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches =
        evaluate_rules(&[ansible_vault], &buffer, &mut context).expect("evaluation failed");

    // All three rules must fire in order: the top-level string, the version
    // regex, and the encryption regex.
    assert_eq!(
        matches.len(),
        3,
        "expected 3 matches (string + 2 regex), got {}: {matches:#?}",
        matches.len()
    );

    // Verify the regex engine captured the expected tokens from the buffer.
    // These assertions fail if regex evaluation or the relative-offset
    // anchor advances incorrectly.
    assert_eq!(
        matches[0].value,
        Value::String("$ANSIBLE_VAULT".to_string()),
        "top-level string match should capture $ANSIBLE_VAULT"
    );
    if let Value::String(s) = &matches[1].value {
        assert!(
            s.contains("1.1"),
            "version regex should capture '1.1', got {s:?}"
        );
    } else {
        panic!(
            "expected Value::String for version regex, got {:?}",
            matches[1].value
        );
    }
    if let Value::String(s) = &matches[2].value {
        assert!(
            s.contains("AES256"),
            "encryption regex should capture 'AES256', got {s:?}"
        );
    } else {
        panic!(
            "expected Value::String for encryption regex, got {:?}",
            matches[2].value
        );
    }

    // Mirror `MagicDatabase::build_result` message concatenation: rules whose
    // message starts with a backspace (`\b`) suppress the leading space.
    let mut description = String::new();
    for m in &matches {
        if let Some(rest) = m.message.strip_prefix('\u{0008}') {
            description.push_str(rest);
        } else if description.is_empty() {
            description.push_str(&m.message);
        } else {
            description.push(' ');
            description.push_str(&m.message);
        }
    }

    assert!(
        description.contains("Ansible Vault"),
        "expected 'Ansible Vault' in description, got: {description:?}"
    );
    assert!(
        description.contains("version"),
        "expected 'version' in description, got: {description:?}"
    );
    assert!(
        description.contains("AES256"),
        "expected 'AES256' in description, got: {description:?}"
    );
}
