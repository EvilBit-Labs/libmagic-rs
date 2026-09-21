// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Single-rule value and pattern evaluation helpers used by
//! [`super::evaluate_single_rule_with_anchor`].
//!
//! Extracted from `engine/mod.rs` as a pure code-motion split (issue #391
//! item 1, Unit U3). `evaluate_pattern_rule` handles `Regex`/`Search` (and
//! flagged `String` equality/inequality) matching; `evaluate_value_rule`
//! handles every other `TypeKind` via the typed-read + coerce + operator
//! pipeline; `string_ordering_display_value` decouples the rendered display
//! value from the (prefix-limited) compared value for `string` ordering
//! comparisons (see GOTCHAS S14.3).

use crate::LibmagicError;
use crate::evaluator::{operators, types};
use crate::parser::ast::{MagicRule, Operator, TypeKind, Value};
use log::debug;

/// Evaluate a pattern-bearing rule (`TypeKind::Regex` / `TypeKind::Search`).
///
/// `read_pattern_match` returns `Some(value)` on a successful match
/// (possibly zero-width, e.g., `a*`) and `None` on a genuine miss; the
/// engine translates those directly into `Equal`/`NotEqual`. Any other
/// operator on a pattern-bearing type is a magic-file semantic bug and
/// surfaces as [`crate::evaluator::types::TypeReadError::UnsupportedType`] -- the earlier
/// fallthrough to `apply_operator` masked this by producing nonsense
/// ordering comparisons against the pattern source text.
///
/// On a miss we return `Value::String(String::new())` as a display
/// placeholder; the engine has already decided `matched = false` by
/// then, so the placeholder only affects display and
/// `bytes_consumed_with_pattern` (which re-derives the match position
/// from the pattern, not this value).
pub(crate) fn evaluate_pattern_rule(
    rule: &MagicRule,
    buffer: &[u8],
    absolute_offset: usize,
    max_string_length: usize,
) -> Result<(bool, crate::parser::ast::Value), LibmagicError> {
    let match_outcome = types::read_pattern_match(
        buffer,
        absolute_offset,
        &rule.typ,
        Some(&rule.value),
        max_string_length,
    )
    .map_err(|e| LibmagicError::EvaluationError(e.into()))?;
    let pattern_found = match_outcome.is_some();
    let matched = match &rule.op {
        crate::parser::ast::Operator::Equal => pattern_found,
        crate::parser::ast::Operator::NotEqual => !pattern_found,
        other => {
            return Err(LibmagicError::EvaluationError(
                types::TypeReadError::UnsupportedType {
                    type_name: format!(
                        "operator {other:?} is not supported for pattern-bearing type {:?}; only Equal (=) and NotEqual (!=) are allowed",
                        rule.typ
                    ),
                }
                .into(),
            ));
        }
    };
    let value = match_outcome.unwrap_or_else(|| crate::parser::ast::Value::String(String::new()));
    Ok((matched, value))
}

/// Evaluate a value-based rule (all non-pattern-bearing `TypeKind` variants).
///
/// Reads the typed value at `absolute_offset`, coerces the rule's
/// expected value to the target type's signedness/width (zero-copy via
/// `Cow::Borrowed` on the hot path), and applies the operator.
/// `BitwiseNot` needs type-aware width masking so the complement is
/// computed at the type's natural width (e.g. byte `NOT 0x00 = 0xFF`,
/// not `u64::MAX`).
pub(crate) fn evaluate_value_rule(
    rule: &MagicRule,
    buffer: &[u8],
    absolute_offset: usize,
    max_string_length: usize,
    flip_endian: bool,
) -> Result<(bool, crate::parser::ast::Value), LibmagicError> {
    // Apply the `use \^name` endian flip (issue #236) at read time, exactly
    // as libmagic's `cvt_flip(m->type, flip)` does in `softmagic.c`. Only the
    // typed READ needs the flipped endianness -- `bit_width()`,
    // `coerce_value_to_type` (the literal's numeric value is endian-invariant),
    // the relative-offset `bytes_consumed` advance, and the string-ordering
    // display read are all endian-invariant, so they keep `rule.typ`.
    // `flip_type_endian` is a cheap no-op clone for the common `flip == false`
    // path.
    let read_typ: std::borrow::Cow<'_, crate::parser::ast::TypeKind> = if flip_endian {
        std::borrow::Cow::Owned(types::flip_type_endian(&rule.typ))
    } else {
        std::borrow::Cow::Borrowed(&rule.typ)
    };
    let read_value = types::read_typed_value_with_pattern(
        buffer,
        absolute_offset,
        read_typ.as_ref(),
        Some(&rule.value),
        max_string_length,
    )
    .map_err(|e| LibmagicError::EvaluationError(e.into()))?;

    // Apply any pre-comparison value transform (`type+N`/`type-N`/`type*N`/
    // `type/N`/`type%N`/`type|N`/`type^N`). The transform runs on the read
    // value before the comparison operator and before printf-style format
    // substitution, so `%d` in the message renders the post-transform
    // number. `&MASK` is *not* handled here -- it lives at the operator
    // layer via `Operator::BitwiseAndMask`.
    let transformed_value = match rule.value_transform {
        None => read_value,
        Some(t) => {
            let transformed = operators::apply_value_transform(&read_value, t)
                .map_err(LibmagicError::EvaluationError)?;
            // Re-narrow to the type's width so both sides of the comparison
            // share one representation. The literal below is narrowed by
            // `coerce_value_to_type`, so without this a masked signed read
            // and its literal disagree despite identical bit patterns:
            // `beshort&0xFFE0 =0xFFE0` on 0xFFE1 masked to i64 65504 while
            // the literal narrowed to i16 -32. That mismatch is why the
            // generic APPn rule in `jpeg` never matched and the segment
            // walk never advanced. libmagic works in the type's machine
            // word throughout.
            types::narrow_transformed_to_type_width(transformed, &rule.typ)
        }
    };

    let expected_value = types::coerce_value_to_type(&rule.value, &rule.typ);
    let expected_ref: &crate::parser::ast::Value = expected_value.as_ref();

    let matched = match &rule.op {
        crate::parser::ast::Operator::BitwiseNot => operators::apply_bitwise_not_with_width(
            &transformed_value,
            expected_ref,
            rule.typ.bit_width(),
        ),
        // Masked equality (`type&MASK VALUE`) must re-normalize the masked
        // result to the type's natural width so a signed read whose high bits
        // are cleared by the mask still compares equal to the sign-extended
        // rule literal (e.g. the Mach-O `0 lelong&0xfffffffe 0xfeedface`
        // rule). See `apply_bitwise_and_mask_with_width`.
        crate::parser::ast::Operator::BitwiseAndMask(mask) => {
            operators::apply_bitwise_and_mask_with_width(
                *mask,
                &transformed_value,
                expected_ref,
                rule.typ.bit_width(),
            )
        }
        op => operators::apply_operator(op, &transformed_value, expected_ref),
    };

    // libmagic renders the FULL string field (`p->s`) for a matched string
    // comparison, while the comparison itself is prefix-limited to
    // `pattern.len()` (`file_strncmp` with `vallen`). The comparison above
    // already read exactly `pattern.len()` bytes -- correct and unchanged --
    // but for an ORDERING operator the rendered detail needs the whole field,
    // not the compared prefix. sgml's `>15 string/t >\0 %.3s document text`
    // is the motivating case: comparing `>\0` reads 1 byte ("1"), but the
    // `%.3s` must render the full field ("1.0") to produce `XML 1.0 ...`.
    // Re-read the full field for DISPLAY only; the `matched` decision above is
    // left byte-identical, so this cannot change any match result.
    let display_value = string_ordering_display_value(
        rule,
        buffer,
        absolute_offset,
        max_string_length,
        transformed_value,
    );
    Ok((matched, display_value))
}

/// Compute the DISPLAY value for a value-rule match, decoupling it from the
/// value used in the comparison.
///
/// For a `string` rule compared with an ORDERING operator (`<`/`>`/`<=`/`>=`),
/// libmagic renders the full string field (`p->s`, read until NUL/EOF) even
/// though the comparison is prefix-limited to `pattern.len()`. This re-reads
/// that full field so `%s`/`%.Ns` format specifiers render the whole value
/// rather than only the compared prefix. For every other type or operator the
/// compared value already IS the field libmagic renders, so `compared` is
/// returned unchanged.
///
/// Only `TypeKind::String` needs this: `PString` (`read_pstring`) and
/// `String16` (`read_string16`) already read their full field independent of
/// `pattern.len()`, and numeric types render the whole value.
///
/// On a display-side read error after a successful match, the compared value
/// is returned rather than propagating -- a matched rule must not abort on a
/// display-only read.
pub(crate) fn string_ordering_display_value(
    rule: &MagicRule,
    buffer: &[u8],
    absolute_offset: usize,
    max_string_length: usize,
    compared: crate::parser::ast::Value,
) -> crate::parser::ast::Value {
    use crate::parser::ast::Operator::{GreaterEqual, GreaterThan, LessEqual, LessThan};

    let is_ordering = matches!(rule.op, LessThan | GreaterThan | LessEqual | GreaterEqual);
    if is_ordering && matches!(rule.typ, TypeKind::String { .. }) {
        // R2/R12 (U3 step 3): when the null-first-byte idiom gates (see
        // `newline_stop_gate`), the display read is ALSO bounded and
        // newline-stopped, matching the any-value read path -- otherwise
        // a `>\0...` rule's rendered field could still carry an embedded
        // newline that only the render-time bound (127 bytes, no stop)
        // would catch. The ordinary (non-gated) ordering idiom -- e.g.
        // `>0.6.1 ... %s` -- keeps the unbounded-by-newline full-field
        // read it always had (GOTCHAS S14.3).
        let read_cap = if newline_stop_gate(rule) {
            types::any_value_string_bound(buffer, absolute_offset, max_string_length)
        } else {
            max_string_length
        };
        match types::read_string(buffer, absolute_offset, Some(read_cap)) {
            Ok(full_field) => full_field,
            // A matched rule must not abort on a display-only read (the compared
            // prefix was already read successfully at this offset moments ago),
            // so fall back to it -- but `debug!` first rather than swallowing the
            // error silently, matching this file's graceful-skip logging
            // discipline. A latent regression here (e.g. `max_string_length`
            // disagreeing with the original read) would otherwise render the
            // truncated prefix with no trace of why the full field was dropped.
            Err(e) => {
                debug!(
                    "string_ordering_display_value: full-field read failed at offset {absolute_offset} for rule '{}': {e}; rendering compared prefix",
                    rule.message
                );
                compared
            }
        }
    } else {
        compared
    }
}

/// Whether R2's newline-stop gate applies to `rule`'s rendered `%s` value.
///
/// The gate fires for an any-value rule (`x`), or for an ordering-compared
/// rule (`<`/`>`/`<=`/`>=`) whose declared comparison pattern's first byte
/// is a null byte. This mirrors upstream libmagic's `*m->value.s == '\0'`
/// first-byte test -- NOT a zero-length test: measured against `file-5.41`,
/// the 4-byte pattern `>\0ABC` gates and the 1-byte pattern `>0` does not,
/// because `'0'` (0x30) is not a null byte.
///
/// Neither the rule's operator nor its pattern's first byte reaches
/// [`crate::output::format::format_magic_message_with_gate`] today. This
/// unit's live consumer is [`string_ordering_display_value`], which reads
/// the gate directly (`rule` is already in scope there) rather than
/// threading it through `RuleMatch`/`concatenate_messages`
/// (`lib.rs`) -- see this crate's plan doc R14 for why that fuller
/// end-to-end wire was left for a follow-up: `RuleMatch` is
/// `#[non_exhaustive]` public API and several in-crate struct-literal
/// construction sites outside this unit's touched-file set would need
/// updating to add a field to it.
pub(crate) fn newline_stop_gate(rule: &MagicRule) -> bool {
    if matches!(rule.op, Operator::AnyValue) {
        return true;
    }
    let is_ordering = matches!(
        rule.op,
        Operator::LessThan | Operator::GreaterThan | Operator::LessEqual | Operator::GreaterEqual
    );
    is_ordering && pattern_first_byte_is_null(&rule.value)
}

/// Whether `pattern`'s first byte is a null byte (0x00).
fn pattern_first_byte_is_null(pattern: &Value) -> bool {
    match pattern {
        Value::String(s) => s.as_bytes().first() == Some(&0),
        Value::Bytes(b) => b.first() == Some(&0),
        Value::Uint(_) | Value::Int(_) | Value::Float(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{OffsetSpec, StringFlags};

    fn rule_with(op: Operator, value: Value) -> MagicRule {
        MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            op,
            value,
            "msg".to_string(),
        )
    }

    #[test]
    fn test_any_value_rule_gates() {
        let rule = rule_with(Operator::AnyValue, Value::String(String::new()));
        assert!(newline_stop_gate(&rule));
    }

    #[test]
    fn test_ordering_rule_with_null_first_byte_pattern_gates() {
        // The measured oracle case: `>\0ABC` gates.
        let rule = rule_with(
            Operator::GreaterThan,
            Value::Bytes(vec![0, b'A', b'B', b'C']),
        );
        assert!(newline_stop_gate(&rule));
    }

    #[test]
    fn test_ordering_rule_with_non_null_first_byte_pattern_does_not_gate() {
        // The measured oracle case: `>0` does not gate -- '0' (0x30) is not
        // a null byte, even though the pattern is only 1 byte long. This is
        // the first-byte test, not a zero-length test.
        let rule = rule_with(Operator::GreaterThan, Value::String("0".to_string()));
        assert!(!newline_stop_gate(&rule));
    }

    #[test]
    fn test_equality_rule_does_not_gate_even_with_null_first_byte_pattern() {
        // The gate is scoped to any-value or *ordering*-compared rules;
        // an equality comparison never gates, regardless of pattern.
        let rule = rule_with(Operator::Equal, Value::Bytes(vec![0, b'A']));
        assert!(!newline_stop_gate(&rule));
    }

    #[test]
    fn test_equality_rule_with_ordinary_pattern_does_not_gate() {
        let rule = rule_with(Operator::Equal, Value::String("hello".to_string()));
        assert!(!newline_stop_gate(&rule));
    }

    #[test]
    fn test_ordering_rule_with_empty_pattern_does_not_panic_or_gate() {
        // An empty pattern has no first byte; must not gate and must not
        // panic on `.first()`.
        let rule = rule_with(Operator::LessThan, Value::String(String::new()));
        assert!(!newline_stop_gate(&rule));
    }

    #[test]
    fn test_all_four_ordering_operators_check_first_byte() {
        for op in [
            Operator::LessThan,
            Operator::GreaterThan,
            Operator::LessEqual,
            Operator::GreaterEqual,
        ] {
            let op_dbg = format!("{op:?}");
            let gated = rule_with(op.clone(), Value::Bytes(vec![0, b'X']));
            assert!(
                newline_stop_gate(&gated),
                "operator {op_dbg} with null-first-byte pattern must gate"
            );

            let ungated = rule_with(op, Value::String("X".to_string()));
            assert!(
                !newline_stop_gate(&ungated),
                "operator {op_dbg} with non-null-first-byte pattern must not gate"
            );
        }
    }

    // =========================================================================
    // R12 (U3 step 3): `string_ordering_display_value` must apply R2's
    // newline stop ONLY when `newline_stop_gate` fires for the rule, and
    // must leave the non-gated ordering render at its full field (R1's
    // 127-byte bound is applied later, at render time, not here).
    // =========================================================================

    #[test]
    fn test_ordering_display_value_gated_by_null_first_byte_stops_at_newline() {
        // `>\0ABC` gates (R2): the null-first-byte idiom. The compared
        // value is irrelevant here -- only the DISPLAY read is under test.
        let rule = rule_with(
            Operator::GreaterThan,
            Value::Bytes(vec![0, b'A', b'B', b'C']),
        );
        let buffer = b"ZZZZABC\nSECOND\n";
        let display = string_ordering_display_value(
            &rule,
            buffer,
            0,
            8192,
            Value::String(String::new()), // compared value is irrelevant here
        );
        assert_eq!(display, Value::String("ZZZZABC".to_string()));
    }

    #[test]
    fn test_ordering_display_value_ungated_keeps_full_field_with_embedded_newline() {
        // Non-null-first-byte pattern does not gate (R2): the display read
        // must NOT stop at the embedded newline -- it renders the whole
        // field (bounded only by `max_string_length`, unchanged from
        // before this unit). Regression guard for GOTCHAS S14.3's
        // `>0.6.1 ... %s` full-field-render idiom.
        let rule = rule_with(Operator::GreaterThan, Value::String("0.6.1".to_string()));
        let buffer = b"0.6.2\nrelease notes";
        let display =
            string_ordering_display_value(&rule, buffer, 0, 8192, Value::String(String::new()));
        assert_eq!(display, Value::String("0.6.2\nrelease notes".to_string()));
    }

    #[test]
    fn test_ordering_display_value_equality_op_is_never_gated_or_reached() {
        // Equality never routes through this function at all (only
        // ordering ops do) -- `compared` passes straight through
        // regardless of buffer content.
        let rule = rule_with(Operator::Equal, Value::Bytes(vec![0, b'A']));
        let buffer = b"whatever\ncontent";
        let display = string_ordering_display_value(
            &rule,
            buffer,
            0,
            8192,
            Value::String("compared".to_string()),
        );
        assert_eq!(display, Value::String("compared".to_string()));
    }
}
