// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Offset specification parsing for magic file rules.
//!
//! Covers absolute, from-end, relative, and indirect offset syntax
//! (`parse_offset`), plus the `>`-prefixed nesting-level parsing used by
//! [`parse_rule_offset`]. Extracted from `grammar/mod.rs` as a pure
//! code-motion split (issue #391 Unit U4) -- no behavior changes.

use nom::{
    IResult, Parser,
    character::complete::{char, multispace0, one_of},
    combinator::opt,
    error::Error as NomError,
    multi::many0,
};

use log::warn;

use crate::parser::ast::{Endianness, IndirectAdjustmentOp, OffsetSpec, TypeKind};

use super::parse_number;

/// Map a single-character pointer specifier to its `TypeKind` and `Endianness`.
///
/// GNU `file` semantics: lowercase = little-endian, uppercase = big-endian.
/// Numeric pointer types are signed by default per GOTCHAS S6.3.
///
/// | Specifier | Width  | Endianness    |
/// |-----------|--------|---------------|
/// | `b`       | 1 byte | Little-endian |
/// | `B`       | 1 byte | Big-endian    |
/// | `s`       | 2 byte | Little-endian |
/// | `S`       | 2 byte | Big-endian    |
/// | `l`       | 4 byte | Little-endian |
/// | `L`       | 4 byte | Big-endian    |
/// | `q`       | 8 byte | Little-endian |
/// | `Q`       | 8 byte | Big-endian    |
fn pointer_specifier_to_type(spec: char) -> Option<(TypeKind, Endianness)> {
    match spec {
        'b' => Some((TypeKind::Byte { signed: true }, Endianness::Little)),
        'B' => Some((TypeKind::Byte { signed: true }, Endianness::Big)),
        's' => Some((
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )),
        'S' => Some((
            TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        )),
        'l' => Some((
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )),
        'L' => Some((
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        )),
        'q' => Some((
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )),
        'Q' => Some((
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        )),
        // `i` and `I` are magic(5) "ID3 variable-byte int" pointer
        // specifiers used in audio:308 for ID3 frame size decoding.
        // We parse them so the magic file loads, but for now treat
        // them as plain 32-bit longs with the corresponding endianness
        // -- real ID3 7-bit-per-byte decoding is a follow-up. Tracked
        // separately as a parsing-vs-semantics gap. The bodies match
        // `l`/`L` exactly today; clippy::match_same_arms is allowed
        // because the arms are intentionally distinct entry points
        // that future ID3-decoding work will diverge.
        #[allow(clippy::match_same_arms)]
        'i' => Some((
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )),
        #[allow(clippy::match_same_arms)]
        'I' => Some((
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            Endianness::Big,
        )),
        _ => None,
    }
}

/// Parse an indirect offset specification with optional arithmetic.
///
/// Accepts these forms:
///
/// - `(base.type)` — no adjustment
/// - `(base.type+N)` / `(base.type-N)` — additive (canonical magic(5))
/// - `(base.type*N)` / `(base.type/N)` / `(base.type%N)` — multiplicative
/// - `(base.type&N)` / `(base.type|N)` / `(base.type^N)` — bitwise
/// - `(base.type)+N` / `(base.type)-N` — additive outside the parens
///   (backwards-compatible alternate form; only `+`/`-` are accepted here)
///
/// Only one adjustment form may be used per rule; combinations like
/// `(19.b-1)+2` or `(0x200.s*2)+4` are not permitted. Subtraction is
/// represented as [`IndirectAdjustmentOp::Add`] with a negative
/// `adjustment`.
fn parse_indirect_offset(input: &str) -> IResult<&str, OffsetSpec> {
    // Inside-paren adjustment supports the full magic(5) operator set.
    // Returns `Some((op, value))` when an operator+operand was consumed.
    //
    // Operands may optionally be wrapped in their own parentheses, e.g.
    // `(0x10.l+(-4))` is equivalent to `(0x10.l-4)`. GNU `file` magic
    // files use this form when a sign character would otherwise be
    // ambiguous with the operator (e.g., `+-4`); the parens make the
    // grouping explicit.
    fn parse_operand(input: &str) -> IResult<&str, i64> {
        if let Some(rest) = input.strip_prefix('(') {
            let (rest, n) = parse_number(rest)?;
            let (rest, _) = char(')')(rest)?;
            Ok((rest, n))
        } else {
            parse_number(input)
        }
    }
    fn parse_inside_adjustment(input: &str) -> IResult<&str, Option<(IndirectAdjustmentOp, i64)>> {
        // Subtraction is folded into Add with a negated operand so the
        // evaluator does not need a dedicated Sub variant.
        if let Some(rest) = input.strip_prefix('+') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Add, n))))
        } else if input.starts_with('-') {
            let (rest, n) = parse_number(input)?;
            Ok((rest, Some((IndirectAdjustmentOp::Add, n))))
        } else if let Some(rest) = input.strip_prefix('*') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Mul, n))))
        } else if let Some(rest) = input.strip_prefix('/') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Div, n))))
        } else if let Some(rest) = input.strip_prefix('%') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Mod, n))))
        } else if let Some(rest) = input.strip_prefix('&') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::And, n))))
        } else if let Some(rest) = input.strip_prefix('|') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Or, n))))
        } else if let Some(rest) = input.strip_prefix('^') {
            let (rest, n) = parse_operand(rest)?;
            Ok((rest, Some((IndirectAdjustmentOp::Xor, n))))
        } else {
            Ok((input, None))
        }
    }

    // Outside-paren adjustment: only `+`/`-` are accepted (legacy form).
    fn parse_outside_adjustment(input: &str) -> IResult<&str, Option<i64>> {
        if let Some(rest) = input.strip_prefix('+') {
            let (rest, n) = parse_number(rest)?;
            Ok((rest, Some(n)))
        } else if input.starts_with('-') {
            let (rest, n) = parse_number(input)?;
            Ok((rest, Some(n)))
        } else {
            Ok((input, None))
        }
    }

    let (input, _) = char('(')(input)?;

    // magic(5) lets the indirect base itself be relative to the current
    // anchor: `(&N.X)` means "read pointer at anchor + N". Detect the
    // optional leading `&` and record the flag; the rest of the parser
    // handles the numeric base offset uniformly.
    let (input, base_relative) = if let Some(rest) = input.strip_prefix('&') {
        (rest, true)
    } else {
        (input, false)
    };

    let (input, base_offset) = parse_number(input)?;
    // magic(5) canonical separator is `.`. `/usr/share/file/magic/msdos`
    // line 638 uses `,` -- a known typo that GNU `file` warns about
    // but tolerates ("No current entry for continuation"). Accept
    // either character so the magic file loads, but emit a warn! when
    // the comma path is taken so users see the typo at default log
    // levels (matching GNU `file`'s diagnostic posture).
    // The pointer specifier is optional. magic(5) writes `(base.type)`, but
    // the bare `(base)` form is accepted by GNU `file` and appears in real
    // magic (`games:74`'s `>>(56) indirect x`); libmagic defaults `in_type`
    // to `FILE_LONG` in host byte order. Without this the rule fails to parse
    // and the tolerant loader drops it along with its children.
    let (input, spec) = opt((one_of(".,"), one_of("bBsSlLqQiI"))).parse(input)?;

    let (pointer_type, endian) = match spec {
        Some((sep, spec_char)) => {
            if sep == ',' {
                warn!(
                    "Indirect offset uses ',' as separator (magic(5) requires '.'); \
                     accepting for GNU `file` typo-tolerance compatibility"
                );
            }
            pointer_specifier_to_type(spec_char).ok_or_else(|| {
                nom::Err::Error(NomError::new(input, nom::error::ErrorKind::OneOf))
            })?
        }
        None => (
            TypeKind::Long {
                endian: Endianness::Native,
                signed: true,
            },
            Endianness::Native,
        ),
    };

    let (input, inside) = parse_inside_adjustment(input)?;
    let (input, _) = char(')')(input)?;

    // Fall back to outside-paren adjustment if no inside form was present.
    let (input, adjustment_op, adjustment) = if let Some((op, n)) = inside {
        (input, op, n)
    } else {
        let (input, outside) = parse_outside_adjustment(input)?;
        (input, IndirectAdjustmentOp::Add, outside.unwrap_or(0))
    };

    Ok((
        input,
        OffsetSpec::Indirect {
            base_offset,
            base_relative,
            pointer_type,
            adjustment,
            adjustment_op,
            result_relative: false,
            endian,
        },
    ))
}

/// Parse an offset specification (absolute or indirect)
///
/// Supports:
/// - Absolute offsets: decimal and hexadecimal, positive and negative
/// - Indirect offsets: `(base.type)` or `(base.type)+adj` syntax
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_offset;
/// use libmagic_rs::parser::ast::{Endianness, OffsetSpec, TypeKind};
///
/// // Absolute offsets
/// assert_eq!(parse_offset("0"), Ok(("", OffsetSpec::Absolute(0))));
/// assert_eq!(parse_offset("123"), Ok(("", OffsetSpec::Absolute(123))));
/// assert_eq!(parse_offset("0x10"), Ok(("", OffsetSpec::Absolute(16))));
/// assert_eq!(parse_offset("-4"), Ok(("", OffsetSpec::Absolute(-4))));
/// assert_eq!(parse_offset("-0xFF"), Ok(("", OffsetSpec::Absolute(-255))));
///
/// // Indirect offset (lowercase = little-endian, signed by default).
/// // `OffsetSpec::Indirect` also carries `base_relative`, `adjustment_op`,
/// // and `result_relative` (shown here at their common defaults).
/// assert_eq!(
///     parse_offset("(0x3c.l)"),
///     Ok(("", OffsetSpec::Indirect {
///         base_offset: 0x3c,
///         base_relative: false,
///         pointer_type: TypeKind::Long { endian: Endianness::Little, signed: true },
///         adjustment: 0,
///         adjustment_op: IndirectAdjustmentOp::Add,
///         result_relative: false,
///         endian: Endianness::Little,
///     }))
/// );
///
/// // Adjustment after closing paren
/// assert_eq!(
///     parse_offset("(0x3c.l)+4"),
///     Ok(("", OffsetSpec::Indirect {
///         base_offset: 0x3c,
///         base_relative: false,
///         pointer_type: TypeKind::Long { endian: Endianness::Little, signed: true },
///         adjustment: 4,
///         adjustment_op: IndirectAdjustmentOp::Add,
///         result_relative: false,
///         endian: Endianness::Little,
///     }))
/// );
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - The input contains invalid number format (propagated from `parse_number`)
/// - Input is empty or contains no parseable offset value
/// - The offset value cannot be represented as a valid `i64`
/// - Indirect offset has invalid pointer specifier or missing closing `)`
pub fn parse_offset(input: &str) -> IResult<&str, OffsetSpec> {
    let (input, _) = multispace0(input)?;

    if input.starts_with('(') {
        let (input, spec) = parse_indirect_offset(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, spec))
    } else if let Some(rest) = input.strip_prefix('&')
        && rest.starts_with('(')
    {
        // `&(...)`: relative wrapper around an indirect spec. Parse the
        // inner indirect normally, then mark its result as relative so the
        // evaluator adds it to the current anchor instead of treating it
        // as an absolute file position. magic(5) uses this in rules like
        // `&(&0.b+8)` to chain anchor-relative pointer reads.
        let (rest, mut spec) = parse_indirect_offset(rest)?;
        if let OffsetSpec::Indirect {
            ref mut result_relative,
            ..
        } = spec
        {
            *result_relative = true;
        }
        let (rest, _) = multispace0(rest)?;
        Ok((rest, spec))
    } else if let Some(rest) = input.strip_prefix('&') {
        // Relative offset: `&N`, `&+N`, or `&-N`. `parse_number` handles the
        // bare and `-`-prefixed cases natively; `+` is consumed manually
        // (see the indirect-offset adjustment parser for the same pattern).
        let (rest, value) = if let Some(after_plus) = rest.strip_prefix('+') {
            parse_number(after_plus)?
        } else {
            parse_number(rest)?
        };
        let (rest, _) = multispace0(rest)?;
        Ok((rest, OffsetSpec::Relative(value)))
    } else {
        // Capture the leading `-` before `parse_number` consumes it: magic(5)
        // `-0` means "0 bytes from the end of the file" -- the EOF position
        // (`buffer.len()`), NOT absolute offset 0. Because `-0 == 0` in a
        // signed integer the sign is otherwise lost, so detect it explicitly
        // and encode `FromEnd(0)`. Used by e.g. gzip's `>>-0 offset >48` to
        // gate the trailing-size trailer on the file being long enough.
        // Other negative offsets (`-4`) keep their `Absolute(-4)` encoding,
        // which the evaluator already resolves from the buffer end.
        let starts_with_minus = input.starts_with('-');
        let (input, offset_value) = parse_number(input)?;
        let (input, _) = multispace0(input)?;
        if starts_with_minus && offset_value == 0 {
            return Ok((input, OffsetSpec::FromEnd(0)));
        }
        Ok((input, OffsetSpec::Absolute(offset_value)))
    }
}

/// Parse the indentation level and offset for magic rules
///
/// Handles both absolute offsets and hierarchical child rules with `>` prefix.
/// Child rules can be nested multiple levels deep with multiple `>` characters.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_rule_offset;
/// use libmagic_rs::parser::ast::OffsetSpec;
///
/// // Absolute offset
/// assert_eq!(parse_rule_offset("0"), Ok(("", (0, OffsetSpec::Absolute(0)))));
/// assert_eq!(parse_rule_offset("16"), Ok(("", (0, OffsetSpec::Absolute(16)))));
///
/// // Child rule (level 1)
/// assert_eq!(parse_rule_offset(">4"), Ok(("", (1, OffsetSpec::Absolute(4)))));
///
/// // Nested child rule (level 2)
/// assert_eq!(parse_rule_offset(">>8"), Ok(("", (2, OffsetSpec::Absolute(8)))));
/// ```
/// Parse rule offset with hierarchy level (> prefixes) and offset specification
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected offset format
pub fn parse_rule_offset(input: &str) -> IResult<&str, (u32, OffsetSpec)> {
    let (input, _) = multispace0(input)?;

    // Count the number of '>' characters for nesting level
    let (input, level_chars) = many0(char('>')).parse(input)?;
    let level = u32::try_from(level_chars.len()).unwrap_or(0);

    // Parse the offset after the '>' characters
    let (input, offset_spec) = parse_offset(input)?;

    Ok((input, (level, offset_spec)))
}
