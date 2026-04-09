// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::{Endianness, OffsetSpec, Operator, TypeKind, Value};

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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let err = evaluate_single_rule(&rule, buffer).unwrap_err();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap().unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer);
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
    let result = evaluate_single_rule(&rule, buffer);
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
    let result = evaluate_single_rule(&rule, buffer);
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
    let result = evaluate_single_rule(&rule, buffer);
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
    let result = evaluate_single_rule(&rule, buffer);
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

    let result = evaluate_single_rule(&rule_no_match, buffer);
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, elf_buffer).unwrap();
    assert!(result.is_some());

    let non_elf_buffer = &[0x50, 0x4b, 0x03, 0x04, 0x14, 0x00];
    let result = evaluate_single_rule(&rule, non_elf_buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    assert!(evaluate_single_rule(&equal_rule, buffer).unwrap().is_some());

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
        evaluate_single_rule(&not_equal_rule, buffer)
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
        evaluate_single_rule(&bitwise_and_rule, buffer)
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
        evaluate_single_rule(&less_than_rule, buffer)
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
        evaluate_single_rule(&greater_than_rule, buffer)
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
        evaluate_single_rule(&less_equal_rule, buffer)
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
        evaluate_single_rule(&greater_equal_rule, buffer)
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
        evaluate_single_rule(&signed_rule, buffer)
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
        evaluate_single_rule(&unsigned_rule, buffer)
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
        let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&max_uint_rule, max_buffer).unwrap();
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
    let result = evaluate_single_rule(&min_int_rule, min_buffer).unwrap();
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
    let result = evaluate_single_rule(&single_byte_rule, single_buffer).unwrap();
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

    let result = evaluate_single_rule(&large_rule, &large_buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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

    let single_result = evaluate_single_rule(&rule, buffer);
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
    let result = evaluate_single_rule(&rule, buffer).unwrap();
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
