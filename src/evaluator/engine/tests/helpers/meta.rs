// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Builders for meta-type-related `MagicRule`s and matching
//! `EvaluationContext`s. Used by `meta_use_tests`,
//! `meta_default_clear_indirect_tests`, and `meta_offset_tests`.

use crate::evaluator::{EvaluationConfig, EvaluationContext, RuleEnvironment};
use crate::parser::ast::{MagicRule, MetaType, OffsetSpec, Operator, TypeKind, Value};
use crate::parser::name_table::NameTable;

/// Build an `EvaluationContext` with the supplied name table and (optional)
/// root-rules list. The root-rules list is retained for parity with the
/// `RuleEnvironment` shape even though `MetaType::Use` itself does not
/// consult it.
pub fn make_context_with_env(name_table: NameTable, root_rules: &[MagicRule]) -> EvaluationContext {
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(name_table),
        root_rules: std::sync::Arc::from(root_rules),
    });
    EvaluationContext::new(EvaluationConfig::default()).with_rule_env(env)
}

/// Minimal helper: wrap a `TypeKind::Meta(MetaType::Use { name, .. })` rule at
/// offset 0 with the given `message` and empty child list.
pub fn use_rule(name: &str) -> MagicRule {
    use_rule_at(name, 0)
}

/// Build a `Use` rule at a specific use-site offset. Used by tests
/// that need to prove subroutine `base_offset` biasing actually
/// depends on the use-site value.
pub fn use_rule_at(name: &str, offset: i64) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Meta(MetaType::Use {
            name: name.to_string(),
            flip_endian: false,
        }),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: format!("use {name}"),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Construct a name table from `(name, subroutine_rules)` pairs.
pub fn build_name_table(entries: Vec<(&str, Vec<MagicRule>)>) -> NameTable {
    // Build via the extraction helper so the table construction matches the
    // real parser path. Wrap each entry in a Name rule whose `children` are
    // the subroutine body.
    let mut top = Vec::new();
    for (name, body) in entries {
        top.push(MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Meta(MetaType::Name(name.to_string())),
            op: Operator::Equal,
            value: Value::Uint(0),
            message: String::new(),
            children: body,
            level: 0,
            strength_modifier: None,
            value_transform: None,
        });
    }
    let (_rules, table) = crate::parser::name_table::extract_name_table(top);
    table
}

/// Construct a name table from `(name, name_message, subroutine_rules)`
/// triples, so tests can exercise a `name` line that carries its own
/// description (GOTCHAS S14.4).
pub fn build_name_table_with_messages(entries: Vec<(&str, &str, Vec<MagicRule>)>) -> NameTable {
    let mut top = Vec::new();
    for (name, name_message, body) in entries {
        top.push(MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Meta(MetaType::Name(name.to_string())),
            op: Operator::Equal,
            value: Value::Uint(0),
            message: name_message.to_string(),
            children: body,
            level: 0,
            strength_modifier: None,
            value_transform: None,
        });
    }
    let (_rules, table) = crate::parser::name_table::extract_name_table(top);
    table
}

/// Build a `Default` rule with the given message and (optional) children.
pub fn default_rule(message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Default),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Build a `Clear` rule. Carries no message in the magic file syntax, but the
/// AST requires a message field.
pub fn clear_rule() -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Meta(MetaType::Clear),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: String::new(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Build a single byte-equality rule at `offset` for `value`.
pub fn byte_eq_rule(offset: i64, value: u64, message: &str) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(value),
        message: message.to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Build an `Indirect` rule at `offset` with optional children.
pub fn indirect_rule(offset: i64, message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Meta(MetaType::Indirect),
        op: Operator::Equal,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}

/// Build an `Offset` rule at `offset` with an `x` (`AnyValue`) operator and
/// the given message. Mirrors `default_rule`/`indirect_rule` helpers.
pub fn offset_rule(offset: i64, message: &str, children: Vec<MagicRule>) -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(offset),
        typ: TypeKind::Meta(MetaType::Offset),
        op: Operator::AnyValue,
        value: Value::Uint(0),
        message: message.to_string(),
        children,
        level: 0,
        strength_modifier: None,
        value_transform: None,
    }
}
