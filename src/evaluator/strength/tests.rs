// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`super`]'s strength calculation.
//!
//! Split out of `strength.rs` to keep the production module inside the
//! project's file-size convention; the calculation itself is ~477 lines
//! while these tests are roughly twice that.

/// Search strength falls as the scan widens, and a wide scan does not
/// outrank a real numeric detector.
///
/// sgml's `0 search/4096/cwt \<!--` is a 4-byte pattern over a 4096-byte
/// window; `cafebabe`'s Mach-O slice detector is `0 lelong&0xfffffffe
/// 0xfeedface`. With a flat search bonus the former sorted first and
/// mislabeled ~15% of `/usr/bin` Mach-O binaries as SGML (#379).
#[test]
fn search_strength_falls_as_the_scan_range_widens() {
    use crate::parser::ast::{Endianness, OffsetSpec, Operator, TypeKind, Value};
    use std::num::NonZeroUsize;

    fn search_rule(range: usize) -> MagicRule {
        MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Search {
                range: NonZeroUsize::new(range),
                flags: crate::parser::ast::SearchFlags::default(),
            },
            Operator::Equal,
            Value::String("<!--".to_string()),
            "exported SGML document text".to_string(),
        )
    }

    let long_detector = MagicRule::new(
        OffsetSpec::Absolute(0),
        TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        },
        Operator::BitwiseAndMask(0xffff_fffe),
        Value::Uint(0xfeed_face),
        "Mach-O".to_string(),
    );
    let long_score = calculate_default_strength(&long_detector);

    // Widening the scan weakens the evidence, so the score must not rise.
    // It saturates rather than falling forever: libmagic's multiplier is
    // `MAX(MULT / range, 1)` with MULT = 10, so any range at or above 10
    // floors to 1 and further widening changes nothing.
    let cases: &[(usize, i32, &str)] = &[
        (4, 32, "tight scan -- multiplier 2"),
        (16, 28, "at the saturation floor -- multiplier 1"),
        (4096, 28, "wide scan, the sgml shape -- still multiplier 1"),
    ];

    let mut previous: Option<i32> = None;
    for &(range, expected, label) in cases {
        let score = calculate_default_strength(&search_rule(range));
        assert_eq!(
            score, expected,
            "{label} (range {range}) scored {score}, expected {expected}"
        );
        if let Some(prev) = previous {
            assert!(
                score <= prev,
                "{label} (range {range}) must not outrank a narrower scan, \
                 got {score} after {prev}"
            );
        }
        previous = Some(score);
    }

    // The actual #379 regression: the sgml shape must lose to the Mach-O
    // detector. A tight scan legitimately ties it, which is why the bar
    // here is the wide case specifically, not every case.
    let wide_score = calculate_default_strength(&search_rule(4096));
    assert!(
        wide_score < long_score,
        "a 4-byte pattern over a 4096-byte window must rank below the long \
         detector, got search={wide_score} long={long_score}"
    );
}
use super::*;
use crate::parser::ast::{Endianness, IndirectAdjustmentOp, StringFlags};

// Helper to create a basic test rule
fn make_rule(typ: TypeKind, op: Operator, offset: OffsetSpec, value: Value) -> MagicRule {
    MagicRule {
        offset,
        typ,
        op,
        value,
        message: "test".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

// ============================================================
// Tests for calculate_default_strength
// ============================================================

#[test]
#[allow(clippy::too_many_lines)]
fn test_calculate_default_strength_table() {
    // Table of (rule_factory, expected_strength, description). Each case
    // exercises one strength contribution dimension (type, operator,
    // offset, or value length); the formula is documented in each row.
    type Case = (fn() -> MagicRule, i32, &'static str);
    let cases: &[Case] = &[
        // --- Type contribution (Equal/Absolute/numeric baseline) ---
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            25, // Byte 5 + Equal 10 + Absolute 10 + Numeric 0
            "type=byte",
        ),
        (
            || {
                make_rule(
                    TypeKind::Short {
                        endian: Endianness::Little,
                        signed: false,
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            30, // Short 10 + Equal 10 + Absolute 10
            "type=short",
        ),
        (
            || {
                make_rule(
                    TypeKind::Long {
                        endian: Endianness::Big,
                        signed: false,
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            35, // Long 15 + Equal 10 + Absolute 10
            "type=long",
        ),
        (
            || {
                make_rule(
                    TypeKind::Quad {
                        endian: Endianness::Little,
                        signed: false,
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            36, // Quad 16 + Equal 10 + Absolute 10
            "type=quad",
        ),
        (
            || {
                make_rule(
                    TypeKind::Date {
                        endian: Endianness::Big,
                        utc: true,
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            35, // Date 15 + Equal 10 + Absolute 10
            "type=date",
        ),
        (
            || {
                make_rule(
                    TypeKind::QDate {
                        endian: Endianness::Little,
                        utc: false,
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            36, // QDate 16 + Equal 10 + Absolute 10
            "type=qdate",
        ),
        (
            || {
                make_rule(
                    TypeKind::String {
                        max_length: None,
                        flags: StringFlags::default(),
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::String("ELF".to_string()),
                )
            },
            43, // String 20 + Equal 10 + Absolute 10 + len(3)
            "type=string len=3",
        ),
        (
            || {
                make_rule(
                    TypeKind::String {
                        max_length: Some(10),
                        flags: StringFlags::default(),
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::String("TEST".to_string()),
                )
            },
            49, // String w/max 25 + Equal 10 + Absolute 10 + len(4)
            "type=string max_length=10",
        ),
        // --- Operator contribution (Byte/Absolute baseline) ---
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::NotEqual,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            20, // Byte 5 + NotEqual 5 + Absolute 10
            "op=not_equal",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::BitwiseAnd,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            18, // Byte 5 + BitwiseAnd 3 + Absolute 10
            "op=bitwise_and",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::BitwiseAndMask(0xFF),
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            22, // Byte 5 + BitwiseAndMask 7 + Absolute 10
            "op=bitwise_and_mask",
        ),
        // Comparison operators (all should give the same strength).
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::LessThan,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            21, // Byte 5 + Comparison 6 + Absolute 10
            "op=less_than",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::GreaterThan,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            21,
            "op=greater_than",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::LessEqual,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            21,
            "op=less_equal",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::GreaterEqual,
                    OffsetSpec::Absolute(0),
                    Value::Uint(0),
                )
            },
            21,
            "op=greater_equal",
        ),
        // --- Offset contribution (Byte/Equal baseline) ---
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::Equal,
                    OffsetSpec::Indirect {
                        base_offset: 0,
                        base_relative: false,
                        pointer_type: TypeKind::Long {
                            endian: Endianness::Little,
                            signed: false,
                        },
                        adjustment: 0,
                        adjustment_op: IndirectAdjustmentOp::Add,
                        result_relative: false,
                        endian: Endianness::Little,
                    },
                    Value::Uint(0),
                )
            },
            20, // Byte 5 + Equal 10 + Indirect 5
            "offset=indirect",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::Equal,
                    OffsetSpec::Relative(4),
                    Value::Uint(0),
                )
            },
            18, // Byte 5 + Equal 10 + Relative 3
            "offset=relative",
        ),
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::Equal,
                    OffsetSpec::FromEnd(-4),
                    Value::Uint(0),
                )
            },
            23, // Byte 5 + Equal 10 + FromEnd 8
            "offset=from_end",
        ),
        // --- Value-length contribution ---
        (
            || {
                make_rule(
                    TypeKind::Byte { signed: true },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
                )
            },
            29, // Byte 5 + Equal 10 + Absolute 10 + bytes len(4)
            "value=bytes len=4",
        ),
        (
            || {
                make_rule(
                    TypeKind::String {
                        max_length: None,
                        flags: StringFlags::default(),
                    },
                    Operator::Equal,
                    OffsetSpec::Absolute(0),
                    Value::String("This is a very long string that exceeds the cap".to_string()),
                )
            },
            60, // String 20 + Equal 10 + Absolute 10 + capped len(20)
            "value=long_string (cap)",
        ),
    ];

    for (factory, expected, desc) in cases {
        let rule = factory();
        let strength = calculate_default_strength(&rule);
        assert_eq!(
            strength, *expected,
            "calculate_default_strength mismatch for case '{desc}'"
        );
    }
}

// ============================================================
// Tests for apply_strength_modifier
// ============================================================

#[test]
fn test_apply_modifier_add() {
    assert_eq!(apply_strength_modifier(50, &StrengthModifier::Add(10)), 60);
}

#[test]
fn test_apply_modifier_subtract() {
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Subtract(10)),
        40
    );
}

#[test]
fn test_apply_modifier_multiply() {
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Multiply(2)),
        100
    );
}

#[test]
fn test_apply_modifier_divide() {
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Divide(2)),
        25
    );
}

#[test]
fn test_apply_modifier_set() {
    assert_eq!(apply_strength_modifier(50, &StrengthModifier::Set(75)), 75);
}

#[test]
fn test_apply_modifier_add_overflow() {
    // Should clamp to MAX_STRENGTH
    assert_eq!(
        apply_strength_modifier(250, &StrengthModifier::Add(100)),
        MAX_STRENGTH
    );
}

#[test]
fn test_apply_modifier_subtract_underflow() {
    // Should clamp to MIN_STRENGTH
    assert_eq!(
        apply_strength_modifier(10, &StrengthModifier::Subtract(100)),
        MIN_STRENGTH
    );
}

#[test]
fn test_apply_modifier_multiply_overflow() {
    // Should clamp to MAX_STRENGTH
    assert_eq!(
        apply_strength_modifier(200, &StrengthModifier::Multiply(10)),
        MAX_STRENGTH
    );
}

#[test]
fn test_apply_modifier_divide_by_zero() {
    // Should return base strength unchanged
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Divide(0)),
        50
    );
}

#[test]
fn test_apply_modifier_set_negative() {
    // Should clamp to MIN_STRENGTH
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Set(-10)),
        MIN_STRENGTH
    );
}

#[test]
fn test_apply_modifier_set_over_max() {
    // Should clamp to MAX_STRENGTH
    assert_eq!(
        apply_strength_modifier(50, &StrengthModifier::Set(1000)),
        MAX_STRENGTH
    );
}

// ============================================================
// Tests for calculate_rule_strength
// ============================================================

#[test]
fn test_rule_strength_without_modifier() {
    let rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    // Byte: 5, Equal: 10, Absolute: 10, Numeric: 0 = 25
    assert_eq!(calculate_rule_strength(&rule), 25);
}

#[test]
fn test_rule_strength_with_add_modifier() {
    let mut rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    rule.strength_modifier = Some(StrengthModifier::Add(20));
    // Base: 25, Add 20 = 45
    assert_eq!(calculate_rule_strength(&rule), 45);
}

#[test]
fn test_rule_strength_with_multiply_modifier() {
    let mut rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    rule.strength_modifier = Some(StrengthModifier::Multiply(2));
    // Base: 25, Multiply by 2 = 50
    assert_eq!(calculate_rule_strength(&rule), 50);
}

#[test]
fn test_rule_strength_with_set_modifier() {
    let mut rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    rule.strength_modifier = Some(StrengthModifier::Set(100));
    // Set overrides base strength
    assert_eq!(calculate_rule_strength(&rule), 100);
}

// ============================================================
// Tests for sort_rules_by_strength
// ============================================================

#[test]
fn test_sort_rules_by_strength_basic() {
    let mut rules = vec![
        {
            let mut r = make_rule(
                TypeKind::Byte { signed: true },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::Uint(0),
            );
            r.message = "byte rule".to_string();
            r
        },
        {
            let mut r = make_rule(
                TypeKind::String {
                    max_length: None,
                    flags: StringFlags::default(),
                },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::String("MAGIC".to_string()),
            );
            r.message = "string rule".to_string();
            r
        },
    ];

    sort_rules_by_strength(&mut rules);

    // String rule should come first (higher strength)
    assert_eq!(rules[0].message, "string rule");
    assert_eq!(rules[1].message, "byte rule");
}

#[test]
fn test_sort_rules_by_strength_preserves_child_file_order() {
    // libmagic's `apprentice_sort` orders whole top-level magic entries by
    // their first line's strength but NEVER reorders continuation
    // (child) lines. `sort_rules_by_strength` is non-recursive to match:
    // it sorts only the top-level slice, leaving each rule's `children`
    // in source order. This is load-bearing for `default`/`clear` firing
    // and for multi-fragment descriptions (e.g. gzip's detail siblings).
    //
    // Build one top-level rule whose children are, in file order, a
    // low-strength `default` FIRST and a high-strength byte comparison
    // SECOND. A recursive sort would swap them (byte outranks default),
    // wrongly letting the comparison sibling suppress the `default`.
    let low_first = {
        let mut r = make_rule(
            TypeKind::Meta(crate::parser::ast::MetaType::Default),
            Operator::AnyValue,
            OffsetSpec::Absolute(0),
            Value::Uint(0),
        );
        r.message = "default-child".to_string();
        r.level = 1;
        r
    };
    let high_second = {
        let mut r = make_rule(
            TypeKind::Long {
                endian: crate::parser::ast::Endianness::Big,
                signed: false,
            },
            Operator::Equal,
            OffsetSpec::Absolute(0),
            Value::Uint(0xDEAD_BEEF),
        );
        r.message = "strong-child".to_string();
        r.level = 1;
        r
    };
    let mut parent = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    parent.message = "parent".to_string();
    parent.children = vec![low_first, high_second];

    let mut rules = vec![parent];
    sort_rules_by_strength(&mut rules);

    let child_order: Vec<&str> = rules[0]
        .children
        .iter()
        .map(|c| c.message.as_str())
        .collect();
    assert_eq!(
        child_order,
        vec!["default-child", "strong-child"],
        "child rules must stay in file order; the non-recursive sort must \
         not reorder continuation rules by strength"
    );
}

#[test]
fn test_sort_rules_by_strength_with_modifier() {
    let mut rules = vec![
        {
            let mut r = make_rule(
                TypeKind::String {
                    max_length: None,
                    flags: StringFlags::default(),
                },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::String("TEST".to_string()),
            );
            r.message = "string rule".to_string();
            // Lower the strength with a modifier
            r.strength_modifier = Some(StrengthModifier::Set(10));
            r
        },
        {
            let mut r = make_rule(
                TypeKind::Byte { signed: true },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::Uint(0),
            );
            r.message = "byte rule".to_string();
            // Boost the strength with a modifier
            r.strength_modifier = Some(StrengthModifier::Set(100));
            r
        },
    ];

    sort_rules_by_strength(&mut rules);

    // Byte rule should now come first due to strength modifier
    assert_eq!(rules[0].message, "byte rule");
    assert_eq!(rules[1].message, "string rule");
}

#[test]
fn test_sort_rules_empty() {
    let mut rules: Vec<MagicRule> = vec![];
    sort_rules_by_strength(&mut rules);
    assert!(rules.is_empty());
}

#[test]
fn test_sort_rules_single() {
    let mut rules = vec![make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    )];
    sort_rules_by_strength(&mut rules);
    assert_eq!(rules.len(), 1);
}

#[test]
fn test_into_sorted_by_strength() {
    let rules = vec![
        {
            let mut r = make_rule(
                TypeKind::Byte { signed: true },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::Uint(0),
            );
            r.message = "byte rule".to_string();
            r
        },
        {
            let mut r = make_rule(
                TypeKind::Long {
                    endian: Endianness::Big,
                    signed: false,
                },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::Uint(0),
            );
            r.message = "long rule".to_string();
            r
        },
    ];

    let sorted = into_sorted_by_strength(rules);

    // Long rule should come first (higher strength)
    assert_eq!(sorted[0].message, "long rule");
    assert_eq!(sorted[1].message, "byte rule");
}

// ============================================================
// Edge case and integration tests
// ============================================================

#[test]
fn test_strength_comparison_string_vs_byte() {
    let string_rule = make_rule(
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::String("AB".to_string()),
    );
    let byte_rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0x7f),
    );

    let string_strength = calculate_rule_strength(&string_rule);
    let byte_strength = calculate_rule_strength(&byte_rule);

    // String should have higher strength even with short value
    assert!(
        string_strength > byte_strength,
        "String strength {string_strength} should be > byte strength {byte_strength}"
    );
}

#[test]
fn test_strength_comparison_absolute_vs_relative_offset() {
    let absolute_rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0x7f),
    );
    let relative_rule = make_rule(
        TypeKind::Byte { signed: true },
        Operator::Equal,
        OffsetSpec::Relative(4),
        Value::Uint(0x7f),
    );

    let absolute_strength = calculate_rule_strength(&absolute_rule);
    let relative_strength = calculate_rule_strength(&relative_rule);

    // Absolute should have higher strength
    assert!(
        absolute_strength > relative_strength,
        "Absolute strength {absolute_strength} should be > relative strength {relative_strength}"
    );
}

// ============================================================
// MetaType strength tests
// ============================================================

fn meta_rule(meta: crate::parser::ast::MetaType, msg: &str) -> MagicRule {
    let mut rule = make_rule(
        TypeKind::Meta(meta),
        Operator::Equal,
        OffsetSpec::Absolute(0),
        Value::Uint(0),
    );
    rule.message = msg.to_string();
    rule
}

#[test]
fn test_meta_default_and_clear_sort_to_bottom() {
    use crate::parser::ast::MetaType;
    let mut rules = vec![
        meta_rule(MetaType::Default, "default"),
        meta_rule(MetaType::Clear, "clear"),
        {
            let mut r = make_rule(
                TypeKind::Byte { signed: true },
                Operator::Equal,
                OffsetSpec::Absolute(0),
                Value::Uint(0),
            );
            r.message = "byte".to_string();
            r
        },
    ];

    sort_rules_by_strength(&mut rules);

    // Byte rule has nonzero strength; default/clear are 0 + Equal 10 +
    // Absolute 10 + numeric 0 = 20. Byte is 5 + Equal 10 + Absolute 10
    // = 25 -- so byte sorts first.
    assert_eq!(rules[0].message, "byte");
}

#[test]
fn test_meta_use_and_indirect_sort_above_default() {
    use crate::parser::ast::MetaType;
    let use_rule = meta_rule(
        MetaType::Use {
            name: "sub".to_string(),
            flip_endian: false,
        },
        "use",
    );
    let indirect_rule = meta_rule(MetaType::Indirect, "indirect");
    let default_rule = meta_rule(MetaType::Default, "default");
    let clear_rule = meta_rule(MetaType::Clear, "clear");

    // use/indirect strength: 5 + Equal 10 + Absolute 10 = 25
    // default/clear strength: 0 + Equal 10 + Absolute 10 = 20
    assert!(
        calculate_default_strength(&use_rule) > calculate_default_strength(&default_rule),
        "use should sort above default"
    );
    assert!(
        calculate_default_strength(&indirect_rule) > calculate_default_strength(&default_rule),
        "indirect should sort above default"
    );
    assert!(
        calculate_default_strength(&use_rule) > calculate_default_strength(&clear_rule),
        "use should sort above clear"
    );
    assert!(
        calculate_default_strength(&indirect_rule) > calculate_default_strength(&clear_rule),
        "indirect should sort above clear"
    );
}

#[test]
fn test_meta_name_strength_is_zero() {
    use crate::parser::ast::MetaType;
    let name_rule = meta_rule(MetaType::Name("foo".to_string()), "name");
    let default_rule = meta_rule(MetaType::Default, "default");
    // Both Name and Default should produce identical strength scores
    // (both contribute 0 from the type axis).
    assert_eq!(
        calculate_default_strength(&name_rule),
        calculate_default_strength(&default_rule),
        "Name strength should equal Default strength (both type-axis 0)"
    );
}

/// Pin the canonical penalty per flag bit (pinned by request from
/// the `CodeRabbit` PR #288 review). Each row asserts which flags
/// reduce rule specificity (penalized) versus which do not
/// (non-penalized).
///
/// **Penalized**: `/c`, `/C`, `/w`, `/W` -- they broaden the match.
/// **Non-penalized**: `/T` (pattern-side trim, not fuzziness), `/b`
/// and `/t` (MIME-output hints, no comparison effect), `/f`
/// (TIGHTENS the match by requiring a word boundary).
///
/// Penalties also stack additively across multiple penalized flags
/// (e.g., `/cw` = 2 points).
#[test]
fn string_flag_specificity_penalty_per_flag_table() {
    let cases: &[(&str, StringFlags, i32)] = &[
        ("no flags", StringFlags::default(), 0),
        (
            "/c only",
            StringFlags::default().with_ignore_lowercase(true),
            1,
        ),
        (
            "/C only",
            StringFlags::default().with_ignore_uppercase(true),
            1,
        ),
        (
            "/w only",
            StringFlags::default().with_compact_optional_whitespace(true),
            1,
        ),
        (
            "/W only",
            StringFlags::default().with_compact_whitespace(true),
            1,
        ),
        (
            "/T only (non-penalized)",
            StringFlags::default().with_trim(true),
            0,
        ),
        (
            "/t only (non-penalized)",
            StringFlags::default().with_text_test(true),
            0,
        ),
        (
            "/b only (non-penalized)",
            StringFlags::default().with_bin_test(true),
            0,
        ),
        (
            "/f only (non-penalized)",
            StringFlags::default().with_full_word(true),
            0,
        ),
        (
            "/cw stacks (case + whitespace)",
            StringFlags::default()
                .with_ignore_lowercase(true)
                .with_compact_optional_whitespace(true),
            2,
        ),
        (
            "/cC stacks (both case folds)",
            StringFlags::default()
                .with_ignore_lowercase(true)
                .with_ignore_uppercase(true),
            2,
        ),
        (
            "all four penalized flags",
            StringFlags::default()
                .with_ignore_lowercase(true)
                .with_ignore_uppercase(true)
                .with_compact_whitespace(true)
                .with_compact_optional_whitespace(true),
            4,
        ),
        (
            "mixed: 2 penalized + 4 non-penalized",
            StringFlags::default()
                .with_ignore_lowercase(true)
                .with_compact_whitespace(true)
                .with_trim(true)
                .with_text_test(true)
                .with_bin_test(true)
                .with_full_word(true),
            2,
        ),
    ];

    for (label, flags, expected) in cases {
        let actual = string_flag_specificity_penalty(*flags);
        assert_eq!(
            actual, *expected,
            "case {label}: expected penalty {expected}, got {actual}"
        );
    }
}
