// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Number and offset parsing tests.
//!
//! Covers `parse_decimal_number`/`parse_hex_number`/`parse_number` and the
//! `parse_offset`/`parse_rule_offset` family: absolute, negative, relative
//! (`&N`), and indirect-offset rule-level parsing.

use super::*;

#[test]
fn test_parse_decimal_number() {
    assert_eq!(parse_decimal_number("123"), Ok(("", 123)));
    assert_eq!(parse_decimal_number("0"), Ok(("", 0)));
    assert_eq!(parse_decimal_number("999"), Ok(("", 999)));

    // Should fail on non-digits
    assert!(parse_decimal_number("abc").is_err());
    assert!(parse_decimal_number("").is_err());
}

#[test]
fn test_parse_hex_number() {
    assert_eq!(parse_hex_number("0x0"), Ok(("", 0)));
    assert_eq!(parse_hex_number("0x10"), Ok(("", 16)));
    assert_eq!(parse_hex_number("0xFF"), Ok(("", 255)));
    assert_eq!(parse_hex_number("0xabc"), Ok(("", 2748)));
    assert_eq!(parse_hex_number("0xABC"), Ok(("", 2748)));

    // Should fail without 0x prefix
    assert!(parse_hex_number("FF").is_err());
    assert!(parse_hex_number("10").is_err());

    // Should fail on invalid hex digits
    assert!(parse_hex_number("0xGG").is_err());
}

#[test]
fn test_parse_number_positive() {
    // Decimal numbers
    assert_eq!(parse_number("0"), Ok(("", 0)));
    assert_eq!(parse_number("123"), Ok(("", 123)));
    assert_eq!(parse_number("999"), Ok(("", 999)));

    // Hexadecimal numbers
    assert_eq!(parse_number("0x0"), Ok(("", 0)));
    assert_eq!(parse_number("0x10"), Ok(("", 16)));
    assert_eq!(parse_number("0xFF"), Ok(("", 255)));
    assert_eq!(parse_number("0xabc"), Ok(("", 2748)));
}

#[test]
fn test_parse_number_negative() {
    // Negative decimal numbers
    assert_eq!(parse_number("-1"), Ok(("", -1)));
    assert_eq!(parse_number("-123"), Ok(("", -123)));
    assert_eq!(parse_number("-999"), Ok(("", -999)));

    // Negative hexadecimal numbers
    assert_eq!(parse_number("-0x1"), Ok(("", -1)));
    assert_eq!(parse_number("-0x10"), Ok(("", -16)));
    assert_eq!(parse_number("-0xFF"), Ok(("", -255)));
    assert_eq!(parse_number("-0xabc"), Ok(("", -2748)));
}

#[test]
fn test_parse_number_edge_cases() {
    // Zero with different formats
    assert_eq!(parse_number("0"), Ok(("", 0)));
    assert_eq!(parse_number("-0"), Ok(("", 0)));
    assert_eq!(parse_number("0x0"), Ok(("", 0)));
    assert_eq!(parse_number("-0x0"), Ok(("", 0)));

    // Large numbers
    assert_eq!(parse_number("2147483647"), Ok(("", 2_147_483_647))); // i32::MAX
    assert_eq!(parse_number("-2147483648"), Ok(("", -2_147_483_648))); // i32::MIN
    assert_eq!(parse_number("0x7FFFFFFF"), Ok(("", 2_147_483_647))); // i32::MAX in hex

    // Should fail on invalid input
    assert!(parse_number("").is_err());
    assert!(parse_number("abc").is_err());
    assert!(parse_number("0xGG").is_err());
    assert!(parse_number("--123").is_err());
}

#[test]
fn test_parse_number_with_remaining_input() {
    // Use helper function to reduce code duplication
    test_number_with_remaining_input();
}

#[test]
fn test_parse_offset_absolute_positive() {
    assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("123"), Ok(("", OffsetSpec::Absolute(123))));
    assert_eq!(parse_offset("999"), Ok(("", OffsetSpec::Absolute(999))));

    // Hexadecimal offsets
    assert_eq!(parse_offset("0x0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16))));
    assert_eq!(parse_offset("0xFF"), Ok(("", OffsetSpec::Absolute(255))));
    assert_eq!(parse_offset("0xabc"), Ok(("", OffsetSpec::Absolute(2748))));
}

#[test]
fn test_parse_offset_absolute_negative() {
    assert_eq!(parse_offset("-1"), Ok(("", OffsetSpec::Absolute(-1))));
    assert_eq!(parse_offset("-123"), Ok(("", OffsetSpec::Absolute(-123))));
    assert_eq!(parse_offset("-999"), Ok(("", OffsetSpec::Absolute(-999))));

    // Negative hexadecimal offsets
    assert_eq!(parse_offset("-0x1"), Ok(("", OffsetSpec::Absolute(-1))));
    assert_eq!(parse_offset("-0x10"), Ok(("", OffsetSpec::Absolute(-16))));
    assert_eq!(parse_offset("-0xFF"), Ok(("", OffsetSpec::Absolute(-255))));
    assert_eq!(
        parse_offset("-0xabc"),
        Ok(("", OffsetSpec::Absolute(-2748)))
    );
}

#[test]
fn test_parse_offset_with_whitespace() {
    // Leading whitespace
    assert_eq!(parse_offset(" 123"), Ok(("", OffsetSpec::Absolute(123))));
    assert_eq!(parse_offset("  0x10"), Ok(("", OffsetSpec::Absolute(16))));
    assert_eq!(parse_offset("\t-42"), Ok(("", OffsetSpec::Absolute(-42))));

    // Trailing whitespace
    assert_eq!(parse_offset("123 "), Ok(("", OffsetSpec::Absolute(123))));
    assert_eq!(parse_offset("0x10  "), Ok(("", OffsetSpec::Absolute(16))));
    assert_eq!(parse_offset("-42\t"), Ok(("", OffsetSpec::Absolute(-42))));

    // Both leading and trailing whitespace
    assert_eq!(parse_offset(" 123 "), Ok(("", OffsetSpec::Absolute(123))));
    assert_eq!(parse_offset("  0x10  "), Ok(("", OffsetSpec::Absolute(16))));
    assert_eq!(parse_offset("\t-42\t"), Ok(("", OffsetSpec::Absolute(-42))));
}

#[test]
fn test_parse_offset_with_remaining_input() {
    // Should parse offset and leave remaining input
    assert_eq!(
        parse_offset("123 byte"),
        Ok(("byte", OffsetSpec::Absolute(123)))
    );
    assert_eq!(parse_offset("0xFF ="), Ok(("=", OffsetSpec::Absolute(255))));
    assert_eq!(
        parse_offset("-42,next"),
        Ok((",next", OffsetSpec::Absolute(-42)))
    );
    assert_eq!(
        parse_offset("0x10\tlong"),
        Ok(("long", OffsetSpec::Absolute(16)))
    );
}

#[test]
fn test_parse_offset_edge_cases() {
    // Zero with different formats. `-0` / `-0x0` are the magic(5)
    // "0 bytes from end of file" form (the EOF position, `buffer.len()`),
    // NOT absolute offset 0 -- the leading `-` is significant even though
    // `-0 == 0` numerically. They encode `FromEnd(0)`; unsigned `0` / `0x0`
    // stay `Absolute(0)`. See gzip's `>>-0 offset >48` trailing-size gate.
    assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("-0"), Ok(("", OffsetSpec::FromEnd(0))));
    assert_eq!(parse_offset("0x0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("-0x0"), Ok(("", OffsetSpec::FromEnd(0))));

    // Large offsets
    assert_eq!(
        parse_offset("2147483647"),
        Ok(("", OffsetSpec::Absolute(2_147_483_647)))
    );
    assert_eq!(
        parse_offset("-2147483648"),
        Ok(("", OffsetSpec::Absolute(-2_147_483_648)))
    );
    assert_eq!(
        parse_offset("0x7FFFFFFF"),
        Ok(("", OffsetSpec::Absolute(2_147_483_647)))
    );

    // Should fail on invalid input
    assert!(parse_offset("").is_err());
    assert!(parse_offset("abc").is_err());
    assert!(parse_offset("0xGG").is_err());
    assert!(parse_offset("--123").is_err());
}

#[test]
fn test_parse_offset_common_magic_file_values() {
    // Common offsets found in magic files
    assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0)))); // File start
    assert_eq!(parse_offset("4"), Ok(("", OffsetSpec::Absolute(4)))); // After magic number
    assert_eq!(parse_offset("16"), Ok(("", OffsetSpec::Absolute(16)))); // Common header offset
    assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16)))); // Same as above in hex
    assert_eq!(parse_offset("512"), Ok(("", OffsetSpec::Absolute(512)))); // Sector boundary
    assert_eq!(parse_offset("0x200"), Ok(("", OffsetSpec::Absolute(512)))); // Same in hex

    // Negative offsets (from end of file)
    assert_eq!(parse_offset("-4"), Ok(("", OffsetSpec::Absolute(-4)))); // 4 bytes from end
    assert_eq!(parse_offset("-16"), Ok(("", OffsetSpec::Absolute(-16)))); // 16 bytes from end
    assert_eq!(parse_offset("-0x10"), Ok(("", OffsetSpec::Absolute(-16)))); // Same in hex
}

#[test]
fn test_parse_offset_boundary_values() {
    // Test boundary values that might cause issues
    assert_eq!(parse_offset("1"), Ok(("", OffsetSpec::Absolute(1))));
    assert_eq!(parse_offset("-1"), Ok(("", OffsetSpec::Absolute(-1))));

    // Powers of 2 (common in binary formats)
    assert_eq!(parse_offset("256"), Ok(("", OffsetSpec::Absolute(256))));
    assert_eq!(parse_offset("0x100"), Ok(("", OffsetSpec::Absolute(256))));
    assert_eq!(parse_offset("1024"), Ok(("", OffsetSpec::Absolute(1024))));
    assert_eq!(parse_offset("0x400"), Ok(("", OffsetSpec::Absolute(1024))));

    // Large but reasonable file offsets
    assert_eq!(
        parse_offset("1048576"),
        Ok(("", OffsetSpec::Absolute(1_048_576)))
    ); // 1MB
    assert_eq!(
        parse_offset("0x100000"),
        Ok(("", OffsetSpec::Absolute(1_048_576)))
    );
}

#[test]
fn test_parse_offset_relative() {
    // `&N` -- relative offset from the GNU `file` previous-match anchor.
    // Bare (`&0`), explicit-positive (`&+4`), and negative (`&-4`) forms
    // all decode to `OffsetSpec::Relative(N)`.
    assert_eq!(parse_offset("&0"), Ok(("", OffsetSpec::Relative(0))));
    assert_eq!(parse_offset("&4"), Ok(("", OffsetSpec::Relative(4))));
    assert_eq!(parse_offset("&+4"), Ok(("", OffsetSpec::Relative(4))));
    assert_eq!(parse_offset("&-4"), Ok(("", OffsetSpec::Relative(-4))));
    assert_eq!(parse_offset("&0x10"), Ok(("", OffsetSpec::Relative(16))));
    assert_eq!(parse_offset("&-0x10"), Ok(("", OffsetSpec::Relative(-16))));

    // Whitespace handling around the relative offset.
    assert_eq!(parse_offset(" &0 "), Ok(("", OffsetSpec::Relative(0))));
    assert_eq!(
        parse_offset("&0 ubyte"),
        Ok(("ubyte", OffsetSpec::Relative(0)))
    );

    // Bare `&` with no number must fail.
    assert!(parse_offset("&").is_err(), "bare `&` must fail");
    assert!(parse_offset("& ").is_err(), "`&` with only space must fail");
}

#[test]
fn test_parse_rule_offset_indirect_child() {
    // Level 1 child with indirect offset: >(0x3c.l)
    assert_eq!(
        parse_rule_offset(">(0x3c.l)"),
        Ok((
            "",
            (
                1,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    base_relative: false,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Little,
                        signed: true
                    },
                    adjustment: 0,
                    adjustment_op: IndirectAdjustmentOp::Add,
                    result_relative: false,
                    endian: Endianness::Little,
                }
            )
        ))
    );
    // Level 2 child with adjustment after paren: >>(0x3c.l)+4
    assert_eq!(
        parse_rule_offset(">>(0x3c.l)+4"),
        Ok((
            "",
            (
                2,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    base_relative: false,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Little,
                        signed: true
                    },
                    adjustment: 4,
                    adjustment_op: IndirectAdjustmentOp::Add,
                    result_relative: false,
                    endian: Endianness::Little,
                }
            )
        ))
    );
}

#[test]
fn test_parse_rule_offset_indirect_with_remaining() {
    // >(0x3c.l) followed by type keyword
    assert_eq!(
        parse_rule_offset(">(0x3c.l) string"),
        Ok((
            "string",
            (
                1,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    base_relative: false,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Little,
                        signed: true
                    },
                    adjustment: 0,
                    adjustment_op: IndirectAdjustmentOp::Add,
                    result_relative: false,
                    endian: Endianness::Little,
                }
            )
        ))
    );
    // >(0x3c.l)+4 followed by type keyword
    assert_eq!(
        parse_rule_offset(">(0x3c.l)+4 string"),
        Ok((
            "string",
            (
                1,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    base_relative: false,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Little,
                        signed: true
                    },
                    adjustment: 4,
                    adjustment_op: IndirectAdjustmentOp::Add,
                    result_relative: false,
                    endian: Endianness::Little,
                }
            )
        ))
    );
}

#[test]
fn test_parse_rule_offset_absolute() {
    assert_eq!(
        parse_rule_offset("0"),
        Ok(("", (0, OffsetSpec::Absolute(0))))
    );
    assert_eq!(
        parse_rule_offset("16"),
        Ok(("", (0, OffsetSpec::Absolute(16))))
    );
    assert_eq!(
        parse_rule_offset("0x10"),
        Ok(("", (0, OffsetSpec::Absolute(16))))
    );
    assert_eq!(
        parse_rule_offset("-4"),
        Ok(("", (0, OffsetSpec::Absolute(-4))))
    );
}

#[test]
fn test_parse_rule_offset_child_rules() {
    assert_eq!(
        parse_rule_offset(">4"),
        Ok(("", (1, OffsetSpec::Absolute(4))))
    );
    assert_eq!(
        parse_rule_offset(">>8"),
        Ok(("", (2, OffsetSpec::Absolute(8))))
    );
    assert_eq!(
        parse_rule_offset(">>>12"),
        Ok(("", (3, OffsetSpec::Absolute(12))))
    );
}

#[test]
fn test_parse_rule_offset_with_whitespace() {
    assert_eq!(
        parse_rule_offset(" 0 "),
        Ok(("", (0, OffsetSpec::Absolute(0))))
    );
    assert_eq!(
        parse_rule_offset("  >4  "),
        Ok(("", (1, OffsetSpec::Absolute(4))))
    );
    assert_eq!(
        parse_rule_offset("\t>>0x10\t"),
        Ok(("", (2, OffsetSpec::Absolute(16))))
    );
}

#[test]
fn test_parse_rule_offset_with_remaining_input() {
    assert_eq!(
        parse_rule_offset("0 byte"),
        Ok(("byte", (0, OffsetSpec::Absolute(0))))
    );
    assert_eq!(
        parse_rule_offset(">4 string"),
        Ok(("string", (1, OffsetSpec::Absolute(4))))
    );
}
