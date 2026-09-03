// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Strength calculation for magic rules
//!
//! This module implements the strength calculation algorithm based on libmagic's
//! `apprentice_magic_strength` function. Strength is used to order rules during
//! evaluation, giving priority to more specific rules.
//!
//! # Algorithm Overview
//!
//! The default strength of a rule is calculated based on several factors:
//! - **Type specificity**: String types have higher strength than numeric types
//! - **Operator specificity**: Equality operators are more specific than bitwise
//! - **Offset type**: Absolute offsets are more reliable than indirect/relative
//! - **Value length**: Longer strings are more specific matches
//!
//! The calculated strength can be modified using `!:strength` directives in magic
//! files, which apply arithmetic operations to the default strength.

use crate::parser::ast::{MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

/// Maximum strength value (clamped to prevent overflow)
pub const MAX_STRENGTH: i32 = 255;

/// Minimum strength value (clamped to prevent negative strength)
pub const MIN_STRENGTH: i32 = 0;
/// libmagic's `MULT` from `apprentice_magic_strength`, used to scale a
/// `search` rule's strength inversely by its scan range.
const SEARCH_RANGE_MULT: usize = 10;

/// Upper bound on a `search` rule's type contribution, matching the
/// constrained-string score so a tight scan cannot outrank an exact match.
const SEARCH_STRENGTH_CAP: i32 = 25;

/// Byte length of a `search` rule's pattern, which is what libmagic's
/// `vallen` measures. A non-pattern operand contributes no length.
fn search_pattern_len(value: &crate::parser::ast::Value) -> i32 {
    use crate::parser::ast::Value;
    let len = match value {
        Value::String(s) => s.len(),
        Value::Bytes(b) => b.len(),
        _ => 0,
    };
    i32::try_from(len).unwrap_or(i32::MAX)
}

/// Calculate the default strength of a magic rule based on its specificity.
///
/// This function implements an algorithm inspired by libmagic's `apprentice_magic_strength`
/// function. The strength is calculated based on:
///
/// - **Type contribution**: How specific the type matching is
/// - **Operator contribution**: How specific the comparison is
/// - **Offset contribution**: How reliable the offset is
/// - **Value length contribution**: For strings, longer matches are more specific
///
/// # Arguments
///
/// * `rule` - The magic rule to calculate strength for
///
/// # Returns
///
/// The calculated default strength as an `i32`, clamped to `[MIN_STRENGTH, MAX_STRENGTH]`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, StringFlags, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::calculate_default_strength;
///
/// let rule = MagicRule::new(OffsetSpec::Absolute(0), TypeKind::String { max_length: None, flags: StringFlags::default() }, Operator::Equal, Value::String("ELF".to_string()), "ELF file".to_string());
///
/// let strength = calculate_default_strength(&rule);
/// assert!(strength > 0);
/// ```
#[must_use]
pub fn calculate_default_strength(rule: &MagicRule) -> i32 {
    let mut strength: i32 = 0;

    // Type contribution: more specific types get higher strength
    strength += match &rule.typ {
        // Strings are most specific (they match exact byte sequences).
        // Flagged strings get a per-flag penalty because some flags broaden
        // what the rule matches: `string/c FOO` matches both `FOO` and
        // `foo`, so it should sort BELOW `string FOO` (which only matches
        // `FOO` exactly) under stop_at_first_match. This mirrors libmagic's
        // `apprentice.c::apprentice_magic_strength` which subtracts 1 per
        // `STRING_IGNORE_*` flag bit. We extend the penalty to whitespace
        // flags (`/w` and `/W`) by the same logic. `/f` (full-word) is
        // NOT penalized because it tightens the match (requires a word
        // boundary) rather than broadening it; `/T` (trim) and the
        // MIME-output hints (`/t`, `/b`) carry no penalty either -- see
        // `string_flag_specificity_penalty` for the canonical list.
        TypeKind::String { max_length, flags } => {
            let base = 20;
            let with_length_bonus = if max_length.is_some() { base + 5 } else { base };
            with_length_bonus - string_flag_specificity_penalty(*flags)
        }
        TypeKind::PString { max_length, .. } => {
            let base = 20;
            if max_length.is_some() { base + 5 } else { base }
        }
        // UCS-2 strings (`lestring16`/`bestring16`) match byte sequences too,
        // but each character is two bytes wide. Treat them like an unbounded
        // `string` -- no `max_length` knob exists at the magic-file level, so
        // the "constrained" bonus does not apply.
        TypeKind::String16 { .. } => 20,
        // Regex matches a pattern -- treat similarly to an unbounded string.
        // A rule with an EXPLICIT count (byte count, or line count with a
        // specific N) is more constrained than a plain `regex` default, so
        // it gets the same bonus as a length-limited string. Note that
        // `RegexCount::Lines(None)` (the `regex/l` shorthand) has the same
        // effective scan window as `RegexCount::Default` -- both walk the
        // full 8192-byte capped window -- so they get the same strength
        // score. Giving `Lines(None)` the "constrained" bonus would reward
        // users for typing `/l` instead of nothing even though the scan
        // window is identical.
        TypeKind::Regex { count, .. } => {
            use crate::parser::ast::RegexCount;
            match count {
                RegexCount::Default | RegexCount::Lines(None) => 20,
                RegexCount::Bytes(_) | RegexCount::Lines(Some(_)) => 25,
            }
        }
        // Search strength scales with pattern length and INVERSELY with the
        // scan range, porting libmagic's `apprentice_magic_strength`
        // (`vallen * MAX(MULT / str_range, 1)`, MULT = 10): a wide scan for a
        // short pattern is weak evidence, a tight scan for a long one is
        // strong. A flat bonus here let sgml's 4-byte `search/4096 \<!--`
        // outrank real numeric detectors and mislabel ~15% of Mach-O
        // binaries as SGML (#379). Capped at the constrained-string score so
        // a tight search cannot outrank an exact string match.
        TypeKind::Search { range, .. } => {
            let pattern_len = search_pattern_len(&rule.value);
            let multiplier = range.map_or(1, |r| {
                // Truncating division is libmagic's behavior, not an accident:
                // any range wider than MULT floors to 0 and clamps to 1.
                #[allow(clippy::integer_division)]
                let m = SEARCH_RANGE_MULT / r.get().max(1);
                i32::try_from(m).unwrap_or(1).max(1)
            });
            (pattern_len.saturating_mul(multiplier)).min(SEARCH_STRENGTH_CAP)
        }
        // 64-bit types are most specific among numerics
        TypeKind::Quad { .. } | TypeKind::Double { .. } | TypeKind::QDate { .. } => 16,
        // 32-bit types are fairly specific
        TypeKind::Long { .. } | TypeKind::Float { .. } | TypeKind::Date { .. } => 15,
        // 16-bit integers are moderately specific
        TypeKind::Short { .. } => 10,
        // Single bytes are least specific
        TypeKind::Byte { .. } => 5,
        // Meta-type directives do not read or compare bytes, so most of
        // them contribute no ordering specificity. `Use` and `Indirect`
        // get a moderate score because the rules they dispatch into can
        // carry real specificity that is opaque from the call site.
        //
        // `clippy::match_same_arms` is silenced here so the per-variant
        // rationale is preserved verbatim instead of being collapsed into
        // a single OR-arm: the variants are semantically distinct (each
        // dispatches into a different evaluator path) and the explicit
        // table is the documentation we want to keep next to the values.
        #[allow(clippy::match_same_arms)]
        TypeKind::Meta(meta) => match meta {
            // `default` must sort below every real rule so it only fires
            // when no sibling matched at the current level.
            crate::parser::ast::MetaType::Default => 0,
            // `clear` is a control-flow toggle with no byte-matching
            // specificity of its own.
            crate::parser::ast::MetaType::Clear => 0,
            // `name` rules are extracted at load time and never sorted at
            // eval time; the value is provided for completeness.
            crate::parser::ast::MetaType::Name(_) => 0,
            // `use` dispatches into a subroutine whose specificity is
            // opaque from the call site -- give it a moderate weight so
            // it sorts above pure no-ops but below real type-bearing rules.
            crate::parser::ast::MetaType::Use { .. } => 5,
            // `indirect` re-evaluates the root rule set at the resolved
            // offset; same rationale as `use` for the moderate weight.
            crate::parser::ast::MetaType::Indirect => 5,
            // `offset` reports the current file offset rather than reading
            // a typed value -- no byte-matching specificity.
            crate::parser::ast::MetaType::Offset => 0,
        },
    };

    // Operator contribution: equality is most specific
    strength += match &rule.op {
        // Exact equality is most specific
        Operator::Equal => 10,
        // Inequality is somewhat specific
        Operator::NotEqual => 5,
        // Comparison operators are moderately specific
        Operator::LessThan
        | Operator::GreaterThan
        | Operator::LessEqual
        | Operator::GreaterEqual => 6,
        // Bitwise AND with mask is moderately specific
        Operator::BitwiseAndMask(_) => 7,
        // Plain bitwise AND is least specific
        Operator::BitwiseAnd => 3,
        // Bitwise XOR and NOT are moderately specific
        Operator::BitwiseXor | Operator::BitwiseNot => 4,
        // Any value always matches, least specific
        Operator::AnyValue => 1,
    };

    // Offset contribution: absolute offsets are most reliable
    strength += match &rule.offset {
        // Absolute offsets are most reliable
        OffsetSpec::Absolute(_) => 10,
        // From-end offsets are also reliable (just from the other end)
        OffsetSpec::FromEnd(_) => 8,
        // Indirect offsets depend on reading a pointer first
        OffsetSpec::Indirect { .. } => 5,
        // Relative offsets depend on previous match position
        OffsetSpec::Relative(_) => 3,
    };

    // Value length contribution: longer values are more specific
    // Only applicable to string and bytes values
    let value_length_bonus = match &rule.value {
        Value::String(s) => {
            // Each character adds to specificity, capped at 20
            i32::try_from(s.len()).unwrap_or(20).min(20)
        }
        Value::Bytes(b) => {
            // Each byte adds to specificity, capped at 20
            i32::try_from(b.len()).unwrap_or(20).min(20)
        }
        // Numeric values don't get length bonus
        Value::Uint(_) | Value::Int(_) | Value::Float(_) => 0,
    };
    strength += value_length_bonus;

    // Clamp to valid range
    strength.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Count how many `string`-flag bits make a rule's pattern fuzzier.
///
/// libmagic `apprentice.c::apprentice_magic_strength` subtracts 1 from the
/// computed strength for each `STRING_IGNORE_*` bit set. We extend the
/// same reasoning to the whitespace flags (`/w` and `/W`) because they
/// broaden what the rule matches.
///
/// Penalized flags (each costs 1 point):
/// - `/c` (`ignore_lowercase`) -- ASCII case-fold when pattern is lowercase
/// - `/C` (`ignore_uppercase`) -- ASCII case-fold when pattern is uppercase
/// - `/w` (`compact_optional_whitespace`) -- file whitespace optional
/// - `/W` (`compact_whitespace`) -- file whitespace required but elastic
///
/// Non-penalized flags (no specificity change):
/// - `/t` (`text_test`) and `/b` (`bin_test`) -- MIME-output hints only
/// - `/T` (`trim`) -- pattern-side normalization, not a fuzziness knob
/// - `/f` (`full_word`) -- TIGHTENS the match by requiring a post-match
///   word boundary; opposite direction from fuzziness, so it should not
///   be penalized.
fn string_flag_specificity_penalty(flags: crate::parser::ast::StringFlags) -> i32 {
    let mut penalty = 0;
    if flags.ignore_lowercase {
        penalty += 1;
    }
    if flags.ignore_uppercase {
        penalty += 1;
    }
    if flags.compact_whitespace {
        penalty += 1;
    }
    if flags.compact_optional_whitespace {
        penalty += 1;
    }
    penalty
}

/// Apply a strength modifier to a base strength value.
///
/// This function applies the arithmetic operation specified by the `StrengthModifier`
/// to the given base strength. The result is clamped to `[MIN_STRENGTH, MAX_STRENGTH]`.
///
/// # Arguments
///
/// * `base_strength` - The default calculated strength
/// * `modifier` - The modifier to apply
///
/// # Returns
///
/// The modified strength, clamped to valid range
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::StrengthModifier;
/// use libmagic_rs::evaluator::strength::apply_strength_modifier;
///
/// // Add 10 to strength
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Add(10)), 60);
///
/// // Subtract 5 from strength
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Subtract(5)), 45);
///
/// // Multiply by 2
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Multiply(2)), 100);
///
/// // Divide by 2
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Divide(2)), 25);
///
/// // Set to absolute value
/// assert_eq!(apply_strength_modifier(50, &StrengthModifier::Set(75)), 75);
/// ```
#[must_use]
pub fn apply_strength_modifier(base_strength: i32, modifier: &StrengthModifier) -> i32 {
    let result = match modifier {
        StrengthModifier::Add(n) => base_strength.saturating_add(*n),
        StrengthModifier::Subtract(n) => base_strength.saturating_sub(*n),
        StrengthModifier::Multiply(n) => base_strength.saturating_mul(*n),
        StrengthModifier::Divide(n) => {
            if *n == 0 {
                // Division by zero: return base strength unchanged
                // (magic file contains !:strength /0 which is invalid)
                base_strength
            } else {
                // `saturating_div` rather than `/`: `i32::MIN / -1` overflows
                // and panics, and library code must not panic. The zero guard
                // above is separate -- `saturating_div` still panics on it.
                base_strength.saturating_div(*n)
            }
        }
        StrengthModifier::Set(n) => *n,
    };

    // Clamp to valid range
    result.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Calculate the final strength of a magic rule, including any modifiers.
///
/// This function first calculates the default strength based on the rule's
/// specificity, then applies any strength modifier if present.
///
/// # Arguments
///
/// * `rule` - The magic rule to calculate strength for
///
/// # Returns
///
/// The final calculated strength, clamped to `[MIN_STRENGTH, MAX_STRENGTH]`
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value, StrengthModifier};
/// use libmagic_rs::evaluator::strength::calculate_rule_strength;
///
/// let rule = MagicRule::new(
///     OffsetSpec::Absolute(0),
///     TypeKind::Byte { signed: true },
///     Operator::Equal,
///     Value::Uint(0x7f),
///     "ELF magic".to_string(),
/// )
/// .with_strength_modifier(StrengthModifier::Add(20));
///
/// let strength = calculate_rule_strength(&rule);
/// // Base: 5 (byte) + 10 (equal) + 10 (absolute) + 0 (numeric) = 25
/// // With modifier: 25 + 20 = 45
/// assert_eq!(strength, 45);
/// ```
#[must_use]
pub fn calculate_rule_strength(rule: &MagicRule) -> i32 {
    let base_strength = calculate_default_strength(rule);

    if let Some(ref modifier) = rule.strength_modifier {
        apply_strength_modifier(base_strength, modifier)
    } else {
        base_strength
    }
}

/// Sort magic rules by their calculated strength in descending order.
///
/// Higher strength rules are evaluated first, as they represent more specific
/// matches. This function sorts the rules in-place.
///
/// # Arguments
///
/// * `rules` - The slice of magic rules to sort
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, StringFlags, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::sort_rules_by_strength;
///
/// let mut rules = vec![
///     MagicRule::new(OffsetSpec::Absolute(0), TypeKind::Byte { signed: true }, Operator::Equal, Value::Uint(0x7f), "byte rule".to_string()),
///     MagicRule::new(OffsetSpec::Absolute(0), TypeKind::String { max_length: None, flags: StringFlags::default() }, Operator::Equal, Value::String("MAGIC".to_string()), "string rule".to_string()),
/// ];
///
/// sort_rules_by_strength(&mut rules);
///
/// // String rule should come first (higher strength)
/// assert_eq!(rules[0].message, "string rule");
/// assert_eq!(rules[1].message, "byte rule");
/// ```
pub fn sort_rules_by_strength(rules: &mut [MagicRule]) {
    // Use a stable sort keyed on the negated strength so that higher-strength
    // rules come first while preserving source order for ties. This avoids
    // breaking tests that rely on deterministic ordering of equal-strength
    // rules.
    rules.sort_by_cached_key(|rule| calculate_rule_strength(rule).saturating_neg());
}

/// Sort magic rules by strength in descending order, recursively sorting child
/// rules as well.
///
/// **Do not use this at magic database load time.** Child rules must stay in
/// source order: `default` and `clear` fire based on whether an *earlier*
/// sibling matched, and multi-fragment descriptions render in file order, so
/// reordering children can suppress a `default` or scramble a description.
/// libmagic's `apprentice_sort` orders whole entries by their first line and
/// never reorders the lines inside one. Load paths use the non-recursive
/// [`sort_rules_by_strength`] for exactly this reason -- see GOTCHAS S13.4 and
/// S13.5.
///
/// This recursive variant exists for callers that genuinely want a fully
/// ordered tree independent of evaluation semantics, such as tooling that
/// inspects or reports on rule specificity. It has no production callers.
///
/// The sort is stable: rules with equal strength preserve their source
/// order, so test assertions and libmagic-file semantics that depend on
/// the original ordering of equal-strength siblings continue to hold.
///
/// # Arguments
///
/// * `rules` - The slice of magic rules to sort (in-place, recursive)
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, StringFlags, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::sort_rules_by_strength_recursive;
///
/// let mut rules: Vec<MagicRule> = vec![];
/// sort_rules_by_strength_recursive(&mut rules);
/// assert!(rules.is_empty());
/// ```
pub fn sort_rules_by_strength_recursive(rules: &mut [MagicRule]) {
    sort_rules_by_strength(rules);
    for rule in rules.iter_mut() {
        sort_rules_by_strength_recursive(&mut rule.children);
    }
}

/// Sort magic rules by strength and return the sorted vec (consuming the input).
///
/// This is a convenience function that takes ownership of the rules vector,
/// sorts it by strength, and returns the sorted vector.
///
/// # Arguments
///
/// * `rules` - The vector of magic rules to sort
///
/// # Returns
///
/// The sorted vector with higher strength rules first
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, StringFlags, TypeKind, Operator, Value};
/// use libmagic_rs::evaluator::strength::into_sorted_by_strength;
///
/// let rules = vec![
///     MagicRule::new(OffsetSpec::Absolute(0), TypeKind::Byte { signed: true }, Operator::Equal, Value::Uint(0), "byte rule".to_string()),
///     MagicRule::new(OffsetSpec::Absolute(0), TypeKind::String { max_length: None, flags: StringFlags::default() }, Operator::Equal, Value::String("MAGIC".to_string()), "string rule".to_string()),
/// ];
///
/// let sorted = into_sorted_by_strength(rules);
/// assert_eq!(sorted[0].message, "string rule");
/// ```
#[must_use]
pub fn into_sorted_by_strength(mut rules: Vec<MagicRule>) -> Vec<MagicRule> {
    sort_rules_by_strength(&mut rules);
    rules
}

#[cfg(test)]
mod tests;
