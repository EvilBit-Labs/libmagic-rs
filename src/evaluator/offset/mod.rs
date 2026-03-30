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

/// Resolve any offset specification to an absolute position
///
/// This is a higher-level function that handles all types of offset specifications.
/// Supports absolute, from-end, and indirect offsets. Relative offsets are not yet
/// implemented.
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
    match spec {
        OffsetSpec::Absolute(offset) => {
            resolve_absolute_offset(*offset, buffer).map_err(|e| map_offset_error(&e, *offset))
        }
        OffsetSpec::Indirect { .. } => indirect::resolve_indirect_offset(spec, buffer),
        OffsetSpec::Relative(_) => relative::resolve_relative_offset(spec, buffer),
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
    fn test_resolve_offset_relative_not_implemented() {
        let buffer = b"Test data";
        let spec = OffsetSpec::Relative(4);

        let result = resolve_offset(&spec, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(crate::error::EvaluationError::UnsupportedType {
                type_name,
            }) => {
                assert!(type_name.contains("Relative offsets not yet implemented"));
            }
            _ => panic!("Expected EvaluationError with UnsupportedType"),
        }
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
