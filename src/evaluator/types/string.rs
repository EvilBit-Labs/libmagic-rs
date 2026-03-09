// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::TypeReadError;
use crate::parser::ast::Value;

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
/// Pascal strings store the length as the first byte (0-255), followed by
/// that many bytes of string data. Unlike C strings, they are not null-terminated.
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
/// The length byte is validated against the remaining buffer size before reading.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_pstring;
/// use libmagic_rs::parser::ast::Value;
///
/// let buffer = b"\x05Hello";
/// let result = read_pstring(buffer, 0, None).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
///
/// let buffer = b"\x0aHelloWorld";
/// let result = read_pstring(buffer, 0, Some(5)).unwrap();
/// assert_eq!(result, Value::String("Hello".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` if:
/// - The offset is beyond buffer bounds (cannot read the length byte)
/// - The string data (length byte value) extends beyond the buffer
pub fn read_pstring(
    buffer: &[u8],
    offset: usize,
    max_length: Option<usize>,
) -> Result<Value, TypeReadError> {
    // Check if we can read the length byte
    let length_byte = *buffer.get(offset).ok_or(TypeReadError::BufferOverrun {
        offset,
        buffer_len: buffer.len(),
    })?;

    let string_length = usize::from(length_byte);

    // Apply max_length limit if specified
    let actual_length = if let Some(max_len) = max_length {
        std::cmp::min(string_length, max_len)
    } else {
        string_length
    };

    // Check if we have enough bytes for the string data
    let string_start = offset.checked_add(1).ok_or(TypeReadError::BufferOverrun {
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
        let result = read_pstring(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_pstring_at_offset() {
        let buffer = b"PREFIX\x03Foo";
        let result = read_pstring(buffer, 6, None).unwrap();
        assert_eq!(result, Value::String("Foo".to_string()));
    }

    #[test]
    fn test_read_pstring_empty_string() {
        let buffer = b"\x00trailing";
        let result = read_pstring(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_pstring_single_char() {
        let buffer = b"\x01A";
        let result = read_pstring(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("A".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_limits() {
        let buffer = b"\x0aHelloWorld";
        let result = read_pstring(buffer, 0, Some(5)).unwrap();
        assert_eq!(result, Value::String("Hello".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_larger_than_prefix() {
        let buffer = b"\x03Foo";
        let result = read_pstring(buffer, 0, Some(100)).unwrap();
        assert_eq!(result, Value::String("Foo".to_string()));
    }

    #[test]
    fn test_read_pstring_max_length_zero() {
        let buffer = b"\x05Hello";
        let result = read_pstring(buffer, 0, Some(0)).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_pstring_buffer_overrun_offset_past_end() {
        let buffer = b"Hello";
        let result = read_pstring(buffer, 10, None);
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
        let result = read_pstring(buffer, 0, None);
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
        let result = read_pstring(buffer, 0, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_pstring_length_byte_only() {
        // Buffer has length byte but no string data, and length > 0
        let buffer = b"\x05";
        let result = read_pstring(buffer, 0, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_pstring_utf8_valid() {
        // "Café" in UTF-8 is 5 bytes: 43 61 66 c3 a9
        let buffer = b"\x05Caf\xc3\xa9";
        let result = read_pstring(buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("Café".to_string()));
    }

    #[test]
    fn test_read_pstring_utf8_invalid() {
        let buffer = b"\x03\xff\xfe\xfd";
        let result = read_pstring(buffer, 0, None).unwrap();
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
        let result = read_pstring(&buffer, 0, None).unwrap();
        assert_eq!(result, Value::String("A".repeat(255)));
    }

    #[test]
    fn test_read_pstring_consistency_with_typed_value() {
        let buffer = b"\x04Test";
        let direct_result = read_pstring(buffer, 0, None).unwrap();

        let type_kind = TypeKind::PString { max_length: None };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Test".to_string()));
    }

    #[test]
    fn test_read_pstring_consistency_with_max_length() {
        let buffer = b"\x0aLongString";
        let direct_result = read_pstring(buffer, 0, Some(4)).unwrap();

        let type_kind = TypeKind::PString {
            max_length: Some(4),
        };
        let typed_result = read_typed_value(buffer, 0, &type_kind).unwrap();

        assert_eq!(direct_result, typed_result);
        assert_eq!(typed_result, Value::String("Long".to_string()));
    }
}
