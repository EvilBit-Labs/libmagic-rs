// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::TypeReadError;
use crate::parser::ast::{Endianness, Value};
use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian};

/// Safely reads a 32-bit IEEE 754 float from the buffer at the specified offset.
///
/// The result is widened to `f64` and returned as `Value::Float(f64)`.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_float;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// // IEEE 754 little-endian representation of 1.0f32: 0x3f800000
/// let buffer = &[0x00, 0x00, 0x80, 0x3f];
/// let result = read_float(buffer, 0, Endianness::Little).unwrap();
/// assert_eq!(result, Value::Float(1.0));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if fewer than 4 bytes are available at the
/// requested offset.
pub fn read_float(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
) -> Result<Value, TypeReadError> {
    let end = offset.checked_add(4).ok_or(TypeReadError::BufferOverrun {
        offset,
        buffer_len: buffer.len(),
    })?;
    let bytes = buffer
        .get(offset..end)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;

    let value = match endian {
        Endianness::Little => LittleEndian::read_f32(bytes),
        Endianness::Big => BigEndian::read_f32(bytes),
        Endianness::Native => NativeEndian::read_f32(bytes),
    };

    Ok(Value::Float(f64::from(value)))
}

/// Safely reads a 64-bit IEEE 754 double from the buffer at the specified offset.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_double;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// // IEEE 754 big-endian representation of 1.0f64: 0x3ff0000000000000
/// let buffer = &[0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
/// let result = read_double(buffer, 0, Endianness::Big).unwrap();
/// assert_eq!(result, Value::Float(1.0));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if fewer than 8 bytes are available at the
/// requested offset.
pub fn read_double(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
) -> Result<Value, TypeReadError> {
    let end = offset.checked_add(8).ok_or(TypeReadError::BufferOverrun {
        offset,
        buffer_len: buffer.len(),
    })?;
    let bytes = buffer
        .get(offset..end)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;

    let value = match endian {
        Endianness::Little => LittleEndian::read_f64(bytes),
        Endianness::Big => BigEndian::read_f64(bytes),
        Endianness::Native => NativeEndian::read_f64(bytes),
    };

    Ok(Value::Float(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_float_endianness() {
        // IEEE 754 representation of 1.0f32: 0x3f800000
        let cases: Vec<(&[u8], Endianness, f64)> = vec![
            // Little-endian: bytes reversed
            (&[0x00, 0x00, 0x80, 0x3f], Endianness::Little, 1.0),
            // Big-endian: bytes in order
            (&[0x3f, 0x80, 0x00, 0x00], Endianness::Big, 1.0),
            // -2.5f32 = 0xc0200000
            (
                &[0x00, 0x00, 0x20, 0xc0],
                Endianness::Little,
                f64::from(-2.5_f32),
            ),
            (
                &[0xc0, 0x20, 0x00, 0x00],
                Endianness::Big,
                f64::from(-2.5_f32),
            ),
            // 0.0f32 = 0x00000000
            (&[0x00, 0x00, 0x00, 0x00], Endianness::Little, 0.0),
            (&[0x00, 0x00, 0x00, 0x00], Endianness::Big, 0.0),
        ];

        for (buffer, endian, expected) in cases {
            let result = read_float(buffer, 0, endian).unwrap();
            assert_eq!(
                result,
                Value::Float(expected),
                "endian={endian:?}, expected={expected}"
            );
        }
    }

    #[test]
    fn test_read_float_native_endian() {
        // 1.0f32 bytes in both possible orders
        let le_bytes = &[0x00, 0x00, 0x80, 0x3f];
        let be_bytes = &[0x3f, 0x80, 0x00, 0x00];

        let le_result = read_float(le_bytes, 0, Endianness::Native);
        let be_result = read_float(be_bytes, 0, Endianness::Native);

        // One of these should produce 1.0
        let either_is_one = matches!(le_result, Ok(Value::Float(v)) if v == 1.0)
            || matches!(be_result, Ok(Value::Float(v)) if v == 1.0);
        assert!(either_is_one, "Native endian should match one byte order");
    }

    #[test]
    fn test_read_float_at_offset() {
        // Two bytes of padding, then 1.0f32 in big-endian
        let buffer = &[0xaa, 0xbb, 0x3f, 0x80, 0x00, 0x00];
        let result = read_float(buffer, 2, Endianness::Big).unwrap();
        assert_eq!(result, Value::Float(1.0));
    }

    #[test]
    fn test_read_float_returns_value_float() {
        let buffer = &[0x00, 0x00, 0x80, 0x3f]; // 1.0f32 LE
        match read_float(buffer, 0, Endianness::Little).unwrap() {
            Value::Float(_) => {}
            other => panic!("Expected Value::Float, got {other:?}"),
        }
    }

    #[test]
    fn test_read_float_buffer_overrun() {
        // Too few bytes
        assert_eq!(
            read_float(&[0x00, 0x00, 0x80], 0, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 3,
            }
        );

        // Empty buffer
        assert_eq!(
            read_float(&[], 0, Endianness::Big).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0,
            }
        );

        // Offset past end
        assert_eq!(
            read_float(&[0x00; 8], 6, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 6,
                buffer_len: 8,
            }
        );
    }

    #[test]
    fn test_read_float_offset_overflow() {
        let buffer = &[0x00; 4];
        assert_eq!(
            read_float(buffer, usize::MAX, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 4,
            }
        );
    }

    #[test]
    fn test_read_double_endianness() {
        // IEEE 754 representation of 1.0f64: 0x3ff0000000000000
        let cases: Vec<(&[u8], Endianness, f64)> = vec![
            // Little-endian
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f],
                Endianness::Little,
                1.0,
            ),
            // Big-endian
            (
                &[0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                1.0,
            ),
            // -2.5f64 = 0xc004000000000000
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xc0],
                Endianness::Little,
                -2.5,
            ),
            (
                &[0xc0, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Big,
                -2.5,
            ),
            // 0.0f64 = all zeros
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Little,
                0.0,
            ),
        ];

        for (buffer, endian, expected) in cases {
            let result = read_double(buffer, 0, endian).unwrap();
            assert_eq!(
                result,
                Value::Float(expected),
                "endian={endian:?}, expected={expected}"
            );
        }
    }

    #[test]
    fn test_read_double_native_endian() {
        let le_bytes = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f];
        let be_bytes = &[0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let le_result = read_double(le_bytes, 0, Endianness::Native);
        let be_result = read_double(be_bytes, 0, Endianness::Native);

        let either_is_one = matches!(le_result, Ok(Value::Float(v)) if v == 1.0)
            || matches!(be_result, Ok(Value::Float(v)) if v == 1.0);
        assert!(either_is_one, "Native endian should match one byte order");
    }

    #[test]
    fn test_read_double_at_offset() {
        // Three bytes of padding, then 1.0f64 in big-endian
        let buffer = &[
            0xaa, 0xbb, 0xcc, 0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let result = read_double(buffer, 3, Endianness::Big).unwrap();
        assert_eq!(result, Value::Float(1.0));
    }

    #[test]
    fn test_read_double_returns_value_float() {
        let buffer = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f]; // 1.0f64 LE
        match read_double(buffer, 0, Endianness::Little).unwrap() {
            Value::Float(_) => {}
            other => panic!("Expected Value::Float, got {other:?}"),
        }
    }

    #[test]
    fn test_read_double_buffer_overrun() {
        // Too few bytes
        assert_eq!(
            read_double(&[0x00; 7], 0, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 7,
            }
        );

        // Empty buffer
        assert_eq!(
            read_double(&[], 0, Endianness::Big).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0,
            }
        );

        // Offset past end
        assert_eq!(
            read_double(&[0x00; 16], 10, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 16,
            }
        );
    }

    #[test]
    fn test_read_double_offset_overflow() {
        let buffer = &[0x00; 8];
        assert_eq!(
            read_double(buffer, usize::MAX, Endianness::Little).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 8,
            }
        );
    }
}
