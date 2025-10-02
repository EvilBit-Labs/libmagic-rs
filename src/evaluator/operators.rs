//! Operator application for magic rule evaluation
//!
//! This module provides functions for applying comparison and bitwise operators
//! to values during magic rule evaluation. It handles type-safe comparisons
//! between different Value variants.

use crate::parser::ast::Value;

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
}
