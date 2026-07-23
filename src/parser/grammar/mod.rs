// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Grammar parsing for magic files using nom parser combinators
//!
//! This module implements the parsing logic for magic file syntax, converting
//! text-based magic rules into the AST representation defined in ast.rs.
//!
//! Submodules split out focused parsing concerns (issue #391 Unit U4):
//! offset specifications (`offset`), comparison operators (`operator`),
//! type+operator+transform parsing (`type_and_operator`), string-family
//! bareword value parsing (`rule_value`), and line-level helpers
//! (`lines`). This top-level module retains the main dispatch entry point,
//! [`parse_magic_rule`].

use nom::{
    IResult, Parser, character::complete::multispace0, combinator::opt, error::Error as NomError,
};

use crate::parser::ast::{MagicRule, MetaType, Operator, TypeKind, Value};
#[cfg(test)]
use crate::parser::ast::{OffsetSpec, StrengthModifier};

mod getstr;
mod lines;
mod numbers;
mod offset;
mod operator;
mod rule_value;
mod type_and_operator;
mod type_suffix;
mod value;

pub use lines::{
    has_continuation, is_comment_line, is_empty_line, is_strength_directive, parse_comment,
    parse_message, parse_strength_directive,
};
pub use numbers::parse_number;
// `parse_offset` has no non-test caller outside `offset.rs` itself (it is
// invoked internally by `parse_rule_offset`); the re-export exists so
// `crate::parser::grammar::parse_offset` keeps resolving for grammar unit
// tests (`use super::*` in `grammar/tests/`), matching its original
// directly-defined-in-mod.rs visibility.
#[allow(unused_imports)]
pub use offset::{parse_offset, parse_rule_offset};
pub use operator::parse_operator;
// `parse_type` is a standalone helper exercised by grammar unit tests only
// (see its `#[allow(dead_code)]` at the definition site).
#[allow(unused_imports)]
pub use type_and_operator::{parse_type, parse_type_and_operator};
pub use value::parse_value;

use rule_value::parse_string_family_value;

#[cfg(test)]
use numbers::{parse_decimal_number, parse_hex_number};
#[cfg(test)]
use value::{parse_escape_sequence, parse_hex_bytes, parse_numeric_value, parse_quoted_string};

/// Parse a complete magic rule line from text format
///
/// Parses a complete magic rule in the format:
/// `[>...]offset type [operator] value [message]`
///
/// Where:
/// - `>...` indicates child rule nesting level (optional)
/// - `offset` is the byte offset to read from
/// - `type` is the data type (byte, short, long, string, etc.)
/// - `operator` is the comparison operator (=, !=, &) - defaults to = if omitted
/// - `value` is the expected value to compare against
/// - `message` is the human-readable description (optional)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_magic_rule;
/// use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value};
///
/// // Basic rule
/// let input = "0 string \\x7fELF ELF executable";
/// let (_, rule) = parse_magic_rule(input).unwrap();
/// assert_eq!(rule.level, 0);
/// assert_eq!(rule.message, "ELF executable");
///
/// // Child rule
/// let input = ">4 byte 1 32-bit";
/// let (_, rule) = parse_magic_rule(input).unwrap();
/// assert_eq!(rule.level, 1);
/// assert_eq!(rule.message, "32-bit");
/// ```
///
/// Consume a leading `x` (`AnyValue`) operator with surrounding whitespace,
/// if present. Used by the Meta-type short-circuit so that
/// `>>&0 offset x at_offset %lld` does not emit `x\tat_offset %lld` as
/// the message. A bare `x` with no following whitespace (e.g. `xylophone`)
/// is left untouched -- we require the `x` to be a standalone token.
fn strip_optional_x_operator(input: &str) -> &str {
    let trimmed = input.trim_start_matches([' ', '\t']);
    if let Some(rest) = trimmed.strip_prefix('x') {
        // Require whitespace or end-of-line after `x` so we don't eat
        // the first character of a message that happens to start with x.
        if rest.is_empty() || rest.starts_with([' ', '\t', '\n', '\r']) {
            return rest.trim_start_matches([' ', '\t']);
        }
    }
    input
}

/// # Errors
///
/// Returns a nom parsing error if:
/// - The offset specification is invalid
/// - The type specification is not recognized
/// - The operator is invalid (if present)
/// - The value cannot be parsed
/// - The input format doesn't match the expected magic rule syntax
pub fn parse_magic_rule(input: &str) -> IResult<&str, MagicRule> {
    let (input, _) = multispace0(input)?;

    // Parse the offset with nesting level
    let (input, (level, offset)) = parse_rule_offset(input)?;

    // Parse the type, any attached operator (`&MASK`), and any
    // pre-comparison value transform (`+N`/`-N`/`*N`/`/N`/`%N`/`|N`/`^N`).
    let (input, (typ, attached_op, value_transform)) = parse_type_and_operator(input)?;

    // Meta-type directives (default, clear, name, use, indirect) conceptually
    // have no operator/value operand, but magic(5) source files (including
    // GNU `file`'s own `searchbug.magic`) often write them with an `x`
    // (AnyValue) placeholder between the type and the message, e.g.
    // `>>&0 offset x at_offset %lld`. Consume an optional leading `x` token
    // here so it does not leak into the rendered message.
    //
    // `offset` is deliberately EXCLUDED from this no-operand path: unlike the
    // other meta-types, the `offset` pseudo-type carries a real comparison
    // operand -- `offset >48` / `offset <48` (gzip's trailing-size gate) read
    // the resolved offset and compare it against N, and only the `offset x`
    // form is a bare AnyValue placeholder. Routing `offset` through the normal
    // operator+value+message path below handles both forms; the engine then
    // applies the comparison (see the `MetaType::Offset` dispatch in
    // `evaluator::engine`). Folding `offset >48` into this block instead
    // silently turned `>48` into message text and made the compare a no-op.
    //
    // `name`/`use` are handled earlier in parse_type_and_operator and
    // already consumed their identifier operand, so the `x` stripping
    // is a no-op for them.
    if matches!(typ, TypeKind::Meta(_)) && !matches!(typ, TypeKind::Meta(MetaType::Offset)) {
        // Meta-type directives have no operand, so an attached operator
        // like `default&0xf` is malformed — reject it here rather than
        // silently dropping it on the floor. `name`/`use` short-circuit in
        // `parse_type_and_operator` and never carry an attached op, so only
        // `default`/`clear`/`indirect`/`offset` can trip this.
        if attached_op.is_some() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            )));
        }
        let input = strip_optional_x_operator(input);
        let (input, message) = if input.trim().is_empty() {
            (input, String::new())
        } else {
            parse_message(input)?
        };
        let rule = MagicRule {
            offset,
            typ,
            op: Operator::AnyValue,
            value: Value::Uint(0),
            message,
            children: vec![],
            level,
            strength_modifier: None,
            value_transform: None,
        };
        return Ok((input, rule));
    }

    // Try to parse a separate operator (optional - use attached operator if present)
    let (input, separate_op) = opt(parse_operator).parse(input)?;

    // When the type carried `&MASK` (encoded as `BitwiseAndMask`) AND a
    // separate operator (`x`, `>`, `!=`, ...) was parsed, magic(5)
    // semantics require treating the mask as a pre-comparison transform
    // rather than a fused mask-and-equal operator. Promote the mask to
    // a `ValueTransform { BitAnd, mask }` so the read value is masked
    // before the comparison runs and before printf-style format
    // substitution sees the value. The legacy `&MASK VALUE` form (no
    // separate op) keeps using `Operator::BitwiseAndMask` for backwards
    // compatibility with existing tests/built-in rules.
    let (op, value_transform) = match (attached_op, separate_op) {
        (Some(Operator::BitwiseAndMask(mask)), Some(separate)) => {
            // Mixing `&MASK` with the existing `+N`/`-N` value-transform
            // syntax on the same rule is not allowed: only one transform
            // per rule. Reject at parse time with a clean error.
            if value_transform.is_some() {
                return Err(nom::Err::Error(NomError::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )));
            }
            #[allow(clippy::cast_possible_wrap)]
            let promoted = crate::parser::ast::ValueTransform {
                op: crate::parser::ast::ValueTransformOp::BitAnd,
                operand: mask as i64,
            };
            (separate, Some(promoted))
        }
        (Some(attached), _) => (attached, value_transform),
        (None, Some(separate)) => (separate, value_transform),
        (None, None) => (Operator::Equal, value_transform),
    };

    // For AnyValue (`x`), no operand is needed -- treat remaining text as message.
    // For string-family types, fall back to a bare (unquoted) single-token
    // literal if the strict `parse_value` alternatives all fail. magic(5)
    // syntax permits writing `string TEST` or `search/12 ABC` without
    // surrounding quotes, and this fallback supports that form without
    // relaxing value parsing for non-string types (where `xyz` must
    // still be rejected -- see `test_parse_value_invalid_input`).
    let is_string_family_type = matches!(
        typ,
        TypeKind::String { .. }
            | TypeKind::String16 { .. }
            | TypeKind::PString { .. }
            | TypeKind::Regex { .. }
            | TypeKind::Search { .. }
    );
    let (input, value) = if op == Operator::AnyValue {
        (input, Value::Uint(0))
    } else if matches!(typ, TypeKind::Regex { .. }) {
        // `regex` patterns get special-cased ahead of the generic
        // string-family fallback below (issue: getstr fidelity fix).
        // Quoted values (`regex/c "hello" ...`) keep using
        // `parse_value`'s existing `parse_quoted_string` path unchanged
        // -- quoting is a project convenience layered on top of
        // magic(5), not part of GNU `file`'s own syntax, and existing
        // quoted-regex rules must keep their current (non-getstr)
        // escape handling. Bareword (unquoted) patterns are routed
        // through the dedicated getstr resolver instead of
        // `parse_hex_bytes`/`parse_bare_string_value`: a pattern
        // beginning with a magic(5) escape (`\^`, `\040`, `\t`, `\x..`)
        // would otherwise be captured by `parse_hex_bytes` as
        // `Value::Bytes` before any string interpretation ran, and
        // Rust's `regex` crate does not interpret octal escapes the way
        // GNU `file`'s `getstr` does -- see `getstr.rs` module docs.
        if input.trim_start().starts_with('"') {
            parse_value(input)?
        } else {
            match getstr::parse_regex_getstr_value(input) {
                Ok(ok) => ok,
                Err(orig_err) => parse_value(input).map_err(|_| orig_err)?,
            }
        }
    } else if is_string_family_type {
        parse_string_family_value(input)?
    } else {
        parse_value(input)?
    };

    // Parse the message (optional - everything remaining on the line)
    let (input, message) = if input.trim().is_empty() {
        (input, String::new())
    } else {
        parse_message(input)?
    };

    let rule = MagicRule {
        offset,
        typ,
        op,
        value,
        message,
        children: vec![], // Children will be added during hierarchical parsing
        level,
        strength_modifier: None, // Will be set during directive parsing
        value_transform,
    };

    Ok((input, rule))
}

#[cfg(test)]
mod tests;
