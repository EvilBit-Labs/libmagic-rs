// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Absolute offset resolution

/// Error types specific to offset resolution
#[derive(Debug, thiserror::Error)]
pub enum OffsetError {
    /// Buffer overrun - offset is beyond buffer bounds
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::offset::OffsetError;
    ///
    /// let err = OffsetError::BufferOverrun { offset: 100, buffer_len: 32 };
    /// assert!(matches!(err, OffsetError::BufferOverrun { .. }));
    /// ```
    #[error("Buffer overrun: offset {offset} is beyond buffer length {buffer_len}")]
    BufferOverrun {
        /// The requested offset
        offset: usize,
        /// The actual buffer length
        buffer_len: usize,
    },

    /// Invalid offset specification
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::offset::OffsetError;
    ///
    /// let err = OffsetError::InvalidOffset { reason: "negative offset exceeds buffer".to_string() };
    /// assert!(matches!(err, OffsetError::InvalidOffset { .. }));
    /// ```
    #[error("Invalid offset: {reason}")]
    InvalidOffset {
        /// Reason why the offset is invalid
        reason: String,
    },

    /// Arithmetic overflow in offset calculation
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::offset::OffsetError;
    ///
    /// let err = OffsetError::ArithmeticOverflow;
    /// assert!(matches!(err, OffsetError::ArithmeticOverflow));
    /// ```
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
        // Positive offset from start.
        //
        // The bound is `>` (not `>=`): `offset == buffer_len` is the EOF
        // position and is a VALID resolution target, matching libmagic's
        // model where offset resolution is permissive and each type read
        // enforces its own width. Verified against real `file` (file-5.41):
        // for a rule whose child offset lands exactly at EOF, a numeric
        // child (`byte x`, `short x`, ...) is dropped -- its width-checked
        // read fails at EOF and produces a non-match -- while a `string x`
        // child renders an EMPTY string. LUKS's `>8 string x [%s,` on a
        // header truncated to 8 bytes prints `[,` in GNU `file`; without
        // permitting `offset == buffer_len` here, that child was silently
        // dropped at offset resolution. Width enforcement now lives entirely
        // in the readers: fixed-width readers use bounds-safe `.get()` /
        // `read_bytes_at` (BufferOverrun -> non-match at EOF), and
        // `read_string` returns an empty string at `offset == buffer_len`.
        // See GOTCHAS S15.1.
        let abs_offset = usize::try_from(offset).map_err(|_| OffsetError::ArithmeticOverflow)?;
        if abs_offset > buffer_len {
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
    fn test_resolve_absolute_offset_at_eof_is_permitted() {
        let buffer = b"Hello"; // len 5

        // `offset == buffer_len` is the EOF position and resolves
        // successfully -- libmagic permits it, deferring width enforcement
        // to the type read (a numeric read fails at EOF -> non-match; a
        // `string x` read yields an empty string). See GOTCHAS S15.1.
        assert_eq!(resolve_absolute_offset(5, buffer).unwrap(), 5);
    }

    #[test]
    fn test_resolve_absolute_offset_out_of_bounds_positive() {
        let buffer = b"Hello"; // len 5

        // Strictly beyond EOF (offset > buffer_len) is a genuine overrun.
        let result = resolve_absolute_offset(6, buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            OffsetError::BufferOverrun { offset, buffer_len } => {
                assert_eq!(offset, 6);
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

        // `offset == buffer_len` holds trivially for an empty buffer
        // (0 == 0), so offset 0 resolves to the EOF position 0 -- the type
        // read then decides: a numeric read finds no bytes (non-match) and
        // a `string x` read yields an empty string. No system-DB rule uses
        // a top-level `0 string x`, and GNU `file` still classifies an
        // empty file as "empty" (verified). See GOTCHAS S15.1.
        assert_eq!(resolve_absolute_offset(0, buffer).unwrap(), 0);
        // Strictly past EOF still fails.
        assert!(resolve_absolute_offset(1, buffer).is_err());
        assert!(resolve_absolute_offset(-1, buffer).is_err());
    }

    #[test]
    fn test_resolve_absolute_offset_edge_cases() {
        let buffer = b"X"; // Single byte buffer

        // Valid cases
        assert_eq!(resolve_absolute_offset(0, buffer).unwrap(), 0);
        assert_eq!(resolve_absolute_offset(-1, buffer).unwrap(), 0);
        // offset == buffer_len (1) is the EOF position, now permitted
        // (width enforced at read time). See GOTCHAS S15.1.
        assert_eq!(resolve_absolute_offset(1, buffer).unwrap(), 1);

        // Invalid cases: strictly past EOF.
        assert!(resolve_absolute_offset(2, buffer).is_err());
        assert!(resolve_absolute_offset(-2, buffer).is_err());
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

        // offset == buffer_len (1024) is the EOF position, now permitted.
        assert_eq!(resolve_absolute_offset(1024, &large_buffer).unwrap(), 1024);
        // Strictly past EOF still fails.
        assert!(resolve_absolute_offset(1025, &large_buffer).is_err());
        assert!(resolve_absolute_offset(-1025, &large_buffer).is_err());
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
                // If it succeeds, the resolved offset must be at most the
                // buffer length. `offset == buffer_len` (the EOF position)
                // is a valid resolution target now that width enforcement
                // lives in the type read (GOTCHAS S15.1); anything strictly
                // greater is a genuine overrun and would have errored.
                assert!(
                    resolved <= buffer.len(),
                    "Resolved offset {resolved} exceeds buffer length {}",
                    buffer.len()
                );
            } else {
                // Failure is acceptable for extreme values
            }
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
}
