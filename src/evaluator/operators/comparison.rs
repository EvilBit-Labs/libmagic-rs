// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Comparison operators for magic rule evaluation

use std::cmp::Ordering;

use crate::parser::ast::Value;

/// Compare two values and return their ordering, if comparable
///
/// Returns `Some(Ordering)` for same-type comparisons (integers, strings, bytes)
/// and cross-type integer comparisons (via `i128` coercion). Returns `None` for
/// incomparable type combinations.
///
/// # Examples
///
/// ```
/// use std::cmp::Ordering;
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::compare_values;
///
/// assert_eq!(compare_values(&Value::Uint(5), &Value::Uint(10)), Some(Ordering::Less));
/// assert_eq!(compare_values(&Value::Int(-1), &Value::Uint(0)), Some(Ordering::Less));
/// assert_eq!(compare_values(&Value::Uint(42), &Value::Int(42)), Some(Ordering::Equal));
/// assert_eq!(compare_values(&Value::Uint(1), &Value::String("1".to_string())), None);
/// ```
#[must_use]
pub fn compare_values(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Uint(a), Value::Uint(b)) => Some(a.cmp(b)),
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Uint(a), Value::Int(b)) => Some(i128::from(*a).cmp(&i128::from(*b))),
        (Value::Int(a), Value::Uint(b)) => Some(i128::from(*a).cmp(&i128::from(*b))),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bytes(a), Value::Bytes(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Apply less-than comparison between two values
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_less_than;
///
/// assert!(apply_less_than(&Value::Uint(5), &Value::Uint(10)));
/// assert!(!apply_less_than(&Value::Uint(10), &Value::Uint(10)));
/// assert!(apply_less_than(&Value::Int(-1), &Value::Uint(0)));
/// ```
#[must_use]
pub fn apply_less_than(left: &Value, right: &Value) -> bool {
    compare_values(left, right) == Some(Ordering::Less)
}

/// Apply greater-than comparison between two values
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_greater_than;
///
/// assert!(apply_greater_than(&Value::Uint(10), &Value::Uint(5)));
/// assert!(!apply_greater_than(&Value::Uint(10), &Value::Uint(10)));
/// assert!(apply_greater_than(&Value::Uint(0), &Value::Int(-1)));
/// ```
#[must_use]
pub fn apply_greater_than(left: &Value, right: &Value) -> bool {
    compare_values(left, right) == Some(Ordering::Greater)
}

/// Apply less-than-or-equal comparison between two values
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_less_equal;
///
/// assert!(apply_less_equal(&Value::Uint(10), &Value::Uint(10)));
/// assert!(apply_less_equal(&Value::Uint(5), &Value::Uint(10)));
/// assert!(!apply_less_equal(&Value::Uint(10), &Value::Uint(5)));
/// ```
#[must_use]
pub fn apply_less_equal(left: &Value, right: &Value) -> bool {
    matches!(
        compare_values(left, right),
        Some(Ordering::Less | Ordering::Equal)
    )
}

/// Apply greater-than-or-equal comparison between two values
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_greater_equal;
///
/// assert!(apply_greater_equal(&Value::Uint(10), &Value::Uint(10)));
/// assert!(apply_greater_equal(&Value::Uint(10), &Value::Uint(5)));
/// assert!(!apply_greater_equal(&Value::Uint(5), &Value::Uint(10)));
/// ```
#[must_use]
pub fn apply_greater_equal(left: &Value, right: &Value) -> bool {
    matches!(
        compare_values(left, right),
        Some(Ordering::Greater | Ordering::Equal)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_values_ordering() {
        use std::cmp::Ordering::*;

        // Same-type integer comparisons
        assert_eq!(
            compare_values(&Value::Uint(5), &Value::Uint(10)),
            Some(Less)
        );
        assert_eq!(
            compare_values(&Value::Uint(10), &Value::Uint(10)),
            Some(Equal)
        );
        assert_eq!(
            compare_values(&Value::Uint(10), &Value::Uint(5)),
            Some(Greater)
        );
        assert_eq!(
            compare_values(&Value::Int(-10), &Value::Int(-5)),
            Some(Less)
        );
        assert_eq!(
            compare_values(&Value::Int(i64::MIN), &Value::Int(0)),
            Some(Less)
        );

        // Cross-type integer comparisons via i128
        assert_eq!(compare_values(&Value::Int(-1), &Value::Uint(0)), Some(Less));
        assert_eq!(
            compare_values(&Value::Uint(42), &Value::Int(42)),
            Some(Equal)
        );
        assert_eq!(
            compare_values(&Value::Uint(u64::MAX), &Value::Int(-1)),
            Some(Greater)
        );

        // String comparisons
        assert_eq!(
            compare_values(&Value::String("abc".into()), &Value::String("abd".into())),
            Some(Less)
        );
        assert_eq!(
            compare_values(&Value::String("abc".into()), &Value::String("abc".into())),
            Some(Equal)
        );

        // Bytes comparisons (lexicographic, including different lengths)
        assert_eq!(
            compare_values(&Value::Bytes(vec![1]), &Value::Bytes(vec![2])),
            Some(Less)
        );
        assert_eq!(
            compare_values(&Value::Bytes(vec![1]), &Value::Bytes(vec![1])),
            Some(Equal)
        );
        assert_eq!(
            compare_values(&Value::Bytes(vec![1]), &Value::Bytes(vec![1, 2])),
            Some(Less)
        );
        assert_eq!(
            compare_values(&Value::Bytes(vec![]), &Value::Bytes(vec![1])),
            Some(Less)
        );

        // Incomparable types return None
        assert_eq!(
            compare_values(&Value::Uint(1), &Value::String("1".into())),
            None
        );
        assert_eq!(compare_values(&Value::Int(1), &Value::Bytes(vec![1])), None);
    }

    #[test]
    fn test_comparison_operators_consistency() {
        // Verify all four comparison functions agree with compare_values
        let pairs = vec![
            (Value::Uint(5), Value::Uint(10)),
            (Value::Uint(10), Value::Uint(10)),
            (Value::Uint(10), Value::Uint(5)),
            (Value::Int(-10), Value::Int(-5)),
            (Value::Int(-1), Value::Uint(0)),
            (Value::Uint(u64::MAX), Value::Int(-1)),
            (Value::String("abc".into()), Value::String("abd".into())),
            (Value::Bytes(vec![1, 2]), Value::Bytes(vec![1, 3])),
            (Value::Bytes(vec![1]), Value::Bytes(vec![1, 2])),
            (Value::Uint(1), Value::String("1".into())), // incomparable
        ];

        for (left, right) in &pairs {
            let ord = compare_values(left, right);
            assert_eq!(
                apply_less_than(left, right),
                ord == Some(Ordering::Less),
                "< for {left:?}, {right:?}"
            );
            assert_eq!(
                apply_greater_than(left, right),
                ord == Some(Ordering::Greater),
                "> for {left:?}, {right:?}"
            );
            assert_eq!(
                apply_less_equal(left, right),
                matches!(ord, Some(Ordering::Less | Ordering::Equal)),
                "<= for {left:?}, {right:?}"
            );
            assert_eq!(
                apply_greater_equal(left, right),
                matches!(ord, Some(Ordering::Greater | Ordering::Equal)),
                ">= for {left:?}, {right:?}"
            );
        }
    }
}
