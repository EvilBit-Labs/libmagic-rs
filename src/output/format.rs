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
/// For a `%s` substitution against a `string`/`pstring`/`string16` value,
/// the rendered field is bounded to at most [`MAX_DESCRIPTION_FIELD_LEN`]
/// bytes (cut on a UTF-8 character boundary); `regex`/`search` values
/// render through a separate, unbounded path and are exempt. This entry
/// point never stops at a newline (R2's gate) -- use
/// [`format_magic_message_with_gate`] to opt into that.
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
    format_magic_message_with_gate(template, value, type_kind, false)
}

/// Substitute printf-style format specifiers in a magic rule message,
/// additionally carrying R2's newline-stop gate signal.
///
/// Identical to [`format_magic_message`] except for `stop_at_newline`: when
/// set, a `%s` substitution against a `string`/`pstring`/`string16` value
/// additionally stops at the first `\r` or `\n` byte within the
/// [`MAX_DESCRIPTION_FIELD_LEN`]-byte bound. The caller computes
/// `stop_at_newline` from the rule's operator and pattern (see
/// `evaluator::engine::value_eval::newline_stop_gate` in the crate
/// source) -- neither reaches this formatter otherwise (R14).
/// `regex`/`search` values ignore this flag entirely (R5).
#[must_use]
// Indexing is invariant-safe: every `bytes[i]` is guarded by an
// `i < bytes.len()` loop condition.
#[allow(clippy::indexing_slicing)]
pub fn format_magic_message_with_gate(
    template: &str,
    value: &Value,
    type_kind: &TypeKind,
    stop_at_newline: bool,
) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    // Start of the most recent run of non-`%` bytes. We copy the run
    // as a string slice rather than byte-by-byte so non-ASCII UTF-8
    // code points survive intact. Scanning still happens at the byte
    // level (safe because `%` is ASCII 0x25 and cannot appear as a
    // UTF-8 continuation byte, which is always >= 0x80).
    let mut plain_start = 0;

    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }

        // Flush any pending plain-text run as a single UTF-8 slice.
        if plain_start < i {
            out.push_str(&template[plain_start..i]);
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
            // Skip the trailing flush -- we have already emitted the
            // remainder above.
            plain_start = bytes.len();
            break;
        };
        let next_i = parsed_spec.end;
        if let Some(rendered) = render(&parsed_spec, value, type_kind, stop_at_newline) {
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
        plain_start = i;
    }

    // Flush any trailing plain-text run.
    if plain_start < bytes.len() {
        out.push_str(&template[plain_start..]);
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
    /// `%c` -- single character (full 0x00-0xff byte range via Latin-1 code points).
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
    /// `.<digits>` precision. Currently honored only for `%s` (`Conv::Str`),
    /// where it truncates the rendered string to at most `precision`
    /// characters -- e.g. sgml's `%.3s` renders the XML version `1.0` from a
    /// longer field. `None` means no precision was given (no truncation).
    precision: Option<usize>,
    conv: Conv,
    /// Byte index of the character *after* this specifier in the template.
    end: usize,
}

/// Maximum width value accepted from a format specifier.
///
/// Caps the field width to prevent crafted magic rules with enormous widths
/// (e.g., `%999999999d`) from driving unbounded `repeat_n` allocations in the
/// padding helpers. 4096 is generous for any real magic-corpus usage.
const MAX_FORMAT_WIDTH: usize = 4096;

/// Bound on how much file-derived text a `%s` substitution renders, taken
/// from GNU `file`'s `MAXstring - 1` (`src/file.h`). `MAXstring` was 96
/// through `file` 5.38 and became 128 at 5.39, so this value (127) holds
/// for `file` 5.39 and later only.
///
/// Applies only to the value-buffer types `string`, `pstring`, and
/// `string16` ([`is_bounded_string_family`]); `regex` and `search` are
/// bounded elsewhere by their own scan-window limits and are exempt from
/// this bound and from the newline stop (R5).
///
/// `pub(crate)` so the any-value and ordering-display *readers*
/// (`evaluator::types::any_value_string_bound`,
/// `evaluator::engine::value_eval::string_ordering_display_value`) can
/// reuse the same constant rather than redefining it (R12, KTD2) --
/// keeping the read/anchor bound and the render bound numerically
/// identical by construction.
pub(crate) const MAX_DESCRIPTION_FIELD_LEN: usize = 127;

/// Parse a format specifier starting at `start` (the first byte after the
/// leading `%`). Returns `None` if the sequence does not end in a
/// recognized conversion character.
// Indexing is invariant-safe: every `bytes[i]` is guarded by an
// `i < bytes.len()` loop or branch condition.
#[allow(clippy::indexing_slicing)]
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

    // Width (decimal digits). Capped at MAX_FORMAT_WIDTH to prevent
    // unbounded allocations from crafted format strings.
    let mut width: usize = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        let digit = (bytes[i] - b'0') as usize;
        width = width
            .saturating_mul(10)
            .saturating_add(digit)
            .min(MAX_FORMAT_WIDTH);
        i += 1;
    }

    // Precision (`.<digits>`). Captured for `%s` truncation (see `render`);
    // numeric rendering is still whole-value and ignores it. A bare `.` with
    // no digits means precision 0 (C semantics). The digit run is capped at
    // MAX_FORMAT_WIDTH for the same crafted-input protection as `width`.
    let mut precision: Option<usize> = None;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let mut prec: usize = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            let digit = (bytes[i] - b'0') as usize;
            prec = prec
                .saturating_mul(10)
                .saturating_add(digit)
                .min(MAX_FORMAT_WIDTH);
            i += 1;
        }
        precision = Some(prec);
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
        precision,
        conv,
        end: i,
    })
}

/// Render the specifier against `value`, or return `None` if the value
/// is type-incompatible with the conversion.
///
/// `stop_at_newline` is R2's gate signal (see [`format_magic_message`]);
/// it is consulted only by the `%s` arm.
fn render(
    spec: &Spec,
    value: &Value,
    type_kind: &TypeKind,
    stop_at_newline: bool,
) -> Option<String> {
    match spec.conv {
        Conv::Percent => Some("%".to_string()),
        Conv::Str => Some(render_str_spec(spec, value, type_kind, stop_at_newline)),
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
            // C printf suppresses the `0x`/`0X` alt-form prefix when the
            // value is zero: `printf("%#x", 0)` emits `"0"`, not `"0x0"`.
            let prefix = if spec.alt_form && n != 0 { "0x" } else { "" };
            Some(render_prefixed_int(&format!("{n:x}"), prefix, spec))
        }
        Conv::HexUpper => {
            let n = coerce_to_u64_masked(value, type_kind)?;
            let prefix = if spec.alt_form && n != 0 { "0X" } else { "" };
            Some(render_prefixed_int(&format!("{n:X}"), prefix, spec))
        }
        Conv::Octal => {
            let n = coerce_to_u64_masked(value, type_kind)?;
            // C printf uses a single "0" prefix for %#o (not Rust's "0o"),
            // and suppresses the prefix when the value itself is zero --
            // the resulting digit `0` already satisfies the "starts with
            // 0" invariant that the alt-form is meant to guarantee.
            let prefix = if spec.alt_form && n != 0 { "0" } else { "" };
            Some(render_prefixed_int(&format!("{n:o}"), prefix, spec))
        }
        Conv::Char => {
            let n = coerce_to_u64(value)?;
            let byte = u8::try_from(n).ok()?;
            // GNU `file` / C printf `%c` converts the int argument to
            // unsigned char and emits it directly for all byte values
            // 0x00-0xff. Rust's `String` must be valid UTF-8, so we
            // embed bytes >= 0x80 as their Latin-1 code points (U+0080
            // through U+00FF) via `char::from(u8)` which is infallible
            // and lossless. Consumers with UTF-8 terminals see the
            // 2-byte UTF-8 encoding of that code point; consumers
            // iterating the returned bytes directly can recover the
            // original byte by re-encoding the code point as Latin-1.
            //
            // POSIX: the `0` flag is ignored for `%c` -- zero-padding only
            // applies to numeric/float conversions. Always use space-padding
            // for `%c`, matching C printf behavior.
            Some(pad_non_numeric(&char::from(byte).to_string(), spec))
        }
    }
}

/// Whether `type_kind` is one of the value-buffer types R1/R2 bound:
/// `string`, `pstring`, `string16`. `regex` and `search` render through a
/// separate, unbounded path (R5); other types render `%s` via decimal or
/// lossy-UTF-8 conversion and are far under the bound regardless.
fn is_bounded_string_family(type_kind: &TypeKind) -> bool {
    matches!(
        type_kind,
        TypeKind::String { .. } | TypeKind::String16 { .. } | TypeKind::PString { .. }
    )
}

/// Bound `base` to at most [`MAX_DESCRIPTION_FIELD_LEN`] bytes, cutting at
/// a UTF-8 character boundary at or before the limit (R3): a multi-byte
/// character straddling the limit is dropped whole rather than split,
/// which renders a few bytes shorter than `file`'s raw-byte cut -- an
/// accepted parity tolerance.
///
/// When `stop_at_newline` is set (R2's gate -- an any-value rule, or an
/// ordering-compared rule whose pattern's first byte is a null byte; see
/// `evaluator::engine::value_eval::newline_stop_gate`), the bound
/// additionally stops at the first `\r` or `\n` byte within the limit.
// Indexing/slicing is invariant-safe: `cut` is clamped to `bytes.len()`
// above and only ever walked downward to a valid char boundary before
// either slice is taken.
#[allow(clippy::indexing_slicing)]
fn bound_description_field(base: &str, stop_at_newline: bool) -> String {
    let bytes = base.as_bytes();
    let mut cut = bytes.len().min(MAX_DESCRIPTION_FIELD_LEN);
    if stop_at_newline
        && let Some(newline_pos) = bytes[..cut].iter().position(|&b| b == b'\r' || b == b'\n')
    {
        cut = newline_pos;
    }
    while cut > 0 && !base.is_char_boundary(cut) {
        cut -= 1;
    }
    base[..cut].to_string()
}

/// Raw-byte counterpart of [`bound_description_field`] for `Value::Bytes`.
///
/// Operates on the file-derived byte slice directly, before any lossy
/// UTF-8 decode, so the [`MAX_DESCRIPTION_FIELD_LEN`] budget is measured on
/// file bytes rather than on decode-inflated `U+FFFD` bytes (see
/// [`render_string_bounded`] for why that ordering matters).
///
/// After cutting to the byte/newline limit, walks back over any trailing
/// UTF-8 continuation bytes (`0b10xxxxxx`) so a valid multi-byte character
/// straddling the cut is dropped whole (R3) rather than left to decode as
/// a stray replacement character. This is a lightweight structural check,
/// not a full UTF-8 validity scan: arbitrary (possibly non-UTF-8) bytes
/// are always safe to slice at any offset, and the later lossy decode
/// handles whatever is left over.
// Indexing/slicing is invariant-safe: `cut` is clamped to `bytes.len()`
// above and only ever walked downward before either slice is taken;
// `bytes.get(cut)` guards the boundary-walk read.
#[allow(clippy::indexing_slicing)]
fn bound_bytes(bytes: &[u8], stop_at_newline: bool) -> Vec<u8> {
    let mut cut = bytes.len().min(MAX_DESCRIPTION_FIELD_LEN);
    if stop_at_newline
        && let Some(newline_pos) = bytes[..cut].iter().position(|&b| b == b'\r' || b == b'\n')
    {
        cut = newline_pos;
    }
    while cut > 0
        && bytes
            .get(cut)
            .is_some_and(|&b| b & 0b1100_0000 == 0b1000_0000)
    {
        cut -= 1;
    }
    bytes[..cut].to_vec()
}

/// Render a [`Value`] for `%s`, applying the R1/R2 description bound when
/// `apply_bound` is set ([`is_bounded_string_family`] gates this at the
/// call site).
///
/// The R1 127-byte budget is measured on FILE-DERIVED bytes, before any
/// lossy UTF-8 decode: for `Value::Bytes` the bound ([`bound_bytes`]) is
/// applied to the raw byte slice first, and only the already-bounded
/// result is decoded via lossy UTF-8. Measuring after decode would let a
/// single invalid byte inflate the count -- `String::from_utf8_lossy`
/// expands each invalid byte into the 3-byte `U+FFFD` replacement
/// character, so bounding the decoded `String`'s byte length would burn
/// the budget up to 3x too fast and cut file-derived content short. This
/// is the high-byte UTF-8 corruption bug class documented in GOTCHAS.md.
///
/// `Value::String` is already valid UTF-8 carrying the file's byte count
/// 1:1 (it comes from `read_string`/`read_string_exact`), so it is bounded
/// directly via [`bound_description_field`]. Numeric values render far
/// under the bound regardless and are never bound.
fn render_string_bounded(value: &Value, apply_bound: bool, stop_at_newline: bool) -> String {
    match value {
        Value::String(s) if apply_bound => bound_description_field(s, stop_at_newline),
        Value::String(s) => s.clone(),
        Value::Bytes(b) if apply_bound => {
            String::from_utf8_lossy(&bound_bytes(b, stop_at_newline)).into_owned()
        }
        Value::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        Value::Uint(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
    }
}

/// Render a `%s` specifier: base string (bounded per R1/R2 for
/// string-family types), then optional `.<precision>` truncation, then
/// width padding.
///
/// The description bound and `.<precision>` truncation compose as a
/// minimum: the bound is applied first, so a precision wider than the
/// bound is a no-op past it, and a precision narrower than the bound still
/// truncates further.
///
/// Precision truncates to at most `precision` characters. Truncation is
/// **char-wise**, not byte-wise: C's `%.Ns` truncates by bytes, but our value
/// is a Rust `String` and a byte-wise cut could split a multi-byte UTF-8
/// sequence. For the ASCII version/name fields that use precision in the magic
/// corpus (`%.3s`, `%-.4s`, `%.10s`) the two are identical; char-wise is the
/// safe choice for the rare non-ASCII case.
///
/// Width padding (via [`pad_non_numeric`]) is applied after truncation so
/// `%4.4s` and `%-.4s` render correctly -- previously `%s` dropped width
/// entirely.
fn render_str_spec(
    spec: &Spec,
    value: &Value,
    type_kind: &TypeKind,
    stop_at_newline: bool,
) -> String {
    let apply_bound = is_bounded_string_family(type_kind);
    let bounded = render_string_bounded(value, apply_bound, stop_at_newline);
    // Truncate to `p` chars in a single pass bounded by `p` (no preceding full
    // `chars().count()` pass, and no byte slicing -- the repo forbids `&s[n..]`
    // for UTF-8 safety). `take(p)` stops after at most `p` chars regardless of
    // string length; when `p >= len` this collects an identical copy, which is
    // a cheap, correct no-op for the rare precision case.
    let truncated = match spec.precision {
        Some(p) => bounded.chars().take(p).collect::<String>(),
        None => bounded,
    };
    pad_non_numeric(&truncated, spec)
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

/// Apply width and alignment to a non-numeric rendered body using space-only padding.
///
/// Used for `%c` (and any other non-numeric conversion where the POSIX `0` flag
/// must be ignored). Zero-padding is not applied regardless of `spec.zero_pad`.
fn pad_non_numeric(body: &str, spec: &Spec) -> String {
    if body.len() >= spec.width {
        return body.to_string();
    }
    let pad = spec.width - body.len();
    let padding: String = std::iter::repeat_n(' ', pad).collect();
    if spec.left_align {
        format!("{body}{padding}")
    } else {
        format!("{padding}{body}")
    }
}

/// Apply width and padding to an already-rendered numeric body.
///
/// For zero-padded right-aligned formatting, a leading `-` sign is kept at
/// the front while zeros are inserted between the sign and the magnitude
/// digits -- matching C printf semantics (e.g., `%05d` with `-7` → `-0007`,
/// not `000-7`).
fn pad_numeric(body: &str, spec: &Spec) -> String {
    if body.len() >= spec.width {
        return body.to_string();
    }
    // C printf sign-aware zero-padding: sign goes before the zeros.
    if spec.zero_pad
        && !spec.left_align
        && let Some(digits) = body.strip_prefix('-')
    {
        let needed = spec.width.saturating_sub(1 + digits.len());
        if needed == 0 {
            return body.to_string();
        }
        let zeros: String = std::iter::repeat_n('0', needed).collect();
        return format!("-{zeros}{digits}");
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
    // Restriction lints without an allow-*-in-tests config option;
    // non-ASCII test data exercises the UTF-8-safe format scanner.
    #![allow(clippy::non_ascii_literal)]

    use super::*;
    use crate::parser::ast::StringFlags;

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

        // %#08o: zero-pad inserts between C-style "0" prefix and digits.
        // C printf uses a single "0" prefix for %#o (not Rust's "0o").
        let out = format_magic_message("%#08o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "00000010");

        // %#X: uppercase alt-form uses "0X" prefix to match the specifier case.
        let out = format_magic_message("%#X", &Value::Uint(0xab), &byte_t());
        assert_eq!(out, "0XAB");
    }

    #[test]
    fn test_string_substitution() {
        let out = format_magic_message(
            "hello %s",
            &Value::String("world".to_string()),
            &TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
        );
        assert_eq!(out, "hello world");

        // Bytes go through lossy UTF-8.
        let out = format_magic_message(
            "data=%s",
            &Value::Bytes(b"abc".to_vec()),
            &TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
        );
        assert_eq!(out, "data=abc");
    }

    #[test]
    fn test_string_precision_truncation() {
        // `%.Ns` truncates the rendered string to at most N characters.
        // sgml's `>15 string/t >\0 %.3s document text` is the motivating
        // rule: the full XML-version field is `1.0" encoding=...` but `%.3s`
        // must render only `1.0` so the description reads `XML 1.0 ...`.
        let str_t = TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        };
        // (template, value, expected)
        let cases: &[(&str, &str, &str)] = &[
            // The XML case: full field truncated to 3 chars.
            (
                "%.3s document text",
                "1.0\" encoding=\"UTF-8\"?>",
                "1.0 document text",
            ),
            // Precision shorter than the string truncates.
            ("%.1s", "1.0", "1"),
            // Precision >= length is a no-op (no padding without width).
            ("%.10s", "abc", "abc"),
            ("%.3s", "abc", "abc"),
            // Precision 0 renders the empty string.
            ("%.0s", "abc", ""),
            // Left-align precision (`-` is a no-op here since no width).
            ("%-.4s", "versionX", "vers"),
            // Width padding is applied AFTER truncation: `%4.4s` on "ab"
            // truncates to "ab" (no-op) then right-aligns to width 4 (pads on
            // the LEFT).
            ("[%4.4s]", "ab", "[  ab]"),
            // `%-4.2s`: truncate "hello" to "he", then `-` left-aligns to
            // width 4 (pads on the RIGHT).
            ("[%-4.2s]", "hello", "[he  ]"),
        ];
        for (template, value, expected) in cases {
            let out = format_magic_message(template, &Value::String((*value).to_string()), &str_t);
            assert_eq!(
                out, *expected,
                "template {template:?} on value {value:?} should render {expected:?}",
            );
        }
    }

    #[test]
    fn test_alt_form_prefix_suppressed_on_zero_value() {
        // C printf special-cases `%#o`, `%#x`, `%#X` with value 0: the
        // alt-form prefix is suppressed because the rendered digit
        // already begins with `0`. Regression guard after pr-review
        // caught that our implementation emitted `"00"` / `"0x0"` /
        // `"0X0"` for zero values.
        let out = format_magic_message("%#o", &Value::Uint(0), &byte_t());
        assert_eq!(out, "0", "%#o with 0 must emit single '0', not '00'");

        let out = format_magic_message("%#x", &Value::Uint(0), &byte_t());
        assert_eq!(out, "0", "%#x with 0 must emit single '0', not '0x0'");

        let out = format_magic_message("%#X", &Value::Uint(0), &byte_t());
        assert_eq!(out, "0", "%#X with 0 must emit single '0', not '0X0'");

        // Non-zero values still get the prefix.
        let out = format_magic_message("%#x", &Value::Uint(1), &byte_t());
        assert_eq!(out, "0x1");
    }

    #[test]
    fn test_octal_substitution() {
        let out = format_magic_message("%o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "10");
        // C printf %#o uses a single "0" prefix, not Rust's "0o".
        let out = format_magic_message("%#o", &Value::Uint(8), &byte_t());
        assert_eq!(out, "010");
    }

    #[test]
    fn test_char_substitution() {
        let out = format_magic_message("[%c]", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "[A]");

        // Full 0x00-0xff range: bytes >= 0x80 are embedded as Latin-1 code points.
        let out = format_magic_message("%c", &Value::Uint(0xa9), &byte_t());
        assert_eq!(out, "\u{00a9}"); // U+00A9 COPYRIGHT SIGN

        // Width with space-padding (right-aligned).
        let out = format_magic_message("%3c", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "  A");

        // Left-aligned width.
        let out = format_magic_message("%-3c|", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "A  |");
    }

    #[test]
    fn test_char_zero_flag_ignored() {
        // POSIX: the `0` flag is ignored for `%c` -- zero-padding applies only to
        // numeric conversions. `%03c` must produce space-padded "  A", not "00A".
        // Regression guard: an earlier revision called `pad_numeric` for `Conv::Char`,
        // which applied zero-padding and diverged from C printf semantics.
        let out = format_magic_message("%03c", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "  A", "%03c must use space-padding, not zero-padding");

        // Combined zero and left-align: `-` overrides `0` for numerics; for %c
        // `0` was never active, but `-` still triggers left-alignment.
        let out = format_magic_message("%-03c|", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "A  |", "%-03c must left-align with spaces");
    }

    #[test]
    fn test_percent_escape() {
        let out = format_magic_message("100%% sure", &Value::Uint(0), &byte_t());
        assert_eq!(out, "100% sure");
    }

    #[test]
    fn test_non_ascii_template_preserved() {
        // Regression guard: earlier revisions iterated by byte and
        // pushed each `b as char`, which re-encoded non-ASCII UTF-8
        // continuation bytes as Latin-1 code points and mangled the
        // output (e.g., "café" -> "cafÃ©"). The plain-run flush path
        // must copy slices of the original template to preserve the
        // original UTF-8 byte sequences.
        let out = format_magic_message("café %d", &Value::Int(42), &long_t());
        assert_eq!(out, "café 42");

        // Non-ASCII around a specifier on both sides.
        let out = format_magic_message("→ %s ←", &Value::String("ok".into()), &byte_t());
        assert_eq!(out, "→ ok ←");

        // Non-ASCII only, no specifiers.
        let out = format_magic_message("über", &Value::Uint(0), &byte_t());
        assert_eq!(out, "über");
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
        // Zero-padded width with negative value: sign must precede zeros.
        // Regression guard for sign-aware zero-padding (C printf semantics).
        let out = format_magic_message("%05d", &Value::Int(-7), &long_t());
        assert_eq!(out, "-0007");
        let out = format_magic_message("%06d", &Value::Int(-42), &long_t());
        assert_eq!(out, "-00042");
        // Zero-padded width.
        let out = format_magic_message("%05d", &Value::Int(42), &long_t());
        assert_eq!(out, "00042");
        // Space-padded width.
        let out = format_magic_message("%5d", &Value::Int(42), &long_t());
        assert_eq!(out, "   42");
        // Negative with space-padding: sign stays in the body, spaces lead.
        let out = format_magic_message("%5d", &Value::Int(-7), &long_t());
        assert_eq!(out, "   -7");
        // Left-aligned (zero flag ignored when `-` is set).
        let out = format_magic_message("%-5d|", &Value::Int(42), &long_t());
        assert_eq!(out, "42   |");
        // Left-aligned negative: body left-aligned, spaces trail.
        let out = format_magic_message("%-6d|", &Value::Int(-7), &long_t());
        assert_eq!(out, "-7    |");
    }

    #[test]
    fn test_width_cap_prevents_large_allocation() {
        // A width larger than MAX_FORMAT_WIDTH must be silently clamped.
        // The output should be valid (the value rendered, possibly padded)
        // rather than triggering a huge allocation.
        let huge_width = format!("%{}d", usize::MAX);
        let out = format_magic_message(&huge_width, &Value::Int(1), &long_t());
        // After clamping, the output is at most MAX_FORMAT_WIDTH+1 chars.
        assert!(
            out.len() <= MAX_FORMAT_WIDTH + 1,
            "output too long: {}",
            out.len()
        );
        assert!(out.ends_with('1'), "rendered value must appear: {out:?}");
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
            &TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
        );
        assert_eq!(out, "v=%d");
    }

    #[test]
    fn test_char_specifier_accepts_full_byte_range() {
        // `%c` emits every byte value 0x00..=0xff directly, matching
        // GNU `file` / C printf semantics. Bytes 0x80-0xff are embedded
        // as their Latin-1 code points via `char::from(u8)`.
        // 0xff maps to U+00FF ('ÿ'); UTF-8 encoding is 0xc3 0xbf.
        let out = format_magic_message("[%c]", &Value::Uint(0xff), &byte_t());
        assert_eq!(out, "[\u{00ff}]");

        // ASCII boundary stays unchanged.
        let out = format_magic_message("[%c]", &Value::Uint(u64::from(b'A')), &byte_t());
        assert_eq!(out, "[A]");

        // Out-of-range (doesn't fit u8) passes through literally.
        let out = format_magic_message("[%c]", &Value::Uint(0x1_0000), &byte_t());
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

    // ---- R1/R2/R3/R5/R14: bounded description field -------------------

    fn string16_t() -> TypeKind {
        TypeKind::String16 {
            endian: crate::parser::ast::Endianness::Little,
        }
    }

    fn pstring_t() -> TypeKind {
        TypeKind::PString {
            max_length: None,
            length_width: crate::parser::ast::PStringLengthWidth::OneByte,
            length_includes_itself: false,
        }
    }

    fn regex_t() -> TypeKind {
        TypeKind::Regex {
            flags: crate::parser::ast::RegexFlags::default(),
            count: crate::parser::ast::RegexCount::Default,
        }
    }

    fn str_t() -> TypeKind {
        TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        }
    }

    #[test]
    fn test_bound_truncates_long_ascii_value_to_127_bytes() {
        // AE1: a 304-byte ASCII value renders exactly 127 bytes.
        let value = "A".repeat(304);
        let out = format_magic_message_with_gate("%s", &Value::String(value), &str_t(), false);
        assert_eq!(out.len(), 127, "rendered field must be exactly 127 bytes");
    }

    #[test]
    fn test_bound_boundary_lengths_127_passes_128_truncates() {
        let exactly_127 = "B".repeat(127);
        let out = format_magic_message_with_gate(
            "%s",
            &Value::String(exactly_127.clone()),
            &str_t(),
            false,
        );
        assert_eq!(
            out, exactly_127,
            "a 127-byte value must pass through untouched"
        );

        let exactly_128 = "B".repeat(128);
        let out =
            format_magic_message_with_gate("%s", &Value::String(exactly_128), &str_t(), false);
        assert_eq!(out.len(), 127, "a 128-byte value must render as 127 bytes");
    }

    #[test]
    fn test_bound_cuts_on_utf8_character_boundary() {
        // 126 ASCII bytes followed by a 2-byte UTF-8 character ('\u{00e9}',
        // "e" with acute accent) straddling the 127-byte cut: bytes 126-127
        // hold the character's first byte, byte 128 its second. The cut
        // must land at 126, not split the character.
        let mut value = "C".repeat(126);
        value.push('\u{00e9}');
        value.push_str("TAIL");
        assert_eq!(
            value.as_bytes()[126],
            0xc3,
            "fixture sanity: char starts at 126"
        );

        let out = format_magic_message_with_gate("%s", &Value::String(value), &str_t(), false);
        assert!(
            std::str::from_utf8(out.as_bytes()).is_ok(),
            "output must be valid UTF-8"
        );
        assert_eq!(
            out.len(),
            126,
            "the straddling character is dropped whole, not split"
        );
        assert_eq!(out, "C".repeat(126));
    }

    #[test]
    fn test_gated_rule_stops_at_first_newline() {
        let out = format_magic_message_with_gate(
            "STR=[%s]",
            &Value::String("ZZZZABC\nSECOND".to_string()),
            &str_t(),
            true,
        );
        assert_eq!(out, "STR=[ZZZZABC]", "must stop before the \\n");
    }

    #[test]
    fn test_gated_rule_stops_at_first_carriage_return() {
        let out = format_magic_message_with_gate(
            "STR=[%s]",
            &Value::String("ZZZZABC\rSECOND".to_string()),
            &str_t(),
            true,
        );
        assert_eq!(out, "STR=[ZZZZABC]", "must stop before the \\r");
    }

    #[test]
    fn test_ungated_rule_does_not_stop_at_newline() {
        // An equality-compared rule (stop_at_newline = false) renders the
        // whole field, newline and all.
        let out = format_magic_message_with_gate(
            "STR=[%s]",
            &Value::String("ZZZZABC\nSECOND".to_string()),
            &str_t(),
            false,
        );
        assert_eq!(out, "STR=[ZZZZABC\nSECOND]");
    }

    #[test]
    fn test_regex_type_exempt_from_bound_and_newline_stop() {
        // R5: regex renders through a separate, unbounded path -- even
        // when `stop_at_newline` is (incorrectly) set to true by a caller,
        // the regex arm must not apply either the 127-byte bound or the
        // newline stop.
        let mut value = "D".repeat(150);
        value.push('\n');
        value.push_str("more-after-newline");
        let expected = value.clone();
        let out = format_magic_message_with_gate("%s", &Value::String(value), &regex_t(), true);
        assert_eq!(
            out, expected,
            "regex must render past 127 bytes and across a newline"
        );
    }

    #[test]
    fn test_pstring_and_string16_are_bounded_and_gated() {
        let value = "E".repeat(304);
        for type_kind in [pstring_t(), string16_t()] {
            let out = format_magic_message_with_gate(
                "%s",
                &Value::String(value.clone()),
                &type_kind,
                false,
            );
            assert_eq!(
                out.len(),
                127,
                "type {type_kind:?} must be bounded to 127 bytes"
            );
        }

        let mut newline_value = "F".repeat(10);
        newline_value.push('\n');
        newline_value.push_str("tail");
        for type_kind in [pstring_t(), string16_t()] {
            let out = format_magic_message_with_gate(
                "%s",
                &Value::String(newline_value.clone()),
                &type_kind,
                true,
            );
            assert_eq!(
                out,
                "F".repeat(10),
                "type {type_kind:?} must stop at newline when gated"
            );
        }
    }

    #[test]
    fn test_bound_measured_on_raw_bytes_not_lossy_decoded_string() {
        // The 127-byte budget must be measured on FILE-DERIVED bytes,
        // before any lossy UTF-8 decode. `String::from_utf8_lossy` expands
        // each invalid byte into the 3-byte U+FFFD replacement character;
        // if the bound were measured on the decoded String's byte length,
        // 3 invalid bytes among 200 raw bytes would inflate to 9 decoded
        // bytes and burn the 127-byte budget far too fast, rendering a
        // field derived from fewer than 127 *file* bytes.
        let mut raw = vec![b'H'; 200];
        raw[10] = 0xff;
        raw[11] = 0xff;
        raw[12] = 0xff;

        let out = format_magic_message_with_gate("%s", &Value::Bytes(raw.clone()), &str_t(), false);

        // Decode exactly the first 127 RAW bytes the same way the
        // implementation must, and compare against that -- not against a
        // fixed byte length, since 3 invalid bytes decode to 9 output
        // bytes (3 x U+FFFD), so the correct rendered length is
        // 127 - 3 (dropped invalid bytes) + 3*3 (their U+FFFD encoding)
        // relative to the raw prefix, not a naive 127.
        let expected = String::from_utf8_lossy(&raw[..127]).into_owned();
        assert_eq!(
            out, expected,
            "must decode exactly the first 127 raw file bytes, not 127 post-decode bytes"
        );
    }

    #[test]
    fn test_precision_composes_as_minimum_with_bound() {
        // `%.200s` and `%.50s` each compose with the 127-byte bound as a
        // minimum: precision wider than the bound is a no-op past it,
        // precision narrower than the bound truncates further.
        let value = "G".repeat(300);

        let out = format_magic_message_with_gate(
            "%.200s",
            &Value::String(value.clone()),
            &str_t(),
            false,
        );
        assert_eq!(
            out.len(),
            127,
            "precision wider than the bound is capped at the bound"
        );

        let out = format_magic_message_with_gate("%.50s", &Value::String(value), &str_t(), false);
        assert_eq!(
            out.len(),
            50,
            "precision narrower than the bound still applies"
        );
    }
}
