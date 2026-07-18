// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Bitwise operators for magic rule evaluation

use crate::evaluator::operators::equality::apply_equal;
use crate::parser::ast::Value;

/// Apply bitwise AND with mask for masked comparison
///
/// Applies a bitmask to the left value, then checks equality with the right value.
/// This is used for `BitwiseAndMask(mask)` operator evaluation in magic rules.
/// Only works with integer types (Uint and Int), returns `false` for other types.
///
/// # Arguments
///
/// * `mask` - The bitmask to apply to the left value
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the masked left value equals the right value, `false` otherwise
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_bitwise_and_mask;
///
/// // Mask 0xFF applied to 0x1234 gives 0x34, compared with 0x34
/// assert!(apply_bitwise_and_mask(0xFF, &Value::Uint(0x1234), &Value::Uint(0x34)));
///
/// // Mask 0xFF applied to 0x1234 gives 0x34, not equal to 0x12
/// assert!(!apply_bitwise_and_mask(0xFF, &Value::Uint(0x1234), &Value::Uint(0x12)));
///
/// // Non-integer types return false
/// assert!(!apply_bitwise_and_mask(0xFF, &Value::String("test".to_string()), &Value::Uint(0x01)));
/// ```
#[must_use]
pub fn apply_bitwise_and_mask(mask: u64, left: &Value, right: &Value) -> bool {
    let masked_left = match left {
        Value::Uint(val) => Value::Uint(val & mask),
        Value::Int(val) => {
            // Convert u64 mask to i64, using bitwise representation for values > i64::MAX
            let i64_mask =
                i64::try_from(mask).unwrap_or_else(|_| i64::from_ne_bytes(mask.to_ne_bytes()));
            Value::Int(val & i64_mask)
        }
        _ => return false, // Can't apply bitwise operations to non-numeric values
    };
    apply_equal(&masked_left, right)
}

/// Apply bitwise AND operation for pattern matching
///
/// Performs bitwise AND operation between two integer values for pattern matching.
/// This implements magic(5)'s bare `&MASK` relational test: the file value must have
/// **every** bit in `right` (the mask) set, not merely *some* bit. Only works with
/// integer types (Uint and Int), returns `false` for other types.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically the mask from magic rule)
///
/// # Returns
///
/// `true` if `(left & right) == right` -- i.e. every bit set in `right` is also set
/// in `left` -- `false` otherwise or for non-integer types.
///
/// # libmagic Compatibility
///
/// This mirrors GNU `file`'s `magiccheck()` (`src/softmagic.c`), whose `'&'` relation
/// is `(v & l) == l` (`v` = file value, `l` = the rule's mask), i.e. "all masked bits
/// set" -- not "any masked bit set". A mask of `0` is vacuously satisfied by any file
/// value (every one of the zero required bits is trivially set), matching libmagic's
/// literal equality test. For a single-bit mask the two interpretations coincide,
/// which is why simple flag-check rules (`>6 leshort &0x0001 \b, encrypted`) are
/// unaffected either way -- only multi-bit bare-`&` masks distinguish them.
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
/// // Multi-bit mask: ALL masked bits must be set, not merely some
/// assert!(apply_bitwise_and(&Value::Uint(0xFF), &Value::Uint(0x0F)));
/// assert!(!apply_bitwise_and(&Value::Uint(0xF0), &Value::Uint(0x0F)));
/// assert!(!apply_bitwise_and(&Value::Uint(0x8F), &Value::Uint(0xFF))); // partial overlap fails
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
        // Unsigned integer bitwise AND: all bits in `right` (the mask) must be set in `left`.
        (Value::Uint(a), Value::Uint(b)) => (a & b) == *b,

        // Signed integer bitwise AND (cast to unsigned for bitwise operations)
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Int(b)) => {
            let (a, b) = (*a as u64, *b as u64);
            (a & b) == b
        }

        // Mixed signed/unsigned integer bitwise AND
        #[allow(clippy::cast_sign_loss)]
        (Value::Uint(a), Value::Int(b)) => {
            let b = *b as u64;
            (a & b) == b
        }
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Uint(b)) => ((*a as u64) & b) == *b,

        // Non-integer types cannot perform bitwise AND
        _ => false,
    }
}

/// Apply bitwise XOR operation for pattern matching
///
/// Performs bitwise XOR between two integer values. Returns `true` if the result is non-zero.
/// Only works with integer types (Uint and Int), returns `false` for other types.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value (typically from magic rule)
///
/// # Returns
///
/// `true` if the bitwise XOR result is non-zero, `false` otherwise or for non-integer types
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_bitwise_xor;
///
/// // XOR of different values is non-zero (true)
/// assert!(apply_bitwise_xor(&Value::Uint(0xFF), &Value::Uint(0x0F)));
///
/// // XOR of same values is zero (false)
/// assert!(!apply_bitwise_xor(&Value::Uint(42), &Value::Uint(42)));
///
/// // Non-integer types return false
/// assert!(!apply_bitwise_xor(
///     &Value::String("test".to_string()),
///     &Value::Uint(0x01),
/// ));
/// ```
#[must_use]
pub fn apply_bitwise_xor(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Uint(a), Value::Uint(b)) => (a ^ b) != 0,
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Int(b)) => ((*a as u64) ^ (*b as u64)) != 0,
        #[allow(clippy::cast_sign_loss)]
        (Value::Uint(a), Value::Int(b)) => (a ^ (*b as u64)) != 0,
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(a), Value::Uint(b)) => ((*a as u64) ^ b) != 0,
        _ => false,
    }
}

/// Apply bitwise NOT then compare with right value
///
/// Computes bitwise complement of the left (file) value, then checks equality with the right
/// (magic rule) value. Unlike `&` and `^` which test whether a bitwise result is non-zero,
/// `~` compares the complement against a specific expected value.
/// Only works with integer types, returns `false` for other types.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value to compare `!left` against
///
/// # Returns
///
/// `true` if `!left == right`, `false` otherwise or for non-integer types
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::Value;
/// use libmagic_rs::evaluator::operators::apply_bitwise_not;
///
/// // NOT of 0 is all bits set (u64::MAX)
/// assert!(apply_bitwise_not(&Value::Uint(0), &Value::Uint(u64::MAX)));
///
/// // NOT of -1 (all bits set) is 0
/// assert!(apply_bitwise_not(&Value::Int(-1), &Value::Int(0)));
///
/// // Non-integer types return false
/// assert!(!apply_bitwise_not(&Value::Bytes(vec![0xff]), &Value::Uint(0)));
/// ```
#[must_use]
pub fn apply_bitwise_not(left: &Value, right: &Value) -> bool {
    apply_bitwise_not_with_width(left, right, None)
}

/// Apply bitwise NOT with type-aware bit-width masking
///
/// When `bit_width` is provided, the complement is masked to the type's natural width.
/// For example, a `ubyte` (8-bit) NOT of `0x00` yields `0xFF`, not `u64::MAX`.
/// Without a bit width, the full 64-bit complement is used.
///
/// # Arguments
///
/// * `left` - The left-hand side value (typically from file data)
/// * `right` - The right-hand side value to compare `!left` against
/// * `bit_width` - Optional bit width for masking (8, 16, 32, or 64)
///
/// # Returns
///
/// `true` if the width-masked complement of `left` equals `right`
#[must_use]
pub fn apply_bitwise_not_with_width(left: &Value, right: &Value, bit_width: Option<u32>) -> bool {
    let complemented = match (left, bit_width) {
        (Value::Uint(val), Some(width)) if width < 64 => {
            let mask = (1u64 << width) - 1;
            Value::Uint(!val & mask)
        }
        (Value::Uint(val), _) => Value::Uint(!val),
        (Value::Int(val), _) => Value::Int(!*val),
        _ => return false,
    };
    apply_equal(&complemented, right)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Zero cases. Under the "all masked bits set" semantics (matching
        // libmagic's `(v & l) == l`), a zero mask is vacuously satisfied by
        // any file value -- there are no required bits to check. Only the
        // "value is zero but mask is not" case fails.
        assert!(!apply_bitwise_and(&Value::Uint(0), &Value::Uint(0xFF))); // Zero value, nonzero mask: unsatisfied
        assert!(apply_bitwise_and(&Value::Uint(0xFF), &Value::Uint(0))); // Zero mask: vacuously true
        assert!(apply_bitwise_and(&Value::Uint(0), &Value::Uint(0))); // Zero mask: vacuously true

        // Maximum values
        assert!(apply_bitwise_and(&Value::Uint(u64::MAX), &Value::Uint(1))); // Max & 1
        assert!(apply_bitwise_and(
            &Value::Uint(u64::MAX),
            &Value::Uint(u64::MAX)
        )); // Max & Max
    }

    #[test]
    fn test_apply_bitwise_and_uint_specific_patterns() {
        // Common magic number patterns. An 0xFF-per-byte mask asks "is every
        // bit in this byte region set" -- it is NOT satisfied just because
        // that byte region happens to be the value of interest (0x7F, 0x504B,
        // etc. are not all-ones bytes/words). A mask built from the ACTUAL
        // bits present in the value (mirroring the value itself in that
        // region) is what bare `&` is for; an 0xFF-style "extract this byte"
        // mask belongs with `BitwiseAndMask` + an explicit equality compare.
        assert!(!apply_bitwise_and(
            &Value::Uint(0x7F45_4C46),
            &Value::Uint(0xFF00_0000)
        )); // ELF's high byte (0x7F) is not all-ones
        assert!(apply_bitwise_and(
            &Value::Uint(0x7F45_4C46),
            &Value::Uint(0x7F00_0000)
        )); // Mask matching the ELF high byte exactly does satisfy
        assert!(!apply_bitwise_and(
            &Value::Uint(0x504B_0304),
            &Value::Uint(0xFFFF_0000)
        )); // ZIP's high word (0x504B) is not all-ones
        assert!(apply_bitwise_and(
            &Value::Uint(0x504B_0304),
            &Value::Uint(0x504B_0000)
        )); // Mask matching the ZIP high word exactly does satisfy
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
        // Zero cases with signed integers. As with the unsigned edge cases,
        // a zero mask is vacuously satisfied (see GOTCHAS S13.3).
        assert!(!apply_bitwise_and(&Value::Int(0), &Value::Int(0xFF))); // Zero value, nonzero mask: unsatisfied
        assert!(apply_bitwise_and(&Value::Int(0xFF), &Value::Int(0))); // Zero mask: vacuously true
        assert!(apply_bitwise_and(&Value::Int(0), &Value::Int(0))); // Zero mask: vacuously true
    }

    #[test]
    fn test_apply_bitwise_and_int_extreme_values() {
        // Extreme signed integer values
        assert!(apply_bitwise_and(&Value::Int(i64::MAX), &Value::Int(1))); // Max positive & 1
        assert!(apply_bitwise_and(
            &Value::Int(i64::MIN),
            &Value::Int(i64::MIN)
        )); // Min & Min (self-AND is always true)

        // Min (only the sign bit set) does NOT have every bit of an
        // all-ones mask set, so `apply_bitwise_and(MIN, -1)` is false --
        // only the reverse direction (does -1 have every bit of MIN set)
        // is true, since -1's bit pattern is a superset of MIN's.
        assert!(!apply_bitwise_and(&Value::Int(i64::MIN), &Value::Int(-1)));
        assert!(apply_bitwise_and(&Value::Int(-1), &Value::Int(i64::MIN)));
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
        // Negative int with uint (negative numbers have high bits set).
        // -1's bit pattern is all-ones, so it has every bit of 1 set: true.
        assert!(apply_bitwise_and(&Value::Int(-1), &Value::Uint(1)));
        // But 1 does NOT have every bit of -1 (all-ones) set: false. This is
        // the asymmetric case pinned by test_apply_bitwise_and_is_not_commutative_in_general.
        assert!(!apply_bitwise_and(&Value::Uint(1), &Value::Int(-1)));
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
            (0b1111_1111_u64, 0b0000_0000_u64, true), // Zero mask: vacuously satisfied (no required bits)
            (0b0000_0000_u64, 0b1111_1111_u64, false), // Value is zero, mask is not: unsatisfied
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
        // Test patterns commonly found in magic files. Under the "all masked
        // bits set" semantics, a mask only matches when EVERY one of its 1
        // bits is also set in the value -- so a mask must be built from bits
        // that are genuinely known-set, not merely "some byte region of
        // interest" (that latter pattern belongs to `Operator::Equal` after
        // masking with `BitwiseAndMask`, or plain `Equal` on the exact value).

        // ELF magic number (0x7F454C46) is not all-ones, so it can never
        // satisfy an all-ones mask via bare `&`; identity is what `Equal`
        // is for. This documents why `&0xFFFFFFFF` is NOT how one would
        // check "is this exactly the ELF magic" in a real magic file.
        let elf_magic = Value::Uint(0x7F45_4C46);
        let elf_all_ones_mask = Value::Uint(0xFFFF_FFFF);
        assert!(!apply_bitwise_and(&elf_magic, &elf_all_ones_mask));

        // Masks built from bits that ARE present in elf_magic still match,
        // because every 1 bit in the mask is also 1 in the value.
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x7F00_0000))); // First byte (0x7F, matches exactly)
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0045_0000))); // Second byte 'E' (0x45, matches exactly)
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0000_4C00))); // Third byte 'L' (0x4C, matches exactly)
        assert!(apply_bitwise_and(&elf_magic, &Value::Uint(0x0000_0046))); // Fourth byte 'F' (0x46, matches exactly)

        // ZIP magic number (0x504B0304): mask built from the actual "PK"
        // signature bits matches; a bit that's genuinely unset in the value
        // (bit 0) does not.
        let zip_magic = Value::Uint(0x504B_0304);
        assert!(apply_bitwise_and(&zip_magic, &Value::Uint(0x504B_0000))); // PK signature (matches exactly)
        assert!(!apply_bitwise_and(&zip_magic, &Value::Uint(0x0000_0001))); // Bit 0 not set

        // PDF magic (%PDF): an 0xFF-style "give me this byte region" mask is
        // NOT satisfied unless the value's bits in that region are all 1 --
        // '%' (0x25) and 'P' (0x50) are not all-ones bytes, so a bare 0xFF
        // mask over that byte fails. This is the case that distinguishes
        // "any bit set" from "all bits set": real magic files would use
        // `Operator::Equal` (or `BitwiseAndMask` + explicit compare value)
        // to test "this byte equals 0x25", not bare `&0xFF00_0000`.
        let pdf_magic = Value::Uint(0x2550_4446); // "%PDF" as uint32
        assert!(!apply_bitwise_and(&pdf_magic, &Value::Uint(0xFF00_0000))); // '%' (0x25) is not all-ones
        assert!(!apply_bitwise_and(&pdf_magic, &Value::Uint(0x00FF_0000))); // 'P' (0x50) is not all-ones
    }

    #[test]
    fn test_apply_bitwise_and_is_not_commutative_in_general() {
        // `apply_bitwise_and(left, right)` tests "does `left` have every bit
        // of `right` set" -- this is a genuinely asymmetric relation (`left`
        // is the file value, `right` is the rule's mask), matching libmagic's
        // `(v & l) == l`. It is NOT commutative in general: swapping which
        // operand plays "value" vs "mask" changes the question being asked.
        // (It IS trivially symmetric when left == right, or when one side's
        // bits are a superset of the other's in both directions -- e.g. two
        // equal masks -- but that is not the general case.) An earlier
        // revision of this crate implemented "any bit set" (`(a & b) != 0`),
        // which genuinely is commutative; that was the wrong semantics (see
        // GOTCHAS S13.3) and this test's name/assertions have been corrected
        // accordingly rather than deleted, so the asymmetry stays pinned.
        let asymmetric_cases = vec![
            (Value::Uint(0xFF), Value::Uint(0x0F)), // 0xFF has all of 0x0F's bits; 0x0F does not have all of 0xFF's
            (Value::Uint(1), Value::Int(-1)), // 1 does not have all bits of all-ones; all-ones has bit 0
        ];

        for (left, right) in asymmetric_cases {
            let left_to_right = apply_bitwise_and(&left, &right);
            let right_to_left = apply_bitwise_and(&right, &left);
            assert_ne!(
                left_to_right, right_to_left,
                "expected asymmetric result for {left:?} & {right:?} vs swapped operands"
            );
        }

        // Self-AND is always true regardless of operand order (a value always
        // has every one of its own bits set), so swapping identical operands
        // trivially agrees.
        let self_case = Value::Uint(0x5555);
        assert_eq!(
            apply_bitwise_and(&self_case, &self_case),
            apply_bitwise_and(&self_case, &self_case)
        );
        assert!(apply_bitwise_and(&self_case, &self_case));
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

    #[test]
    fn test_apply_bitwise_xor_uint() {
        assert!(apply_bitwise_xor(&Value::Uint(0xFF), &Value::Uint(0x0F)));
        assert!(!apply_bitwise_xor(&Value::Uint(0xFF), &Value::Uint(0xFF)));
        assert!(apply_bitwise_xor(&Value::Uint(1), &Value::Uint(2)));
        assert!(!apply_bitwise_xor(&Value::Uint(0), &Value::Uint(0)));
    }

    #[test]
    fn test_apply_bitwise_xor_int() {
        assert!(apply_bitwise_xor(&Value::Int(0xFF), &Value::Int(0x0F)));
        assert!(!apply_bitwise_xor(&Value::Int(42), &Value::Int(42)));
        assert!(apply_bitwise_xor(&Value::Int(-1), &Value::Int(0)));
    }

    #[test]
    fn test_apply_bitwise_xor_cross_type() {
        assert!(apply_bitwise_xor(&Value::Uint(0xFF), &Value::Int(0x0F)));
        assert!(apply_bitwise_xor(&Value::Int(0xFF), &Value::Uint(0x0F)));
        assert!(!apply_bitwise_xor(&Value::Uint(42), &Value::Int(42)));
    }

    #[test]
    fn test_apply_bitwise_xor_same_value() {
        assert!(!apply_bitwise_xor(&Value::Uint(100), &Value::Uint(100)));
        assert!(!apply_bitwise_xor(&Value::Int(-1), &Value::Int(-1)));
    }

    #[test]
    fn test_apply_bitwise_xor_non_numeric() {
        assert!(!apply_bitwise_xor(
            &Value::Bytes(vec![1, 2]),
            &Value::Uint(1)
        ));
        assert!(!apply_bitwise_xor(
            &Value::String("x".to_string()),
            &Value::Uint(0xFF)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_uint() {
        assert!(apply_bitwise_not(&Value::Uint(0), &Value::Uint(u64::MAX)));
        assert!(apply_bitwise_not(&Value::Uint(u64::MAX), &Value::Uint(0)));
        assert!(!apply_bitwise_not(&Value::Uint(0xFF), &Value::Uint(0)));
    }

    #[test]
    fn test_apply_bitwise_not_int() {
        assert!(apply_bitwise_not(&Value::Int(0), &Value::Int(-1)));
        assert!(apply_bitwise_not(&Value::Int(-1), &Value::Int(0)));
    }

    #[test]
    fn test_apply_bitwise_not_all_bits_set() {
        assert!(apply_bitwise_not(
            &Value::Uint(0xFFFF_FFFF_FFFF_FFFF),
            &Value::Uint(0)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_non_numeric() {
        assert!(!apply_bitwise_not(
            &Value::Bytes(vec![0xff]),
            &Value::Uint(0)
        ));
        assert!(!apply_bitwise_not(
            &Value::String("x".to_string()),
            &Value::Uint(0)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_with_byte_width() {
        // At byte width (8 bits), ~0x00 = 0xFF
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0x00),
            &Value::Uint(0xFF),
            Some(8)
        ));
        // At byte width, ~0xFF = 0x00
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0xFF),
            &Value::Uint(0x00),
            Some(8)
        ));
        // At byte width, ~0x42 = 0xBD
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0x42),
            &Value::Uint(0xBD),
            Some(8)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_with_short_width() {
        // At short width (16 bits), ~0x0000 = 0xFFFF
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0x0000),
            &Value::Uint(0xFFFF),
            Some(16)
        ));
        // At short width, ~0x1234 = 0xEDCB
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0x1234),
            &Value::Uint(0xEDCB),
            Some(16)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_with_long_width() {
        // At long width (32 bits), ~0x00000000 = 0xFFFFFFFF
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0x0000_0000),
            &Value::Uint(0xFFFF_FFFF),
            Some(32)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_with_quad_width() {
        // At quad width (64 bits), ~0 = u64::MAX (no masking needed)
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0),
            &Value::Uint(u64::MAX),
            Some(64)
        ));
    }

    #[test]
    fn test_apply_bitwise_not_with_no_width() {
        // No width specified: full 64-bit complement (same as apply_bitwise_not)
        assert!(apply_bitwise_not_with_width(
            &Value::Uint(0),
            &Value::Uint(u64::MAX),
            None
        ));
    }
}
