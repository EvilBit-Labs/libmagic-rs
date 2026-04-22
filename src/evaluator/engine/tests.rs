// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::{Endianness, OffsetSpec, Operator, TypeKind, Value};

/// Legacy one-shot single-rule helper used by the engine unit tests.
///
/// The public [`evaluate_single_rule`] API was reshaped in todo 025 to
/// accept a mutable [`EvaluationContext`] and return `Vec<RuleMatch>` by
/// delegating through [`evaluate_rules`]. That delegation folds
/// data-dependent errors (buffer overrun, invalid offset, etc.) into an
/// empty vector -- great for library callers, but many of the tests
/// below were written against the older raw evaluator which returned
/// `Result<Option<(usize, Value)>, LibmagicError>` and specifically
/// asserted the `Err` path on out-of-bounds reads. This helper preserves
/// that lower-level contract so the historical tests keep exercising the
/// raw evaluator semantics without being rewritten en masse; the new
/// public surface is covered by its own targeted tests.
fn evaluate_single_rule_legacy(
    rule: &MagicRule,
    buffer: &[u8],
) -> Result<Option<(usize, crate::parser::ast::Value)>, LibmagicError> {
    evaluate_single_rule_with_anchor(rule, buffer, 0, 0)
}

#[test]
fn test_evaluate_single_rule_relative_resolves_against_anchor_zero() {
    // Public evaluate_single_rule has no EvaluationContext, so OffsetSpec::Relative
    // resolves against an implicit anchor of 0 -- equivalent to absolute offset N.
    // Pin this contract so a future refactor cannot silently regress to UnsupportedType.
    let rule = MagicRule {
        offset: OffsetSpec::Relative(3),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xCC),
        message: "relative-no-context".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    // Anchor=0 + delta 3 -> reads at absolute offset 3.
    let buffer = &[0xAA, 0xBB, 0xDD, 0xCC, 0xEE];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "evaluate_single_rule with Relative(3) should resolve to absolute 3"
    );
    let (offset, value) = result.unwrap();
    assert_eq!(offset, 3);
    assert_eq!(value, Value::Uint(0xCC));
}

#[test]
fn test_evaluate_rules_anchor_near_saturation_skips_relative_child_gracefully() {
    // Pin the contract that an anchor at or near `usize::MAX` does not
    // panic and instead causes subsequent Relative rules to fail bounds
    // checks gracefully. We can't construct a real match at usize::MAX
    // (no realistic buffer is that big), so inject the saturated anchor
    // directly via the pub(crate) setter and then evaluate a Relative rule.
    use crate::EvaluationConfig;
    use crate::evaluator::EvaluationContext;

    let buffer = [0xAA, 0xBB, 0xCC, 0xDD];
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX);

    // Relative(0) -> target = usize::MAX, which is >= buffer.len() and
    // returns BufferOverrun -> graceful skip in evaluate_rules.
    let rule_zero = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-zero-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let matches = evaluate_rules(&[rule_zero], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(0) at usize::MAX anchor must skip, not match or panic"
    );

    // Relative(+1) -> checked_add_signed -> overflow -> InvalidOffset -> skip.
    ctx.set_last_match_end(usize::MAX);
    let rule_pos = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-plus-one-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let matches = evaluate_rules(&[rule_pos], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(+1) at usize::MAX anchor must skip via InvalidOffset, not panic"
    );

    // Relative(-N) where N is small -> usize::MAX - N, still >= buffer.len() -> skip.
    ctx.set_last_match_end(usize::MAX);
    let rule_neg = MagicRule {
        offset: OffsetSpec::Relative(-1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-minus-one-near-sat".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let matches = evaluate_rules(&[rule_neg], &buffer, &mut ctx).unwrap();
    assert!(
        matches.is_empty(),
        "Relative(-1) at usize::MAX anchor must skip, not panic"
    );
}

#[test]
fn test_evaluate_single_rule_relative_negative_with_zero_anchor_errors() {
    // Public evaluate_single_rule uses an implicit anchor of 0. A negative
    // Relative delta underflows the anchor and must return
    // EvaluationError::InvalidOffset -- NOT Ok(None) (the "no match" path)
    // and NOT Absolute(-N)-style from-end semantics. Pin the contract so a
    // future refactor can't silently convert this to a graceful skip.
    use crate::LibmagicError;
    use crate::error::EvaluationError;

    let rule = MagicRule {
        offset: OffsetSpec::Relative(-1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-neg-top-level".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let buffer = &[0xAA, 0xBB];
    let err = evaluate_single_rule_legacy(&rule, buffer).unwrap_err();
    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset { offset: -1 })
        ),
        "Relative(-1) at anchor 0 must Err(InvalidOffset), got {err:?}"
    );
}

#[test]
fn test_evaluate_single_rule_relative_zero_resolves_to_buffer_start() {
    // Relative(0) with anchor=0 resolves to absolute 0.
    let rule = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "relative-zero".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xAA, 0xBB];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap().unwrap();
    assert_eq!(result.0, 0);
}

#[test]
fn test_evaluate_single_rule_byte_equal_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_byte_equal_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x50, 0x4b, 0x03, 0x04]; // ZIP magic bytes
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_byte_not_equal_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x00),
        message: "Non-zero byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0x7f != 0x00
}

#[test]
fn test_evaluate_single_rule_byte_not_equal_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x7f),
        message: "Not ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_none()); // 0x7f == 0x7f, so NotEqual is false
}

#[test]
fn test_evaluate_single_rule_byte_bitwise_and_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80), // Check if high bit is set
        message: "High bit set".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0x45, 0x4c, 0x46]; // 0xff has high bit set
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some()); // 0xff & 0x80 = 0x80 (non-zero)
}

#[test]
fn test_evaluate_single_rule_byte_bitwise_and_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80), // Check if high bit is set
        message: "High bit set".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // 0x7f has high bit clear
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_none()); // 0x7f & 0x80 = 0x00 (zero)
}

#[test]
fn test_evaluate_single_rule_short_little_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Little-endian short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x34, 0x12, 0x56, 0x78]; // 0x1234 in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_big_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Big-endian short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x1234 in big-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_signed_positive() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(32767), // 0x7fff
        message: "Positive signed short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0x7f, 0x00, 0x00]; // 0x7fff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_short_signed_negative() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-1), // 0xffff as signed
        message: "Negative signed short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0x00, 0x00]; // 0xffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_little_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Little-endian long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x78, 0x56, 0x34, 0x12, 0x00]; // 0x12345678 in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_big_endian() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Big-endian long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78, 0x00]; // 0x12345678 in big-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_signed_positive() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(2_147_483_647), // 0x7fffffff
        message: "Positive signed long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0x7f, 0x00]; // 0x7fffffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_long_signed_negative() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-1), // 0xffffffff as signed
        message: "Negative signed long".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0xff, 0xff, 0xff, 0xff, 0x00]; // 0xffffffff in little-endian
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_different_offsets() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "ELF class byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_negative_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(-1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x46),
        message: "Last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_from_end_offset() {
    let rule = MagicRule {
        offset: OffsetSpec::FromEnd(-2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "Second to last byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_offset_out_of_bounds() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(10),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_short_insufficient_bytes() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234),
        message: "Insufficient bytes".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_long_insufficient_bytes() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "Insufficient bytes".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_empty_buffer() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Empty buffer".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[];
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Buffer overrun"));
        }
        _ => panic!("Expected EvaluationError"),
    }
}

#[test]
fn test_evaluate_single_rule_string_type_supported() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("test".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = b"test\x00 data";
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_some());

    let rule_no_match = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("hello".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let result = evaluate_single_rule_legacy(&rule_no_match, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_none());
}

#[test]
fn test_evaluate_single_rule_cross_type_comparison() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Int(42),
        message: "Cross-type comparison".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_shorts() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xff00),
        message: "High byte check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x34, 0x12];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_longs() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Big,
            signed: false,
        },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0xffff_0000),
        message: "High word check".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x12, 0x34, 0x56, 0x78];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_comprehensive_elf_check() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x464c_457f),
        message: "ELF executable".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let elf_buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let result = evaluate_single_rule_legacy(&rule, elf_buffer).unwrap();
    assert!(result.is_some());

    let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04, 0x14, 0x00];
    let result = evaluate_single_rule_legacy(&rule, non_elf_buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_native_endianness() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Short {
            endian: Endianness::Native,
            signed: false,
        },
        op: Operator::NotEqual,
        value: Value::Uint(0),
        message: "Non-zero native short".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x01, 0x02];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_all_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    let equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "Equal test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let not_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::NotEqual,
        value: Value::Uint(0x42),
        message: "NotEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&not_equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let bitwise_and_rule = MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Byte { signed: true },
        op: Operator::BitwiseAnd,
        value: Value::Uint(0x80),
        message: "BitwiseAnd test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&bitwise_and_rule, buffer)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_evaluate_single_rule_comparison_operators() {
    let buffer = &[0x42, 0x00, 0xff, 0x80];

    let less_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0x42),
        message: "LessThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&less_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    let greater_than_rule = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterThan,
        value: Value::Uint(0x42),
        message: "GreaterThan test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&greater_than_rule, buffer)
            .unwrap()
            .is_some()
    );

    let less_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessEqual,
        value: Value::Uint(0x42),
        message: "LessEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&less_equal_rule, buffer)
            .unwrap()
            .is_some()
    );

    let greater_equal_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterEqual,
        value: Value::Uint(0x42),
        message: "GreaterEqual test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&greater_equal_rule, buffer)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_evaluate_comparison_with_signed_byte() {
    let buffer = &[0x80];

    let signed_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "signed less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&signed_rule, buffer)
            .unwrap()
            .is_some()
    );

    let unsigned_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::LessThan,
        value: Value::Uint(0),
        message: "unsigned less".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    assert!(
        evaluate_single_rule_legacy(&unsigned_rule, buffer)
            .unwrap()
            .is_none()
    );
}

#[test]
fn test_evaluate_comparison_operators_negative_cases() {
    let buffer = &[0x42];

    let cases: Vec<(Operator, u64, bool)> = vec![
        (Operator::LessThan, 66, false),
        (Operator::LessThan, 67, true),
        (Operator::GreaterThan, 66, false),
        (Operator::GreaterThan, 65, true),
        (Operator::LessEqual, 65, false),
        (Operator::LessEqual, 66, true),
        (Operator::GreaterEqual, 67, false),
        (Operator::GreaterEqual, 66, true),
    ];

    for (op, value, expected) in cases {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: op.clone(),
            value: Value::Uint(value),
            message: "test".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };
        let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
        assert_eq!(
            result.is_some(),
            expected,
            "{op:?} with value {value}: expected {expected}"
        );
    }
}

#[test]
fn test_evaluate_single_rule_edge_case_values() {
    let max_uint_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0xffff_ffff),
        message: "Max uint32".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let max_buffer = &[0xff, 0xff, 0xff, 0xff];
    let result = evaluate_single_rule_legacy(&max_uint_rule, max_buffer).unwrap();
    assert!(result.is_some());

    let min_int_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        op: Operator::Equal,
        value: Value::Int(-2_147_483_648),
        message: "Min int32".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let min_buffer = &[0x00, 0x00, 0x00, 0x80];
    let result = evaluate_single_rule_legacy(&min_int_rule, min_buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_various_buffer_sizes() {
    let single_byte_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xaa),
        message: "Single byte".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let single_buffer = &[0xaa];
    let result = evaluate_single_rule_legacy(&single_byte_rule, single_buffer).unwrap();
    assert!(result.is_some());

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let large_buffer: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let large_rule = MagicRule {
        offset: OffsetSpec::Absolute(1000),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint((1000 % 256) as u64),
        message: "Large buffer".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let result = evaluate_single_rule_legacy(&large_rule, &large_buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_rules_empty_list() {
    let rules = vec![];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_single_matching_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF magic");
    assert_eq!(matches[0].offset, 0);
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[0].value, Value::Int(0x7f));
}

#[test]
fn test_evaluate_rules_single_non_matching_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50),
        message: "ZIP magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_multiple_rules_stop_at_first() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_list = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_list, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "First match");
}

#[test]
fn test_evaluate_rules_multiple_rules_find_all() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "First match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x45),
        message: "Second match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_set = vec![rule1, rule2];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_set, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "First match");
    assert_eq!(matches[1].message, "Second match");
}

#[test]
fn test_evaluate_rules_hierarchical_parent_child() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[1].level, 1);
}

#[test]
fn test_evaluate_rules_hierarchical_parent_no_match() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x50),
        message: "ZIP".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_evaluate_rules_hierarchical_parent_match_child_no_match() {
    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "32-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
}

#[test]
fn test_evaluate_rules_deep_hierarchy() {
    let grandchild_rule = MagicRule {
        offset: OffsetSpec::Absolute(5),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "little-endian".to_string(),
        children: vec![],
        level: 2,
        strength_modifier: None,
    };

    let child_rule = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![grandchild_rule],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child_rule],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[0].level, 0);
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[1].level, 1);
    assert_eq!(matches[2].message, "little-endian");
    assert_eq!(matches[2].level, 2);
}

#[test]
fn test_evaluate_rules_multiple_children() {
    let child1 = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x02),
        message: "64-bit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let child2 = MagicRule {
        offset: OffsetSpec::Absolute(5),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "little-endian".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };

    let parent_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF".to_string(),
        children: vec![child1, child2],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![parent_rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "ELF");
    assert_eq!(matches[1].message, "64-bit");
    assert_eq!(matches[2].message, "little-endian");
}

#[test]
fn test_evaluate_rules_recursion_depth_limit() {
    let mut current_rule = MagicRule {
        offset: OffsetSpec::Absolute(10),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Deep level".to_string(),
        children: vec![],
        level: 10,
        strength_modifier: None,
    };

    for i in (0u32..10u32).rev() {
        current_rule = MagicRule {
            offset: OffsetSpec::Absolute(i64::from(i)),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i)),
            message: format!("Level {i}"),
            children: vec![current_rule],
            level: i,
            strength_modifier: None,
        };
    }

    let rules = vec![current_rule];
    let buffer = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    let config = EvaluationConfig {
        max_recursion_depth: 5,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(msg) => {
            let error_string = format!("{msg}");
            assert!(error_string.contains("Recursion limit exceeded"));
        }
        _ => panic!("Expected EvaluationError for recursion limit"),
    }
}

#[test]
fn test_evaluate_rules_with_config_convenience() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();

    let matches = evaluate_rules_with_config(&rules, buffer, &config).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "ELF magic");
}

#[test]
fn test_evaluate_rules_timeout() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        timeout_ms: Some(0),
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(
        matches!(result, Err(LibmagicError::Timeout { timeout_ms: 0 })),
        "Expected timeout error, got: {result:?}"
    );
}

#[test]
fn test_evaluate_rules_empty_buffer() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_ok());

    let matches = result.unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_evaluate_rules_mixed_matching_non_matching() {
    let rule1 = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Matches".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule2 = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x99),
        message: "Doesn't match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule3 = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x4c),
        message: "Also matches".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rule_collection = vec![rule1, rule2, rule3];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rule_collection, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Matches");
    assert_eq!(matches[1].message, "Also matches");
}

#[test]
fn test_evaluate_rules_context_state_preservation() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let rules = vec![rule];
    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    context.set_current_offset(100);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}

#[test]
fn test_error_recovery_skip_problematic_rules() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Another valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid rule");
    assert_eq!(matches[1].message, "Another valid rule");
}

#[test]
fn test_error_recovery_child_rule_failures() {
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Parent rule".to_string(),
        children: vec![
            MagicRule {
                offset: OffsetSpec::Absolute(1),
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x45),
                message: "Valid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
            MagicRule {
                offset: OffsetSpec::Absolute(100),
                typ: TypeKind::Byte { signed: true },
                op: Operator::Equal,
                value: Value::Uint(0x00),
                message: "Invalid child".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
        ],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Parent rule");
    assert_eq!(matches[1].message, "Valid child");
}

#[test]
fn test_error_recovery_mixed_rule_types() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid byte".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(3),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234),
            message: "Invalid short".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::String {
                max_length: Some(3),
            },
            op: Operator::Equal,
            value: Value::String("ELF".to_string()),
            message: "Valid string".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, b'E', b'L', b'F'];
    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "Valid byte");
    assert_eq!(matches[1].message, "Valid string");
}

#[test]
fn test_error_recovery_all_rules_fail() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Out of bounds".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(2),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0x1234_5678),
            message: "Insufficient bytes".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_error_recovery_timeout_propagation() {
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Test rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 10,
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: Some(0),
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);

    assert!(
        matches!(result, Err(LibmagicError::Timeout { timeout_ms: 0 })),
        "Expected timeout error, got: {result:?}"
    );
}

#[test]
fn test_error_recovery_recursion_limit_propagation() {
    let rules = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Parent".to_string(),
        children: vec![MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Child".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
        }],
        level: 0,
        strength_modifier: None,
    }];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig {
        max_recursion_depth: 0,
        max_string_length: 1024,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: None,
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, buffer, &mut context);
    assert!(result.is_err());

    match result.unwrap_err() {
        LibmagicError::EvaluationError(crate::error::EvaluationError::RecursionLimitExceeded {
            ..
        }) => {}
        _ => panic!("Expected recursion limit error"),
    }
}

#[test]
fn test_error_recovery_preserves_context_state() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    context.set_current_offset(42);
    let initial_offset = context.current_offset();
    let initial_depth = context.recursion_depth();

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 1);

    assert_eq!(context.current_offset(), initial_offset);
    assert_eq!(context.recursion_depth(), initial_depth);
}

#[test]
fn test_any_value_parse_and_evaluate_paren_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = ">0 byte x (0)";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "(0)");

    let buffer = &[0x00, 0x01, 0x02, 0x03];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_any_value_parse_and_evaluate_backslash_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 long x \\b, data";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "\\b, data");

    let buffer = &[0xFF, 0xFE, 0xFD, 0xFC];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_any_value_parse_and_evaluate_no_message() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte x";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::AnyValue);

    let buffer = &[0x42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "AnyValue rule should match unconditionally"
    );
}

#[test]
fn test_bitwise_xor_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte ^0x01 XOR match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseXor);
    assert_eq!(rule.message, "XOR match");

    let buffer = &[0x0F];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "BitwiseXor should match when XOR is non-zero"
    );
}

#[test]
fn test_bitwise_xor_parse_and_evaluate_no_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 byte ^0x42 XOR no match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseXor);

    let buffer = &[0x42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseXor should not match when XOR is zero"
    );
}

#[test]
fn test_bitwise_not_parse_and_evaluate_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 ubyte ~0xFF NOT match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);
    assert_eq!(rule.message, "NOT match");

    let buffer = &[0x00];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "BitwiseNot should match when NOT(value) equals operand at byte width"
    );
}

#[test]
fn test_bitwise_not_parse_and_evaluate_no_match() {
    use crate::parser::grammar::parse_magic_rule;

    let input = "0 ubyte ~0x01 NOT no match";
    let (_, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(rule.op, Operator::BitwiseNot);

    let buffer = &[0x42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "BitwiseNot should not match when NOT(value) != operand"
    );
}

#[test]
fn test_evaluate_rules_skips_out_of_bounds_rule() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(100),
        typ: TypeKind::Byte { signed: true },
        op: Operator::Equal,
        value: Value::Uint(0x00),
        message: "Out of bounds rule".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[0x7f, 0x45];

    let single_result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(single_result.is_err());

    let rules = vec![rule];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_mixed_valid_and_invalid_rules_yield_valid_matches() {
    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "Valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(100),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "Invalid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x45),
            message: "Another valid rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let buffer = &[0x7f, 0x45, 0x4c, 0x46];

    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
}

// ============================================================
// PString (Pascal string) Tests
// ============================================================

#[test]
fn test_evaluate_single_rule_pstring_match() {
    // Pascal string: length byte (5) followed by "Hello"
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("Hello".to_string()),
        message: "Pascal string detected".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[5, b'H', b'e', b'l', b'l', b'o'];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_some(),
        "PString rule should match when buffer contains matching pascal string"
    );
}

#[test]
fn test_evaluate_single_rule_pstring_no_match() {
    // Pascal string in buffer is "Hello", rule expects "World"
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("World".to_string()),
        message: "Should not match".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let buffer = &[5, b'H', b'e', b'l', b'l', b'o'];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(
        result.is_none(),
        "PString rule should not match when strings differ"
    );
}

#[test]
fn test_evaluate_single_rule_pstring_with_child_rule() {
    // Parent: PString at offset 0 matches "ELF"
    // Child: byte at offset 4 equals 0x02
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("ELF".to_string()),
        message: "Pascal ELF".to_string(),
        children: vec![MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x02),
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
        }],
        level: 0,
        strength_modifier: None,
    };

    // Buffer: length=3, "ELF", then 0x02 at offset 4
    let buffer = &[3, b'E', b'L', b'F', 0x02];
    let config = EvaluationConfig::default();
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[rule], buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "Both parent PString and child byte rules should match"
    );
}

// ============================================================
// Deep nesting & resource exhaustion safety tests (todo 028)
// ============================================================

/// Build a linear chain of `depth` nested rules, each reading a distinct byte.
/// Level 0 reads buffer[0], level 1 reads buffer[1], ..., level (depth-1) reads
/// buffer[depth-1]. The buffer is constructed so every level matches.
fn build_linear_nested_chain(depth: u32) -> (MagicRule, Vec<u8>) {
    assert!(depth > 0, "depth must be > 0");
    let buffer: Vec<u8> = (0..depth).map(|i| (i & 0xFF) as u8).collect();

    // Start with the deepest (innermost) rule and build outward.
    let last = depth - 1;
    let mut current = MagicRule {
        offset: OffsetSpec::Absolute(i64::from(last)),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(last & 0xFF)),
        message: format!("Level {last}"),
        children: vec![],
        level: last,
        strength_modifier: None,
    };

    for i in (0..last).rev() {
        current = MagicRule {
            offset: OffsetSpec::Absolute(i64::from(i)),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i & 0xFF)),
            message: format!("Level {i}"),
            children: vec![current],
            level: i,
            strength_modifier: None,
        };
    }

    (current, buffer)
}

#[test]
fn test_deep_nesting_twenty_levels_all_match() {
    // Build a 20-level linear nested rule tree. With max_recursion_depth=20,
    // every level should match. A 20-level tree only needs 19 increments of
    // the recursion counter (the leaf has no children), so it fits under the
    // default limit exactly.
    let (root, buffer) = build_linear_nested_chain(20);
    let rules = vec![root];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&rules, &buffer, &mut context)
        .expect("20-level chain should evaluate without error under default limit");

    assert_eq!(
        matches.len(),
        20,
        "Every one of 20 nested levels should match, got {}",
        matches.len()
    );
    for (i, m) in matches.iter().enumerate() {
        assert_eq!(m.message, format!("Level {i}"));
    }
}

#[test]
fn test_deep_nesting_exceeds_limit_returns_recursion_error() {
    // Same 20-level tree, but with max_recursion_depth=5. Evaluation must
    // return RecursionLimitExceeded and must not panic.
    let (root, buffer) = build_linear_nested_chain(20);
    let rules = vec![root];

    let config = EvaluationConfig {
        max_recursion_depth: 5,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let result = evaluate_rules(&rules, &buffer, &mut context);
    let err = result.expect_err("Expected RecursionLimitExceeded for 20-level tree with limit 5");

    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            )
        ),
        "Expected RecursionLimitExceeded, got: {err:?}"
    );
}

#[test]
fn test_resource_exhaustion_large_rule_count_completes_or_times_out() {
    // Generate 2000 independent flat rules. Each rule reads byte 0 and
    // compares against a distinct value; only one will match. Evaluation
    // must either complete successfully or return a Timeout error -- it
    // must never panic and must finish well under the test timeout.
    let rule_count: u32 = 2000;
    let rules: Vec<MagicRule> = (0..rule_count)
        .map(|i| MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(u64::from(i & 0xFF)),
            message: format!("Rule {i}"),
            children: vec![],
            level: 0,
            strength_modifier: None,
        })
        .collect();

    // Buffer containing 0x00 so exactly the rules with value 0 match
    // (there will be several because we mask with 0xFF above).
    let buffer = vec![0u8; 64];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        // Generous timeout: even CI should finish 2000 trivial byte reads
        // in well under 10 seconds.
        timeout_ms: Some(10_000),
    };
    let mut context = EvaluationContext::new(config);

    let start = std::time::Instant::now();
    let result = evaluate_rules(&rules, &buffer, &mut context);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "Large-rule-count evaluation took too long: {elapsed:?}"
    );

    match result {
        Ok(matches) => {
            // Rules whose (i & 0xFF) == 0 match byte 0: that's rule_count/256
            // rounded up. Verify we got at least one match and did not panic.
            assert!(
                !matches.is_empty(),
                "Expected at least one match in large rule set"
            );
        }
        Err(LibmagicError::Timeout { .. }) => {
            // Acceptable: the timeout fired instead of completing.
        }
        Err(e) => panic!("Unexpected error from large rule set evaluation: {e:?}"),
    }
}

#[test]
fn test_resource_exhaustion_large_buffer_completes_without_panic() {
    // 1 MiB buffer of zeros. Evaluate a handful of rules across it and
    // verify no panic and reasonable runtime.
    let buffer = vec![0u8; 1024 * 1024];

    let rules = vec![
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "zero byte at 0".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Uint(0),
            message: "zero long at 0".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(512 * 1024),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "zero byte at mid".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
        // Out-of-bounds rule should fail gracefully, not panic.
        MagicRule {
            offset: OffsetSpec::Absolute(i64::try_from(buffer.len() + 100).unwrap()),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0x00),
            message: "out of bounds".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        },
    ];

    let config = EvaluationConfig {
        max_recursion_depth: 20,
        max_string_length: 8192,
        stop_at_first_match: false,
        enable_mime_types: false,
        timeout_ms: Some(10_000),
    };
    let mut context = EvaluationContext::new(config);

    let start = std::time::Instant::now();
    let matches = evaluate_rules(&rules, &buffer, &mut context)
        .expect("Large-buffer evaluation should not return an error");
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "Large-buffer evaluation took too long: {elapsed:?}"
    );

    // Three in-bounds rules should match; the out-of-bounds rule should
    // silently fail without panicking.
    assert_eq!(
        matches.len(),
        3,
        "Expected 3 matches (in-bounds rules only), got {}",
        matches.len()
    );
}

/// A regex rule whose pattern contains metacharacters must succeed when the
/// pattern actually matches the buffer. Prior to this fix, the engine compared
/// the matched text (e.g., "123") against the pattern literal ("[0-9]+") via
/// `apply_operator`, which failed for any real regex.
#[test]
fn test_regex_rule_with_metacharacters_matches() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("[0-9]+".to_string()),
        message: "has digits".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"abc123def", &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "has digits");
}

/// A regex rule whose pattern does not match must not match, confirming that
/// the logical-match shortcut only fires on a non-empty reader result.
#[test]
fn test_regex_rule_with_metacharacters_no_match() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("[0-9]+".to_string()),
        message: "has digits".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"abcdef", &mut context).unwrap();
    assert!(matches.is_empty());
}

/// A search rule with `Operator::NotEqual` succeeds only when the literal
/// pattern is absent from the window.
#[test]
fn test_search_rule_not_equal_succeeds_when_pattern_absent() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(64).unwrap(),
        },
        op: Operator::NotEqual,
        value: Value::String("needle".to_string()),
        message: "no needle".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_single_rule(&rule, b"plain haystack", &mut context).unwrap();
    assert_eq!(matches.len(), 1);
}

/// A non-Equal/NotEqual operator on a pattern-bearing type must surface as
/// a hard error, not silently produce an ordering comparison against the
/// pattern source text. Pre-fix, `regex > "[0-9]+"` matched by coincidence
/// whenever the empty "no match" sentinel happened to lexicographically
/// exceed the pattern literal.
#[test]
fn test_regex_rule_with_ordering_operator_is_rejected() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::GreaterThan,
        value: Value::String("[0-9]+".to_string()),
        message: "bogus".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_single_rule(&rule, b"abcdef", &mut context);
    match result {
        Err(LibmagicError::EvaluationError(_)) => {}
        other => panic!("expected EvaluationError for ordering operator on regex, got {other:?}"),
    }
}

#[test]
fn test_search_rule_with_bitwise_operator_is_rejected() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(32).unwrap(),
        },
        op: Operator::BitwiseAnd,
        value: Value::String("needle".to_string()),
        message: "bogus".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_single_rule(&rule, b"plain haystack", &mut context);
    assert!(
        matches!(result, Err(LibmagicError::EvaluationError(_))),
        "expected EvaluationError for bitwise operator on search"
    );
}

/// A child rule with `OffsetSpec::Relative(0)` after a parent regex match
/// must resolve to `parent_absolute_offset + match_length`, so the byte the
/// child reads is the first byte *after* the parent's match. This is the
/// regression test GOTCHAS 2.1 warns about: if `bytes_consumed_with_pattern`
/// returns the wrong number for `TypeKind::Regex`, the child lands at the
/// wrong offset and either misses or matches the wrong byte.
#[test]
fn test_regex_parent_advances_anchor_for_relative_child() {
    // Buffer: "abc123X" -- parent regex "abc" matches bytes 0..3, so a
    // Relative(0) child should read byte 3 = '1' (0x31). A Relative(-1)
    // child would read byte 2 = 'c' (0x63).
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'1')),
        message: "first digit".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("abc".to_string()),
        message: "abc prefix".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc123X", &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "expected parent + child match, got {}: {matches:?}",
        matches.len()
    );
    assert_eq!(matches[0].message, "abc prefix");
    assert_eq!(matches[1].message, "first digit");
}

/// A child rule with `OffsetSpec::Relative(0)` after a parent search match
/// must land at `match_index + pattern.len()` — NOT at `window_end` (the
/// pre-fix window-size advance would land on a completely different byte).
#[test]
fn test_search_parent_advances_anchor_to_match_end_not_window_end() {
    // Buffer: "XXXneedleYY_ZZ" -- parent `search/32 "needle"` finds the
    // pattern at index 3, length 6, match-end = 9. A Relative(0) child
    // should read byte 9 = 'Y' (0x59). With the bug, the anchor would
    // advance by 32 bytes (way past the buffer) or (with range=14) by 14
    // to index 14 which is past the buffer end.
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'Y')),
        message: "trailing Y".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(14).unwrap(),
        },
        op: Operator::Equal,
        value: Value::String("needle".to_string()),
        message: "found needle".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"XXXneedleYY_ZZ", &mut context).unwrap();
    assert_eq!(matches.len(), 2, "expected parent + child, got {matches:?}");
    assert_eq!(matches[1].message, "trailing Y");
}

/// Sanity check the negative: when the parent search finds the pattern
/// early in the window, a Relative(-N) child should still resolve against
/// the match-end anchor. This catches a class of bugs where the anchor
/// update uses the wrong base offset.
#[test]
fn test_search_parent_relative_child_at_positive_offset() {
    // Buffer: "prefix_NEEDLE_after_stuff" -- "NEEDLE" is at index 7, len
    // 6, match-end = 13. A Relative(1) child should read byte 14 = 'a'.
    let child = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "a after".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(32).unwrap(),
        },
        op: Operator::Equal,
        value: Value::String("NEEDLE".to_string()),
        message: "found".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"prefix_NEEDLE_after_stuff", &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].message, "a after");
}

// =============================================================================
// Tests for MetaType::Use semantics
// =============================================================================

use crate::evaluator::RuleEnvironment;
use crate::parser::ast::MetaType;
use crate::parser::name_table::NameTable;

/// Build an `EvaluationContext` with the supplied name table and (optional)
/// root-rules list. The root-rules list is retained for parity with the
/// `RuleEnvironment` shape even though `MetaType::Use` itself does not
/// consult it.
fn make_context_with_env(name_table: NameTable, root_rules: &[MagicRule]) -> EvaluationContext {
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(name_table),
        root_rules: std::sync::Arc::from(root_rules),
    });
    EvaluationContext::new(EvaluationConfig::default()).with_rule_env(env)
}

/// Minimal helper: wrap a `TypeKind::Meta(MetaType::Use(name))` rule at
/// offset 0 with the given `message` and empty child list.
fn use_rule(name: &str) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Use(name.to_string())),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: format!("use {name}"),
        children: vec![],
        level: 0,
        strength_modifier: None,
    }
}

/// Construct a name table from `(name, subroutine_rules)` pairs.
fn build_name_table(entries: Vec<(&str, Vec<MagicRule>)>) -> NameTable {
    // Build via the extraction helper so the table construction matches the
    // real parser path. Wrap each entry in a Name rule whose `children` are
    // the subroutine body.
    let mut top = Vec::new();
    for (name, body) in entries {
        top.push(MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Meta(MetaType::Name(name.to_string())),
            op: Operator::Equal,
            value: Value::Uint(0),
            message: String::new(),
            children: body,
            level: 0,
            strength_modifier: None,
        });
    }
    let (_rules, table) = crate::parser::name_table::extract_name_table(top);
    table
}

#[test]
fn test_use_known_name_evaluates_subroutine() {
    // The subroutine `part2` reads byte 3 and expects 0x42.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(3),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "sub-match".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    }];
    let table = build_name_table(vec![("part2", subroutine)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0x00u8, 0x00, 0x00, 0x42, 0x00];
    let rules = vec![use_rule("part2")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "subroutine should produce exactly one match"
    );
    assert_eq!(matches[0].message, "sub-match");
}

#[test]
fn test_use_unknown_name_returns_no_match() {
    // Empty name table so the lookup fails; the evaluator should not panic
    // and should produce zero matches.
    let table = NameTable::empty();
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0x00u8, 0x42];
    let rules = vec![use_rule("nonexistent")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(matches.is_empty(), "unknown name should yield no matches");
}

#[test]
fn test_use_without_rule_env_returns_no_match() {
    // A default context has no rule_env attached; `use` rules should be
    // silent no-ops in that case rather than returning an error or panicking.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let buffer = [0x00u8, 0x42];
    let rules = vec![use_rule("part2")];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "Use with no rule_env should produce no matches"
    );
}

#[test]
fn test_use_recursion_limit() {
    // Build a mutually-recursive pair: subroutine A calls B, B calls A.
    // With the default recursion limit, this should surface as
    // `RecursionLimitExceeded` rather than a stack overflow.
    let a_body = vec![use_rule("b")];
    let b_body = vec![use_rule("a")];
    let table = build_name_table(vec![("a", a_body), ("b", b_body)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0u8; 8];
    let rules = vec![use_rule("a")];
    let result = evaluate_rules(&rules, &buffer, &mut context);
    assert!(
        matches!(
            result,
            Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            ))
        ),
        "mutual recursion through use must surface RecursionLimitExceeded, got {result:?}"
    );
}

#[test]
fn test_use_child_rules_evaluated_after_subroutine() {
    // `Use` itself does not expose a visible RuleMatch today, so we cover
    // the "subroutine matches come first" invariant by verifying that the
    // subroutine's match appears in the output and is followed by a
    // sibling rule's match in the surrounding scope.
    //
    // `EvaluationConfig::default()` sets `stop_at_first_match = true`, which
    // (correctly, after the Comment 2 fix) short-circuits sibling iteration
    // once the `use` path produces a match. To exercise the ordering
    // invariant between the subroutine and its sibling we opt into the
    // "completeness" semantics by disabling first-match short-circuit.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "sibling".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let rules = vec![use_rule("sub"), sibling];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].message, "sub-head");
    assert_eq!(matches[1].message, "sibling");
}

#[test]
fn test_use_stop_at_first_match_short_circuits_siblings() {
    // Comment 2 regression guard: with the default
    // `stop_at_first_match = true` config, a successful `use` subroutine
    // must prevent later sibling top-level rules from being evaluated,
    // matching the short-circuit semantics every other rule kind obeys.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    let mut context = make_context_with_env(table, &[]);

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "sibling".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let rules = vec![use_rule("sub"), sibling];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "stop-at-first-match must halt sibling iteration once the use path produces a match"
    );
    assert_eq!(matches[0].message, "sub-head");
}

#[test]
fn test_use_rule_children_are_evaluated() {
    // Comment 1 regression guard: a `use` rule with its own children must
    // descend into those children after the subroutine runs, so that
    // libmagic chains like `>>0 use part2` followed by continuation rules
    // continue producing matches in document order.
    let subroutine = vec![MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "sub-head".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    }];
    let table = build_name_table(vec![("sub", subroutine)]);
    // Disable stop-at-first-match so both the subroutine and the child
    // rule are visible in the match vector.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let child = MagicRule {
        offset: OffsetSpec::Absolute(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xBB),
        message: "use-child".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    };
    let mut use_with_child = use_rule("sub");
    use_with_child.children = vec![child];

    let buffer = [0xAAu8, 0xBB, 0xCC];
    let matches = evaluate_rules(&[use_with_child], &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "use rule's own children must run after the subroutine"
    );
    assert_eq!(matches[0].message, "sub-head");
    assert_eq!(matches[1].message, "use-child");
}

#[test]
fn test_name_rule_leaked_is_noop() {
    // Programmatic consumers may construct a Name rule directly and pass
    // it to the evaluator (e.g. property tests). The evaluator must not
    // panic; it should instead treat the rule as a silent no-op.
    let leaked = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Name("orphan".to_string())),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: String::new(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[leaked], &[0u8; 4], &mut context).unwrap();
    assert!(matches.is_empty(), "leaked Name rule should be a no-op");
}

// =============================================================================
// MetaType::Default / Clear / Indirect tests
// =============================================================================

/// Build a `Default` rule with the given message and (optional) children.
fn default_rule(message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
    }
}

/// Build a `Clear` rule. Carries no message in the magic file syntax, but the
/// AST requires a message field.
fn clear_rule() -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Clear),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: String::new(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    }
}

/// Build a single byte-equality rule at `offset` for `value`.
fn byte_eq_rule(offset: i64, value: u64, message: &str) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(value),
        message: message.to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    }
}

#[test]
fn test_default_fires_when_no_sibling_matched() {
    let rules = vec![default_rule("DEFAULT-FIRES", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "default with no prior sibling match should fire"
    );
    assert_eq!(matches[0].message, "DEFAULT-FIRES");
}

#[test]
fn test_default_skipped_when_sibling_matched() {
    // Disable stop-at-first-match so we can see whether the default would
    // have fired or not.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        byte_eq_rule(0, 0xAA, "real-match"),
        default_rule("DEFAULT-SKIPPED", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "default after a successful sibling should not fire"
    );
    assert_eq!(matches[0].message, "real-match");
}

#[test]
fn test_default_fires_only_once() {
    // Two consecutive default rules: the first sets sibling_matched, so
    // the second must not fire.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        default_rule("FIRST-DEFAULT", vec![]),
        default_rule("SECOND-DEFAULT", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "only the first default should fire when no real sibling matched"
    );
    assert_eq!(matches[0].message, "FIRST-DEFAULT");
}

#[test]
fn test_default_children_evaluated() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let child = byte_eq_rule(0, 0xAA, "default-child");
    let rules = vec![default_rule("PARENT-DEFAULT", vec![child])];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "default rule's children must be evaluated when the default fires"
    );
    assert_eq!(matches[0].message, "PARENT-DEFAULT");
    assert_eq!(matches[1].message, "default-child");
}

#[test]
fn test_clear_resets_sibling_matched() {
    // Sequence: byte-match, default-skipped, clear, default-fires.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        byte_eq_rule(0, 0xAA, "byte-match"),
        default_rule("DEFAULT-SKIPPED", vec![]),
        clear_rule(),
        default_rule("DEFAULT-FIRES-AFTER-CLEAR", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert_eq!(
        matches.len(),
        2,
        "clear must reset sibling_matched so a later default fires"
    );
    assert_eq!(matches[0].message, "byte-match");
    assert_eq!(matches[1].message, "DEFAULT-FIRES-AFTER-CLEAR");
}

#[test]
fn test_clear_at_top_is_noop() {
    let rules = vec![clear_rule(), default_rule("AFTER-CLEAR", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "clear at top of list is a no-op; default after still fires"
    );
    assert_eq!(matches[0].message, "AFTER-CLEAR");
}

#[test]
fn test_clear_does_not_produce_match() {
    let rules = vec![clear_rule()];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(matches.is_empty(), "clear alone must produce no match");
}

#[test]
fn test_default_clear_per_level_isolation() {
    // Parent has its own sibling_matched flag. The child list runs with a
    // fresh flag, so a child-level `default` must fire even though the
    // parent's flag is true.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "parent-match".to_string(),
        children: vec![
            byte_eq_rule(1, 0xBB, "child-byte-match"),
            default_rule("CHILD-DEFAULT-SKIPPED", vec![]),
            clear_rule(),
            default_rule("CHILD-DEFAULT-AFTER-CLEAR", vec![]),
        ],
        level: 0,
        strength_modifier: None,
    };
    let mut context = EvaluationContext::new(config);
    let buffer = [0xAAu8, 0xBB];
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();

    // Expected order: parent-match, child-byte-match, CHILD-DEFAULT-AFTER-CLEAR
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "parent-match",
            "child-byte-match",
            "CHILD-DEFAULT-AFTER-CLEAR"
        ],
        "child-level sibling_matched must be isolated from parent-level state"
    );
}

/// Build an `Indirect` rule at `offset` with optional children.
fn indirect_rule(offset: i64, message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Meta(MetaType::Indirect),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
    }
}

#[test]
fn test_indirect_evaluates_root_rules_at_offset() {
    // Root rules: detect a "ZIP-like" header (0x50 0x4b) at offset 0 of the
    // sub-buffer. The indirect rule fires at offset 4 of the outer buffer,
    // which means the sub-buffer starts at byte 4. Place 0x50 0x4b there.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let root_rule = byte_eq_rule(0, 0x50, "ZIP-like-header");
    let root_rules: Vec<MagicRule> = vec![root_rule];

    // Build an environment where root_rules is the same as the rules we
    // dispatch into.
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    // Buffer: ELF magic at offset 0, ZIP-like at offset 4. The indirect
    // rule is the trigger; the root re-entry detects 0x50 at sub-buffer 0.
    let buffer = [0x7fu8, 0x45, 0x4c, 0x46, 0x50, 0x4b, 0x03, 0x04];
    let rules = vec![indirect_rule(4, "indirect-trigger", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| m.message == "ZIP-like-header"),
        "indirect must dispatch root rules against the sub-buffer at offset 4; got {matches:?}"
    );
}

#[test]
fn test_indirect_out_of_bounds_is_noop() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(&[byte_eq_rule(0, 0x00, "root")] as &[MagicRule]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let buffer = [0u8; 4];
    // Indirect at offset 100, which is well past the 4-byte buffer.
    let rules = vec![indirect_rule(100, "indirect-oob", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "indirect past buffer end must be a graceful no-op"
    );
}

#[test]
fn test_indirect_without_env_is_noop() {
    // Property tests synthesize Indirect rules without an attached
    // RuleEnvironment, so this path must be a graceful no-op (matching the
    // `Use`-without-env contract). The engine logs at `debug!` rather than
    // panicking via `debug_assert!` to preserve the never-panics invariant
    // exercised by `prop_arbitrary_rule_evaluation_never_panics`.
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let buffer = [0u8; 4];
    let rules = vec![indirect_rule(0, "indirect-no-env", vec![])];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "indirect without env must produce no matches"
    );
}

#[test]
fn test_indirect_recursion_limit() {
    // Root rules contain an indirect rule that points back to offset 0,
    // creating an infinite re-entry chain. Must surface as
    // `RecursionLimitExceeded`, not stack overflow.
    let inner_indirect = indirect_rule(0, "recursive-indirect", vec![]);
    let root_rules: Vec<MagicRule> = vec![inner_indirect];
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(NameTable::empty()),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(EvaluationConfig::default()).with_rule_env(env);

    let buffer = [0u8; 8];
    let rules = vec![indirect_rule(0, "outer-indirect", vec![])];
    let result = evaluate_rules(&rules, &buffer, &mut context);
    assert!(
        matches!(
            result,
            Err(LibmagicError::EvaluationError(
                crate::error::EvaluationError::RecursionLimitExceeded { .. }
            ))
        ),
        "infinite indirect recursion must surface RecursionLimitExceeded, got {result:?}"
    );
}

// =======================================================================
// MetaType::Offset dispatch (issue #42)
// =======================================================================

/// Build an `Offset` rule at `offset` with an `x` (`AnyValue`) operator and
/// the given message. Mirrors `default_rule`/`indirect_rule` helpers.
fn offset_rule(offset: i64, message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Meta(MetaType::Offset),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
    }
}

#[test]
fn test_offset_emits_match_with_resolved_position() {
    let rules = vec![offset_rule(5, "pos=%lld", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 10], &mut context).unwrap();
    assert_eq!(matches.len(), 1, "offset rule must emit exactly one match");
    assert_eq!(matches[0].offset, 5, "match.offset is the resolved offset");
    assert_eq!(
        matches[0].value,
        Value::Uint(5),
        "match.value carries the resolved offset for format substitution"
    );
    assert_eq!(matches[0].message, "pos=%lld");
}

#[test]
fn test_offset_at_zero() {
    // Regression guard: offset 0 must still produce a match (not be
    // indistinguishable from "no match").
    let rules = vec![offset_rule(0, "top", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].value, Value::Uint(0));
}

#[test]
fn test_offset_out_of_bounds_graceful_skip() {
    // Offset past the end of the buffer is a data-dependent skip, not an
    // error. Matches the Indirect dispatch's graceful-skip discipline.
    let rules = vec![offset_rule(1_000_000, "unreachable", vec![])];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "offset past buffer end must produce no match"
    );
}

#[test]
fn test_offset_non_x_operator_is_skipped() {
    // magic(5) only allows `x` on an `offset` rule. Anything else is
    // semantically undefined -> debug-log + skip.
    let mut rule = offset_rule(0, "bogus", vec![]);
    rule.op = Operator::Equal;
    rule.value = Value::Uint(5);
    let rules = vec![rule];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    assert!(
        matches.is_empty(),
        "offset rule with non-AnyValue operator must be skipped"
    );
}

#[test]
fn test_offset_evaluates_children() {
    // A child byte rule at offset 0 runs AFTER the parent offset rule
    // fires. The child's own offset is resolved independently.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let mut parent = offset_rule(
        0,
        "parent-offset",
        vec![byte_eq_rule(0, 0x42, "child-byte")],
    );
    // Child level must be deeper than parent per MagicRule::validate.
    parent.children[0].level = 1;
    let buffer = [0x42u8, 0x00, 0x00];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(messages, vec!["parent-offset", "child-byte"]);
}

#[test]
fn test_offset_advances_anchor_for_children() {
    // An offset rule at position 5 advances `last_match_end` to 5 *for its
    // children* -- but NOT for sibling rules at the same level. This
    // matches libmagic's continuation-level semantics: each sibling at
    // level L resolves `&N` against the parent-level anchor, not against
    // the previous sibling's advance. See the `entry_anchor` discipline
    // in `evaluate_rules`.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // A child of the offset rule uses &0 to resolve at the offset rule's
    // resolved position (5). buffer[5] = 0x42.
    let mut child = byte_eq_rule(0, 0x42, "child-at-offset-anchor");
    child.offset = OffsetSpec::Relative(0);
    child.level = 1;

    let buffer = [0x00u8, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00];
    let rules = vec![offset_rule(5, "mark", vec![child])];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches
            .iter()
            .any(|m| m.message == "child-at-offset-anchor"),
        "child of offset rule must resolve against offset's anchor (5); got {matches:?}"
    );
}

#[test]
fn test_offset_does_not_advance_anchor_for_continuation_siblings() {
    // Regression guard for the libmagic continuation-sibling anchor
    // semantic: two CHILD siblings at the same level resolve `&N`
    // against the parent-level anchor, not against the previous
    // sibling's advance. This is gated on `recursion_depth > 0`;
    // top-level siblings still chain (see
    // `relative_anchor_can_decrease_...` in the relative-offset
    // integration tests).
    //
    // Parent `byte` at offset 0 matches 0x01 -> anchor = 1. Two
    // child siblings at &0 must both read buffer[1] = 0x42. If the
    // first child incorrectly advanced the anchor to 2, the second
    // would read buffer[2] = 0x00 and miss.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x01),
        message: "parent".to_string(),
        children: vec![
            MagicRule {
                offset: OffsetSpec::Relative(0),
                typ: TypeKind::Byte { signed: false },
                op: Operator::Equal,
                value: Value::Uint(0x42),
                message: "sibling-1".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
            MagicRule {
                offset: OffsetSpec::Relative(0),
                typ: TypeKind::Byte { signed: false },
                op: Operator::Equal,
                value: Value::Uint(0x42),
                message: "sibling-2".to_string(),
                children: vec![],
                level: 1,
                strength_modifier: None,
            },
        ],
        level: 0,
        strength_modifier: None,
    };

    let buffer = [0x01u8, 0x42, 0x00, 0x00];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&[parent], &buffer, &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["parent", "sibling-1", "sibling-2"],
        "both continuation siblings must resolve against parent anchor (1); \
         if sibling-1 advanced the anchor to 2, sibling-2 would read \
         buffer[2]=0x00 and fail"
    );
}

#[test]
fn test_offset_sets_sibling_matched() {
    // An offset rule match suppresses a following `default` sibling --
    // same discipline as any other matching rule.
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let rules = vec![
        offset_rule(0, "offset-match", vec![]),
        default_rule("DEFAULT-SUPPRESSED", vec![]),
    ];
    let mut context = EvaluationContext::new(config);
    let matches = evaluate_rules(&rules, &[0u8; 4], &mut context).unwrap();
    let messages: Vec<&str> = matches.iter().map(|m| m.message.as_str()).collect();
    assert_eq!(
        messages,
        vec!["offset-match"],
        "default must be suppressed when offset sibling matched; got {matches:?}"
    );
}
