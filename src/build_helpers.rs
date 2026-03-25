// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

/// Build-time helpers for compiling magic rules.
///
/// This module contains functionality used by the build script to parse magic files
/// and generate Rust code for built-in rules. It is extracted into a library module
/// to enable comprehensive testing of the build process, including error cases.
///
/// Serialization logic is provided by [`crate::parser::codegen`], which is shared
/// with `build.rs` to avoid duplication.
use crate::error::ParseError;
use crate::parser::parse_text_magic_file;

// Re-export codegen functions used by tests
#[cfg(test)]
use crate::parser::codegen::{
    format_byte_vec, format_number, generate_builtin_rules, serialize_children,
    serialize_endianness, serialize_offset_spec, serialize_operator, serialize_type_kind,
    serialize_value,
};

/// Parses a magic file and generates Rust code for the built-in rules.
///
/// This function wraps the parsing and code generation steps, providing a testable
/// interface for the build script logic.
///
/// # Errors
///
/// Returns a `ParseError` if the magic file content is invalid or malformed.
pub fn parse_and_generate_builtin_rules(magic_content: &str) -> Result<String, ParseError> {
    let rules = parse_text_magic_file(magic_content)?;
    Ok(crate::parser::codegen::generate_builtin_rules(&rules))
}

/// Formats a parse error for display in build script output.
///
/// This function converts a `ParseError` into a human-readable message suitable
/// for display when the build script fails.
#[must_use]
pub fn format_parse_error(error: &ParseError) -> String {
    match error {
        ParseError::InvalidSyntax { line, message } => {
            format!("Error parsing builtin_rules.magic at line {line}: {message}")
        }
        ParseError::UnsupportedFeature { line, feature } => {
            format!("Error parsing builtin_rules.magic at line {line}: {feature}")
        }
        ParseError::InvalidOffset { line, offset } => {
            format!("Error parsing builtin_rules.magic at line {line}: {offset}")
        }
        ParseError::InvalidType { line, type_spec } => {
            format!("Error parsing builtin_rules.magic at line {line}: {type_spec}")
        }
        ParseError::InvalidOperator { line, operator } => {
            format!("Error parsing builtin_rules.magic at line {line}: {operator}")
        }
        ParseError::InvalidValue { line, value } => {
            format!("Error parsing builtin_rules.magic at line {line}: {value}")
        }
        ParseError::UnsupportedFormat {
            line,
            format_type,
            message,
        } => format!("Error parsing builtin_rules.magic at line {line}: {format_type} {message}"),
        ParseError::IoError(err) => {
            format!("Error parsing builtin_rules.magic: I/O error: {err}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};
    use crate::parser::codegen::format_string_literal;

    #[test]
    fn test_format_parse_error_invalid_syntax() {
        let error = ParseError::InvalidSyntax {
            line: 42,
            message: "expected offset".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 42"));
        assert!(formatted.contains("expected offset"));
        assert!(formatted.contains("builtin_rules.magic"));
    }

    #[test]
    fn test_format_parse_error_unsupported_feature() {
        let error = ParseError::UnsupportedFeature {
            line: 10,
            feature: "regex patterns".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 10"));
        assert!(formatted.contains("regex patterns"));
    }

    #[test]
    fn test_format_parse_error_invalid_offset() {
        let error = ParseError::InvalidOffset {
            line: 5,
            offset: "invalid offset spec".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 5"));
        assert!(formatted.contains("invalid offset spec"));
    }

    #[test]
    fn test_format_parse_error_invalid_type() {
        let error = ParseError::InvalidType {
            line: 7,
            type_spec: "unknown type".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 7"));
        assert!(formatted.contains("unknown type"));
    }

    #[test]
    fn test_format_parse_error_invalid_operator() {
        let error = ParseError::InvalidOperator {
            line: 12,
            operator: "bad operator".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 12"));
        assert!(formatted.contains("bad operator"));
    }

    #[test]
    fn test_format_parse_error_invalid_value() {
        let error = ParseError::InvalidValue {
            line: 15,
            value: "malformed value".to_string(),
        };
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("line 15"));
        assert!(formatted.contains("malformed value"));
    }

    #[test]
    fn test_serialize_offset_spec_absolute() {
        let offset = OffsetSpec::Absolute(42);
        let serialized = serialize_offset_spec(&offset);
        assert_eq!(serialized, "OffsetSpec::Absolute(42)");
    }

    #[test]
    fn test_serialize_offset_spec_relative() {
        let offset = OffsetSpec::Relative(-10);
        let serialized = serialize_offset_spec(&offset);
        assert_eq!(serialized, "OffsetSpec::Relative(-10)");
    }

    #[test]
    fn test_serialize_offset_spec_from_end() {
        let offset = OffsetSpec::FromEnd(-16);
        let serialized = serialize_offset_spec(&offset);
        assert_eq!(serialized, "OffsetSpec::FromEnd(-16)");
    }

    #[test]
    fn test_serialize_type_kind_byte() {
        let signed = TypeKind::Byte { signed: true };
        assert_eq!(
            serialize_type_kind(&signed),
            "TypeKind::Byte { signed: true }"
        );
        let unsigned = TypeKind::Byte { signed: false };
        assert_eq!(
            serialize_type_kind(&unsigned),
            "TypeKind::Byte { signed: false }"
        );
    }

    #[test]
    fn test_serialize_type_kind_short() {
        let typ = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let serialized = serialize_type_kind(&typ);
        assert!(serialized.contains("TypeKind::Short"));
        assert!(serialized.contains("Endianness::Little"));
        assert!(serialized.contains("signed: false"));
    }

    #[test]
    fn test_serialize_type_kind_long() {
        let typ = TypeKind::Long {
            endian: Endianness::Big,
            signed: true,
        };
        let serialized = serialize_type_kind(&typ);
        assert!(serialized.contains("TypeKind::Long"));
        assert!(serialized.contains("Endianness::Big"));
        assert!(serialized.contains("signed: true"));
    }

    #[test]
    fn test_serialize_type_kind_quad() {
        let typ = TypeKind::Quad {
            endian: Endianness::Little,
            signed: true,
        };
        let serialized = serialize_type_kind(&typ);
        assert!(serialized.contains("TypeKind::Quad"));
        assert!(serialized.contains("Endianness::Little"));
        assert!(serialized.contains("signed: true"));

        let typ2 = TypeKind::Quad {
            endian: Endianness::Big,
            signed: false,
        };
        let serialized2 = serialize_type_kind(&typ2);
        assert!(serialized2.contains("TypeKind::Quad"));
        assert!(serialized2.contains("Endianness::Big"));
        assert!(serialized2.contains("signed: false"));
    }

    #[test]
    fn test_serialize_type_kind_float() {
        let cases = [
            (
                TypeKind::Float {
                    endian: Endianness::Native,
                },
                "TypeKind::Float { endian: Endianness::Native }",
            ),
            (
                TypeKind::Float {
                    endian: Endianness::Little,
                },
                "TypeKind::Float { endian: Endianness::Little }",
            ),
            (
                TypeKind::Float {
                    endian: Endianness::Big,
                },
                "TypeKind::Float { endian: Endianness::Big }",
            ),
        ];
        for (typ, expected) in &cases {
            assert_eq!(serialize_type_kind(typ), *expected);
        }
    }

    #[test]
    fn test_serialize_type_kind_double() {
        let cases = [
            (
                TypeKind::Double {
                    endian: Endianness::Native,
                },
                "TypeKind::Double { endian: Endianness::Native }",
            ),
            (
                TypeKind::Double {
                    endian: Endianness::Little,
                },
                "TypeKind::Double { endian: Endianness::Little }",
            ),
            (
                TypeKind::Double {
                    endian: Endianness::Big,
                },
                "TypeKind::Double { endian: Endianness::Big }",
            ),
        ];
        for (typ, expected) in &cases {
            assert_eq!(serialize_type_kind(typ), *expected);
        }
    }

    #[test]
    fn test_serialize_value_float() {
        // Positive finite literal
        let serialized = serialize_value(&Value::Float(3.125));
        assert_eq!(serialized, "Value::Float(3.125)");

        // Negative finite literal
        let serialized = serialize_value(&Value::Float(-1.0));
        assert_eq!(serialized, "Value::Float(-1.0)");

        // Non-finite values produce valid Rust expressions
        assert_eq!(
            serialize_value(&Value::Float(f64::NAN)),
            "Value::Float(f64::NAN)"
        );
        assert_eq!(
            serialize_value(&Value::Float(f64::INFINITY)),
            "Value::Float(f64::INFINITY)"
        );
        assert_eq!(
            serialize_value(&Value::Float(f64::NEG_INFINITY)),
            "Value::Float(f64::NEG_INFINITY)"
        );
    }

    #[test]
    fn test_serialize_type_kind_string() {
        let typ1 = TypeKind::String { max_length: None };
        let serialized1 = serialize_type_kind(&typ1);
        assert_eq!(serialized1, "TypeKind::String { max_length: None }");

        let typ2 = TypeKind::String {
            max_length: Some(256),
        };
        let serialized2 = serialize_type_kind(&typ2);
        assert_eq!(serialized2, "TypeKind::String { max_length: Some(256) }");
    }

    #[test]
    fn test_serialize_type_kind_pstring() {
        use crate::parser::ast::PStringLengthWidth;
        let typ1 = TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        };
        let serialized1 = serialize_type_kind(&typ1);
        assert_eq!(
            serialized1,
            "TypeKind::PString { max_length: None, length_width: PStringLengthWidth::OneByte, length_includes_itself: false }"
        );

        let typ2 = TypeKind::PString {
            max_length: Some(128),
            length_width: PStringLengthWidth::FourByteLE,
            length_includes_itself: false,
        };
        let serialized2 = serialize_type_kind(&typ2);
        assert_eq!(
            serialized2,
            "TypeKind::PString { max_length: Some(128), length_width: PStringLengthWidth::FourByteLE, length_includes_itself: false }"
        );
    }

    #[test]
    fn test_serialize_operator() {
        assert_eq!(serialize_operator(&Operator::Equal), "Operator::Equal");
        assert_eq!(
            serialize_operator(&Operator::NotEqual),
            "Operator::NotEqual"
        );
        assert_eq!(
            serialize_operator(&Operator::LessThan),
            "Operator::LessThan"
        );
        assert_eq!(
            serialize_operator(&Operator::GreaterThan),
            "Operator::GreaterThan"
        );
        assert_eq!(
            serialize_operator(&Operator::LessEqual),
            "Operator::LessEqual"
        );
        assert_eq!(
            serialize_operator(&Operator::GreaterEqual),
            "Operator::GreaterEqual"
        );
        assert_eq!(
            serialize_operator(&Operator::BitwiseAnd),
            "Operator::BitwiseAnd"
        );
        assert_eq!(
            serialize_operator(&Operator::BitwiseAndMask(0xFF)),
            "Operator::BitwiseAndMask(255)"
        );
    }

    #[test]
    fn test_serialize_value_uint() {
        let value = Value::Uint(12345);
        let serialized = serialize_value(&value);
        assert_eq!(serialized, "Value::Uint(12_345)");
    }

    #[test]
    fn test_serialize_value_int() {
        let value = Value::Int(-100);
        let serialized = serialize_value(&value);
        assert!(serialized.contains("Value::Int"));
    }

    #[test]
    fn test_serialize_value_bytes() {
        let value = Value::Bytes(vec![0x7F, 0x45, 0x4C, 0x46]);
        let serialized = serialize_value(&value);
        assert_eq!(serialized, "Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])");
    }

    #[test]
    fn test_serialize_value_string() {
        let value = Value::String("test".to_string());
        let serialized = serialize_value(&value);
        assert!(serialized.contains("Value::String"));
        assert!(serialized.contains("test"));
    }

    #[test]
    fn test_format_number_small() {
        assert_eq!(format_number(42), "42");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(9999), "9999");
    }

    #[test]
    fn test_format_number_large() {
        assert_eq!(format_number(10000), "10_000");
        assert_eq!(format_number(123_456), "123_456");
        assert_eq!(format_number(1_234_567_890), "1_234_567_890");
    }

    #[test]
    fn test_serialize_endianness() {
        assert_eq!(
            serialize_endianness(Endianness::Little),
            "Endianness::Little"
        );
        assert_eq!(serialize_endianness(Endianness::Big), "Endianness::Big");
        assert_eq!(
            serialize_endianness(Endianness::Native),
            "Endianness::Native"
        );
    }

    #[test]
    fn test_format_byte_vec_empty() {
        let result = format_byte_vec(&[]);
        assert_eq!(result, "vec![]");
    }

    #[test]
    fn test_format_byte_vec_single() {
        let result = format_byte_vec(&[0x42]);
        assert_eq!(result, "vec![0x42]");
    }

    #[test]
    fn test_format_byte_vec_multiple() {
        let result = format_byte_vec(&[0x12, 0x34, 0x56]);
        assert_eq!(result, "vec![0x12, 0x34, 0x56]");
    }

    #[test]
    fn test_format_string_literal() {
        assert_eq!(format_string_literal("hello"), "\"hello\"");
        assert_eq!(format_string_literal("test\n"), "\"test\\n\"");
        assert_eq!(format_string_literal("quote\"here"), "\"quote\\\"here\"");
    }

    #[test]
    fn test_generate_builtin_rules_empty() {
        let rules: Vec<MagicRule> = vec![];
        let generated = generate_builtin_rules(&rules);

        assert!(generated.contains("LazyLock<Vec<MagicRule>>"));
        assert!(generated.contains("vec![]") || generated.contains("vec!["));
        assert!(generated.contains("use crate::parser::ast"));
        assert!(generated.contains("use std::sync::LazyLock"));
    }

    #[test]
    fn test_generate_builtin_rules_single_rule() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7F),
            message: "test".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };

        let generated = generate_builtin_rules(&[rule]);

        assert!(generated.contains("OffsetSpec::Absolute(0)"));
        assert!(generated.contains("TypeKind::Byte { signed: true }"));
        assert!(generated.contains("Operator::Equal"));
        assert!(generated.contains("Value::Uint(127)"));
        assert!(generated.contains("test"));
        assert!(generated.contains("level: 0"));
    }

    #[test]
    fn test_serialize_children_empty() {
        let result = serialize_children(&[], 4);
        assert_eq!(result, "Vec::new()");
    }

    #[test]
    fn test_serialize_children_with_nested_rule() {
        let child = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "child".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
        };

        let result = serialize_children(&[child], 4);

        assert!(result.contains("vec!["));
        assert!(result.contains("OffsetSpec::Absolute(4)"));
        assert!(result.contains("level: 1"));
        assert!(result.contains("child"));
    }

    // Tests for invalid magic file parsing failure path
    #[test]
    fn test_parse_and_generate_invalid_syntax() {
        let invalid_magic = "this is not valid magic syntax";
        let result = parse_and_generate_builtin_rules(invalid_magic);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("builtin_rules.magic"));
    }

    #[test]
    fn test_parse_and_generate_invalid_offset() {
        let invalid_magic = "999999999999999999999 byte =0x7F ELF";
        let result = parse_and_generate_builtin_rules(invalid_magic);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("builtin_rules.magic"));
    }

    #[test]
    fn test_parse_and_generate_invalid_type() {
        let invalid_magic = "0 invalidtype =0x7F test";
        let result = parse_and_generate_builtin_rules(invalid_magic);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("builtin_rules.magic"));
    }

    #[test]
    fn test_parse_and_generate_empty_input() {
        let empty_magic = "";
        let result = parse_and_generate_builtin_rules(empty_magic);

        // Empty input should succeed with no rules
        assert!(result.is_ok());
        let generated = result.unwrap();
        assert!(generated.contains("vec![]") || generated.contains("vec!["));
    }

    #[test]
    fn test_parse_and_generate_valid_magic() {
        let valid_magic = "0 byte =0x7F ELF executable";
        let result = parse_and_generate_builtin_rules(valid_magic);

        assert!(result.is_ok());
        let generated = result.unwrap();
        assert!(generated.contains("OffsetSpec::Absolute(0)"));
        assert!(generated.contains("TypeKind::Byte { signed: true }"));
        assert!(generated.contains("Value::Uint(127)"));
        assert!(generated.contains("ELF executable"));
    }

    #[test]
    fn test_parse_and_generate_malformed_value() {
        let invalid_magic = "0 byte =notahexvalue test";
        let result = parse_and_generate_builtin_rules(invalid_magic);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let formatted = format_parse_error(&error);
        assert!(formatted.contains("builtin_rules.magic"));
    }
}
