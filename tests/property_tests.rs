// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Property-based tests for libmagic-rs
//!
//! Uses proptest to verify properties that should hold for all valid inputs:
//! - Evaluator never panics on any buffer
//! - Buffer access is always bounds-checked
//! - Metadata is consistent
//! - Serde roundtrips preserve data

use proptest::prelude::*;

use libmagic_rs::parser::ast::PStringLengthWidth;
use libmagic_rs::{
    Endianness, EvaluationConfig, MagicDatabase, MagicRule, OffsetSpec, Operator, TypeKind, Value,
};

/// Generate a valid OffsetSpec for testing
fn arb_offset_spec() -> impl Strategy<Value = OffsetSpec> {
    prop_oneof![
        (-1000i64..=1000i64).prop_map(OffsetSpec::Absolute),
        (-100i64..=100i64).prop_map(OffsetSpec::Relative),
        (-100i64..=0i64).prop_map(OffsetSpec::FromEnd),
    ]
}

/// Generate a valid endianness for testing (includes Native)
fn arb_endianness() -> impl Strategy<Value = Endianness> {
    prop_oneof![
        Just(Endianness::Little),
        Just(Endianness::Big),
        Just(Endianness::Native),
    ]
}

/// Generate a valid TypeKind for testing
fn arb_type_kind() -> impl Strategy<Value = TypeKind> {
    prop_oneof![
        any::<bool>().prop_map(|signed| TypeKind::Byte { signed }),
        (arb_endianness(), any::<bool>())
            .prop_map(|(endian, signed)| { TypeKind::Short { endian, signed } }),
        (arb_endianness(), any::<bool>())
            .prop_map(|(endian, signed)| { TypeKind::Long { endian, signed } }),
        (arb_endianness(), any::<bool>())
            .prop_map(|(endian, signed)| { TypeKind::Quad { endian, signed } }),
        arb_endianness().prop_map(|endian| TypeKind::Float { endian }),
        arb_endianness().prop_map(|endian| TypeKind::Double { endian }),
        (0usize..256usize).prop_map(|len| TypeKind::String {
            max_length: Some(len),
        }),
        (
            0usize..256usize,
            prop_oneof![
                Just(PStringLengthWidth::OneByte),
                Just(PStringLengthWidth::TwoByteBE),
                Just(PStringLengthWidth::TwoByteLE),
                Just(PStringLengthWidth::FourByteBE),
                Just(PStringLengthWidth::FourByteLE),
            ],
            any::<bool>(),
        )
            .prop_map(|(len, width, includes_self)| TypeKind::PString {
                max_length: Some(len),
                length_width: width,
                length_includes_itself: includes_self,
            }),
        {
            // Fair-weighted generator for `RegexCount`: each of the
            // four sub-states (Default, Bytes, Lines(Some), Lines(None))
            // gets roughly equal sampling via `prop_oneof!`. The old
            // uniform `0..4` dispatch gave Lines 2x weight and further
            // collapsed Bytes into Default on a None raw_count, leaving
            // Bytes at ~12.5% effective sample rate. Under the new
            // weighting each variant fires on ~25% of samples.
            let count_strategy = prop_oneof![
                Just(libmagic_rs::parser::ast::RegexCount::Default),
                (1u32..=4096u32).prop_map(|n| libmagic_rs::parser::ast::RegexCount::Bytes(
                    ::std::num::NonZeroU32::new(n).expect("range excludes 0")
                )),
                (1u32..=4096u32).prop_map(|n| libmagic_rs::parser::ast::RegexCount::Lines(
                    ::std::num::NonZeroU32::new(n)
                )),
                Just(libmagic_rs::parser::ast::RegexCount::Lines(None)),
            ];
            (any::<bool>(), any::<bool>(), count_strategy).prop_map(
                |(case_insensitive, start_offset, count)| TypeKind::Regex {
                    flags: libmagic_rs::parser::ast::RegexFlags {
                        case_insensitive,
                        start_offset,
                    },
                    count,
                },
            )
        },
        (1usize..=4096usize).prop_map(|range| TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(range).unwrap(),
        }),
    ]
}

/// Generate a valid Operator for testing
fn arb_operator() -> impl Strategy<Value = Operator> {
    prop_oneof![
        Just(Operator::Equal),
        Just(Operator::NotEqual),
        Just(Operator::LessThan),
        Just(Operator::GreaterThan),
        Just(Operator::LessEqual),
        Just(Operator::GreaterEqual),
        Just(Operator::BitwiseAnd),
        (0u64..=255u64).prop_map(Operator::BitwiseAndMask),
        Just(Operator::BitwiseXor),
        Just(Operator::BitwiseNot),
        Just(Operator::AnyValue),
    ]
}

/// Generate a valid Value for testing
fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        (0u64..=u32::MAX as u64).prop_map(Value::Uint),
        (i32::MIN as i64..=i32::MAX as i64).prop_map(Value::Int),
        (-1e10f64..1e10f64).prop_map(Value::Float),
        prop::collection::vec(any::<u8>(), 0..32).prop_map(Value::Bytes),
        "[a-zA-Z0-9 ]{0,32}".prop_map(Value::String),
    ]
}

/// Generate a valid MagicRule for testing
fn arb_magic_rule() -> impl Strategy<Value = MagicRule> {
    (
        arb_offset_spec(),
        arb_type_kind(),
        arb_operator(),
        arb_value(),
        "[a-zA-Z0-9 _-]{1,64}",
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
    /// Property: Evaluation should never panic on any valid buffer
    #[test]
    fn prop_evaluation_never_panics(buffer in arb_buffer()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        let result = db.evaluate_buffer(&buffer);

        match result {
            Ok(eval_result) => {
                prop_assert!(!eval_result.description.is_empty());
                prop_assert!(eval_result.confidence >= 0.0);
                prop_assert!(eval_result.confidence <= 1.0);
            }
            Err(e) => {
                prop_assert!(!e.to_string().is_empty());
            }
        }
    }

    /// Property: EvaluationConfig validation accepts reasonable values
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

        prop_assert!(config.validate().is_ok());
    }

    /// Property: Evaluation result metadata is consistent with input
    #[test]
    fn prop_metadata_valid(buffer in arb_buffer()) {
        let db = MagicDatabase::with_builtin_rules()
            .expect("builtin rules should load");

        let result = db.evaluate_buffer(&buffer)
            .expect("should evaluate");

        prop_assert_eq!(result.metadata.file_size as usize, buffer.len());
        prop_assert!(result.metadata.evaluation_time_ms >= 0.0);
        prop_assert!(result.metadata.rules_evaluated > 0);
    }

    /// Property: Arbitrary rules should serialize/deserialize consistently
    #[test]
    fn prop_rule_serde_roundtrip(rule in arb_magic_rule()) {
        let json = serde_json::to_string(&rule)
            .expect("should serialize");

        let deserialized: MagicRule = serde_json::from_str(&json)
            .expect("should deserialize");

        prop_assert_eq!(rule.message, deserialized.message);
        prop_assert_eq!(rule.level, deserialized.level);
    }

    /// Parser must never panic on arbitrary input (bytes converted via lossy UTF-8).
    #[test]
    fn prop_parser_never_panics_on_arbitrary_input(
        input in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        let text = String::from_utf8_lossy(&input);
        let _ = libmagic_rs::parser::parse_text_magic_file(&text);
    }

    /// Evaluator must never panic on arbitrary (rule, buffer) pairs, with a 1-second timeout guard.
    #[test]
    fn prop_arbitrary_rule_evaluation_never_panics(
        rule in arb_magic_rule(),
        buffer in prop::collection::vec(any::<u8>(), 0..1024)
    ) {
        use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
        let config = EvaluationConfig {
            timeout_ms: Some(1000),
            ..EvaluationConfig::default()
        };
        let mut context = EvaluationContext::new(config);
        let _ = evaluate_rules(&[rule], &buffer, &mut context);
    }
}

// =============================================================================
// Known-pattern detection (regular tests, not property tests)
// =============================================================================

#[test]
fn test_elf_detection() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");
    let elf_buffer = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];

    let result = db.evaluate_buffer(&elf_buffer).expect("should evaluate");
    assert!(
        result.description.contains("ELF"),
        "Expected ELF detection, got: {}",
        result.description
    );
}

#[test]
fn test_zip_detection() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");
    let zip_buffer = vec![0x50, 0x4b, 0x03, 0x04];

    let result = db.evaluate_buffer(&zip_buffer).expect("should evaluate");
    assert!(
        result.description.contains("ZIP"),
        "Expected ZIP detection, got: {}",
        result.description
    );
}

#[test]
fn test_empty_buffer_handled() {
    let db = MagicDatabase::with_builtin_rules().expect("builtin rules should load");

    let result = db.evaluate_buffer(&[]).expect("should evaluate");
    assert!(!result.description.is_empty());
}

#[test]
fn test_zero_recursion_fails_validation() {
    let config = EvaluationConfig {
        max_recursion_depth: 0,
        ..EvaluationConfig::default()
    };

    assert!(config.validate().is_err());
}
