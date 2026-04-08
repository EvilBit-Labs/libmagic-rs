// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for relative offset (`OffsetSpec::Relative`) evaluation.
//!
//! Relative offsets resolve against the end of the most recent successful
//! match (the GNU `file` "previous match" anchor). The evaluation engine
//! threads this anchor through `EvaluationContext::last_match_end()`, and
//! advances it after each successful match by the number of bytes the read
//! consumed.
//!
//! Magic-file syntax for `&+N`/`&-N` is not yet wired into the parser, so
//! these tests construct rules programmatically and exercise them through
//! `evaluate_rules` directly.

use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
use libmagic_rs::parser::ast::PStringLengthWidth;
use libmagic_rs::{Endianness, EvaluationConfig, MagicRule, OffsetSpec, Operator, TypeKind, Value};

fn cfg() -> EvaluationConfig {
    EvaluationConfig {
        stop_at_first_match: false,
        ..Default::default()
    }
}

fn child_rule(offset: OffsetSpec, typ: TypeKind, value: Value, message: &str) -> MagicRule {
    MagicRule {
        offset,
        typ,
        op: Operator::Equal,
        value,
        message: message.to_string(),
        children: vec![],
        level: 1,
        strength_modifier: None,
    }
}

#[test]
fn relative_child_after_long_parent() {
    // Buffer: 4-byte LE long (0x12345678) followed by another 4-byte LE long
    // (0xCAFEBABE). Parent matches the first long, child uses Relative(0)
    // and reads at offset 4 (= parent end).
    let buffer = [0x78, 0x56, 0x34, 0x12, 0xBE, 0xBA, 0xFE, 0xCA];

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "parent-long".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(0),
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            Value::Uint(0xCAFE_BABE),
            "child-long",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2, "expected parent + child match");
    assert_eq!(matches[0].message, "parent-long");
    assert_eq!(matches[0].offset, 0);
    assert_eq!(matches[1].message, "child-long");
    assert_eq!(matches[1].offset, 4);
}

#[test]
fn relative_child_with_positive_delta() {
    // Parent matches one byte at offset 0; child uses Relative(2) and reads
    // at offset 1 (parent_end) + 2 = 3.
    let buffer = [0x7F, 0xAA, 0xBB, 0x42, 0xCC];

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x7F),
        message: "p".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(2),
            TypeKind::Byte { signed: false },
            Value::Uint(0x42),
            "c",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].offset, 3);
}

#[test]
fn relative_child_with_negative_delta() {
    // Parent matches a 4-byte long at offset 4; child Relative(-7) reads at
    // (4+4) - 7 = 1.
    let buffer = [0x00, 0xAA, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12, 0x00];

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "p".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(-7),
            TypeKind::Byte { signed: false },
            Value::Uint(0xAA),
            "c",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].offset, 1);
    assert_eq!(matches[1].value, Value::Uint(0xAA));
}

#[test]
fn relative_chain_marches_forward() {
    // Three consecutive 4-byte LE longs; root + two relative children.
    let buffer = [
        0x78, 0x56, 0x34, 0x12, // 0x12345678
        0xBE, 0xBA, 0xFE, 0xCA, // 0xCAFEBABE
        0xEF, 0xBE, 0xAD, 0xDE, // 0xDEADBEEF
    ];

    let leaf = child_rule(
        OffsetSpec::Relative(0),
        TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        Value::Uint(0xDEAD_BEEF),
        "leaf",
    );
    let mut middle = child_rule(
        OffsetSpec::Relative(0),
        TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        Value::Uint(0xCAFE_BABE),
        "middle",
    );
    middle.children = vec![leaf];

    let root = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "root".to_string(),
        children: vec![middle],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[root], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 3);
    let offsets: Vec<usize> = matches.iter().map(|m| m.offset).collect();
    assert_eq!(offsets, vec![0, 4, 8]);
}

#[test]
fn relative_after_string_parent_includes_nul_terminator() {
    // String "MZ" at offset 0 followed by NUL (3 bytes consumed), then a
    // byte the child reads via Relative(0).
    let buffer = b"MZ\x00\x42rest";

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("MZ".to_string()),
        message: "mz".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Value::Uint(0x42),
            "byte-after-mz",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2, "child should match after MZ + NUL");
    assert_eq!(matches[1].offset, 3);
}

#[test]
fn relative_after_pstring_parent_consumes_prefix_and_payload() {
    // pstring(/B) at offset 0 with prefix 0x05, payload "Hello" (6 bytes
    // total), then a byte at offset 6.
    let buffer = b"\x05Hello\x42tail";

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        },
        op: Operator::Equal,
        value: Value::String("Hello".to_string()),
        message: "pstr".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(0),
            TypeKind::Byte { signed: false },
            Value::Uint(0x42),
            "byte-after-pstr",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[1].offset, 6);
}

#[test]
fn relative_top_level_resolves_from_zero_anchor() {
    // No prior match: top-level Relative(2) -> absolute 2.
    let buffer = [0xAA, 0xBB, 0x42, 0xCC];

    let rule = MagicRule {
        offset: OffsetSpec::Relative(2),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "top".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[rule], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].offset, 2);
}

#[test]
fn relative_sibling_propagation_at_top_level() {
    // GNU `file` semantics: anchor advances monotonically; the second
    // top-level rule sees the anchor that the first rule left behind.
    // First rule matches a 4-byte long at offset 0 -> anchor becomes 4.
    // Second rule uses Relative(0) -> reads at offset 4.
    let buffer = [0x78, 0x56, 0x34, 0x12, 0x42, 0x00, 0x00, 0x00];

    let first = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "first".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let second = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "second".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[first, second], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].offset, 0);
    assert_eq!(matches[1].offset, 4);
}

#[test]
fn relative_out_of_bounds_skips_child_gracefully() {
    // Parent matches; child uses Relative(50) which lands past the buffer.
    // Engine should skip the child and continue without panicking.
    let buffer = [0x7F, 0xAA, 0xBB];

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x7F),
        message: "p".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(50),
            TypeKind::Byte { signed: false },
            Value::Uint(0x00),
            "c",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 1, "only the parent should match");
    assert_eq!(matches[0].message, "p");
}

#[test]
fn relative_anchor_can_decrease_when_later_sibling_matches_at_lower_position() {
    // GNU `file` semantics: the anchor reflects the END of the most recent
    // match -- not a high-watermark. If a later sibling matches at a lower
    // absolute position, the anchor moves backwards. This test pins the
    // documented "may increase or decrease" behavior so a future
    // optimization that adds a max() guard fails loudly.
    //
    // Buffer layout:
    //   offset 0: 0x42 (matched by rule_b at offset 2 via Absolute(2)... wait,
    //             we need rule_a to match HIGHER, then rule_b to match LOWER.)
    //
    // Layout: 16 bytes. Rule A matches a 4-byte LE long at offset 8.
    // After A, anchor = 12. Rule B matches a single byte at offset 2
    // (Absolute(2)). After B, anchor = 3. Rule C uses Relative(0) and
    // must read at offset 3, NOT offset 12.
    let buffer = [
        0x00, 0x00, 0xAA, 0x99, 0x00, 0x00, 0x00, 0x00, // bytes 0-7
        0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, // bytes 8-15
    ];

    let rule_a = MagicRule {
        offset: OffsetSpec::Absolute(8),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "rule-a-high".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let rule_b = MagicRule {
        offset: OffsetSpec::Absolute(2),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xAA),
        message: "rule-b-low".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let rule_c = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x99),
        message: "rule-c-relative".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[rule_a, rule_b, rule_c], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 3, "all three rules should match");
    assert_eq!(matches[0].message, "rule-a-high");
    assert_eq!(matches[0].offset, 8);
    assert_eq!(matches[1].message, "rule-b-low");
    assert_eq!(matches[1].offset, 2);
    assert_eq!(
        matches[2].offset, 3,
        "rule C must read at offset 3 (rule B's end), proving the anchor moved backwards from 12 -> 3"
    );
}

#[test]
fn relative_anchor_persists_across_non_matching_intermediate_sibling() {
    // First top-level rule matches a 4-byte LE long -> anchor advances to 4.
    // Second top-level rule does NOT match (wrong expected value) -> anchor
    // stays at 4.
    // Third top-level rule uses Relative(0) -> reads at offset 4.
    let buffer = [0x78, 0x56, 0x34, 0x12, 0x42, 0x00, 0x00, 0x00];

    let first = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "first".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let middle_no_match = MagicRule {
        offset: OffsetSpec::Absolute(4),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0xDE), // does not match (real byte is 0x42)
        message: "middle-skip".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let third = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "third".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[first, middle_no_match, third], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 2, "first + third match, middle skipped");
    assert_eq!(matches[0].message, "first");
    assert_eq!(matches[1].message, "third");
    assert_eq!(matches[1].offset, 4);
}

#[test]
fn relative_anchor_resets_between_evaluations_via_reset() {
    // Evaluate against a first buffer, advancing the anchor. Reset the
    // context. Evaluate against a second buffer with a Relative(0) rule;
    // the anchor must start at 0, not the leaked value from the first run.
    let buffer_a = [0x78, 0x56, 0x34, 0x12];
    let buffer_b = [0x42, 0xAA, 0xBB, 0xCC];

    let pass_one = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        },
        op: Operator::Equal,
        value: Value::Uint(0x1234_5678),
        message: "pass-one".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };
    let pass_two = MagicRule {
        offset: OffsetSpec::Relative(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x42),
        message: "pass-two".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let _ = evaluate_rules(&[pass_one], &buffer_a, &mut ctx).unwrap();
    ctx.reset();
    let matches = evaluate_rules(&[pass_two], &buffer_b, &mut ctx).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].offset, 0,
        "Relative(0) should resolve to 0 after reset"
    );
}

#[test]
fn relative_underflow_skips_child_gracefully() {
    // Anchor=1 (after parent byte), child Relative(-100) underflows.
    let buffer = [0x7F, 0xAA];

    let parent = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::Equal,
        value: Value::Uint(0x7F),
        message: "p".to_string(),
        children: vec![child_rule(
            OffsetSpec::Relative(-100),
            TypeKind::Byte { signed: false },
            Value::Uint(0x00),
            "c",
        )],
        level: 0,
        strength_modifier: None,
    };

    let mut ctx = EvaluationContext::new(cfg());
    let matches = evaluate_rules(&[parent], &buffer, &mut ctx).unwrap();
    assert_eq!(matches.len(), 1);
}
