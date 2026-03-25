// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::TypeReadError;
use crate::parser::ast::{PStringLengthWidth, Value};

/// Safely reads a null-terminated string from the buffer at the specified offset.
///
/// This function reads bytes from the buffer starting at the given offset until it
/// encounters a null byte (0x00) or reaches the maximum length limit. The resulting
/// bytes are converted to a UTF-8 string with proper error handling for invalid
/// sequences.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading the string from
/// * `max_length` - Optional maximum number of bytes to read excluding the null terminator.
///   If a NUL is found within `max_length` bytes, it is not counted in the result length.
///   If no NUL is found, up to `max_length` bytes are returned with no trailing NUL.
///   When `None`, reads until the first NUL or end of buffer.
///
/// # Returns
///
/// Returns `Ok(Value::String(string))` if the read is successful. Invalid UTF-8 byte
/// sequences are replaced with the Unicode replacement character (U+FFFD) rather than
/// producing an error.
///
/// # Security
///
/// This function provides several security guarantees:
/// - Bounds checking prevents reading beyond buffer limits
/// - Length limits prevent excessive memory allocation
/// - Invalid UTF-8 sequences are safely replaced with U+FFFD, preventing undefined behavior
/// - Null termination handling prevents runaway reads
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_string;
/// use libmagic_rs::parser::ast::Value;
///
/// let buffer = b"Hello\x00World";
/// let result = read_string(buffer, 0, None).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
///
/// let buffer = b"VeryLongString\x00";
/// let result = read_string(buffer, 0, Some(4)).unwrap();
/// assert_eq!(result, Value::String("Very".to_string()));
///
/// let buffer = b"NoNull";
/// let result = read_string(buffer, 0, Some(6)).unwrap();
/// assert_eq!(result, Value::String("NoNull".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if the offset is greater than or equal to the buffer
/// length.
pub fn read_string(
    buffer: &[u8],
    offset: usize,
    max_length: Option<usize>,
) -> Result<Value, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let remaining_buffer = &buffer[offset..];
    let read_length = if let Some(max_len) = max_length {
        let search_len = std::cmp::min(max_len, remaining_buffer.len());
        memchr::memchr(0, &remaining_buffer[..search_len]).unwrap_or(search_len)
    } else {
        memchr::memchr(0, remaining_buffer).unwrap_or(remaining_buffer.len())
    };

    let string_bytes = &remaining_buffer[..read_length];
    let string_value = String::from_utf8_lossy(string_bytes).into_owned();

    Ok(Value::String(string_value))
}

/// Reads a Pascal-style length-prefixed string from the buffer.
///
/// Pascal strings store the length prefix (1, 2, or 4 bytes depending on
/// `length_width`), followed by that many bytes of string data. Unlike C
/// strings, they are not null-terminated.
///
/// # Arguments
///
/// * `buffer` - The byte buffer to read from
/// * `offset` - The offset position to start reading from
/// * `max_length` - Optional maximum length limit (caps the length byte value)
///
/// # Returns
///
/// Returns `Ok(Value::String(string))` if successful. Invalid UTF-8 byte sequences
/// are replaced with the Unicode replacement character (U+FFFD).
///
/// # Security
///
/// This function provides bounds checking to prevent reading beyond buffer limits.
/// When `max_length` is set, bounds are validated against the capped length, not the
/// raw length byte. This matches GNU `file` behavior: `max_length` is intended to
/// handle cases where the length byte may reference more data than actually exists.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_pstring;
/// use libmagic_rs::parser::ast::{Value, PStringLengthWidth};
///
/// let buffer = b"\x05Hello";
/// let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
///
/// let buffer = b"\x00\x05Hello";
/// let result = read_pstring(buffer, 0, None, PStringLengthWidth::TwoByteBE, false).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
///
/// let buffer = b"\x05\x00\x00\x00Hello";
/// let result = read_pstring(buffer, 0, None, PStringLengthWidth::FourByteLE, false).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if:
/// - The offset is beyond buffer bounds (cannot read the length prefix)
/// - The string data (length prefix value) extends beyond the buffer
pub fn read_pstring(
    buffer: &[u8],
    offset: usize,
    max_length: Option<usize>,
    length_width: PStringLengthWidth,
    length_includes_itself: bool,
) -> Result<Value, TypeReadError> {
    let width = length_width.byte_count();
    // Check if we can read the length prefix (checked arithmetic to prevent overflow)
    let prefix_end = offset
        .checked_add(width)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;
    let len_bytes = buffer
        .get(offset..prefix_end)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;
    let string_length = match length_width {
        PStringLengthWidth::OneByte => usize::from(len_bytes[0]),
        PStringLengthWidth::TwoByteBE => {
            let arr: [u8; 2] = len_bytes
                .try_into()
                .map_err(|_| TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: buffer.len(),
                })?;
            usize::from(u16::from_be_bytes(arr))
        }
        PStringLengthWidth::TwoByteLE => {
            let arr: [u8; 2] = len_bytes
                .try_into()
                .map_err(|_| TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: buffer.len(),
                })?;
            usize::from(u16::from_le_bytes(arr))
        }
        PStringLengthWidth::FourByteBE => {
            let arr: [u8; 4] = len_bytes
                .try_into()
                .map_err(|_| TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: buffer.len(),
                })?;
            u32::from_be_bytes(arr) as usize
        }
        PStringLengthWidth::FourByteLE => {
            let arr: [u8; 4] = len_bytes
                .try_into()
                .map_err(|_| TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: buffer.len(),
                })?;
            u32::from_le_bytes(arr) as usize
        }
    };

    // /J flag: the stored length includes the prefix width itself
    let string_length = if length_includes_itself {
        string_length
            .checked_sub(width)
            .ok_or(TypeReadError::InvalidPStringLength {
                stored_length: string_length,
                prefix_width: width,
            })?
    } else {
        string_length
    };

    let actual_length = if let Some(max_len) = max_length {
        std::cmp::min(string_length, max_len)
    } else {
        string_length
    };

    let string_start = offset
        .checked_add(width)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;
    let string_end =
        string_start
            .checked_add(actual_length)
            .ok_or(TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: buffer.len(),
            })?;
    if string_end > buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset: string_end,
            buffer_len: buffer.len(),
        });
    }
    let string_bytes = &buffer[string_start..string_end];
    let string_value = String::from_utf8_lossy(string_bytes).into_owned();
    Ok(Value::String(string_value))
}

#[cfg(test)]
mod tests {
    use crate::parser::ast::PStringLengthWidth;
    #[test]
    fn test_read_pstring_one_byte_width() {
        let result = read_pstring(b"\x03abc", 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("abc".to_string()));

        let result =
            read_pstring(b"\x05Hello", 0, Some(3), PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Hel".to_string()));
    }

    #[test]
    fn test_read_pstring_two_byte_width() {
        let result = read_pstring(
            b"\x03\x00abc",
            0,
            None,
            PStringLengthWidth::TwoByteLE,
            false,
        )
        .unwrap();
        assert_eq!(result, Value::String("abc".to_string()));

        let result = read_pstring(
            b"\x05\x00Hello",
            0,
            Some(3),
            PStringLengthWidth::TwoByteLE,
            false,
        )
        .unwrap();
        assert_eq!(result, Value::String("Hel".to_string()));
    }

    #[test]
    fn test_read_pstring_four_byte_width() {
        let result = read_pstring(
            b"\x03\x00\x00\x00abc",
            0,
            None,
            PStringLengthWidth::FourByteLE,
            false,
        )
        .unwrap();
        assert_eq!(result, Value::String("abc".to_string()));

        let result = read_pstring(
            b"\x05\x00\x00\x00Hello",
            0,
            Some(3),
            PStringLengthWidth::FourByteLE,
            false,
        )
        .unwrap();
        assert_eq!(result, Value::String("Hel".to_string()));
    }

    #[test]
    fn test_read_pstring_buffer_overrun_widths() {
        // Not enough bytes for length prefix
        let cases: &[(&[u8], usize, PStringLengthWidth)] = &[
            (b"", 0, PStringLengthWidth::OneByte),
            (b"\x01", 1, PStringLengthWidth::OneByte),
            (b"\x01", 0, PStringLengthWidth::TwoByteLE),
            (b"\x01\x00", 0, PStringLengthWidth::FourByteLE),
        ];
        for &(buffer, offset, width) in cases {
            let result = read_pstring(buffer, offset, None, width, false);
            assert!(
                matches!(result, Err(TypeReadError::BufferOverrun { .. })),
                "Expected buffer overrun for buffer {buffer:?}, offset {offset}, width {width:?}"
            );
        }
    }

    #[test]
    fn test_read_pstring_data_overrun_widths() {
        // Enough for prefix, not enough for data
        let cases: &[(&[u8], usize, PStringLengthWidth)] = &[
            (b"\x05ab", 0, PStringLengthWidth::OneByte),
            (b"\x05\x00ab", 0, PStringLengthWidth::TwoByteLE),
            (b"\x05\x00\x00\x00ab", 0, PStringLengthWidth::FourByteLE),
        ];
        for &(buffer, offset, width) in cases {
            let result = read_pstring(buffer, offset, None, width, false);
            assert!(
                matches!(result, Err(TypeReadError::BufferOverrun { .. })),
                "Expected buffer overrun for buffer {buffer:?}, offset {offset}, width {width:?}"
            );
        }
    }
    use super::*;
    use crate::evaluator::types::read_typed_value;
    use crate::parser::ast::TypeKind;

    #[test]
    fn test_read_string_null_terminated() {
        let buffer = b"Hello\x00World";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_string_null_terminated_at_offset() {
        let buffer = b"Prefix\x00Hello\x00Suffix";
        let result = read_string(buffer, 7, None).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_string_with_max_length_shorter_than_null() {
        let buffer = b"VeryLongString\x00";
        let result = read_string(buffer, 0, Some(4)).unwrap();
        assert_eq!(result, Value::String("Very".to_string()));
    }

    #[test]
    fn test_read_string_with_max_length_longer_than_null() {
        let buffer = b"Short\x00LongerSuffix";
        let result = read_string(buffer, 0, Some(10)).unwrap();
        assert_eq!(result, Value::String("Short".to_string()));
    }

    #[test]
    fn test_read_string_no_null_terminator_with_max_length() {
        let buffer = b"NoNullTerminator";
        let result = read_string(buffer, 0, Some(6)).unwrap();
        assert_eq!(result, Value::String("NoNull".to_string()));
    }

    #[test]
    fn test_read_string_no_null_terminator_no_max_length() {
        let buffer = b"NoNullTerminator";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("NoNullTerminator".to_string()));
    }

    #[test]
    fn test_read_string_empty_string() {
        let buffer = b"\x00Hello";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_string_empty_buffer() {
        let buffer = b"";
        let result = read_string(buffer, 0, None);
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
    fn test_read_string_offset_out_of_bounds() {
        let buffer = b"Hello";
        let result = read_string(buffer, 10, None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 5
            }
        );
    }

    #[test]
    fn test_read_string_offset_at_buffer_end() {
        let buffer = b"Hello";
        let result = read_string(buffer, 5, None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 5,
                buffer_len: 5
            }
        );
    }

    #[test]
    fn test_read_string_max_length_zero() {
        let buffer = b"Hello\x00World";
        let result = read_string(buffer, 0, Some(0)).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_string_max_length_larger_than_buffer() {
        let buffer = b"Short";
        let result = read_string(buffer, 0, Some(100)).unwrap();
        assert_eq!(result, Value::String("Short".to_string()));
    }

    #[test]
    fn test_read_string_utf8_valid() {
        let buffer = b"Caf\xc3\xa9\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Café".to_string()));
    }

    #[test]
    fn test_read_string_utf8_invalid() {
        let buffer = b"Invalid\xff\xfe\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert!(matches!(result, Value::String(_)));
        if let Value::String(s) = result {
            assert!(s.starts_with("Invalid"));
            assert!(s.contains('\u{FFFD}'));
        }
    }

    #[test]
    fn test_read_string_binary_data() {
        let buffer = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00, 0x80, 0x90];
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_string_multiple_nulls() {
        let buffer = b"First\x00\x00Second\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("First".to_string()));

        let result = read_string(buffer, 6, None).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_string_ascii_control_characters() {
        let buffer = b"Hello\x09World\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Hello\tWorld".to_string()));
    }

    #[test]
    fn test_read_string_single_character() {
        let buffer = b"A\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("A".to_string()));
    }

    #[test]
    fn test_read_string_max_length_exact_match() {
        let buffer = b"Exact\x00";
        let result = read_string(buffer, 0, Some(5)).unwrap();
        assert_eq!(result, Value::String("Exact".to_string()));
    }

    #[test]
    fn test_read_string_at_buffer_boundary() {
        let buffer = b"Hello";
        let result = read_string(buffer, 4, Some(1)).unwrap();
        assert_eq!(result, Value::String("o".to_string()));
    }

    #[test]
    fn test_read_string_whitespace_handling() {
        let buffer = b"  Spaces  \x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("  Spaces  ".to_string()));
    }

    #[test]
    fn test_read_string_newline_characters() {
        let buffer = b"Line1\nLine2\r\n\x00";
        let result = read_string(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Line1\nLine2\r\n".to_string()));
    }

    #[test]
    fn test_read_string_consistency_with_typed_value() {
        let buffer = b"Test\x00String";
        let direct_result = read_string(buffer, 0, None).unwrap();

        let type_kind = TypeKind::String { max_length: None };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Test".to_string()));
    }

    #[test]
    fn test_read_string_consistency_with_max_length() {
        let buffer = b"LongString\x00";
        let direct_result = read_string(buffer, 0, Some(4)).unwrap();

        let type_kind = TypeKind::String {
            max_length: Some(4),
        };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Long".to_string()));
    }

    #[test]
    fn test_read_string_edge_case_combinations() {
        let test_cases = [
            (b"" as &[u8], 0, None, true),
            (b"\x00", 0, None, false),
            (b"A", 0, Some(0), false),
            (b"AB", 1, Some(1), false),
        ];

        for (buffer, offset, max_length, should_fail) in test_cases {
            let result = read_string(buffer, offset, max_length);

            if should_fail {
                assert!(
                    result.is_err(),
                    "Expected failure for buffer {buffer:?}, offset {offset}, max_length {max_length:?}"
                );
            } else {
                assert!(
                    result.is_ok(),
                    "Expected success for buffer {buffer:?}, offset {offset}, max_length {max_length:?}"
                );
            }
        }
    }

    // ============================================================
    // read_pstring tests
    // ============================================================

    #[test]
    fn test_read_pstring_basic() {
        let buffer = b"\x05Hello";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_pstring_at_offset() {
        let buffer = b"PREFIX\x03Foo";
        let result = read_pstring(buffer, 6, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Foo".to_string()));
    }

    #[test]
    fn test_read_pstring_empty_string() {
        let buffer = b"\x00trailing";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_pstring_single_char() {
        let buffer = b"\x01A";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("A".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_limits() {
        let buffer = b"\x0aHelloWorld";
        let result = read_pstring(buffer, 0, Some(5), PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_larger_than_prefix() {
        let buffer = b"\x03Foo";
        let result =
            read_pstring(buffer, 0, Some(100), PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Foo".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_zero() {
        let buffer = b"\x05Hello";
        let result = read_pstring(buffer, 0, Some(0), PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_pstring_buffer_overrun_offset_past_end() {
        let buffer = b"Hello";
        let result = read_pstring(buffer, 10, None, PStringLengthWidth::OneByte, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 10,
                buffer_len: 5,
            }
        );
    }

    #[test]
    fn test_read_pstring_buffer_overrun_empty_buffer() {
        let buffer = b"";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 0,
                buffer_len: 0,
            }
        );
    }

    #[test]
    fn test_read_pstring_buffer_overrun_length_exceeds_data() {
        // Length byte says 10 but only 3 bytes follow
        let buffer = b"\x0aFoo";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 11, // string_end = 1 (start) + 10 (length byte)
                buffer_len: 4,
            }
        );
    }

    #[test]
    fn test_read_pstring_length_byte_only() {
        // Buffer has length byte but no string data, and length > 0
        let buffer = b"\x05";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: 6, // string_end = 1 (start) + 5 (length byte)
                buffer_len: 1,
            }
        );
    }

    #[test]
    fn test_read_pstring_offset_overflow() {
        let buffer = b"\x05Hello";
        let result = read_pstring(buffer, usize::MAX, None, PStringLengthWidth::OneByte, false);
        assert_eq!(
            result.unwrap_err(),
            TypeReadError::BufferOverrun {
                offset: usize::MAX,
                buffer_len: buffer.len(),
            }
        );
    }

    #[test]
    fn test_read_pstring_max_length_caps_when_buffer_short() {
        // Length byte says 10, only 5 data bytes follow, but max_length=5 caps the read
        let buffer = b"\x0aHello";
        let result = read_pstring(buffer, 0, Some(5), PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_pstring_utf8_valid() {
        // "Café" in UTF-8 is 5 bytes: 43 61 66 c3 a9
        let buffer = b"\x05Caf\xc3\xa9";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("Café".to_string()));
    }

    #[test]
    fn test_read_pstring_utf8_invalid() {
        let buffer = b"\x03\xff\xfe\xfd";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        if let Value::String(s) = result {
            assert!(s.contains('\u{FFFD}'));
        } else {
            panic!("Expected Value::String");
        }
    }

    #[test]
    fn test_read_pstring_max_length_byte_value() {
        // Length byte = 255, with exactly 255 bytes of data
        let mut buffer = vec![0xFF];
        buffer.extend(std::iter::repeat_n(b'A', 255));
        let result = read_pstring(&buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();
        assert_eq!(result, Value::String("A".repeat(255)));
    }

    #[test]
    fn test_read_pstring_consistency_with_typed_value() {
        let buffer = b"\x04Test";
        let direct_result =
            read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, false).unwrap();

        let type_kind = TypeKind::PString {
            max_length: None,
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Test".to_string()));
    }

    #[test]
    fn test_read_pstring_consistency_with_max_length() {
        let buffer = b"\x0aLongString";
        let direct_result =
            read_pstring(buffer, 0, Some(4), PStringLengthWidth::OneByte, false).unwrap();

        let type_kind = TypeKind::PString {
            max_length: Some(4),
            length_width: PStringLengthWidth::OneByte,
            length_includes_itself: false,
        };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Long".to_string()));
    }

    #[test]
    fn test_read_pstring_big_endian() {
        let cases: &[(&[u8], PStringLengthWidth, &str)] = &[
            // 2-byte BE: length 3 in big-endian = [0x00, 0x03]
            (b"\x00\x03abc", PStringLengthWidth::TwoByteBE, "abc"),
            // 4-byte BE: length 5 in big-endian = [0x00, 0x00, 0x00, 0x05]
            (
                b"\x00\x00\x00\x05Hello",
                PStringLengthWidth::FourByteBE,
                "Hello",
            ),
        ];
        for &(buffer, width, expected) in cases {
            let result = read_pstring(buffer, 0, None, width, false);
            assert_eq!(
                result.unwrap(),
                Value::String(expected.to_string()),
                "Failed for width {width:?}"
            );
        }
    }

    #[test]
    fn test_read_pstring_length_includes_itself() {
        let cases: &[(&[u8], PStringLengthWidth, bool, &str)] = &[
            // 1-byte /J: stored length=4, minus 1 byte prefix = 3 bytes of data
            (b"\x04abc", PStringLengthWidth::OneByte, true, "abc"),
            // 2-byte LE /J: stored length=5, minus 2 byte prefix = 3 bytes of data
            (b"\x05\x00abc", PStringLengthWidth::TwoByteLE, true, "abc"),
            // 2-byte BE /J: stored length=7, minus 2 byte prefix = 5 bytes of data
            (
                b"\x00\x07Hello",
                PStringLengthWidth::TwoByteBE,
                true,
                "Hello",
            ),
            // 4-byte LE /J: stored length=9, minus 4 byte prefix = 5 bytes of data
            (
                b"\x09\x00\x00\x00Hello",
                PStringLengthWidth::FourByteLE,
                true,
                "Hello",
            ),
            // 4-byte BE /J: stored length=7, minus 4 byte prefix = 3 bytes of data
            (
                b"\x00\x00\x00\x07abc",
                PStringLengthWidth::FourByteBE,
                true,
                "abc",
            ),
        ];
        for &(buffer, width, includes_itself, expected) in cases {
            let result = read_pstring(buffer, 0, None, width, includes_itself);
            assert_eq!(
                result.unwrap(),
                Value::String(expected.to_string()),
                "Failed for width {width:?}, includes_itself={includes_itself}"
            );
        }
    }

    #[test]
    fn test_read_pstring_j_flag_length_equals_prefix_width() {
        // /J where length exactly equals prefix width -> empty string
        let buffer = b"\x01";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::OneByte, true).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_pstring_j_flag_length_less_than_prefix_width() {
        // /J where length < prefix width -> InvalidPStringLength error
        // LE bytes [0x01, 0x00] = stored length 1, but prefix width is 2
        let buffer = b"\x01\x00xx";
        let result = read_pstring(buffer, 0, None, PStringLengthWidth::TwoByteLE, true);
        assert!(
            matches!(
                result,
                Err(TypeReadError::InvalidPStringLength {
                    stored_length: 1,
                    prefix_width: 2
                })
            ),
            "Expected InvalidPStringLength, got {result:?}"
        );
    }

    #[test]
    fn test_read_pstring_j_flag_with_max_length() {
        // /J + max_length interaction: subtract prefix width first, then cap
        // stored length=9, width=4, /J gives 5, max_length=3 caps to 3
        let buffer = b"\x09\x00\x00\x00Hello";
        let result = read_pstring(buffer, 0, Some(3), PStringLengthWidth::FourByteLE, true);
        assert_eq!(result.unwrap(), Value::String("Hel".to_string()));
    }

    #[test]
    fn test_read_pstring_j_flag_zero_length_all_widths() {
        // /J where stored length equals prefix width -> empty string
        let cases: &[(&[u8], PStringLengthWidth)] = &[
            (b"\x01", PStringLengthWidth::OneByte),
            (b"\x00\x02", PStringLengthWidth::TwoByteBE),
            (b"\x02\x00", PStringLengthWidth::TwoByteLE),
            (b"\x00\x00\x00\x04", PStringLengthWidth::FourByteBE),
            (b"\x04\x00\x00\x00", PStringLengthWidth::FourByteLE),
        ];
        for &(buffer, width) in cases {
            let result = read_pstring(buffer, 0, None, width, true);
            assert_eq!(
                result.unwrap(),
                Value::String(String::new()),
                "Expected empty string for /J with width {width:?}"
            );
        }
    }
}
