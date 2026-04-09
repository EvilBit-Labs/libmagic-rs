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
    combinator::{map, opt},
    error::Error as NomError,
    multi::many0,
    sequence::pair,
};

use crate::parser::ast::{
    Endianness, MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value,
};

mod numbers;
mod value;

pub use numbers::parse_number;
pub use value::parse_value;

#[cfg(test)]
use numbers::parse_hex_number;
use numbers::{parse_decimal_number, parse_unsigned_number};
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
        _ => None,
    }
}

/// Parse an indirect offset specification: `(base.type)` or `(base.type)+/-adj`
///
/// Reads a pointer specifier after the dot, closes the parenthesized expression,
/// then optionally parses `+N` or `-N` adjustment after the `)`.
fn parse_indirect_offset(input: &str) -> IResult<&str, OffsetSpec> {
    let (input, _) = char('(')(input)?;
    let (input, base_offset) = parse_number(input)?;
    let (input, _) = char('.')(input)?;
    let (input, spec_char) = one_of("bBsSlLqQ")(input)?;

    let (pointer_type, endian) = pointer_specifier_to_type(spec_char)
        .ok_or_else(|| nom::Err::Error(NomError::new(input, nom::error::ErrorKind::OneOf)))?;

    let (input, _) = char(')')(input)?;

    // Optional adjustment AFTER closing paren: (base.type)+N or (base.type)-N
    // parse_number handles '-' but not '+', so consume '+' manually
    let (input, adjustment) = if input.starts_with('+') {
        let (input, _) = char('+')(input)?;
        parse_number(input)?
    } else if input.starts_with('-') {
        parse_number(input)?
    } else {
        (input, 0)
    };

    Ok((
        input,
        OffsetSpec::Indirect {
            base_offset,
            pointer_type,
            adjustment,
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
        Some(b'!') => {
            // Only "!=" is valid; bare "!" is an error.
            if bytes.get(1).copied() == Some(b'=') {
                (Operator::NotEqual, 2)
            } else {
                return Err(err());
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
            if input[1..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
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

/// Parse pstring suffix flags after the `/` character.
///
/// Recognizes width characters (`B`, `H`, `h`, `L`, `l`) and the optional `J`
/// modifier that indicates the stored length includes the length field itself.
///
/// Returns `Ok((remaining_input, width, length_includes_itself))` on success,
/// or `Err` if an unrecognized suffix character is found.
fn parse_pstring_suffix(
    input: &str,
) -> Result<(&str, crate::parser::ast::PStringLengthWidth, bool), nom::Err<nom::error::Error<&str>>>
{
    use crate::parser::ast::PStringLengthWidth;

    // Parse width character
    let (rest, width) = if let Some(rest) = input.strip_prefix('B') {
        (rest, PStringLengthWidth::OneByte)
    } else if let Some(rest) = input.strip_prefix('H') {
        (rest, PStringLengthWidth::TwoByteBE)
    } else if let Some(rest) = input.strip_prefix('h') {
        (rest, PStringLengthWidth::TwoByteLE)
    } else if let Some(rest) = input.strip_prefix('L') {
        (rest, PStringLengthWidth::FourByteBE)
    } else if let Some(rest) = input.strip_prefix('l') {
        (rest, PStringLengthWidth::FourByteLE)
    } else if let Some(rest) = input.strip_prefix('J') {
        // Bare /J with no width = default OneByte + self-inclusive
        return Ok((rest, PStringLengthWidth::OneByte, true));
    } else {
        // Unrecognized suffix character after '/'
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::OneOf,
        )));
    };

    // Parse optional J flag after width character
    let (rest, includes_j) = if let Some(rest) = rest.strip_prefix('J') {
        (rest, true)
    } else {
        (rest, false)
    };

    Ok((rest, width, includes_j))
}

/// Parse a type specification with an optional attached bitwise-AND mask operator
/// (e.g., `lelong&0xf0000000`).
///
/// Returns the `TypeKind` and an optional `Operator`.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::parser::grammar::parse_type_and_operator;
/// use libmagic_rs::parser::ast::{TypeKind, Operator, Endianness};
///
/// // Type without operator
/// let (_, (kind, op)) = parse_type_and_operator("lelong").unwrap();
/// assert_eq!(kind, TypeKind::Long { endian: Endianness::Little, signed: true });
/// assert_eq!(op, None);
///
/// // Type with mask operator
/// let (_, (kind, op)) = parse_type_and_operator("lelong&0xf0000000").unwrap();
/// assert!(matches!(op, Some(Operator::BitwiseAndMask(_))));
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match the expected format
pub fn parse_type_and_operator(input: &str) -> IResult<&str, (TypeKind, Option<Operator>)> {
    use crate::parser::ast::PStringLengthWidth;

    let (input, _) = multispace0(input)?;

    let (mut input, type_name) = crate::parser::types::parse_type_keyword(input)?;

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

    // Check for attached operator with mask (like &0xf0000000)
    // Uses unsigned parsing so full u64 masks (e.g. 0xffffffffffffffff) are supported.
    // If '&' is followed by digits/0x but the mask parse fails (overflow, etc.),
    // we return a hard error instead of silently falling back to standalone '&'.
    let (input, attached_op) = if let Some(after_amp) = input.strip_prefix('&') {
        if after_amp.starts_with("0x") || after_amp.starts_with(|c: char| c.is_ascii_digit()) {
            // '&' followed by what looks like a number -- must parse as mask
            let (rest, mask) = parse_unsigned_number(after_amp).map_err(|_| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
            })?;
            (rest, Some(Operator::BitwiseAndMask(mask)))
        } else if after_amp.starts_with('&') {
            // Reject '&&' -- not valid operator syntax
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        } else {
            // Standalone '&' (no digits following)
            (after_amp, Some(Operator::BitwiseAnd))
        }
    } else {
        (input, None)
    };

    let (input, _) = multispace0(input)?;

    let mut type_kind = crate::parser::types::type_keyword_to_kind(type_name);
    // Patch PString with parsed length_width and length_includes_itself
    if let TypeKind::PString { max_length, .. } = type_kind {
        type_kind = TypeKind::PString {
            max_length,
            length_width: pstring_length_width,
            length_includes_itself: pstring_length_includes_itself,
        };
    }

    Ok((input, (type_kind, attached_op)))
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
/// assert_eq!(parse_type("string"), Ok(("", TypeKind::String { max_length: None })));
/// ```
///
/// # Errors
/// Returns a nom parsing error if the input doesn't match any known type
#[allow(dead_code)] // Standalone helper exercised by grammar unit tests.
pub fn parse_type(input: &str) -> IResult<&str, TypeKind> {
    let (input, (type_kind, _)) = parse_type_and_operator(input)?;
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
    fn clamp_to_i32(n: i64) -> i32 {
        // Use i64::from for lossless conversion, then clamp and convert back
        let clamped = n.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
        // Safe to unwrap: clamped value is guaranteed to be in i32 range
        i32::try_from(clamped).unwrap()
    }

    let (input, _) = multispace0(input)?;
    let (input, _) = tag("!:strength")(input)?;
    let (input, _) = multispace0(input)?;

    // Parse the operator: +, -, *, /, = or bare number (implies =)
    let (input, modifier) = alt((
        // +N -> Add
        map(pair(char('+'), parse_number), |(_, n)| {
            StrengthModifier::Add(clamp_to_i32(n))
        }),
        // -N -> Subtract (note: parse_number handles negative, so we need special handling)
        map(pair(char('-'), parse_decimal_number), |(_, n)| {
            StrengthModifier::Subtract(clamp_to_i32(n))
        }),
        // *N -> Multiply
        map(pair(char('*'), parse_number), |(_, n)| {
            StrengthModifier::Multiply(clamp_to_i32(n))
        }),
        // /N -> Divide
        map(pair(char('/'), parse_number), |(_, n)| {
            StrengthModifier::Divide(clamp_to_i32(n))
        }),
        // =N -> Set
        map(pair(char('='), parse_number), |(_, n)| {
            StrengthModifier::Set(clamp_to_i32(n))
        }),
        // Bare number -> Set
        map(parse_number, |n| StrengthModifier::Set(clamp_to_i32(n))),
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

    // Parse the type and any attached operator
    let (input, (typ, attached_op)) = parse_type_and_operator(input)?;

    // Try to parse a separate operator (optional - use attached operator if present)
    let (input, separate_op) = opt(parse_operator).parse(input)?;
    let op = attached_op.or(separate_op).unwrap_or(Operator::Equal);

    // For AnyValue (`x`), no operand is needed -- treat remaining text as message
    let (input, value) = if op == Operator::AnyValue {
        (input, Value::Uint(0))
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
    };

    Ok((input, rule))
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
