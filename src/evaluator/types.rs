//! Type interpretation for reading and converting bytes from file buffers
//!
//! This module provides functions for safely reading different data types from byte buffers
//! with proper bounds checking and error handling.

use crate::parser::ast::Value;
use thiserror::Error;

/// Errors that can occur during type reading operations
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypeReadError {
    /// Buffer access beyond available data
    #[error(
        "Buffer overrun: attempted to read at offset {offset} but buffer length is {buffer_len}"
    )]
    BufferOverrun {
        /// The offset that was attempted to be accessed
        offset: usize,
        /// The actual length of the buffer
        buffer_len: usize,
    },
}

/// Safely reads a single byte from the buffer at the specified offset
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to read the byte from
///
/// # Returns
///
/// Returns `Ok(Value::Uint(byte_value))` if the read is successful, or
/// `Err(TypeReadError::BufferOverrun)` if the offset is beyond the buffer bounds.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_byte;
/// use libmagic_rs::parser::ast::Value;
///
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
///
/// // Read first byte (0x7f)
/// let result = read_byte(buffer, 0).unwrap();
/// assert_eq!(result, Value::Uint(0x7f));
///
/// // Read last byte (0x46)
/// let result = read_byte(buffer, 3).unwrap();
/// assert_eq!(result, Value::Uint(0x46));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if the offset is greater than or equal to
/// the buffer length.
pub fn read_byte(buffer: &[u8], offset: usize) -> Result<Value, TypeReadError> {
    buffer
        .get(offset)
        .map(|&byte| Value::Uint(u64::from(byte)))
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_byte_success() {
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];

        // Test reading each byte
        assert_eq!(read_byte(buffer, 0).unwrap(), Value::Uint(0x7f));
        assert_eq!(read_byte(buffer, 1).unwrap(), Value::Uint(0x45));
        assert_eq!(read_byte(buffer, 2).unwrap(), Value::Uint(0x4c));
        assert_eq!(read_byte(buffer, 3).unwrap(), Value::Uint(0x46));
    }

    #[test]
    fn test_read_byte_zero_value() {
        let buffer = &[0x00, 0xff];

        // Test reading zero byte
        assert_eq!(read_byte(buffer, 0).unwrap(), Value::Uint(0));
        // Test reading max byte value
        assert_eq!(read_byte(buffer, 1).unwrap(), Value::Uint(255));
    }

    #[test]
    fn test_read_byte_single_byte_buffer() {
        let buffer = &[0x42];

        // Should succeed for offset 0
        assert_eq!(read_byte(buffer, 0).unwrap(), Value::Uint(0x42));

        // Should fail for offset 1
        let result = read_byte(buffer, 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 1,
                buffer_len: 1
            }
        );
    }

    #[test]
    fn test_read_byte_empty_buffer() {
        let buffer = &[];

        // Should fail for any offset
        let result = read_byte(buffer, 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0
            }
        );
    }

    #[test]
    fn test_read_byte_out_of_bounds() {
        let buffer = &[0x01, 0x02, 0x03];

        // Test various out-of-bounds offsets
        let test_cases = [3, 4, 10, 100, usize::MAX];

        for offset in test_cases {
            let result = read_byte(buffer, offset);
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: 3
                }
            );
        }
    }

    #[test]
    fn test_read_byte_large_buffer() {
        // Test with a larger buffer to ensure no performance issues
        let buffer: Vec<u8> = (0..=255).collect();

        // Test reading from various positions
        assert_eq!(read_byte(&buffer, 0).unwrap(), Value::Uint(0));
        assert_eq!(read_byte(&buffer, 127).unwrap(), Value::Uint(127));
        assert_eq!(read_byte(&buffer, 255).unwrap(), Value::Uint(255));

        // Test out of bounds
        let result = read_byte(&buffer, 256);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 256,
                buffer_len: 256
            }
        );
    }

    #[test]
    fn test_read_byte_all_byte_values() {
        // Test that all possible byte values are correctly converted to u64
        let buffer: Vec<u8> = (0..=255).collect();

        for (i, &expected_byte) in buffer.iter().enumerate() {
            let result = read_byte(&buffer, i).unwrap();
            assert_eq!(result, Value::Uint(u64::from(expected_byte)));
        }
    }

    #[test]
    fn test_type_read_error_display() {
        let error = TypeReadError::BufferOverrun {
            offset: 10,
            buffer_len: 5,
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("Buffer overrun"));
        assert!(error_string.contains("offset 10"));
        assert!(error_string.contains("buffer length is 5"));
    }

    #[test]
    fn test_type_read_error_debug() {
        let error = TypeReadError::BufferOverrun {
            offset: 42,
            buffer_len: 20,
        };

        let debug_string = format!("{error:?}");
        assert!(debug_string.contains("BufferOverrun"));
        assert!(debug_string.contains("offset: 42"));
        assert!(debug_string.contains("buffer_len: 20"));
    }

    #[test]
    fn test_type_read_error_equality() {
        let error1 = TypeReadError::BufferOverrun {
            offset: 5,
            buffer_len: 3,
        };
        let error2 = TypeReadError::BufferOverrun {
            offset: 5,
            buffer_len: 3,
        };
        let error3 = TypeReadError::BufferOverrun {
            offset: 6,
            buffer_len: 3,
        };

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_read_byte_boundary_conditions() {
        let buffer = &[0xaa, 0xbb, 0xcc];

        // Test reading at exact boundary (last valid index)
        assert_eq!(read_byte(buffer, 2).unwrap(), Value::Uint(0xcc));

        // Test reading just past boundary
        let result = read_byte(buffer, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_byte_return_type() {
        let buffer = &[0x80]; // Test with high bit set

        let result = read_byte(buffer, 0).unwrap();

        // Verify it returns Value::Uint, not Value::Int
        match result {
            Value::Uint(val) => assert_eq!(val, 0x80),
            _ => panic!("Expected Value::Uint variant"),
        }
    }
}
