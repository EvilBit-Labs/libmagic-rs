// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Coverage for the use-vs-indirect result-framing contract (issue #471).
//!
//! A `use` rule's Indirect result is base-relative; an `indirect` rule's stays
//! absolute. Same `(N.T+adj)` syntax, opposite result rules, and the only
//! discriminator is the invoking rule's meta-type -- so both directions are
//! pinned here end to end, not just at the helper (GOTCHAS S3.10).
//!
//! Split out of `meta_use_tests` to keep that file within the project's
//! file-size guidance, matching the themed-child-file pattern its
//! `meta_offset_tests` and `meta_default_clear_indirect_tests` siblings use.
//!
//! Every offset below is derived from the arithmetic GOTCHAS S3.10 states, not
//! recorded from a run of the current code.

use super::*;

fn jpeg_style_indirect_spec() -> OffsetSpec {
    OffsetSpec::Indirect {
        base_offset: 2,
        base_relative: false,
        // Signed by default (GOTCHAS S3.7/S6.3): `.S` parses to signed.
        pointer_type: TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        },
        adjustment: 2,
        adjustment_op: IndirectAdjustmentOp::Add,
        result_relative: false,
        endian: Endianness::Big,
    }
}

/// The measured jpeg walk: `jpeg_segment` invoked at use-site 20 reads a
/// segment length of 128 at byte 22, so the next segment is at
/// `20 + (128 + 2) = 150`. Dropping the base would land on 130 instead.
#[test]
fn test_use_indirect_result_is_base_relative_jpeg_shape() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Inner subroutine reads its own byte 0. Inside a `use`, `Absolute(0)`
    // resolves to the use-site base, so this reads whatever position the
    // framing rule produced.
    let inner = vec![byte_eq_rule(0, 0xAA, "reached-150")];
    // Outer subroutine holds the indirect `use`, mirroring jpeg_segment's
    // recursive `>>(2.S+2) use jpeg_segment`.
    let outer = vec![use_rule_indirect_at("inner", jpeg_style_indirect_spec())];

    let table = build_name_table(vec![("outer", outer), ("inner", inner)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[][..]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let mut buffer = vec![0x00u8; 160];
    // Segment length 128 as a big-endian short at byte 22 (= base 20 + 2).
    buffer[22] = 0x00;
    buffer[23] = 0x80;
    // Sentinel at the correct target; the unrebased position is left
    // deliberately non-matching. Only one read happens, so this byte documents
    // where a dropped rebase would land rather than discriminating on its own.
    buffer[150] = 0xAA;
    buffer[130] = 0xBB;

    // Outer `use` at literal site 20 sets the caller base the inner use reads.
    let rules = vec![use_rule_at("outer", 20)];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| m.message.contains("reached-150")),
        "a `use` rule's Indirect result must be rebased by the use-site base \
         (20 + 130 = 150, GOTCHAS S3.10); a match at 130 means the base was \
         dropped. got {matches:?}"
    );
}

/// The same rule invoked at use-site 0 must resolve unrebased -- the
/// end-to-end counterpart of the helper's zero-base case.
#[test]
fn test_use_indirect_result_at_zero_base_is_unrebased() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };
    let inner = vec![byte_eq_rule(0, 0xBB, "reached-130")];
    let outer_rule = use_rule_indirect_at("inner", jpeg_style_indirect_spec());

    let table = build_name_table(vec![("inner", inner)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(&[][..]),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let mut buffer = vec![0x00u8; 160];
    // At base 0 the pointer site is byte 2, not byte 22.
    buffer[2] = 0x00;
    buffer[3] = 0x80;
    buffer[130] = 0xBB;
    buffer[150] = 0xAA;

    let matches = evaluate_rules(&[outer_rule], &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| m.message.contains("reached-130")),
        "at a zero base the Indirect result is already absolute and must not \
         move; got {matches:?}"
    );
}

/// mach-o's `>(8.L) indirect x` inside `use mach-o` at file offset 8: the
/// pointer read site is biased by the base, but the value it yields is an
/// absolute file position and must NOT be rebased. Here the pointer at
/// `base(8) + 8 = 16` holds 64, so re-entry happens at 64, not at 8 + 64 = 72.
#[test]
fn test_indirect_result_inside_use_stays_absolute_macho_shape() {
    let config = EvaluationConfig {
        stop_at_first_match: false,
        ..EvaluationConfig::default()
    };

    // Root rules run against the sliced sub-buffer with a reset base, so
    // `Absolute(0)` is the first byte at the re-entry position.
    let root_rules = vec![byte_eq_rule(0, 0xAA, "reentered-at-64")];

    let subroutine = vec![indirect_rule_at_indirect_offset(
        OffsetSpec::Indirect {
            base_offset: 8,
            base_relative: false,
            // Signed by default (GOTCHAS S3.7/S6.3): `.L` parses to signed.
            pointer_type: TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            adjustment: 0,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: Endianness::Big,
        },
        "",
    )];

    let table = build_name_table(vec![("macho", subroutine)]);
    let env = std::sync::Arc::new(RuleEnvironment {
        name_table: std::sync::Arc::new(table),
        root_rules: std::sync::Arc::from(root_rules.as_slice()),
    });
    let mut context = EvaluationContext::new(config).with_rule_env(env);

    let mut buffer = vec![0x00u8; 128];
    // arch[0].offset as a big-endian long at byte 16 (= base 8 + 8), value 64.
    buffer[16..20].copy_from_slice(&64u32.to_be_bytes());
    // Payload at the absolute position; the wrongly-rebased position is left
    // deliberately non-matching (documentary, not discriminating -- see above).
    buffer[64] = 0xAA;
    buffer[72] = 0xBB;

    let rules = vec![use_rule_at("macho", 8)];
    let matches = evaluate_rules(&rules, &buffer, &mut context).unwrap();

    assert!(
        matches.iter().any(|m| {
            crate::evaluator::strip_no_separator_marker(&m.message).unwrap_or(&m.message)
                == "reentered-at-64"
        }),
        "an `indirect` rule's dereferenced result is an absolute file position \
         and must not be biased by the subroutine base (GOTCHAS S3.10); a match \
         at 72 would mean it was. got {matches:?}"
    );
}
