// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::TypeReadError;
use crate::parser::ast::{Endianness, PStringLengthWidth, Value};

/// Maximum number of UCS-2 code units consumed by a single `string16` read.
///
/// Caps the worst-case scan so a buffer with no `0x00 0x00` terminator does
/// not allocate megabytes of decoded `String`. Mirrors libmagic's implicit
/// cap for `lestring16` / `bestring16` (`MAXstring` in `softmagic.c`).
const STRING16_MAX_UNITS: usize = 8192;

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
/// ```ignore
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
// Slicing is invariant-safe: `offset < buffer.len()` is checked at entry;
// `search_len` and `read_length` are clamped by `min`/`memchr` results.
#[allow(clippy::indexing_slicing)]
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
    let string_value = bytes_to_string_fast(string_bytes);

    Ok(Value::String(string_value))
}

/// Read exactly `length` bytes from the buffer at `offset`, with NO NUL
/// truncation. Used for libmagic-compatible `string PATTERN` comparison
/// where the magic value's full byte length must be compared byte-for-byte
/// against the file (including any embedded NULs in the pattern).
///
/// Differs from [`read_string`]: that function stops at the first NUL it
/// finds in the buffer, which is wrong for patterns that legitimately
/// contain NUL bytes (e.g., `0 string PNCIHISK\0 ...`). magic(5)'s
/// comparison semantic is "do the first len(pattern) bytes of the file
/// equal these bytes?", regardless of whether either side contains NULs.
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` when `offset + length` exceeds
/// the buffer.
pub fn read_string_exact(
    buffer: &[u8],
    offset: usize,
    length: usize,
) -> Result<Value, TypeReadError> {
    let end = offset
        .checked_add(length)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;
    let slice = buffer
        .get(offset..end)
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })?;
    // This path exists for BYTE-EXACT `string PATTERN` comparison (GOTCHAS
    // S6.4), so it must not corrupt the read with lossy UTF-8 replacement.
    // When the slice is valid UTF-8, return `Value::String` (so `%s` output on
    // the matched value renders normally). When it is NOT -- e.g. a rule with a
    // high-byte value like gzip's `0 string \037\213` (bytes 0x1f 0x8b, where
    // 0x8b is invalid UTF-8) -- return the RAW bytes as `Value::Bytes` instead.
    // A lossy `String` decode would turn 0x8b into U+FFFD (3 bytes 0xEF BF BD),
    // which never equals the raw-byte pattern, so the rule would silently never
    // match. `apply_equal` / `compare_values` compare `Bytes` and `String` by
    // underlying byte sequence (GOTCHAS S2.3), so either variant compares
    // correctly against a `String` or `Bytes` pattern operand.
    match std::str::from_utf8(slice) {
        Ok(valid) => Ok(Value::String(valid.to_string())),
        Err(_) => Ok(Value::Bytes(slice.to_vec())),
    }
}

/// Compare a magic-rule `pattern` against `buffer[offset..]` using libmagic
/// `string`-flag semantics.
///
/// Returns `Some(consumed_bytes)` when the pattern matches under the given
/// flags, where `consumed_bytes` is the count of *buffer* bytes consumed by
/// the match (which can exceed `pattern.len()` when `/w` or `/W` allowed the
/// file to have additional whitespace). Returns `None` on miss.
///
/// **Buffer bytes consumed is load-bearing for relative-offset child rules.**
/// `&N` offsets resolve against the previous match's end position
/// (GOTCHAS S3.8). When `/w` or `/W` lets the file consume more bytes than
/// the pattern, that count is what advances the anchor, NOT `pattern.len()`.
/// Returning the consumed count here keeps `bytes_consumed_with_pattern`
/// honest without re-scanning the buffer at anchor-advance time.
///
/// **Trim is applied by the caller, not here.** `read_pattern_match` (in
/// the parent module) trims the pattern before invoking this function
/// when `flags.trim` is set; this function ignores the `trim` field on
/// the assumption that the caller has already normalized the pattern.
/// Likewise `flags.text_test` and `flags.bin_test` are MIME-output hints
/// with no effect on comparison and are not consulted here.
///
/// **`/c` vs `/C` is asymmetric and pattern-controlled** -- see [`StringFlags`]
/// and GOTCHAS S6.5 for the canonical contract. This function implements the
/// libmagic `src/softmagic.c` contract using `u8::to_ascii_lowercase` /
/// `to_ascii_uppercase` for the fold so the comparison stays ASCII-only and
/// locale-independent.
///
/// # Arguments
///
/// * `pattern` -- The magic rule's pattern bytes, already trimmed when
///   `/T` was set (the caller is responsible for the trim).
/// * `buffer` -- The file slice (full buffer; this function indexes from
///   `offset` internally).
/// * `offset` -- Absolute byte offset in `buffer` to start matching.
/// * `flags` -- The parsed flag set from the magic rule.
///
/// # Returns
///
/// `Some(consumed_bytes)` on match, `None` on miss or when `offset` is past
/// the end of the buffer.
///
/// [`StringFlags`]: crate::parser::ast::StringFlags
#[must_use]
// Indexing is invariant-safe: `pattern[a]` is guarded by the
// `a < pattern.len()` loop condition.
#[allow(clippy::indexing_slicing)]
pub fn compare_string_with_flags(
    pattern: &[u8],
    buffer: &[u8],
    offset: usize,
    flags: crate::parser::ast::StringFlags,
) -> Option<usize> {
    let file = buffer.get(offset..)?;
    let mut a = 0usize; // pattern cursor
    let mut b = 0usize; // file cursor

    while a < pattern.len() {
        let pc = pattern[a];

        // Whitespace flags fire on pattern whitespace and consume runs in
        // the file independently of the case-fold flags.
        if flags.compact_whitespace && pc.is_ascii_whitespace() {
            // `/W` -- file MUST have at least one whitespace byte here;
            // additional whitespace is consumed greedily.
            let fc = *file.get(b)?;
            if !fc.is_ascii_whitespace() {
                return None;
            }
            a += 1;
            b += 1;
            while file.get(b).is_some_and(u8::is_ascii_whitespace) {
                b += 1;
            }
            continue;
        }
        if flags.compact_optional_whitespace && pc.is_ascii_whitespace() {
            // `/w` -- file MAY have zero or more whitespace bytes here.
            a += 1;
            while file.get(b).is_some_and(u8::is_ascii_whitespace) {
                b += 1;
            }
            continue;
        }

        // Non-whitespace path: need a byte in the file to compare.
        let fc = *file.get(b)?;

        // Case-fold direction is controlled by the pattern character per
        // libmagic softmagic.c. Lowercase pattern + `/c` => fold file to
        // lower; uppercase pattern + `/C` => fold file to upper. Anything
        // else is literal byte comparison (including mixed-case patterns
        // where the "wrong" case position stays literal).
        if flags.ignore_lowercase && pc.is_ascii_lowercase() {
            if fc.to_ascii_lowercase() != pc {
                return None;
            }
        } else if flags.ignore_uppercase && pc.is_ascii_uppercase() {
            if fc.to_ascii_uppercase() != pc {
                return None;
            }
        } else if pc != fc {
            return None;
        }

        a += 1;
        b += 1;
    }

    // Post-match word-boundary check for `/f`. The byte after the matched
    // region must be either end-of-file (returned by `get(b).is_none()`)
    // or a non-word character. "Word char" = ASCII alphanumeric or `_`,
    // matching libmagic `softmagic.c`'s `isalpha || isdigit || == '_'`.
    if flags.full_word
        && let Some(&boundary) = file.get(b)
        && (boundary.is_ascii_alphanumeric() || boundary == b'_')
    {
        return None;
    }

    Some(b)
}

/// Convert bytes to an owned `String`, avoiding a double allocation on the
/// common valid-UTF-8 path.
///
/// Both branches produce an owned `String` (one allocation), but differ in
/// cost: `String::from(valid_str)` is a single `memcpy`, whereas
/// `from_utf8_lossy(...).into_owned()` must scan for and insert replacement
/// characters. The fast path skips the lossy scan entirely.
#[inline]
fn bytes_to_string_fast(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(valid) => String::from(valid),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Reads a UCS-2 string from the buffer with the given endianness.
///
/// Each character is encoded as a 16-bit code unit; the reader consumes
/// pairs of bytes until it encounters a U+0000 terminator (the 2-byte
/// sequence `0x00 0x00`), runs out of buffer, or hits the
/// `STRING16_MAX_UNITS` cap. A trailing odd byte (when the remaining
/// buffer length is not even) is ignored.
///
/// Surrogate-pair (D800-DFFF) code units are emitted as the Unicode
/// replacement character (U+FFFD) -- libmagic's UCS-2 path does not
/// resolve surrogates, and emitting a placeholder keeps the comparison
/// length stable for ASCII rules.
///
/// # Arguments
///
/// * `buffer` -- The byte buffer to read from.
/// * `offset` -- Absolute byte offset to start reading at.
/// * `endian` -- Byte order of the 16-bit code units.
///
/// # Returns
///
/// `Ok(Value::String(decoded))` on success. The decoded string is empty
/// when the offset is exactly at the start of a NUL terminator.
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` when `offset >= buffer.len()`.
///
/// # Examples
///
/// ```ignore
/// use libmagic_rs::evaluator::types::read_string16;
/// use libmagic_rs::parser::ast::{Endianness, Value};
///
/// // "MZ" in UCS-2 little-endian, NUL-terminated
/// let buffer = b"M\x00Z\x00\x00\x00";
/// let result = read_string16(buffer, 0, Endianness::Little).unwrap();
/// assert_eq!(result, Value::String("MZ".to_string()));
///
/// // "MZ" in UCS-2 big-endian, NUL-terminated
/// let buffer = b"\x00M\x00Z\x00\x00";
/// let result = read_string16(buffer, 0, Endianness::Big).unwrap();
/// assert_eq!(result, Value::String("MZ".to_string()));
/// ```
// Indexing is invariant-safe: `offset < buffer.len()` is checked at entry
// and `chunks_exact(2)` yields exactly 2-byte slices.
#[allow(clippy::indexing_slicing)]
pub fn read_string16(
    buffer: &[u8],
    offset: usize,
    endian: Endianness,
) -> Result<Value, TypeReadError> {
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }

    let remaining = &buffer[offset..];
    let mut decoded = String::new();
    let mut chunks = remaining.chunks_exact(2);
    let mut units_read = 0usize;
    for pair in chunks.by_ref() {
        if units_read >= STRING16_MAX_UNITS {
            break;
        }
        let code_unit = match endian {
            Endianness::Little | Endianness::Native => u16::from_le_bytes([pair[0], pair[1]]),
            Endianness::Big => u16::from_be_bytes([pair[0], pair[1]]),
        };
        if code_unit == 0 {
            break;
        }
        // UCS-2 only covers the BMP; surrogate halves are not a valid
        // standalone code point, so substitute U+FFFD.
        let ch = char::from_u32(u32::from(code_unit)).unwrap_or('\u{FFFD}');
        decoded.push(ch);
        units_read = units_read.saturating_add(1);
    }

    Ok(Value::String(decoded))
}

/// Compute the bytes consumed by a successful `read_string16` call.
///
/// Mirrors [`read_string16`]: walks 16-bit code units and stops at the
/// terminator, the buffer end, or the [`STRING16_MAX_UNITS`] cap. Returns
/// the byte count (always a multiple of 2, except when a NUL terminator is
/// included -- in which case the count is still even because the
/// terminator itself is two bytes). A successful read with zero bytes
/// returns `0`.
// Indexing is invariant-safe: `chunks_exact(2)` yields exactly 2-byte
// slices.
#[allow(clippy::indexing_slicing)]
pub(crate) fn string16_bytes_consumed(buffer: &[u8], offset: usize, endian: Endianness) -> usize {
    let Some(remaining) = buffer.get(offset..) else {
        return 0;
    };
    let mut units_read = 0usize;
    let mut consumed = 0usize;
    for pair in remaining.chunks_exact(2) {
        if units_read >= STRING16_MAX_UNITS {
            return consumed;
        }
        let code_unit = match endian {
            Endianness::Little | Endianness::Native => u16::from_le_bytes([pair[0], pair[1]]),
            Endianness::Big => u16::from_be_bytes([pair[0], pair[1]]),
        };
        if code_unit == 0 {
            // Include the NUL terminator in the consumed byte count so the
            // relative-offset anchor advances past it (matching the
            // string/pstring conventions).
            return consumed.saturating_add(2);
        }
        consumed = consumed.saturating_add(2);
        units_read = units_read.saturating_add(1);
    }
    consumed
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
/// ```ignore
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
// Slicing is invariant-safe: `string_start..string_end` is validated
// against `buffer.len()` before the slice.
#[allow(clippy::indexing_slicing)]
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
        PStringLengthWidth::OneByte => {
            usize::from(*len_bytes.first().ok_or(TypeReadError::BufferOverrun {
                offset,
                buffer_len: buffer.len(),
            })?)
        }
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
    let string_value = bytes_to_string_fast(string_bytes);
    Ok(Value::String(string_value))
}

#[cfg(test)]
mod tests {
    // Restriction lints without an allow-*-in-tests config option;
    // non-ASCII test data exercises the UCS-2/UTF-8 handling paths.
    #![allow(clippy::non_ascii_literal)]

    use super::*;
    use crate::parser::ast::{PStringLengthWidth, StringFlags, TypeKind};
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
    use crate::evaluator::types::read_typed_value;

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

        let type_kind = TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        };
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
            flags: StringFlags::default(),
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

    // ========================================================================
    // read_string16 tests
    // ========================================================================

    #[test]
    fn test_read_string16_le_ascii() {
        // "NTLDR" in UCS-2 little-endian, NUL-terminated
        let buffer = b"N\x00T\x00L\x00D\x00R\x00\x00\x00";
        let result = read_string16(buffer, 0, Endianness::Little).unwrap();
        assert_eq!(result, Value::String("NTLDR".to_string()));
    }

    #[test]
    fn test_read_string16_be_ascii() {
        // "NTLDR" in UCS-2 big-endian, NUL-terminated
        let buffer = b"\x00N\x00T\x00L\x00D\x00R\x00\x00";
        let result = read_string16(buffer, 0, Endianness::Big).unwrap();
        assert_eq!(result, Value::String("NTLDR".to_string()));
    }

    #[test]
    fn test_read_string16_no_terminator_runs_to_buffer_end() {
        // 3 LE-encoded UCS-2 chars and no terminator -- read all of them.
        let buffer = b"A\x00B\x00C\x00";
        let result = read_string16(buffer, 0, Endianness::Little).unwrap();
        assert_eq!(result, Value::String("ABC".to_string()));
    }

    #[test]
    fn test_read_string16_empty_at_terminator() {
        // Offset directly on a NUL pair -> zero-length string.
        let buffer = b"\x00\x00rest";
        let result = read_string16(buffer, 0, Endianness::Little).unwrap();
        assert_eq!(result, Value::String(String::new()));
    }

    #[test]
    fn test_read_string16_offset_past_end() {
        let buffer = b"\x00\x00";
        let err = read_string16(buffer, 5, Endianness::Little).unwrap_err();
        assert!(matches!(err, TypeReadError::BufferOverrun { .. }));
    }

    #[test]
    fn test_read_string16_trailing_odd_byte_is_ignored() {
        // 4 LE bytes for "AB" + a single trailing byte that cannot form a
        // 16-bit unit: the reader stops cleanly without panicking.
        let buffer = b"A\x00B\x00\x42";
        let result = read_string16(buffer, 0, Endianness::Little).unwrap();
        assert_eq!(result, Value::String("AB".to_string()));
    }

    #[test]
    fn test_read_string16_le_non_ascii() {
        // Greek capital alpha (U+0391) in UCS-2 LE: 0x91 0x03
        let buffer = b"\x91\x03\x00\x00";
        let result = read_string16(buffer, 0, Endianness::Little).unwrap();
        assert_eq!(result, Value::String("\u{0391}".to_string()));
    }

    #[test]
    fn test_read_string16_be_surrogate_replaced_with_fffd() {
        // 0xD800 is a high surrogate -- not a valid standalone scalar value
        // in UCS-2 / Rust char. The reader must substitute U+FFFD instead
        // of crashing.
        let buffer = b"\xD8\x00\x00\x00";
        let result = read_string16(buffer, 0, Endianness::Big).unwrap();
        assert_eq!(result, Value::String("\u{FFFD}".to_string()));
    }

    #[test]
    fn test_string16_bytes_consumed_matches_read() {
        // 3 LE chars + NUL terminator -> 8 bytes consumed (incl. terminator).
        let buffer = b"A\x00B\x00C\x00\x00\x00trail";
        assert_eq!(string16_bytes_consumed(buffer, 0, Endianness::Little), 8);

        // Same 3 chars, no terminator -> 6 bytes consumed (chars only).
        let buffer = b"A\x00B\x00C\x00";
        assert_eq!(string16_bytes_consumed(buffer, 0, Endianness::Little), 6);

        // BE encoding: same expected lengths.
        let buffer = b"\x00A\x00B\x00\x00rest";
        assert_eq!(string16_bytes_consumed(buffer, 0, Endianness::Big), 6);
    }

    // -- compare_string_with_flags --------------------------------------
    //
    // Each subsection pins one flag (or combination) against the libmagic
    // contract documented in softmagic.c. The asymmetric /c vs /C cases
    // (GOTCHAS S6.5) get extra coverage because that is the most likely
    // future regression vector.

    #[test]
    fn test_compare_string_with_flags_no_flags_matches_exact_bytes() {
        let flags = StringFlags::default();
        assert_eq!(
            compare_string_with_flags(b"FOO", b"FOObar", 0, flags),
            Some(3),
            "exact byte match consumes pattern.len() bytes"
        );
        assert_eq!(
            compare_string_with_flags(b"FOO", b"foobar", 0, flags),
            None,
            "case mismatch without /c fails"
        );
    }

    #[test]
    fn test_compare_string_with_flags_c_lowercase_pattern() {
        // /c with a lowercase pattern matches any case in the file.
        let flags = StringFlags::default().with_ignore_lowercase(true);
        assert_eq!(
            compare_string_with_flags(b"foo", b"FOObar", 0, flags),
            Some(3)
        );
        assert_eq!(
            compare_string_with_flags(b"foo", b"FoObar", 0, flags),
            Some(3)
        );
        assert_eq!(
            compare_string_with_flags(b"foo", b"foobar", 0, flags),
            Some(3)
        );
    }

    #[test]
    fn test_compare_string_with_flags_c_mixed_case_pattern() {
        // Mixed-case pattern with /c: lowercase positions fold; uppercase
        // positions are literal. This is the asymmetry from GOTCHAS S6.5.
        let flags = StringFlags::default().with_ignore_lowercase(true);
        // `FoO`: 'F' literal (must match 'F'); 'o' folds (any case); 'O'
        // literal (must match 'O').
        assert_eq!(
            compare_string_with_flags(b"FoO", b"FoObar", 0, flags),
            Some(3),
            "FoO should match FoO"
        );
        assert_eq!(
            compare_string_with_flags(b"FoO", b"FOObar", 0, flags),
            Some(3),
            "FoO should match FOO (middle 'o' folds; F and O are literal)"
        );
        assert_eq!(
            compare_string_with_flags(b"FoO", b"fOObar", 0, flags),
            None,
            "FoO should NOT match fOO -- uppercase 'F' position is literal"
        );
        assert_eq!(
            compare_string_with_flags(b"FoO", b"Foobar", 0, flags),
            None,
            "FoO should NOT match Foo -- uppercase 'O' position is literal"
        );
    }

    #[test]
    fn test_compare_string_with_flags_capital_c_uppercase_pattern() {
        // /C with an uppercase pattern matches any case in the file.
        let flags = StringFlags::default().with_ignore_uppercase(true);
        assert_eq!(
            compare_string_with_flags(b"FOO", b"foobar", 0, flags),
            Some(3)
        );
        assert_eq!(
            compare_string_with_flags(b"FOO", b"FooBar", 0, flags),
            Some(3)
        );
        // Lowercase pattern with /C should be literal -- /C only fires on
        // uppercase pattern chars.
        assert_eq!(
            compare_string_with_flags(b"foo", b"FOObar", 0, flags),
            None,
            "/C does not fire on lowercase pattern chars"
        );
    }

    #[test]
    fn test_compare_string_with_flags_w_optional_whitespace() {
        // /w: pattern whitespace allows zero or more file whitespace.
        let flags = StringFlags::default().with_compact_optional_whitespace(true);
        assert_eq!(
            compare_string_with_flags(b"a b", b"ab", 0, flags),
            Some(2),
            "zero file whitespace accepted"
        );
        assert_eq!(
            compare_string_with_flags(b"a b", b"a b", 0, flags),
            Some(3),
            "one file whitespace consumed"
        );
        assert_eq!(
            compare_string_with_flags(b"a b", b"a    b", 0, flags),
            Some(6),
            "multiple file whitespace consumed -- consumed_bytes > pattern.len()"
        );
        assert_eq!(
            compare_string_with_flags(b"a b", b"a\tb", 0, flags),
            Some(3),
            "tab is ASCII whitespace"
        );
    }

    #[test]
    fn test_compare_string_with_flags_capital_w_compact_whitespace() {
        // /W: pattern whitespace REQUIRES at least one file whitespace.
        let flags = StringFlags::default().with_compact_whitespace(true);
        assert_eq!(
            compare_string_with_flags(b"a b", b"ab", 0, flags),
            None,
            "no file whitespace => no match under /W"
        );
        assert_eq!(
            compare_string_with_flags(b"a b", b"a b", 0, flags),
            Some(3),
            "exactly one space"
        );
        assert_eq!(
            compare_string_with_flags(b"a b", b"a    b", 0, flags),
            Some(6),
            "multiple spaces collapsed to single match"
        );
    }

    #[test]
    fn test_compare_string_with_flags_combined_cw() {
        // /cw: lowercase pattern + whitespace-optional applied together.
        let flags = StringFlags::default()
            .with_ignore_lowercase(true)
            .with_compact_optional_whitespace(true);
        assert_eq!(
            compare_string_with_flags(b"foo bar", b"FOOBAR", 0, flags),
            Some(6),
            "case folded AND zero whitespace consumed"
        );
        assert_eq!(
            compare_string_with_flags(b"foo bar", b"foo   bar", 0, flags),
            Some(9),
            "case folded (already lower) + 3 whitespace consumed"
        );
    }

    #[test]
    fn test_compare_string_with_flags_full_word_boundary() {
        // /f: post-match must be EOF or non-word character.
        let flags = StringFlags::default().with_full_word(true);
        assert_eq!(
            compare_string_with_flags(b"int", b"int ", 0, flags),
            Some(3),
            "space boundary OK"
        );
        assert_eq!(
            compare_string_with_flags(b"int", b"int.", 0, flags),
            Some(3),
            "punctuation boundary OK"
        );
        assert_eq!(
            compare_string_with_flags(b"int", b"int", 0, flags),
            Some(3),
            "EOF boundary OK"
        );
        assert_eq!(
            compare_string_with_flags(b"int", b"integer", 0, flags),
            None,
            "followed by alphanumeric => fail"
        );
        assert_eq!(
            compare_string_with_flags(b"int", b"int_x", 0, flags),
            None,
            "underscore is a word char per libmagic contract"
        );
    }

    #[test]
    fn test_compare_string_with_flags_pattern_longer_than_buffer() {
        // No flags: ran-out-of-file produces None, not panic.
        let flags = StringFlags::default();
        assert_eq!(
            compare_string_with_flags(b"FOO", b"FO", 0, flags),
            None,
            "early termination on EOF"
        );
    }

    #[test]
    fn test_compare_string_with_flags_empty_pattern_always_matches() {
        let flags = StringFlags::default();
        // Empty pattern => zero consumed, vacuously true.
        assert_eq!(
            compare_string_with_flags(b"", b"anything", 0, flags),
            Some(0)
        );
        // Even an empty buffer matches an empty pattern.
        assert_eq!(compare_string_with_flags(b"", b"", 0, flags), Some(0));
    }

    #[test]
    fn test_compare_string_with_flags_offset_past_buffer_end() {
        // Out-of-range offset must not panic; returns None.
        let flags = StringFlags::default();
        assert_eq!(compare_string_with_flags(b"FOO", b"abc", 10, flags), None);
        assert_eq!(compare_string_with_flags(b"FOO", b"", 0, flags), None);
    }

    #[test]
    fn test_compare_string_with_flags_consumed_bytes_drives_anchor() {
        // This is the load-bearing contract from the U4 plan: when /W
        // consumes more file bytes than pattern bytes, the returned count
        // is what relative-offset child rules use to advance the anchor.
        // The regression risk is returning `pattern.len()` instead.
        let flags = StringFlags::default().with_compact_whitespace(true);
        // Pattern "a b" (3 bytes) against file "a    b" (6 bytes) ->
        // anchor must advance by 6, NOT 3.
        assert_eq!(
            compare_string_with_flags(b"a b", b"a    b", 0, flags),
            Some(6),
            "anchor-advance contract: consumed_bytes reflects file consumption"
        );
    }
}
