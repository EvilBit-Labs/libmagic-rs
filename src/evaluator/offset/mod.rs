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
/// delegates with `last_match_end = 0`, which means an `OffsetSpec::Relative`
/// passed here resolves as if it were `OffsetSpec::Absolute` of the same
/// delta -- matching libmagic's "no prior match" semantics. Callers that
/// need relative offsets to anchor against actual prior matches should use
/// `evaluate_rules` and let the engine thread the anchor.
///
/// **Behavior change:** before the relative-offset feature landed in v0.5,
/// this function returned `EvaluationError::UnsupportedType` for
/// `OffsetSpec::Relative`. It now resolves successfully against anchor 0.
/// Callers with existing error-handling code that pattern-matched
/// `UnsupportedType` for relative offsets must remove that arm.
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
    match spec {
        OffsetSpec::Absolute(offset) => {
            resolve_absolute_offset(*offset, buffer).map_err(|e| map_offset_error(&e, *offset))
        }
        OffsetSpec::Indirect { .. } => indirect::resolve_indirect_offset(spec, buffer),
        OffsetSpec::Relative(_) => relative::resolve_relative_offset(spec, buffer, last_match_end),
        OffsetSpec::FromEnd(offset) => {
            // FromEnd is handled the same as negative Absolute offsets
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
            pointer_type: crate::parser::ast::TypeKind::Byte { signed: false },
            adjustment: 0,
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
            pointer_type: crate::parser::ast::TypeKind::Byte { signed: false },
            adjustment: 0,
            endian: crate::parser::ast::Endianness::Little,
        };
        assert_eq!(resolve_offset_with_context(&spec, buffer, 42).unwrap(), 5);
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
}
