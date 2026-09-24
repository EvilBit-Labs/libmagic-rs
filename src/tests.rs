// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_evaluation_config_default() {
    let config = EvaluationConfig::default();

    assert_eq!(config.max_recursion_depth, 20);
    assert_eq!(config.max_string_length, 8192);
    assert!(config.stop_at_first_match);
    assert!(!config.enable_mime_types);
    assert_eq!(config.timeout_ms, None);
}

#[test]
fn test_evaluation_config_new() {
    let config = EvaluationConfig::new();
    let default_config = EvaluationConfig::default();

    assert_eq!(config, default_config);
}

#[test]
fn test_evaluation_config_performance() {
    let config = EvaluationConfig::performance();

    assert_eq!(config.max_recursion_depth, 10);
    assert_eq!(config.max_string_length, 1024);
    assert!(config.stop_at_first_match);
    assert!(!config.enable_mime_types);
    assert_eq!(config.timeout_ms, Some(1000));
}

#[test]
fn test_evaluation_config_comprehensive() {
    let config = EvaluationConfig::comprehensive();

    assert_eq!(config.max_recursion_depth, 50);
    assert_eq!(config.max_string_length, 32768);
    assert!(!config.stop_at_first_match);
    assert!(config.enable_mime_types);
    assert_eq!(config.timeout_ms, Some(30000));
}

#[test]
fn test_evaluation_config_validate_valid() {
    let config = EvaluationConfig::default();
    assert!(config.validate().is_ok());

    let performance_config = EvaluationConfig::performance();
    assert!(performance_config.validate().is_ok());

    let comprehensive_config = EvaluationConfig::comprehensive();
    assert!(comprehensive_config.validate().is_ok());
}

#[test]
fn test_evaluation_config_validate_zero_recursion_depth() {
    let config = EvaluationConfig {
        max_recursion_depth: 0,
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("max_recursion_depth must be greater than 0"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_excessive_recursion_depth() {
    let config = EvaluationConfig {
        max_recursion_depth: 1001,
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("max_recursion_depth must not exceed 1000"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_zero_string_length() {
    let config = EvaluationConfig {
        max_string_length: 0,
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("max_string_length must be greater than 0"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_excessive_string_length() {
    let config = EvaluationConfig {
        max_string_length: 1_048_577, // 1MB + 1
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("max_string_length must not exceed"));
            assert!(message.contains("bytes to prevent memory exhaustion"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_zero_timeout() {
    let config = EvaluationConfig {
        timeout_ms: Some(0),
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("timeout_ms must be greater than 0 if specified"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_excessive_timeout() {
    let config = EvaluationConfig {
        timeout_ms: Some(300_001), // 5 minutes + 1ms
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::ConfigError { reason: message } => {
            assert!(message.contains("timeout_ms must not exceed 300000"));
        }
        _ => panic!("Expected ConfigError"),
    }
}

#[test]
fn test_evaluation_config_validate_boundary_values() {
    // Test minimum valid values
    let min_config = EvaluationConfig {
        max_recursion_depth: 1,
        max_string_length: 1,
        timeout_ms: Some(1),
        ..Default::default()
    };
    assert!(min_config.validate().is_ok());

    // Test maximum valid values (avoiding the security constraint)
    let max_config = EvaluationConfig {
        max_recursion_depth: 100,     // Max allowed with large string length
        max_string_length: 1_048_576, // 1MB
        timeout_ms: Some(300_000),    // 5 minutes
        ..Default::default()
    };
    assert!(max_config.validate().is_ok());

    // Test maximum recursion depth with smaller string length
    let max_recursion_config = EvaluationConfig {
        max_recursion_depth: 1000,
        max_string_length: 65536, // Max allowed with high recursion depth
        timeout_ms: Some(300_000),
        ..Default::default()
    };
    assert!(max_recursion_config.validate().is_ok());
}

#[test]
fn test_evaluation_config_clone() {
    let config = EvaluationConfig {
        max_recursion_depth: 15,
        max_string_length: 4096,
        stop_at_first_match: false,
        enable_mime_types: true,
        timeout_ms: Some(5000),
    };

    let cloned_config = config.clone();
    assert_eq!(config, cloned_config);
}

#[test]
fn test_evaluation_config_debug() {
    let config = EvaluationConfig::default();
    let debug_str = format!("{config:?}");

    assert!(debug_str.contains("EvaluationConfig"));
    assert!(debug_str.contains("max_recursion_depth"));
    assert!(debug_str.contains("max_string_length"));
    assert!(debug_str.contains("stop_at_first_match"));
    assert!(debug_str.contains("enable_mime_types"));
    assert!(debug_str.contains("timeout_ms"));
}

#[test]
fn test_evaluation_config_partial_eq() {
    let config1 = EvaluationConfig::default();
    let config2 = EvaluationConfig::default();
    let config3 = EvaluationConfig::performance();

    assert_eq!(config1, config2);
    assert_ne!(config1, config3);
}

#[test]
fn test_evaluation_config_custom_values() {
    let config = EvaluationConfig {
        max_recursion_depth: 25,
        max_string_length: 16384,
        stop_at_first_match: false,
        enable_mime_types: true,
        timeout_ms: Some(10000),
    };

    assert_eq!(config.max_recursion_depth, 25);
    assert_eq!(config.max_string_length, 16384);
    assert!(!config.stop_at_first_match);
    assert!(config.enable_mime_types);
    assert_eq!(config.timeout_ms, Some(10000));

    assert!(config.validate().is_ok());
}

#[test]
fn test_libmagic_error_from_parse_error() {
    let parse_error = ParseError::invalid_syntax(10, "test error");
    let libmagic_error = LibmagicError::from(parse_error);

    match libmagic_error {
        LibmagicError::ParseError(_) => (),
        _ => panic!("Expected ParseError variant"),
    }
}

#[test]
fn test_libmagic_error_from_evaluation_error() {
    let eval_error = EvaluationError::buffer_overrun(100);
    let libmagic_error = LibmagicError::from(eval_error);

    match libmagic_error {
        LibmagicError::EvaluationError(_) => (),
        _ => panic!("Expected EvaluationError variant"),
    }
}

#[test]
fn test_with_builtin_rules() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    // Verify built-in rules are loaded
    assert!(
        !db.root_rules.is_empty(),
        "Built-in rules should not be empty"
    );

    // Verify source_path is None for built-in rules
    assert!(db.source_path().is_none());

    // Test ELF detection with built-in rules
    let elf_header = b"\x7fELF\x02\x01\x01\x00";
    let elf_result = db.evaluate_buffer(elf_header).unwrap();
    assert!(
        elf_result.description.contains("ELF"),
        "Expected ELF detection, got: {}",
        elf_result.description
    );

    // Test ZIP detection with built-in rules
    let zip_header = b"PK\x03\x04";
    let zip_result = db.evaluate_buffer(zip_header).unwrap();
    assert!(
        zip_result.description.contains("ZIP"),
        "Expected ZIP detection, got: {}",
        zip_result.description
    );

    // Unmatched plain ASCII content falls back to the text/data
    // classifier's "ASCII text" result, matching GNU `file` (verified:
    // `file` reports "ASCII text, with no line terminators" for this
    // exact buffer) -- NOT the old hardcoded "data" this test asserted
    // before the text/data fallback (GOTCHAS S13.2 / issue: blank output
    // for readable files) was implemented.
    let unknown_text_result = db.evaluate_buffer(b"random unknown content").unwrap();
    assert_eq!(unknown_text_result.description, "ASCII text");

    // Genuinely binary, unmatched content still falls back to "data".
    let unknown_binary_result = db
        .evaluate_buffer(&[0x00, 0x01, 0x02, 0xFF, 0xFE, 0x10])
        .unwrap();
    assert_eq!(unknown_binary_result.description, "data");
}

#[test]
fn test_evaluation_metadata_default() {
    let metadata = EvaluationMetadata::default();

    assert_eq!(metadata.file_size, 0);
    assert!((metadata.evaluation_time_ms - 0.0).abs() < 0.001);
    assert_eq!(metadata.rules_evaluated, 0);
    assert!(metadata.magic_file.is_none());
    assert!(!metadata.timed_out);
}

#[test]
fn test_evaluation_result_has_metadata() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    let elf_header = b"\x7fELF\x02\x01\x01\x00";
    let result = db.evaluate_buffer(elf_header).unwrap();

    // Check metadata is populated
    assert_eq!(result.metadata.file_size, 8);
    assert!(result.metadata.evaluation_time_ms >= 0.0);
    assert!(result.metadata.rules_evaluated > 0);
    assert!(result.metadata.magic_file.is_none()); // Built-in rules
    assert!(!result.metadata.timed_out);
}

#[test]
fn test_evaluation_result_has_matches() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    let elf_header = b"\x7fELF\x02\x01\x01\x00";
    let result = db.evaluate_buffer(elf_header).unwrap();

    // Should have at least one match for ELF
    assert!(!result.matches.is_empty());

    // First match should have confidence > 0
    let first_match = &result.matches[0];
    assert!(first_match.confidence > 0.0);
}

#[test]
fn test_evaluation_result_confidence_from_matches() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    let elf_header = b"\x7fELF\x02\x01\x01\x00";
    let result = db.evaluate_buffer(elf_header).unwrap();

    // Result confidence should match first match confidence
    if !result.matches.is_empty() {
        assert!((result.confidence - result.matches[0].confidence).abs() < 0.001);
    }
}

#[test]
fn test_evaluation_result_no_match_has_zero_confidence() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    // Genuinely binary content so no built-in rule matches and the
    // text/data fallback (GOTCHAS S13.2) reports "data" -- confirming
    // confidence is 0.0 for a fallback-classified result, not just an
    // empty-matches result.
    let unknown_result = db
        .evaluate_buffer(&[0x00, 0x01, 0x02, 0xFF, 0xFE, 0x10])
        .unwrap();

    assert_eq!(unknown_result.description, "data");
    assert!((unknown_result.confidence - 0.0).abs() < 0.001);
    assert!(unknown_result.matches.is_empty());
}

#[test]
fn test_concatenate_messages_simple() {
    let matches = vec![
        evaluator::RuleMatch {
            message: "ELF".to_string(),
            offset: 0,
            level: 0,
            value: Value::Bytes(vec![0x7f]),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.3,
        },
        evaluator::RuleMatch {
            message: "64-bit".to_string(),
            offset: 4,
            level: 1,
            value: Value::Uint(2),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.5,
        },
    ];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(result, "ELF 64-bit");
}

#[test]
fn test_concatenate_messages_with_backspace() {
    let matches = vec![
        evaluator::RuleMatch {
            message: "ELF".to_string(),
            offset: 0,
            level: 0,
            value: Value::Bytes(vec![0x7f]),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.3,
        },
        evaluator::RuleMatch {
            message: "\u{0008}, 64-bit".to_string(), // backspace prefix
            offset: 4,
            level: 1,
            value: Value::Uint(2),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.5,
        },
    ];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(result, "ELF, 64-bit"); // No space before comma
}

/// Regression: the `\b` no-separator marker reaches concatenation as the
/// LITERAL two-character sequence `\b` (backslash + 'b'), NOT a U+0008 byte,
/// because the message parser preserves description text verbatim (matching
/// GNU `file`, which keeps the desc literal and special-cases a leading `\b`
/// at print time). Real rules like msdos's `\b, for MS Windows` and the
/// Mach-O universal-binary `\b]` were rendering the literal marker into the
/// output ("PE STUB    \b, for MS Windows", "...architectures: \b]") until
/// concatenation learned to strip the literal `\b` too (GOTCHAS S14.1).
#[test]
fn test_concatenate_messages_with_literal_backslash_b_marker() {
    let matches = vec![
        evaluator::RuleMatch {
            message: "PE STUB".to_string(),
            offset: 0,
            level: 0,
            value: Value::Uint(0),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.3,
        },
        evaluator::RuleMatch {
            // Literal backslash + 'b', exactly as `parse_message` produces it.
            message: "\\b, for MS Windows".to_string(),
            offset: 4,
            level: 1,
            value: Value::Uint(0),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.5,
        },
    ];
    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(
        result, "PE STUB, for MS Windows",
        "literal \\b marker must suppress the space and not appear in the output"
    );
}

/// Regression test for review finding M5 / GOTCHAS S14.1: the `\b`
/// (backspace) prefix must suppress the leading separator even when the
/// backspace-prefixed message is the first entry. This is a degenerate
/// case — there is no prior space to suppress — but the stripping must
/// still happen so the output text doesn't contain a stray backspace.
#[test]
fn test_concatenate_messages_backspace_on_first_match() {
    let matches = vec![evaluator::RuleMatch {
        message: "\u{0008}leading-strip".to_string(),
        offset: 0,
        level: 0,
        value: Value::Uint(0),
        type_kind: TypeKind::Byte { signed: false },
        confidence: 0.5,
    }];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(
        result, "leading-strip",
        "backspace-prefixed first match must strip the backspace byte"
    );
}

/// Regression test for review finding M5 / GOTCHAS S14.1: two consecutive
/// backspace-prefixed messages must both strip their prefixes, producing
/// a space-free concatenation (relevant for "ELF\b, 64-bit\b, LSB" style
/// chains).
#[test]
fn test_concatenate_messages_consecutive_backspaces() {
    let make_match = |message: &str| evaluator::RuleMatch {
        message: message.to_string(),
        offset: 0,
        level: 0,
        value: Value::Uint(0),
        type_kind: TypeKind::Byte { signed: false },
        confidence: 0.5,
    };
    let matches = vec![
        make_match("ELF"),
        make_match("\u{0008}, 64-bit"),
        make_match("\u{0008}, LSB"),
    ];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(
        result, "ELF, 64-bit, LSB",
        "consecutive backspace-prefixed messages must each strip cleanly"
    );
}

/// Regression test for review finding M5 / GOTCHAS S14.1: a backspace
/// prefix followed by an empty rest must append nothing (not a bare
/// backspace, not a stray space).
#[test]
fn test_concatenate_messages_backspace_empty_rest() {
    let matches = vec![
        evaluator::RuleMatch {
            message: "prefix".to_string(),
            offset: 0,
            level: 0,
            value: Value::Uint(0),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.5,
        },
        evaluator::RuleMatch {
            message: "\u{0008}".to_string(),
            offset: 0,
            level: 0,
            value: Value::Uint(0),
            type_kind: TypeKind::Byte { signed: false },
            confidence: 0.5,
        },
    ];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(
        result, "prefix",
        "backspace with empty rest must contribute nothing to the output"
    );
}

/// End-to-end proof that R2's newline-stop gate is live through the real
/// evaluation + rendering pipeline (issue #498, U3/R12/R14).
///
/// These tests exercise the production dispatch chain directly:
/// `evaluate_rules` -> `evaluate_single_rule_with_anchor` ->
/// `evaluate_value_rule` -> `read_typed_value_with_pattern`'s any-value
/// arm (now bounded, see `evaluator::types::any_value_string_bound`) ->
/// `MagicDatabase::concatenate_messages` -> `format_magic_message`. The
/// newline stop is enforced entirely at the READ (Half A / R12); the
/// render-layer `format_magic_message_with_gate` signal is not threaded
/// through `RuleMatch` in this unit (see `newline_stop_gate`'s doc
/// comment in `evaluator::engine::value_eval` for why).
#[cfg(test)]
mod newline_gate_end_to_end_tests {
    use super::MagicDatabase;
    use crate::evaluator::evaluate_rules;
    use crate::parser::ast::StringFlags;
    use crate::{EvaluationContext, MagicRule, OffsetSpec, Operator, TypeKind, Value};

    /// Proof scenario from the plan: `0 string x STR=[%s]` over
    /// `ZZZZABC\nSECOND\n` must render exactly one line, matching
    /// `file-5.41`'s newline-stop behavior for the any-value idiom.
    /// Before this unit: 3 lines (the value read the whole multi-line
    /// buffer). After: 1 line.
    #[test]
    fn test_any_value_string_x_renders_single_line_across_embedded_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            Operator::AnyValue,
            Value::Uint(0),
            "STR=[%s]".to_string(),
        );
        let buffer = b"ZZZZABC\nSECOND\n";
        let mut ctx = EvaluationContext::new(crate::EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1, "the any-value rule should match once");

        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(
            description.lines().count(),
            1,
            "description must be a single line, got: {description:?}"
        );
        assert_eq!(description, "STR=[ZZZZABC]");
    }

    /// Proof scenario from the plan: an equality-compared rule whose
    /// PATTERN itself contains an embedded newline (so the matched buffer
    /// must contain it too) renders that newline verbatim -- equality
    /// never gates R2's stop, regardless of content.
    #[test]
    fn test_equality_compared_rule_with_embedded_newline_does_not_stop() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            Operator::Equal,
            Value::String("AB\nCD".to_string()),
            "eq=[%s]".to_string(),
        );
        let buffer = b"AB\nCD";
        let mut ctx = EvaluationContext::new(crate::EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1, "the equality rule should match once");

        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(
            description, "eq=[AB\nCD]",
            "an equality-compared rule must never stop at an embedded newline"
        );
    }

    /// Proof scenario from the plan: an ordering rule whose pattern's
    /// first byte is a null byte gates R2's newline stop for its
    /// full-field display render; one whose pattern's first byte is NOT
    /// null does not gate and keeps the embedded newline.
    #[test]
    fn test_ordering_rule_null_first_byte_gates_end_to_end() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            Operator::GreaterThan,
            Value::Bytes(vec![0, b'A']), // null-first-byte pattern: gates
            "gated=[%s]".to_string(),
        );
        let buffer = b"ZZZZ\nSECOND";
        let mut ctx = EvaluationContext::new(crate::EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);

        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, "gated=[ZZZZ]");
    }

    #[test]
    fn test_ordering_rule_non_null_first_byte_does_not_gate_end_to_end() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            Operator::GreaterThan,
            Value::String("A".to_string()), // non-null-first-byte: does not gate
            "ungated=[%s]".to_string(),
        );
        let buffer = b"ZZZZ\nSECOND";
        let mut ctx = EvaluationContext::new(crate::EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);

        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(
            description, "ungated=[ZZZZ\nSECOND]",
            "non-gated ordering render must keep the embedded newline"
        );
    }
}

/// End-to-end proof that R5 (extending R1/R2's any-value newline-stop +
/// 127-byte render bound to `pstring` and `string16`) is live through the
/// real evaluation + rendering pipeline, mirroring
/// `newline_gate_end_to_end_tests` above for plain `string`.
///
/// Every rendered string in this module was measured directly against
/// `file-5.41` with an equivalent hand-written magic file; each test's doc
/// comment states the measured oracle value.
#[cfg(test)]
mod pstring_string16_newline_gate_end_to_end_tests {
    use super::MagicDatabase;
    use crate::evaluator::evaluate_rules;
    use crate::parser::ast::PStringLengthWidth;
    use crate::{
        EvaluationConfig, EvaluationContext, MagicRule, OffsetSpec, Operator, TypeKind, Value,
    };

    fn pstring_type() -> TypeKind {
        TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        }
    }

    fn le_string16_type() -> TypeKind {
        TypeKind::String16 {
            endian: crate::Endianness::Little,
        }
    }

    fn ucs2le(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for ch in s.chars() {
            out.extend_from_slice(&(ch as u16).to_le_bytes());
        }
        out
    }

    /// Measured: `0 pstring x PS=[%s]` over a 1-byte-prefix "ABC\ndef"
    /// payload followed by unrelated trailing bytes renders `PS=[ABC]`
    /// (one line), not the full multi-line payload.
    #[test]
    fn test_pstring_any_value_renders_single_line_across_embedded_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        );
        let buffer = b"\x07ABC\ndeftail\n";
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description.lines().count(), 1);
        assert_eq!(description, "PS=[ABC]");
    }

    /// Measured: `0 lestring16 x S16=[%s]` over "AB\ncd" (UCS-2LE,
    /// NUL-terminated) followed by unrelated trailing bytes renders
    /// `S16=[AB]` (one line).
    #[test]
    fn test_string16_any_value_renders_single_line_across_embedded_newline() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            le_string16_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "S16=[%s]".to_string(),
        );
        let mut buffer = ucs2le("AB\ncd");
        buffer.extend_from_slice(&[0, 0]);
        buffer.extend_from_slice(b"tail");
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description.lines().count(), 1);
        assert_eq!(description, "S16=[AB]");
    }

    /// Measured: `0 pstring =ABC\ndef EQ=[%s]` over the same payload
    /// renders the embedded newline verbatim (`EQ=[ABC` / `def]` -- two
    /// lines) -- an equality-compared pstring never stops.
    #[test]
    fn test_pstring_equality_compared_with_embedded_newline_does_not_stop() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::Equal,
            Value::String("ABC\ndef".to_string()),
            "EQ=[%s]".to_string(),
        );
        let buffer = b"\x07ABC\ndef";
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, "EQ=[ABC\ndef]");
    }

    /// Measured: `0 lestring16 =AB\ncd EQ16=[%s]` over the same payload
    /// renders the embedded newline verbatim -- an equality-compared
    /// string16 never stops either.
    #[test]
    fn test_string16_equality_compared_with_embedded_newline_does_not_stop() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            le_string16_type(),
            Operator::Equal,
            Value::String("AB\ncd".to_string()),
            "EQ16=[%s]".to_string(),
        );
        let mut buffer = ucs2le("AB\ncd");
        buffer.extend_from_slice(&[0, 0]);
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        assert_eq!(matches.len(), 1);
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, "EQ16=[AB\ncd]");
    }

    /// Measured: a 200-byte pstring payload of 'Q' with no newline renders
    /// exactly 127 'Q' characters (the R1 render bound), not the full
    /// declared payload.
    #[test]
    fn test_pstring_any_value_caps_at_127_bytes() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        );
        let mut buffer = vec![200u8];
        buffer.extend(std::iter::repeat_n(b'Q', 200));
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, format!("PS=[{}]", "Q".repeat(127)));
    }

    /// Measured: a 300-code-unit lestring16 payload of 'Q' with no
    /// newline renders exactly 127 'Q' characters -- the bound is in
    /// DECODED characters, not raw (2-bytes-per-unit) source bytes.
    #[test]
    fn test_string16_any_value_caps_at_127_chars() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            le_string16_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "S16=[%s]".to_string(),
        );
        let buffer = ucs2le(&"Q".repeat(300));
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), &buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, format!("S16=[{}]", "Q".repeat(127)));
    }

    /// A configured `max_string_length` smaller than 127 (the existing
    /// CWE-770 security pin, GOTCHAS 2A-H1) must still win over the
    /// 127-byte render bound for BOTH pstring and string16 any-value
    /// reads -- this is rmagic's own safety composition, not a `file`
    /// behavior (GNU `file` has no equivalent config), so it is verified
    /// as an internal invariant rather than against a `file` oracle.
    #[test]
    fn test_max_string_length_config_wins_over_127_for_pstring_and_string16() {
        let config = EvaluationConfig::default().with_max_string_length(10);

        let pstring_rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        );
        let mut pstring_buffer = vec![200u8];
        pstring_buffer.extend(std::iter::repeat_n(b'Q', 200));
        let mut ctx = EvaluationContext::new(config.clone());
        let matches = evaluate_rules(
            std::slice::from_ref(&pstring_rule),
            &pstring_buffer,
            &mut ctx,
        )
        .expect("evaluate_rules should not error for this simple rule");
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, format!("PS=[{}]", "Q".repeat(10)));

        let string16_rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            le_string16_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "S16=[%s]".to_string(),
        );
        let string16_buffer = ucs2le(&"Q".repeat(300));
        let mut ctx = EvaluationContext::new(config);
        let matches = evaluate_rules(
            std::slice::from_ref(&string16_rule),
            &string16_buffer,
            &mut ctx,
        )
        .expect("evaluate_rules should not error for this simple rule");
        let description = MagicDatabase::concatenate_messages(&matches);
        assert_eq!(description, format!("S16=[{}]", "Q".repeat(10)));
    }

    /// Measured with a relative-offset child (`>&0 byte x next=0x%02x`):
    /// the pstring any-value anchor lands at prefix width (1) + the
    /// newline's index within the payload (3) = 4, which is the newline
    /// byte itself (`0x0a`).
    #[test]
    fn test_pstring_any_value_anchor_advances_past_prefix_width_and_stops_at_newline() {
        let child = MagicRule::new(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Operator::AnyValue,
            Value::Uint(0),
            "next=%d".to_string(),
        );
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        )
        .with_children(vec![child]);
        let buffer = b"\x07ABC\ndeftail\n";
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&parent), buffer, &mut ctx)
            .expect("evaluate_rules should not error for this simple rule");
        // parent match + child match
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[1].value, Value::Uint(u64::from(b'\n')));
    }

    /// A pstring whose declared prefix length exceeds the remaining
    /// buffer must still degrade to a non-match without panicking, even
    /// for an any-value rule now routed through the new bounded read
    /// path (existing behavior, must not regress).
    #[test]
    fn test_pstring_any_value_declared_prefix_exceeds_buffer_degrades_to_non_match() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            pstring_type(),
            Operator::AnyValue,
            Value::Uint(0),
            "PS=[%s]".to_string(),
        );
        let buffer = b"\x05ab"; // declares 5 bytes, only 2 available
        let mut ctx = EvaluationContext::new(EvaluationConfig::default());
        let matches = evaluate_rules(std::slice::from_ref(&rule), buffer, &mut ctx)
            .expect("evaluate_rules should not error (a read failure is a non-match, not a propagated error)");
        assert!(matches.is_empty());
    }
}
