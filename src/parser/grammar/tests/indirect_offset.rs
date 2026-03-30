// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Indirect offset parsing tests
//
// GNU `file` semantics: lowercase = little-endian, uppercase = big-endian.
// Numeric pointer types are signed by default (GOTCHAS S6.3).
// Adjustment is parsed AFTER the closing `)`: (base.type)+adj

#[test]
fn test_parse_offset_indirect_all_specifiers() {
    // Table-driven: (input, expected_pointer_type, expected_endian)
    let cases: &[(&str, TypeKind, Endianness)] = &[
        // .b / .B - byte (little-endian, signed)
        ("(0.b)", TypeKind::Byte { signed: true }, Endianness::Little),
        ("(0.B)", TypeKind::Byte { signed: true }, Endianness::Big),
        // .s - short little-endian, .S - short big-endian
        (
            "(0.s)",
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        ),
        (
            "(0.S)",
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        ),
        // .l - long little-endian, .L - long big-endian
        (
            "(0x3c.l)",
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        ),
        (
            "(0x3c.L)",
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        ),
        // .q - quad little-endian, .Q - quad big-endian
        (
            "(0.q)",
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        ),
        (
            "(0.Q)",
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        ),
    ];

    for (input, expected_type, expected_endian) in cases {
        let base = if input.contains("0x3c") { 0x3c } else { 0 };
        assert_eq!(
            parse_offset(input),
            Ok((
                "",
                OffsetSpec::Indirect {
                    base_offset: base,
                    pointer_type: expected_type.clone(),
                    adjustment: 0,
                    endian: *expected_endian,
                }
            )),
            "Failed for input: {input}"
        );
    }
}

#[test]
fn test_parse_offset_indirect_with_positive_adjustment() {
    // Adjustment AFTER closing paren: (base.type)+adj
    assert_eq!(
        parse_offset("(0x3c.l)+4"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 4,
                endian: Endianness::Little,
            }
        ))
    );
    assert_eq!(
        parse_offset("(0.b)+0xFF"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0,
                pointer_type: TypeKind::Byte { signed: true },
                adjustment: 255,
                endian: Endianness::Little,
            }
        ))
    );
}

#[test]
fn test_parse_offset_indirect_with_negative_adjustment() {
    assert_eq!(
        parse_offset("(0x3c.l)-8"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: -8,
                endian: Endianness::Little,
            }
        ))
    );
    assert_eq!(
        parse_offset("(100.s)-0x10"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: 100,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: -16,
                endian: Endianness::Little,
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
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 0,
                endian: Endianness::Little,
            }
        ))
    );
    // Negative base with adjustment after paren
    assert_eq!(
        parse_offset("(-0x10.s)+2"),
        Ok((
            "",
            OffsetSpec::Indirect {
                base_offset: -16,
                pointer_type: TypeKind::Short {
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 2,
                endian: Endianness::Little,
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
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 0,
                endian: Endianness::Little,
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
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 0,
                endian: Endianness::Little,
            }
        ))
    );
    // Trailing content after adjustment-free form
    assert_eq!(
        parse_offset("(0x3c.l) string"),
        Ok((
            "string",
            OffsetSpec::Indirect {
                base_offset: 0x3c,
                pointer_type: TypeKind::Long {
                    endian: Endianness::Little,
                    signed: true
                },
                adjustment: 0,
                endian: Endianness::Little,
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
                        endian: Endianness::Little,
                        signed: true
                    },
                    adjustment: 0,
                    endian: Endianness::Little,
                }
            )
        ))
    );
}
