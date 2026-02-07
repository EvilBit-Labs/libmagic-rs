//! Property-based tests for libmagic-rs
//!
//! Uses proptest to verify properties that should hold for all valid inputs:
//! - Parser accepts valid magic syntax
//! - Evaluator never panics on valid input
//! - Buffer access is always bounds-checked
//! - Offset calculations are correct

use proptest::prelude::*;

use libmagic_rs::{
    EvaluationConfig, MagicDatabase, MagicRule, OffsetSpec, Operator, TypeKind, Value,
};

/// Generate a valid OffsetSpec for testing
fn arb_offset_spec() -> impl Strategy<Value = OffsetSpec> {
    prop_oneof![
        // Absolute offset (reasonable range)
        (-1000i64..=1000i64).prop_map(OffsetSpec::Absolute),
        // Relative offset
        (-100i64..=100i64).prop_map(OffsetSpec::Relative),
        // FromEnd offset (usually negative)
        (-100i64..=0i64).prop_map(OffsetSpec::FromEnd),
    ]
}

/// Generate a valid TypeKind for testing
fn arb_type_kind() -> impl Strategy<Value = TypeKind> {
    prop_oneof![
        Just(TypeKind::Byte),
        (any::<bool>(), any::<bool>()).prop_map(|(is_big, signed)| {
            TypeKind::Short {
                endian: if is_big {
                    libmagic_rs::Endianness::Big
                } else {
                    libmagic_rs::Endianness::Little
                },
                signed,
            }
        }),
        (any::<bool>(), any::<bool>()).prop_map(|(is_big, signed)| {
            TypeKind::Long {
                endian: if is_big {
                    libmagic_rs::Endianness::Big
                } else {
                    libmagic_rs::Endianness::Little
                },
                signed,
            }
        }),
        (0usize..256usize).prop_map(|len| TypeKind::String {
            max_length: Some(len),
        }),
    ]
}

/// Generate a valid Operator for testing
fn arb_operator() -> impl Strategy<Value = Operator> {
    prop_oneof![
        Just(Operator::Equal),
        Just(Operator::NotEqual),
        Just(Operator::BitwiseAnd),
        (0u64..=255u64).prop_map(Operator::BitwiseAndMask),
    ]
}

/// Generate a valid Value for testing
fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        (0u64..=u32::MAX as u64).prop_map(Value::Uint),
        (i32::MIN as i64..=i32::MAX as i64).prop_map(Value::Int),
        prop::collection::vec(any::<u8>(), 0..32).prop_map(Value::Bytes),
        "[a-zA-Z0-9 ]{0,32}".prop_map(Value::String),
    ]
}

/// Generate a valid message string for testing
fn arb_message() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _-]{1,64}"
}

/// Generate a valid MagicRule for testing
fn arb_magic_rule() -> impl Strategy<Value = MagicRule> {
    (
        arb_offset_spec(),
        arb_type_kind(),
        arb_operator(),
        arb_value(),
        arb_message(),
    )
        .prop_map(|(offset, typ, op, value, message)| MagicRule {
            offset,
            typ,
            op,
            value,
            message,
            children: vec![],
            level: 0,
            strength_modifier: None,
        })
}

/// Generate arbitrary binary data for testing
fn arb_buffer() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..1024)
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    /// Property: Built-in rules should load successfully
    #[test]
    fn prop_builtin_rules_always_load(_seed in any::<u64>()) {
        let result = MagicDatabase::with_builtin_rules();
        prop_assert!(result.is_ok(), "Built-in rules should always load");
    }

    /// Property: Evaluation should never panic on any valid buffer
    #[test]
    fn prop_evaluation_never_panics(buffer in arb_buffer()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        // This should not panic regardless of buffer contents
        let result = db.evaluate_buffer(&buffer);

        // Result should be Ok (evaluation succeeded) or contain valid error
        match result {
            Ok(eval_result) => {
                // Description should be non-empty
                prop_assert!(!eval_result.description.is_empty());
                // Confidence should be in valid range
                prop_assert!(eval_result.confidence >= 0.0);
                prop_assert!(eval_result.confidence <= 1.0);
            }
            Err(e) => {
                // Error message should be non-empty
                prop_assert!(!e.to_string().is_empty());
            }
        }
    }

    /// Property: Empty buffer should be handled gracefully
    #[test]
    fn prop_empty_buffer_handled(_seed in any::<u64>()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        let result = db.evaluate_buffer(&[]);
        prop_assert!(result.is_ok());

        let eval_result = result.expect("should be ok");
        // Empty buffer should either match nothing or return "data"
        prop_assert!(!eval_result.description.is_empty());
    }

    /// Property: EvaluationConfig validation should be consistent
    #[test]
    fn prop_config_validation_consistent(
        recursion_depth in 1u32..100u32,
        string_length in 1usize..10000usize,
        timeout in 1u64..100000u64
    ) {
        let config = EvaluationConfig {
            max_recursion_depth: recursion_depth,
            max_string_length: string_length,
            stop_at_first_match: true,
            enable_mime_types: false,
            timeout_ms: Some(timeout),
        };

        // Validation should succeed for reasonable values
        let result = config.validate();
        prop_assert!(result.is_ok());
    }

    /// Property: Invalid config should always fail validation
    #[test]
    fn prop_zero_recursion_fails(_seed in any::<u64>()) {
        let config = EvaluationConfig {
            max_recursion_depth: 0,
            ..EvaluationConfig::default()
        };

        prop_assert!(config.validate().is_err());
    }

    /// Property: Evaluation result metadata is valid
    #[test]
    fn prop_metadata_valid(buffer in arb_buffer()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        let result = db.evaluate_buffer(&buffer)
            .expect("should evaluate");

        // File size should match buffer length
        prop_assert_eq!(result.metadata.file_size as usize, buffer.len());

        // Evaluation time should be non-negative
        prop_assert!(result.metadata.evaluation_time_ms >= 0.0);

        // Rules evaluated should be positive (built-in rules exist)
        prop_assert!(result.metadata.rules_evaluated > 0);
    }

    /// Property: Known magic patterns should be detected
    #[test]
    fn prop_elf_detection(_seed in any::<u64>()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        // ELF magic number
        let elf_buffer = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];

        let result = db.evaluate_buffer(&elf_buffer)
            .expect("should evaluate");

        // Should detect as ELF
        prop_assert!(
            result.description.contains("ELF"),
            "Expected ELF detection, got: {}",
            result.description
        );
    }

    /// Property: Known magic patterns should be detected (ZIP)
    #[test]
    fn prop_zip_detection(_seed in any::<u64>()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        // ZIP magic number
        let zip_buffer = vec![0x50, 0x4b, 0x03, 0x04];

        let result = db.evaluate_buffer(&zip_buffer)
            .expect("should evaluate");

        // Should detect as ZIP
        prop_assert!(
            result.description.contains("ZIP"),
            "Expected ZIP detection, got: {}",
            result.description
        );
    }

    /// Property: Arbitrary rules should serialize/deserialize consistently
    #[test]
    fn prop_rule_serde_roundtrip(rule in arb_magic_rule()) {
        // Serialize to JSON
        let json = serde_json::to_string(&rule)
            .expect("should serialize");

        // Deserialize back
        let deserialized: MagicRule = serde_json::from_str(&json)
            .expect("should deserialize");

        // Core fields should match
        prop_assert_eq!(rule.message, deserialized.message);
        prop_assert_eq!(rule.level, deserialized.level);
    }

    /// Property: OffsetSpec serialization roundtrip
    #[test]
    fn prop_offset_spec_serde(offset in arb_offset_spec()) {
        let json = serde_json::to_string(&offset)
            .expect("should serialize");

        let deserialized: OffsetSpec = serde_json::from_str(&json)
            .expect("should deserialize");

        prop_assert_eq!(offset, deserialized);
    }

    /// Property: TypeKind serialization roundtrip
    #[test]
    fn prop_type_kind_serde(type_kind in arb_type_kind()) {
        let json = serde_json::to_string(&type_kind)
            .expect("should serialize");

        let deserialized: TypeKind = serde_json::from_str(&json)
            .expect("should deserialize");

        prop_assert_eq!(type_kind, deserialized);
    }

    /// Property: Operator serialization roundtrip
    #[test]
    fn prop_operator_serde(operator in arb_operator()) {
        let json = serde_json::to_string(&operator)
            .expect("should serialize");

        let deserialized: Operator = serde_json::from_str(&json)
            .expect("should deserialize");

        prop_assert_eq!(operator, deserialized);
    }

    /// Property: Value serialization roundtrip
    #[test]
    fn prop_value_serde(value in arb_value()) {
        let json = serde_json::to_string(&value)
            .expect("should serialize");

        let deserialized: Value = serde_json::from_str(&json)
            .expect("should deserialize");

        prop_assert_eq!(value, deserialized);
    }

    /// Property: Buffer with random prefix still works (ELF)
    #[test]
    fn prop_random_prefix_handling_elf(
        prefix in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        // Create buffer with prefix followed by ELF magic
        let mut buffer = prefix;
        buffer.extend([0x7f, b'E', b'L', b'F']);

        // Should not panic
        let result = db.evaluate_buffer(&buffer);
        prop_assert!(result.is_ok());
    }

    /// Property: Buffer with random prefix still works (ZIP)
    #[test]
    fn prop_random_prefix_handling_zip(
        prefix in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        // Create buffer with prefix followed by ZIP magic
        let mut buffer = prefix;
        buffer.extend([0x50, 0x4b, 0x03, 0x04]);

        // Should not panic
        let result = db.evaluate_buffer(&buffer);
        prop_assert!(result.is_ok());
    }
}
