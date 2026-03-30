// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Indirect offset resolution
//!
//! Indirect offsets read a pointer value from the file at a base offset,
//! then use that value (with optional adjustment) as the final offset.

use crate::LibmagicError;
use crate::error::EvaluationError;
use crate::evaluator::types::{TypeReadError, read_byte, read_long, read_quad, read_short};
use crate::parser::ast::{Endianness, OffsetSpec, TypeKind, Value};

use super::{map_offset_error, resolve_absolute_offset};

/// Resolve an indirect offset specification.
///
/// Indirect offsets dereference a pointer stored in the file buffer:
/// 1. Resolve `base_offset` to an absolute position (supports negative/from-end).
/// 2. Read a numeric pointer value at that position using `pointer_type` and `endian`.
/// 3. Apply `adjustment` with checked arithmetic.
/// 4. Validate the final offset against `buffer.len()`.
///
/// # Arguments
///
/// * `spec` - Must be `OffsetSpec::Indirect { .. }`
/// * `buffer` - The file buffer to read from
///
/// # Errors
///
/// * `EvaluationError::InvalidOffset` - If `base_offset` is out of bounds or arithmetic overflows
/// * `EvaluationError::BufferOverrun` - If the pointer read or final offset exceeds buffer bounds
/// * `EvaluationError::UnsupportedType` - If `pointer_type` is not a numeric type
pub fn resolve_indirect_offset(spec: &OffsetSpec, buffer: &[u8]) -> Result<usize, LibmagicError> {
    let (base_offset, pointer_type, adjustment, endian) = match spec {
        OffsetSpec::Indirect {
            base_offset,
            pointer_type,
            adjustment,
            endian,
        } => (*base_offset, pointer_type, *adjustment, *endian),
        _ => {
            return Err(LibmagicError::EvaluationError(
                EvaluationError::internal_error(
                    "resolve_indirect_offset called with non-indirect spec",
                ),
            ));
        }
    };

    // Step 1: Resolve base_offset to an absolute position
    let abs_base = resolve_absolute_offset(base_offset, buffer)
        .map_err(|e| map_offset_error(&e, base_offset))?;

    // Step 2: Read pointer value using the appropriate numeric reader
    let pointer_value = read_pointer(buffer, abs_base, pointer_type, endian)?;

    // Step 3: Apply adjustment with checked arithmetic
    let final_offset = apply_adjustment(pointer_value, adjustment)?;

    // Step 4: Validate final offset against buffer length
    if final_offset >= buffer.len() {
        return Err(LibmagicError::EvaluationError(
            EvaluationError::BufferOverrun {
                offset: final_offset,
            },
        ));
    }

    Ok(final_offset)
}

/// Read a pointer value from the buffer and extract it as a raw `u64`.
fn read_pointer(
    buffer: &[u8],
    offset: usize,
    pointer_type: &TypeKind,
    endian: Endianness,
) -> Result<u64, LibmagicError> {
    let value = match pointer_type {
        TypeKind::Byte { signed } => read_byte(buffer, offset, *signed),
        TypeKind::Short { signed, .. } => read_short(buffer, offset, endian, *signed),
        TypeKind::Long { signed, .. } => read_long(buffer, offset, endian, *signed),
        TypeKind::Quad { signed, .. } => read_quad(buffer, offset, endian, *signed),
        _ => {
            return Err(LibmagicError::EvaluationError(
                EvaluationError::unsupported_type(format!(
                    "Indirect offset pointer type not supported: {pointer_type:?}"
                )),
            ));
        }
    }
    .map_err(|e| map_type_read_error(e, offset))?;

    extract_raw_unsigned(&value)
}

/// Extract a raw unsigned integer from a `Value`, converting signed values.
fn extract_raw_unsigned(value: &Value) -> Result<u64, LibmagicError> {
    match value {
        Value::Uint(v) => Ok(*v),
        #[allow(clippy::cast_sign_loss)]
        Value::Int(v) => Ok(*v as u64),
        _ => Err(LibmagicError::EvaluationError(
            EvaluationError::internal_error("Pointer read returned non-integer value"),
        )),
    }
}

/// Apply an `i64` adjustment to a `u64` pointer value with checked arithmetic.
fn apply_adjustment(pointer: u64, adjustment: i64) -> Result<usize, LibmagicError> {
    let adjusted = if adjustment >= 0 {
        #[allow(clippy::cast_sign_loss)]
        pointer
            .checked_add(adjustment as u64)
            .ok_or_else(|| overflow_error(pointer, adjustment))?
    } else {
        // Negative adjustment
        if adjustment == i64::MIN {
            return Err(overflow_error(pointer, adjustment));
        }
        #[allow(clippy::cast_sign_loss)]
        let abs_adj = (-adjustment) as u64;
        pointer
            .checked_sub(abs_adj)
            .ok_or_else(|| overflow_error(pointer, adjustment))?
    };

    usize::try_from(adjusted).map_err(|_| overflow_error(pointer, adjustment))
}

/// Map a `TypeReadError` to a `LibmagicError`.
fn map_type_read_error(e: TypeReadError, offset: usize) -> LibmagicError {
    match e {
        TypeReadError::BufferOverrun { .. } => {
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun { offset })
        }
        other => LibmagicError::EvaluationError(EvaluationError::from(other)),
    }
}

/// Create an overflow error for failed adjustment arithmetic.
fn overflow_error(_pointer: u64, adjustment: i64) -> LibmagicError {
    LibmagicError::EvaluationError(EvaluationError::InvalidOffset { offset: adjustment })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Endianness;

    /// Helper to build an `OffsetSpec::Indirect` for tests.
    fn indirect(
        base_offset: i64,
        pointer_type: TypeKind,
        adjustment: i64,
        endian: Endianness,
    ) -> OffsetSpec {
        OffsetSpec::Indirect {
            base_offset,
            pointer_type,
            adjustment,
            endian,
        }
    }

    // ── Byte pointer ─────────────────────────────────────────────

    #[test]
    fn test_byte_pointer_unsigned() {
        // Buffer: [pointer=0x04, ..., target_byte_at_4]
        let buffer = &[0x04, 0x00, 0x00, 0x00, 0xAA];
        let spec = indirect(0, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 4);
    }

    #[test]
    fn test_byte_pointer_signed_positive() {
        let buffer = &[0x03, 0x00, 0x00, 0xBB];
        let spec = indirect(0, TypeKind::Byte { signed: true }, 0, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 3);
    }

    // ── Short pointer, both endiannesses ─────────────────────────

    #[test]
    fn test_short_pointer_little_endian() {
        // LE short at offset 0: bytes [0x04, 0x00] → 0x0004
        let mut buffer = vec![0x04, 0x00, 0x00, 0x00, 0xCC];
        buffer.resize(5, 0);
        let spec = indirect(
            0,
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        assert_eq!(resolve_indirect_offset(&spec, &buffer).unwrap(), 4);
    }

    #[test]
    fn test_short_pointer_big_endian() {
        // BE short at offset 0: bytes [0x00, 0x04] → 0x0004
        let buffer = &[0x00, 0x04, 0x00, 0x00, 0xDD];
        let spec = indirect(
            0,
            TypeKind::Short {
                endian: Endianness::Big,
                signed: false,
            },
            0,
            Endianness::Big,
        );
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 4);
    }

    // ── Long pointer, both endiannesses ──────────────────────────

    #[test]
    fn test_long_pointer_little_endian() {
        // LE long at offset 0: bytes [0x08, 0x00, 0x00, 0x00] → 8
        let mut buffer = vec![0x08, 0x00, 0x00, 0x00];
        buffer.resize(9, 0xAA);
        let spec = indirect(
            0,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        assert_eq!(resolve_indirect_offset(&spec, &buffer).unwrap(), 8);
    }

    #[test]
    fn test_long_pointer_big_endian() {
        // BE long at offset 0: bytes [0x00, 0x00, 0x00, 0x06] → 6
        let buffer = &[0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0xFF];
        let spec = indirect(
            0,
            TypeKind::Long {
                endian: Endianness::Big,
                signed: false,
            },
            0,
            Endianness::Big,
        );
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 6);
    }

    // ── Quad pointer ─────────────────────────────────────────────

    #[test]
    fn test_quad_pointer_little_endian() {
        // LE quad at offset 0: value = 16
        let mut buffer = vec![0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        buffer.resize(17, 0xBB);
        let spec = indirect(
            0,
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        assert_eq!(resolve_indirect_offset(&spec, &buffer).unwrap(), 16);
    }

    #[test]
    fn test_quad_pointer_big_endian() {
        // BE quad at offset 0: bytes [0x00..0x00, 0x10] → 0x0000_0000_0000_0010 = 16
        let mut buffer = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10];
        buffer.resize(17, 0xCC);
        let spec = indirect(
            0,
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: false,
            },
            0,
            Endianness::Big,
        );
        assert_eq!(resolve_indirect_offset(&spec, &buffer).unwrap(), 16);
    }

    // ── extract_raw_unsigned unit tests ────────────────────────

    #[test]
    fn test_extract_raw_unsigned_negative_one() {
        // Value::Int(-1) must reinterpret as u64::MAX (0xFFFF_FFFF_FFFF_FFFF)
        let value = Value::Int(-1);
        assert_eq!(extract_raw_unsigned(&value).unwrap(), u64::MAX);
    }

    #[test]
    fn test_extract_raw_unsigned_negative_two() {
        // Value::Int(-2) must reinterpret as u64::MAX - 1
        let value = Value::Int(-2);
        assert_eq!(extract_raw_unsigned(&value).unwrap(), 0xFFFF_FFFF_FFFF_FFFE);
    }

    #[test]
    fn test_extract_raw_unsigned_i32_min_sign_extended() {
        // A signed 32-bit -1 is sign-extended to i64 -1 by the reader,
        // so extract_raw_unsigned must yield u64::MAX.
        let value = Value::Int(-1);
        assert_eq!(extract_raw_unsigned(&value).unwrap(), 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn test_extract_raw_unsigned_positive_int() {
        let value = Value::Int(42);
        assert_eq!(extract_raw_unsigned(&value).unwrap(), 42);
    }

    #[test]
    fn test_extract_raw_unsigned_uint() {
        let value = Value::Uint(0xDEAD_BEEF);
        assert_eq!(extract_raw_unsigned(&value).unwrap(), 0xDEAD_BEEF);
    }

    #[test]
    fn test_extract_raw_unsigned_rejects_non_integer() {
        let value = Value::String("hello".to_string());
        assert!(extract_raw_unsigned(&value).is_err());
    }

    // ── read_pointer signed-negative unit tests ─────────────────

    #[test]
    fn test_read_pointer_signed_long_negative_one() {
        // LE signed long: [0xFF, 0xFF, 0xFF, 0xFF] → i32 = -1 → i64 = -1 → u64 = 0xFFFF_FFFF_FFFF_FFFF
        let buffer = &[0xFF, 0xFF, 0xFF, 0xFF];
        let raw = read_pointer(
            buffer,
            0,
            &TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )
        .unwrap();
        assert_eq!(raw, u64::MAX);
    }

    #[test]
    fn test_read_pointer_signed_short_negative_two() {
        // LE signed short: [0xFE, 0xFF] → i16 = -2 → i64 = -2 → u64 = 0xFFFF_FFFF_FFFF_FFFE
        let buffer = &[0xFE, 0xFF];
        let raw = read_pointer(
            buffer,
            0,
            &TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            Endianness::Little,
        )
        .unwrap();
        assert_eq!(raw, 0xFFFF_FFFF_FFFF_FFFE);
    }

    #[test]
    fn test_read_pointer_signed_byte_negative_one() {
        // Signed byte: [0xFF] → i8 = -1 → i64 = -1 → u64 = 0xFFFF_FFFF_FFFF_FFFF
        let buffer = &[0xFF];
        let raw = read_pointer(
            buffer,
            0,
            &TypeKind::Byte { signed: true },
            Endianness::Little,
        )
        .unwrap();
        assert_eq!(raw, u64::MAX);
    }

    // ── Signed negative pointer end-to-end ──────────────────────

    #[test]
    fn test_signed_short_negative_pointer_overruns_after_raw_conversion() {
        // Signed LE short at offset 0: bytes [0xFE, 0xFF] → i16 = -2
        // read_pointer extracts raw u64 = 0xFFFF_FFFF_FFFF_FFFE (verified by unit tests above).
        // That enormous pointer value must fail bounds validation, NOT be rejected
        // during extraction. An implementation that rejects negative Value::Int early
        // would not reach the bounds check.
        let buffer = &[0xFE, 0xFF, 0x00, 0x00];
        let spec = indirect(
            0,
            TypeKind::Short {
                endian: Endianness::Little,
                signed: true,
            },
            0,
            Endianness::Little,
        );
        let err = resolve_indirect_offset(&spec, buffer).unwrap_err();

        // After raw unsigned reinterpretation, the pointer is 0xFFFF_FFFF_FFFF_FFFE.
        // On 64-bit: usize::try_from succeeds → BufferOverrun with that exact offset.
        // On 32-bit: usize::try_from overflows → InvalidOffset from apply_adjustment.
        if usize::BITS == 64 {
            assert!(
                matches!(
                    err,
                    LibmagicError::EvaluationError(EvaluationError::BufferOverrun { offset })
                    if offset == 0xFFFF_FFFF_FFFF_FFFE
                ),
                "Expected BufferOverrun at 0xFFFF_FFFF_FFFF_FFFE, got: {err:?}"
            );
        } else {
            assert!(
                matches!(
                    err,
                    LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
                ),
                "Expected InvalidOffset from usize::try_from overflow on 32-bit, got: {err:?}"
            );
        }
    }

    #[test]
    fn test_signed_long_negative_pointer_with_adjustment_overruns() {
        // Signed LE long at offset 0: bytes [0xFF, 0xFF, 0xFF, 0xFF] → i32 = -1
        // extract_raw_unsigned converts Value::Int(-1) → u64::MAX (0xFFFF_FFFF_FFFF_FFFF).
        // Adjustment of -1 yields u64::MAX - 1 = 0xFFFF_FFFF_FFFF_FFFE via checked_sub.
        // Must fail at bounds validation, not during raw extraction.
        let buffer = &[0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
        let spec = indirect(
            0,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            -1,
            Endianness::Little,
        );
        let err = resolve_indirect_offset(&spec, buffer).unwrap_err();

        // After raw reinterpretation: u64::MAX. After adjustment of -1: 0xFFFF_FFFF_FFFF_FFFE.
        // On 64-bit: usize::try_from succeeds → BufferOverrun with that exact offset.
        // On 32-bit: usize::try_from overflows → InvalidOffset from apply_adjustment.
        if usize::BITS == 64 {
            assert!(
                matches!(
                    err,
                    LibmagicError::EvaluationError(EvaluationError::BufferOverrun { offset })
                    if offset == 0xFFFF_FFFF_FFFF_FFFE
                ),
                "Expected BufferOverrun at 0xFFFF_FFFF_FFFF_FFFE, got: {err:?}"
            );
        } else {
            assert!(
                matches!(
                    err,
                    LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
                ),
                "Expected InvalidOffset from usize::try_from overflow on 32-bit, got: {err:?}"
            );
        }
    }

    // ── Positive and negative adjustments ────────────────────────

    #[test]
    fn test_positive_adjustment() {
        // Pointer value = 2, adjustment = +3 → final = 5
        let buffer = &[0x02, 0x00, 0x00, 0x00, 0x00, 0xEE];
        let spec = indirect(0, TypeKind::Byte { signed: false }, 3, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 5);
    }

    #[test]
    fn test_negative_adjustment() {
        // Pointer value = 5, adjustment = -2 → final = 3
        let buffer = &[0x05, 0x00, 0x00, 0xFF];
        let spec = indirect(0, TypeKind::Byte { signed: false }, -2, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 3);
    }

    // ── From-end base offset ─────────────────────────────────────

    #[test]
    fn test_from_end_base_offset() {
        // 8-byte buffer, base_offset = -1 → resolves to index 7
        // Byte at index 7 = 0x02 → pointer value = 2 → final = 2
        let buffer = &[0x00, 0x00, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x02];
        let spec = indirect(-1, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 2);
    }

    // ── Pointer read overrun ─────────────────────────────────────

    #[test]
    fn test_pointer_read_overrun_short() {
        // Buffer has 1 byte, trying to read a short (2 bytes) at offset 0
        let buffer = &[0x04];
        let spec = indirect(
            0,
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun { .. })
        ));
    }

    #[test]
    fn test_pointer_read_overrun_long() {
        // Buffer has 3 bytes, trying to read a long (4 bytes) at offset 0
        let buffer = &[0x00, 0x00, 0x00];
        let spec = indirect(
            0,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun { .. })
        ));
    }

    // ── Final offset overrun ─────────────────────────────────────

    #[test]
    fn test_final_offset_overrun() {
        // Pointer value = 0xFF (255), buffer only 5 bytes → overrun
        let buffer = &[0xFF, 0x00, 0x00, 0x00, 0x00];
        let spec = indirect(0, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun { .. })
        ));
    }

    #[test]
    fn test_final_offset_overrun_with_adjustment() {
        // Pointer value = 3, adjustment = +10, buffer only 8 bytes → 13 overruns
        let buffer = &[0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let spec = indirect(0, TypeKind::Byte { signed: false }, 10, Endianness::Little);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::BufferOverrun { .. })
        ));
    }

    // ── Arithmetic overflow/underflow ────────────────────────────

    #[test]
    fn test_adjustment_overflow() {
        // Unsigned quad reading u64::MAX + positive adjustment → overflow
        let buffer = &[0xFF; 16];
        let spec = indirect(
            0,
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            },
            1,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
        ));
    }

    #[test]
    fn test_adjustment_underflow() {
        // Pointer value = 0, adjustment = -1 → underflow
        let buffer = &[0x00, 0x00, 0x00, 0x00];
        let spec = indirect(0, TypeKind::Byte { signed: false }, -1, Endianness::Little);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset { .. })
        ));
    }

    // ── Unsupported pointer types ────────────────────────────────

    #[test]
    fn test_unsupported_pointer_type_string() {
        let buffer = &[0x00, 0x00, 0x00, 0x00];
        let spec = indirect(
            0,
            TypeKind::String { max_length: None },
            0,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::UnsupportedType { .. })
        ));
    }

    #[test]
    fn test_unsupported_pointer_type_float() {
        let buffer = &[0x00, 0x00, 0x00, 0x00];
        let spec = indirect(
            0,
            TypeKind::Float {
                endian: Endianness::Little,
            },
            0,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::UnsupportedType { .. })
        ));
    }

    #[test]
    fn test_unsupported_pointer_type_double() {
        let buffer = &[0x00; 8];
        let spec = indirect(
            0,
            TypeKind::Double {
                endian: Endianness::Little,
            },
            0,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::UnsupportedType { .. })
        ));
    }

    // ── PE-header-style 32-bit LE pointer at 0x3c ────────────────

    #[test]
    fn test_pe_header_style_offset_0x3c() {
        // Simulate a PE file: 32-bit LE pointer at offset 0x3C points to PE header.
        // At offset 0x3C we store LE u32 = 0x80 (128).
        let mut buffer = vec![0u8; 256];
        // Write LE u32 value 0x80 at offset 0x3C
        buffer[0x3C] = 0x80;
        buffer[0x3D] = 0x00;
        buffer[0x3E] = 0x00;
        buffer[0x3F] = 0x00;
        // Place "PE\0\0" signature at offset 0x80
        buffer[0x80] = b'P';
        buffer[0x81] = b'E';
        buffer[0x82] = 0x00;
        buffer[0x83] = 0x00;

        let spec = indirect(
            0x3C,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            0,
            Endianness::Little,
        );
        let offset = resolve_indirect_offset(&spec, &buffer).unwrap();
        assert_eq!(offset, 0x80);
        // Verify we can read the PE signature at that offset
        assert_eq!(&buffer[offset..offset + 4], b"PE\0\0");
    }

    // ── Base offset out of bounds ────────────────────────────────

    #[test]
    fn test_base_offset_out_of_bounds() {
        let buffer = &[0x00, 0x01, 0x02];
        let spec = indirect(100, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
    }

    // ── Signed pointer extraction ────────────────────────────────

    #[test]
    fn test_signed_long_pointer_positive() {
        // Signed long value = 4 (positive) → final offset = 4
        let buffer = &[0x04, 0x00, 0x00, 0x00, 0xAA];
        let spec = indirect(
            0,
            TypeKind::Long {
                endian: Endianness::Little,
                signed: true,
            },
            0,
            Endianness::Little,
        );
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 4);
    }

    // ── Non-indirect spec produces internal error ────────────────

    #[test]
    fn test_non_indirect_spec_returns_error() {
        let buffer = &[0x00; 8];
        let spec = OffsetSpec::Absolute(0);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LibmagicError::EvaluationError(EvaluationError::InternalError { .. })
        ));
    }
}
