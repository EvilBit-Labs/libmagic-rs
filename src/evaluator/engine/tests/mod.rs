// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate/module instead.
#![allow(clippy::modulo_arithmetic)]

use super::*;
use crate::parser::ast::{
    Endianness, OffsetSpec, Operator, SearchFlags, StringFlags, TypeKind, Value,
};

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
    evaluate_single_rule_with_anchor(
        rule,
        buffer,
        0,
        0,
        crate::evaluator::types::DEFAULT_MAX_STRING_LENGTH,
    )
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("test".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let buffer = b"test\x00 data";
    let result = evaluate_single_rule_legacy(&rule, buffer);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert!(matches.is_some());

    let rule_no_match = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("hello".to_string()),
        message: "String type".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
    };

    let buffer = &[42];
    let result = evaluate_single_rule_legacy(&rule, buffer).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_shorts() {
    // BitwiseAnd requires ALL masked bits to be set (see GOTCHAS S13.3):
    // `(value & mask) == mask`, not merely "some bit overlaps". The mask
    // here (0xff00) asks "is the entire high byte set" -- so the buffer's
    // high byte must genuinely be 0xff for this to match.
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
        value_transform: None,
    };

    // Little-endian [0x34, 0xff] -> 0xff34; high byte is 0xff, all mask bits set.
    let matching_buffer = &[0x34, 0xff];
    let result = evaluate_single_rule_legacy(&rule, matching_buffer).unwrap();
    assert!(result.is_some());

    // Little-endian [0x34, 0x12] -> 0x1234; high byte is 0x12, not all mask
    // bits set, so this must NOT match under the corrected semantics.
    let non_matching_buffer = &[0x34, 0x12];
    let result = evaluate_single_rule_legacy(&rule, non_matching_buffer).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_evaluate_single_rule_bitwise_and_with_longs() {
    // Same "all masked bits set" contract as the shorts test above, applied
    // to a 32-bit mask spanning the high word.
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
        value_transform: None,
    };

    // Big-endian [0xff, 0xff, 0x56, 0x78] -> 0xffff5678; high word is 0xffff,
    // all mask bits set.
    let matching_buffer = &[0xff, 0xff, 0x56, 0x78];
    let result = evaluate_single_rule_legacy(&rule, matching_buffer).unwrap();
    assert!(result.is_some());

    // Big-endian [0x12, 0x34, 0x56, 0x78] -> 0x12345678; high word is
    // 0x1234, not all mask bits set, so this must NOT match.
    let non_matching_buffer = &[0x12, 0x34, 0x56, 0x78];
    let result = evaluate_single_rule_legacy(&rule, non_matching_buffer).unwrap();
    assert!(result.is_none());
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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

/// Build a flat, top-level, message-only byte rule matching a distinct
/// value at a distinct offset. Shared by the `stop_at_first_match`
/// message-bearing tests below so each test only needs to state the
/// interesting bit: which offsets carry which messages.
fn message_only_byte_rule(offset: i64, byte: u8, message: &str) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(byte)),
        message: message.to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// GOTCHAS S13.2 (refined): a message-less top-level match must not
/// shadow a later, message-bearing sibling under `stop_at_first_match:
/// true`. This is the exact shape of the assembler-source-text /
/// plain-ASCII-text blank-output bug -- a gating rule with no message
/// matches first in strength order and used to terminate evaluation
/// before the real classification rule was ever tried.
#[test]
fn test_evaluate_rules_message_less_match_does_not_stop_at_first_match() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let gating_rule = message_only_byte_rule(0, 0xAA, "");
    let real_rule = message_only_byte_rule(1, 0xBB, "Second match");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[gating_rule, real_rule], buffer, &mut context).unwrap();

    // Both rules matched: the message-less gating rule did not terminate
    // the search, so the message-bearing rule behind it was reached and
    // its match is present.
    assert_eq!(matches.len(), 2, "both rules should have matched");
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "Second match");
}

/// Reverse of the above: when the message-BEARING rule comes first, the
/// original `stop_at_first_match` short-circuit still applies -- this
/// fix only relaxes the stop condition for message-less matches, it does
/// not disable early-exit for the common (and performance-sensitive)
/// case where the very first top-level rule already produces output.
#[test]
fn test_evaluate_rules_message_bearing_match_still_stops_at_first_match() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let real_rule = message_only_byte_rule(0, 0xAA, "First match");
    let gating_rule = message_only_byte_rule(1, 0xBB, "");
    let never_reached = message_only_byte_rule(2, 0xCC, "Should not be reached");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[real_rule, gating_rule, never_reached],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(
        matches.len(),
        1,
        "evaluation must still stop right after the first message-bearing match"
    );
    assert_eq!(matches[0].message, "First match");
}

/// Several message-less matches in a row must all be skipped over (not
/// discarded -- just not treated as terminating) until a message-bearing
/// rule is reached, at which point the usual stop-at-first-match
/// short-circuit applies again.
#[test]
fn test_evaluate_rules_multiple_message_less_matches_before_a_real_one() {
    let buffer = &[0xAA, 0xBB, 0xCC, 0xDD];
    let gating_one = message_only_byte_rule(0, 0xAA, "");
    let gating_two = message_only_byte_rule(1, 0xBB, "");
    let real_rule = message_only_byte_rule(2, 0xCC, "Real message");
    let never_reached = message_only_byte_rule(3, 0xDD, "Should not be reached");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[gating_one, gating_two, real_rule, never_reached],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "");
    assert_eq!(matches[2].message, "Real message");
}

/// Genuinely-no-usable-output case: every top-level rule matches but
/// none of them carries a message. Under `stop_at_first_match: true`
/// evaluation must run to exhaustion (there is nothing to stop at) --
/// all matches are collected, and it is the caller's (here:
/// `MagicDatabase::build_result`'s) job to fall back to text/data
/// classification when the resulting description is empty.
#[test]
fn test_evaluate_rules_all_message_less_matches_runs_to_exhaustion() {
    let buffer = &[0xAA, 0xBB];
    let gating_one = message_only_byte_rule(0, 0xAA, "");
    let gating_two = message_only_byte_rule(1, 0xBB, "");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[gating_one, gating_two], buffer, &mut context).unwrap();

    assert_eq!(matches.len(), 2, "both message-less matches are retained");
    assert!(matches.iter().all(|m| m.message.is_empty()));
}

/// A message consisting solely of whitespace, or solely of the GNU
/// `file` backspace continuation marker (`\b`), is just as
/// "message-less" as an empty string for `stop_at_first_match` purposes
/// -- see `is_message_bearing`'s doc comment for the rationale.
#[test]
fn test_evaluate_rules_whitespace_and_backspace_only_messages_do_not_stop() {
    let buffer = &[0xAA, 0xBB, 0xCC];
    let whitespace_only = message_only_byte_rule(0, 0xAA, "   ");
    let backspace_only = message_only_byte_rule(1, 0xBB, "\u{8}");
    let real_rule = message_only_byte_rule(2, 0xCC, "Real message");

    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(
        &[whitespace_only, backspace_only, real_rule],
        buffer,
        &mut context,
    )
    .unwrap();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[2].message, "Real message");
}

/// A message-less top-level rule whose CHILD produces real output text
/// still counts as "producing output" for `stop_at_first_match`
/// purposes -- this is the normal, common shape for gating rules like
/// `c-lang`'s `0 search/8192 "#include"` -> `>0 regex \^#include c`
/// chain, and must keep stopping at the first sibling that (directly or
/// via a descendant) yields a description; only a rule with NO
/// message-bearing output anywhere in its subtree should be skipped
/// past.
#[test]
fn test_evaluate_rules_message_less_parent_with_message_bearing_child_still_stops() {
    let child_rule = message_only_byte_rule(1, 0xBB, "child message");
    let mut parent_rule = message_only_byte_rule(0, 0xAA, "");
    parent_rule.children = vec![child_rule];

    let never_reached = message_only_byte_rule(2, 0xCC, "Should not be reached");

    let buffer = &[0xAA, 0xBB, 0xCC];
    let config = EvaluationConfig {
        stop_at_first_match: true,
        ..Default::default()
    };
    let mut context = EvaluationContext::new(config);

    let matches = evaluate_rules(&[parent_rule, never_reached], buffer, &mut context).unwrap();

    assert_eq!(
        matches.len(),
        2,
        "parent (message-less) + child (message-bearing) should be present, \
         and evaluation should stop there"
    );
    assert_eq!(matches[0].message, "");
    assert_eq!(matches[1].message, "child message");
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
                value_transform: None,
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
                value_transform: None,
            },
        ],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(1),
            typ: TypeKind::String {
                max_length: Some(3),
                flags: StringFlags::default(),
            },
            op: Operator::Equal,
            value: Value::String("ELF".to_string()),
            message: "Valid string".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
        }],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
        }],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
            value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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
            flags: SearchFlags::default(),
        },
        op: Operator::NotEqual,
        value: Value::String("needle".to_string()),
        message: "no needle".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
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
            flags: SearchFlags::default(),
        },
        op: Operator::BitwiseAnd,
        value: Value::String("needle".to_string()),
        message: "bogus".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
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
        value_transform: None,
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

// =============================================================================
// fix-system-magic-regex-graceful, U2: narrow graceful-skip of the
// missing-pattern-operand `TypeReadError::UnsupportedType` condition.
//
// Before this fix, a regex/search rule whose `value` operand was not a
// `String`/`Bytes` pattern (GOTCHAS S2.4) caused `evaluate_rules` to
// propagate a fatal `Err`, aborting evaluation of the ENTIRE rule set (and,
// via `MagicDatabase`, the entire magic file) rather than skipping just the
// one broken rule. See docs/plans/2026-07-17-001-fix-system-magic-regex-
// graceful-plan.md.
// =============================================================================

/// Builds a `TypeKind::Regex` rule whose `value` is `Value::Uint(0)` --
/// neither `Value::String` nor `Value::Bytes` -- so `read_pattern_match`
/// always returns `Err(UnsupportedType { type_name: "regex without string
/// pattern" })`, regardless of U1's `Value::Bytes` backstop. This isolates
/// U2's engine-level skip from U1's evaluator-level acceptance.
fn broken_pattern_regex_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

/// FLOOR ANCHOR (U2 execution note): this is the regression test that must
/// be written and observed to FAIL before the engine fix is wired in. Prior
/// to the fix, `evaluate_rules` returns `Err(LibmagicError::EvaluationError(
/// EvaluationError::TypeReadError(TypeReadError::UnsupportedType { .. })))`
/// for a top-level pattern-less regex rule -- aborting analysis of every
/// target when the system magic DB contains such a rule. The floor
/// requirement (R1/R2) is that `evaluate_rules` must return `Ok` and treat
/// the broken rule as a non-match.
#[test]
fn test_evaluate_rules_skips_pattern_less_regex_rule_gracefully() {
    let rule = broken_pattern_regex_rule("broken top-level regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_rules(&[rule], b"anything to scan", &mut context);
    let matches =
        result.expect("evaluate_rules must not fatally abort on a pattern-less regex rule");
    assert!(
        matches.is_empty(),
        "the broken rule must contribute no match, got {matches:?}"
    );
}

/// All-three-sites parity (a): a broken pattern-less regex at the TOP LEVEL
/// alongside a normal sibling rule -- the sibling must still match and the
/// broken rule must be silently skipped.
#[test]
fn test_pattern_operand_skip_at_top_level_site() {
    let broken = broken_pattern_regex_rule("broken top-level regex", 0);
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "leading a".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    let matches = evaluate_rules(&[broken, sibling], b"abc", &mut context)
        .expect("top-level pattern-less regex must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// All-three-sites parity (b): a broken pattern-less regex as a CHILD under
/// a matched parent (the inline child-recursion catch arm, ~L1108-1118).
/// The parent match must still be emitted even though its child is broken.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site() {
    let broken_child = broken_pattern_regex_rule("broken child regex", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc", &mut context)
        .expect("a broken child regex must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// All-three-sites parity (c): a broken pattern-less regex as a child of a
/// `default` rule -- exercises the `evaluate_children_or_warn` path
/// (~L522-530). The `default` match must still be emitted.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site() {
    let broken_child = broken_pattern_regex_rule("broken default child regex", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"anything", &mut context)
        .expect("a broken child regex under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

/// NEGATIVE (pins R3 narrowness): an `UnsupportedType` condition that is
/// NOT the missing-pattern-operand class -- here, a non-Equal/NotEqual
/// operator on a pattern-bearing type -- must still propagate fatally.
/// This proves U2's skip did not widen into swallowing the whole
/// `UnsupportedType` variant.
#[test]
fn test_evaluate_rules_propagates_non_pattern_missing_unsupported_type() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::GreaterThan,
        value: Value::String("[0-9]+".to_string()),
        message: "bogus ordering".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let result = evaluate_rules(&[rule], b"abc123", &mut context);
    assert!(
        matches!(result, Err(LibmagicError::EvaluationError(_))),
        "a non-pattern-missing UnsupportedType must still propagate, got {result:?}"
    );
}

/// NEGATIVE: a regex whose pattern fails to compile under the
/// `REGEX_COMPILE_SIZE_LIMIT` (1 MiB) CWE-1333 denial-of-service guard is skipped (not
/// fatal) per KTD5, but this is a distinct, louder-logged condition than
/// the ordinary missing-pattern skip. There is no log-capturing test seam
/// in this crate (no `test-log`/`tracing-test` dev-dependency), so this
/// test asserts only the behavioral half of the contract -- the rule is
/// skipped, not fatal -- and the `warn!` vs `debug!` split is verified by
/// code inspection (`log_pattern_operand_skip` in `engine/mod.rs`).
#[test]
fn test_pathological_regex_compile_failure_is_skipped_not_fatal() {
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("a{1000000}".to_string()),
        message: "pathological regex".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection must be skipped, not fatal");
    assert!(
        matches.is_empty(),
        "the pathological regex rule must contribute no match, got {matches:?}"
    );
}

/// IO/offset arms untouched (KTD4 regression guard): a `BufferOverrun`
/// condition (via an anchor pinned to `usize::MAX`) must still be skipped
/// exactly as before, proving U2 added a new arm rather than replacing the
/// pre-existing IO/offset catch set.
#[test]
fn test_buffer_overrun_still_skipped_after_pattern_operand_guard_added() {
    let buffer = [0xAA, 0xBB, 0xCC, 0xDD];
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX);

    let rule = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rel-zero-near-sat-regression".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let matches = evaluate_rules(&[rule], &buffer, &mut ctx)
        .expect("BufferOverrun must still be skipped gracefully, not propagated");
    assert!(
        matches.is_empty(),
        "Relative(0) at usize::MAX anchor must skip, not match or panic"
    );
}

// -----------------------------------------------------------------------
// C2 hardening: the missing-pattern-operand skip is asserted end-to-end
// only for `Regex` above. `Search` and flagged `String` share the SAME
// allowlisted consts (`types::SEARCH_MISSING_PATTERN_MSG` /
// `types::FLAGGED_STRING_MISSING_PATTERN_MSG`) and the same three engine
// catch sites, so this closes R2 for every pattern-bearing type and
// guards the C1 const extraction against silent drift.
// -----------------------------------------------------------------------

/// Builds a `TypeKind::Search` rule whose `value` is `Value::Uint(0)` --
/// neither `Value::String` nor `Value::Bytes` -- so `read_pattern_match`
/// always returns `Err(UnsupportedType { type_name:
/// SEARCH_MISSING_PATTERN_MSG })`.
fn broken_pattern_search_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(16).unwrap(),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Builds a flagged `TypeKind::String` rule (non-empty `flags`, routing
/// through the pattern-bearing path per GOTCHAS S2.4) whose `value` is
/// `Value::Uint(0)`, so `read_pattern_match` always returns
/// `Err(UnsupportedType { type_name: FLAGGED_STRING_MISSING_PATTERN_MSG })`.
fn broken_pattern_flagged_string_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String {
            max_length: None,
            flags: StringFlags {
                ignore_lowercase: true,
                ..StringFlags::default()
            },
        },
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Top-level site: a pattern-less `search` rule is skipped, not fatal.
#[test]
fn test_pattern_operand_skip_at_top_level_site_search() {
    let broken = broken_pattern_search_rule("broken top-level search", 0);
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "leading a".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    let matches = evaluate_rules(&[broken, sibling], b"abc", &mut context)
        .expect("top-level pattern-less search must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// Child-recursion site: a pattern-less `search` rule under a matched
/// parent must not abort the parent's match.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site_search() {
    let broken_child = broken_pattern_search_rule("broken child search", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc", &mut context)
        .expect("a broken child search must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a pattern-less `search` rule as a
/// child of `default` must not abort the `default` match.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site_search() {
    let broken_child = broken_pattern_search_rule("broken default child search", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"anything", &mut context)
        .expect("a broken child search under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

/// Top-level site: a pattern-less flagged `string` rule is skipped, not
/// fatal.
#[test]
fn test_pattern_operand_skip_at_top_level_site_flagged_string() {
    let broken = broken_pattern_flagged_string_rule("broken top-level flagged string", 0);
    let sibling = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "leading a".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context =
        EvaluationContext::new(EvaluationConfig::default().with_stop_at_first_match(false));
    let matches = evaluate_rules(&[broken, sibling], b"abc", &mut context)
        .expect("top-level pattern-less flagged string must be skipped, not fatal");
    assert_eq!(
        matches.len(),
        1,
        "expected only the sibling match, got {matches:?}"
    );
    assert_eq!(matches[0].message, "leading a");
}

/// Child-recursion site: a pattern-less flagged `string` rule under a
/// matched parent must not abort the parent's match.
#[test]
fn test_pattern_operand_skip_under_matched_parent_child_recursion_site_flagged_string() {
    let broken_child = broken_pattern_flagged_string_rule("broken child flagged string", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"abc", &mut context)
        .expect("a broken child flagged string must not abort evaluation of the parent");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a pattern-less flagged `string` rule
/// as a child of `default` must not abort the `default` match.
#[test]
fn test_pattern_operand_skip_under_default_children_or_warn_site_flagged_string() {
    let broken_child = broken_pattern_flagged_string_rule("broken default child flagged string", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"anything", &mut context)
        .expect("a broken child flagged string under `default` must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

// -----------------------------------------------------------------------
// E hardening: the compile-failure (warn!) skip is proven end-to-end only
// at the top-level dispatch site above
// (`test_pathological_regex_compile_failure_is_skipped_not_fatal`). Add
// the two missing sites for 3-site parity with the missing-pattern
// (debug!) coverage.
// -----------------------------------------------------------------------

/// Builds a `TypeKind::Regex` rule whose pattern is syntactically valid
/// (`Value::String`, so U1's `Value::Bytes` backstop is irrelevant here)
/// but rejected by the `REGEX_COMPILE_SIZE_LIMIT` (1 MiB) CWE-1333
/// denial-of-service guard at compile time.
fn pathological_regex_rule(message: &str, level: u32) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String("a{1000000}".to_string()),
        message: message.to_string(),
        children: vec![],
        level,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Child-recursion site: a regex compile-size rejection under a matched
/// parent must not abort the parent's match.
#[test]
fn test_pathological_regex_compile_failure_skipped_under_matched_parent_child_recursion_site() {
    let broken_child = pathological_regex_rule("broken compile child regex", 1);
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "parent byte".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"aaaa", &mut context)
        .expect("a regex compile-size rejection under a matched parent must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the parent match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "parent byte");
}

/// `evaluate_children_or_warn` site: a regex compile-size rejection as a
/// child of `default` must not abort the `default` match.
#[test]
fn test_pathological_regex_compile_failure_skipped_under_default_children_or_warn_site() {
    let broken_child = pathological_regex_rule("broken compile default child regex", 1);
    let default_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: "default fallback".to_string(),
        children: vec![broken_child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[default_rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection under default children must not abort evaluation");
    assert_eq!(
        matches.len(),
        1,
        "expected only the default match (broken child skipped), got {matches:?}"
    );
    assert_eq!(matches[0].message, "default fallback");
}

// -----------------------------------------------------------------------
// H hardening: pin the debug!/warn! log-level contract with a real
// log-capture seam (`testing_logger`, which captures the `log` facade
// this crate uses -- not `tracing`). Previously these contracts were
// asserted by code inspection only.
// -----------------------------------------------------------------------

/// Test-only helper: `testing_logger::CapturedLog` does not implement
/// `Debug`, so format captured logs manually for failure messages.
fn format_logs(logs: &[testing_logger::CapturedLog]) -> String {
    logs.iter()
        .map(|l| format!("{:?}: {}", l.level, l.body))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The ordinary missing-pattern-operand skip (top-level site) logs at
/// `debug!`, not `warn!` -- it is an expected, low-severity data
/// condition, not a security-relevant signal.
#[test]
fn test_missing_pattern_operand_skip_logs_at_debug_level() {
    testing_logger::setup();
    let rule = broken_pattern_regex_rule("broken top-level regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"anything to scan", &mut context)
        .expect("pattern-less regex must be skipped, not fatal");
    assert!(matches.is_empty());
    testing_logger::validate(|captured_logs| {
        let skip_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("Skipping top-level rule"))
            .collect();
        assert_eq!(
            skip_logs.len(),
            1,
            "expected exactly one skip log entry, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            skip_logs[0].level,
            log::Level::Debug,
            "missing-pattern-operand skip must log at debug!, not warn! -- \
             got {:?}: {:?}",
            skip_logs[0].level,
            skip_logs[0].body
        );
    });
}

/// A regex compile-size rejection (`REGEX_COMPILE_SIZE_LIMIT`,
/// CWE-1333) logs at `warn!`, not `debug!` -- a malicious or pathological
/// magic file's rejection must stay visible in logs even though
/// evaluation of the rest of the file continues.
#[test]
fn test_regex_compile_failure_skip_logs_at_warn_level() {
    testing_logger::setup();
    let rule = pathological_regex_rule("pathological regex", 0);
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[rule], b"aaaa", &mut context)
        .expect("a regex compile-size rejection must be skipped, not fatal");
    assert!(matches.is_empty());
    testing_logger::validate(|captured_logs| {
        let skip_logs: Vec<_> = captured_logs
            .iter()
            .filter(|l| l.body.contains("regex compile failure"))
            .collect();
        assert_eq!(
            skip_logs.len(),
            1,
            "expected exactly one compile-failure log entry, got {:?}",
            format_logs(captured_logs)
        );
        assert_eq!(
            skip_logs[0].level,
            log::Level::Warn,
            "regex compile-failure skip must log at warn!, not debug! -- \
             got {:?}: {:?}",
            skip_logs[0].level,
            skip_logs[0].body
        );
    });
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
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(14).unwrap(),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("needle".to_string()),
        message: "found needle".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
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
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(32).unwrap(),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("NEEDLE".to_string()),
        message: "found".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"prefix_NEEDLE_after_stuff", &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].message, "a after");
}

// Flagged-string engine dispatch tests are split into
// `string_flags_dispatch_tests.rs` (see the submodule declaration at the
// bottom of this file). They cover the engine routing layer; lower-level
// `compare_string_with_flags` tests live in
// `src/evaluator/types/string.rs::tests`, and integration / conformance
// coverage lives in `tests/string_flags_integration.rs`.

// Shared test helpers have been extracted into the `helpers` sub-tree so
// this module stays focused on its own test wiring; the meta_* submodules
// continue to access helpers via `super::*` thanks to the glob re-export
// below. The three bare `use` items are for types that the submodules
// still reference directly (e.g. `MetaType::Default`, `RuleEnvironment {
// ... }` literal construction) and therefore must stay in this module's
// namespace for `super::*` to reach.
mod helpers;
pub(super) use crate::evaluator::RuleEnvironment;
pub(super) use crate::parser::ast::MetaType;
pub(super) use crate::parser::name_table::NameTable;
pub(super) use helpers::meta::*;

// Submodule declarations
#[cfg(test)]
mod meta_default_clear_indirect_tests;
#[cfg(test)]
mod meta_offset_tests;
#[cfg(test)]
mod meta_use_tests;
#[cfg(test)]
mod string_flags_dispatch_tests;
