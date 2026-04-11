// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Strength calculation for magic rules
//!
//! This module implements the strength calculation algorithm based on libmagic's
//! `apprentice_magic_strength` function. Strength is used to order rules during
//! evaluation, giving priority to more specific rules.
//!
//! # Algorithm Overview
//!
//! The default strength of a rule is calculated based on several factors:
//! - **Type specificity**: String types have higher strength than numeric types
//! - **Operator specificity**: Equality operators are more specific than bitwise
//! - **Offset type**: Absolute offsets are more reliable than indirect/relative
//! - **Value length**: Longer strings are more specific matches
//!
//! The calculated strength can be modified using `!:strength` directives in magic
//! files, which apply arithmetic operations to the default strength.

use crate::parser::ast::{MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

/// Maximum strength value (clamped to prevent overflow)
pub const MAX_STRENGTH: i32 = 255;

/// Minimum strength value (clamped to prevent negative strength)
pub const MIN_STRENGTH: i32 = 0;

/// Calculate the default strength of a magic rule based on its specificity.
///
/// This function implements an algorithm inspired by libmagic's `apprentice_magic_strength`
/// function. The strength is calculated based on:
///
/// - **Type contribution**: How specific the type matching is
/// - **Operator contribution**: How specific the comparison is
/// - **Offset contribution**: How reliable the offset is
/// - **Value length contribution**: For strings, longer matches are more specific
///
/// # Arguments
///
/// * `rule` - The magic rule to calculate strength for
///
/// # Returns
///
/// The calculated default strength as an `i32`, clamped to `[MIN_STRENGTH, MAX_STRENGTH]`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::calculate_default_strength;
///
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::String { max_length: None },
///     op: Operator::Equal,
///     value: Value::String("ELF".to_string()),
///     message: "ELF file".to_string(),
///     children: vec![],
///     level: 0,
///     strength_modifier: None,
/// };
///
/// let strength = calculate_default_strength(&rule);
/// assert!(strength > 0);
/// ```
#[must_use]
pub fn calculate_default_strength(rule: &MagicRule) -> i32 {
    let mut strength: i32 = 0;

    // Type contribution: more specific types get higher strength
    strength += match &rule.typ {
        // Strings are most specific (they match exact byte sequences)
        TypeKind::String { max_length } | TypeKind::PString { max_length, .. } => {
            // Base string strength
            let base = 20;
            // Add bonus for limited-length strings (more constrained match)
            if max_length.is_some() { base + 5 } else { base }
        }
        // Regex matches a pattern -- treat similarly to an unbounded string.
        // A rule with an EXPLICIT count (byte count, or line count with a
        // specific N) is more constrained than a plain `regex` default, so
        // it gets the same bonus as a length-limited string. Note that
        // `RegexCount::Lines(None)` (the `regex/l` shorthand) has the same
        // effective scan window as `RegexCount::Default` -- both walk the
        // full 8192-byte capped window -- so they get the same strength
        // score. Giving `Lines(None)` the "constrained" bonus would reward
        // users for typing `/l` instead of nothing even though the scan
        // window is identical.
        TypeKind::Regex { count, .. } => {
            use crate::parser::ast::RegexCount;
            match count {
                RegexCount::Default | RegexCount::Lines(None) => 20,
                RegexCount::Bytes(_) | RegexCount::Lines(Some(_)) => 25,
            }
        }
        // Search is always a bounded scan (the range is mandatory), so it
        // gets the "constrained match" bonus unconditionally. This matches
        // the max_length bonus used for String and PString.
        TypeKind::Search { .. } => 25,
        // 64-bit types are most specific among numerics
        TypeKind::Quad { .. } | TypeKind::Double { .. } | TypeKind::QDate { .. } => 16,
        // 32-bit types are fairly specific
        TypeKind::Long { .. } | TypeKind::Float { .. } | TypeKind::Date { .. } => 15,
        // 16-bit integers are moderately specific
        TypeKind::Short { .. } => 10,
        // Single bytes are least specific
        TypeKind::Byte { .. } => 5,
    };

    // Operator contribution: equality is most specific
    strength += match &rule.op {
        // Exact equality is most specific
        Operator::Equal => 10,
        // Inequality is somewhat specific
        Operator::NotEqual => 5,
        // Comparison operators are moderately specific
        Operator::LessThan
        | Operator::GreaterThan
        | Operator::LessEqual
        | Operator::GreaterEqual => 6,
        // Bitwise AND with mask is moderately specific
        Operator::BitwiseAndMask(_) => 7,
        // Plain bitwise AND is least specific
        Operator::BitwiseAnd => 3,
        // Bitwise XOR and NOT are moderately specific
        Operator::BitwiseXor | Operator::BitwiseNot => 4,
        // Any value always matches, least specific
        Operator::AnyValue => 1,
    };

    // Offset contribution: absolute offsets are most reliable
    strength += match &rule.offset {
        // Absolute offsets are most reliable
        OffsetSpec::Absolute(_) => 10,
        // From-end offsets are also reliable (just from the other end)
        OffsetSpec::FromEnd(_) => 8,
        // Indirect offsets depend on reading a pointer first
        OffsetSpec::Indirect { .. } => 5,
        // Relative offsets depend on previous match position
        OffsetSpec::Relative(_) => 3,
    };

    // Value length contribution: longer values are more specific
    // Only applicable to string and bytes values
    let value_length_bonus = match &rule.value {
        Value::String(s) => {
            // Each character adds to specificity, capped at 20
            i32::try_from(s.len()).unwrap_or(20).min(20)
        }
        Value::Bytes(b) => {
            // Each byte adds to specificity, capped at 20
            i32::try_from(b.len()).unwrap_or(20).min(20)
        }
        // Numeric values don't get length bonus
        Value::Uint(_) | Value::Int(_) | Value::Float(_) => 0,
    };
    strength += value_length_bonus;

    // Clamp to valid range
    strength.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Apply a strength modifier to a base strength value.
///
/// This function applies the arithmetic operation specified by the `StrengthModifier`
/// to the given base strength. The result is clamped to `[MIN_STRENGTH, MAX_STRENGTH]`.
///
/// # Arguments
///
/// * `base_strength` - The default calculated strength
/// * `modifier` - The modifier to apply
///
/// # Returns
///
/// The modified strength, clamped to valid range
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::StrengthModifier;
/// use libmagic_rs::evaluator::strength::apply_strength_modifier;
///
/// // Add 10 to strength
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Add(10)), 60);
///
/// // Subtract 5 from strength
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Subtract(5)), 45);
///
/// // Multiply by 2
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Multiply(2)), 100);
///
/// // Divide by 2
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Divide(2)), 25);
///
/// // Set to absolute value
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Set(75)), 75);
/// ```
#[must_use]
pub fn apply_strength_modifier(base_strength: i32, modifier: &StrengthModifier) -> i32 {
    let result = match modifier {
        StrengthModifier::Add(n) => base_strength.saturating_add(*n),
        StrengthModifier::Subtract(n) => base_strength.saturating_sub(*n),
        StrengthModifier::Multiply(n) => base_strength.saturating_mul(*n),
        StrengthModifier::Divide(n) => {
            if *n == 0 {
                // Division by zero: return base strength unchanged
                // (magic file contains !:strength /0 which is invalid)
                base_strength
            } else {
                base_strength / n
            }
        }
        StrengthModifier::Set(n) => *n,
    };

    // Clamp to valid range
    result.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Calculate the final strength of a magic rule, including any modifiers.
///
/// This function first calculates the default strength based on the rule's
/// specificity, then applies any strength modifier if present.
///
/// # Arguments
///
/// * `rule` - The magic rule to calculate strength for
///
/// # Returns
///
/// The final calculated strength, clamped to `[MIN_STRENGTH, MAX_STRENGTH]`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value, StrengthModifier};
/// use libmagic_rs::evaluator::strength::calculate_rule_strength;
///
/// let rule = MagicRule {
///     offset: OffsetSpec::Absolute(0),
///     typ: TypeKind::Byte { signed: true },
///     op: Operator::Equal,
///     value: Value::Uint(0x7f),
///     message: "ELF magic".to_string(),
///     children: vec![],
///     level: 0,
///     strength_modifier: Some(StrengthModifier::Add(20)),
/// };
///
/// let strength = calculate_rule_strength(&rule);
/// // Base: 5 (byte) + 10 (equal) + 10 (absolute) + 0 (numeric) = 25
/// // With modifier: 25 + 20 = 45
/// assert_eq!(strength, 45);
/// ```
#[must_use]
pub fn calculate_rule_strength(rule: &MagicRule) -> i32 {
    let base_strength = calculate_default_strength(rule);

    if let Some(ref modifier) = rule.strength_modifier {
        apply_strength_modifier(base_strength, modifier)
    } else {
        base_strength
    }
}

/// Sort magic rules by their calculated strength in descending order.
///
/// Higher strength rules are evaluated first, as they represent more specific
/// matches. This function sorts the rules in-place.
///
/// # Arguments
///
/// * `rules` - The slice of magic rules to sort
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::sort_rules_by_strength;
///
/// let mut rules = vec![
///     MagicRule {
///         offset: OffsetSpec::Absolute(0),
///         typ: TypeKind::Byte { signed: true },
///         op: Operator::Equal,
///         value: Value::Uint(0x7f),
///         message: "byte rule".to_string(),
///         children: vec![],
///         level: 0,
///         strength_modifier: None,
///     },
///     MagicRule {
///         offset: OffsetSpec::Absolute(0),
///         typ: TypeKind::String { max_length: None },
///         op: Operator::Equal,
///         value: Value::String("MAGIC".to_string()),
///         message: "string rule".to_string(),
///         children: vec![],
///         level: 0,
///         strength_modifier: None,
///     },
/// ];
///
/// sort_rules_by_strength(&mut rules);
///
/// // String rule should come first (higher strength)
/// assert_eq!(rules[0].message, "string rule");
/// assert_eq!(rules[1].message, "byte rule");
/// ```
pub fn sort_rules_by_strength(rules: &mut [MagicRule]) {
    // Use a stable sort keyed on the negated strength so that higher-strength
    // rules come first while preserving source order for ties. This avoids
    // breaking tests that rely on deterministic ordering of equal-strength
    // rules.
    rules.sort_by_cached_key(|rule| calculate_rule_strength(rule).saturating_neg());
}

/// Sort magic rules by strength in descending order, recursively sorting child
/// rules as well.
///
/// This is intended for use at magic database load time so that first-match
/// evaluation encounters more-specific rules earlier. Child rules (nested
/// under a parent match) are also sorted so that the same ordering benefit
/// applies within each hierarchical level.
///
/// The sort is stable: rules with equal strength preserve their source
/// order, so test assertions and libmagic-file semantics that depend on
/// the original ordering of equal-strength siblings continue to hold.
///
/// # Arguments
///
/// * `rules` - The slice of magic rules to sort (in-place, recursive)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::sort_rules_by_strength_recursive;
///
/// let mut rules: Vec<MagicRule> = vec![];
/// sort_rules_by_strength_recursive(&mut rules);
/// assert!(rules.is_empty());
/// ```
pub fn sort_rules_by_strength_recursive(rules: &mut [MagicRule]) {
    sort_rules_by_strength(rules);
    for rule in rules.iter_mut() {
        sort_rules_by_strength_recursive(&mut rule.children);
    }
}

/// Sort magic rules by strength and return the sorted vec (consuming the input).
///
/// This is a convenience function that takes ownership of the rules vector,
/// sorts it by strength, and returns the sorted vector.
///
/// # Arguments
///
/// * `rules` - The vector of magic rules to sort
///
/// # Returns
///
/// The sorted vector with higher strength rules first
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::into_sorted_by_strength;
///
/// let rules = vec![
///     MagicRule {
///         offset: OffsetSpec::Absolute(0),
///         typ: TypeKind::Byte { signed: true },
///         op: Operator::Equal,
///         value: Value::Uint(0),
///         message: "byte rule".to_string(),
///         children: vec![],
///         level: 0,
///         strength_modifier: None,
///     },
///     MagicRule {
///         offset: OffsetSpec::Absolute(0),
///         typ: TypeKind::String { max_length: None },
///         op: Operator::Equal,
///         value: Value::String("MAGIC".to_string()),
///         message: "string rule".to_string(),
///         children: vec![],
///         level: 0,
///         strength_modifier: None,
///     },
/// ];
///
/// let sorted = into_sorted_by_strength(rules);
/// assert_eq!(sorted[0].message, "string rule");
/// ```
#[must_use]
pub fn into_sorted_by_strength(mut rules: Vec<MagicRule>) -> Vec<MagicRule> {
    sort_rules_by_strength(&mut rules);
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Endianness;

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
                        TypeKind::String { max_length: None },
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
                            pointer_type: TypeKind::Long {
                                endian: Endianness::Little,
                                signed: false,
                            },
                            adjustment: 0,
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
                        TypeKind::String { max_length: None },
                        Operator::Equal,
                        OffsetSpec::Absolute(0),
                        Value::String(
                            "This is a very long string that exceeds the cap".to_string(),
                        ),
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
                    TypeKind::String { max_length: None },
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
    fn test_sort_rules_by_strength_with_modifier() {
        let mut rules = vec![
            {
                let mut r = make_rule(
                    TypeKind::String { max_length: None },
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
            TypeKind::String { max_length: None },
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
}
