// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for `search` anchor-advance semantics
//! (match-end vs. window-end) and signed-masked-long / high-byte
//! `string` value matching against real binary-format signatures.

use super::*;

/// A child rule with `OffsetSpec::Relative(0)` after a parent search match
/// must land at `match_index + pattern.len()` — NOT at `window_end` (the
/// pre-fix window-size advance would land on a completely different byte).
#[test]
fn test_search_parent_advances_anchor_to_match_end_not_window_end() {
    // Buffer: "XXXneedleYY_ZZ" -- parent `search/32 "needle"` finds the
    // pattern at index 3, length 6, match-end = 9. A Relative(0) child
    // should read byte 9 = 'Y' (0x59). With the bug, the anchor would
    // advance by 32 bytes (way past the buffer) or (with range=14) by 14
    // to index 14 which is past the buffer end.
    let child = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'Y')),
        message: "trailing Y".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(14),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("needle".to_string()),
        message: "found needle".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"XXXneedleYY_ZZ", &mut context).unwrap();
    assert_eq!(matches.len(), 2, "expected parent + child, got {matches:?}");
    assert_eq!(matches[1].message, "trailing Y");
}

/// Sanity check the negative: when the parent search finds the pattern
/// early in the window, a Relative(-N) child should still resolve against
/// the match-end anchor. This catches a class of bugs where the anchor
/// update uses the wrong base offset.
#[test]
fn test_search_parent_relative_child_at_positive_offset() {
    // Buffer: "prefix_NEEDLE_after_stuff" -- "NEEDLE" is at index 7, len
    // 6, match-end = 13. A Relative(1) child should read byte 14 = 'a'.
    let child = MagicRule {
        offset: OffsetSpec::Relative(1),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(u64::from(b'a')),
        message: "a after".to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
        value_transform: None,
    };
    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Search {
            range: ::std::num::NonZeroUsize::new(32),
            flags: SearchFlags::default(),
        },
        op: Operator::Equal,
        value: Value::String("NEEDLE".to_string()),
        message: "found".to_string(),
        children: vec![child],
        level: 0,
        strength_modifier: None,
        value_transform: None,
    };

    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches = evaluate_rules(&[parent], b"prefix_NEEDLE_after_stuff", &mut context).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].message, "a after");
}

/// Regression (end-to-end): the Mach-O 64-bit signature rule
/// `0 lelong&0xfffffffe 0xfeedface` must match a buffer beginning with the
/// little-endian magic `cf fa ed fe`. `lelong` is signed, so the read is
/// sign-extended to i64; before the width-aware masked-comparison fix the
/// 32-bit mask cleared the high bits (making the result positive) while the
/// rule literal stayed sign-extended (negative), so the two i64 values never
/// compared equal and the rule silently failed -- letting a weak
/// `measure`/Lepton rule win on real Mach-O binaries. See
/// `operators::apply_bitwise_and_mask_with_width`.
#[test]
fn test_signed_masked_long_matches_macho_signature_end_to_end() {
    use crate::parser::grammar::parse_magic_rule;

    let (_, rule) = parse_magic_rule("0\tlelong&0xfffffffe\t0xfeedface\tMach-O")
        .expect("the Mach-O magic rule must parse");
    // Real Mach-O 64-bit header prefix (0xFEEDFACF little-endian) + padding.
    let buffer = [0xcf_u8, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01];
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches =
        evaluate_rules(&[rule], &buffer, &mut context).expect("evaluation must not fatally error");
    assert!(
        matches.iter().any(|m| m.message.contains("Mach-O")),
        "signed lelong&0xfffffffe must detect the Mach-O signature, got: {matches:?}"
    );

    // Negative control: a non-Mach-O long must NOT match.
    let (_, rule2) =
        parse_magic_rule("0\tlelong&0xfffffffe\t0xfeedface\tMach-O").expect("rule must parse");
    let other = [0x12_u8, 0x34, 0x56, 0x78];
    let mut ctx2 = EvaluationContext::new(EvaluationConfig::default());
    let none = evaluate_rules(&[rule2], &other, &mut ctx2).expect("evaluation must not error");
    assert!(
        !none.iter().any(|m| m.message.contains("Mach-O")),
        "a non-Mach-O buffer must not match the signature"
    );
}

/// Regression (end-to-end): a `string` rule whose value contains a byte
/// `>= 0x80` (invalid UTF-8) must still match the raw file bytes. The gzip
/// signature `0 string \037\213` (bytes 0x1f 0x8b, `\213` = octal 0x8b) used
/// to silently never match: `read_string_exact` decoded the file bytes via
/// lossy UTF-8, turning 0x8b into U+FFFD, which never equalled the raw-byte
/// pattern -- so gzip (and any high-byte string signature) classified as
/// `data`. `read_string_exact` now returns the raw bytes as `Value::Bytes`
/// for non-UTF-8 slices; `apply_equal` compares Bytes/String by byte sequence.
#[test]
fn test_string_rule_with_high_byte_value_matches_raw_bytes_end_to_end() {
    use crate::parser::grammar::parse_magic_rule;

    // gzip magic: 0x1f 0x8b via octal escapes.
    let (_, rule) = parse_magic_rule("0\tstring\t\\037\\213\tgzip compressed data")
        .expect("gzip magic rule must parse");
    let buffer = [0x1f_u8, 0x8b, 0x08, 0x00]; // real gzip header prefix
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    let matches =
        evaluate_rules(&[rule], &buffer, &mut context).expect("evaluation must not error");
    assert!(
        matches.iter().any(|m| m.message.contains("gzip")),
        "a high-byte string value must match the raw file bytes, got: {matches:?}"
    );

    // Negative control: a buffer without the signature must not match.
    let (_, rule2) =
        parse_magic_rule("0\tstring\t\\037\\213\tgzip compressed data").expect("rule must parse");
    let other = [0x1f_u8, 0x9d, 0x00, 0x00]; // 0x1f 0x9d is compress(1), not gzip
    let mut ctx2 = EvaluationContext::new(EvaluationConfig::default());
    let none = evaluate_rules(&[rule2], &other, &mut ctx2).expect("evaluation must not error");
    assert!(
        !none.iter().any(|m| m.message.contains("gzip")),
        "a non-gzip high byte must not match the gzip signature"
    );
}

// Flagged-string engine dispatch tests are split into
// `string_flags_dispatch_tests.rs` (see the submodule declaration at the
// bottom of this file). They cover the engine routing layer; lower-level
// `compare_string_with_flags` tests live in
// `src/evaluator/types/string.rs::tests`, and integration / conformance
// coverage lives in `tests/string_flags_integration.rs`.

// Shared test helpers have been extracted into the `helpers` sub-tree so
// this module stays focused on its own test wiring; the meta_* submodules
// continue to access helpers via `super::*` thanks to the glob re-export
// below. The three bare `use` items are for types that the submodules
// still reference directly (e.g. `MetaType::Default`, `RuleEnvironment {
// ... }` literal construction) and therefore must stay in this module's
// namespace for `super::*` to reach.
