// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Operator application for magic rule evaluation
//!
//! This module provides functions for applying comparison and bitwise operators
//! to values during magic rule evaluation. It handles type-safe comparisons
//! between different Value variants. Supports XOR (`^`), NOT (`~`), and
//! any-value (`x`) matching in addition to equality, comparison, and bitwise AND.

mod bitwise;
mod comparison;
mod equality;

pub use bitwise::{
    apply_bitwise_and, apply_bitwise_and_mask, apply_bitwise_not, apply_bitwise_not_with_width,
    apply_bitwise_xor,
};
pub use comparison::{
    apply_greater_equal, apply_greater_than, apply_less_equal, apply_less_than, compare_values,
};
pub use equality::{apply_equal, apply_not_equal};

use crate::error::EvaluationError;
use crate::parser::ast::{Operator, Value, ValueTransform, ValueTransformOp};

/// Apply a magic-file pre-comparison `ValueTransform` to a numeric value
/// read from the file. The result is what the rule's comparison operator
/// sees, and what printf-style format specifiers (`%d`, `%x`, ...) render
/// into the message.
///
/// Magic file usage examples:
/// - `lelong+1 x volume %d` -- read a long, add 1, format as `%d`
/// - `ulequad/1073741824 x size %lluGB` -- read a quad, divide by 1 GiB
///
/// # Numeric promotion
///
/// Both `Value::Uint` and `Value::Int` operands are supported. The
/// transform is computed in the value's existing type:
/// - `Uint` op `i64` -> the `i64` operand is reinterpreted bitwise as
///   `u64` for `Mul`/`Or`/`Xor` and as `i64` for the others (matching
///   libmagic's `apprentice.c::mconvert`, which treats the operand as a
///   raw machine word for bitwise ops and a signed integer for
///   arithmetic). `Sub` on a `Uint` is rejected if it would underflow;
///   `Add` clamps a negative operand to subtraction.
/// - `Int` uses signed arithmetic throughout.
///
/// `Float`, `String`, and `Bytes` values are left unchanged because
/// magic-file arithmetic transforms are only meaningful on integer
/// reads. Returning the value unchanged keeps the comparison flow
/// well-defined while preserving the GOTCHAS S2.3 catch-all discipline
/// for unknown `Value` variants.
///
/// # Errors
///
/// Returns `EvaluationError::InvalidValueTransform` when:
/// - `Div` or `Mod` is applied with a zero operand;
/// - any arithmetic op overflows the natural integer range.
pub fn apply_value_transform(
    value: &Value,
    transform: ValueTransform,
) -> Result<Value, EvaluationError> {
    let signed_operand = transform.operand;
    // For bitwise ops on a `Uint`, reinterpret the i64 operand bit-for-bit
    // as u64. For arithmetic ops, use unsigned_abs / sign-aware
    // checked_{add,sub}.
    #[allow(clippy::cast_sign_loss)]
    let bitwise_operand = signed_operand as u64;

    match (value, transform.op) {
        (Value::Uint(v), ValueTransformOp::Add) => {
            let lhs = *v;
            // Allow negative operand to mean subtraction (consistent with
            // how `Sub` is encoded as `Add(-N)` on the parser side, but
            // the parser actually emits an explicit `Sub` op -- this
            // branch handles AST values constructed programmatically).
            let result = if signed_operand >= 0 {
                lhs.checked_add(signed_operand.unsigned_abs())
            } else {
                lhs.checked_sub(signed_operand.unsigned_abs())
            };
            result
                .map(Value::Uint)
                .ok_or_else(|| invalid_transform("Add", value, signed_operand))
        }
        (Value::Uint(v), ValueTransformOp::Sub) => {
            let lhs = *v;
            // Sub on a Uint with a positive operand subtracts; with a
            // negative operand it adds. Either way, reject overflow.
            let result = if signed_operand >= 0 {
                lhs.checked_sub(signed_operand.unsigned_abs())
            } else {
                lhs.checked_add(signed_operand.unsigned_abs())
            };
            result
                .map(Value::Uint)
                .ok_or_else(|| invalid_transform("Sub", value, signed_operand))
        }
        (Value::Uint(v), ValueTransformOp::Mul) => v
            .checked_mul(bitwise_operand)
            .map(Value::Uint)
            .ok_or_else(|| invalid_transform("Mul", value, signed_operand)),
        (Value::Uint(v), ValueTransformOp::Div) => {
            if signed_operand == 0 {
                return Err(invalid_transform("Div", value, signed_operand));
            }
            v.checked_div(bitwise_operand)
                .map(Value::Uint)
                .ok_or_else(|| invalid_transform("Div", value, signed_operand))
        }
        (Value::Uint(v), ValueTransformOp::Mod) => {
            if signed_operand == 0 {
                return Err(invalid_transform("Mod", value, signed_operand));
            }
            v.checked_rem(bitwise_operand)
                .map(Value::Uint)
                .ok_or_else(|| invalid_transform("Mod", value, signed_operand))
        }
        (Value::Uint(v), ValueTransformOp::BitAnd) => Ok(Value::Uint(v & bitwise_operand)),
        (Value::Uint(v), ValueTransformOp::Or) => Ok(Value::Uint(v | bitwise_operand)),
        (Value::Uint(v), ValueTransformOp::Xor) => Ok(Value::Uint(v ^ bitwise_operand)),

        (Value::Int(v), ValueTransformOp::Add) => v
            .checked_add(signed_operand)
            .map(Value::Int)
            .ok_or_else(|| invalid_transform("Add", value, signed_operand)),
        (Value::Int(v), ValueTransformOp::Sub) => v
            .checked_sub(signed_operand)
            .map(Value::Int)
            .ok_or_else(|| invalid_transform("Sub", value, signed_operand)),
        (Value::Int(v), ValueTransformOp::Mul) => v
            .checked_mul(signed_operand)
            .map(Value::Int)
            .ok_or_else(|| invalid_transform("Mul", value, signed_operand)),
        (Value::Int(v), ValueTransformOp::Div) => {
            if signed_operand == 0 {
                return Err(invalid_transform("Div", value, signed_operand));
            }
            v.checked_div(signed_operand)
                .map(Value::Int)
                .ok_or_else(|| invalid_transform("Div", value, signed_operand))
        }
        (Value::Int(v), ValueTransformOp::Mod) => {
            if signed_operand == 0 {
                return Err(invalid_transform("Mod", value, signed_operand));
            }
            v.checked_rem(signed_operand)
                .map(Value::Int)
                .ok_or_else(|| invalid_transform("Mod", value, signed_operand))
        }
        (Value::Int(v), ValueTransformOp::BitAnd) => {
            #[allow(clippy::cast_possible_wrap)]
            let result = *v & (bitwise_operand as i64);
            Ok(Value::Int(result))
        }
        (Value::Int(v), ValueTransformOp::Or) => {
            // Bitwise OR on i64 view of the integer.
            #[allow(clippy::cast_possible_wrap)]
            let result = *v | (bitwise_operand as i64);
            Ok(Value::Int(result))
        }
        (Value::Int(v), ValueTransformOp::Xor) => {
            #[allow(clippy::cast_possible_wrap)]
            let result = *v ^ (bitwise_operand as i64);
            Ok(Value::Int(result))
        }

        // Non-numeric values are returned unchanged. Magic-file
        // arithmetic transforms on a string-valued read are not
        // well-defined; libmagic-compatible behavior is to skip the
        // transform rather than fail the whole rule.
        _ => Ok(value.clone()),
    }
}

fn invalid_transform(op: &str, value: &Value, operand: i64) -> EvaluationError {
    EvaluationError::InvalidValueTransform {
        reason: format!("{op}({operand}) failed on {value:?} (overflow or div-by-zero)"),
    }
}

/// Apply any-value operator: always returns true (unconditional match).
///
/// The `x` operator in libmagic matches any value unconditionally. This is used
/// for rules that should always match at a given offset regardless of the data.
///
/// # Arguments
///
/// * `_left` - The left-hand side value (ignored)
/// * `_right` - The right-hand side value (ignored)
///
/// # Returns
///
/// Always returns `true`.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_any_value;
///
/// // Always returns true regardless of values
/// assert!(apply_any_value(&Value::Uint(0), &Value::Uint(0)));
/// assert!(apply_any_value(
///     &Value::String("anything".to_string()),
///     &Value::Uint(42),
/// ));
/// ```
#[must_use]
pub fn apply_any_value(_left: &Value, _right: &Value) -> bool {
    true
}

/// Apply operator to two values using the specified operator type
///
/// This is the main operator application interface that dispatches to the appropriate
/// operator function based on the `Operator` enum variant. This function serves as
/// the primary entry point for operator evaluation in magic rule processing.
///
/// # Arguments
///
/// * `operator` - The operator to apply (`Equal`, `NotEqual`, `LessThan`,
///   `GreaterThan`, `LessEqual`, `GreaterEqual`, `BitwiseAnd`, `BitwiseAndMask`,
///   `BitwiseXor`, `BitwiseNot`, or `AnyValue`)
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
///
/// // Bitwise XOR, NOT, and any-value
/// assert!(apply_operator(
///     &Operator::BitwiseXor,
///     &Value::Uint(0xFF),
///     &Value::Uint(0x0F)
/// ));
/// assert!(apply_operator(
///     &Operator::AnyValue,
///     &Value::Uint(0),
///     &Value::Uint(0)
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
        Operator::BitwiseXor => apply_bitwise_xor(left, right),
        Operator::BitwiseNot => apply_bitwise_not(left, right),
        Operator::AnyValue => apply_any_value(left, right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // apply_value_transform tests
    // ========================================================================

    fn t(op: ValueTransformOp, operand: i64) -> ValueTransform {
        ValueTransform { op, operand }
    }

    #[test]
    fn test_apply_value_transform_uint_arithmetic() {
        // Add: 100 + 1 = 101 (regression for `lelong+1` in filesystems)
        assert_eq!(
            apply_value_transform(&Value::Uint(100), t(ValueTransformOp::Add, 1)).unwrap(),
            Value::Uint(101)
        );
        // Sub: 100 - 25 = 75
        assert_eq!(
            apply_value_transform(&Value::Uint(100), t(ValueTransformOp::Sub, 25)).unwrap(),
            Value::Uint(75)
        );
        // Mul: 4 * 8 = 32
        assert_eq!(
            apply_value_transform(&Value::Uint(4), t(ValueTransformOp::Mul, 8)).unwrap(),
            Value::Uint(32)
        );
        // Div: 16 GiB / 1 GiB = 16 (regression for `ulequad/1073741824`)
        let sixteen_gib = 16u64 * 1024 * 1024 * 1024;
        assert_eq!(
            apply_value_transform(
                &Value::Uint(sixteen_gib),
                t(ValueTransformOp::Div, 1_073_741_824)
            )
            .unwrap(),
            Value::Uint(16)
        );
        // Mod: 17 % 5 = 2
        assert_eq!(
            apply_value_transform(&Value::Uint(17), t(ValueTransformOp::Mod, 5)).unwrap(),
            Value::Uint(2)
        );
        // Or:  0xF0 | 0x0F = 0xFF
        assert_eq!(
            apply_value_transform(&Value::Uint(0xF0), t(ValueTransformOp::Or, 0x0F)).unwrap(),
            Value::Uint(0xFF)
        );
        // Xor: 0xFF ^ 0x0F = 0xF0
        assert_eq!(
            apply_value_transform(&Value::Uint(0xFF), t(ValueTransformOp::Xor, 0x0F)).unwrap(),
            Value::Uint(0xF0)
        );
    }

    #[test]
    fn test_apply_value_transform_div_or_mod_by_zero_errors() {
        let err = apply_value_transform(&Value::Uint(10), t(ValueTransformOp::Div, 0)).unwrap_err();
        assert!(err.to_string().contains("Div"));
        let err = apply_value_transform(&Value::Uint(10), t(ValueTransformOp::Mod, 0)).unwrap_err();
        assert!(err.to_string().contains("Mod"));
    }

    #[test]
    fn test_apply_value_transform_overflow_errors() {
        let err =
            apply_value_transform(&Value::Uint(u64::MAX), t(ValueTransformOp::Add, 1)).unwrap_err();
        assert!(err.to_string().contains("Add"));
        let err =
            apply_value_transform(&Value::Uint(u64::MAX), t(ValueTransformOp::Mul, 2)).unwrap_err();
        assert!(err.to_string().contains("Mul"));
    }

    /// Regression: errors from `apply_value_transform` must be classified as
    /// `EvaluationError::InvalidValueTransform` so the engine's graceful-skip
    /// arm catches them. Previously the helper used `internal_error`, which is
    /// NOT in the graceful-skip list -- a single rule with `lequad*N` against
    /// an overflow-prone buffer would abort the entire evaluation instead of
    /// dropping the rule and continuing.
    #[test]
    fn test_apply_value_transform_errors_use_invalid_value_transform_variant() {
        let err = apply_value_transform(&Value::Uint(10), t(ValueTransformOp::Div, 0)).unwrap_err();
        assert!(
            matches!(err, EvaluationError::InvalidValueTransform { .. }),
            "Div-by-zero must produce InvalidValueTransform, got {err:?}"
        );

        let err =
            apply_value_transform(&Value::Uint(u64::MAX), t(ValueTransformOp::Mul, 2)).unwrap_err();
        assert!(
            matches!(err, EvaluationError::InvalidValueTransform { .. }),
            "Overflow must produce InvalidValueTransform, got {err:?}"
        );

        let err =
            apply_value_transform(&Value::Int(i64::MIN), t(ValueTransformOp::Sub, 1)).unwrap_err();
        assert!(
            matches!(err, EvaluationError::InvalidValueTransform { .. }),
            "Int underflow must produce InvalidValueTransform, got {err:?}"
        );
    }

    #[test]
    fn test_apply_value_transform_int_arithmetic() {
        assert_eq!(
            apply_value_transform(&Value::Int(-5), t(ValueTransformOp::Add, 10)).unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            apply_value_transform(&Value::Int(-3), t(ValueTransformOp::Mul, -2)).unwrap(),
            Value::Int(6)
        );
    }

    #[test]
    fn test_apply_value_transform_non_numeric_passthrough() {
        // String/Bytes/Float values are returned unchanged because
        // magic-file arithmetic transforms only make sense on integer
        // reads.
        let s = Value::String("abc".to_string());
        assert_eq!(
            apply_value_transform(&s, t(ValueTransformOp::Add, 1)).unwrap(),
            s
        );
    }

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

            assert_eq!(
                apply_operator(&Operator::BitwiseXor, &left, &right),
                apply_bitwise_xor(&left, &right),
                "apply_operator(BitwiseXor) should match apply_bitwise_xor for {left:?}, {right:?}"
            );

            assert_eq!(
                apply_operator(&Operator::BitwiseNot, &left, &right),
                apply_bitwise_not(&left, &right),
                "apply_operator(BitwiseNot) should match apply_bitwise_not for {left:?}, {right:?}"
            );

            assert!(
                apply_operator(&Operator::AnyValue, &left, &right),
                "apply_operator(AnyValue) should always be true for {left:?}, {right:?}"
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
        // Cross-type empty Bytes vs empty String now compare equal (libmagic
        // policy: same byte sequence wins). NotEqual flips accordingly.
        assert!(apply_operator(
            &Operator::Equal,
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
    fn test_apply_operator_bitwise_xor() {
        assert!(apply_operator(
            &Operator::BitwiseXor,
            &Value::Uint(0xFF),
            &Value::Uint(0x0F)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseXor,
            &Value::Uint(42),
            &Value::Uint(42)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseXor,
            &Value::String("x".to_string()),
            &Value::Uint(1)
        ));
    }

    #[test]
    fn test_apply_operator_bitwise_not() {
        assert!(apply_operator(
            &Operator::BitwiseNot,
            &Value::Uint(0),
            &Value::Uint(u64::MAX)
        ));
        assert!(apply_operator(
            &Operator::BitwiseNot,
            &Value::Int(-1),
            &Value::Int(0)
        ));
        assert!(!apply_operator(
            &Operator::BitwiseNot,
            &Value::Bytes(vec![0]),
            &Value::Uint(0xFF)
        ));
    }

    #[test]
    fn test_apply_operator_any_value() {
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::Uint(0),
            &Value::Uint(0)
        ));
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::Int(42),
            &Value::Int(0)
        ));
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::Bytes(vec![1, 2, 3]),
            &Value::Bytes(vec![])
        ));
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::String("x".to_string()),
            &Value::String("y".to_string())
        ));
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::Uint(1),
            &Value::String(String::new())
        ));
        assert!(apply_operator(
            &Operator::AnyValue,
            &Value::Bytes(vec![]),
            &Value::Bytes(vec![])
        ));
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
            Operator::BitwiseXor,
            Operator::BitwiseNot,
            Operator::AnyValue,
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
                        Operator::BitwiseXor => apply_bitwise_xor(left, right),
                        Operator::BitwiseNot => apply_bitwise_not(left, right),
                        Operator::AnyValue => apply_any_value(left, right),
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
