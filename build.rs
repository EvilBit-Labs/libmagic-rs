// Stub module to satisfy error.rs dependencies during build
#[allow(dead_code)]
mod evaluator {
    pub mod types {
        #[derive(Debug)]
        pub struct TypeReadError;
        impl std::fmt::Display for TypeReadError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "TypeReadError")
            }
        }
        impl std::error::Error for TypeReadError {}
    }
}

#[allow(dead_code, unused_imports)]
#[path = "src/error.rs"]
mod error;
#[allow(dead_code, unused_imports)]
#[path = "src/parser/mod.rs"]
mod parser;

use error::ParseError;
use parser::ast::{Endianness, MagicRule, OffsetSpec, Operator, TypeKind, Value};
use parser::parse_text_magic_file;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

const INDENT_WIDTH: usize = 4;

fn main() {
    println!("cargo:rerun-if-changed=src/builtin_rules.magic");

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to read CARGO_MANIFEST_DIR: {err}");
            process::exit(1);
        }
    };

    let magic_path = Path::new(&manifest_dir).join("src/builtin_rules.magic");
    let magic_content = match fs::read_to_string(&magic_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", magic_path.display());
            process::exit(1);
        }
    };

    let rules = match parse_text_magic_file(&magic_content) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", format_parse_error(&err));
            process::exit(1);
        }
    };

    let out_dir = match env::var("OUT_DIR") {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to read OUT_DIR: {err}");
            process::exit(1);
        }
    };

    let output_path = Path::new(&out_dir).join("builtin_rules.rs");
    let generated = generate_builtin_rules(&rules);

    if let Err(err) = fs::write(&output_path, generated) {
        eprintln!("Failed to write {}: {err}", output_path.display());
        process::exit(1);
    }
}

fn format_parse_error(error: &ParseError) -> String {
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
            serialize_endianness(endian)
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
            serialize_endianness(endian),
            signed
        ),
        TypeKind::Long { endian, signed } => format!(
            "TypeKind::Long {{ endian: {}, signed: {} }}",
            serialize_endianness(endian),
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
        Value::Uint(number) => format!("Value::Uint({number})"),
        Value::Int(number) => format!("Value::Int({number})"),
        Value::Bytes(bytes) => format!("Value::Bytes({})", format_byte_vec(bytes)),
        Value::String(text) => format!(
            "Value::String(String::from({}))",
            format_string_literal(text)
        ),
    }
}

fn serialize_endianness(endian: &Endianness) -> String {
    match endian {
        Endianness::Little => "Endianness::Little".to_string(),
        Endianness::Big => "Endianness::Big".to_string(),
        Endianness::Native => "Endianness::Native".to_string(),
    }
}

fn format_byte_vec(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "vec![]".to_string();
    }

    let mut output = String::from("vec![");
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("0x{byte:02x}"));
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
