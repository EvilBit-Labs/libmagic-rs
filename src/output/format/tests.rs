// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`super`]'s printf-style format substitution.
//!
//! Split out of `format.rs` to keep the production module inside the
//! project's file-size convention.

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
    let out =
        format_magic_message_with_gate("%s", &Value::String(exactly_127.clone()), &str_t(), false);
    assert_eq!(
        out, exactly_127,
        "a 127-byte value must pass through untouched"
    );

    let exactly_128 = "B".repeat(128);
    let out = format_magic_message_with_gate("%s", &Value::String(exactly_128), &str_t(), false);
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
        let out =
            format_magic_message_with_gate("%s", &Value::String(value.clone()), &type_kind, false);
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

    let out =
        format_magic_message_with_gate("%.200s", &Value::String(value.clone()), &str_t(), false);
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
