// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Name table for `MetaType::Name` subroutine extraction.
//!
//! When a magic file declares `0 name <identifier>` at the top level, its
//! children form a named subroutine that can be invoked later via
//! `use <identifier>`. This module extracts those definitions out of the
//! flat rule list at load time, so the evaluator can look them up by name
//! without re-walking the AST.

use std::collections::HashMap;

use log::warn;

use crate::parser::ast::{MagicRule, MetaType, TypeKind};

/// A lookup table mapping subroutine names to their child rule lists.
///
/// Built by [`extract_name_table`] from a parsed magic file's top-level
/// rule list. The evaluator consults this table when it encounters a
/// `TypeKind::Meta(MetaType::Use(name))` rule to retrieve the rules that
/// should be evaluated as if inlined at the `use` site.
#[derive(Debug, Default, Clone)]
pub(crate) struct NameTable {
    inner: HashMap<String, Vec<MagicRule>>,
}

impl NameTable {
    /// Create an empty name table.
    #[must_use]
    pub(crate) fn empty() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Look up a subroutine's rule list by name.
    #[must_use]
    pub(crate) fn get(&self, name: &str) -> Option<&Vec<MagicRule>> {
        self.inner.get(name)
    }

    /// Mutable access to the underlying map.
    ///
    /// Used by `MagicDatabase` after load to sort each subroutine's rules
    /// by strength without allocating a new map.
    pub(crate) fn values_mut(
        &mut self,
    ) -> std::collections::hash_map::ValuesMut<'_, String, Vec<MagicRule>> {
        self.inner.values_mut()
    }

    /// Merge another name table into this one.
    ///
    /// Used when loading a magic directory: each file's extracted name
    /// table is merged into the accumulating table. On key collisions,
    /// the first-seen definition is kept and a warning is emitted.
    pub(crate) fn merge(&mut self, other: Self) {
        for (name, rules) in other.inner {
            if self.inner.contains_key(&name) {
                warn!("duplicate name definition '{name}' across magic files; keeping first");
                continue;
            }
            self.inner.insert(name, rules);
        }
    }
}

/// Partition a top-level rule list, hoisting `name` rules into a
/// [`NameTable`] and returning the remaining non-`Name` rules.
///
/// - Top-level `Name` rules are removed; their `children` become the
///   subroutine body in the returned table. On duplicate names, the
///   first definition wins and a warning is logged.
/// - `Name` rules appearing below the top level (as a child of another
///   rule) are dropped with a warning; they are not well-defined in
///   magic(5) and would confuse the evaluator's lookup path.
/// - Non-`Name` rules are returned unchanged, with their own children
///   also scrubbed of any nested `Name` rules.
pub(crate) fn extract_name_table(rules: Vec<MagicRule>) -> (Vec<MagicRule>, NameTable) {
    let mut table = NameTable::empty();
    let mut kept = Vec::with_capacity(rules.len());

    for rule in rules {
        if let TypeKind::Meta(MetaType::Name(ref name)) = rule.typ {
            if table.inner.contains_key(name) {
                warn!("duplicate name definition '{name}'; keeping first");
                continue;
            }
            // Recursively scrub nested Name rules from the subroutine's
            // children (shouldn't appear in practice, but be defensive).
            let children = scrub_nested_names(rule.children, rule.level);
            table.inner.insert(name.clone(), children);
        } else {
            let scrubbed_children = scrub_nested_names(rule.children, rule.level + 1);
            kept.push(MagicRule {
                children: scrubbed_children,
                ..rule
            });
        }
    }

    (kept, table)
}

/// Walk a child list and drop any `Name` rules found below the top level.
fn scrub_nested_names(children: Vec<MagicRule>, parent_level: u32) -> Vec<MagicRule> {
    let mut kept = Vec::with_capacity(children.len());
    for child in children {
        if let TypeKind::Meta(MetaType::Name(ref name)) = child.typ {
            warn!("name directive '{name}' at level {parent_level} is not top-level; skipping");
            continue;
        }
        let scrubbed = scrub_nested_names(child.children, child.level + 1);
        kept.push(MagicRule {
            children: scrubbed,
            ..child
        });
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{OffsetSpec, Operator, Value};

    fn make_rule(level: u32, typ: TypeKind, message: &str, children: Vec<MagicRule>) -> MagicRule {
        MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ,
            op: Operator::Equal,
            value: Value::Uint(0),
            message: message.to_string(),
            children,
            level,
            strength_modifier: None,
        }
    }

    #[test]
    fn test_extract_empty() {
        let (rules, table) = extract_name_table(vec![]);
        assert!(rules.is_empty());
        assert!(table.get("anything").is_none());
    }

    #[test]
    fn test_extract_single_name_rule() {
        let child = make_rule(1, TypeKind::Byte { signed: false }, "child", vec![]);
        let name_rule = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("sub".to_string())),
            "",
            vec![child],
        );
        let (rules, table) = extract_name_table(vec![name_rule]);
        assert!(rules.is_empty());
        let subroutine = table.get("sub").expect("sub subroutine");
        assert_eq!(subroutine.len(), 1);
        assert_eq!(subroutine[0].message, "child");
    }

    #[test]
    fn test_extract_preserves_non_name_rules() {
        let byte_rule = make_rule(0, TypeKind::Byte { signed: false }, "hello", vec![]);
        let (rules, table) = extract_name_table(vec![byte_rule]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "hello");
        assert!(table.get("anything").is_none());
    }

    #[test]
    fn test_extract_duplicate_name_keeps_first() {
        let first = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("dup".to_string())),
            "first",
            vec![make_rule(
                1,
                TypeKind::Byte { signed: false },
                "first-child",
                vec![],
            )],
        );
        let second = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("dup".to_string())),
            "second",
            vec![make_rule(
                1,
                TypeKind::Byte { signed: false },
                "second-child",
                vec![],
            )],
        );
        let (_, table) = extract_name_table(vec![first, second]);
        let subroutine = table.get("dup").expect("first dup kept");
        assert_eq!(subroutine.len(), 1);
        assert_eq!(subroutine[0].message, "first-child");
    }

    #[test]
    fn test_merge_combines_tables() {
        let sub_a = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("a".to_string())),
            "",
            vec![],
        );
        let sub_b = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("b".to_string())),
            "",
            vec![],
        );
        let (_, mut table_a) = extract_name_table(vec![sub_a]);
        let (_, table_b) = extract_name_table(vec![sub_b]);
        table_a.merge(table_b);
        assert!(table_a.get("a").is_some());
        assert!(table_a.get("b").is_some());
    }

    #[test]
    fn test_merge_duplicate_keeps_existing() {
        let first = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("dup".to_string())),
            "",
            vec![make_rule(
                1,
                TypeKind::Byte { signed: false },
                "first-child",
                vec![],
            )],
        );
        let second = make_rule(
            0,
            TypeKind::Meta(MetaType::Name("dup".to_string())),
            "",
            vec![make_rule(
                1,
                TypeKind::Byte { signed: false },
                "second-child",
                vec![],
            )],
        );
        let (_, mut table_a) = extract_name_table(vec![first]);
        let (_, table_b) = extract_name_table(vec![second]);
        table_a.merge(table_b);
        let subroutine = table_a.get("dup").expect("dup kept from first table");
        assert_eq!(subroutine[0].message, "first-child");
    }
}
