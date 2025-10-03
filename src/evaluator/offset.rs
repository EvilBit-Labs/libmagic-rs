//! Offset resolution for magic rule evaluation
//!
//! This module provides functions for resolving different types of offset specifications
//! into absolute byte positions within file buffers, with proper bounds checking.

use crate::LibmagicError;
use crate::parser::ast::OffsetSpec;

/// Error types specific to offset resolution
#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    /// Buffer overrun - offset is beyond buffer bounds
    #[error("Buffer overrun: offset {offset} is beyond buffer length {buffer_len}")]
    BufferOverrun {
        /// The requested offset
        offset: usize,
        /// The actual buffer length
        buffer_len: usize,
    },

    /// Invalid offset specification
    #[error("Invalid offset: {reason}")]
    InvalidOffset {
        /// Reason why the offset is invalid
        reason: String,
    },

    /// Arithmetic overflow in offset calculation
    #[error("Arithmetic overflow in offset calculation")]
    ArithmeticOverflow,
}

/// Resolve an absolute offset with bounds checking
///
/// This function takes an absolute offset (which can be negative for offsets from the end)
/// and resolves it to a valid position within the buffer bounds.
///
/// # Arguments
///
/// * `offset` - The absolute offset (positive from start, negative from end)
/// * `buffer` - The file buffer to check bounds against
///
/// # Returns
///
/// Returns the resolved absolute offset as a `usize`, or an `OffsetError` if the offset
/// is out of bounds or invalid.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::evaluator::offset::resolve_absolute_offset;
///
/// let buffer = b"Hello, World!";
///
/// // Positive offset from start
/// let offset = resolve_absolute_offset(0, buffer).unwrap();
/// assert_eq!(offset, 0);
///
/// let offset = resolve_absolute_offset(7, buffer).unwrap();
/// assert_eq!(offset, 7);
///
/// // Negative offset from end
/// let offset = resolve_absolute_offset(-1, buffer).unwrap();
/// assert_eq!(offset, 12); // Last character
///
/// let offset = resolve_absolute_offset(-6, buffer).unwrap();
/// assert_eq!(offset, 7); // "World!"
/// ```
///
/// # Errors
///
/// * `OffsetError::BufferOverrun` - If the resolved offset is beyond buffer bounds
/// * `OffsetError::ArithmeticOverflow` - If offset calculation overflows
pub fn resolve_absolute_offset(offset: i64, buffer: &[u8]) -> Result<usize, OffsetError> {
    let buffer_len = buffer.len();

    if offset >= 0 {
        // Positive offset from start
        let abs_offset = usize::try_from(offset).map_err(|_| OffsetError::ArithmeticOverflow)?;
        if abs_offset >= buffer_len {
            return Err(OffsetError::BufferOverrun {
                offset: abs_offset,
                buffer_len,
            });
        }
        Ok(abs_offset)
    } else {
        // Negative offset from end
        // Handle i64::MIN case which can't be negated safely
        if offset == i64::MIN {
            return Err(OffsetError::ArithmeticOverflow);
        }

        let offset_from_end =
            usize::try_from(-offset).map_err(|_| OffsetError::ArithmeticOverflow)?;

        if offset_from_end > buffer_len {
            return Err(OffsetError::BufferOverrun {
                offset: buffer_len.saturating_sub(offset_from_end),
                buffer_len,
            });
        }

        // Calculate position from end
        let resolved_offset = buffer_len - offset_from_end;
        Ok(resolved_offset)
    }
}

/// Resolve any offset specification to an absolute position
///
/// This is a higher-level function that handles all types of offset specifications.
/// Currently only supports absolute offsets, but will be extended to handle indirect,
/// relative, and from-end offsets in future tasks.
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
        OffsetSpec::Absolute(offset) => resolve_absolute_offset(*offset, buffer)
            .map_err(|e| LibmagicError::EvaluationError(e.to_string())),
        OffsetSpec::Indirect { .. } => {
            // TODO: Implement indirect offset resolution in task 15.2
            Err(LibmagicError::EvaluationError(
                "Indirect offsets not yet implemented".to_string(),
            ))
        }
        OffsetSpec::Relative(_) => {
            // TODO: Implement relative offset resolution in future task
            Err(LibmagicError::EvaluationError(
                "Relative offsets not yet implemented".to_string(),
            ))
        }
        OffsetSpec::FromEnd(offset) => {
            // FromEnd is handled the same as negative Absolute offsets
            resolve_absolute_offset(*offset, buffer)
                .map_err(|e| LibmagicError::EvaluationError(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_absolute_offset_positive() {
        let buffer = b"Hello, World!";

        // Test valid positive offsets
        assert_eq!(resolve_absolute_offset(0, buffer).unwrap(), 0);
        assert_eq!(resolve_absolute_offset(1, buffer).unwrap(), 1);
        assert_eq!(resolve_absolute_offset(7, buffer).unwrap(), 7);
        assert_eq!(resolve_absolute_offset(12, buffer).unwrap(), 12); // Last valid index
    }

    #[test]
    fn test_resolve_absolute_offset_negative() {
        let buffer = b"Hello, World!";

        // Test valid negative offsets (from end)
        assert_eq!(resolve_absolute_offset(-1, buffer).unwrap(), 12); // Last character
        assert_eq!(resolve_absolute_offset(-6, buffer).unwrap(), 7); // "World!"
        assert_eq!(resolve_absolute_offset(-13, buffer).unwrap(), 0); // First character
    }

    #[test]
    fn test_resolve_absolute_offset_out_of_bounds_positive() {
        let buffer = b"Hello";

        // Test positive offset beyond buffer
        let result = resolve_absolute_offset(5, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            OffsetError::BufferOverrun { offset, buffer_len } => {
                assert_eq!(offset, 5);
                assert_eq!(buffer_len, 5);
            }
            _ => panic!("Expected BufferOverrun error"),
        }

        // Test way beyond buffer
        let result = resolve_absolute_offset(100, buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_absolute_offset_out_of_bounds_negative() {
        let buffer = b"Hi";

        // Test negative offset beyond buffer start
        let result = resolve_absolute_offset(-3, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            OffsetError::BufferOverrun { .. } => {
                // Expected error type
            }
            _ => panic!("Expected BufferOverrun error"),
        }

        // Test way beyond buffer start
        let result = resolve_absolute_offset(-100, buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_absolute_offset_empty_buffer() {
        let buffer = b"";

        // Any offset in empty buffer should fail
        assert!(resolve_absolute_offset(0, buffer).is_err());
        assert!(resolve_absolute_offset(1, buffer).is_err());
        assert!(resolve_absolute_offset(-1, buffer).is_err());
    }

    #[test]
    fn test_resolve_absolute_offset_edge_cases() {
        let buffer = b"X"; // Single byte buffer

        // Valid cases
        assert_eq!(resolve_absolute_offset(0, buffer).unwrap(), 0);
        assert_eq!(resolve_absolute_offset(-1, buffer).unwrap(), 0);

        // Invalid cases
        assert!(resolve_absolute_offset(1, buffer).is_err());
        assert!(resolve_absolute_offset(-2, buffer).is_err());
    }

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
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Buffer overrun"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_resolve_offset_indirect_not_implemented() {
        let buffer = b"Test data";
        let spec = OffsetSpec::Indirect {
            base_offset: 0,
            pointer_type: crate::parser::ast::TypeKind::Byte,
            adjustment: 0,
            endian: crate::parser::ast::Endianness::Little,
        };

        let result = resolve_offset(&spec, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Indirect offsets not yet implemented"));
            }
            _ => panic!("Expected EvaluationError for unimplemented feature"),
        }
    }

    #[test]
    fn test_resolve_offset_relative_not_implemented() {
        let buffer = b"Test data";
        let spec = OffsetSpec::Relative(4);

        let result = resolve_offset(&spec, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            LibmagicError::EvaluationError(msg) => {
                assert!(msg.contains("Relative offsets not yet implemented"));
            }
            _ => panic!("Expected EvaluationError for unimplemented feature"),
        }
    }

    #[test]
    fn test_offset_error_display() {
        let error = OffsetError::BufferOverrun {
            offset: 10,
            buffer_len: 5,
        };
        let error_str = error.to_string();
        assert!(error_str.contains("Buffer overrun"));
        assert!(error_str.contains("10"));
        assert!(error_str.contains('5'));

        let error = OffsetError::InvalidOffset {
            reason: "test reason".to_string(),
        };
        let error_str = error.to_string();
        assert!(error_str.contains("Invalid offset"));
        assert!(error_str.contains("test reason"));

        let error = OffsetError::ArithmeticOverflow;
        let error_str = error.to_string();
        assert!(error_str.contains("Arithmetic overflow"));
    }

    #[test]
    fn test_large_buffer_offsets() {
        // Test with a larger buffer to ensure no integer overflow issues
        let large_buffer = vec![0u8; 1024];

        // Test positive offsets
        assert_eq!(resolve_absolute_offset(0, &large_buffer).unwrap(), 0);
        assert_eq!(resolve_absolute_offset(512, &large_buffer).unwrap(), 512);
        assert_eq!(resolve_absolute_offset(1023, &large_buffer).unwrap(), 1023);

        // Test negative offsets
        assert_eq!(resolve_absolute_offset(-1, &large_buffer).unwrap(), 1023);
        assert_eq!(resolve_absolute_offset(-512, &large_buffer).unwrap(), 512);
        assert_eq!(resolve_absolute_offset(-1024, &large_buffer).unwrap(), 0);

        // Test out of bounds
        assert!(resolve_absolute_offset(1024, &large_buffer).is_err());
        assert!(resolve_absolute_offset(-1025, &large_buffer).is_err());
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

    /// Test for potential integer overflow vulnerabilities in offset calculations
    #[test]
    fn test_offset_security_edge_cases() {
        let buffer = b"test";

        // Test potential overflow scenarios
        let overflow_cases = vec![i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1];

        for offset in overflow_cases {
            let result = resolve_absolute_offset(offset, buffer);
            // Should either succeed with valid offset or fail gracefully
            if let Ok(resolved) = result {
                // If it succeeds, the resolved offset must be within buffer bounds
                assert!(
                    resolved < buffer.len(),
                    "Resolved offset {resolved} exceeds buffer length {}",
                    buffer.len()
                );
            } else {
                // Failure is acceptable for extreme values
            }
        }
    }
}
#[test]
fn test_resolve_absolute_offset_arithmetic_overflow() {
    let buffer = b"test";

    // Test with i64::MIN which should cause overflow when negated
    let result = resolve_absolute_offset(i64::MIN, buffer);
    assert!(result.is_err());

    match result.unwrap_err() {
        OffsetError::ArithmeticOverflow => {
            // Expected error type
        }
        _ => panic!("Expected ArithmeticOverflow error"),
    }
}
