//! Operator application for magic rule evaluation
//!
//! This module provides functions for applying comparison and bitwise operators
//! to values during magic rule evaluation. It handles type-safe comparisons
//! between different Value variants.

use crate::parser::ast::{Operator, Value};

/// Apply equality comparison between two values
///
/// Compares two `Value` instances for equality, handling proper type matching.
/// Values of different types are considered unequal.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the values are equal and of the same type, `false` otherwise
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_equal;
///
/// // Same type, same value
/// assert!(apply_equal(&Value::Uint(42), &Value::Uint(42)));
///
/// // Same type, different value
/// assert!(!apply_equal(&Value::Uint(42), &Value::Uint(24)));
///
/// // Different types, same numeric value
/// assert!(!apply_equal(&Value::Uint(42), &Value::Int(42)));
///
/// // String comparison
/// assert!(apply_equal(
///     &Value::String("hello".to_string()),
///     &Value::String("hello".to_string())
/// ));
/// ```
#[must_use]
pub fn apply_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        // Unsigned integer comparison
        (Value::Uint(a), Value::Uint(b)) => a == b,

        // Signed integer comparison
        (Value::Int(a), Value::Int(b)) => a == b,

        // Byte sequence comparison
        (Value::Bytes(a), Value::Bytes(b)) => a == b,

        // String comparison
        (Value::String(a), Value::String(b)) => a == b,

        // Different types are never equal
        _ => false,
    }
}

/// Apply inequality comparison between two values
///
/// Compares two `Value` instances for inequality, implementing the negation
/// of equality comparison logic. Returns `true` if the values are not equal
/// or are of different types.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the values are not equal or of different types, `false` if they are equal
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_not_equal;
///
/// // Same type, different value
/// assert!(apply_not_equal(&Value::Uint(42), &Value::Uint(24)));
///
/// // Same type, same value
/// assert!(!apply_not_equal(&Value::Uint(42), &Value::Uint(42)));
///
/// // Different types (always not equal)
/// assert!(apply_not_equal(&Value::Uint(42), &Value::Int(42)));
///
/// // String comparison
/// assert!(apply_not_equal(
///     &Value::String("hello".to_string()),
///     &Value::String("world".to_string())
/// ));
/// ```
#[must_use]
pub fn apply_not_equal(left: &Value, right: &Value) -> bool {
    !apply_equal(left, right)
}

/// Apply bitwise AND operation for pattern matching
///
/// Performs bitwise AND operation between two integer values for pattern matching.
/// This is commonly used in magic rules to check if specific bits are set in a value.
/// Only works with integer types (Uint and Int), returns `false` for other types.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically the mask from magic rule)
///
/// # Returns
///
/// `true` if the bitwise AND result is non-zero, `false` otherwise or for non-integer types
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_bitwise_and;
///
/// // Check if bit 0 is set
/// assert!(apply_bitwise_and(&Value::Uint(0x01), &Value::Uint(0x01)));
/// assert!(!apply_bitwise_and(&Value::Uint(0x02), &Value::Uint(0x01)));
///
/// // Check multiple bits
/// assert!(apply_bitwise_and(&Value::Uint(0xFF), &Value::Uint(0x0F)));
/// assert!(!apply_bitwise_and(&Value::Uint(0xF0), &Value::Uint(0x0F)));
///
/// // Works with signed integers too
/// assert!(apply_bitwise_and(&Value::Int(-1), &Value::Int(0x01)));
///
/// // Non-integer types return false
/// assert!(!apply_bitwise_and(&Value::String("test".to_string()), &Value::Uint(0x01)));
/// ```
#[must_use]
pub fn apply_bitwise_and(left: &Value, right: &Value) -> bool {
    match (left, right) {
        // Unsigned integer bitwise AND
        (Value::Uint(a), Value::Uint(b)) => (a & b) != 0,

        // Signed integer bitwise AND (cast to unsigned for bitwise operations)
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Int(b)) => ((*a as u64) & (*b as u64)) != 0,

        // Mixed signed/unsigned integer bitwise AND
        #[allow(clippy::cast_sign_loss)]
        (Value::Uint(a), Value::Int(b)) => (a & (*b as u64)) != 0,
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Uint(b)) => ((*a as u64) & b) != 0,

        // Non-integer types cannot perform bitwise AND
        _ => false,
    }
}

/// Apply operator to two values using the specified operator type
///
/// This is the main operator application interface that dispatches to the appropriate
/// operator function based on the `Operator` enum variant. This function serves as
/// the primary entry point for operator evaluation in magic rule processing.
///
/// # Arguments
///
/// * `operator` - The operator to apply (`Equal`, `NotEqual`, or `BitwiseAnd`)
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
/// // Bitwise AND operation
/// assert!(apply_operator(
///     &Operator::BitwiseAnd,
///     &Value::Uint(0xFF),
///     &Value::Uint(0x0F)
/// ));
///
/// // Cross-type comparisons
/// assert!(!apply_operator(
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
        Operator::BitwiseAnd => apply_bitwise_and(left, right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_equal_uint_same_value() {
        let left = Value::Uint(42);
        let right = Value::Uint(42);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_uint_different_value() {
        let left = Value::Uint(42);
        let right = Value::Uint(24);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_uint_zero() {
        let left = Value::Uint(0);
        let right = Value::Uint(0);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_uint_max_value() {
        let left = Value::Uint(u64::MAX);
        let right = Value::Uint(u64::MAX);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_same_value() {
        let left = Value::Int(42);
        let right = Value::Int(42);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_different_value() {
        let left = Value::Int(42);
        let right = Value::Int(-42);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_negative() {
        let left = Value::Int(-100);
        let right = Value::Int(-100);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_zero() {
        let left = Value::Int(0);
        let right = Value::Int(0);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_extreme_values() {
        let left = Value::Int(i64::MAX);
        let right = Value::Int(i64::MAX);
        assert!(apply_equal(&left, &right));

        let left = Value::Int(i64::MIN);
        let right = Value::Int(i64::MIN);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_same_value() {
        let left = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        let right = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_different_value() {
        let left = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        let right = Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_empty() {
        let left = Value::Bytes(vec![]);
        let right = Value::Bytes(vec![]);
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_different_length() {
        let left = Value::Bytes(vec![0x7f, 0x45]);
        let right = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_single_byte() {
        let left = Value::Bytes(vec![0x7f]);
        let right = Value::Bytes(vec![0x7f]);
        assert!(apply_equal(&left, &right));

        let left = Value::Bytes(vec![0x7f]);
        let right = Value::Bytes(vec![0x45]);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_same_value() {
        let left = Value::String("hello".to_string());
        let right = Value::String("hello".to_string());
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_different_value() {
        let left = Value::String("hello".to_string());
        let right = Value::String("world".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_empty() {
        let left = Value::String(String::new());
        let right = Value::String(String::new());
        assert!(apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_case_sensitive() {
        let left = Value::String("Hello".to_string());
        let right = Value::String("hello".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_unicode() {
        let left = Value::String("🦀 Rust".to_string());
        let right = Value::String("🦀 Rust".to_string());
        assert!(apply_equal(&left, &right));

        let left = Value::String("🦀 Rust".to_string());
        let right = Value::String("🐍 Python".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_string_whitespace() {
        let left = Value::String("hello world".to_string());
        let right = Value::String("hello world".to_string());
        assert!(apply_equal(&left, &right));

        let left = Value::String("hello world".to_string());
        let right = Value::String("hello  world".to_string()); // Extra space
        assert!(!apply_equal(&left, &right));
    }

    // Cross-type comparison tests (should all return false)
    #[test]
    fn test_apply_equal_uint_vs_int() {
        let left = Value::Uint(42);
        let right = Value::Int(42);
        assert!(!apply_equal(&left, &right));

        let left = Value::Uint(0);
        let right = Value::Int(0);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_uint_vs_bytes() {
        let left = Value::Uint(42);
        let right = Value::Bytes(vec![42]);
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_uint_vs_string() {
        let left = Value::Uint(42);
        let right = Value::String("42".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_vs_bytes() {
        let left = Value::Int(-42);
        let right = Value::Bytes(vec![214]); // -42 as u8
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_int_vs_string() {
        let left = Value::Int(-42);
        let right = Value::String("-42".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_bytes_vs_string() {
        let left = Value::Bytes(vec![104, 101, 108, 108, 111]); // "hello" as bytes
        let right = Value::String("hello".to_string());
        assert!(!apply_equal(&left, &right));
    }

    #[test]
    fn test_apply_equal_all_cross_type_combinations() {
        let values = [
            Value::Uint(42),
            Value::Int(42),
            Value::Bytes(vec![42]),
            Value::String("42".to_string()),
        ];

        // Test that no cross-type comparisons return true
        for (i, left) in values.iter().enumerate() {
            for (j, right) in values.iter().enumerate() {
                if i != j {
                    assert!(
                        !apply_equal(left, right),
                        "Cross-type comparison should be false: {left:?} vs {right:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_apply_equal_reflexivity() {
        let values = vec![
            Value::Uint(42),
            Value::Int(-42),
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            Value::String("hello".to_string()),
        ];

        // Test that all values are equal to themselves
        for value in values {
            assert!(
                apply_equal(&value, &value),
                "Value should be equal to itself: {value:?}"
            );
        }
    }

    #[test]
    fn test_apply_equal_symmetry() {
        let test_cases = vec![
            (Value::Uint(42), Value::Uint(42)),
            (Value::Int(-100), Value::Int(-100)),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![1, 2, 3])),
            (
                Value::String("test".to_string()),
                Value::String("test".to_string()),
            ),
        ];

        // Test that equality is symmetric: a == b implies b == a
        for (left, right) in test_cases {
            let left_to_right = apply_equal(&left, &right);
            let right_to_left = apply_equal(&right, &left);
            assert_eq!(
                left_to_right, right_to_left,
                "Equality should be symmetric: {left:?} vs {right:?}"
            );
        }
    }

    #[test]
    fn test_apply_equal_transitivity() {
        // Test transitivity: if a == b and b == c, then a == c
        let a = Value::Uint(123);
        let b = Value::Uint(123);
        let c = Value::Uint(123);

        assert!(apply_equal(&a, &b));
        assert!(apply_equal(&b, &c));
        assert!(apply_equal(&a, &c));
    }

    #[test]
    fn test_apply_equal_edge_cases() {
        // Test with maximum values
        let max_unsigned = Value::Uint(u64::MAX);
        let max_signed = Value::Int(i64::MAX);
        let min_int = Value::Int(i64::MIN);

        assert!(apply_equal(&max_unsigned, &max_unsigned));
        assert!(apply_equal(&max_signed, &max_signed));
        assert!(apply_equal(&min_int, &min_int));

        // Test with empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let empty_string = Value::String(String::new());

        assert!(apply_equal(&empty_bytes, &empty_bytes));
        assert!(apply_equal(&empty_string, &empty_string));
        assert!(!apply_equal(&empty_bytes, &empty_string));
    }

    // Tests for apply_not_equal function
    #[test]
    fn test_apply_not_equal_uint_same_value() {
        let left = Value::Uint(42);
        let right = Value::Uint(42);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_uint_different_value() {
        let left = Value::Uint(42);
        let right = Value::Uint(24);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_uint_zero() {
        let left = Value::Uint(0);
        let right = Value::Uint(0);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_uint_max_value() {
        let left = Value::Uint(u64::MAX);
        let right = Value::Uint(u64::MAX);
        assert!(!apply_not_equal(&left, &right));

        let left = Value::Uint(u64::MAX);
        let right = Value::Uint(0);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_same_value() {
        let left = Value::Int(42);
        let right = Value::Int(42);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_different_value() {
        let left = Value::Int(42);
        let right = Value::Int(-42);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_negative() {
        let left = Value::Int(-100);
        let right = Value::Int(-100);
        assert!(!apply_not_equal(&left, &right));

        let left = Value::Int(-100);
        let right = Value::Int(100);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_zero() {
        let left = Value::Int(0);
        let right = Value::Int(0);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_extreme_values() {
        let left = Value::Int(i64::MAX);
        let right = Value::Int(i64::MAX);
        assert!(!apply_not_equal(&left, &right));

        let left = Value::Int(i64::MIN);
        let right = Value::Int(i64::MIN);
        assert!(!apply_not_equal(&left, &right));

        let left = Value::Int(i64::MAX);
        let right = Value::Int(i64::MIN);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_same_value() {
        let left = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        let right = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_different_value() {
        let left = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        let right = Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_empty() {
        let left = Value::Bytes(vec![]);
        let right = Value::Bytes(vec![]);
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_different_length() {
        let left = Value::Bytes(vec![0x7f, 0x45]);
        let right = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_single_byte() {
        let left = Value::Bytes(vec![0x7f]);
        let right = Value::Bytes(vec![0x7f]);
        assert!(!apply_not_equal(&left, &right));

        let left = Value::Bytes(vec![0x7f]);
        let right = Value::Bytes(vec![0x45]);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_same_value() {
        let left = Value::String("hello".to_string());
        let right = Value::String("hello".to_string());
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_different_value() {
        let left = Value::String("hello".to_string());
        let right = Value::String("world".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_empty() {
        let left = Value::String(String::new());
        let right = Value::String(String::new());
        assert!(!apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_case_sensitive() {
        let left = Value::String("Hello".to_string());
        let right = Value::String("hello".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_unicode() {
        let left = Value::String("🦀 Rust".to_string());
        let right = Value::String("🦀 Rust".to_string());
        assert!(!apply_not_equal(&left, &right));

        let left = Value::String("🦀 Rust".to_string());
        let right = Value::String("🐍 Python".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_string_whitespace() {
        let left = Value::String("hello world".to_string());
        let right = Value::String("hello world".to_string());
        assert!(!apply_not_equal(&left, &right));

        let left = Value::String("hello world".to_string());
        let right = Value::String("hello  world".to_string()); // Extra space
        assert!(apply_not_equal(&left, &right));
    }

    // Cross-type comparison tests for not_equal (should all return true)
    #[test]
    fn test_apply_not_equal_uint_vs_int() {
        let left = Value::Uint(42);
        let right = Value::Int(42);
        assert!(apply_not_equal(&left, &right));

        let left = Value::Uint(0);
        let right = Value::Int(0);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_uint_vs_bytes() {
        let left = Value::Uint(42);
        let right = Value::Bytes(vec![42]);
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_uint_vs_string() {
        let left = Value::Uint(42);
        let right = Value::String("42".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_vs_bytes() {
        let left = Value::Int(-42);
        let right = Value::Bytes(vec![214]); // -42 as u8
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_int_vs_string() {
        let left = Value::Int(-42);
        let right = Value::String("-42".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_bytes_vs_string() {
        let left = Value::Bytes(vec![104, 101, 108, 108, 111]); // "hello" as bytes
        let right = Value::String("hello".to_string());
        assert!(apply_not_equal(&left, &right));
    }

    #[test]
    fn test_apply_not_equal_all_cross_type_combinations() {
        let values = [
            Value::Uint(42),
            Value::Int(42),
            Value::Bytes(vec![42]),
            Value::String("42".to_string()),
        ];

        // Test that all cross-type comparisons return true for not_equal
        for (i, left) in values.iter().enumerate() {
            for (j, right) in values.iter().enumerate() {
                if i != j {
                    assert!(
                        apply_not_equal(left, right),
                        "Cross-type comparison should be true for not_equal: {left:?} vs {right:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_apply_not_equal_consistency_with_equal() {
        let test_cases = vec![
            (Value::Uint(42), Value::Uint(42)),
            (Value::Uint(42), Value::Uint(24)),
            (Value::Int(-100), Value::Int(-100)),
            (Value::Int(-100), Value::Int(100)),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![1, 2, 3])),
            (Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![3, 2, 1])),
            (
                Value::String("test".to_string()),
                Value::String("test".to_string()),
            ),
            (
                Value::String("test".to_string()),
                Value::String("different".to_string()),
            ),
            // Cross-type cases
            (Value::Uint(42), Value::Int(42)),
            (Value::Uint(42), Value::String("42".to_string())),
            (Value::Bytes(vec![42]), Value::Uint(42)),
        ];

        // Test that apply_not_equal is always the negation of apply_equal
        for (left, right) in test_cases {
            let equal_result = apply_equal(&left, &right);
            let not_equal_result = apply_not_equal(&left, &right);
            assert_eq!(
                equal_result, !not_equal_result,
                "apply_not_equal should be negation of apply_equal: {left:?} vs {right:?}"
            );
        }
    }

    #[test]
    fn test_apply_not_equal_edge_cases() {
        // Test with maximum values
        let max_unsigned = Value::Uint(u64::MAX);
        let max_signed = Value::Int(i64::MAX);
        let min_int = Value::Int(i64::MIN);

        assert!(!apply_not_equal(&max_unsigned, &max_unsigned));
        assert!(!apply_not_equal(&max_signed, &max_signed));
        assert!(!apply_not_equal(&min_int, &min_int));

        // Test with empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let empty_string = Value::String(String::new());

        assert!(!apply_not_equal(&empty_bytes, &empty_bytes));
        assert!(!apply_not_equal(&empty_string, &empty_string));
        assert!(apply_not_equal(&empty_bytes, &empty_string));
    }

    #[test]
    fn test_apply_not_equal_various_value_combinations() {
        // Test various combinations to ensure comprehensive coverage
        let test_cases = vec![
            // Uint variations
            (Value::Uint(0), Value::Uint(1), true),
            (Value::Uint(100), Value::Uint(100), false),
            (Value::Uint(u64::MAX), Value::Uint(u64::MAX - 1), true),
            // Int variations
            (Value::Int(0), Value::Int(-1), true),
            (Value::Int(-50), Value::Int(-50), false),
            (Value::Int(i64::MIN), Value::Int(i64::MAX), true),
            // Bytes variations
            (Value::Bytes(vec![0]), Value::Bytes(vec![1]), true),
            (
                Value::Bytes(vec![255, 254]),
                Value::Bytes(vec![255, 254]),
                false,
            ),
            (Value::Bytes(vec![]), Value::Bytes(vec![0]), true),
            // String variations
            (
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                true,
            ),
            (
                Value::String("same".to_string()),
                Value::String("same".to_string()),
                false,
            ),
            (
                Value::String(String::new()),
                Value::String("non-empty".to_string()),
                true,
            ),
        ];

        for (left, right, expected) in test_cases {
            assert_eq!(
                apply_not_equal(&left, &right),
                expected,
                "apply_not_equal({left:?}, {right:?}) should be {expected}"
            );
        }
    }

    // Tests for apply_bitwise_and function
    #[test]
    fn test_apply_bitwise_and_uint_basic() {
        // Basic bit checking
        assert!(apply_bitwise_and(&Value::Uint(0x01), &Value::Uint(0x01))); // Bit 0 set
        assert!(!apply_bitwise_and(&Value::Uint(0x02), &Value::Uint(0x01))); // Bit 0 not set
        assert!(apply_bitwise_and(&Value::Uint(0x03), &Value::Uint(0x01))); // Bit 0 set among others
    }

    #[test]
    fn test_apply_bitwise_and_uint_multiple_bits() {
        // Multiple bit patterns
        assert!(apply_bitwise_and(&Value::Uint(0xFF), &Value::Uint(0x0F))); // Any of lower 4 bits
        assert!(!apply_bitwise_and(&Value::Uint(0xF0), &Value::Uint(0x0F))); // None of lower 4 bits
        assert!(!apply_bitwise_and(&Value::Uint(0xAA), &Value::Uint(0x55))); // No overlap (0xAA = 10101010, 0x55 = 01010101)
        assert!(apply_bitwise_and(&Value::Uint(0xAA), &Value::Uint(0xAA))); // Same pattern
    }

    #[test]
    fn test_apply_bitwise_and_uint_edge_cases() {
        // Zero cases
        assert!(!apply_bitwise_and(&Value::Uint(0), &Value::Uint(0xFF))); // Zero & anything = 0
        assert!(!apply_bitwise_and(&Value::Uint(0xFF), &Value::Uint(0))); // Anything & zero = 0
        assert!(!apply_bitwise_and(&Value::Uint(0), &Value::Uint(0))); // Zero & zero = 0

        // Maximum values
        assert!(apply_bitwise_and(&Value::Uint(u64::MAX), &Value::Uint(1))); // Max & 1
        assert!(apply_bitwise_and(
            &Value::Uint(u64::MAX),
            &Value::Uint(u64::MAX)
        )); // Max & Max
    }

    #[test]
    fn test_apply_bitwise_and_uint_specific_patterns() {
        // Common magic number patterns
        assert!(apply_bitwise_and(
            &Value::Uint(0x7F45_4C46),
            &Value::Uint(0xFF00_0000)
        )); // ELF magic high byte
        assert!(apply_bitwise_and(
            &Value::Uint(0x504B_0304),
            &Value::Uint(0xFFFF_0000)
        )); // ZIP magic high word
        assert!(!apply_bitwise_and(
            &Value::Uint(0x1234_5678),
            &Value::Uint(0x0000_0001)
        )); // Bit 0 not set
    }

    #[test]
    fn test_apply_bitwise_and_int_basic() {
        // Basic signed integer bitwise AND
        assert!(apply_bitwise_and(&Value::Int(1), &Value::Int(1))); // Positive & positive
        assert!(!apply_bitwise_and(&Value::Int(2), &Value::Int(1))); // Different bits
        assert!(apply_bitwise_and(&Value::Int(3), &Value::Int(1))); // Multiple bits, one matches
    }

    #[test]
    fn test_apply_bitwise_and_int_negative() {
        // Negative number bitwise AND (uses two's complement)
        assert!(apply_bitwise_and(&Value::Int(-1), &Value::Int(1))); // -1 has all bits set
        assert!(apply_bitwise_and(&Value::Int(-2), &Value::Int(2))); // -2 & 2 should have bit 1 set
        assert!(!apply_bitwise_and(&Value::Int(-2), &Value::Int(1))); // -2 & 1 should be 0 (bit 0 not set in -2)
    }

    #[test]
    fn test_apply_bitwise_and_int_zero() {
        // Zero cases with signed integers
        assert!(!apply_bitwise_and(&Value::Int(0), &Value::Int(0xFF))); // Zero & anything = 0
        assert!(!apply_bitwise_and(&Value::Int(0xFF), &Value::Int(0))); // Anything & zero = 0
        assert!(!apply_bitwise_and(&Value::Int(0), &Value::Int(0))); // Zero & zero = 0
    }

    #[test]
    fn test_apply_bitwise_and_int_extreme_values() {
        // Extreme signed integer values
        assert!(apply_bitwise_and(&Value::Int(i64::MAX), &Value::Int(1))); // Max positive & 1
        assert!(apply_bitwise_and(
            &Value::Int(i64::MIN),
            &Value::Int(i64::MIN)
        )); // Min & Min
        assert!(apply_bitwise_and(&Value::Int(i64::MIN), &Value::Int(-1))); // Min & -1 (all bits set)
    }

    #[test]
    fn test_apply_bitwise_and_mixed_int_uint() {
        // Mixed signed/unsigned operations
        assert!(apply_bitwise_and(&Value::Uint(0xFF), &Value::Int(0x0F))); // Uint & Int
        assert!(apply_bitwise_and(&Value::Int(0xFF), &Value::Uint(0x0F))); // Int & Uint
        assert!(!apply_bitwise_and(&Value::Uint(0xF0), &Value::Int(0x0F))); // No overlap
        assert!(!apply_bitwise_and(&Value::Int(0xF0), &Value::Uint(0x0F))); // No overlap
    }

    #[test]
    fn test_apply_bitwise_and_mixed_negative_uint() {
        // Negative int with uint (negative numbers have high bits set)
        assert!(apply_bitwise_and(&Value::Int(-1), &Value::Uint(1))); // -1 & 1
        assert!(apply_bitwise_and(&Value::Uint(1), &Value::Int(-1))); // 1 & -1
        assert!(!apply_bitwise_and(&Value::Int(-2), &Value::Uint(1))); // -2 & 1 (bit 0 not set in -2)
        assert!(!apply_bitwise_and(&Value::Uint(1), &Value::Int(-2))); // 1 & -2
    }

    #[test]
    fn test_apply_bitwise_and_non_integer_types() {
        // Non-integer types should return false
        assert!(!apply_bitwise_and(
            &Value::String("test".to_string()),
            &Value::Uint(0x01)
        ));
        assert!(!apply_bitwise_and(
            &Value::Uint(0x01),
            &Value::String("test".to_string())
        ));
        assert!(!apply_bitwise_and(
            &Value::Bytes(vec![1]),
            &Value::Uint(0x01)
        ));
        assert!(!apply_bitwise_and(
            &Value::Uint(0x01),
            &Value::Bytes(vec![1])
        ));
        assert!(!apply_bitwise_and(
            &Value::String("a".to_string()),
            &Value::String("b".to_string())
        ));
        assert!(!apply_bitwise_and(
            &Value::Bytes(vec![1]),
            &Value::Bytes(vec![1])
        ));
    }

    #[test]
    fn test_apply_bitwise_and_all_non_integer_combinations() {
        let non_integer_values = [Value::String("test".to_string()), Value::Bytes(vec![42])];

        let integer_values = [Value::Uint(42), Value::Int(42)];

        // Test all combinations of non-integer with integer
        for non_int in &non_integer_values {
            for int_val in &integer_values {
                assert!(
                    !apply_bitwise_and(non_int, int_val),
                    "Non-integer & integer should be false: {non_int:?} & {int_val:?}"
                );
                assert!(
                    !apply_bitwise_and(int_val, non_int),
                    "Integer & non-integer should be false: {int_val:?} & {non_int:?}"
                );
            }
        }

        // Test all combinations of non-integer with non-integer
        for left in &non_integer_values {
            for right in &non_integer_values {
                assert!(
                    !apply_bitwise_and(left, right),
                    "Non-integer & non-integer should be false: {left:?} & {right:?}"
                );
            }
        }
    }

    #[test]
    fn test_apply_bitwise_and_bit_patterns() {
        // Test specific bit patterns commonly used in magic rules
        let test_cases = vec![
            // (value, mask, expected)
            (0b0000_0001_u64, 0b0000_0001_u64, true), // Bit 0 set
            (0b0000_0010_u64, 0b0000_0001_u64, false), // Bit 0 not set
            (0b0000_0011_u64, 0b0000_0001_u64, true), // Bit 0 set among others
            (0b1111_1111_u64, 0b0000_1111_u64, true), // Any of lower 4 bits
            (0b1111_0000_u64, 0b0000_1111_u64, false), // None of lower 4 bits
            (0b1010_1010_u64, 0b0101_0101_u64, false), // No overlap
            (0b1010_1010_u64, 0b1010_1010_u64, true), // Perfect match
            (0b1111_1111_u64, 0b0000_0000_u64, false), // Mask is zero
            (0b0000_0000_u64, 0b1111_1111_u64, false), // Value is zero
        ];

        for (value, mask, expected) in test_cases {
            assert_eq!(
                apply_bitwise_and(&Value::Uint(value), &Value::Uint(mask)),
                expected,
                "apply_bitwise_and(0b{value:08b}, 0b{mask:08b}) should be {expected}"
            );
        }
    }

    #[test]
    fn test_apply_bitwise_and_magic_file_patterns() {
        // Test patterns commonly found in magic files

        // ELF magic number (0x7F454C46) - check if it's an ELF file
        let elf_magic = Value::Uint(0x7F45_4C46);
        let elf_mask = Value::Uint(0xFFFF_FFFF);
        assert!(apply_bitwise_and(&elf_magic, &elf_mask));

        // Check specific bytes in ELF magic
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x7F00_0000))); // First byte
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0045_0000))); // Second byte 'E'
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0000_4C00))); // Third byte 'L'
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0000_0046))); // Fourth byte 'F'

        // ZIP magic number (0x504B0304) - check if it's a ZIP file
        let zip_magic = Value::Uint(0x504B_0304);
        assert!(apply_bitwise_and(&zip_magic, &Value::Uint(0x504B_0000))); // PK signature
        assert!(!apply_bitwise_and(&zip_magic, &Value::Uint(0x0000_0001))); // Bit 0 not set

        // PDF magic (%PDF) - first few bytes
        let pdf_magic = Value::Uint(0x2550_4446); // "%PDF" as uint32
        assert!(apply_bitwise_and(&pdf_magic, &Value::Uint(0xFF00_0000))); // '%' character
        assert!(apply_bitwise_and(&pdf_magic, &Value::Uint(0x00FF_0000))); // 'P' character
    }

    #[test]
    fn test_apply_bitwise_and_symmetry() {
        // Test that bitwise AND is commutative for integer types
        let test_cases = vec![
            (Value::Uint(0xFF), Value::Uint(0x0F)),
            (Value::Int(42), Value::Int(24)),
            (Value::Uint(0xAAAA), Value::Int(0x5555)),
            (Value::Int(-1), Value::Uint(1)),
        ];

        for (left, right) in test_cases {
            let left_to_right = apply_bitwise_and(&left, &right);
            let right_to_left = apply_bitwise_and(&right, &left);
            assert_eq!(
                left_to_right, right_to_left,
                "Bitwise AND should be commutative: {left:?} & {right:?}"
            );
        }
    }

    #[test]
    fn test_apply_bitwise_and_associativity_concept() {
        // While we can't test true associativity with binary function,
        // we can test that the operation behaves consistently
        let value = Value::Uint(0b1111_0000);
        let mask1 = Value::Uint(0b1100_0000);
        let mask2 = Value::Uint(0b0011_0000);
        let combined_mask = Value::Uint(0b1111_0000);

        // (value & mask1) should be true if any bits match
        assert!(apply_bitwise_and(&value, &mask1));
        assert!(apply_bitwise_and(&value, &mask2));
        assert!(apply_bitwise_and(&value, &combined_mask));
    }

    // Tests for apply_operator function
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

        // Cross-type should be false
        assert!(!apply_operator(
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

        // Cross-type should be true (not equal)
        assert!(apply_operator(
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
            // Different types
            (Value::Uint(42), Value::Int(42)),
            (Value::Uint(42), Value::String("42".to_string())),
            (Value::Int(42), Value::Bytes(vec![42])),
        ];

        for (left, right) in test_cases {
            // Equal should always be false for different values
            assert!(
                !apply_operator(&Operator::Equal, &left, &right),
                "Equal should be false for different values: {left:?} == {right:?}"
            );

            // NotEqual should always be true for different values
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
        assert!(apply_operator(
            &Operator::NotEqual,
            &zero_uint,
            &zero_signed
        )); // Different types
    }

    #[test]
    fn test_apply_operator_all_combinations() {
        let operators = [Operator::Equal, Operator::NotEqual, Operator::BitwiseAnd];
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
                        Operator::BitwiseAnd => apply_bitwise_and(left, right),
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
