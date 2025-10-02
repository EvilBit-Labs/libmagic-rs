//! Type interpretation for reading and converting bytes from file buffers
//!
//! This module provides functions for safely reading different data types from byte buffers
//! with proper bounds checking and error handling.

use crate::parser::ast::{Endianness, TypeKind, Value};
use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian};
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
    /// Unsupported type variant
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// The name of the unsupported type
        type_name: String,
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

/// Safely reads a 16-bit integer from the buffer at the specified offset
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to read the 16-bit value from
/// * `endian` - The byte order to use for interpretation
/// * `signed` - Whether to interpret the value as signed or unsigned
///
/// # Returns
///
/// Returns `Ok(Value::Uint(value))` for unsigned values or `Ok(Value::Int(value))` for signed values
/// if the read is successful, or `Err(TypeReadError::BufferOverrun)` if there are insufficient bytes.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_short;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// let buffer = &[0x34, 0x12, 0xff, 0x7f]; // Little-endian data
///
/// // Read unsigned little-endian short (0x1234)
/// let result = read_short(buffer, 0, Endianness::Little, false).unwrap();
/// assert_eq!(result, Value::Uint(0x1234));
///
/// // Read signed little-endian short (0x7fff = 32767)
/// let result = read_short(buffer, 2, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::Int(32767));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if there are fewer than 2 bytes available
/// starting at the specified offset.
pub fn read_short(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    signed: bool,
) -> Result<Value, TypeReadError> {
    let bytes = buffer
        .get(offset..offset + 2)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;

    let value = match endian {
        Endianness::Little => LittleEndian::read_u16(bytes),
        Endianness::Big => BigEndian::read_u16(bytes),
        Endianness::Native => NativeEndian::read_u16(bytes),
    };

    if signed {
        #[allow(clippy::cast_possible_wrap)]
        Ok(Value::Int(i64::from(value as i16)))
    } else {
        Ok(Value::Uint(u64::from(value)))
    }
}

/// Safely reads a 32-bit integer from the buffer at the specified offset
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to read the 32-bit value from
/// * `endian` - The byte order to use for interpretation
/// * `signed` - Whether to interpret the value as signed or unsigned
///
/// # Returns
///
/// Returns `Ok(Value::Uint(value))` for unsigned values or `Ok(Value::Int(value))` for signed values
/// if the read is successful, or `Err(TypeReadError::BufferOverrun)` if there are insufficient bytes.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_long;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// let buffer = &[0x78, 0x56, 0x34, 0x12, 0xff, 0xff, 0xff, 0x7f];
///
/// // Read unsigned little-endian long (0x12345678)
/// let result = read_long(buffer, 0, Endianness::Little, false).unwrap();
/// assert_eq!(result, Value::Uint(0x12345678));
///
/// // Read signed little-endian long (0x7fffffff = 2147483647)
/// let result = read_long(buffer, 4, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::Int(2147483647));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if there are fewer than 4 bytes available
/// starting at the specified offset.
pub fn read_long(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    signed: bool,
) -> Result<Value, TypeReadError> {
    let bytes = buffer
        .get(offset..offset + 4)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;

    let value = match endian {
        Endianness::Little => LittleEndian::read_u32(bytes),
        Endianness::Big => BigEndian::read_u32(bytes),
        Endianness::Native => NativeEndian::read_u32(bytes),
    };

    if signed {
        #[allow(clippy::cast_possible_wrap)]
        Ok(Value::Int(i64::from(value as i32)))
    } else {
        Ok(Value::Uint(u64::from(value)))
    }
}

/// Reads and interprets bytes according to the specified `TypeKind`
///
/// This is the main interface for type interpretation that dispatches to the appropriate
/// reading function based on the `TypeKind` variant.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to read from
/// * `type_kind` - The type specification that determines how to interpret the bytes
///
/// # Returns
///
/// Returns the interpreted value as a `Value` enum variant, or an error if the read fails.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_typed_value;
/// use libmagic_rs::parser::ast::{TypeKind, Endianness, Value};
///
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x34, 0x12];
///
/// // Read a byte
/// let byte_result = read_typed_value(buffer, 0, &TypeKind::Byte).unwrap();
/// assert_eq!(byte_result, Value::Uint(0x7f));
///
/// // Read a little-endian short
/// let short_type = TypeKind::Short {
///     endian: Endianness::Little,
///     signed: false,
/// };
/// let short_result = read_typed_value(buffer, 4, &short_type).unwrap();
/// assert_eq!(short_result, Value::Uint(0x1234));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if there are insufficient bytes for the requested type,
/// or `TypeReadError::UnsupportedType` for type variants that are not yet implemented.
pub fn read_typed_value(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
) -> Result<Value, TypeReadError> {
    match type_kind {
        TypeKind::Byte => read_byte(buffer, offset),
        TypeKind::Short { endian, signed } => read_short(buffer, offset, *endian, *signed),
        TypeKind::Long { endian, signed } => read_long(buffer, offset, *endian, *signed),
        TypeKind::String { max_length: _ } => {
            // For now, return an error for unsupported string type
            // This will be implemented in a future task
            Err(TypeReadError::UnsupportedType {
                type_name: "String".to_string(),
            })
        }
    }
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

    // Tests for read_short function
    #[test]
    fn test_read_short_little_endian_unsigned() {
        let buffer = &[0x34, 0x12, 0x78, 0x56]; // 0x1234, 0x5678 in little-endian

        // Read first short (0x1234)
        let result = read_short(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234));

        // Read second short (0x5678)
        let result = read_short(buffer, 2, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678));
    }

    #[test]
    fn test_read_short_big_endian_unsigned() {
        let buffer = &[0x12, 0x34, 0x56, 0x78]; // 0x1234, 0x5678 in big-endian

        // Read first short (0x1234)
        let result = read_short(buffer, 0, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234));

        // Read second short (0x5678)
        let result = read_short(buffer, 2, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678));
    }

    #[test]
    fn test_read_short_native_endian_unsigned() {
        let buffer = &[0x34, 0x12, 0x78, 0x56];

        // Read using native endianness
        let result = read_short(buffer, 0, Endianness::Native, false).unwrap();

        // The exact value depends on the system's endianness, but it should be valid
        match result {
            Value::Uint(val) => {
                // Should be either 0x1234 (little-endian) or 0x3412 (big-endian)
                assert!(val == 0x1234 || val == 0x3412);
            }
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_short_signed_positive() {
        let buffer = &[0xff, 0x7f]; // 0x7fff = 32767 in little-endian

        let result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(32767));
    }

    #[test]
    fn test_read_short_signed_negative() {
        let buffer = &[0x00, 0x80]; // 0x8000 = -32768 in little-endian (signed)

        let result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(-32768));
    }

    #[test]
    fn test_read_short_signed_vs_unsigned() {
        let buffer = &[0xff, 0xff]; // 0xffff

        // Unsigned interpretation
        let unsigned_result = read_short(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(unsigned_result, Value::Uint(65535));

        // Signed interpretation
        let signed_result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(signed_result, Value::Int(-1));
    }

    #[test]
    fn test_read_short_buffer_overrun() {
        let buffer = &[0x12]; // Only 1 byte available

        // Should fail when trying to read 2 bytes
        let result = read_short(buffer, 0, Endianness::Little, false);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 1
            }
        );
    }

    #[test]
    fn test_read_short_offset_out_of_bounds() {
        let buffer = &[0x12, 0x34, 0x56];

        // Should fail when trying to read 2 bytes starting at offset 2 (only 1 byte left)
        let result = read_short(buffer, 2, Endianness::Little, false);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 2,
                buffer_len: 3
            }
        );
    }

    #[test]
    fn test_read_short_empty_buffer() {
        let buffer = &[];

        let result = read_short(buffer, 0, Endianness::Little, false);
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
    fn test_read_short_all_endianness_variants() {
        let buffer = &[0x12, 0x34];

        // Test all endianness variants
        let little = read_short(buffer, 0, Endianness::Little, false).unwrap();
        let big = read_short(buffer, 0, Endianness::Big, false).unwrap();
        let native = read_short(buffer, 0, Endianness::Native, false).unwrap();

        // Little-endian: 0x3412, Big-endian: 0x1234
        assert_eq!(little, Value::Uint(0x3412));
        assert_eq!(big, Value::Uint(0x1234));

        // Native should match one of them
        match native {
            Value::Uint(val) => assert!(val == 0x1234 || val == 0x3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    // Tests for read_long function
    #[test]
    fn test_read_long_little_endian_unsigned() {
        let buffer = &[0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56]; // 0x12345678, 0x56789abc

        // Read first long (0x12345678)
        let result = read_long(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678));

        // Read second long (0x56789abc)
        let result = read_long(buffer, 4, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678_9abc));
    }

    #[test]
    fn test_read_long_big_endian_unsigned() {
        let buffer = &[0x12, 0x34, 0x56, 0x78, 0x56, 0x78, 0x9a, 0xbc]; // 0x12345678, 0x56789abc

        // Read first long (0x12345678)
        let result = read_long(buffer, 0, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678));

        // Read second long (0x56789abc)
        let result = read_long(buffer, 4, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678_9abc));
    }

    #[test]
    fn test_read_long_native_endian_unsigned() {
        let buffer = &[0x78, 0x56, 0x34, 0x12];

        // Read using native endianness
        let result = read_long(buffer, 0, Endianness::Native, false).unwrap();

        // The exact value depends on the system's endianness, but it should be valid
        match result {
            Value::Uint(val) => {
                // Should be either 0x12345678 (little-endian) or 0x78563412 (big-endian)
                assert!(val == 0x1234_5678 || val == 0x7856_3412);
            }
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_long_signed_positive() {
        let buffer = &[0xff, 0xff, 0xff, 0x7f]; // 0x7fffffff = 2147483647 in little-endian

        let result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(2_147_483_647));
    }

    #[test]
    fn test_read_long_signed_negative() {
        let buffer = &[0x00, 0x00, 0x00, 0x80]; // 0x80000000 = -2147483648 in little-endian (signed)

        let result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(-2_147_483_648));
    }

    #[test]
    fn test_read_long_signed_vs_unsigned() {
        let buffer = &[0xff, 0xff, 0xff, 0xff]; // 0xffffffff

        // Unsigned interpretation
        let unsigned_result = read_long(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(unsigned_result, Value::Uint(4_294_967_295));

        // Signed interpretation
        let signed_result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(signed_result, Value::Int(-1));
    }

    #[test]
    fn test_read_long_buffer_overrun() {
        let buffer = &[0x12, 0x34, 0x56]; // Only 3 bytes available

        // Should fail when trying to read 4 bytes
        let result = read_long(buffer, 0, Endianness::Little, false);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 3
            }
        );
    }

    #[test]
    fn test_read_long_offset_out_of_bounds() {
        let buffer = &[0x12, 0x34, 0x56, 0x78, 0x9a];

        // Should fail when trying to read 4 bytes starting at offset 2 (only 3 bytes left)
        let result = read_long(buffer, 2, Endianness::Little, false);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 2,
                buffer_len: 5
            }
        );
    }

    #[test]
    fn test_read_long_empty_buffer() {
        let buffer = &[];

        let result = read_long(buffer, 0, Endianness::Little, false);
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
    fn test_read_long_all_endianness_variants() {
        let buffer = &[0x12, 0x34, 0x56, 0x78];

        // Test all endianness variants
        let little = read_long(buffer, 0, Endianness::Little, false).unwrap();
        let big = read_long(buffer, 0, Endianness::Big, false).unwrap();
        let native = read_long(buffer, 0, Endianness::Native, false).unwrap();

        // Little-endian: 0x78563412, Big-endian: 0x12345678
        assert_eq!(little, Value::Uint(0x7856_3412));
        assert_eq!(big, Value::Uint(0x1234_5678));

        // Native should match one of them
        match native {
            Value::Uint(val) => assert!(val == 0x1234_5678 || val == 0x7856_3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_long_extreme_values() {
        // Test maximum unsigned 32-bit value
        let max_buffer = &[0xff, 0xff, 0xff, 0xff];
        let max_result = read_long(max_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(max_result, Value::Uint(u64::from(u32::MAX)));

        // Test zero value
        let zero_buffer = &[0x00, 0x00, 0x00, 0x00];
        let zero_result = read_long(zero_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(zero_result, Value::Uint(0));
    }

    #[test]
    fn test_read_short_extreme_values() {
        // Test maximum unsigned 16-bit value
        let max_buffer = &[0xff, 0xff];
        let max_result = read_short(max_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(max_result, Value::Uint(u64::from(u16::MAX)));

        // Test zero value
        let zero_buffer = &[0x00, 0x00];
        let zero_result = read_short(zero_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(zero_result, Value::Uint(0));
    }

    #[test]
    fn test_multi_byte_reading_consistency() {
        // Test that reading the same bytes with different functions gives consistent results
        let buffer = &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xde, 0xf0];

        // Read as individual bytes
        let byte0 = read_byte(buffer, 0).unwrap();
        let byte1 = read_byte(buffer, 1).unwrap();

        // Read as short
        let short = read_short(buffer, 0, Endianness::Little, false).unwrap();

        // Verify consistency
        match (byte0, byte1, short) {
            (Value::Uint(b0), Value::Uint(b1), Value::Uint(s)) => {
                assert_eq!(s, b0 + (b1 << 8)); // Little-endian composition
            }
            _ => panic!("Expected all Uint values"),
        }
    }

    // Tests for UnsupportedType error
    #[test]
    fn test_unsupported_type_error() {
        let error = TypeReadError::UnsupportedType {
            type_name: "CustomType".to_string(),
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("Unsupported type"));
        assert!(error_string.contains("CustomType"));
    }

    #[test]
    fn test_unsupported_type_error_debug() {
        let error = TypeReadError::UnsupportedType {
            type_name: "TestType".to_string(),
        };

        let debug_string = format!("{error:?}");
        assert!(debug_string.contains("UnsupportedType"));
        assert!(debug_string.contains("TestType"));
    }

    #[test]
    fn test_unsupported_type_error_equality() {
        let error1 = TypeReadError::UnsupportedType {
            type_name: "Type1".to_string(),
        };
        let error2 = TypeReadError::UnsupportedType {
            type_name: "Type1".to_string(),
        };
        let error3 = TypeReadError::UnsupportedType {
            type_name: "Type2".to_string(),
        };

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    // Tests for read_typed_value function
    #[test]
    fn test_read_typed_value_byte() {
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];
        let type_kind = TypeKind::Byte;

        let result = read_typed_value(buffer, 0, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x7f));

        let result = read_typed_value(buffer, 3, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x46));
    }

    #[test]
    fn test_read_typed_value_short_unsigned_little_endian() {
        let buffer = &[0x34, 0x12, 0x78, 0x56];
        let type_kind = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };

        let result = read_typed_value(buffer, 0, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x1234));

        let result = read_typed_value(buffer, 2, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x5678));
    }

    #[test]
    fn test_read_typed_value_short_signed_big_endian() {
        let buffer = &[0x80, 0x00, 0x7f, 0xff];
        let type_kind = TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        };

        // 0x8000 = -32768 in signed 16-bit
        let result = read_typed_value(buffer, 0, &type_kind).unwrap();
        assert_eq!(result, Value::Int(-32768));

        // 0x7fff = 32767 in signed 16-bit
        let result = read_typed_value(buffer, 2, &type_kind).unwrap();
        assert_eq!(result, Value::Int(32767));
    }

    #[test]
    fn test_read_typed_value_long_unsigned_little_endian() {
        let buffer = &[0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56];
        let type_kind = TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        };

        let result = read_typed_value(buffer, 0, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678));

        let result = read_typed_value(buffer, 4, &type_kind).unwrap();
        assert_eq!(result, Value::Uint(0x5678_9abc));
    }

    #[test]
    fn test_read_typed_value_long_signed_big_endian() {
        let buffer = &[0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff];
        let type_kind = TypeKind::Long {
            endian: Endianness::Big,
            signed: true,
        };

        // 0x80000000 = -2147483648 in signed 32-bit
        let result = read_typed_value(buffer, 0, &type_kind).unwrap();
        assert_eq!(result, Value::Int(-2_147_483_648));

        // 0x7fffffff = 2147483647 in signed 32-bit
        let result = read_typed_value(buffer, 4, &type_kind).unwrap();
        assert_eq!(result, Value::Int(2_147_483_647));
    }

    #[test]
    fn test_read_typed_value_native_endian() {
        let buffer = &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xde, 0xf0];

        // Test short with native endianness
        let short_type = TypeKind::Short {
            endian: Endianness::Native,
            signed: false,
        };
        let short_result = read_typed_value(buffer, 0, &short_type).unwrap();
        match short_result {
            Value::Uint(val) => {
                // Should be either 0x1234 (little-endian) or 0x3412 (big-endian)
                assert!(val == 0x1234 || val == 0x3412);
            }
            _ => panic!("Expected Value::Uint variant"),
        }

        // Test long with native endianness
        let long_type = TypeKind::Long {
            endian: Endianness::Native,
            signed: false,
        };
        let long_result = read_typed_value(buffer, 0, &long_type).unwrap();
        match long_result {
            Value::Uint(val) => {
                // Should be either 0x56781234 (little-endian) or 0x12345678 (big-endian)
                assert!(val == 0x5678_1234 || val == 0x1234_5678);
            }
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_typed_value_string_unsupported() {
        let buffer = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00]; // "Hello\0"
        let type_kind = TypeKind::String { max_length: None };

        let result = read_typed_value(buffer, 0, &type_kind);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::UnsupportedType {
                type_name: "String".to_string()
            }
        );
    }

    #[test]
    fn test_read_typed_value_string_with_max_length_unsupported() {
        let buffer = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00];
        let type_kind = TypeKind::String {
            max_length: Some(10),
        };

        let result = read_typed_value(buffer, 0, &type_kind);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::UnsupportedType {
                type_name: "String".to_string()
            }
        );
    }

    #[test]
    fn test_read_typed_value_buffer_overrun() {
        let buffer = &[0x12, 0x34];

        // Try to read a long (4 bytes) from a 2-byte buffer
        let long_type = TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        };
        let result = read_typed_value(buffer, 0, &long_type);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 2
            }
        );

        // Try to read a short (2 bytes) at offset 1 from a 2-byte buffer
        let short_type = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let result = read_typed_value(buffer, 1, &short_type);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 1,
                buffer_len: 2
            }
        );
    }

    #[test]
    fn test_read_typed_value_all_supported_types() {
        let buffer = &[0x7f, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a];

        // Test all supported TypeKind variants
        let test_cases = vec![
            (TypeKind::Byte, 0, Value::Uint(0x7f)),
            (
                TypeKind::Short {
                    endian: Endianness::Little,
                    signed: false,
                },
                1,
                Value::Uint(0x1234), // bytes [0x34, 0x12] -> 0x1234 little-endian
            ),
            (
                TypeKind::Short {
                    endian: Endianness::Big,
                    signed: false,
                },
                1,
                Value::Uint(0x3412), // bytes [0x34, 0x12] -> 0x3412 big-endian
            ),
            (
                TypeKind::Long {
                    endian: Endianness::Little,
                    signed: false,
                },
                1,
                Value::Uint(0x5678_1234), // bytes [0x34, 0x12, 0x78, 0x56] -> 0x56781234 little-endian
            ),
            (
                TypeKind::Long {
                    endian: Endianness::Big,
                    signed: false,
                },
                1,
                Value::Uint(0x3412_7856), // bytes [0x34, 0x12, 0x78, 0x56] -> 0x34127856 big-endian
            ),
        ];

        for (type_kind, offset, expected) in test_cases {
            let result = read_typed_value(buffer, offset, &type_kind).unwrap();
            assert_eq!(result, expected, "Failed for type: {type_kind:?}");
        }
    }

    #[test]
    fn test_read_typed_value_signed_vs_unsigned() {
        let buffer = &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff];

        // Test signed vs unsigned interpretation for shorts
        let unsigned_short = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let signed_short = TypeKind::Short {
            endian: Endianness::Little,
            signed: true,
        };

        let unsigned_result = read_typed_value(buffer, 0, &unsigned_short).unwrap();
        let signed_result = read_typed_value(buffer, 0, &signed_short).unwrap();

        assert_eq!(unsigned_result, Value::Uint(65535));
        assert_eq!(signed_result, Value::Int(-1));

        // Test signed vs unsigned interpretation for longs
        let unsigned_long = TypeKind::Long {
            endian: Endianness::Little,
            signed: false,
        };
        let signed_long = TypeKind::Long {
            endian: Endianness::Little,
            signed: true,
        };

        let unsigned_result = read_typed_value(buffer, 0, &unsigned_long).unwrap();
        let signed_result = read_typed_value(buffer, 0, &signed_long).unwrap();

        assert_eq!(unsigned_result, Value::Uint(4_294_967_295));
        assert_eq!(signed_result, Value::Int(-1));
    }

    #[test]
    fn test_read_typed_value_consistency_with_direct_calls() {
        let buffer = &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xde, 0xf0];

        // Test that read_typed_value gives same results as direct function calls
        let byte_type = TypeKind::Byte;
        let direct_byte = read_byte(buffer, 0).unwrap();
        let typed_byte = read_typed_value(buffer, 0, &byte_type).unwrap();
        assert_eq!(direct_byte, typed_byte);

        let short_type = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let direct_short = read_short(buffer, 0, Endianness::Little, false).unwrap();
        let typed_short = read_typed_value(buffer, 0, &short_type).unwrap();
        assert_eq!(direct_short, typed_short);

        let long_type = TypeKind::Long {
            endian: Endianness::Big,
            signed: true,
        };
        let direct_long = read_long(buffer, 0, Endianness::Big, true).unwrap();
        let typed_long = read_typed_value(buffer, 0, &long_type).unwrap();
        assert_eq!(direct_long, typed_long);
    }

    #[test]
    fn test_read_typed_value_empty_buffer() {
        let buffer = &[];

        // All types should fail on empty buffer
        let types = vec![
            TypeKind::Byte,
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
        ];

        for type_kind in types {
            let result = read_typed_value(buffer, 0, &type_kind);
            assert!(result.is_err());
            match result.unwrap_err() {
                TypeReadError::BufferOverrun { offset, buffer_len } => {
                    assert_eq!(offset, 0);
                    assert_eq!(buffer_len, 0);
                }
                TypeReadError::UnsupportedType { .. } => panic!("Expected BufferOverrun error"),
            }
        }
    }
}
