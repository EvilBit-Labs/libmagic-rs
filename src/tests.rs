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
    assert!(!db.rules.is_empty(), "Built-in rules should not be empty");

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

    // Test unknown data falls back to "data"
    let unknown_result = db.evaluate_buffer(b"random unknown content").unwrap();
    assert_eq!(unknown_result.description, "data");
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

    let unknown_result = db.evaluate_buffer(b"random unknown content").unwrap();

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
            confidence: 0.3,
        },
        evaluator::RuleMatch {
            message: "64-bit".to_string(),
            offset: 4,
            level: 1,
            value: Value::Uint(2),
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
            confidence: 0.3,
        },
        evaluator::RuleMatch {
            message: "\u{0008}, 64-bit".to_string(), // backspace prefix
            offset: 4,
            level: 1,
            value: Value::Uint(2),
            confidence: 0.5,
        },
    ];

    let result = MagicDatabase::concatenate_messages(&matches);
    assert_eq!(result, "ELF, 64-bit"); // No space before comma
}
