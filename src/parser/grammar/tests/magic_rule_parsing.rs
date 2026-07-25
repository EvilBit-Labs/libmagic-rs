// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! `parse_magic_rule` and `parse_message` tests.
//!
//! Covers full magic-rule-line parsing: messages, child rules,
//! operators, string/pstring/regex/search values, hex/negative
//! offsets, `x` (any-value) message handling, and real-world
//! magic-file examples.

use super::*;

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
    assert_eq!(
        rule.typ,
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default()
        }
    );
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
    assert_eq!(
        rule.typ,
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default()
        }
    );
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
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
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
fn test_parse_magic_rule_regex_and_search() {
    use crate::parser::ast::{RegexCount, RegexFlags};
    use std::num::{NonZeroU32, NonZeroUsize};

    // regex/c: case-insensitive flag
    let input = r#"0 regex/c "hello" case-insensitive match"#;
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(remaining, "");
    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(
        rule.typ,
        TypeKind::Regex {
            flags: RegexFlags {
                case_insensitive: true,
                ..RegexFlags::default()
            },
            count: RegexCount::Default,
        }
    );
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::String("hello".to_string()));
    assert_eq!(rule.message, "case-insensitive match");

    // search/256
    let input = r#"0 search/256 "MZ" DOS executable"#;
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(remaining, "");
    assert_eq!(
        rule.typ,
        TypeKind::Search {
            range: NonZeroUsize::new(256),
            flags: SearchFlags::default(),
        }
    );
    assert_eq!(rule.op, Operator::Equal);
    assert_eq!(rule.value, Value::String("MZ".to_string()));
    assert_eq!(rule.message, "DOS executable");

    // regex/1l: line-based with a count of 1 (mirrors regex-eol.magic
    // syntax). The count is now preserved, not discarded.
    let input = r#">1 regex/1l "[0-9]+" version line"#;
    let (remaining, rule) = parse_magic_rule(input).unwrap();
    assert_eq!(remaining, "");
    assert_eq!(rule.level, 1);
    assert_eq!(
        rule.typ,
        TypeKind::Regex {
            flags: RegexFlags::default(),
            count: RegexCount::Lines(NonZeroU32::new(1)),
        }
    );
    assert_eq!(rule.message, "version line");
}
