// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Offset resolution for magic rule evaluation
//!
//! This module provides functions for resolving different types of offset specifications
//! into absolute byte positions within file buffers, with proper bounds checking.

mod absolute;
mod indirect;
mod relative;

pub use absolute::{OffsetError, resolve_absolute_offset};

use crate::LibmagicError;
use crate::parser::ast::OffsetSpec;

/// Map an `OffsetError` to a `LibmagicError` for a given original offset value
pub(crate) fn map_offset_error(e: &OffsetError, original_offset: i64) -> LibmagicError {
    match e {
        OffsetError::BufferOverrun {
            offset,
            buffer_len: _,
        } => LibmagicError::EvaluationError(crate::error::EvaluationError::BufferOverrun {
            offset: *offset,
        }),
        OffsetError::InvalidOffset { reason: _ } | OffsetError::ArithmeticOverflow => {
            LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
                offset: original_offset,
            })
        }
    }
}

/// Resolve any offset specification to an absolute position.
///
/// Convenience wrapper for callers that do not have a relative-offset anchor
/// (e.g., tests, top-level evaluation with no prior match). Internally
/// delegates with `last_match_end = 0`. For `OffsetSpec::Relative`, that
/// means non-negative deltas behave like absolute offsets from the start of
/// the buffer (`Relative(N)` for `N >= 0` resolves to absolute `N`), but
/// negative deltas underflow the anchor and return
/// `EvaluationError::InvalidOffset` -- they are *not* interpreted like
/// `OffsetSpec::Absolute(-N)` from the end of the buffer. Callers that need
/// relative offsets to anchor against actual prior matches should use
/// `evaluate_rules` and let the engine thread the anchor.
///
/// **Behavior change:** before the relative-offset feature landed in v0.5,
/// this function returned `EvaluationError::UnsupportedType` for
/// `OffsetSpec::Relative`. It now resolves against anchor 0, which can
/// succeed (non-negative delta) or fail with `InvalidOffset` (negative
/// delta) depending on the value. Callers with existing error-handling code
/// that pattern-matched `UnsupportedType` for relative offsets must remove
/// that arm.
///
/// # Arguments
///
/// * `spec` - The offset specification to resolve
/// * `buffer` - The file buffer to resolve against
///
/// # Returns
///
/// Returns the resolved absolute offset as a `usize`, or a `LibmagicError` if resolution fails.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::offset::resolve_offset;
/// use libmagic_rs::parser::ast::OffsetSpec;
///
/// let buffer = b"Test data";
/// let spec = OffsetSpec::Absolute(4);
///
/// let offset = resolve_offset(&spec, buffer).unwrap();
/// assert_eq!(offset, 4);
/// ```
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If offset resolution fails
pub fn resolve_offset(spec: &OffsetSpec, buffer: &[u8]) -> Result<usize, LibmagicError> {
    resolve_offset_with_context(spec, buffer, 0)
}

/// Resolve any offset specification, including relative offsets, against a
/// previous-match anchor.
///
/// This is the full dispatcher used by the evaluation engine. It handles all
/// `OffsetSpec` variants:
///
/// - [`OffsetSpec::Absolute`] / [`OffsetSpec::FromEnd`]: resolved against the
///   buffer (sign-aware), `last_match_end` ignored.
/// - [`OffsetSpec::Indirect`]: resolved by reading a pointer value from the
///   buffer, `last_match_end` ignored.
/// - [`OffsetSpec::Relative`]: resolved as `last_match_end + delta`,
///   bounds-checked. The anchor `0` makes top-level relative offsets resolve
///   from the file start.
///
/// `pub(crate)` because the anchor-threading contract is internal to the
/// evaluation engine -- external callers use [`resolve_offset`] (which
/// hardcodes anchor 0) or go through `evaluate_rules`.
///
/// # Arguments
///
/// * `spec` - The offset specification to resolve
/// * `buffer` - The file buffer to resolve against
/// * `last_match_end` - End offset of the most recent successful match.
///   Supplied by the engine via `EvaluationContext::last_match_end()`. Pass
///   `0` if no prior match exists.
///
/// # Errors
///
/// * `LibmagicError::EvaluationError` - If offset resolution fails for any
///   variant. Relative-offset failures surface as `BufferOverrun` (target
///   past end of buffer) or `InvalidOffset` (arithmetic over/underflow).
pub(crate) fn resolve_offset_with_context(
    spec: &OffsetSpec,
    buffer: &[u8],
    last_match_end: usize,
) -> Result<usize, LibmagicError> {
    resolve_offset_with_base(spec, buffer, last_match_end, 0)
}

/// Like [`resolve_offset_with_context`] but applies a subroutine
/// `base_offset` to positive absolute offsets.
///
/// Inside a `MetaType::Use` subroutine body, `OffsetSpec::Absolute(n)`
/// with `n >= 0` resolves to `base_offset + n`, matching magic(5)
/// semantics where the subroutine's offsets are relative to the
/// caller's invocation point. Negative `Absolute`, `FromEnd`,
/// `Relative`, and `Indirect` are unaffected -- they already have
/// well-defined frames of reference (buffer end, previous match, or
/// a pointer read from the buffer).
pub(crate) fn resolve_offset_with_base(
    spec: &OffsetSpec,
    buffer: &[u8],
    last_match_end: usize,
    base_offset: usize,
) -> Result<usize, LibmagicError> {
    match spec {
        OffsetSpec::Absolute(offset) => {
            // Apply base_offset only to positive absolute offsets.
            // Negative values mean "from end" and should not be shifted
            // by the subroutine base.
            let effective = if *offset >= 0 {
                // Use checked conversions so overflow is reported as
                // InvalidOffset rather than silently producing a huge
                // biased value that later surfaces as BufferOverrun.
                let abs = usize::try_from(*offset).map_err(|_| {
                    LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
                        offset: *offset,
                    })
                })?;
                let biased = base_offset
                    .checked_add(abs)
                    .ok_or(LibmagicError::EvaluationError(
                        crate::error::EvaluationError::InvalidOffset { offset: *offset },
                    ))?;
                i64::try_from(biased).map_err(|_| {
                    LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
                        offset: *offset,
                    })
                })?
            } else {
                *offset
            };
            resolve_absolute_offset(effective, buffer).map_err(|e| map_offset_error(&e, effective))
        }
        OffsetSpec::Indirect { .. } => {
            indirect::resolve_indirect_offset_with_anchor(spec, buffer, Some(last_match_end))
        }
        OffsetSpec::Relative(_) => relative::resolve_relative_offset(spec, buffer, last_match_end),
        OffsetSpec::FromEnd(offset) => {
            // FromEnd is handled the same as negative Absolute offsets.
            // Base offset does not apply -- "from end" is always
            // relative to the buffer itself.
            resolve_absolute_offset(*offset, buffer).map_err(|e| map_offset_error(&e, *offset))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_offset_absolute() {
        let buffer = b"Test data for offset resolution";
        let spec = OffsetSpec::Absolute(5);

        let result = resolve_offset(&spec, buffer).unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_resolve_offset_absolute_negative() {
        let buffer = b"Test data";
        let spec = OffsetSpec::Absolute(-4);

        let result = resolve_offset(&spec, buffer).unwrap();
        assert_eq!(result, 5); // 9 - 4 = 5
    }

    #[test]
    fn test_resolve_offset_from_end() {
        let buffer = b"Test data";
        let spec = OffsetSpec::FromEnd(-3);

        let result = resolve_offset(&spec, buffer).unwrap();
        assert_eq!(result, 6); // 9 - 3 = 6
    }

    #[test]
    fn test_resolve_offset_absolute_out_of_bounds() {
        let buffer = b"Short";
        let spec = OffsetSpec::Absolute(10);

        let result = resolve_offset(&spec, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(crate::error::EvaluationError::BufferOverrun {
                ..
            }) => {
                // Expected error type
            }
            _ => panic!("Expected EvaluationError with BufferOverrun"),
        }
    }

    #[test]
    fn test_resolve_offset_indirect_success() {
        // Byte pointer at offset 0 with value 5 → resolves to offset 5
        let buffer = b"\x05TestXdata";
        let spec = OffsetSpec::Indirect {
            base_offset: 0,
            base_relative: false,
            pointer_type: crate::parser::ast::TypeKind::Byte { signed: false },
            adjustment: 0,
            adjustment_op: crate::parser::ast::IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: crate::parser::ast::Endianness::Little,
        };

        let result = resolve_offset(&spec, buffer).unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_resolve_offset_relative_via_context() {
        // Anchor 4 + delta 3 = absolute 7, in-bounds.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(3);
        let resolved = resolve_offset_with_context(&spec, buffer, 4).unwrap();
        assert_eq!(resolved, 7);
    }

    #[test]
    fn test_resolve_offset_relative_top_level_default() {
        // Calling resolve_offset (no context) should default the anchor to 0.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(5);
        assert_eq!(resolve_offset(&spec, buffer).unwrap(), 5);
    }

    #[test]
    fn test_resolve_offset_with_context_passthrough_absolute() {
        // The context-aware dispatcher must not affect non-relative variants.
        let buffer = b"Test data";
        let spec = OffsetSpec::Absolute(4);
        // last_match_end is irrelevant for Absolute.
        assert_eq!(resolve_offset_with_context(&spec, buffer, 100).unwrap(), 4);
    }

    #[test]
    fn test_resolve_offset_with_context_passthrough_from_end() {
        let buffer = b"Test data";
        let spec = OffsetSpec::FromEnd(-3);
        assert_eq!(resolve_offset_with_context(&spec, buffer, 999).unwrap(), 6);
    }

    #[test]
    fn test_resolve_offset_with_context_passthrough_indirect() {
        // Same indirect setup as test_resolve_offset_indirect_success above.
        let buffer = b"\x05TestXdata";
        let spec = OffsetSpec::Indirect {
            base_offset: 0,
            base_relative: false,
            pointer_type: crate::parser::ast::TypeKind::Byte { signed: false },
            adjustment: 0,
            adjustment_op: crate::parser::ast::IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: crate::parser::ast::Endianness::Little,
        };
        assert_eq!(resolve_offset_with_context(&spec, buffer, 42).unwrap(), 5);
    }

    #[test]
    fn test_resolve_offset_with_base_biases_positive_absolute() {
        // Positive Absolute inside a subroutine body is biased by
        // `base_offset`. This is the load-bearing invariant of
        // `MetaType::Use` subroutine semantics.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::Absolute(4);
        // base_offset = 10 -> resolves to 14 (not 4).
        assert_eq!(
            resolve_offset_with_base(&spec, buffer, 0, 10).unwrap(),
            14,
            "positive Absolute must be biased by base_offset inside a subroutine"
        );
    }

    #[test]
    fn test_resolve_offset_with_base_does_not_bias_negative_absolute() {
        // Negative Absolute means "from-end" semantics (magic(5)
        // allows either explicit `FromEnd` or negative `Absolute`).
        // The subroutine base_offset is relative to the file start
        // and has no meaning for from-end positions.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::Absolute(-4);
        // Without bias: resolves to len - 4 = 12.
        // Buggy with-bias would give: 10 + (len - 4) or similar.
        assert_eq!(
            resolve_offset_with_base(&spec, buffer, 0, 10).unwrap(),
            12,
            "negative Absolute must NOT be biased"
        );
    }

    #[test]
    fn test_resolve_offset_with_base_does_not_bias_from_end() {
        // `FromEnd` is always relative to the buffer, not the
        // subroutine's use-site.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::FromEnd(-4);
        assert_eq!(
            resolve_offset_with_base(&spec, buffer, 0, 10).unwrap(),
            12,
            "FromEnd must NOT be biased"
        );
    }

    #[test]
    fn test_resolve_offset_with_base_does_not_bias_relative() {
        // `Relative(N)` resolves against the previous-match anchor,
        // not the subroutine base. Inside a subroutine body,
        // `last_match_end` is seeded to the use-site by
        // `SubroutineScope::enter`, so this already has the correct
        // frame of reference without additional bias.
        let buffer = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(3);
        // last_match_end = 2, base_offset = 10.
        // Expected: 2 + 3 = 5 (bias does NOT apply).
        assert_eq!(
            resolve_offset_with_base(&spec, buffer, 2, 10).unwrap(),
            5,
            "Relative must NOT be biased (already resolved against last_match_end)"
        );
    }

    #[test]
    fn test_resolve_offset_with_base_does_not_bias_indirect() {
        // `Indirect` reads a pointer from the buffer; the pointer's
        // value is an absolute file position, not a subroutine-
        // relative one.
        let buffer = b"\x05TestXdata";
        let spec = OffsetSpec::Indirect {
            base_offset: 0,
            base_relative: false,
            pointer_type: crate::parser::ast::TypeKind::Byte { signed: false },
            adjustment: 0,
            adjustment_op: crate::parser::ast::IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: crate::parser::ast::Endianness::Little,
        };
        assert_eq!(
            resolve_offset_with_base(&spec, buffer, 0, 10).unwrap(),
            5,
            "Indirect must NOT be biased"
        );
    }

    #[test]
    fn test_resolve_offset_comprehensive() {
        let buffer = b"0123456789ABCDEF";

        // Test various absolute offsets
        let test_cases = vec![
            (OffsetSpec::Absolute(0), 0),
            (OffsetSpec::Absolute(8), 8),
            (OffsetSpec::Absolute(15), 15),
            (OffsetSpec::Absolute(-1), 15),
            (OffsetSpec::Absolute(-8), 8),
            (OffsetSpec::Absolute(-16), 0),
            (OffsetSpec::FromEnd(-1), 15),
            (OffsetSpec::FromEnd(-8), 8),
            (OffsetSpec::FromEnd(-16), 0),
        ];

        for (spec, expected) in test_cases {
            let result = resolve_offset(&spec, buffer).unwrap();
            assert_eq!(result, expected, "Failed for spec: {spec:?}");
        }
    }

    /// Regression test for RU0: `base_offset + large_positive_absolute` that
    /// overflows `usize` must produce `InvalidOffset`, not `BufferOverrun`.
    ///
    /// Before the fix, saturating arithmetic turned overflow into `usize::MAX`
    /// (or `i64::MAX`), which then flowed into `resolve_absolute_offset` and
    /// surfaced as a `BufferOverrun` at that giant offset -- losing the more
    /// precise overflow signal.
    #[test]
    fn test_resolve_offset_with_base_overflow_yields_invalid_offset() {
        let buffer = b"0123456789ABCDEF"; // 16 bytes
        // base_offset near usize::MAX combined with any positive Absolute
        // must overflow. Use usize::MAX - 1 so that adding even 2 overflows.
        let base = usize::MAX - 1;
        let spec = OffsetSpec::Absolute(2); // base + 2 overflows usize

        let result = resolve_offset_with_base(&spec, buffer, 0, base);
        assert!(
            result.is_err(),
            "overflow of base_offset + absolute must fail"
        );
        match result.unwrap_err() {
            LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
                ..
            }) => {
                // Correct: overflow reported as InvalidOffset, not BufferOverrun.
            }
            LibmagicError::EvaluationError(crate::error::EvaluationError::BufferOverrun {
                ..
            }) => {
                panic!(
                    "overflow of base_offset + absolute must be InvalidOffset, not BufferOverrun"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
