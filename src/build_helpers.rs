/// Build-time helpers for compiling magic rules.
///
/// This module contains functionality used by the build script to parse magic files
/// and generate Rust code for built-in rules. It is extracted into a library module
/// to enable comprehensive testing of the build process, including error cases.
use crate::error::ParseError;
use crate::parser::ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};
use crate::parser::parse_text_magic_file;

const INDENT_WIDTH: usize = 4;

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
    Ok(generate_builtin_rules(&rules))
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

fn generate_builtin_rules(rules: &[MagicRule]) -> String {
    let mut output = String::new();

    push_line(
        &mut output,
        "use crate::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value, Endianness};",
    );
    push_line(&mut output, "use std::sync::LazyLock;");
    push_line(&mut output, "");
    push_line(
        &mut output,
        "/// Built-in magic rules compiled at build time.",
    );
    push_line(&mut output, "///");
    push_line(
        &mut output,
        "/// This static contains magic rules parsed from `src/builtin_rules.magic` during",
    );
    push_line(
        &mut output,
        "/// the build process. The rules are lazily initialized on first access.",
    );
    push_line(&mut output, "///");
    push_line(
        &mut output,
        "/// Use [`get_builtin_rules()`] to access these rules instead of using this static directly.",
    );
    push_line(
        &mut output,
        "pub static BUILTIN_RULES: LazyLock<Vec<MagicRule>> = LazyLock::new(|| {",
    );
    push_line(&mut output, "    vec![");

    for rule in rules {
        let serialized = serialize_magic_rule(rule, INDENT_WIDTH * 2);
        output.push_str(&serialized);
        output.push(',');
        output.push('\n');
    }

    push_line(&mut output, "    ]");
    push_line(&mut output, "});\n");
    output
}

fn serialize_magic_rule(rule: &MagicRule, indent: usize) -> String {
    let mut output = String::new();

    push_indent(&mut output, indent);
    output.push_str("MagicRule {\n");

    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "offset",
        &serialize_offset_spec(&rule.offset),
    );
    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "typ",
        &serialize_type_kind(&rule.typ),
    );
    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "op",
        &serialize_operator(&rule.op),
    );
    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "value",
        &serialize_value(&rule.value),
    );
    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "message",
        &format!("String::from({})", format_string_literal(&rule.message)),
    );

    push_indent(&mut output, indent + INDENT_WIDTH);
    output.push_str("children: ");
    output.push_str(&serialize_children(&rule.children, indent + INDENT_WIDTH));
    output.push_str(",\n");

    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "level",
        &rule.level.to_string(),
    );

    push_indent(&mut output, indent);
    output.push('}');

    output
}

fn serialize_children(children: &[MagicRule], indent: usize) -> String {
    if children.is_empty() {
        return "Vec::new()".to_string();
    }

    let mut output = String::new();
    output.push_str("vec![\n");

    for child in children {
        let serialized = serialize_magic_rule(child, indent + INDENT_WIDTH);
        output.push_str(&serialized);
        output.push_str(",\n");
    }

    push_indent(&mut output, indent);
    output.push(']');
    output
}

fn serialize_offset_spec(offset: &OffsetSpec) -> String {
    match offset {
        OffsetSpec::Absolute(value) => format!("OffsetSpec::Absolute({value})"),
        OffsetSpec::Indirect {
            base_offset,
            pointer_type,
            adjustment,
            endian,
        } => format!(
            "OffsetSpec::Indirect {{ base_offset: {base_offset}, pointer_type: {}, adjustment: {adjustment}, endian: {} }}",
            serialize_type_kind(pointer_type),
            serialize_endianness(*endian)
        ),
        OffsetSpec::Relative(value) => format!("OffsetSpec::Relative({value})"),
        OffsetSpec::FromEnd(value) => format!("OffsetSpec::FromEnd({value})"),
    }
}

fn serialize_type_kind(typ: &TypeKind) -> String {
    match typ {
        TypeKind::Byte => "TypeKind::Byte".to_string(),
        TypeKind::Short { endian, signed } => format!(
            "TypeKind::Short {{ endian: {}, signed: {} }}",
            serialize_endianness(*endian),
            signed
        ),
        TypeKind::Long { endian, signed } => format!(
            "TypeKind::Long {{ endian: {}, signed: {} }}",
            serialize_endianness(*endian),
            signed
        ),
        TypeKind::String { max_length } => match max_length {
            Some(value) => {
                format!("TypeKind::String {{ max_length: Some({value}) }}")
            }
            None => "TypeKind::String { max_length: None }".to_string(),
        },
    }
}

fn serialize_operator(op: &Operator) -> String {
    match op {
        Operator::Equal => "Operator::Equal".to_string(),
        Operator::NotEqual => "Operator::NotEqual".to_string(),
        Operator::BitwiseAnd => "Operator::BitwiseAnd".to_string(),
        Operator::BitwiseAndMask(mask) => format!("Operator::BitwiseAndMask({mask})"),
    }
}

fn serialize_value(value: &Value) -> String {
    match value {
        Value::Uint(number) => format!("Value::Uint({})", format_number(*number)),
        Value::Int(number) => format!("Value::Int({number})"),
        Value::Bytes(bytes) => format!("Value::Bytes({})", format_byte_vec(bytes)),
        Value::String(text) => format!(
            "Value::String(String::from({}))",
            format_string_literal(text)
        ),
    }
}

/// Format a number with underscores for readability (`clippy::unreadable_literal`)
fn format_number(num: u64) -> String {
    if num < 10000 {
        num.to_string()
    } else {
        let num_str = num.to_string();
        let mut result = String::new();
        let len = num_str.len();

        for (i, ch) in num_str.chars().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push('_');
            }
            result.push(ch);
        }
        result
    }
}

fn serialize_endianness(endian: Endianness) -> String {
    match endian {
        Endianness::Little => "Endianness::Little".to_string(),
        Endianness::Big => "Endianness::Big".to_string(),
        Endianness::Native => "Endianness::Native".to_string(),
    }
}

fn format_byte_vec(bytes: &[u8]) -> String {
    use std::fmt::Write;

    if bytes.is_empty() {
        return "vec![]".to_string();
    }

    let mut output = String::from("vec![");
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(output, "0x{byte:02x}").unwrap();
    }
    output.push(']');
    output
}

fn format_string_literal(value: &str) -> String {
    let escaped = value.escape_default().to_string();
    format!("\"{escaped}\"")
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push(' ');
    }
}

fn push_field(output: &mut String, indent: usize, name: &str, value: &str) {
    push_indent(output, indent);
    output.push_str(name);
    output.push_str(": ");
    output.push_str(value);
    output.push_str(",\n");
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let typ = TypeKind::Byte;
        let serialized = serialize_type_kind(&typ);
        assert_eq!(serialized, "TypeKind::Byte");
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
    fn test_serialize_operator() {
        assert_eq!(serialize_operator(&Operator::Equal), "Operator::Equal");
        assert_eq!(
            serialize_operator(&Operator::NotEqual),
            "Operator::NotEqual"
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
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(0x7F),
            message: "test".to_string(),
            children: vec![],
            level: 0,
        };

        let generated = generate_builtin_rules(&[rule]);

        assert!(generated.contains("OffsetSpec::Absolute(0)"));
        assert!(generated.contains("TypeKind::Byte"));
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
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "child".to_string(),
            children: vec![],
            level: 1,
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
        assert!(generated.contains("TypeKind::Byte"));
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
