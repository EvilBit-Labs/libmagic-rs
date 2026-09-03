// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Branch coverage for [`apply_use_result_base`] (issue #471).
//!
//! `apply_use_result_base` decides whether a dereferenced indirect offset is
//! rebased by the subroutine base. GOTCHAS S3.10 states the contract: a `use`
//! rule's `Indirect` result is base-relative, an `indirect` rule's stays
//! absolute, and three cases are excluded -- a non-`Indirect` spec, a zero
//! base, and `&(N.X)` (`result_relative`), whose anchor addition already
//! carries the use-site.
//!
//! These tests live here rather than in `engine/tests/` because
//! `apply_use_result_base` is module-private: a child module of `subroutine`
//! is the only place that can reach it. Widening it to `pub(crate)` to move
//! the tests would trade a permanent visibility change for a test location
//! (see `docs/solutions/developer-experience/rust-test-visibility-boundary.md`).
//!
//! Every expected value below is derived from the arithmetic GOTCHAS S3.10
//! states, never recorded from a run of the current code. A table written from
//! observed output would pass by construction and agree with a latent bug as
//! readily as with correct behavior.

use super::apply_use_result_base;
use crate::LibmagicError;
use crate::error::EvaluationError;
use crate::evaluator::{EvaluationConfig, EvaluationContext};
use crate::parser::ast::{Endianness, IndirectAdjustmentOp, OffsetSpec, TypeKind};

/// The measured JPEG case from GOTCHAS S3.10: `jpeg_segment` invoked at
/// use-site 20 reads a segment length of 128 at byte 22, and the next segment
/// sits at `20 + (128 + 2) = 150`.
const JPEG_USE_SITE: usize = 20;
const JPEG_RESOLVED: usize = 130;
const JPEG_REBASED: usize = 150;

/// Build a plain `use`-style indirect spec: `(2.S+2)`, no anchor relativity.
fn indirect_spec() -> OffsetSpec {
    OffsetSpec::Indirect {
        base_offset: 2,
        base_relative: false,
        pointer_type: TypeKind::Short {
            endian: Endianness::Big,
            signed: false,
        },
        adjustment: 2,
        adjustment_op: IndirectAdjustmentOp::Add,
        result_relative: false,
        endian: Endianness::Big,
    }
}

/// The `&(N.X)` form: the anchor addition already carries the use-site, so
/// adding the base again would double-count.
fn result_relative_spec() -> OffsetSpec {
    match indirect_spec() {
        OffsetSpec::Indirect {
            base_offset,
            base_relative,
            pointer_type,
            adjustment,
            adjustment_op,
            endian,
            ..
        } => OffsetSpec::Indirect {
            base_offset,
            base_relative,
            pointer_type,
            adjustment,
            adjustment_op,
            result_relative: true,
            endian,
        },
        other => other,
    }
}

fn context_with_base(base: usize) -> EvaluationContext {
    let mut context = EvaluationContext::new(EvaluationConfig::default());
    context.set_base_offset(base);
    context
}

/// Walks every branch of the rebasing rule in one table.
///
/// Each case names the branch it pins and the reason the expected value is
/// what it is, so a future failure reports a contract rather than two numbers.
#[test]
fn apply_use_result_base_branch_matrix() {
    let buffer = vec![0u8; 256];

    let cases: &[(&str, OffsetSpec, usize, usize, usize)] = &[
        (
            "use + Indirect + non-zero base rebases: GOTCHAS S3.10 jpeg walk, \
             use-site 20 + resolved 130 = 150",
            indirect_spec(),
            JPEG_USE_SITE,
            JPEG_RESOLVED,
            JPEG_REBASED,
        ),
        (
            "non-Indirect spec is already in the caller's frame and must not be rebased",
            // 130 == JPEG_RESOLVED; a literal avoids a lossy usize -> i64 cast.
            OffsetSpec::Absolute(130),
            JPEG_USE_SITE,
            JPEG_RESOLVED,
            JPEG_RESOLVED,
        ),
        (
            "zero base is top level (tplink's use-site always resolves to 0): no rebase",
            indirect_spec(),
            0,
            JPEG_RESOLVED,
            JPEG_RESOLVED,
        ),
        (
            "result_relative `&(N.X)` is excluded: the anchor addition already \
             carries the use-site, so rebasing would double-count",
            result_relative_spec(),
            JPEG_USE_SITE,
            JPEG_RESOLVED,
            JPEG_RESOLVED,
        ),
        (
            "Relative spec is not Indirect: no rebase",
            OffsetSpec::Relative(4),
            JPEG_USE_SITE,
            JPEG_RESOLVED,
            JPEG_RESOLVED,
        ),
        (
            "FromEnd spec is not Indirect: no rebase",
            OffsetSpec::FromEnd(-4),
            JPEG_USE_SITE,
            JPEG_RESOLVED,
            JPEG_RESOLVED,
        ),
    ];

    for (contract, spec, base, resolved, expected) in cases {
        let context = context_with_base(*base);
        let actual = apply_use_result_base(spec, *resolved, &context, &buffer)
            .unwrap_or_else(|e| panic!("{contract}: unexpected error {e:?}"));
        assert_eq!(
            actual, *expected,
            "{contract} (base={base}, resolved={resolved})"
        );
    }
}

/// A rebase that cannot fit in `usize` must surface as an evaluation error
/// rather than wrapping or panicking. Adversarial magic can drive the pointer
/// value arbitrarily high.
#[test]
fn apply_use_result_base_overflow_is_invalid_offset_not_panic() {
    let buffer = vec![0u8; 256];
    let context = context_with_base(1);

    let err = apply_use_result_base(&indirect_spec(), usize::MAX, &context, &buffer)
        .expect_err("usize::MAX + base(1) must not silently wrap");

    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
        ),
        "overflow must report InvalidOffset, got {err:?}"
    );
}

/// A rebased offset past the end of the buffer must be reported, not read.
/// The buffer is a direct parameter, so this branch is reachable from here.
#[test]
fn apply_use_result_base_past_end_of_buffer_is_buffer_overrun() {
    // 150 is the rebased target from the jpeg case; a 32-byte buffer cannot
    // hold it.
    let buffer = vec![0u8; 32];
    let context = context_with_base(JPEG_USE_SITE);

    let err = apply_use_result_base(&indirect_spec(), JPEG_RESOLVED, &context, &buffer)
        .expect_err("rebased offset 150 is past the end of a 32-byte buffer");

    assert!(
        matches!(
            err,
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun {
                offset: JPEG_REBASED
            })
        ),
        "must report BufferOverrun at the rebased offset {JPEG_REBASED}, got {err:?}"
    );
}

/// The EOF position itself is a valid resolution target (GOTCHAS S15.1);
/// only strictly past it is an overrun. This pins the boundary so a future
/// `>=` tightening is caught here rather than by a dropped child rule.
#[test]
fn apply_use_result_base_rebase_landing_exactly_at_eof_is_permitted() {
    let buffer = vec![0u8; JPEG_REBASED];
    let context = context_with_base(JPEG_USE_SITE);

    let actual = apply_use_result_base(&indirect_spec(), JPEG_RESOLVED, &context, &buffer)
        .expect("a rebase landing exactly at buffer.len() is a valid position");

    assert_eq!(
        actual, JPEG_REBASED,
        "offset == buffer.len() is the EOF position, not an overrun"
    );
}
