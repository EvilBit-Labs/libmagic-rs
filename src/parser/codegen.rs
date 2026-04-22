// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Code generation for magic rule serialization
//!
//! This module provides functions to serialize parsed magic rules into Rust source
//! code. It is shared between the build script (`build.rs`) and the testable build
//! helpers (`src/build_helpers.rs`), eliminating the previous duplication of 16
//! serialization functions across both files.
//!
//! The generated code creates `MagicRule` struct literals that are compiled into the
//! binary as built-in rules.

use super::ast::{
    Endianness, MagicRule, MetaType, OffsetSpec, Operator, PStringLengthWidth, StrengthModifier,
    TypeKind, Value,
};

const INDENT_WIDTH: usize = 4;

/// Generate the complete Rust source for built-in rules
///
/// Produces a Rust source file containing a `BUILTIN_RULES` static that lazily
/// initializes a `Vec<MagicRule>` from the given parsed rules.
pub fn generate_builtin_rules(rules: &[MagicRule]) -> String {
    let mut output = String::new();

    // Allow unused_imports since StrengthModifier may not be used if no rules have strength modifiers
    push_line(&mut output, "#[allow(unused_imports)]");
    push_line(
        &mut output,
        "use crate::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value, Endianness, StrengthModifier, PStringLengthWidth, MetaType};",
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

/// Serialize a single magic rule as a Rust struct literal
pub fn serialize_magic_rule(rule: &MagicRule, indent: usize) -> String {
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

    push_field(
        &mut output,
        indent + INDENT_WIDTH,
        "strength_modifier",
        &serialize_strength_modifier(rule.strength_modifier),
    );

    push_indent(&mut output, indent);
    output.push('}');

    output
}

/// Serialize child rules as a Rust `vec![]` literal
pub fn serialize_children(children: &[MagicRule], indent: usize) -> String {
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

/// Serialize an offset specification as a Rust expression
pub fn serialize_offset_spec(offset: &OffsetSpec) -> String {
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

/// Serialize a type kind as a Rust expression
pub fn serialize_type_kind(typ: &TypeKind) -> String {
    match typ {
        TypeKind::Byte { signed } => format!("TypeKind::Byte {{ signed: {signed} }}"),
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
        TypeKind::Quad { endian, signed } => format!(
            "TypeKind::Quad {{ endian: {}, signed: {} }}",
            serialize_endianness(*endian),
            signed
        ),
        TypeKind::Float { endian } => format!(
            "TypeKind::Float {{ endian: {} }}",
            serialize_endianness(*endian)
        ),
        TypeKind::Double { endian } => format!(
            "TypeKind::Double {{ endian: {} }}",
            serialize_endianness(*endian)
        ),
        TypeKind::Date { endian, utc } => format!(
            "TypeKind::Date {{ endian: {}, utc: {} }}",
            serialize_endianness(*endian),
            utc
        ),
        TypeKind::QDate { endian, utc } => format!(
            "TypeKind::QDate {{ endian: {}, utc: {} }}",
            serialize_endianness(*endian),
            utc
        ),
        TypeKind::String { max_length } => match max_length {
            Some(value) => {
                format!("TypeKind::String {{ max_length: Some({value}) }}")
            }
            None => "TypeKind::String { max_length: None }".to_string(),
        },
        TypeKind::PString {
            max_length,
            length_width,
            length_includes_itself,
        } => match max_length {
            Some(value) => {
                format!(
                    "TypeKind::PString {{ max_length: Some({value}), length_width: {}, length_includes_itself: {} }}",
                    serialize_pstring_length_width(*length_width),
                    length_includes_itself
                )
            }
            None => format!(
                "TypeKind::PString {{ max_length: None, length_width: {}, length_includes_itself: {} }}",
                serialize_pstring_length_width(*length_width),
                length_includes_itself
            ),
        },
        TypeKind::Regex { flags, count } => {
            // We emit `NonZero::new(N).unwrap_or(NonZero::<...>::MIN)`
            // rather than `.expect("nonzero")` for two reasons:
            // (1) AGENTS.md forbids `unwrap()`/`expect()` in library
            //     code, and this serializer's output IS library code
            //     (compiled into `builtin_rules.rs`);
            // (2) `N` here comes from a `NonZeroU32`/`NonZeroUsize`
            //     value we just called `.get()` on, so it is
            //     provably nonzero and the `unwrap_or` fallback is
            //     unreachable at runtime. Using `unwrap_or` keeps the
            //     invariant expression explicit without introducing a
            //     panic marker into generated code.
            let count_expr = match count {
                crate::parser::ast::RegexCount::Default => {
                    "crate::parser::ast::RegexCount::Default".to_string()
                }
                crate::parser::ast::RegexCount::Bytes(n) => format!(
                    "crate::parser::ast::RegexCount::Bytes(::std::num::NonZeroU32::new({}).unwrap_or(::std::num::NonZeroU32::MIN))",
                    n.get()
                ),
                crate::parser::ast::RegexCount::Lines(None) => {
                    "crate::parser::ast::RegexCount::Lines(None)".to_string()
                }
                crate::parser::ast::RegexCount::Lines(Some(n)) => format!(
                    "crate::parser::ast::RegexCount::Lines(Some(::std::num::NonZeroU32::new({}).unwrap_or(::std::num::NonZeroU32::MIN)))",
                    n.get()
                ),
            };
            format!(
                "TypeKind::Regex {{ flags: crate::parser::ast::RegexFlags {{ case_insensitive: {}, start_offset: {} }}, count: {count_expr} }}",
                flags.case_insensitive, flags.start_offset
            )
        }
        TypeKind::Search { range } => format!(
            "TypeKind::Search {{ range: ::std::num::NonZeroUsize::new({}).unwrap_or(::std::num::NonZeroUsize::MIN) }}",
            range.get()
        ),
        TypeKind::Meta(meta) => match meta {
            MetaType::Default => "TypeKind::Meta(MetaType::Default)".to_string(),
            MetaType::Clear => "TypeKind::Meta(MetaType::Clear)".to_string(),
            MetaType::Indirect => "TypeKind::Meta(MetaType::Indirect)".to_string(),
            MetaType::Offset => "TypeKind::Meta(MetaType::Offset)".to_string(),
            MetaType::Name(id) => format!(
                "TypeKind::Meta(MetaType::Name(String::from({})))",
                format_string_literal(id)
            ),
            MetaType::Use(id) => format!(
                "TypeKind::Meta(MetaType::Use(String::from({})))",
                format_string_literal(id)
            ),
        },
    }
}

/// Serialize a `PStringLengthWidth` as a Rust path
pub fn serialize_pstring_length_width(width: PStringLengthWidth) -> &'static str {
    match width {
        PStringLengthWidth::OneByte => "PStringLengthWidth::OneByte",
        PStringLengthWidth::TwoByteBE => "PStringLengthWidth::TwoByteBE",
        PStringLengthWidth::TwoByteLE => "PStringLengthWidth::TwoByteLE",
        PStringLengthWidth::FourByteBE => "PStringLengthWidth::FourByteBE",
        PStringLengthWidth::FourByteLE => "PStringLengthWidth::FourByteLE",
    }
}

/// Serialize an operator as a Rust expression
pub fn serialize_operator(op: &Operator) -> String {
    match op {
        Operator::Equal => "Operator::Equal".to_string(),
        Operator::NotEqual => "Operator::NotEqual".to_string(),
        Operator::LessThan => "Operator::LessThan".to_string(),
        Operator::GreaterThan => "Operator::GreaterThan".to_string(),
        Operator::LessEqual => "Operator::LessEqual".to_string(),
        Operator::GreaterEqual => "Operator::GreaterEqual".to_string(),
        Operator::BitwiseAnd => "Operator::BitwiseAnd".to_string(),
        Operator::BitwiseAndMask(mask) => format!("Operator::BitwiseAndMask({mask})"),
        Operator::BitwiseXor => "Operator::BitwiseXor".to_string(),
        Operator::BitwiseNot => "Operator::BitwiseNot".to_string(),
        Operator::AnyValue => "Operator::AnyValue".to_string(),
    }
}

/// Serialize a value as a Rust expression
pub fn serialize_value(value: &Value) -> String {
    match value {
        Value::Uint(number) => format!("Value::Uint({})", format_number(*number)),
        Value::Int(number) => format!("Value::Int({})", format_signed_number(*number)),
        Value::Float(f) => {
            if f.is_nan() {
                "Value::Float(f64::NAN)".to_string()
            } else if *f == f64::INFINITY {
                "Value::Float(f64::INFINITY)".to_string()
            } else if *f == f64::NEG_INFINITY {
                "Value::Float(f64::NEG_INFINITY)".to_string()
            } else {
                format!("Value::Float({f:?})")
            }
        }
        Value::Bytes(bytes) => format!("Value::Bytes({})", format_byte_vec(bytes)),
        Value::String(text) => format!(
            "Value::String(String::from({}))",
            format_string_literal(text)
        ),
    }
}

/// Serialize an endianness value as a Rust expression
pub fn serialize_endianness(endian: Endianness) -> String {
    match endian {
        Endianness::Little => "Endianness::Little".to_string(),
        Endianness::Big => "Endianness::Big".to_string(),
        Endianness::Native => "Endianness::Native".to_string(),
    }
}

/// Serialize a strength modifier as a Rust expression
pub fn serialize_strength_modifier(modifier: Option<StrengthModifier>) -> String {
    match modifier {
        None => "None".to_string(),
        Some(StrengthModifier::Add(val)) => format!("Some(StrengthModifier::Add({val}))"),
        Some(StrengthModifier::Subtract(val)) => {
            format!("Some(StrengthModifier::Subtract({val}))")
        }
        Some(StrengthModifier::Multiply(val)) => {
            format!("Some(StrengthModifier::Multiply({val}))")
        }
        Some(StrengthModifier::Divide(val)) => format!("Some(StrengthModifier::Divide({val}))"),
        Some(StrengthModifier::Set(val)) => format!("Some(StrengthModifier::Set({val}))"),
    }
}

/// Format an unsigned number with underscores for readability (`clippy::unreadable_literal`)
pub fn format_number(num: u64) -> String {
    if num < 10000 {
        num.to_string()
    } else {
        let num_str = num.to_string();
        let mut result = String::new();
        let len = num_str.len();

        for (i, ch) in num_str.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                result.push('_');
            }
            result.push(ch);
        }
        result
    }
}

/// Format a signed number with underscores for readability (`clippy::unreadable_literal`)
pub fn format_signed_number(num: i64) -> String {
    if num < 0 {
        let abs = num.unsigned_abs();
        format!("-{}", format_number(abs))
    } else {
        // Safe: num >= 0, so the cast cannot lose the sign
        format_number(num.unsigned_abs())
    }
}

/// Format a byte slice as a Rust `vec![]` literal
pub fn format_byte_vec(bytes: &[u8]) -> String {
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

/// Format a string as a Rust string literal with escaping
pub fn format_string_literal(value: &str) -> String {
    let escaped = value.escape_default().to_string();
    format!("\"{escaped}\"")
}

/// Append a line to the output string
fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

/// Append indentation to the output string
fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push(' ');
    }
}

/// Append a named field to the output string
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

    /// Security regression test for review finding S-L2.
    ///
    /// Verifies that `serialize_magic_rule` escapes attacker-controlled
    /// content in the `message` field so that it cannot inject Rust code
    /// into the generated `builtin_rules.rs` source. The test simulates
    /// a malicious `builtin_rules.magic` message by constructing a rule
    /// programmatically and asserts that the injection tokens do not
    /// appear as bare Rust tokens in the generated output.
    #[test]
    fn test_serialize_escapes_injection_in_message() {
        let malicious = r#""; panic!("pwned-from-message"); let _ = ""#;
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0),
            message: malicious.to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };

        let generated = serialize_magic_rule(&rule, 0);

        // Injection tokens must NOT appear as bare Rust code.
        assert!(
            !generated.contains(r#"panic!("pwned-from-message")"#),
            "injected Rust tokens leaked into generated source:\n{generated}"
        );
        // The escaped form should be present (every `"` in the message
        // becomes `\"` via `str::escape_default`).
        assert!(
            generated.contains(r#"\""#),
            "escaped quote missing from serialized message; \
             escape_default may be broken:\n{generated}"
        );
    }

    /// Security regression test for review finding S-L2: ensure message
    /// strings containing raw newlines, tabs, and control bytes are
    /// escaped rather than written verbatim into the generated source
    /// (which would break string-literal syntax or create multi-line
    /// source fragments).
    #[test]
    fn test_serialize_escapes_control_bytes_in_message() {
        let message = "line1\nline2\ttab\u{0008}backspace";
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(0),
            message: message.to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };

        let generated = serialize_magic_rule(&rule, 0);

        // Raw control characters must not appear verbatim.
        assert!(
            !generated.contains("line1\nline2"),
            "raw newline leaked into generated source:\n{generated}"
        );
        assert!(
            !generated.contains("line2\ttab"),
            "raw tab leaked into generated source:\n{generated}"
        );
        // Escaped forms must be present.
        assert!(
            generated.contains(r"\n"),
            "escaped newline missing from serialized message:\n{generated}"
        );
    }

    /// Security regression test for `MetaType::Name` / `MetaType::Use`:
    /// the identifier is user-controlled (from the magic file) and must
    /// be escaped the same way as the message field. A malicious
    /// identifier containing `"`, `panic!`, or other Rust tokens must
    /// not escape the string literal and land as bare code in the
    /// generated `builtin_rules.rs`.
    #[test]
    fn test_serialize_meta_name_escapes_injection() {
        let malicious = r#""; panic!("pwned-from-meta"); let _ = ""#;
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Meta(MetaType::Name(malicious.to_string())),
            op: Operator::AnyValue,
            value: Value::Uint(0),
            message: "meta rule".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
        };

        let generated = serialize_magic_rule(&rule, 0);

        assert!(
            !generated.contains(r#"panic!("pwned-from-meta")"#),
            "injected Rust tokens leaked through MetaType::Name identifier:\n{generated}"
        );
        assert!(
            generated.contains(r#"\""#),
            "escaped quote missing from serialized MetaType::Name identifier:\n{generated}"
        );
    }
}
