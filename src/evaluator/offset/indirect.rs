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

    // Validate: outer endian must match inner TypeKind endian (single source of truth).
    // Byte has no inner endian field so only multi-byte types need the check.
    match pointer_type {
        TypeKind::Short { endian: inner, .. }
        | TypeKind::Long { endian: inner, .. }
        | TypeKind::Quad { endian: inner, .. } => {
            debug_assert_eq!(
                *inner, endian,
                "Indirect offset: inner TypeKind endianness ({inner:?}) \
                 contradicts outer endian field ({endian:?})"
            );
        }
        _ => {}
    }

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

    #[test]
    fn test_pointer_type_and_endianness() {
        let cases: &[(&str, &[u8], TypeKind, Endianness, usize)] = &[
            (
                "byte unsigned",
                &[0x04, 0x00, 0x00, 0x00, 0xAA],
                TypeKind::Byte { signed: false },
                Endianness::Little,
                4,
            ),
            (
                "byte signed positive",
                &[0x03, 0x00, 0x00, 0xBB],
                TypeKind::Byte { signed: true },
                Endianness::Little,
                3,
            ),
            (
                "short LE",
                &[0x04, 0x00, 0x00, 0x00, 0xCC],
                TypeKind::Short {
                    endian: Endianness::Little,
                    signed: false,
                },
                Endianness::Little,
                4,
            ),
            (
                "short BE",
                &[0x00, 0x04, 0x00, 0x00, 0xDD],
                TypeKind::Short {
                    endian: Endianness::Big,
                    signed: false,
                },
                Endianness::Big,
                4,
            ),
            (
                "long LE",
                &[0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
                TypeKind::Long {
                    endian: Endianness::Little,
                    signed: false,
                },
                Endianness::Little,
                6,
            ),
            (
                "long BE",
                &[0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0xFF],
                TypeKind::Long {
                    endian: Endianness::Big,
                    signed: false,
                },
                Endianness::Big,
                6,
            ),
            (
                "signed long positive",
                &[0x04, 0x00, 0x00, 0x00, 0xAA],
                TypeKind::Long {
                    endian: Endianness::Little,
                    signed: true,
                },
                Endianness::Little,
                4,
            ),
        ];
        for (name, buf, ptype, endian, expected) in cases {
            let spec = indirect(0, ptype.clone(), 0, *endian);
            assert_eq!(
                resolve_indirect_offset(&spec, buf).unwrap(),
                *expected,
                "Failed for case: {name}"
            );
        }

        // Quad types need resizable buffers (target offset > inline slice length)
        let quad_cases: &[(&str, Endianness, &[u8])] = &[
            (
                "quad LE",
                Endianness::Little,
                &[0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ),
            (
                "quad BE",
                Endianness::Big,
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10],
            ),
        ];
        for (name, endian, prefix) in quad_cases {
            let mut buffer = prefix.to_vec();
            buffer.resize(17, 0xBB);
            let spec = indirect(
                0,
                TypeKind::Quad {
                    endian: *endian,
                    signed: false,
                },
                0,
                *endian,
            );
            assert_eq!(
                resolve_indirect_offset(&spec, &buffer).unwrap(),
                16,
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_extract_raw_unsigned_values() {
        let ok_cases: &[(&str, Value, u64)] = &[
            ("Int(-1) -> u64::MAX", Value::Int(-1), u64::MAX),
            (
                "Int(-2) -> u64::MAX-1",
                Value::Int(-2),
                0xFFFF_FFFF_FFFF_FFFE,
            ),
            (
                "Int(-1) sign-extended",
                Value::Int(-1),
                0xFFFF_FFFF_FFFF_FFFF,
            ),
            ("Int(42)", Value::Int(42), 42),
            ("Uint(0xDEAD_BEEF)", Value::Uint(0xDEAD_BEEF), 0xDEAD_BEEF),
        ];
        for (name, value, expected) in ok_cases {
            assert_eq!(
                extract_raw_unsigned(value).unwrap(),
                *expected,
                "Failed for case: {name}"
            );
        }

        let err_value = Value::String("hello".to_string());
        assert!(
            extract_raw_unsigned(&err_value).is_err(),
            "Failed for case: rejects non-integer"
        );
    }

    #[test]
    fn test_read_pointer_signed_negative() {
        let cases: &[(&str, &[u8], TypeKind, u64)] = &[
            (
                "signed long -1",
                &[0xFF, 0xFF, 0xFF, 0xFF],
                TypeKind::Long {
                    endian: Endianness::Little,
                    signed: true,
                },
                u64::MAX,
            ),
            (
                "signed short -2",
                &[0xFE, 0xFF],
                TypeKind::Short {
                    endian: Endianness::Little,
                    signed: true,
                },
                0xFFFF_FFFF_FFFF_FFFE,
            ),
            (
                "signed byte -1",
                &[0xFF],
                TypeKind::Byte { signed: true },
                u64::MAX,
            ),
        ];
        for (name, buf, ptype, expected) in cases {
            let raw = read_pointer(buf, 0, ptype, Endianness::Little).unwrap();
            assert_eq!(raw, *expected, "Failed for case: {name}");
        }
    }

    #[test]
    fn test_signed_short_negative_pointer_overruns_after_raw_conversion() {
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
    fn test_adjustments() {
        let cases: &[(&str, &[u8], i64, usize)] = &[
            ("positive +3", &[0x02, 0x00, 0x00, 0x00, 0x00, 0xEE], 3, 5),
            ("negative -2", &[0x05, 0x00, 0x00, 0xFF], -2, 3),
        ];
        for (name, buf, adj, expected) in cases {
            let spec = indirect(
                0,
                TypeKind::Byte { signed: false },
                *adj,
                Endianness::Little,
            );
            assert_eq!(
                resolve_indirect_offset(&spec, buf).unwrap(),
                *expected,
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_from_end_base_offset() {
        let buffer = &[0x00, 0x00, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x02];
        let spec = indirect(-1, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        assert_eq!(resolve_indirect_offset(&spec, buffer).unwrap(), 2);
    }

    #[test]
    fn test_pointer_read_overrun() {
        let cases: &[(&str, &[u8], TypeKind)] = &[
            (
                "short from 1-byte buffer",
                &[0x04],
                TypeKind::Short {
                    endian: Endianness::Little,
                    signed: false,
                },
            ),
            (
                "long from 3-byte buffer",
                &[0x00, 0x00, 0x00],
                TypeKind::Long {
                    endian: Endianness::Little,
                    signed: false,
                },
            ),
        ];
        for (name, buf, ptype) in cases {
            let spec = indirect(0, ptype.clone(), 0, Endianness::Little);
            let result = resolve_indirect_offset(&spec, buf);
            assert!(
                matches!(
                    result,
                    Err(LibmagicError::EvaluationError(
                        EvaluationError::BufferOverrun { .. }
                    ))
                ),
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_final_offset_overrun() {
        let cases: &[(&str, &[u8], i64)] = &[
            (
                "pointer=0xFF, no adjustment",
                &[0xFF, 0x00, 0x00, 0x00, 0x00],
                0,
            ),
            (
                "pointer=3, adjustment=+10",
                &[0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                10,
            ),
        ];
        for (name, buf, adj) in cases {
            let spec = indirect(
                0,
                TypeKind::Byte { signed: false },
                *adj,
                Endianness::Little,
            );
            let result = resolve_indirect_offset(&spec, buf);
            assert!(
                matches!(
                    result,
                    Err(LibmagicError::EvaluationError(
                        EvaluationError::BufferOverrun { .. }
                    ))
                ),
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_adjustment_overflow_underflow() {
        let cases: &[(&str, &[u8], TypeKind, i64)] = &[
            (
                "overflow: u64::MAX + 1",
                &[0xFF; 16],
                TypeKind::Quad {
                    endian: Endianness::Little,
                    signed: false,
                },
                1,
            ),
            (
                "underflow: 0 - 1",
                &[0x00, 0x00, 0x00, 0x00],
                TypeKind::Byte { signed: false },
                -1,
            ),
        ];
        for (name, buf, ptype, adj) in cases {
            let spec = indirect(0, ptype.clone(), *adj, Endianness::Little);
            let result = resolve_indirect_offset(&spec, buf);
            assert!(
                matches!(
                    result,
                    Err(LibmagicError::EvaluationError(
                        EvaluationError::InvalidOffset { .. }
                    ))
                ),
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_unsupported_pointer_types() {
        let cases: &[(&str, &[u8], TypeKind)] = &[
            ("string", &[0x00; 4], TypeKind::String { max_length: None }),
            (
                "float",
                &[0x00; 4],
                TypeKind::Float {
                    endian: Endianness::Little,
                },
            ),
            (
                "double",
                &[0x00; 8],
                TypeKind::Double {
                    endian: Endianness::Little,
                },
            ),
        ];
        for (name, buf, ptype) in cases {
            let spec = indirect(0, ptype.clone(), 0, Endianness::Little);
            let result = resolve_indirect_offset(&spec, buf);
            assert!(
                matches!(
                    result,
                    Err(LibmagicError::EvaluationError(
                        EvaluationError::UnsupportedType { .. }
                    ))
                ),
                "Failed for case: {name}"
            );
        }
    }

    #[test]
    fn test_pe_header_style_offset_0x3c() {
        let mut buffer = vec![0u8; 256];
        buffer[0x3C] = 0x80;
        buffer[0x3D] = 0x00;
        buffer[0x3E] = 0x00;
        buffer[0x3F] = 0x00;
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
        assert_eq!(&buffer[offset..offset + 4], b"PE\0\0");
    }

    #[test]
    fn test_base_offset_out_of_bounds() {
        let buffer = &[0x00, 0x01, 0x02];
        let spec = indirect(100, TypeKind::Byte { signed: false }, 0, Endianness::Little);
        assert!(resolve_indirect_offset(&spec, buffer).is_err());
    }

    #[test]
    fn test_non_indirect_spec_returns_error() {
        let buffer = &[0x00; 8];
        let spec = OffsetSpec::Absolute(0);
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(
            matches!(
                result,
                Err(LibmagicError::EvaluationError(
                    EvaluationError::InternalError { .. }
                ))
            ),
            "Expected InternalError for non-indirect spec"
        );
    }
}
