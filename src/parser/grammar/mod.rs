// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Grammar parsing for magic files using nom parser combinators
//!
//! This module implements the parsing logic for magic file syntax, converting
//! text-based magic rules into the AST representation defined in ast.rs.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, multispace0, one_of},
    combinator::opt,
    error::Error as NomError,
    multi::many0,
    sequence::preceded,
};

use log::warn;

use crate::parser::ast::{
    Endianness, IndirectAdjustmentOp, MagicRule, MetaType, OffsetSpec, Operator, StrengthModifier,
    TypeKind, Value,
};

mod numbers;
mod type_suffix;
mod value;

pub use numbers::parse_number;
pub use value::parse_value;

use numbers::parse_decimal_number;
#[cfg(test)]
use numbers::parse_hex_number;
use type_suffix::{
    parse_attached_operator, parse_pstring_suffix, parse_regex_suffix, parse_search_suffix,
    parse_value_transform,
};
#[cfg(test)]
use value::{parse_escape_sequence, parse_hex_bytes, parse_numeric_value, parse_quoted_string};

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
    let (input, sep) = one_of(".,").parse(input)?;
    if sep == ',' {
        warn!(
            "Indirect offset uses ',' as separator (magic(5) requires '.'); \
             accepting for GNU `file` typo-tolerance compatibility"
        );
    }
    let (input, spec_char) = one_of("bBsSlLqQiI")(input)?;

    let (pointer_type, endian) = pointer_specifier_to_type(spec_char)
        .ok_or_else(|| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::OneOf)))?;

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
/// // Indirect offset (lowercase = little-endian, signed by default)
/// assert_eq!(
///     parse_offset("(0x3c.l)"),
///     Ok(("", OffsetSpec::Indirect {
///         base_offset: 0x3c,
///         pointer_type: TypeKind::Long { endian: Endianness::Little, signed: true },
///         adjustment: 0,
///         endian: Endianness::Little,
///     }))
/// );
///
/// // Adjustment after closing paren
/// assert_eq!(
///     parse_offset("(0x3c.l)+4"),
///     Ok(("", OffsetSpec::Indirect {
///         base_offset: 0x3c,
///         pointer_type: TypeKind::Long { endian: Endianness::Little, signed: true },
///         adjustment: 4,
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
        let (input, offset_value) = parse_number(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, OffsetSpec::Absolute(offset_value)))
    }
}

/// Parse comparison operators for magic rules
///
/// Supports both symbolic and text representations of operators:
/// - `=` or `==` for equality
/// - `!=` or `<>` for inequality
/// - `<` for less-than
/// - `>` for greater-than
/// - `<=` for less-than-or-equal
/// - `>=` for greater-than-or-equal
/// - `&` for bitwise AND
/// - `^` for bitwise XOR
/// - `~` for bitwise NOT
/// - `x` for any value (always matches)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_operator;
/// use libmagic_rs::parser::ast::Operator;
///
/// assert_eq!(parse_operator("="), Ok(("", Operator::Equal)));
/// assert_eq!(parse_operator("=="), Ok(("", Operator::Equal)));
/// assert_eq!(parse_operator("!="), Ok(("", Operator::NotEqual)));
/// assert_eq!(parse_operator("<>"), Ok(("", Operator::NotEqual)));
/// assert_eq!(parse_operator("<"), Ok(("", Operator::LessThan)));
/// assert_eq!(parse_operator(">"), Ok(("", Operator::GreaterThan)));
/// assert_eq!(parse_operator("<="), Ok(("", Operator::LessEqual)));
/// assert_eq!(parse_operator(">="), Ok(("", Operator::GreaterEqual)));
/// assert_eq!(parse_operator("&"), Ok(("", Operator::BitwiseAnd)));
/// assert_eq!(parse_operator("^"), Ok(("", Operator::BitwiseXor)));
/// assert_eq!(parse_operator("~"), Ok(("", Operator::BitwiseNot)));
/// assert_eq!(parse_operator("x"), Ok(("", Operator::AnyValue)));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input does not start with a recognized operator symbol
/// - Input is empty or contains no valid operator
/// - Operator syntax is incomplete (e.g., just `!` without `=`)
pub fn parse_operator(input: &str) -> IResult<&str, Operator> {
    let (input, _) = multispace0(input)?;

    let bytes = input.as_bytes();
    let err = || nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag));

    // Dispatch on the first byte and inspect the second byte to choose between
    // long-form and short-form operators. Boundary checks reject invalid
    // sequences like "===", "&&", "^^", "~~", and "x42".
    let (op, consumed) = match bytes.first().copied() {
        Some(b'=') => {
            // "=" or "==" -- reject "===" (and longer runs of '=')
            if bytes.get(1).copied() == Some(b'=') {
                if bytes.get(2).copied() == Some(b'=') {
                    return Err(err());
                }
                (Operator::Equal, 2)
            } else {
                (Operator::Equal, 1)
            }
        }
        // "!=" or bare "!" -- both map to NotEqual. magic(5) uses the bare
        // form (e.g., `!0xb8c0078e` means "not equal to 0xb8c0078e"); the
        // `!=` form is accepted as a convenience and matches operators in
        // other parts of this parser.
        Some(b'!') => {
            if bytes.get(1).copied() == Some(b'=') {
                (Operator::NotEqual, 2)
            } else {
                (Operator::NotEqual, 1)
            }
        }
        Some(b'<') => {
            // "<=", "<>", or bare "<"
            match bytes.get(1).copied() {
                Some(b'=') => (Operator::LessEqual, 2),
                Some(b'>') => (Operator::NotEqual, 2),
                _ => (Operator::LessThan, 1),
            }
        }
        Some(b'>') => {
            // ">=" or bare ">"
            if bytes.get(1).copied() == Some(b'=') {
                (Operator::GreaterEqual, 2)
            } else {
                (Operator::GreaterThan, 1)
            }
        }
        Some(b'&') => {
            // Reject "&&"
            if bytes.get(1).copied() == Some(b'&') {
                return Err(err());
            }
            (Operator::BitwiseAnd, 1)
        }
        Some(b'^') => {
            // Reject "^^"
            if bytes.get(1).copied() == Some(b'^') {
                return Err(err());
            }
            (Operator::BitwiseXor, 1)
        }
        Some(b'~') => {
            // Reject "~~"
            if bytes.get(1).copied() == Some(b'~') {
                return Err(err());
            }
            (Operator::BitwiseNot, 1)
        }
        Some(b'x') => {
            // Word boundary: 'x' must not be followed by an alphanumeric or '_'
            // (e.g., "x42" or "xfoo" is not AnyValue).
            if input
                .get(1..)
                .is_some_and(|s| s.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
            {
                return Err(err());
            }
            (Operator::AnyValue, 1)
        }
        _ => return Err(err()),
    };

    let remaining = &input[consumed..];
    let (remaining, _) = multispace0(remaining)?;
    Ok((remaining, op))
}

/// Parse the identifier operand of a `name` / `use` meta-type directive.
///
/// Called from [`parse_type_and_operator`] when the leading keyword is
/// `name` or `use`. Enforces that the keyword is followed by whitespace,
/// an identifier matching `[A-Za-z0-9_-]+`, and no further non-whitespace
/// content on the line. Malformed identifiers such as `part2=foo`
/// (operator-adjacent continuation) or `part 2` (split identifier) are
/// rejected as parse errors rather than silently consumed as a message.
fn parse_name_or_use_meta<'a>(
    type_name: &str,
    input: &'a str,
) -> IResult<
    &'a str,
    (
        TypeKind,
        Option<Operator>,
        Option<crate::parser::ast::ValueTransform>,
    ),
> {
    use nom::character::complete::space1;

    // Require at least one whitespace character between the keyword and
    // the identifier. `space1` rejects an empty gap, which enforces
    // "bare `name` / `use` with no identifier" as a parse error.
    let (input, _) = space1(input)?;

    // magic(5) allows a `\^` prefix on a `use` identifier to mean "invoke
    // the named subroutine but flip the endianness of every read inside
    // it". We do not yet implement the endian flip semantically (tracked
    // as issue #236), but the file must still load: consume the prefix
    // and treat the rest of the identifier as a normal `use` reference.
    // Emit a warn! so users see why their LE/BE detection paired with
    // `use \^name` produces wrong metadata at default log levels.
    let input = if type_name == "use" {
        if let Some(rest) = input.strip_prefix("\\^") {
            warn!(
                "use directive with `\\^` prefix: endian-flip semantics \
                 are not yet implemented (issue #236). Subroutine reads \
                 will use their declared endianness; metadata fields may \
                 be incorrect. Identifier: {:?}",
                rest.split_whitespace().next().unwrap_or("")
            );
            rest
        } else {
            input
        }
    } else {
        input
    };

    let (after_id, id) =
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-').parse(input)?;
    if id.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            after_id,
            nom::error::ErrorKind::AlphaNumeric,
        )));
    }

    // The character immediately following the identifier must be
    // whitespace or end-of-input. Anything else (e.g. `=`, `!`, `<`,
    // `>`, `&`, `^`, `~`, `|`, punctuation) means `take_while` truncated
    // a malformed identifier such as `part2=foo`: reject instead of
    // silently treating the leftover text as a message.
    if let Some(next_char) = after_id.chars().next()
        && !matches!(next_char, ' ' | '\t' | '\n' | '\r')
    {
        return Err(nom::Err::Error(NomError::new(
            after_id,
            nom::error::ErrorKind::Alpha,
        )));
    }

    // Consume horizontal whitespace after the identifier. Real-world
    // magic files sometimes append a descriptive message after a
    // `name`/`use` directive (e.g. `0 name xbase-prf dBase Printer
    // Form`). magic(5) does not officially document this, but GNU
    // `file` tolerates it -- the trailing text is silently ignored
    // because `name`/`use` rules don't have a message slot in the
    // softmagic struct. We do the same: consume horizontal whitespace
    // and a single optional trailing token, stopping at end-of-line.
    // We deliberately do NOT reject embedded whitespace inside the
    // identifier itself (which would be a real malformed rule like
    // `part 2`); that's enforced earlier when `take_while` truncates
    // the identifier on the first non-id character.
    let mut tail = after_id;
    while let Some(rest) = tail.strip_prefix(' ').or_else(|| tail.strip_prefix('\t')) {
        tail = rest;
    }
    // Skip any trailing text (descriptive label) up to end-of-line.
    if let Some(next_char) = tail.chars().next()
        && !matches!(next_char, '\n' | '\r')
    {
        // Drop the rest of the line silently. magic(5)'s `name`/`use`
        // directives have no message slot, so anything after the
        // identifier is informational only.
        let line_end = tail.find(['\n', '\r']).unwrap_or(tail.len());
        tail = &tail[line_end..];
    }

    let meta = if type_name == "name" {
        MetaType::Name(id.to_string())
    } else {
        MetaType::Use(id.to_string())
    };
    let (rest, _) = multispace0(tail)?;
    Ok((rest, (TypeKind::Meta(meta), None, None)))
}

/// Parse a type specification with an optional attached bitwise-AND mask operator
/// (e.g., `lelong&0xf0000000`).
///
/// Returns the `TypeKind`, an optional attached `Operator` (`&MASK`), and an
/// optional pre-comparison `ValueTransform` (`+N`, `-N`, `*N`, `/N`, `%N`,
/// `|N`, `^N`).
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_type_and_operator;
/// use libmagic_rs::parser::ast::{TypeKind, Operator, Endianness};
///
/// // Type without operator or transform
/// let (_, (kind, op, transform)) = parse_type_and_operator("lelong").unwrap();
/// assert_eq!(kind, TypeKind::Long { endian: Endianness::Little, signed: true });
/// assert_eq!(op, None);
/// assert_eq!(transform, None);
///
/// // Type with mask operator
/// let (_, (kind, op, _)) = parse_type_and_operator("lelong&0xf0000000").unwrap();
/// assert!(matches!(op, Some(Operator::BitwiseAndMask(_))));
///
/// // Type with arithmetic transform
/// let (_, (kind, op, transform)) = parse_type_and_operator("lelong+1").unwrap();
/// assert_eq!(op, None);
/// assert!(transform.is_some());
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected format
pub fn parse_type_and_operator(
    input: &str,
) -> IResult<
    &str,
    (
        TypeKind,
        Option<Operator>,
        Option<crate::parser::ast::ValueTransform>,
    ),
> {
    use crate::parser::ast::{PStringLengthWidth, RegexCount, RegexFlags};

    let (input, _) = multispace0(input)?;

    let (mut input, type_name) = crate::parser::types::parse_type_keyword(input)?;

    // `name` and `use` are meta-type directives with a mandatory
    // identifier suffix. They short-circuit the operator/value parse
    // path via `parse_name_or_use_meta`, which also rejects malformed
    // identifiers (operator-adjacent continuations like `part2=foo` or
    // split identifiers like `part 2`).
    if type_name == "name" || type_name == "use" {
        return parse_name_or_use_meta(type_name, input);
    }

    // Handle pstring suffixes: /B, /H, /h, /L, /l, and optional /J modifier
    let mut pstring_length_width = PStringLengthWidth::OneByte;
    let mut pstring_length_includes_itself = false;
    if type_name == "pstring"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, width, includes_j) = parse_pstring_suffix(suffix_rest)?;
        input = rest;
        pstring_length_width = width;
        pstring_length_includes_itself = includes_j;
    }

    // Handle regex suffixes via the extracted helper. See
    // `grammar/type_suffix.rs::parse_regex_suffix` for the full
    // "any-order flag/count interleaving, duplicate counts rejected"
    // semantics and the `RegexCount` collapse logic.
    let mut regex_flags = RegexFlags::default();
    let mut regex_count = RegexCount::Default;
    if type_name == "regex"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, (flags, count)) = parse_regex_suffix(input, suffix_rest)?;
        regex_flags = flags;
        regex_count = count;
        input = rest;
    }

    // Handle search suffix: required decimal range plus optional flags
    // (e.g., `search/256`, `search/256/s`, `search/256/cs`). Per GNU
    // `file` magic(5), the range is mandatory. `search/0` and bare
    // `search` are rejected at parse time via `NonZeroUsize`.
    let mut search_suffix: Option<(::std::num::NonZeroUsize, crate::parser::ast::SearchFlags)> =
        None;
    if type_name == "search"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let (rest, parsed) = parse_search_suffix(input, suffix_rest)?;
        search_suffix = Some(parsed);
        input = rest;
    }

    // Handle string flag suffixes (e.g., `string/w`, `string/cW`).
    // magic(5) flag letters per libmagic `src/file.h`:
    //   `/W` STRING_COMPACT_WHITESPACE
    //   `/w` STRING_COMPACT_OPTIONAL_WHITESPACE
    //   `/c` STRING_IGNORE_LOWERCASE  (pattern lowercase => file folded)
    //   `/C` STRING_IGNORE_UPPERCASE  (pattern uppercase => file folded)
    //   `/t` STRING_TEXTTEST          (text-mode hint)
    //   `/T` STRING_TRIM              (trim pattern leading/trailing ws)
    //   `/b` STRING_BINTEST           (binary-mode hint)
    //   `/f` STRING_FULL_WORD         (post-match word-boundary check)
    //
    // `/B` is deliberately NOT accepted here -- it is the pstring
    // 1-byte length-width letter (`CHAR_PSTRING_1_BE`) and is not a
    // string flag in libmagic. An earlier draft of this loop
    // accepted `'B'`; that was wrong and is now rejected. See
    // GOTCHAS S6.6.
    let mut string_flags = crate::parser::ast::StringFlags::default();
    if type_name == "string"
        && let Some(suffix_rest) = input.strip_prefix('/')
    {
        let mut consumed = 0usize;
        for ch in suffix_rest.chars() {
            match ch {
                'W' => string_flags.compact_whitespace = true,
                'w' => string_flags.compact_optional_whitespace = true,
                'c' => string_flags.ignore_lowercase = true,
                'C' => string_flags.ignore_uppercase = true,
                't' => string_flags.text_test = true,
                'T' => string_flags.trim = true,
                'b' => string_flags.bin_test = true,
                'f' => string_flags.full_word = true,
                _ => break,
            }
            consumed += ch.len_utf8();
        }
        if consumed > 0 {
            input = &suffix_rest[consumed..];
        }
        // If `consumed == 0`, the `/` is not followed by a known flag
        // letter. Leave `input` pointing at the `/` so the value
        // parser (or the trailing-junk check) fails meaningfully. This
        // is the path that rejects `string/B` and `string/x`.
    }

    // Check for a pre-comparison value transform (e.g., `lelong+1` or
    // `ulequad/1073741824`). magic(5) supports `+`, `-`, `*`, `/`, `%`,
    // `|`, and `^` between the type keyword and the comparison value;
    // the transform runs on the read value before the comparison
    // operator and before printf-style format substitution.
    let (input, value_transform) = parse_value_transform(input)?;

    // Check for an attached bitwise operator with optional mask (e.g.,
    // `&0xf0000000` or bare `&`). See `type_suffix::parse_attached_operator`
    // for the recognized forms and their error behavior. magic(5) does
    // not allow combining `&MASK` with another value transform on the
    // same rule, so the parsers are sequential and either-or in
    // practice.
    let (input, attached_op) = parse_attached_operator(input)?;

    let (input, _) = multispace0(input)?;

    // Build Regex/Search directly from the parsed suffixes; fall back to
    // `type_keyword_to_kind` for every other type. PString still uses the
    // patch-after-construct pattern because `type_keyword_to_kind` supplies
    // its `max_length` default and the suffix parser only produces the
    // length-width and `/J` flag.
    let type_kind = match type_name {
        "regex" => TypeKind::Regex {
            flags: regex_flags,
            count: regex_count,
        },
        "search" => {
            // Mandatory range: reject bare `search` at parse time.
            let (range, flags) = search_suffix.ok_or_else(|| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag))
            })?;
            TypeKind::Search { range, flags }
        }
        _ => {
            // `type_keyword_to_kind` returns:
            //  * `Ok(Some(kind))` for every fully-specified keyword
            //    (byte, short, long, quad, float/double, dates,
            //    string, pstring and variants).
            //  * `Ok(None)` for suffix-required keywords (`regex`,
            //    `search`), which are handled by the match arms above
            //    and should never reach this branch.
            //  * `Err(UnknownTypeKeyword)` for a keyword that was never
            //    produced by `parse_type_keyword`. Under the grammar's
            //    normal flow this is unreachable because `type_name`
            //    was just returned by `parse_type_keyword`, but the
            //    function is `pub` and we do not rely on panics to
            //    enforce the invariant -- we convert both "shouldn't
            //    happen" cases into a nom parse error anchored at the
            //    current input position so the parser can backtrack or
            //    report a clean failure without aborting the process.
            let Ok(Some(mut kind)) = crate::parser::types::type_keyword_to_kind(type_name) else {
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )));
            };
            if let TypeKind::PString { max_length, .. } = kind {
                kind = TypeKind::PString {
                    max_length,
                    length_width: pstring_length_width,
                    length_includes_itself: pstring_length_includes_itself,
                };
            }
            // Stamp the parsed string flags onto the `string` variant.
            // `type_keyword_to_kind` returns `flags: StringFlags::default()`
            // because the flag-bearing suffix is grammar-layer only; the
            // type-keyword layer never sees it.
            if let TypeKind::String { max_length, .. } = kind {
                kind = TypeKind::String {
                    max_length,
                    flags: string_flags,
                };
            }
            kind
        }
    };

    Ok((input, (type_kind, attached_op, value_transform)))
}

/// Parse a type specification (byte, short, long, quad, string, etc.)
///
/// Supports various type formats found in magic files:
/// - `byte` / `ubyte` - single byte (signed / unsigned)
/// - `short` / `ushort` - 16-bit integer (native endian, signed / unsigned)
/// - `leshort` / `uleshort` - 16-bit little-endian integer
/// - `beshort` / `ubeshort` - 16-bit big-endian integer
/// - `long` / `ulong` - 32-bit integer (native endian, signed / unsigned)
/// - `lelong` / `ulelong` - 32-bit little-endian integer
/// - `belong` / `ubelong` - 32-bit big-endian integer
/// - `quad` / `uquad` - 64-bit integer (native endian, signed / unsigned)
/// - `lequad` / `ulequad` - 64-bit little-endian integer
/// - `bequad` / `ubequad` - 64-bit big-endian integer
/// - `string` - null-terminated string
/// - `pstring` - Pascal string (length-prefixed, supports `/B` (1-byte, default), `/H` or `/h` (2-byte), `/L` or `/l` (4-byte) suffixes)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_type;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness};
///
/// assert_eq!(parse_type("byte"), Ok(("", TypeKind::Byte { signed: true })));
/// assert_eq!(parse_type("leshort"), Ok(("", TypeKind::Short { endian: Endianness::Little, signed: true })));
/// assert_eq!(parse_type("bequad"), Ok(("", TypeKind::Quad { endian: Endianness::Big, signed: true })));
/// assert_eq!(parse_type("string"), Ok(("", TypeKind::String { max_length: None, flags: StringFlags::default() })));
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match any known type
#[allow(dead_code)] // Standalone helper exercised by grammar unit tests.
pub fn parse_type(input: &str) -> IResult<&str, TypeKind> {
    let (input, (type_kind, _, _)) = parse_type_and_operator(input)?;
    Ok((input, type_kind))
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

/// Parse the message part of a magic rule
///
/// The message is everything after the value until the end of the line.
/// It may contain format specifiers and can be empty.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_message;
///
/// assert_eq!(parse_message("ELF executable"), Ok(("", "ELF executable".to_string())));
/// assert_eq!(parse_message(""), Ok(("", "".to_string())));
/// assert_eq!(parse_message("  \tPDF document  "), Ok(("", "PDF document".to_string())));
/// ```
/// Parse the message/description part of a magic rule
///
/// # Errors
/// Returns a nom parsing error if the input cannot be parsed as a message
pub fn parse_message(input: &str) -> IResult<&str, String> {
    let (input, _) = multispace0(input)?;

    // Take everything until end of line, trimming whitespace
    // Use take_while instead of take_while1 to handle empty messages
    let (input, message_text) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
    let message = message_text.trim().to_string();

    Ok((input, message))
}

/// Parse a strength directive (`!:strength` line)
///
/// Parses the `!:strength` directive that modifies rule strength.
/// Format: `!:strength [+|-|*|/|=]N` or `!:strength N`
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_strength_directive;
/// use libmagic_rs::parser::ast::StrengthModifier;
///
/// assert_eq!(parse_strength_directive("!:strength +10"), Ok(("", StrengthModifier::Add(10))));
/// assert_eq!(parse_strength_directive("!:strength -5"), Ok(("", StrengthModifier::Subtract(5))));
/// assert_eq!(parse_strength_directive("!:strength *2"), Ok(("", StrengthModifier::Multiply(2))));
/// assert_eq!(parse_strength_directive("!:strength /2"), Ok(("", StrengthModifier::Divide(2))));
/// assert_eq!(parse_strength_directive("!:strength =50"), Ok(("", StrengthModifier::Set(50))));
/// assert_eq!(parse_strength_directive("!:strength 50"), Ok(("", StrengthModifier::Set(50))));
/// ```
///
/// # Errors
///
/// Returns a nom parsing error if:
/// - Input doesn't start with `!:strength`
/// - The modifier value cannot be parsed as a valid integer
/// - The operator is invalid
pub fn parse_strength_directive(input: &str) -> IResult<&str, StrengthModifier> {
    // Helper to safely convert i64 to i32 with clamping to valid strength range.
    // This prevents silent truncation to 0 on overflow while keeping values in bounds.
    // Clamping to `[i32::MIN, i32::MAX]` is lossless via `as i32`, so no
    // `unwrap()`/`expect()` is needed (AGENTS.md bans panic markers in
    // library code regardless of whether the unwrap is provably safe).
    #[allow(clippy::cast_possible_truncation)]
    fn clamp_to_i32(n: i64) -> i32 {
        n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    let (input, _) = multispace0(input)?;
    let (input, _) = tag("!:strength")(input)?;
    let (input, _) = multispace0(input)?;

    // Parse the operator: +, -, *, /, = or bare number (implies =).
    // Optional whitespace is permitted between the operator character and
    // the operand to match GNU `file` magic(5) parsers, which accept forms
    // like `!:strength / 2` in real-world magic files (e.g., the Minix
    // entries in /usr/share/file/magic/filesystems).
    //
    // Use `preceded` + `Parser::map` (nom 8 idiom) rather than
    // `map(pair(..), |(_, n)| ..)` so the throwaway `_` goes away and
    // the composition matches the rest of this file's `.parse(input)?`
    // style. Tuples implement `Parser` in nom 8 and are used as the first
    // element of `preceded` to consume the operator char plus any trailing
    // whitespace.
    let (input, modifier) = alt((
        // +N -> Add
        preceded((char('+'), multispace0), parse_number)
            .map(|n| StrengthModifier::Add(clamp_to_i32(n))),
        // -N -> Subtract (parse_number handles negative directly; we
        // need parse_decimal_number after the explicit `-` consumer
        // so the sign is applied exactly once).
        preceded((char('-'), multispace0), parse_decimal_number)
            .map(|n| StrengthModifier::Subtract(clamp_to_i32(n))),
        // *N -> Multiply
        preceded((char('*'), multispace0), parse_number)
            .map(|n| StrengthModifier::Multiply(clamp_to_i32(n))),
        // /N -> Divide
        preceded((char('/'), multispace0), parse_number)
            .map(|n| StrengthModifier::Divide(clamp_to_i32(n))),
        // =N -> Set
        preceded((char('='), multispace0), parse_number)
            .map(|n| StrengthModifier::Set(clamp_to_i32(n))),
        // Bare number -> Set
        parse_number.map(|n| StrengthModifier::Set(clamp_to_i32(n))),
    ))
    .parse(input)?;

    Ok((input, modifier))
}

/// Check if a line is a strength directive (starts with !:strength)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::is_strength_directive;
///
/// assert!(is_strength_directive("!:strength +10"));
/// assert!(is_strength_directive("  !:strength -5"));
/// assert!(!is_strength_directive("0 byte 1"));
/// ```
#[must_use]
pub fn is_strength_directive(input: &str) -> bool {
    input.trim().starts_with("!:strength")
}

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

    // Meta-type directives (default, clear, name, use, indirect, offset)
    // conceptually have no operator/value operand, but magic(5) source
    // files (including GNU `file`'s own `searchbug.magic`) often write
    // them with an `x` (AnyValue) placeholder between the type and the
    // message, e.g. `>>&0 offset x at_offset %lld`. Consume an optional
    // leading `x` token here so it does not leak into the rendered
    // message.
    //
    // `name`/`use` are handled earlier in parse_type_and_operator and
    // already consumed their identifier operand, so the `x` stripping
    // is a no-op for them.
    if matches!(typ, TypeKind::Meta(_)) {
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
    } else if is_string_family_type {
        match parse_value(input) {
            Ok(ok) => ok,
            Err(orig_err) => parse_bare_string_value(input).map_err(|_| orig_err)?,
        }
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

/// Parse a bare (unquoted) single-token string literal as a `Value::String`.
///
/// Used only as a fallback for string-family types (`string`, `pstring`,
/// `regex`, `search`) when the strict [`parse_value`] alternatives all
/// fail. Consumes leading whitespace, then reads a run of non-whitespace
/// characters as the literal value, **interpreting magic(5) escape
/// sequences** along the way: `\0`, `\n`, `\r`, `\t`, `\\`, `\"`, `\'`,
/// `\NNN` (3-digit octal), and `\xNN` (hex). This supports magic-file
/// rules like `0 string PNCIHISK\0 ...` where the trailing `\0` denotes
/// a literal NUL byte that must be present in the file.
///
/// Without escape interpretation, the comparison value stored in the
/// AST is the literal six-byte string `\` + `0` instead of `\x00`, and
/// the rule never matches against a real on-disk byte sequence ending
/// in NUL. This was a regression that prevented even simple top-level
/// rules from matching when loaded from a magic file.
///
/// # Errors
/// Returns a nom parsing error if the input contains no non-whitespace
/// token (e.g. it is empty or consists entirely of whitespace).
fn parse_bare_string_value(input: &str) -> IResult<&str, Value> {
    let (input, _) = multispace0(input)?;
    if input.is_empty() || input.starts_with(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
    {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut remaining = input;
    while let Some(ch) = remaining.chars().next() {
        if ch.is_whitespace() || ch == '\n' || ch == '\r' {
            break;
        }
        if ch == '\\' {
            // Try a hex byte (`\xNN`) first since `parse_escape_sequence`
            // doesn't recognise it.
            if let Ok((rest, b)) = value::parse_hex_byte_with_prefix(remaining) {
                bytes.push(b);
                remaining = rest;
                continue;
            }
            if let Ok((rest, esc)) = value::parse_escape_sequence(remaining) {
                // `parse_escape_sequence` returns a `char`, but the
                // escape table covers single-byte values (NUL, control
                // chars, octal `\NNN` clamped to a `u8`). Cast back to
                // `u8` so the buffer stays byte-accurate when the
                // value is later compared against file bytes.
                let code = esc as u32;
                if let Ok(byte) = u8::try_from(code) {
                    bytes.push(byte);
                } else {
                    // Escape produced a non-byte char (shouldn't happen
                    // with the current escape grammar, but guard
                    // anyway). Encode as UTF-8 so we never lose data
                    // silently.
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(esc.encode_utf8(&mut buf).as_bytes());
                }
                remaining = rest;
                continue;
            }
            // Lone `\` not followed by a recognised escape -- treat as
            // a literal backslash and continue. This matches GNU `file`
            // tolerance for malformed escapes.
            bytes.push(b'\\');
            remaining = &remaining[1..];
            continue;
        }
        // Plain character: encode as UTF-8 (ASCII is one byte; non-ASCII
        // is 2-4 bytes which matches how the file would store the same
        // characters in a UTF-8 magic file).
        let mut buf = [0u8; 4];
        let utf8 = ch.encode_utf8(&mut buf).as_bytes();
        bytes.extend_from_slice(utf8);
        remaining = &remaining[ch.len_utf8()..];
    }

    if bytes.is_empty() {
        return Err(nom::Err::Error(NomError::new(
            input,
            nom::error::ErrorKind::TakeWhile1,
        )));
    }

    // The downstream comparison is `Value::String` against the buffer's
    // bytes. Use `from_utf8_lossy` so non-UTF-8 byte sequences (like
    // `\xff`) round-trip as best they can; the buffer-side read uses
    // the same lossy conversion, so equality still holds.
    let value = String::from_utf8_lossy(&bytes).into_owned();
    Ok((remaining, Value::String(value)))
}

/// Parse a comment line (starts with #)
///
/// Comments in magic files start with '#' and continue to the end of the line.
/// This function consumes the entire comment line.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_comment;
///
/// assert_eq!(parse_comment("# This is a comment"), Ok(("", "This is a comment".to_string())));
/// assert_eq!(parse_comment("#"), Ok(("", "".to_string())));
/// ```
/// Parse a comment line (starting with #)
///
/// # Errors
/// Returns a nom parsing error if the input is not a valid comment
pub fn parse_comment(input: &str) -> IResult<&str, String> {
    let (input, _) = multispace0(input)?;
    let (input, _) = char('#').parse(input)?;
    let (input, comment_text) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
    let comment = comment_text.trim().to_string();
    Ok((input, comment))
}

/// Check if a line is empty or contains only whitespace
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::is_empty_line;
///
/// assert!(is_empty_line(""));
/// assert!(is_empty_line("   "));
/// assert!(is_empty_line("\t\t"));
/// assert!(!is_empty_line("0 byte 1"));
/// ```
#[must_use]
pub fn is_empty_line(input: &str) -> bool {
    input.trim().is_empty()
}

/// Check if a line is a comment (starts with #)
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::is_comment_line;
///
/// assert!(is_comment_line("# This is a comment"));
/// assert!(is_comment_line("#"));
/// assert!(is_comment_line("  # Indented comment"));
/// assert!(!is_comment_line("0 byte 1"));
/// ```
#[must_use]
pub fn is_comment_line(input: &str) -> bool {
    input.trim().starts_with('#')
}

/// Check if a line ends with a continuation character (\)
///
/// Magic files support line continuation with backslash at the end of lines.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::has_continuation;
///
/// assert!(has_continuation("0 string test \\"));
/// assert!(has_continuation("message continues \\"));
/// assert!(!has_continuation("0 string test"));
/// ```
#[must_use]
pub fn has_continuation(input: &str) -> bool {
    input.trim_end().ends_with('\\')
}
#[cfg(test)]
mod tests;
