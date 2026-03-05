// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Operator application for magic rule evaluation
//!
//! This module provides functions for applying comparison and bitwise operators
//! to values during magic rule evaluation. It handles type-safe comparisons
//! between different Value variants.

mod bitwise;
mod comparison;
mod equality;

pub use bitwise::{apply_bitwise_and, apply_bitwise_and_mask};
pub use comparison::{
    apply_greater_equal, apply_greater_than, apply_less_equal, apply_less_than, compare_values,
};
pub use equality::{apply_equal, apply_not_equal};

use crate::parser::ast::{Operator, Value};

/// Apply operator to two values using the specified operator type
///
/// This is the main operator application interface that dispatches to the appropriate
/// operator function based on the `Operator` enum variant. This function serves as
/// the primary entry point for operator evaluation in magic rule processing.
///
/// # Arguments
///
/// * `operator` - The operator to apply (`Equal`, `NotEqual`, `LessThan`,
///   `GreaterThan`, `LessEqual`, `GreaterEqual`, `BitwiseAnd`, or `BitwiseAndMask`)
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the operator condition is satisfied, `false` otherwise
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{Operator, Value};
/// use libmagic_rs::evaluator::operators::apply_operator;
///
/// // Equality comparison
/// assert!(apply_operator(
///     &Operator::Equal,
///     &Value::Uint(42),
///     &Value::Uint(42)
/// ));
///
/// // Inequality comparison
/// assert!(apply_operator(
///     &Operator::NotEqual,
///     &Value::Uint(42),
///     &Value::Uint(24)
/// ));
///
/// // Less-than comparison
/// assert!(apply_operator(
///     &Operator::LessThan,
///     &Value::Uint(5),
///     &Value::Uint(10)
/// ));
///
/// // Greater-than comparison
/// assert!(apply_operator(
///     &Operator::GreaterThan,
///     &Value::Uint(10),
///     &Value::Uint(5)
/// ));
///
/// // Less-than-or-equal comparison
/// assert!(apply_operator(
///     &Operator::LessEqual,
///     &Value::Uint(10),
///     &Value::Uint(10)
/// ));
///
/// // Greater-than-or-equal comparison
/// assert!(apply_operator(
///     &Operator::GreaterEqual,
///     &Value::Uint(10),
///     &Value::Uint(10)
/// ));
///
/// // Bitwise AND operation
/// assert!(apply_operator(
///     &Operator::BitwiseAnd,
///     &Value::Uint(0xFF),
///     &Value::Uint(0x0F)
/// ));
///
/// // Cross-type integer coercion
/// assert!(apply_operator(
///     &Operator::Equal,
///     &Value::Uint(42),
///     &Value::Int(42)
/// ));
/// ```
#[must_use]
pub fn apply_operator(operator: &Operator, left: &Value, right: &Value) -> bool {
    match operator {
        Operator::Equal => apply_equal(left, right),
        Operator::NotEqual => apply_not_equal(left, right),
        Operator::LessThan => apply_less_than(left, right),
        Operator::GreaterThan => apply_greater_than(left, right),
        Operator::LessEqual => apply_less_equal(left, right),
        Operator::GreaterEqual => apply_greater_equal(left, right),
        Operator::BitwiseAnd => apply_bitwise_and(left, right),
        Operator::BitwiseAndMask(mask) => apply_bitwise_and_mask(*mask, left, right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_operator_equal() {
        // Test Equal operator dispatch
        assert!(apply_operator(
            &Operator::Equal,
            &Value::Uint(42),
            &Value::Uint(42)
        ));
        assert!(!apply_operator(
            &Operator::Equal,
            &Value::Uint(42),
            &Value::Uint(24)
        ));

        // Test with different value types
        assert!(apply_operator(
            &Operator::Equal,
            &Value::String("hello".to_string()),
            &Value::String("hello".to_string())
        ));
        assert!(!apply_operator(
            &Operator::Equal,
            &Value::String("hello".to_string()),
            &Value::String("world".to_string())
        ));

        // Cross-type integer coercion
        assert!(apply_operator(
            &Operator::Equal,
            &Value::Uint(42),
            &Value::Int(42)
        ));
    }

    #[test]
    fn test_apply_operator_not_equal() {
        // Test NotEqual operator dispatch
        assert!(!apply_operator(
            &Operator::NotEqual,
            &Value::Uint(42),
            &Value::Uint(42)
        ));
        assert!(apply_operator(
            &Operator::NotEqual,
            &Value::Uint(42),
            &Value::Uint(24)
        ));

        // Test with different value types
        assert!(!apply_operator(
            &Operator::NotEqual,
            &Value::String("hello".to_string()),
            &Value::String("hello".to_string())
        ));
        assert!(apply_operator(
            &Operator::NotEqual,
            &Value::String("hello".to_string()),
            &Value::String("world".to_string())
        ));

        // Cross-type integer coercion: same value, so not-equal is false
        assert!(!apply_operator(
            &Operator::NotEqual,
            &Value::Uint(42),
            &Value::Int(42)
        ));
    }

    #[test]
    fn test_apply_operator_bitwise_and() {
        // Test BitwiseAnd operator dispatch
        assert!(apply_operator(
            &Operator::BitwiseAnd,
            &Value::Uint(0xFF),
            &Value::Uint(0x0F)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &Value::Uint(0xF0),
            &Value::Uint(0x0F)
        ));

        // Test with signed integers
        assert!(apply_operator(
            &Operator::BitwiseAnd,
            &Value::Int(-1),
            &Value::Int(1)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &Value::Int(-2),
            &Value::Int(1)
        ));

        // Test with mixed types
        assert!(apply_operator(
            &Operator::BitwiseAnd,
            &Value::Uint(0xFF),
            &Value::Int(0x0F)
        ));

        // Non-integer types should return false
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &Value::String("test".to_string()),
            &Value::Uint(0x01)
        ));
    }

    #[test]
    fn test_apply_operator_all_operators_with_same_values() {
        let test_cases = vec![
            // Same values - Equal should be true, NotEqual false, BitwiseAnd depends on value
            (Value::Uint(42), Value::Uint(42)),
            (Value::Int(-100), Value::Int(-100)),
            (
                Value::String("test".to_string()),
                Value::String("test".to_string()),
            ),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![1, 2, 3])),
        ];

        for (left, right) in test_cases {
            // Equal should always be true for same values
            assert!(
                apply_operator(&Operator::Equal, &left, &right),
                "Equal should be true for same values: {left:?} == {right:?}"
            );

            // NotEqual should always be false for same values
            assert!(
                !apply_operator(&Operator::NotEqual, &left, &right),
                "NotEqual should be false for same values: {left:?} != {right:?}"
            );

            // BitwiseAnd behavior depends on the value type and content
            let bitwise_result = apply_operator(&Operator::BitwiseAnd, &left, &right);
            match &left {
                Value::Uint(n) => {
                    // For unsigned integers, BitwiseAnd should be true if value is non-zero
                    let expected = *n != 0;
                    assert_eq!(
                        bitwise_result, expected,
                        "BitwiseAnd for Uint({n}) should be {expected}"
                    );
                }
                Value::Int(n) => {
                    // For signed integers, BitwiseAnd should be true if value is non-zero
                    let expected = *n != 0;
                    assert_eq!(
                        bitwise_result, expected,
                        "BitwiseAnd for Int({n}) should be {expected}"
                    );
                }
                _ => {
                    // For non-integers, BitwiseAnd should always be false
                    assert!(
                        !bitwise_result,
                        "BitwiseAnd should be false for non-integer types: {left:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_apply_operator_all_operators_with_different_values() {
        let test_cases = vec![
            // Different values of same type
            (Value::Uint(42), Value::Uint(24)),
            (Value::Int(100), Value::Int(-100)),
            (
                Value::String("hello".to_string()),
                Value::String("world".to_string()),
            ),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![4, 5, 6])),
            // Different types (non-coercible)
            (Value::Uint(42), Value::String("42".to_string())),
            (Value::Int(42), Value::Bytes(vec![42])),
        ];

        for (left, right) in test_cases {
            // Equal should always be false for truly different values
            assert!(
                !apply_operator(&Operator::Equal, &left, &right),
                "Equal should be false for different values: {left:?} == {right:?}"
            );

            // NotEqual should always be true for truly different values
            assert!(
                apply_operator(&Operator::NotEqual, &left, &right),
                "NotEqual should be true for different values: {left:?} != {right:?}"
            );

            // BitwiseAnd behavior depends on the value types and content
            let bitwise_result = apply_operator(&Operator::BitwiseAnd, &left, &right);
            match (&left, &right) {
                (Value::Uint(a), Value::Uint(b)) => {
                    let expected = (a & b) != 0;
                    assert_eq!(
                        bitwise_result, expected,
                        "BitwiseAnd for Uint({a}) & Uint({b}) should be {expected}"
                    );
                }
                (Value::Int(a), Value::Int(b)) => {
                    #[allow(clippy::cast_sign_loss)]
                    let expected = ((*a as u64) & (*b as u64)) != 0;
                    assert_eq!(
                        bitwise_result, expected,
                        "BitwiseAnd for Int({a}) & Int({b}) should be {expected}"
                    );
                }
                (Value::Uint(a), Value::Int(b)) | (Value::Int(b), Value::Uint(a)) => {
                    #[allow(clippy::cast_sign_loss)]
                    let expected = (a & (*b as u64)) != 0;
                    assert_eq!(
                        bitwise_result, expected,
                        "BitwiseAnd for mixed Uint/Int should be {expected}"
                    );
                }
                _ => {
                    // For non-integer types, BitwiseAnd should always be false
                    assert!(
                        !bitwise_result,
                        "BitwiseAnd should be false for non-integer types: {left:?} & {right:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_apply_operator_consistency_with_individual_functions() {
        let test_cases = vec![
            (Value::Uint(42), Value::Uint(42)),
            (Value::Uint(42), Value::Uint(24)),
            (Value::Int(-100), Value::Int(-100)),
            (Value::Int(100), Value::Int(-100)),
            (
                Value::String("test".to_string()),
                Value::String("test".to_string()),
            ),
            (
                Value::String("hello".to_string()),
                Value::String("world".to_string()),
            ),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![1, 2, 3])),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![4, 5, 6])),
            // Cross-type cases
            (Value::Uint(42), Value::Int(42)),
            (Value::Uint(42), Value::String("42".to_string())),
            (Value::Int(42), Value::Bytes(vec![42])),
        ];

        for (left, right) in test_cases {
            // Test that apply_operator gives same results as individual functions
            assert_eq!(
                apply_operator(&Operator::Equal, &left, &right),
                apply_equal(&left, &right),
                "apply_operator(Equal) should match apply_equal for {left:?}, {right:?}"
            );

            assert_eq!(
                apply_operator(&Operator::NotEqual, &left, &right),
                apply_not_equal(&left, &right),
                "apply_operator(NotEqual) should match apply_not_equal for {left:?}, {right:?}"
            );

            assert_eq!(
                apply_operator(&Operator::BitwiseAnd, &left, &right),
                apply_bitwise_and(&left, &right),
                "apply_operator(BitwiseAnd) should match apply_bitwise_and for {left:?}, {right:?}"
            );
        }
    }

    #[test]
    fn test_apply_operator_magic_rule_scenarios() {
        // Test scenarios that would commonly appear in magic rules

        // ELF magic number check
        let elf_magic = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        let elf_expected = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert!(apply_operator(&Operator::Equal, &elf_magic, &elf_expected));
        assert!(!apply_operator(
            &Operator::NotEqual,
            &elf_magic,
            &elf_expected
        ));

        // ZIP magic number check
        let zip_magic = Value::Uint(0x504B_0304);
        let zip_expected = Value::Uint(0x504B_0304);
        assert!(apply_operator(&Operator::Equal, &zip_magic, &zip_expected));

        // Bit flag checking (common in binary formats)
        let flags = Value::Uint(0b1101_0110);
        let flag_mask = Value::Uint(0b0000_0010); // Check if bit 1 is set
        assert!(apply_operator(&Operator::BitwiseAnd, &flags, &flag_mask));

        let no_flag_mask = Value::Uint(0b0000_0001); // Check if bit 0 is set
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &flags,
            &no_flag_mask
        ));

        // String matching for text-based formats
        let content = Value::String("#!/bin/bash".to_string());
        let shebang = Value::String("#!/bin/bash".to_string());
        assert!(apply_operator(&Operator::Equal, &content, &shebang));

        let not_shebang = Value::String("#!/usr/bin/python".to_string());
        assert!(apply_operator(&Operator::NotEqual, &content, &not_shebang));

        // Version number checking
        let version = Value::Uint(2);
        let expected_version = Value::Uint(2);
        let old_version = Value::Uint(1);
        assert!(apply_operator(
            &Operator::Equal,
            &version,
            &expected_version
        ));
        assert!(apply_operator(&Operator::NotEqual, &version, &old_version));
    }

    #[test]
    fn test_apply_operator_edge_cases() {
        // Test with extreme values
        let max_uint = Value::Uint(u64::MAX);
        let min_signed = Value::Int(i64::MIN);
        let max_signed = Value::Int(i64::MAX);

        // Self-comparison should work
        assert!(apply_operator(&Operator::Equal, &max_uint, &max_uint));
        assert!(apply_operator(&Operator::Equal, &min_signed, &min_signed));
        assert!(apply_operator(&Operator::Equal, &max_signed, &max_signed));

        // Cross-extreme comparisons
        assert!(apply_operator(&Operator::NotEqual, &max_uint, &min_signed));
        assert!(apply_operator(
            &Operator::NotEqual,
            &max_signed,
            &min_signed
        ));

        // Bitwise operations with extreme values
        assert!(apply_operator(
            &Operator::BitwiseAnd,
            &max_uint,
            &Value::Uint(1)
        ));
        assert!(apply_operator(
            &Operator::BitwiseAnd,
            &min_signed,
            &min_signed
        ));

        // Empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let empty_string = Value::String(String::new());
        assert!(apply_operator(&Operator::Equal, &empty_bytes, &empty_bytes));
        assert!(apply_operator(
            &Operator::Equal,
            &empty_string,
            &empty_string
        ));
        assert!(apply_operator(
            &Operator::NotEqual,
            &empty_bytes,
            &empty_string
        ));

        // Zero values
        let zero_uint = Value::Uint(0);
        let zero_signed = Value::Int(0);
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &zero_uint,
            &Value::Uint(0xFF)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseAnd,
            &zero_signed,
            &Value::Int(0xFF)
        ));
        assert!(!apply_operator(
            &Operator::NotEqual,
            &zero_uint,
            &zero_signed
        )); // Cross-type integer coercion: 0 == 0
    }

    #[test]
    fn test_apply_operator_all_combinations() {
        let operators = [
            Operator::Equal,
            Operator::NotEqual,
            Operator::LessThan,
            Operator::GreaterThan,
            Operator::LessEqual,
            Operator::GreaterEqual,
            Operator::BitwiseAnd,
            Operator::BitwiseAndMask(0xFF),
        ];
        let values = [
            Value::Uint(42),
            Value::Int(-42),
            Value::Bytes(vec![42]),
            Value::String("42".to_string()),
        ];

        // Test all operator-value combinations to ensure no panics
        for operator in &operators {
            for left in &values {
                for right in &values {
                    // This should not panic for any combination
                    let result = apply_operator(operator, left, right);

                    // Verify the result is consistent with individual function calls
                    let expected = match operator {
                        Operator::Equal => apply_equal(left, right),
                        Operator::NotEqual => apply_not_equal(left, right),
                        Operator::LessThan => apply_less_than(left, right),
                        Operator::GreaterThan => apply_greater_than(left, right),
                        Operator::LessEqual => apply_less_equal(left, right),
                        Operator::GreaterEqual => apply_greater_equal(left, right),
                        Operator::BitwiseAnd => apply_bitwise_and(left, right),
                        Operator::BitwiseAndMask(mask) => {
                            apply_bitwise_and_mask(*mask, left, right)
                        }
                    };

                    assert_eq!(
                        result, expected,
                        "apply_operator({operator:?}, {left:?}, {right:?}) should match individual function"
                    );
                }
            }
        }
    }
}
