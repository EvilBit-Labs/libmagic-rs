// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Equality and inequality operators for magic rule evaluation

use std::cmp::Ordering;

use crate::parser::ast::Value;

use super::compare_values;

/// Machine-epsilon threshold used when comparing `Value::Float` operands for
/// equality.  Two floats are considered equal when `|a - b| <= FLOAT_EPSILON`.
/// Special values (NaN, infinity) are handled explicitly before the epsilon
/// check.
const FLOAT_EPSILON: f64 = f64::EPSILON;

/// Return `true` when two `f64` values are considered equal under
/// epsilon-aware semantics.
///
/// * **NaN**: NaN is never equal to anything (including itself).
/// * **Infinity**: positive/negative infinity are only equal to the same sign
///   of infinity.
/// * **Finite**: `|a - b| <= FLOAT_EPSILON`.
fn floats_equal(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    // Infinities: equal only when same sign (inf - inf = NaN, so must check first).
    // Exact comparison is correct here -- infinities have precise IEEE 754 bit patterns.
    if a.is_infinite() || b.is_infinite() {
        #[allow(clippy::float_cmp)]
        return a == b;
    }
    (a - b).abs() <= FLOAT_EPSILON
}

/// Apply equality comparison between two values
///
/// Compares two `Value` instances for equality, handling proper type matching.
/// Cross-type integer comparisons (`Uint` vs `Int`) are supported via `i128`
/// coercion.  Float comparisons use epsilon-aware equality
/// (`|a - b| <= f64::EPSILON`).  Incompatible types (e.g., string vs integer)
/// are considered unequal.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the values are equal (including cross-type integer coercion and
/// epsilon-aware float comparison), `false` otherwise
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
/// // Cross-type integer coercion
/// assert!(apply_equal(&Value::Uint(42), &Value::Int(42)));
///
/// // String comparison
/// assert!(apply_equal(
///     &Value::String("hello".to_string()),
///     &Value::String("hello".to_string())
/// ));
///
/// // Float epsilon-aware equality
/// assert!(apply_equal(&Value::Float(1.0), &Value::Float(1.0)));
/// ```
#[must_use]
pub fn apply_equal(left: &Value, right: &Value) -> bool {
    if let (Value::Float(a), Value::Float(b)) = (left, right) {
        return floats_equal(*a, *b);
    }
    // String/Bytes cross-type equality: when the parser ingests a magic
    // value like `\177ELF` it produces `Value::Bytes([0x7f, 'E', 'L',
    // 'F'])`, but `read_string_exact` returns `Value::String("\x7fELF")`.
    // The two represent the same byte sequence and must compare equal so
    // that real-world rules with backslash-escaped values match. Compare
    // by underlying byte sequence in both directions.
    match (left, right) {
        (Value::String(s), Value::Bytes(b)) | (Value::Bytes(b), Value::String(s)) => {
            return s.as_bytes() == b.as_slice();
        }
        _ => {}
    }
    compare_values(left, right) == Some(Ordering::Equal)
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
/// // Cross-type integers with same numeric value (equal via coercion)
/// assert!(!apply_not_equal(&Value::Uint(42), &Value::Int(42)));
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
        let left = Value::String("\u{1f980} Rust".to_string());
        let right = Value::String("\u{1f980} Rust".to_string());
        assert!(apply_equal(&left, &right));

        let left = Value::String("\u{1f980} Rust".to_string());
        let right = Value::String("\u{1f40d} Python".to_string());
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
        // Same numeric value across types should match
        let left = Value::Uint(42);
        let right = Value::Int(42);
        assert!(apply_equal(&left, &right));

        let left = Value::Uint(0);
        let right = Value::Int(0);
        assert!(apply_equal(&left, &right));

        // Negative Int cannot equal Uint
        let left = Value::Uint(42);
        let right = Value::Int(-42);
        assert!(!apply_equal(&left, &right));

        // Large Uint that doesn't fit in i64 cannot equal Int
        let left = Value::Uint(u64::MAX);
        let right = Value::Int(-1);
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
        // libmagic-compatible policy (since the `\177ELF`-style escape fix):
        // `Value::Bytes` and `Value::String` compare equal when their
        // underlying byte sequences match. The parser produces
        // `Value::Bytes` for backslash-escape patterns like `\177ELF`,
        // while `read_string_exact` returns `Value::String` -- the
        // comparison must succeed for the rule to match.
        let left = Value::Bytes(vec![104, 101, 108, 108, 111]); // "hello" as bytes
        let right = Value::String("hello".to_string());
        assert!(apply_equal(&left, &right));
        // Different byte sequences still don't compare equal.
        let other = Value::String("world".to_string());
        assert!(!apply_equal(&left, &other));
    }

    #[test]
    fn test_apply_equal_all_cross_type_combinations() {
        let values = [
            Value::Uint(42),
            Value::Int(42),
            Value::Bytes(vec![42]),
            Value::String("42".to_string()),
        ];

        // Test cross-type comparisons
        for (i, left) in values.iter().enumerate() {
            for (j, right) in values.iter().enumerate() {
                if i != j {
                    let result = apply_equal(left, right);
                    // Uint(42) and Int(42) should be equal (cross-type coercion)
                    if (i <= 1) && (j <= 1) {
                        assert!(
                            result,
                            "Integer cross-type comparison should be true: {left:?} vs {right:?}"
                        );
                    } else {
                        assert!(
                            !result,
                            "Non-integer cross-type comparison should be false: {left:?} vs {right:?}"
                        );
                    }
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

        // Cross-type edge cases
        // u64::MAX != -1 in i64 (different mathematical values)
        assert!(!apply_equal(&max_unsigned, &Value::Int(-1)));
        // i64::MAX can be represented as u64, so should match
        assert!(apply_equal(&Value::Uint(i64::MAX as u64), &max_signed));

        // Test with empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let empty_string = Value::String(String::new());

        assert!(apply_equal(&empty_bytes, &empty_bytes));
        assert!(apply_equal(&empty_string, &empty_string));
        // Cross-type empty Bytes vs empty String: their underlying byte
        // sequences are both zero-length, so the libmagic-compatible
        // policy says they compare equal (see test_apply_equal_bytes_vs_string).
        assert!(apply_equal(&empty_bytes, &empty_string));
    }

    // Tests for apply_not_equal function
    //
    // Per project guidelines, we do not maintain 1:1 mirrors of the apply_equal
    // tests above. Instead, the single consistency test below proves the
    // contract `apply_not_equal == !apply_equal` over a representative table of
    // value pairs covering all variants and edge cases that the per-case
    // not_equal tests previously exercised. Float-specific cases are covered
    // separately because of the epsilon/NaN/infinity semantics.

    #[test]
    fn test_apply_not_equal_consistency_with_equal() {
        let test_cases = vec![
            // Uint variants: same/different/zero/max/cross-max
            (Value::Uint(42), Value::Uint(42)),
            (Value::Uint(42), Value::Uint(24)),
            (Value::Uint(0), Value::Uint(0)),
            (Value::Uint(u64::MAX), Value::Uint(u64::MAX)),
            (Value::Uint(u64::MAX), Value::Uint(0)),
            // Int variants: same/different/negative/zero/extremes
            (Value::Int(42), Value::Int(42)),
            (Value::Int(42), Value::Int(-42)),
            (Value::Int(-100), Value::Int(-100)),
            (Value::Int(-100), Value::Int(100)),
            (Value::Int(0), Value::Int(0)),
            (Value::Int(i64::MAX), Value::Int(i64::MAX)),
            (Value::Int(i64::MIN), Value::Int(i64::MIN)),
            (Value::Int(i64::MAX), Value::Int(i64::MIN)),
            // Bytes variants: same/different/empty/different-length/single
            (
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            ),
            (
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
                Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]),
            ),
            (Value::Bytes(vec![]), Value::Bytes(vec![])),
            (
                Value::Bytes(vec![0x7f, 0x45]),
                Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            ),
            (Value::Bytes(vec![0x7f]), Value::Bytes(vec![0x7f])),
            (Value::Bytes(vec![0x7f]), Value::Bytes(vec![0x45])),
            // String variants: same/different/empty/case/unicode/whitespace
            (
                Value::String("hello".to_string()),
                Value::String("hello".to_string()),
            ),
            (
                Value::String("hello".to_string()),
                Value::String("world".to_string()),
            ),
            (Value::String(String::new()), Value::String(String::new())),
            (
                Value::String("Hello".to_string()),
                Value::String("hello".to_string()),
            ),
            (
                Value::String("\u{1f980} Rust".to_string()),
                Value::String("\u{1f980} Rust".to_string()),
            ),
            (
                Value::String("\u{1f980} Rust".to_string()),
                Value::String("\u{1f40d} Python".to_string()),
            ),
            (
                Value::String("hello world".to_string()),
                Value::String("hello world".to_string()),
            ),
            (
                Value::String("hello world".to_string()),
                Value::String("hello  world".to_string()),
            ),
            // Cross-type cases (Uint vs Int with coercion, plus incompatible)
            (Value::Uint(42), Value::Int(42)),
            (Value::Uint(0), Value::Int(0)),
            (Value::Uint(42), Value::Int(-42)),
            (Value::Uint(42), Value::Bytes(vec![42])),
            (Value::Uint(42), Value::String("42".to_string())),
            (Value::Int(-42), Value::Bytes(vec![214])),
            (Value::Int(-42), Value::String("-42".to_string())),
            (
                Value::Bytes(vec![104, 101, 108, 108, 111]),
                Value::String("hello".to_string()),
            ),
            (Value::Bytes(vec![42]), Value::Uint(42)),
            // Edge: empty bytes vs empty string (cross-type, unequal)
            (Value::Bytes(vec![]), Value::String(String::new())),
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

    // ============================================================
    // Float epsilon-aware equality / inequality tests
    // ============================================================

    #[test]
    fn test_apply_equal_float_exact_same_value() {
        assert!(apply_equal(&Value::Float(1.0), &Value::Float(1.0)));
        assert!(apply_equal(&Value::Float(0.0), &Value::Float(0.0)));
        assert!(apply_equal(&Value::Float(-3.125), &Value::Float(-3.125)));
    }

    #[test]
    fn test_apply_equal_float_near_equal_within_epsilon() {
        // Values that differ by exactly f64::EPSILON should be considered equal
        let a = 1.0_f64;
        let b = a + f64::EPSILON;
        assert!(
            apply_equal(&Value::Float(a), &Value::Float(b)),
            "values differing by f64::EPSILON should be equal"
        );
    }

    #[test]
    fn test_apply_equal_float_clearly_unequal() {
        assert!(!apply_equal(&Value::Float(1.0), &Value::Float(2.0)));
        assert!(!apply_equal(&Value::Float(0.0), &Value::Float(1.0)));
        assert!(!apply_equal(&Value::Float(-1.0), &Value::Float(1.0)));
    }

    #[test]
    fn test_apply_equal_float_infinity() {
        let pos_inf = f64::INFINITY;
        let neg_inf = f64::NEG_INFINITY;

        assert!(apply_equal(&Value::Float(pos_inf), &Value::Float(pos_inf)));
        assert!(apply_equal(&Value::Float(neg_inf), &Value::Float(neg_inf)));
        assert!(!apply_equal(&Value::Float(pos_inf), &Value::Float(neg_inf)));
        assert!(!apply_equal(&Value::Float(pos_inf), &Value::Float(1.0)));
    }

    #[test]
    fn test_apply_equal_float_nan() {
        let nan = f64::NAN;
        assert!(!apply_equal(&Value::Float(nan), &Value::Float(nan)));
        assert!(!apply_equal(&Value::Float(nan), &Value::Float(0.0)));
        assert!(!apply_equal(&Value::Float(0.0), &Value::Float(nan)));
    }

    #[test]
    fn test_apply_not_equal_float_consistency_with_equal() {
        // apply_not_equal must be the negation of apply_equal for all float
        // edge cases: exact equality, epsilon-near, clearly unequal, NaN
        // (NaN != NaN), and infinities (same sign equal, opposite sign unequal).
        let nan = f64::NAN;
        let near_a = 1.0_f64;
        let near_b = near_a + f64::EPSILON;
        let test_cases = vec![
            (Value::Float(1.0), Value::Float(1.0)),
            (Value::Float(0.0), Value::Float(0.0)),
            (Value::Float(near_a), Value::Float(near_b)),
            (Value::Float(1.0), Value::Float(2.0)),
            (Value::Float(-1.0), Value::Float(1.0)),
            (Value::Float(nan), Value::Float(nan)),
            (Value::Float(nan), Value::Float(0.0)),
            (Value::Float(0.0), Value::Float(nan)),
            (Value::Float(f64::INFINITY), Value::Float(f64::INFINITY)),
            (
                Value::Float(f64::NEG_INFINITY),
                Value::Float(f64::NEG_INFINITY),
            ),
            (Value::Float(f64::INFINITY), Value::Float(f64::NEG_INFINITY)),
            (Value::Float(f64::INFINITY), Value::Float(1.0)),
        ];
        for (left, right) in test_cases {
            let equal_result = apply_equal(&left, &right);
            let not_equal_result = apply_not_equal(&left, &right);
            assert_eq!(
                equal_result, !not_equal_result,
                "apply_not_equal should be negation of apply_equal: {left:?} vs {right:?}"
            );
        }
    }
}
