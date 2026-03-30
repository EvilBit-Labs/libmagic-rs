// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::parser::ast::Endianness;
use crate::parser::ast::PStringLengthWidth;

/// Helper function to test parsing with various whitespace patterns
#[allow(dead_code)] // TODO: Use this helper in future whitespace tests
fn test_with_whitespace_variants<T, F>(input: &str, expected: &T, parser: F)
where
    T: Clone + PartialEq + std::fmt::Debug,
    F: Fn(&str) -> IResult<&str, T>,
{
    // Test with various whitespace patterns - pre-allocate Vec with known capacity
    let mut whitespace_variants = Vec::with_capacity(9);
    whitespace_variants.extend([
        format!(" {input}"),    // Leading space
        format!("  {input}"),   // Leading spaces
        format!("\t{input}"),   // Leading tab
        format!("{input} "),    // Trailing space
        format!("{input}  "),   // Trailing spaces
        format!("{input}\t"),   // Trailing tab
        format!(" {input} "),   // Both leading and trailing space
        format!("  {input}  "), // Both leading and trailing spaces
        format!("\t{input}\t"), // Both leading and trailing tabs
    ]);

    for variant in whitespace_variants {
        assert_eq!(
            parser(&variant),
            Ok(("", expected.clone())),
            "Failed to parse with whitespace: '{variant}'"
        );
    }
}

/// Helper function to test number parsing with remaining input
fn test_number_with_remaining_input() {
    // Pre-allocate with known capacity for better performance
    let test_cases = [
        ("123abc", 123, "abc"),
        ("0xFF rest", 255, " rest"),
        ("-42 more", -42, " more"),
        ("0x10,next", 16, ",next"),
    ];

    for (input, expected_num, expected_remaining) in test_cases {
        assert_eq!(
            parse_number(input),
            Ok((expected_remaining, expected_num)),
            "Failed to parse number with remaining input: '{input}'"
        );
    }
}

#[test]
fn test_parse_pstring_invalid_suffix_rejected() {
    // Invalid suffix characters after '/' should produce a parse error
    let invalid_cases = ["pstring/Z", "pstring/X", "pstring/W", "pstring/1"];
    for input in invalid_cases {
        let result = parse_type_and_operator(input);
        assert!(
            result.is_err(),
            "Expected error for invalid suffix: {input}"
        );
    }
}

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
    // Zero with different formats
    assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("-0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("0x0"), Ok(("", OffsetSpec::Absolute(0))));
    assert_eq!(parse_offset("-0x0"), Ok(("", OffsetSpec::Absolute(0))));

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

// Indirect offset parsing tests
#[test]
fn test_parse_offset_indirect_all_specifiers() {
    // .b / .B - byte
    assert_eq!(
        parse_offset("(0.b)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Byte { signed: false },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0.B)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Byte { signed: false },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );

    // .s - short native, .S - short big-endian
    assert_eq!(
        parse_offset("(0.s)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0.S)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Big,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Big,
            }
        ))
    );

    // .l - long native, .L - long big-endian
    assert_eq!(
        parse_offset("(0x3c.l)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0x3c.L)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Big,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Big,
            }
        ))
    );

    // .q - quad native, .Q - quad big-endian
    assert_eq!(
        parse_offset("(0.q)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Quad {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0.Q)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Quad {
                    endian: Endianness::Big,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Big,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_with_positive_adjustment() {
    assert_eq!(
        parse_offset("(0x3c.l+4)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 4,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0.b+0xFF)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Byte { signed: false },
                adjustment: 255,
                endian: Endianness::Native,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_with_negative_adjustment() {
    assert_eq!(
        parse_offset("(0x3c.l-8)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: -8,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(100.s-0x10)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 100,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: -16,
                endian: Endianness::Native,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_negative_base() {
    // Negative base offsets (from end of file)
    assert_eq!(
        parse_offset("(-4.l)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: -4,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    assert_eq!(
        parse_offset("(-0x10.s+2)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: -16,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 2,
                endian: Endianness::Native,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_hex_base() {
    assert_eq!(
        parse_offset("(0xFF.l)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0xFF,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_with_whitespace() {
    // Leading whitespace should be handled
    assert_eq!(
        parse_offset(" (0x3c.l)"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
    // Trailing content should be left unconsumed
    assert_eq!(
        parse_offset("(0x3c.l) string"),
        Ok((
            "string",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Native,
                    signed: false
                },
                adjustment: 0,
                endian: Endianness::Native,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_parse_failures() {
    // Missing closing paren
    assert!(parse_offset("(0x3c.l").is_err());
    // Missing dot and type
    assert!(parse_offset("(0x3c)").is_err());
    // Invalid specifier character
    assert!(parse_offset("(0x3c.x)").is_err());
    // Empty parens
    assert!(parse_offset("()").is_err());
    // Missing base
    assert!(parse_offset("(.l)").is_err());
}

#[test]
fn test_parse_rule_offset_indirect() {
    // Level 0 indirect
    assert_eq!(
        parse_rule_offset("(0x3c.l)"),
        Ok((
            "",
            (
                0,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Native,
                        signed: false
                    },
                    adjustment: 0,
                    endian: Endianness::Native,
                }
            )
        ))
    );
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
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Native,
                        signed: false
                    },
                    adjustment: 0,
                    endian: Endianness::Native,
                }
            )
        ))
    );
    // Level 2 child with indirect offset + adjustment
    assert_eq!(
        parse_rule_offset(">>(0x3c.l+4)"),
        Ok((
            "",
            (
                2,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Native,
                        signed: false
                    },
                    adjustment: 4,
                    endian: Endianness::Native,
                }
            )
        ))
    );
}

#[test]
fn test_parse_rule_offset_indirect_with_remaining() {
    assert_eq!(
        parse_rule_offset(">(0x3c.l) string"),
        Ok((
            "string",
            (
                1,
                OffsetSpec::Indirect {
                    base_offset: 0x3c,
                    pointer_type: TypeKind::Long {
                        endian: Endianness::Native,
                        signed: false
                    },
                    adjustment: 0,
                    endian: Endianness::Native,
                }
            )
        ))
    );
}

// Operator parsing tests
#[test]
fn test_parse_operator_equality() {
    // Single equals sign
    assert_eq!(parse_operator("="), Ok(("", Operator::Equal)));

    // Double equals sign
    assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));

    // With whitespace
    assert_eq!(parse_operator(" = "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("  ==  "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("\t=\t"), Ok(("", Operator::Equal)));
}

#[test]
fn test_parse_operator_inequality() {
    // Not equals
    assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));

    // Alternative not equals syntax
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));

    // With whitespace
    assert_eq!(parse_operator(" != "), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("  <>  "), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("\t!=\t"), Ok(("", Operator::NotEqual)));
}

#[test]
fn test_parse_operator_bitwise_and() {
    // Bitwise AND
    assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));

    // With whitespace
    assert_eq!(parse_operator(" & "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("  &  "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("\t&\t"), Ok(("", Operator::BitwiseAnd)));
}

#[test]
fn test_parse_operator_bitwise_xor() {
    assert_eq!(parse_operator("^"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator(" ^ "), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("  ^  "), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("\t^\t"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator("^ 0xFF"), Ok(("0xFF", Operator::BitwiseXor)));
    assert!(parse_operator("^^").is_err());
}

#[test]
fn test_parse_operator_bitwise_not() {
    assert_eq!(parse_operator("~"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator(" ~ "), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("  ~  "), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("\t~\t"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator("~ 0xff"), Ok(("0xff", Operator::BitwiseNot)));
    assert!(parse_operator("~~").is_err());
}

#[test]
fn test_parse_operator_any_value() {
    assert_eq!(parse_operator("x"), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator(" x "), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("  x  "), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("\tx\t"), Ok(("", Operator::AnyValue)));
    assert_eq!(parse_operator("x 42"), Ok(("42", Operator::AnyValue)));
    assert_eq!(
        parse_operator("x version"),
        Ok(("version", Operator::AnyValue))
    );
}

#[test]
fn test_parse_operator_with_remaining_input() {
    // Should parse operator and leave remaining input
    assert_eq!(parse_operator("= 123"), Ok(("123", Operator::Equal)));
    assert_eq!(
        parse_operator("!= value"),
        Ok(("value", Operator::NotEqual))
    );
    assert_eq!(parse_operator("& 0xFF"), Ok(("0xFF", Operator::BitwiseAnd)));
    assert_eq!(parse_operator("^ 0xFF"), Ok(("0xFF", Operator::BitwiseXor)));
    assert_eq!(parse_operator("~ 0xff"), Ok(("0xff", Operator::BitwiseNot)));
    assert_eq!(parse_operator("x 42"), Ok(("42", Operator::AnyValue)));
    assert_eq!(
        parse_operator("== \"string\""),
        Ok(("\"string\"", Operator::Equal))
    );
    assert_eq!(parse_operator("<> test"), Ok(("test", Operator::NotEqual)));
}

#[test]
fn test_parse_operator_precedence() {
    // Test that longer operators are matched first
    // This ensures "==" is parsed as Equal, not "=" followed by "="
    assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("== extra"), Ok(("extra", Operator::Equal)));

    // Test that "!=" is parsed correctly, not as "!" followed by "="
    assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));
    assert_eq!(
        parse_operator("!= extra"),
        Ok(("extra", Operator::NotEqual))
    );

    // Test that "<>" is parsed correctly, not as "<" followed by ">"
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));
    assert_eq!(
        parse_operator("<> extra"),
        Ok(("extra", Operator::NotEqual))
    );

    // Test that "<=" is parsed as LessEqual, not "<" followed by "="
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));
    assert_eq!(
        parse_operator("<= extra"),
        Ok(("extra", Operator::LessEqual))
    );

    // Test that ">=" is parsed as GreaterEqual, not ">" followed by "="
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));
    assert_eq!(
        parse_operator(">= extra"),
        Ok(("extra", Operator::GreaterEqual))
    );
}

#[test]
fn test_parse_operator_invalid_input() {
    // Should fail on invalid operators
    assert!(parse_operator("").is_err());
    assert!(parse_operator("abc").is_err());
    assert!(parse_operator("123").is_err());
    assert!(parse_operator("!").is_err());
    assert!(parse_operator("===").is_err()); // Too many equals
    assert!(parse_operator("&&").is_err()); // Double ampersand not supported
    assert!(parse_operator("^^").is_err()); // Double caret not supported
    assert!(parse_operator("~~").is_err()); // Double tilde not supported
}

#[test]
fn test_parse_operator_edge_cases() {
    // Test operators at start of various contexts - multispace0 consumes all whitespace
    assert_eq!(parse_operator("=\n"), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("!=\r\n"), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator("&\t\t"), Ok(("", Operator::BitwiseAnd)));

    // Test with mixed whitespace
    assert_eq!(parse_operator(" \t = \t "), Ok(("", Operator::Equal)));
    assert_eq!(parse_operator("\t != \t"), Ok(("", Operator::NotEqual)));
    assert_eq!(parse_operator(" \t& \t "), Ok(("", Operator::BitwiseAnd)));
    assert_eq!(parse_operator(" ^\n"), Ok(("", Operator::BitwiseXor)));
    assert_eq!(parse_operator(" ~\r\n"), Ok(("", Operator::BitwiseNot)));
    assert_eq!(parse_operator(" x\t\t"), Ok(("", Operator::AnyValue)));
}

#[test]
fn test_parse_operator_common_magic_file_patterns() {
    // Test patterns commonly found in magic files
    assert_eq!(
        parse_operator("= 0x7f454c46"),
        Ok(("0x7f454c46", Operator::Equal))
    );
    assert_eq!(parse_operator("!= 0"), Ok(("0", Operator::NotEqual)));
    assert_eq!(
        parse_operator("& 0xFF00"),
        Ok(("0xFF00", Operator::BitwiseAnd))
    );
    assert_eq!(
        parse_operator("== \"ELF\""),
        Ok(("\"ELF\"", Operator::Equal))
    );
    assert_eq!(parse_operator("<> \"\""), Ok(("\"\"", Operator::NotEqual)));
    assert_eq!(parse_operator("^ 0xff"), Ok(("0xff", Operator::BitwiseXor)));
    assert_eq!(parse_operator("~ 0x80"), Ok(("0x80", Operator::BitwiseNot)));
    assert_eq!(
        parse_operator("x version"),
        Ok(("version", Operator::AnyValue))
    );

    // Test with various spacing patterns found in real magic files
    assert_eq!(
        parse_operator("=\t0x504b0304"),
        Ok(("0x504b0304", Operator::Equal))
    );
    assert_eq!(parse_operator("!=  0"), Ok(("0", Operator::NotEqual)));
    assert_eq!(
        parse_operator("&   0xFFFF"),
        Ok(("0xFFFF", Operator::BitwiseAnd))
    );
}

#[test]
fn test_parse_operator_all_variants() {
    // Ensure all operator variants are tested
    let test_cases = [
        ("=", Operator::Equal),
        ("==", Operator::Equal),
        ("!=", Operator::NotEqual),
        ("<>", Operator::NotEqual),
        ("<", Operator::LessThan),
        (">", Operator::GreaterThan),
        ("<=", Operator::LessEqual),
        (">=", Operator::GreaterEqual),
        ("&", Operator::BitwiseAnd),
        ("^", Operator::BitwiseXor),
        ("~", Operator::BitwiseNot),
        ("x", Operator::AnyValue),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            parse_operator(input),
            Ok(("", expected)),
            "Failed to parse operator: '{input}'"
        );
    }
}

#[test]
fn test_parse_operator_less_than() {
    // Basic less-than
    assert_eq!(parse_operator("<"), Ok(("", Operator::LessThan)));

    // With whitespace
    assert_eq!(parse_operator(" < "), Ok(("", Operator::LessThan)));
    assert_eq!(parse_operator("  <  "), Ok(("", Operator::LessThan)));
    assert_eq!(parse_operator("\t<\t"), Ok(("", Operator::LessThan)));

    // With remaining input
    assert_eq!(parse_operator("< 42"), Ok(("42", Operator::LessThan)));
}

#[test]
fn test_parse_operator_greater_than() {
    // Basic greater-than
    assert_eq!(parse_operator(">"), Ok(("", Operator::GreaterThan)));

    // With whitespace
    assert_eq!(parse_operator(" > "), Ok(("", Operator::GreaterThan)));
    assert_eq!(parse_operator("  >  "), Ok(("", Operator::GreaterThan)));
    assert_eq!(parse_operator("\t>\t"), Ok(("", Operator::GreaterThan)));

    // With remaining input
    assert_eq!(parse_operator("> 42"), Ok(("42", Operator::GreaterThan)));
}

#[test]
fn test_parse_operator_less_equal() {
    // Basic less-or-equal
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));

    // With whitespace
    assert_eq!(parse_operator(" <= "), Ok(("", Operator::LessEqual)));
    assert_eq!(parse_operator("  <=  "), Ok(("", Operator::LessEqual)));
    assert_eq!(parse_operator("\t<=\t"), Ok(("", Operator::LessEqual)));

    // With remaining input
    assert_eq!(parse_operator("<= 42"), Ok(("42", Operator::LessEqual)));
}

#[test]
fn test_parse_operator_greater_equal() {
    // Basic greater-or-equal
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));

    // With whitespace
    assert_eq!(parse_operator(" >= "), Ok(("", Operator::GreaterEqual)));
    assert_eq!(parse_operator("  >=  "), Ok(("", Operator::GreaterEqual)));
    assert_eq!(parse_operator("\t>=\t"), Ok(("", Operator::GreaterEqual)));

    // With remaining input
    assert_eq!(parse_operator(">= 42"), Ok(("42", Operator::GreaterEqual)));
}

#[test]
fn test_parse_operator_comparison_disambiguation() {
    // <> still parses as NotEqual
    assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));

    // <= parses as LessEqual, not LessThan with "=" remaining
    assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));

    // >= parses as GreaterEqual, not GreaterThan with "=" remaining
    assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));

    // "< >" (with space) parses as LessThan with "> " remaining
    assert_eq!(parse_operator("< >"), Ok((">", Operator::LessThan)));

    // "> =" (with space) parses as GreaterThan with "= " remaining
    assert_eq!(parse_operator("> ="), Ok(("=", Operator::GreaterThan)));
}

// Value parsing tests
#[test]
fn test_parse_hex_bytes_with_backslash_x() {
    // Single hex byte with \x prefix
    assert_eq!(parse_hex_bytes("\\x7f"), Ok(("", vec![0x7f])));
    assert_eq!(parse_hex_bytes("\\x45"), Ok(("", vec![0x45])));
    assert_eq!(parse_hex_bytes("\\x00"), Ok(("", vec![0x00])));
    assert_eq!(parse_hex_bytes("\\xFF"), Ok(("", vec![0xFF])));

    // Multiple hex bytes with \x prefix
    assert_eq!(
        parse_hex_bytes("\\x7f\\x45\\x4c\\x46"),
        Ok(("", vec![0x7f, 0x45, 0x4c, 0x46]))
    );
    assert_eq!(
        parse_hex_bytes("\\x50\\x4b\\x03\\x04"),
        Ok(("", vec![0x50, 0x4b, 0x03, 0x04]))
    );
}

#[test]
fn test_parse_hex_bytes_without_prefix() {
    // Single hex byte without prefix (only works if it contains hex letters)
    assert_eq!(parse_hex_bytes("7f"), Ok(("", vec![0x7f])));
    assert_eq!(
        parse_hex_bytes("45"),
        Err(nom::Err::Error(NomError::new(
            "45",
            nom::error::ErrorKind::Tag
        )))
    ); // No hex letters
    assert_eq!(parse_hex_bytes("ab"), Ok(("", vec![0xab])));
    assert_eq!(parse_hex_bytes("FF"), Ok(("", vec![0xFF])));

    // Multiple hex bytes without prefix
    assert_eq!(
        parse_hex_bytes("7f454c46"),
        Ok(("", vec![0x7f, 0x45, 0x4c, 0x46]))
    );
    assert_eq!(
        parse_hex_bytes("504b0304"),
        Ok(("", vec![0x50, 0x4b, 0x03, 0x04]))
    );
}

#[test]
fn test_parse_hex_bytes_mixed_case() {
    // Test mixed case hex digits
    assert_eq!(parse_hex_bytes("aB"), Ok(("", vec![0xab])));
    assert_eq!(parse_hex_bytes("Cd"), Ok(("", vec![0xcd])));
    assert_eq!(parse_hex_bytes("\\xEf"), Ok(("", vec![0xef])));
    assert_eq!(parse_hex_bytes("\\x1A"), Ok(("", vec![0x1a])));
}

#[test]
fn test_parse_hex_bytes_empty() {
    // Empty input should return error (no hex bytes to parse)
    assert_eq!(
        parse_hex_bytes(""),
        Err(nom::Err::Error(NomError::new(
            "",
            nom::error::ErrorKind::Tag
        )))
    );
}

#[test]
fn test_parse_hex_bytes_with_remaining_input() {
    // Should parse hex bytes and leave remaining input
    assert_eq!(
        parse_hex_bytes("7f45 rest"),
        Ok((" rest", vec![0x7f, 0x45]))
    );
    assert_eq!(
        parse_hex_bytes("\\x50\\x4b next"),
        Ok((" next", vec![0x50, 0x4b]))
    );
    assert_eq!(parse_hex_bytes("ab\""), Ok(("\"", vec![0xab])));
}

#[test]
fn test_parse_escape_sequence() {
    // Standard escape sequences
    assert_eq!(parse_escape_sequence("\\n"), Ok(("", '\n')));
    assert_eq!(parse_escape_sequence("\\r"), Ok(("", '\r')));
    assert_eq!(parse_escape_sequence("\\t"), Ok(("", '\t')));
    assert_eq!(parse_escape_sequence("\\\\"), Ok(("", '\\')));
    assert_eq!(parse_escape_sequence("\\\""), Ok(("", '"')));
    assert_eq!(parse_escape_sequence("\\'"), Ok(("", '\'')));
    assert_eq!(parse_escape_sequence("\\0"), Ok(("", '\0')));
}

#[test]
fn test_parse_escape_sequence_with_remaining() {
    // Should parse escape and leave remaining input
    assert_eq!(parse_escape_sequence("\\n rest"), Ok((" rest", '\n')));
    assert_eq!(parse_escape_sequence("\\t\""), Ok(("\"", '\t')));
}

#[test]
fn test_parse_escape_sequence_invalid() {
    // Should fail on invalid escape sequences
    assert!(parse_escape_sequence("n").is_err()); // Missing backslash
    assert!(parse_escape_sequence("\\").is_err()); // Incomplete escape
    assert!(parse_escape_sequence("").is_err()); // Empty input
}

#[test]
fn test_parse_quoted_string_simple() {
    // Simple quoted strings
    assert_eq!(
        parse_quoted_string("\"hello\""),
        Ok(("", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"world\""),
        Ok(("", "world".to_string()))
    );
    assert_eq!(parse_quoted_string("\"\""), Ok(("", String::new())));
}

#[test]
fn test_parse_quoted_string_with_escapes() {
    // Strings with escape sequences
    assert_eq!(
        parse_quoted_string("\"Hello\\nWorld\""),
        Ok(("", "Hello\nWorld".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Tab\\tSeparated\""),
        Ok(("", "Tab\tSeparated".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Quote: \\\"text\\\"\""),
        Ok(("", "Quote: \"text\"".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Backslash: \\\\\""),
        Ok(("", "Backslash: \\".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"Null\\0terminated\""),
        Ok(("", "Null\0terminated".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_with_whitespace() {
    // Strings with leading/trailing whitespace
    assert_eq!(
        parse_quoted_string(" \"hello\" "),
        Ok(("", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\t\"world\"\t"),
        Ok(("", "world".to_string()))
    );
    assert_eq!(
        parse_quoted_string("  \"test\"  "),
        Ok(("", "test".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_with_remaining_input() {
    // Should parse string and leave remaining input
    assert_eq!(
        parse_quoted_string("\"hello\" world"),
        Ok(("world", "hello".to_string()))
    );
    assert_eq!(
        parse_quoted_string("\"test\" = 123"),
        Ok(("= 123", "test".to_string()))
    );
}

#[test]
fn test_parse_quoted_string_invalid() {
    // Should fail on invalid quoted strings
    assert!(parse_quoted_string("hello").is_err()); // No quotes
    assert!(parse_quoted_string("\"hello").is_err()); // Missing closing quote
    assert!(parse_quoted_string("hello\"").is_err()); // Missing opening quote
    assert!(parse_quoted_string("").is_err()); // Empty input
}

#[test]
fn test_parse_numeric_value_positive() {
    // Positive integers
    assert_eq!(parse_numeric_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_numeric_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_numeric_value("999"), Ok(("", Value::Uint(999))));

    // Positive hex values
    assert_eq!(parse_numeric_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_numeric_value("0x10"), Ok(("", Value::Uint(16))));
    assert_eq!(parse_numeric_value("0xFF"), Ok(("", Value::Uint(255))));
    assert_eq!(parse_numeric_value("0xabc"), Ok(("", Value::Uint(2748))));
}

#[test]
fn test_parse_numeric_value_negative() {
    // Negative integers
    assert_eq!(parse_numeric_value("-1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_numeric_value("-123"), Ok(("", Value::Int(-123))));
    assert_eq!(parse_numeric_value("-999"), Ok(("", Value::Int(-999))));

    // Negative hex values
    assert_eq!(parse_numeric_value("-0x1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_numeric_value("-0x10"), Ok(("", Value::Int(-16))));
    assert_eq!(parse_numeric_value("-0xFF"), Ok(("", Value::Int(-255))));
    assert_eq!(parse_numeric_value("-0xabc"), Ok(("", Value::Int(-2748))));
}

#[test]
fn test_parse_numeric_value_with_whitespace() {
    // With leading/trailing whitespace
    assert_eq!(parse_numeric_value(" 123 "), Ok(("", Value::Uint(123))));
    assert_eq!(parse_numeric_value("\t-456\t"), Ok(("", Value::Int(-456))));
    assert_eq!(parse_numeric_value("  0xFF  "), Ok(("", Value::Uint(255))));
}

#[test]
fn test_parse_numeric_value_with_remaining_input() {
    // Should parse number and leave remaining input (numeric parser consumes trailing whitespace)
    assert_eq!(
        parse_numeric_value("123 rest"),
        Ok(("rest", Value::Uint(123)))
    );
    assert_eq!(
        parse_numeric_value("-456 more"),
        Ok(("more", Value::Int(-456)))
    );
    assert_eq!(parse_numeric_value("0xFF)"), Ok((")", Value::Uint(255))));
}

#[test]
fn test_parse_numeric_value_large_unsigned_quad() {
    // Full u64 range -- values above i64::MAX required for uquad
    let test_cases = [
        // u64::MAX in hex
        ("0xffffffffffffffff", Value::Uint(u64::MAX)),
        // u64::MAX in decimal
        ("18446744073709551615", Value::Uint(u64::MAX)),
        // Exactly i64::MAX + 1 (first value that overflows i64)
        ("0x8000000000000000", Value::Uint(0x8000_0000_0000_0000)),
        // i64::MAX + 1 in decimal
        (
            "9223372036854775808",
            Value::Uint(9_223_372_036_854_775_808),
        ),
        // i64::MAX still works as Uint
        ("0x7fffffffffffffff", Value::Uint(i64::MAX as u64)),
        ("9223372036854775807", Value::Uint(i64::MAX as u64)),
        // Common magic constant patterns
        ("0xDEADBEEFDEADBEEF", Value::Uint(0xDEAD_BEEF_DEAD_BEEF)),
        ("0xCAFEBABECAFEBABE", Value::Uint(0xCAFE_BABE_CAFE_BABE)),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            parse_numeric_value(input),
            Ok(("", expected)),
            "Failed to parse large unsigned quad literal: '{input}'"
        );
    }
}

#[test]
fn test_parse_value_string_literals() {
    // String value parsing
    assert_eq!(
        parse_value("\"hello\""),
        Ok(("", Value::String("hello".to_string())))
    );
    assert_eq!(
        parse_value("\"ELF\""),
        Ok(("", Value::String("ELF".to_string())))
    );
    assert_eq!(parse_value("\"\""), Ok(("", Value::String(String::new()))));

    // String with escape sequences
    assert_eq!(
        parse_value("\"Line1\\nLine2\""),
        Ok(("", Value::String("Line1\nLine2".to_string())))
    );
    assert_eq!(
        parse_value("\"Tab\\tSeparated\""),
        Ok(("", Value::String("Tab\tSeparated".to_string())))
    );
    assert_eq!(
        parse_value("\"Null\\0Term\""),
        Ok(("", Value::String("Null\0Term".to_string())))
    );
}

#[test]
fn test_parse_value_numeric_literals() {
    // Positive integers
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("999"), Ok(("", Value::Uint(999))));

    // Negative integers
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));
    assert_eq!(parse_value("-123"), Ok(("", Value::Int(-123))));
    assert_eq!(parse_value("-999"), Ok(("", Value::Int(-999))));

    // Hexadecimal values
    assert_eq!(parse_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("0x10"), Ok(("", Value::Uint(16))));
    assert_eq!(parse_value("0xFF"), Ok(("", Value::Uint(255))));
    assert_eq!(parse_value("-0xFF"), Ok(("", Value::Int(-255))));
}

#[test]
fn test_parse_value_hex_byte_sequences() {
    // Hex bytes with \x prefix
    assert_eq!(parse_value("\\x7f"), Ok(("", Value::Bytes(vec![0x7f]))));
    assert_eq!(
        parse_value("\\x7f\\x45\\x4c\\x46"),
        Ok(("", Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])))
    );

    // Hex bytes without prefix
    assert_eq!(parse_value("7f"), Ok(("", Value::Bytes(vec![0x7f]))));
    assert_eq!(
        parse_value("7f454c46"),
        Ok(("", Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])))
    );

    // Mixed case
    assert_eq!(parse_value("aB"), Ok(("", Value::Bytes(vec![0xab]))));
    assert_eq!(parse_value("\\xCd"), Ok(("", Value::Bytes(vec![0xcd]))));
}

#[test]
fn test_parse_value_with_whitespace() {
    // All value types with whitespace - trailing whitespace is consumed by individual parsers
    assert_eq!(
        parse_value(" \"hello\" "),
        Ok(("", Value::String("hello".to_string())))
    );
    assert_eq!(parse_value("  123  "), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("\t-456\t"), Ok(("", Value::Int(-456))));
    // Floats consume trailing whitespace (consistent with integers)
    assert_eq!(parse_value("  3.125  "), Ok(("", Value::Float(3.125))));
    assert_eq!(parse_value("\t-1.0\t"), Ok(("", Value::Float(-1.0))));
    // Hex bytes don't consume trailing whitespace by themselves
    assert_eq!(
        parse_value("  \\x7f\\x45  "),
        Ok(("  ", Value::Bytes(vec![0x7f, 0x45])))
    );
}

#[test]
fn test_parse_value_with_remaining_input() {
    // Should parse value and leave remaining input
    // Note: Individual parsers handle whitespace differently
    assert_eq!(
        parse_value("\"hello\" world"),
        Ok(("world", Value::String("hello".to_string())))
    );
    assert_eq!(
        parse_value("123 rest"),
        Ok(("rest", Value::Uint(123))) // Numeric parser consumes trailing space
    );
    assert_eq!(
        parse_value("-456 more"),
        Ok(("more", Value::Int(-456))) // Numeric parser consumes trailing space
    );
    assert_eq!(
        parse_value("\\x7f\\x45 next"),
        Ok((" next", Value::Bytes(vec![0x7f, 0x45]))) // Hex bytes don't consume trailing space
    );
}

#[test]
fn test_parse_value_edge_cases() {
    // Zero values in different formats
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-0"), Ok(("", Value::Int(0))));
    assert_eq!(parse_value("0x0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-0x0"), Ok(("", Value::Int(0))));

    // Large values
    assert_eq!(
        parse_value("2147483647"),
        Ok(("", Value::Uint(2_147_483_647)))
    );
    assert_eq!(
        parse_value("-2147483648"),
        Ok(("", Value::Int(-2_147_483_648)))
    );
    assert_eq!(
        parse_value("0x7FFFFFFF"),
        Ok(("", Value::Uint(2_147_483_647)))
    );

    // Empty input should fail
    assert!(parse_value("").is_err());
}

#[test]
fn test_parse_value_invalid_input() {
    // Should fail on completely invalid input
    assert!(parse_value("xyz").is_err()); // Not a valid value format
    assert!(parse_value("0xGG").is_err()); // Invalid hex digits
    assert!(parse_value("\"unclosed").is_err()); // Unclosed string
    assert!(parse_value("--123").is_err()); // Invalid number format
}

#[test]
fn test_parse_value_common_magic_file_patterns() {
    // Test patterns commonly found in magic files
    assert_eq!(
        parse_value("0x7f454c46"),
        Ok(("", Value::Uint(0x7f45_4c46)))
    );
    assert_eq!(
        parse_value("\"ELF\""),
        Ok(("", Value::String("ELF".to_string())))
    );
    assert_eq!(
        parse_value("\\x50\\x4b\\x03\\x04"),
        Ok(("", Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04])))
    );
    assert_eq!(
        parse_value("\"\\377ELF\""),
        Ok(("", Value::String("\u{00ff}ELF".to_string())))
    );
    assert_eq!(parse_value("0"), Ok(("", Value::Uint(0))));
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));
}

#[test]
fn test_parse_value_type_precedence() {
    // Test that parsing precedence works correctly
    // Quoted strings should be parsed as strings, not hex bytes
    assert_eq!(
        parse_value("\"7f\""),
        Ok(("", Value::String("7f".to_string())))
    );

    // Hex patterns should be parsed as bytes when not quoted
    assert_eq!(parse_value("7f"), Ok(("", Value::Bytes(vec![0x7f]))));

    // Numbers should be parsed as numbers when they don't look like hex bytes
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("-123"), Ok(("", Value::Int(-123))));

    // Hex numbers with 0x prefix should be parsed as numbers
    assert_eq!(parse_value("0x123"), Ok(("", Value::Uint(0x123))));
}

#[test]
fn test_parse_value_float_literals() {
    // Positive floats
    assert_eq!(parse_value("3.125"), Ok(("", Value::Float(3.125))));
    assert_eq!(parse_value("0.5"), Ok(("", Value::Float(0.5))));
    assert_eq!(parse_value("100.0"), Ok(("", Value::Float(100.0))));

    // Negative floats
    assert_eq!(parse_value("-1.0"), Ok(("", Value::Float(-1.0))));
    assert_eq!(parse_value("-0.001"), Ok(("", Value::Float(-0.001))));

    // Scientific notation
    assert_eq!(parse_value("2.5e10"), Ok(("", Value::Float(2.5e10))));
    assert_eq!(parse_value("1.0E-3"), Ok(("", Value::Float(1.0e-3))));
    assert_eq!(parse_value("-3.0e+2"), Ok(("", Value::Float(-3.0e+2))));

    // Integers should NOT be parsed as floats
    assert_eq!(parse_value("123"), Ok(("", Value::Uint(123))));
    assert_eq!(parse_value("-456"), Ok(("", Value::Int(-456))));
    assert_eq!(parse_value("0x1a"), Ok(("", Value::Uint(26))));

    // Float with remaining input (trailing whitespace consumed, like integers)
    assert_eq!(parse_value("1.5 rest"), Ok(("rest", Value::Float(1.5))));

    // Non-finite floats (overflow to infinity) should never produce Value::Float
    // parse_value falls through to other parsers, so we check the result type
    if let Ok((_, value)) = parse_value("1.0e309") {
        assert!(
            !matches!(value, Value::Float(f) if !f.is_finite()),
            "overflow should not produce non-finite Value::Float"
        );
    }
}

#[test]
fn test_parse_value_boundary_conditions() {
    // Test boundary conditions for different value types

    // Single character strings
    assert_eq!(
        parse_value("\"a\""),
        Ok(("", Value::String("a".to_string())))
    );
    assert_eq!(
        parse_value("\"1\""),
        Ok(("", Value::String("1".to_string())))
    );

    // Single hex byte
    assert_eq!(parse_value("ab"), Ok(("", Value::Bytes(vec![0xab]))));
    assert_eq!(parse_value("\\x00"), Ok(("", Value::Bytes(vec![0x00]))));

    // Minimum and maximum values
    assert_eq!(parse_value("1"), Ok(("", Value::Uint(1))));
    assert_eq!(parse_value("-1"), Ok(("", Value::Int(-1))));

    // Powers of 2 (common in binary formats)
    assert_eq!(parse_value("256"), Ok(("", Value::Uint(256))));
    assert_eq!(parse_value("0x100"), Ok(("", Value::Uint(256))));
    assert_eq!(parse_value("1024"), Ok(("", Value::Uint(1024))));
    assert_eq!(parse_value("0x400"), Ok(("", Value::Uint(1024))));
}

#[test]
fn test_parse_operator_whitespace_handling() {
    // Test comprehensive whitespace handling
    let operators = ["=", "==", "!=", "<>", "&"];
    let whitespace_patterns = [
        "",     // No whitespace
        " ",    // Single space
        "  ",   // Multiple spaces
        "\t",   // Tab
        "\t\t", // Multiple tabs
        " \t",  // Mixed space and tab
        "\t ",  // Mixed tab and space
    ];

    for op in operators {
        for leading_ws in whitespace_patterns {
            for trailing_ws in whitespace_patterns {
                let input = format!("{leading_ws}{op}{trailing_ws}");
                let result = parse_operator(&input);

                assert!(
                    result.is_ok(),
                    "Failed to parse operator with whitespace: '{input}'"
                );

                let (remaining, _) = result.unwrap();
                assert_eq!(remaining, "", "Unexpected remaining input for: '{input}'");
            }
        }
    }
}

#[test]
fn test_parse_type_basic() {
    assert_eq!(
        parse_type("byte"),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("short"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("long"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("string"),
        Ok(("", TypeKind::String { max_length: None }))
    );
}

#[test]
fn test_parse_type_endianness() {
    assert_eq!(
        parse_type("leshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("beshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("lelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
    assert_eq!(
        parse_type("belong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true
            }
        ))
    );
}

#[test]
fn test_parse_type_with_whitespace() {
    assert_eq!(
        parse_type(" byte "),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("\tstring\t"),
        Ok(("", TypeKind::String { max_length: None }))
    );
    assert_eq!(
        parse_type("  lelong  "),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true
            }
        ))
    );
}

#[test]
fn test_parse_type_with_remaining_input() {
    assert_eq!(
        parse_type("byte ="),
        Ok(("=", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("string \\x7f"),
        Ok(("\\x7f", TypeKind::String { max_length: None }))
    );
}

#[test]
fn test_parse_type_invalid() {
    assert!(parse_type("").is_err());
    assert!(parse_type("invalid").is_err());
    assert!(parse_type("int").is_err());
}

#[test]
fn test_parse_type_unsigned_variants() {
    assert_eq!(
        parse_type("ubyte"),
        Ok(("", TypeKind::Byte { signed: false }))
    );
    assert_eq!(
        parse_type("ushort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubeshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("uleshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulelong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("uquad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ubequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: false,
            }
        ))
    );
    assert_eq!(
        parse_type("ulequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            }
        ))
    );
}

#[test]
fn test_parse_type_signed_defaults() {
    // In libmagic, unprefixed types are signed by default
    assert_eq!(
        parse_type("byte"),
        Ok(("", TypeKind::Byte { signed: true }))
    );
    assert_eq!(
        parse_type("short"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("long"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("beshort"),
        Ok((
            "",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("belong"),
        Ok((
            "",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("quad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Native,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("bequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            }
        ))
    );
    assert_eq!(
        parse_type("lequad"),
        Ok((
            "",
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            }
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

#[test]
fn test_parse_message_basic() {
    assert_eq!(
        parse_message("ELF executable"),
        Ok(("", "ELF executable".to_string()))
    );
    assert_eq!(
        parse_message("PDF document"),
        Ok(("", "PDF document".to_string()))
    );
    assert_eq!(parse_message(""), Ok(("", String::new())));
}

#[test]
fn test_parse_message_with_whitespace() {
    assert_eq!(
        parse_message("  ELF executable  "),
        Ok(("", "ELF executable".to_string()))
    );
    assert_eq!(
        parse_message("\tPDF document\t"),
        Ok(("", "PDF document".to_string()))
    );
    assert_eq!(parse_message("   "), Ok(("", String::new())));
}

#[test]
fn test_parse_message_complex() {
    assert_eq!(
        parse_message("ELF 64-bit LSB executable"),
        Ok(("", "ELF 64-bit LSB executable".to_string()))
    );
    assert_eq!(
        parse_message("ZIP archive, version %d.%d"),
        Ok(("", "ZIP archive, version %d.%d".to_string()))
    );
}

#[test]
fn test_parse_magic_rule_basic() {
    let input = "0 string \\x7fELF ELF executable";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(rule.typ, TypeKind::String { max_length: None });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]));
    assert_eq!(rule.message, "ELF executable");
    assert!(rule.children.is_empty());
}

#[test]
fn test_parse_magic_rule_child() {
    let input = ">4 byte 1 32-bit";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 1);
    assert_eq!(rule.offset, OffsetSpec::Absolute(4));
    assert_eq!(rule.typ, TypeKind::Byte { signed: true });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(1));
    assert_eq!(rule.message, "32-bit");
}

#[test]
fn test_parse_magic_rule_with_operator() {
    let input = "0 lelong&0xf0000000 0x10000000 MIPS-II";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(
        rule.typ,
        TypeKind::Long {
            endian: Endianness::Little,
            signed: true
        }
    );
    assert_eq!(rule.op, Operator::BitwiseAndMask(0xf000_0000));
    assert_eq!(rule.value, Value::Uint(0x1000_0000));
    assert_eq!(rule.message, "MIPS-II");
}

#[test]
fn test_parse_magic_rule_no_message() {
    let input = "0 byte 0x7f";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(rule.typ, TypeKind::Byte { signed: true });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(0x7f));
    assert_eq!(rule.message, "");
}

#[test]
fn test_parse_magic_rule_nested() {
    let input = ">>8 leshort 0x014c Microsoft COFF";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 2);
    assert_eq!(rule.offset, OffsetSpec::Absolute(8));
    assert_eq!(
        rule.typ,
        TypeKind::Short {
            endian: Endianness::Little,
            signed: true
        }
    );
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(0x014c));
    assert_eq!(rule.message, "Microsoft COFF");
}

#[test]
fn test_parse_magic_rule_with_whitespace() {
    let input = "  >  4   byte   =   1   32-bit  ";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 1);
    assert_eq!(rule.offset, OffsetSpec::Absolute(4));
    assert_eq!(rule.typ, TypeKind::Byte { signed: true });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(1));
    assert_eq!(rule.message, "32-bit");
}

#[test]
fn test_parse_magic_rule_string_value() {
    let input = "0 string \"PK\" ZIP archive";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(rule.typ, TypeKind::String { max_length: None });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::String("PK".to_string()));
    assert_eq!(rule.message, "ZIP archive");
}

#[test]
fn test_parse_magic_rule_hex_offset() {
    let input = "0x10 belong 0x12345678 Test data";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(16));
    assert_eq!(
        rule.typ,
        TypeKind::Long {
            endian: Endianness::Big,
            signed: true
        }
    );
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(0x1234_5678));
    assert_eq!(rule.message, "Test data");
}

#[test]
fn test_parse_magic_rule_negative_offset() {
    let input = "-4 byte 0 End marker";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(-4));
    assert_eq!(rule.typ, TypeKind::Byte { signed: true });
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::Uint(0));
    assert_eq!(rule.message, "End marker");
}

#[test]
fn test_parse_comment() {
    assert_eq!(
        parse_comment("# This is a comment"),
        Ok(("", "This is a comment".to_string()))
    );
    assert_eq!(parse_comment("#"), Ok(("", String::new())));
    assert_eq!(
        parse_comment("# ELF executables"),
        Ok(("", "ELF executables".to_string()))
    );
}

#[test]
fn test_parse_comment_with_whitespace() {
    assert_eq!(
        parse_comment("  # Indented comment  "),
        Ok(("", "Indented comment".to_string()))
    );
    assert_eq!(
        parse_comment("\t#\tTabbed comment\t"),
        Ok(("", "Tabbed comment".to_string()))
    );
}

#[test]
fn test_is_empty_line() {
    assert!(is_empty_line(""));
    assert!(is_empty_line("   "));
    assert!(is_empty_line("\t\t"));
    assert!(is_empty_line(" \t \t "));
    assert!(!is_empty_line("0 byte 1"));
    assert!(!is_empty_line("  # comment"));
}

#[test]
fn test_is_comment_line() {
    assert!(is_comment_line("# This is a comment"));
    assert!(is_comment_line("#"));
    assert!(is_comment_line("  # Indented comment"));
    assert!(is_comment_line("\t# Tabbed comment"));
    assert!(!is_comment_line("0 byte 1"));
    assert!(!is_comment_line("string test"));
}

#[test]
fn test_has_continuation() {
    assert!(has_continuation("0 string test \\"));
    assert!(has_continuation("message continues \\"));
    assert!(has_continuation("line ends with backslash\\"));
    assert!(has_continuation("  trailing whitespace  \\  "));
    assert!(!has_continuation("0 string test"));
    assert!(!has_continuation("no continuation"));
    assert!(!has_continuation("backslash in middle \\ here"));
}

#[test]
fn test_parse_magic_rule_real_world_examples() {
    // Real examples from /usr/share/file/magic/elf
    let examples = [
        "0 string \\177ELF ELF",
        ">4 byte 1 32-bit",
        ">4 byte 2 64-bit",
        ">5 byte 1 LSB",
        ">5 byte 2 MSB",
        ">>0 lelong&0xf0000000 0x10000000 MIPS-II",
    ];

    for example in examples {
        let result = parse_magic_rule(example);
        assert!(
            result.is_ok(),
            "Failed to parse real-world example: '{example}'"
        );

        let (remaining, rule) = result.unwrap();
        assert_eq!(remaining, "", "Unexpected remaining input for: '{example}'");
        assert!(
            !rule.message.is_empty() || example.contains("\\177ELF"),
            "Empty message for: '{example}'"
        );
    }
}

#[test]
fn test_parse_magic_rule_edge_cases() {
    // Test various edge cases
    let edge_cases = [
        (
            "0 byte 0",
            0,
            TypeKind::Byte { signed: true },
            Value::Uint(0),
            "",
        ),
        (
            ">>>16 string \"\" Empty string",
            3,
            TypeKind::String { max_length: None },
            Value::String(String::new()),
            "Empty string",
        ),
        (
            "0x100 lelong 0xFFFFFFFF Max value",
            0,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            Value::Uint(0xFFFF_FFFF),
            "Max value",
        ),
    ];

    for (input, expected_level, expected_type, expected_value, expected_message) in edge_cases {
        let (remaining, rule) = parse_magic_rule(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(rule.level, expected_level);
        assert_eq!(rule.typ, expected_type);
        assert_eq!(rule.value, expected_value);
        assert_eq!(rule.message, expected_message);
    }
}

#[test]
fn test_parse_magic_rule_invalid_input() {
    let invalid_inputs = [
        "",               // Empty input
        "invalid format", // No valid offset
        "0",              // Missing type
        "0 invalid_type", // Invalid type
        "0 byte",         // Missing value
    ];

    for invalid_input in invalid_inputs {
        let result = parse_magic_rule(invalid_input);
        assert!(
            result.is_err(),
            "Should fail to parse invalid input: '{invalid_input}'"
        );
    }
}

// AnyValue (`x`) operator tests -- no operand required
#[test]
fn test_parse_magic_rule_any_value_with_paren_message() {
    // Message starting with `(` must not be consumed as a value
    let input = ">0 byte x (0)";
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert!(remaining.is_empty(), "remaining: {remaining:?}");
    assert_eq!(rule.level, 1);
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.value, Value::Uint(0));
    assert_eq!(rule.message, "(0)");
}

#[test]
fn test_parse_magic_rule_any_value_with_backslash_message() {
    // Message starting with `\b,` (backspace escape) must be preserved exactly
    let input = "0 long x \\b, data";
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert!(remaining.is_empty(), "remaining: {remaining:?}");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.value, Value::Uint(0));
    assert_eq!(rule.message, "\\b, data");
}

#[test]
fn test_parse_magic_rule_any_value_no_message() {
    let input = "0 byte x";
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert!(remaining.is_empty(), "remaining: {remaining:?}");
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.value, Value::Uint(0));
    assert_eq!(rule.message, "");
}

#[test]
fn test_parse_magic_rule_any_value_plain_message() {
    let input = ">0 short x version";
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert!(remaining.is_empty(), "remaining: {remaining:?}");
    assert_eq!(rule.level, 1);
    assert_eq!(rule.op, Operator::AnyValue);
    assert_eq!(rule.message, "version");
}

// Strength directive tests
#[test]
fn test_parse_strength_directive_add() {
    assert_eq!(
        parse_strength_directive("!:strength +10"),
        Ok(("", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("!:strength +0"),
        Ok(("", StrengthModifier::Add(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength +100"),
        Ok(("", StrengthModifier::Add(100)))
    );
}

#[test]
fn test_parse_strength_directive_subtract() {
    assert_eq!(
        parse_strength_directive("!:strength -5"),
        Ok(("", StrengthModifier::Subtract(5)))
    );
    assert_eq!(
        parse_strength_directive("!:strength -0"),
        Ok(("", StrengthModifier::Subtract(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength -50"),
        Ok(("", StrengthModifier::Subtract(50)))
    );
}

#[test]
fn test_parse_strength_directive_multiply() {
    assert_eq!(
        parse_strength_directive("!:strength *2"),
        Ok(("", StrengthModifier::Multiply(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength *10"),
        Ok(("", StrengthModifier::Multiply(10)))
    );
}

#[test]
fn test_parse_strength_directive_divide() {
    assert_eq!(
        parse_strength_directive("!:strength /2"),
        Ok(("", StrengthModifier::Divide(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength /10"),
        Ok(("", StrengthModifier::Divide(10)))
    );
}

#[test]
fn test_parse_strength_directive_set_explicit() {
    assert_eq!(
        parse_strength_directive("!:strength =50"),
        Ok(("", StrengthModifier::Set(50)))
    );
    assert_eq!(
        parse_strength_directive("!:strength =0"),
        Ok(("", StrengthModifier::Set(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength =100"),
        Ok(("", StrengthModifier::Set(100)))
    );
}

#[test]
fn test_parse_strength_directive_set_bare() {
    // Bare number implies Set
    assert_eq!(
        parse_strength_directive("!:strength 50"),
        Ok(("", StrengthModifier::Set(50)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 0"),
        Ok(("", StrengthModifier::Set(0)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 100"),
        Ok(("", StrengthModifier::Set(100)))
    );
}

#[test]
fn test_parse_strength_directive_with_whitespace() {
    assert_eq!(
        parse_strength_directive("  !:strength +10"),
        Ok(("", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("\t!:strength -5"),
        Ok(("", StrengthModifier::Subtract(5)))
    );
    assert_eq!(
        parse_strength_directive("!:strength  *2"),
        Ok(("", StrengthModifier::Multiply(2)))
    );
    assert_eq!(
        parse_strength_directive("!:strength   50"),
        Ok(("", StrengthModifier::Set(50)))
    );
}

#[test]
fn test_parse_strength_directive_with_remaining_input() {
    // Should leave remaining content after the directive
    assert_eq!(
        parse_strength_directive("!:strength +10 extra"),
        Ok((" extra", StrengthModifier::Add(10)))
    );
    assert_eq!(
        parse_strength_directive("!:strength 50\n"),
        Ok(("\n", StrengthModifier::Set(50)))
    );
}

#[test]
fn test_parse_strength_directive_invalid() {
    // Should fail on invalid input
    assert!(parse_strength_directive("").is_err());
    assert!(parse_strength_directive("!:invalid").is_err());
    assert!(parse_strength_directive("strength +10").is_err());
    assert!(parse_strength_directive("0 byte 1").is_err());
}

#[test]
fn test_is_strength_directive() {
    assert!(is_strength_directive("!:strength +10"));
    assert!(is_strength_directive("!:strength -5"));
    assert!(is_strength_directive("!:strength 50"));
    assert!(is_strength_directive("  !:strength +10"));
    assert!(is_strength_directive("\t!:strength *2"));

    assert!(!is_strength_directive("0 byte 1"));
    assert!(!is_strength_directive("# comment"));
    assert!(!is_strength_directive(""));
    assert!(!is_strength_directive("!:mime application/pdf"));
}

// PString type tests
#[test]
fn test_parse_type_pstring() {
    assert_eq!(
        parse_type("pstring"),
        Ok((
            "",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
}

#[test]
fn test_parse_type_pstring_with_remaining_input() {
    assert_eq!(
        parse_type("pstring ="),
        Ok((
            "=",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
    assert_eq!(
        parse_type("pstring \"hello\""),
        Ok((
            "\"hello\"",
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false
            }
        ))
    );
}

#[test]
fn test_parse_magic_rule_pstring() {
    let input = "0 pstring \"PascalStr\" Pascal string data";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 0);
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(
        rule.typ,
        TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false
        }
    );
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::String("PascalStr".to_string()));
    assert_eq!(rule.message, "Pascal string data");
}

#[test]
fn test_parse_magic_rule_pstring_child_rule() {
    let input = ">4 pstring \"test\" child pstring rule";
    let (remaining, rule) = parse_magic_rule(input).unwrap();

    assert_eq!(remaining, "");
    assert_eq!(rule.level, 1);
    assert_eq!(rule.offset, OffsetSpec::Absolute(4));
    assert_eq!(
        rule.typ,
        TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false
        }
    );
    assert_eq!(rule.message, "child pstring rule");
}

#[test]
fn test_parse_type_and_operator_pstring_standalone_and() {
    let (remaining, (typ, op)) = parse_type_and_operator("pstring& ").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(
        typ,
        TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false
        }
    );
    assert_eq!(op, Some(Operator::BitwiseAnd));
}

#[test]
fn test_parse_type_and_operator_quad_full_width_mask() {
    // Full u64 mask (0xffffffffffffffff) must parse successfully, not silently
    // fall back to standalone '&' leaving the mask as leftover input.
    let (remaining, (typ, op)) = parse_type_and_operator("uquad&0xffffffffffffffff").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(
        typ,
        TypeKind::Quad {
            endian: Endianness::Native,
            signed: false,
        }
    );
    assert_eq!(op, Some(Operator::BitwiseAndMask(u64::MAX)));
}

#[test]
fn test_parse_type_and_operator_quad_mask_various() {
    // Hex mask within i64 range
    let (remaining, (_, op)) = parse_type_and_operator("quad&0x7fffffffffffffff").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAndMask(i64::MAX as u64)));

    // Decimal mask
    let (remaining, (_, op)) = parse_type_and_operator("uquad&255").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAndMask(255)));

    // Standalone '&' (no digits following) still works
    let (remaining, (_, op)) = parse_type_and_operator("uquad& ").unwrap();
    assert_eq!(remaining, "");
    assert_eq!(op, Some(Operator::BitwiseAnd));
}

#[test]
fn test_parse_type_and_operator_mask_overflow_fails() {
    // Decimal value exceeding u64::MAX must fail, not silently reinterpret
    let result = parse_type_and_operator("uquad&99999999999999999999");
    assert!(
        result.is_err(),
        "overflowing mask should produce a parse error"
    );

    // Hex value exceeding u64 (17 hex digits) must fail
    let result = parse_type_and_operator("uquad&0x1ffffffffffffffff");
    assert!(
        result.is_err(),
        "overflowing hex mask should produce a parse error"
    );
}

#[test]
fn test_parse_type_and_operator_pstring_suffixes() {
    use crate::parser::ast::TypeKind;
    let cases: &[(&str, PStringLengthWidth, bool, &str)] = &[
        ("pstring", PStringLengthWidth::OneByte, false, ""),
        ("pstring/B", PStringLengthWidth::OneByte, false, ""),
        ("pstring/H", PStringLengthWidth::TwoByteBE, false, ""),
        ("pstring/h", PStringLengthWidth::TwoByteLE, false, ""),
        ("pstring/L", PStringLengthWidth::FourByteBE, false, ""),
        ("pstring/l", PStringLengthWidth::FourByteLE, false, ""),
        ("pstring/H =", PStringLengthWidth::TwoByteBE, false, "="),
        ("pstring/J", PStringLengthWidth::OneByte, true, ""),
        ("pstring/BJ", PStringLengthWidth::OneByte, true, ""),
        ("pstring/HJ", PStringLengthWidth::TwoByteBE, true, ""),
        ("pstring/hJ", PStringLengthWidth::TwoByteLE, true, ""),
        ("pstring/LJ", PStringLengthWidth::FourByteBE, true, ""),
        ("pstring/lJ", PStringLengthWidth::FourByteLE, true, ""),
    ];
    for &(input, expected_width, expected_j, expected_rest) in cases {
        let (rest, (kind, op)) = parse_type_and_operator(input).expect(input);
        assert_eq!(rest, expected_rest, "rest for input: {input}");
        assert!(op.is_none(), "operator for input: {input}");
        match kind {
            TypeKind::PString {
                max_length,
                length_width,
                length_includes_itself,
            } => {
                assert_eq!(max_length, None, "max_length for input: {input}");
                assert_eq!(
                    length_width, expected_width,
                    "length_width for input: {input}"
                );
                assert_eq!(
                    length_includes_itself, expected_j,
                    "length_includes_itself for input: {input}"
                );
            }
            _ => panic!("Expected PString for input: {input}, got {kind:?}"),
        }
    }
}
