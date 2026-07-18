// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Rule hierarchy building for magic file parsing.
//!
//! Constructs parent-child relationships between magic rules based on
//! indentation levels, implementing a stack-based algorithm for hierarchy construction.

use super::preprocessing::{LineInfo, parse_magic_rule_line};
use crate::error::ParseError;
use crate::parser::ast::MagicRule;
use log::warn;

/// Count the leading `>`-continuation markers on a magic-rule line, i.e. its
/// nesting level, without fully parsing the rule. Used to drop the subtree of
/// a line that failed to parse (its deeper-indented descendants), so an
/// orphaned child cannot silently re-attach to the wrong parent.
fn leading_indent_level(content: &str) -> usize {
    content
        .trim_start()
        .bytes()
        .take_while(|&b| b == b'>')
        .count()
}

/// Builds a hierarchical structure from a flat list of parsed magic rules.
///
/// This function establishes parent-child relationships based on indentation levels.
/// Rules at deeper indentation levels become children of the most recent rule at a
/// shallower level. This implements a stack-based algorithm for hierarchy construction.
///
/// # Arguments
///
/// * `lines` - A vector of preprocessed `LineInfo` structs
///
/// # Returns
///
/// `Result<Vec<MagicRule>, ParseError>` - Root-level rules with children attached
///
/// # Behavior
///
/// - Rules with `level=0` are root rules
/// - Rules with `level=1` become children of the most recent `level=0` rule
/// - Rules with `level=2` become children of the most recent `level=1` rule
/// - When indentation decreases, the stack is unwound and completed rules are attached
/// - Orphaned child rules (starting with '>' but with no preceding parent) are
///   added to the root list with their hierarchy level preserved
///
/// # Errors
///
/// Returns an error if:
/// - Any line contains invalid magic rule syntax
/// - Rule parsing fails (propagated from `parse_magic_rule_line`)
pub(crate) fn build_rule_hierarchy(lines: Vec<LineInfo>) -> Result<Vec<MagicRule>, ParseError> {
    build_rule_hierarchy_impl(lines, false)
}

/// Line-tolerant variant of [`build_rule_hierarchy`] for the **runtime**
/// loader: an unparseable rule (and its subtree) is skipped with a warning
/// instead of aborting the whole file, matching GNU `file`. This lets a real
/// system magic file that contains a construct this parser cannot yet handle
/// (e.g. the `compress` file's one `ustring` XZ rule) still contribute its
/// other rules (gzip, bzip2, ...). The strict [`build_rule_hierarchy`] is kept
/// for build-time codegen of the crate's own builtin rules, where a malformed
/// rule should fail the build. See GOTCHAS S3.11.
pub(crate) fn build_rule_hierarchy_tolerant(
    lines: Vec<LineInfo>,
) -> Result<Vec<MagicRule>, ParseError> {
    build_rule_hierarchy_impl(lines, true)
}

fn build_rule_hierarchy_impl(
    lines: Vec<LineInfo>,
    tolerant: bool,
) -> Result<Vec<MagicRule>, ParseError> {
    /// Helper to pop a rule from the stack and attach it to its parent or roots
    fn pop_and_attach(stack: &mut Vec<MagicRule>, roots: &mut Vec<MagicRule>) {
        if let Some(completed) = stack.pop() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(completed);
            } else {
                roots.push(completed);
            }
        }
    }

    let mut stack: Vec<MagicRule> = Vec::new();
    let mut roots: Vec<MagicRule> = Vec::new();
    let mut pending_strength: Option<crate::parser::ast::StrengthModifier> = None;
    // When a rule line fails to parse we skip it AND its deeper-indented
    // descendants (the subtree it heads), tracked by this level threshold.
    let mut skip_subtree_deeper_than: Option<usize> = None;

    for line in lines {
        if line.is_comment {
            continue;
        }

        // Drop the descendants of a rule that failed to parse: any line more
        // deeply nested than the failed line belongs to its subtree.
        if let Some(threshold) = skip_subtree_deeper_than {
            if leading_indent_level(&line.content) > threshold {
                continue;
            }
            // Back at the failed line's level (a sibling) or shallower --
            // resume normal processing.
            skip_subtree_deeper_than = None;
        }

        // Handle strength directive: store modifier for next rule
        if line.strength_modifier.is_some() {
            pending_strength = line.strength_modifier;
            continue;
        }

        // Line-tolerant parsing (matches GNU `file`): a single unparseable rule
        // no longer aborts the whole file. Skip the offending rule and its
        // subtree with a warning, keeping the rest of the file's rules -- so a
        // file like `compress` (whose one `ustring` XZ rule this parser cannot
        // yet handle) still contributes its gzip/bzip2 detection instead of
        // being dropped entirely. See GOTCHAS S3.11.
        let mut rule = match parse_magic_rule_line(&line) {
            Ok(rule) => rule,
            Err(err) => {
                if !tolerant {
                    // Strict mode (build-time codegen): a malformed rule fails.
                    return Err(err);
                }
                let preview: String = line.content.chars().take(80).collect();
                warn!(
                    "skipping unparseable magic rule at line {}: {err} (rule: {preview:?})",
                    line.line_number
                );
                // The failed line's subtree is meaningless without it, and an
                // orphaned strength directive must not leak onto a later rule.
                skip_subtree_deeper_than = Some(leading_indent_level(&line.content));
                pending_strength = None;
                continue;
            }
        };

        // Apply pending strength modifier to this rule
        if pending_strength.is_some() {
            rule.strength_modifier = pending_strength.take();
        }

        // Unwind stack until we find a parent with lower level
        while stack.last().is_some_and(|top| top.level >= rule.level) {
            pop_and_attach(&mut stack, &mut roots);
        }

        stack.push(rule);
    }

    // Unwind remaining stack
    while !stack.is_empty() {
        pop_and_attach(&mut stack, &mut roots);
    }

    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::preprocessing::LineInfo;

    fn li(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: false,
            strength_modifier: None,
        }
    }

    // ============================================================
    // Tests for build_rule_hierarchy (10+ test cases)
    // ============================================================

    #[test]
    fn test_build_rule_hierarchy_single_root() {
        let lines = vec![li(1, "0 string \\x7fELF ELF executable")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 0);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_one_child() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 1);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_multiple_children() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
            li(3, ">4 byte 2 64-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_nested_three_levels() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 class"),
            li(3, ">>5 byte 1 subtype"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children[0].level, 2);
    }

    #[test]
    fn test_build_rule_hierarchy_multiple_roots() {
        let lines = vec![
            li(1, r#"0 string "ELF" "ELF executable""#),
            li(2, r#"0 string "%PDF" "PDF document""#),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_sibling_rules() {
        let lines = vec![
            li(1, "0 byte 1 Root"),
            li(2, ">4 byte 1 Child1"),
            li(3, ">4 byte 2 Child2"),
            li(4, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_deep_nesting() {
        let lines = vec![
            li(1, "0 byte 1 L0"),
            li(2, ">4 byte 1 L1"),
            li(3, ">>5 byte 2 L2"),
            li(4, ">>>6 byte 3 L3"),
            li(5, ">>>>7 byte 4 L4"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(
            roots[0].children[0].children[0].children[0].children.len(),
            1
        );
    }

    #[test]
    fn test_build_rule_hierarchy_return_to_root_level() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 Child"),
            li(3, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[1].children.len(), 0);
    }

    #[test]
    fn test_build_rule_hierarchy_orphaned_child() {
        let lines = vec![li(1, ">4 byte 1 Orphaned child")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 1);
    }

    #[test]
    fn test_build_rule_hierarchy_complex_structure() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 C1"),
            li(3, ">4 byte 2 C2"),
            li(4, ">>6 byte 3 GC1"),
            li(5, "0 byte 2 Root2"),
            li(6, ">4 byte 4 C3"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[0].children[1].children.len(), 1);
        assert_eq!(roots[1].children.len(), 1);
    }
}
