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
/// the rendered field is bounded to at most `MAX_DESCRIPTION_FIELD_LEN`
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
/// `MAX_DESCRIPTION_FIELD_LEN`-byte bound. The caller computes
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

/// Shared first step of both description bounds: clamp to
/// `MAX_DESCRIPTION_FIELD_LEN` and, when gated, stop at the first `\r` or
/// `\n` inside that window.
///
/// Callers then walk the result back to their own boundary kind -- a `str`
/// char boundary for [`bound_description_field`], a UTF-8 continuation-byte
/// boundary for [`bound_bytes`] -- which is why only this prefix is shared.
// Slicing is invariant-safe: `cut` is clamped to `bytes.len()` on the line
// above before `bytes[..cut]` is taken.
#[allow(clippy::indexing_slicing)]
fn bounded_newline_cut(bytes: &[u8], stop_at_newline: bool) -> usize {
    let mut cut = bytes.len().min(MAX_DESCRIPTION_FIELD_LEN);
    if stop_at_newline
        && let Some(newline_pos) = bytes[..cut].iter().position(|&b| b == b'\r' || b == b'\n')
    {
        cut = newline_pos;
    }
    cut
}

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

/// Bound `base` to at most `MAX_DESCRIPTION_FIELD_LEN` bytes, cutting at
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
    let mut cut = bounded_newline_cut(base.as_bytes(), stop_at_newline);
    while cut > 0 && !base.is_char_boundary(cut) {
        cut -= 1;
    }
    base[..cut].to_string()
}

/// Raw-byte counterpart of [`bound_description_field`] for `Value::Bytes`.
///
/// Operates on the file-derived byte slice directly, before any lossy
/// UTF-8 decode, so the `MAX_DESCRIPTION_FIELD_LEN` budget is measured on
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
    let mut cut = bounded_newline_cut(bytes, stop_at_newline);
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
mod tests;
