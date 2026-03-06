// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::TypeReadError;
use crate::parser::ast::{Endianness, Value};
use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian};

/// Safely reads a single byte from the buffer at the specified offset.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to read the byte from
/// * `signed` - Whether to interpret the byte as signed (`i8`) or unsigned (`u8`)
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_byte;
/// use libmagic_rs::parser::ast::Value;
///
/// let buffer = &[0x7f, 0x80, 0x4c, 0x46];
///
/// let result = read_byte(buffer, 1, false).unwrap();
/// assert_eq!(result, Value::Uint(0x80));
///
/// let result = read_byte(buffer, 1, true).unwrap();
/// assert_eq!(result, Value::Int(-128));
/// ```
/// # Errors
/// Returns `TypeReadError::BufferOverrun` if `offset` is outside the buffer.
pub fn read_byte(buffer: &[u8], offset: usize, signed: bool) -> Result<Value, TypeReadError> {
    buffer
        .get(offset)
        .map(|&byte| {
            if signed {
                #[allow(clippy::cast_possible_wrap)]
                Value::Int(i64::from(byte as i8))
            } else {
                Value::Uint(u64::from(byte))
            }
        })
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })
}

/// Safely reads a 16-bit integer from the buffer at the specified offset.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
/// * `signed` - Whether to interpret the value as signed (`i16`) or unsigned (`u16`)
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_short;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// let buffer = &[0x34, 0x12, 0xff, 0x7f];
///
/// let result = read_short(buffer, 0, Endianness::Little, false).unwrap();
/// assert_eq!(result, Value::Uint(0x1234));
///
/// let result = read_short(buffer, 2, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::Int(32767));
/// ```
/// # Errors
/// Returns `TypeReadError::BufferOverrun` if fewer than 2 bytes are available at the
/// requested offset.
pub fn read_short(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    signed: bool,
) -> Result<Value, TypeReadError> {
    let end = offset.checked_add(2).ok_or(TypeReadError::BufferOverrun {
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

/// Safely reads a 32-bit integer from the buffer at the specified offset.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
/// * `signed` - Whether to interpret the value as signed (`i32`) or unsigned (`u32`)
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_long;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// let buffer = &[0x78, 0x56, 0x34, 0x12, 0xff, 0xff, 0xff, 0x7f];
///
/// let result = read_long(buffer, 0, Endianness::Little, false).unwrap();
/// assert_eq!(result, Value::Uint(0x12345678));
///
/// let result = read_long(buffer, 4, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::Int(2147483647));
/// ```
/// # Errors
/// Returns `TypeReadError::BufferOverrun` if fewer than 4 bytes are available at the
/// requested offset.
pub fn read_long(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    signed: bool,
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

/// Safely reads a 64-bit integer from the buffer at the specified offset.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `endian` - The byte order to use when interpreting the bytes
/// * `signed` - Whether to interpret the value as signed (`i64`) or unsigned (`u64`)
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_quad;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// let buffer = &[0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12];
///
/// let result = read_quad(buffer, 0, Endianness::Little, false).unwrap();
/// assert_eq!(result, Value::Uint(0x1234_5678_90ab_cdef));
///
/// let result = read_quad(buffer, 0, Endianness::Little, true).unwrap();
/// assert_eq!(result, Value::Int(0x1234_5678_90ab_cdef));
/// ```
/// # Errors
/// Returns `TypeReadError::BufferOverrun` if fewer than 8 bytes are available at the
/// requested offset.
pub fn read_quad(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
    signed: bool,
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
        Endianness::Little => LittleEndian::read_u64(bytes),
        Endianness::Big => BigEndian::read_u64(bytes),
        Endianness::Native => NativeEndian::read_u64(bytes),
    };

    if signed {
        #[allow(clippy::cast_possible_wrap)]
        Ok(Value::Int(value as i64))
    } else {
        Ok(Value::Uint(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_byte_values() {
        let buffer: Vec<u8> = (0..=255).collect();
        for (i, &byte) in buffer.iter().enumerate() {
            assert_eq!(
                read_byte(&buffer, i, false).unwrap(),
                Value::Uint(u64::from(byte))
            );
        }
    }

    #[test]
    fn test_read_byte_out_of_bounds() {
        assert_eq!(
            read_byte(&[], 0, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0
            }
        );
        assert_eq!(
            read_byte(&[0x42], 1, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 1,
                buffer_len: 1
            }
        );
        assert_eq!(
            read_byte(&[1, 2, 3], 100, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 100,
                buffer_len: 3
            }
        );
    }

    #[test]
    fn test_read_byte_signedness() {
        let cases: Vec<(u8, bool, Value)> = vec![
            (0x00, false, Value::Uint(0)),
            (0x7f, false, Value::Uint(127)),
            (0x80, false, Value::Uint(128)),
            (0xff, false, Value::Uint(255)),
            (0x00, true, Value::Int(0)),
            (0x7f, true, Value::Int(127)),
            (0x80, true, Value::Int(-128)),
            (0xff, true, Value::Int(-1)),
        ];
        for (byte, signed, expected) in cases {
            let result = read_byte(&[byte], 0, signed).unwrap();
            assert_eq!(result, expected, "byte=0x{byte:02x}, signed={signed}");
        }
    }

    #[test]
    fn test_read_short_little_endian_unsigned() {
        let buffer = &[0x34, 0x12, 0x78, 0x56];
        let result = read_short(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234));

        let result = read_short(buffer, 2, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678));
    }

    #[test]
    fn test_read_short_big_endian_unsigned() {
        let buffer = &[0x12, 0x34, 0x56, 0x78];
        let result = read_short(buffer, 0, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234));

        let result = read_short(buffer, 2, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678));
    }

    #[test]
    fn test_read_short_native_endian_unsigned() {
        let buffer = &[0x34, 0x12, 0x78, 0x56];
        let result = read_short(buffer, 0, Endianness::Native, false).unwrap();

        match result {
            Value::Uint(val) => assert!(val == 0x1234 || val == 0x3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_short_signed_positive() {
        let buffer = &[0xff, 0x7f];
        let result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(32767));
    }

    #[test]
    fn test_read_short_signed_negative() {
        let buffer = &[0x00, 0x80];
        let result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(-32768));
    }

    #[test]
    fn test_read_short_signed_vs_unsigned() {
        let buffer = &[0xff, 0xff];
        let unsigned_result = read_short(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(unsigned_result, Value::Uint(65535));

        let signed_result = read_short(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(signed_result, Value::Int(-1));
    }

    #[test]
    fn test_read_short_buffer_overrun() {
        let buffer = &[0x12];
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
        let little = read_short(buffer, 0, Endianness::Little, false).unwrap();
        let big = read_short(buffer, 0, Endianness::Big, false).unwrap();
        let native = read_short(buffer, 0, Endianness::Native, false).unwrap();

        assert_eq!(little, Value::Uint(0x3412));
        assert_eq!(big, Value::Uint(0x1234));

        match native {
            Value::Uint(val) => assert!(val == 0x1234 || val == 0x3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_short_extreme_values() {
        let max_buffer = &[0xff, 0xff];
        let max_result = read_short(max_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(max_result, Value::Uint(u64::from(u16::MAX)));

        let zero_buffer = &[0x00, 0x00];
        let zero_result = read_short(zero_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(zero_result, Value::Uint(0));
    }

    #[test]
    fn test_read_long_little_endian_unsigned() {
        let buffer = &[0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56];
        let result = read_long(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678));

        let result = read_long(buffer, 4, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678_9abc));
    }

    #[test]
    fn test_read_long_big_endian_unsigned() {
        let buffer = &[0x12, 0x34, 0x56, 0x78, 0x56, 0x78, 0x9a, 0xbc];
        let result = read_long(buffer, 0, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678));

        let result = read_long(buffer, 4, Endianness::Big, false).unwrap();
        assert_eq!(result, Value::Uint(0x5678_9abc));
    }

    #[test]
    fn test_read_long_native_endian_unsigned() {
        let buffer = &[0x78, 0x56, 0x34, 0x12];
        let result = read_long(buffer, 0, Endianness::Native, false).unwrap();

        match result {
            Value::Uint(val) => assert!(val == 0x1234_5678 || val == 0x7856_3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_long_signed_positive() {
        let buffer = &[0xff, 0xff, 0xff, 0x7f];
        let result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(2_147_483_647));
    }

    #[test]
    fn test_read_long_signed_negative() {
        let buffer = &[0x00, 0x00, 0x00, 0x80];
        let result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(result, Value::Int(-2_147_483_648));
    }

    #[test]
    fn test_read_long_signed_vs_unsigned() {
        let buffer = &[0xff, 0xff, 0xff, 0xff];
        let unsigned_result = read_long(buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(unsigned_result, Value::Uint(4_294_967_295));

        let signed_result = read_long(buffer, 0, Endianness::Little, true).unwrap();
        assert_eq!(signed_result, Value::Int(-1));
    }

    #[test]
    fn test_read_long_buffer_overrun() {
        let buffer = &[0x12, 0x34, 0x56];
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
        let little = read_long(buffer, 0, Endianness::Little, false).unwrap();
        let big = read_long(buffer, 0, Endianness::Big, false).unwrap();
        let native = read_long(buffer, 0, Endianness::Native, false).unwrap();

        assert_eq!(little, Value::Uint(0x7856_3412));
        assert_eq!(big, Value::Uint(0x1234_5678));

        match native {
            Value::Uint(val) => assert!(val == 0x1234_5678 || val == 0x7856_3412),
            _ => panic!("Expected Value::Uint variant"),
        }
    }

    #[test]
    fn test_read_long_extreme_values() {
        let max_buffer = &[0xff, 0xff, 0xff, 0xff];
        let max_result = read_long(max_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(max_result, Value::Uint(u64::from(u32::MAX)));

        let zero_buffer = &[0x00, 0x00, 0x00, 0x00];
        let zero_result = read_long(zero_buffer, 0, Endianness::Little, false).unwrap();
        assert_eq!(zero_result, Value::Uint(0));
    }

    #[test]
    fn test_read_quad_endianness_and_signedness() {
        let cases: Vec<(&[u8], Endianness, bool, Value)> = vec![
            (
                &[0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12],
                Endianness::Little,
                false,
                Value::Uint(0x1234_5678_90ab_cdef),
            ),
            (
                &[0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef],
                Endianness::Big,
                false,
                Value::Uint(0x1234_5678_90ab_cdef),
            ),
            (
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
                Endianness::Little,
                true,
                Value::Int(i64::MAX),
            ),
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80],
                Endianness::Little,
                true,
                Value::Int(i64::MIN),
            ),
            (
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                Endianness::Big,
                true,
                Value::Int(-1),
            ),
            (
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                Endianness::Little,
                false,
                Value::Uint(u64::MAX),
            ),
            (
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                Endianness::Little,
                false,
                Value::Uint(0),
            ),
        ];
        for (buffer, endian, signed, expected) in cases {
            let result = read_quad(buffer, 0, endian, signed).unwrap();
            assert_eq!(result, expected, "endian={endian:?}, signed={signed}");
        }
    }

    #[test]
    fn test_read_quad_buffer_overrun() {
        let buffer = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        assert_eq!(
            read_quad(buffer, 0, Endianness::Little, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 7
            }
        );

        assert_eq!(
            read_quad(&[], 0, Endianness::Big, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0
            }
        );

        let buffer = &[0x00; 16];
        assert_eq!(
            read_quad(buffer, 10, Endianness::Little, false).unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 16
            }
        );
    }

    #[test]
    fn test_read_quad_at_offset() {
        let buffer = &[0x00, 0x00, 0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12];
        let result = read_quad(buffer, 2, Endianness::Little, false).unwrap();
        assert_eq!(result, Value::Uint(0x1234_5678_90ab_cdef));
    }

    #[test]
    fn test_read_short_offset_overflow() {
        let buffer = &[0x12, 0x34];
        let result = read_short(buffer, usize::MAX, Endianness::Little, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 2,
            }
        );
    }

    #[test]
    fn test_read_long_offset_overflow() {
        let buffer = &[0x12, 0x34, 0x56, 0x78];
        let result = read_long(buffer, usize::MAX, Endianness::Little, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 4,
            }
        );
    }

    #[test]
    fn test_read_quad_offset_overflow() {
        let buffer = &[0x01; 8];
        let result = read_quad(buffer, usize::MAX, Endianness::Little, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: 8,
            }
        );
    }

    #[test]
    fn test_multi_byte_reading_consistency() {
        let buffer = &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xde, 0xf0];

        let byte0 = read_byte(buffer, 0, false).unwrap();
        let byte1 = read_byte(buffer, 1, false).unwrap();
        let short = read_short(buffer, 0, Endianness::Little, false).unwrap();

        match (byte0, byte1, short) {
            (Value::Uint(b0), Value::Uint(b1), Value::Uint(s)) => {
                assert_eq!(s, b0 + (b1 << 8));
            }
            _ => panic!("Expected all Uint values"),
        }
    }
}
