// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Printf-style format specifier substitution for magic rule messages.
//!
//! Magic file messages frequently contain C-style format specifiers such as
//! `%lld`, `%02x`, or `%s` that reference the rule's read value. GNU `file`
//! renders the message with the value substituted at the specifier's
//! position; without this pass libmagic-rs would emit the literal
//! specifier tokens (e.g., `at_offset %lld`) and diverge visibly from
//! `file(1)` output.
//!
//! The substitution is intentionally narrow: it supports the subset of
//! C's `printf` syntax that appears in shipping magic corpora (notably
//! `third_party/tests/searchbug.magic` and the GNU `file` `Magdir`
//! collection). Unrecognized specifiers pass through literally with a
//! `debug!` log rather than erroring -- matching the evaluator's
//! graceful-skip discipline.
//!
//! Width masking for hex specifiers uses [`crate::parser::ast::TypeKind::bit_width`]
//! so that e.g. a signed byte rendered with `%02x` produces the unsigned
//! 8-bit interpretation (`0xff`, not `0xffffffffffffffff`).
//!
//! See the project plan at
//! `docs/plans/2026-04-22-001-feat-meta-type-offset-and-format-substitution-plan.md`
//! for scope, and GOTCHAS.md S14.2 for historical context.

use log::debug;

use crate::parser::ast::{TypeKind, Value};

/// Substitute printf-style format specifiers in a magic rule message.
///
/// Walks `template` left to right. Plain text is copied verbatim; on
/// each `%`, the full specifier (`%[flags][width][.precision][length]<conv>`)
/// is parsed and substituted from `value`. `%%` emits a single `%`.
/// Unrecognized or malformed specifiers are passed through literally
/// with a `debug!` log.
///
/// `type_kind` is consulted only for hex specifiers, which need the
/// natural bit width of the underlying read to mask sign-extended
/// values correctly. For non-hex specifiers `type_kind` is ignored.
///
/// # Examples
///
/// ```
/// use libmagic_rs::output::format::format_magic_message;
/// use libmagic_rs::parser::ast::{TypeKind, Value};
///
/// let out = format_magic_message(
///     "at_offset %lld",
///     &Value::Uint(11),
///     &TypeKind::Byte { signed: false },
/// );
/// assert_eq!(out, "at_offset 11");
///
/// let out = format_magic_message(
///     "followed_by 0x%02x",
///     &Value::Uint(0x31),
///     &TypeKind::Byte { signed: false },
/// );
/// assert_eq!(out, "followed_by 0x31");
///
/// // Unknown specifier falls through literally.
/// let out = format_magic_message("%q", &Value::Uint(0), &TypeKind::Byte { signed: false });
/// assert_eq!(out, "%q");
///
/// // `%%` is an escaped literal percent.
/// let out = format_magic_message("100%% sure", &Value::Uint(0), &TypeKind::Byte { signed: false });
/// assert_eq!(out, "100% sure");
/// ```
#[must_use]
pub fn format_magic_message(template: &str, value: &Value, type_kind: &TypeKind) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b != b'%' {
            // SAFETY: iterating by byte but template is valid UTF-8; any
            // non-ASCII multi-byte character has all continuation bytes
            // > 0x7f which cannot equal b'%' (0x25), so we never split
            // a UTF-8 codepoint here. Push as char.
            out.push(b as char);
            i += 1;
            continue;
        }

        // Start of a format specifier at position i.
        let spec_start = i;
        let Some(parsed_spec) = parse_spec(bytes, i + 1) else {
            // Malformed specifier (e.g., trailing `%` with nothing after,
            // or a sequence that doesn't end in a valid conversion char).
            // Pass through the remaining literal and stop scanning.
            debug!(
                "format_magic_message: malformed specifier at byte {i} in template {template:?}; passing through remainder literally",
            );
            out.push_str(&template[i..]);
            break;
        };
        let next_i = parsed_spec.end;
        if let Some(rendered) = render(&parsed_spec, value, type_kind) {
            out.push_str(&rendered);
        } else {
            // Type mismatch or unsupported conversion; pass through the
            // literal specifier and log.
            let literal = &template[spec_start..next_i];
            debug!(
                "format_magic_message: unsupported specifier {literal:?} for value {value:?}; passing through literally",
            );
            out.push_str(literal);
        }
        i = next_i;
    }

    out
}

/// Kinds of conversion characters we recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conv {
    /// `%d`, `%i`, `%ld`, `%lld` -- signed decimal.
    Signed,
    /// `%u`, `%lu`, `%llu` -- unsigned decimal.
    Unsigned,
    /// `%x` -- lowercase hex.
    HexLower,
    /// `%X` -- uppercase hex.
    HexUpper,
    /// `%o` -- octal.
    Octal,
    /// `%s` -- string.
    Str,
    /// `%c` -- single character (from an integer codepoint, ASCII range only).
    Char,
    /// `%%` -- literal percent.
    Percent,
}

/// Parsed format specifier.
#[derive(Debug, Clone)]
struct Spec {
    zero_pad: bool,
    left_align: bool,
    alt_form: bool,
    width: usize,
    conv: Conv,
    /// Byte index of the character *after* this specifier in the template.
    end: usize,
}

/// Parse a format specifier starting at `start` (the first byte after the
/// leading `%`). Returns `None` if the sequence does not end in a
/// recognized conversion character.
fn parse_spec(bytes: &[u8], start: usize) -> Option<Spec> {
    let mut i = start;
    let mut zero_pad = false;
    let mut left_align = false;
    let mut alt_form = false;

    // Flags (subset: 0, -, #). Other flags (+, space) are parsed but ignored.
    while i < bytes.len() {
        match bytes[i] {
            b'0' => {
                zero_pad = true;
                i += 1;
            }
            b'-' => {
                left_align = true;
                i += 1;
            }
            b'#' => {
                alt_form = true;
                i += 1;
            }
            b'+' | b' ' => {
                // Accepted for syntactic completeness, no rendering effect
                // in the current subset.
                i += 1;
            }
            _ => break,
        }
    }

    // Width (decimal digits).
    let mut width: usize = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        let digit = (bytes[i] - b'0') as usize;
        width = width.saturating_mul(10).saturating_add(digit);
        i += 1;
    }

    // Precision (`.<digits>`): parsed and skipped -- no current consumer
    // requires precision handling, and numeric rendering is whole-value.
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }

    // Length modifier (`h`, `hh`, `l`, `ll`, `j`, `z`, `t`). We consume
    // these for syntactic completeness but never rely on them -- all
    // numeric rendering uses full u64/i64 width.
    while i < bytes.len() {
        match bytes[i] {
            b'l' | b'h' | b'j' | b'z' | b't' => i += 1,
            _ => break,
        }
    }

    if i >= bytes.len() {
        return None;
    }

    let conv = match bytes[i] {
        b'd' | b'i' => Conv::Signed,
        b'u' => Conv::Unsigned,
        b'x' => Conv::HexLower,
        b'X' => Conv::HexUpper,
        b'o' => Conv::Octal,
        b's' => Conv::Str,
        b'c' => Conv::Char,
        b'%' => Conv::Percent,
        _ => return None,
    };
    i += 1;

    Some(Spec {
        zero_pad,
        left_align,
        alt_form,
        width,
        conv,
        end: i,
    })
}

/// Render the specifier against `value`, or return `None` if the value
/// is type-incompatible with the conversion.
fn render(spec: &Spec, value: &Value, type_kind: &TypeKind) -> Option<String> {
    match spec.conv {
        Conv::Percent => Some("%".to_string()),
        Conv::Str => Some(render_string(value)),
        Conv::Signed => {
            let n = coerce_to_i64(value)?;
            Some(pad_numeric(&n.to_string(), spec))
        }
        Conv::Unsigned => {
            let n = coerce_to_u64(value)?;
            Some(pad_numeric(&n.to_string(), spec))
        }
        Conv::HexLower => {
            let n = coerce_to_u64_masked(value, type_kind)?;
            Some(render_prefixed_int(
                &format!("{n:x}"),
                if spec.alt_form { "0x" } else { "" },
                spec,
            ))
        }
        Conv::HexUpper => {
            let n = coerce_to_u64_masked(value, type_kind)?;
            Some(render_prefixed_int(
                &format!("{n:X}"),
                if spec.alt_form { "0x" } else { "" },
                spec,
            ))
        }
        Conv::Octal => {
            let n = coerce_to_u64_masked(value, type_kind)?;
            Some(render_prefixed_int(
                &format!("{n:o}"),
                if spec.alt_form { "0o" } else { "" },
                spec,
            ))
        }
        Conv::Char => {
            let n = coerce_to_u64(value)?;
            let byte = u8::try_from(n).ok()?;
            if byte > 0x7f {
                return None;
            }
            Some(pad_numeric(&(byte as char).to_string(), spec))
        }
    }
}

/// Render a [`Value`] for `%s`. Strings pass through; byte sequences are
/// converted via lossy UTF-8; numbers render as decimal (GNU `file` does
/// the same for mixed-type `%s` substitutions).
fn render_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        Value::Uint(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
    }
}

/// Coerce a numeric-ish [`Value`] to `i64`. Float values are truncated
/// toward zero (documented intent -- matches C's `(long long)float`
/// semantics that libmagic's `printf` path relies on). String/Bytes
/// values have no sensible mapping and return `None`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn coerce_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Int(n) => Some(*n),
        // u64 -> i64 bit-pattern reinterpret: matches C's implicit
        // cast in `printf("%lld", (unsigned long long)...)`.
        Value::Uint(n) => Some(*n as i64),
        // f64 -> i64 truncation toward zero, matching C behavior for
        // `printf("%d", (double)...)`.
        Value::Float(f) => Some(*f as i64),
        Value::String(_) | Value::Bytes(_) => None,
    }
}

/// Coerce a numeric-ish [`Value`] to `u64`. Mirrors [`coerce_to_i64`]
/// but preserves the unsigned bit pattern when the source is signed.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn coerce_to_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Uint(n) => Some(*n),
        // i64 -> u64 bit-pattern reinterpret for rendering; parallels
        // the `coerce_to_i64` case.
        Value::Int(n) => Some(*n as u64),
        Value::Float(f) => Some(*f as u64),
        Value::String(_) | Value::Bytes(_) => None,
    }
}

/// Coerce a numeric-ish [`Value`] to `u64`, masked to the natural bit
/// width of `type_kind`. Used by hex/octal specifiers to avoid
/// surprising sign-extended renderings like `byte = -1` rendering as
/// `ffffffffffffffff` when the user expected `ff`.
fn coerce_to_u64_masked(value: &Value, type_kind: &TypeKind) -> Option<u64> {
    let raw = coerce_to_u64(value)?;
    let mask = match type_kind.bit_width() {
        Some(8) => 0xff_u64,
        Some(16) => 0xffff_u64,
        Some(32) => 0xffff_ffff_u64,
        // 64-bit, unknown width, or any other case: no mask needed.
        _ => return Some(raw),
    };
    Some(raw & mask)
}

/// Render a numeric body with an alt-form prefix (`0x` / `0o` / empty),
/// applying width and padding correctly.
///
/// For zero-padded widths (`%#0Nx`), C printf inserts zeros *between*
/// the prefix and the digits: `%#06x` + `0xab` -> `0x00ab`, not
/// `  0xab`. For space-padded widths (`%#Nx`), the spaces go *before*
/// the prefix: `%#6x` + `0xab` -> `  0xab`. For left-aligned widths
/// (`%-#6x`), trailing spaces follow the digits: `0xab  `.
fn render_prefixed_int(digits: &str, prefix: &str, spec: &Spec) -> String {
    // The effective body length for width comparison is prefix + digits.
    let body_len = prefix.len() + digits.len();
    if body_len >= spec.width {
        return format!("{prefix}{digits}");
    }
    let pad = spec.width - body_len;
    if spec.zero_pad && !spec.left_align {
        // Zeros insert between the prefix and the digits.
        let zeros: String = std::iter::repeat_n('0', pad).collect();
        format!("{prefix}{zeros}{digits}")
    } else if spec.left_align {
        let spaces: String = std::iter::repeat_n(' ', pad).collect();
        format!("{prefix}{digits}{spaces}")
    } else {
        let spaces: String = std::iter::repeat_n(' ', pad).collect();
        format!("{spaces}{prefix}{digits}")
    }
}

/// Apply width and padding to an already-rendered numeric body.
fn pad_numeric(body: &str, spec: &Spec) -> String {
    if body.len() >= spec.width {
        return body.to_string();
    }
    let pad = spec.width - body.len();
    let pad_char = if spec.zero_pad && !spec.left_align {
        '0'
    } else {
        ' '
    };
    let padding: String = std::iter::repeat_n(pad_char, pad).collect();
    if spec.left_align {
        format!("{body}{padding}")
    } else {
        format!("{padding}{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn byte_t() -> TypeKind {
        TypeKind::Byte { signed: false }
    }

    fn long_t() -> TypeKind {
        TypeKind::Long {
            endian: crate::parser::ast::Endianness::Little,
            signed: true,
        }
    }

    // ---- happy path --------------------------------------------------

    #[test]
    fn test_signed_decimal_substitution() {
        // Covers %d, %i, %ld, %lld (length modifiers are accepted and ignored).
        let cases = [
            ("v=%d", Value::Int(-7), "v=-7"),
            ("v=%i", Value::Int(42), "v=42"),
            ("v=%ld", Value::Int(10), "v=10"),
            ("at_offset %lld", Value::Uint(11), "at_offset 11"),
        ];
        for (tmpl, val, expected) in cases {
            assert_eq!(
                format_magic_message(tmpl, &val, &byte_t()),
                expected,
                "template {tmpl:?} with value {val:?}",
            );
        }
    }

    #[test]
    fn test_unsigned_decimal_substitution() {
        let out = format_magic_message("n=%u", &Value::Uint(200), &byte_t());
        assert_eq!(out, "n=200");

        // i64::MIN as unsigned should come through as 2^63.
        let out = format_magic_message("n=%llu", &Value::Int(i64::MIN), &long_t());
        assert_eq!(out, "n=9223372036854775808");
    }

    #[test]
    fn test_hex_substitution_with_byte_width_masking() {
        // The canonical searchbug.result case: ubyte `%02x`.
        let out = format_magic_message("0x%02x", &Value::Uint(0x31), &byte_t());
        assert_eq!(out, "0x31");

        // Byte -1 (sign-extended to u64::MAX in Value::Int) must render as "ff",
        // not "ffffffffffffffff", when the underlying type is a byte.
        let out = format_magic_message("0x%02x", &Value::Int(-1), &byte_t());
        assert_eq!(out, "0xff");

        // %X is uppercase.
        let out = format_magic_message("%X", &Value::Uint(0xdead_beef), &long_t());
        assert_eq!(out, "DEADBEEF");

        // %#x emits the "0x" prefix via alt form.
        let out = format_magic_message("%#x", &Value::Uint(0xab), &byte_t());
        assert_eq!(out, "0xab");

        // %#06x: zero-pad inserts between prefix and digits (C printf semantics),
        // not before the prefix. Regression guard for correctness review COR-002.
        let out = format_magic_message("%#06x", &Value::Uint(0xab), &byte_t());
        assert_eq!(out, "0x00ab");

        // Space-padded width with alt-form prefix: spaces go before prefix.
        let out = format_magic_message("%#6x", &Value::Uint(0xab), &byte_t());
        assert_eq!(out, "  0xab");

        // Left-aligned with alt-form prefix: spaces trail the digits.
        let out = format_magic_message("%-#6x|", &Value::Uint(0xab), &byte_t());
        assert_eq!(out, "0xab  |");

        // %#08o: zero-pad inserts between "0o" prefix and digits.
        let out = format_magic_message("%#08o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "0o000010");
    }

    #[test]
    fn test_string_substitution() {
        let out = format_magic_message(
            "hello %s",
            &Value::String("world".to_string()),
            &TypeKind::String { max_length: None },
        );
        assert_eq!(out, "hello world");

        // Bytes go through lossy UTF-8.
        let out = format_magic_message(
            "data=%s",
            &Value::Bytes(b"abc".to_vec()),
            &TypeKind::String { max_length: None },
        );
        assert_eq!(out, "data=abc");
    }

    #[test]
    fn test_octal_substitution() {
        let out = format_magic_message("%o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "10");
        let out = format_magic_message("%#o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "0o10");
    }

    #[test]
    fn test_char_substitution() {
        let out = format_magic_message("[%c]", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "[A]");
    }

    #[test]
    fn test_percent_escape() {
        let out = format_magic_message("100%% sure", &Value::Uint(0), &byte_t());
        assert_eq!(out, "100% sure");
    }

    #[test]
    fn test_multiple_specifiers_in_one_template() {
        // Note: current implementation binds every specifier to the single
        // `value`; multiple specifiers are rendered against the same value.
        // This matches libmagic's single-argument model -- magic rules only
        // expose one read value per rule.
        let out = format_magic_message("a=%d b=%d", &Value::Int(5), &long_t());
        assert_eq!(out, "a=5 b=5");
    }

    #[test]
    fn test_width_padding() {
        // Zero-padded width.
        let out = format_magic_message("%05d", &Value::Int(42), &long_t());
        assert_eq!(out, "00042");
        // Space-padded width.
        let out = format_magic_message("%5d", &Value::Int(42), &long_t());
        assert_eq!(out, "   42");
        // Left-aligned (zero flag ignored when `-` is set).
        let out = format_magic_message("%-5d|", &Value::Int(42), &long_t());
        assert_eq!(out, "42   |");
    }

    // ---- edge cases --------------------------------------------------

    #[test]
    fn test_empty_template() {
        assert_eq!(
            format_magic_message("", &Value::Uint(0), &byte_t()),
            String::new()
        );
    }

    #[test]
    fn test_literal_with_no_specifiers() {
        assert_eq!(
            format_magic_message("hello world", &Value::Uint(0), &byte_t()),
            "hello world"
        );
    }

    #[test]
    fn test_trailing_percent_with_no_spec() {
        // A stray `%` at end-of-string: pass through literally.
        let out = format_magic_message("done %", &Value::Uint(0), &byte_t());
        assert_eq!(out, "done %");
    }

    #[test]
    fn test_unknown_specifier_pass_through() {
        // `%q` is not in our subset.
        let out = format_magic_message("bad %q end", &Value::Uint(0), &byte_t());
        assert_eq!(out, "bad %q end");
    }

    #[test]
    fn test_type_mismatch_string_conv_on_uint_still_renders() {
        // `%s` against an integer value -- GNU `file` renders the number
        // as decimal; libmagic-rs matches that behavior via `render_string`.
        let out = format_magic_message("v=%s", &Value::Uint(42), &byte_t());
        assert_eq!(out, "v=42");
    }

    #[test]
    fn test_type_mismatch_numeric_conv_on_string_passes_through() {
        // `%d` against a string has no sensible coercion -> literal.
        let out = format_magic_message(
            "v=%d",
            &Value::String("hi".to_string()),
            &TypeKind::String { max_length: None },
        );
        assert_eq!(out, "v=%d");
    }

    #[test]
    fn test_char_specifier_rejects_non_ascii() {
        // Values above 0x7f cannot be rendered as `%c` -> pass through literally.
        let out = format_magic_message("[%c]", &Value::Uint(0xff), &byte_t());
        assert_eq!(out, "[%c]");
    }

    #[test]
    fn test_byte_width_masking_on_negative_signed_byte() {
        // Regression guard: a signed byte carrying -1 (the representation
        // on the Value side is Int(-1)) must NOT render as a 64-bit mask.
        let out = format_magic_message("%x", &Value::Int(-1), &byte_t());
        assert_eq!(out, "ff");
    }

    #[test]
    fn test_hex_width_masking_respects_16bit() {
        let short_t = TypeKind::Short {
            endian: crate::parser::ast::Endianness::Little,
            signed: true,
        };
        let out = format_magic_message("%x", &Value::Int(-1), &short_t);
        assert_eq!(out, "ffff");
    }
}
