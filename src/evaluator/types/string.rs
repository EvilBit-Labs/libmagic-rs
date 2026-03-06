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
}
