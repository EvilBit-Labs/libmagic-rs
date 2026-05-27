// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Indirect offset resolution
//!
//! Indirect offsets read a pointer value from the file at a base offset,
//! then use that value (with optional adjustment) as the final offset.

use crate::LibmagicError;
use crate::error::EvaluationError;
use crate::evaluator::types::{TypeReadError, read_byte, read_long, read_quad, read_short};
use crate::parser::ast::{Endianness, IndirectAdjustmentOp, OffsetSpec, TypeKind, Value};

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
#[cfg(test)]
pub fn resolve_indirect_offset(spec: &OffsetSpec, buffer: &[u8]) -> Result<usize, LibmagicError> {
    resolve_indirect_offset_with_anchor(spec, buffer, None)
}

/// Resolve an indirect offset with an optional anchor for the
/// `base_relative` / `result_relative` flags.
///
/// `base_relative` is `(&N.X)` syntax: the pointer-read base is
/// `anchor + base_offset` rather than absolute. `result_relative` is
/// `&(N.X)` syntax: the resolved pointer value is added to the anchor
/// instead of being treated as an absolute file position. Both fall
/// back to absolute behavior when `anchor` is `None`.
pub fn resolve_indirect_offset_with_anchor(
    spec: &OffsetSpec,
    buffer: &[u8],
    anchor: Option<usize>,
) -> Result<usize, LibmagicError> {
    let (
        base_offset,
        base_relative,
        pointer_type,
        adjustment,
        adjustment_op,
        result_relative,
        endian,
    ) = match spec {
        OffsetSpec::Indirect {
            base_offset,
            base_relative,
            pointer_type,
            adjustment,
            adjustment_op,
            result_relative,
            endian,
        } => (
            *base_offset,
            *base_relative,
            pointer_type,
            *adjustment,
            *adjustment_op,
            *result_relative,
            *endian,
        ),
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

    // Step 1: Resolve base_offset to an absolute position. When the
    // `base_relative` flag is set, the base is `anchor + base_offset`
    // (matching magic(5) `(&N.X)` syntax). If no anchor is supplied,
    // fall back to absolute resolution.
    let abs_base = if base_relative {
        let anchor_pos = anchor.unwrap_or(0);
        let signed_anchor = i64::try_from(anchor_pos).map_err(|_| {
            LibmagicError::EvaluationError(EvaluationError::InvalidOffset {
                offset: base_offset,
            })
        })?;
        let combined =
            signed_anchor
                .checked_add(base_offset)
                .ok_or(LibmagicError::EvaluationError(
                    EvaluationError::InvalidOffset {
                        offset: base_offset,
                    },
                ))?;
        resolve_absolute_offset(combined, buffer).map_err(|e| map_offset_error(&e, combined))?
    } else {
        resolve_absolute_offset(base_offset, buffer)
            .map_err(|e| map_offset_error(&e, base_offset))?
    };

    // Step 2: Read pointer value using the appropriate numeric reader
    let pointer_value = read_pointer(buffer, abs_base, pointer_type, endian)?;

    // Step 3: Apply adjustment with checked arithmetic
    let mut final_offset = apply_adjustment(pointer_value, adjustment, adjustment_op)?;

    // Step 3b: When `result_relative` is set (`&(...)` syntax), add the
    // anchor so the final offset is anchor-relative.
    if result_relative {
        let anchor_pos = anchor.unwrap_or(0);
        final_offset =
            final_offset
                .checked_add(anchor_pos)
                .ok_or(LibmagicError::EvaluationError(
                    EvaluationError::InvalidOffset {
                        offset: base_offset,
                    },
                ))?;
    }

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

/// Apply an arithmetic operation to a `u64` pointer value with the given
/// `i64` operand and checked arithmetic.
///
/// `IndirectAdjustmentOp::Add` accepts negative operands (subtraction is
/// folded into Add by the parser). Multiplicative and bitwise operators
/// take signed operands too, but `Div` and `Mod` reject a zero operand
/// with `EvaluationError::InvalidOffset`.
fn apply_adjustment(
    pointer: u64,
    adjustment: i64,
    op: IndirectAdjustmentOp,
) -> Result<usize, LibmagicError> {
    // Bitwise and multiplicative ops view the operand as a `u64`. Casting
    // `i64 -> u64` reinterprets the bit pattern (so `-1` becomes
    // `u64::MAX`), which matches libmagic's `apprentice.c::do_offset`
    // behavior of operating on raw machine words. Add still uses signed
    // semantics so that `(N.X-1)` (encoded by the parser as `Add(-1)`)
    // performs subtraction.
    #[allow(clippy::cast_sign_loss)]
    let operand_u64 = adjustment as u64;

    let adjusted = match op {
        IndirectAdjustmentOp::Add => {
            // `i64::unsigned_abs` handles `i64::MIN` without overflow (it
            // returns `2^63` as `u64`), so no special case is needed.
            if adjustment >= 0 {
                pointer
                    .checked_add(adjustment.unsigned_abs())
                    .ok_or_else(|| overflow_error(pointer, adjustment))?
            } else {
                pointer
                    .checked_sub(adjustment.unsigned_abs())
                    .ok_or_else(|| overflow_error(pointer, adjustment))?
            }
        }
        IndirectAdjustmentOp::Mul => pointer
            .checked_mul(operand_u64)
            .ok_or_else(|| overflow_error(pointer, adjustment))?,
        IndirectAdjustmentOp::Div => pointer
            .checked_div(operand_u64)
            .ok_or_else(|| overflow_error(pointer, adjustment))?,
        IndirectAdjustmentOp::Mod => pointer
            .checked_rem(operand_u64)
            .ok_or_else(|| overflow_error(pointer, adjustment))?,
        IndirectAdjustmentOp::And => pointer & operand_u64,
        IndirectAdjustmentOp::Or => pointer | operand_u64,
        IndirectAdjustmentOp::Xor => pointer ^ operand_u64,
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
    use crate::parser::ast::{Endianness, StringFlags};

    fn indirect(
        base_offset: i64,
        pointer_type: TypeKind,
        adjustment: i64,
        endian: Endianness,
    ) -> OffsetSpec {
        OffsetSpec::Indirect {
            base_offset,
            base_relative: false,
            pointer_type,
            adjustment,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian,
        }
    }

    /// Helper for constructing an indirect spec with a non-`Add` op
    /// (used by arithmetic-op test cases).
    fn indirect_with_op(
        base_offset: i64,
        pointer_type: TypeKind,
        adjustment: i64,
        adjustment_op: IndirectAdjustmentOp,
        endian: Endianness,
    ) -> OffsetSpec {
        OffsetSpec::Indirect {
            base_offset,
            base_relative: false,
            pointer_type,
            adjustment,
            adjustment_op,
            result_relative: false,
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
    }

    #[test]
    fn test_quad_pointer_endianness() {
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
            (
                "string",
                &[0x00; 4],
                TypeKind::String {
                    max_length: None,
                    flags: StringFlags::default(),
                },
            ),
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

    #[test]
    fn test_resolve_indirect_offset_arithmetic_ops() {
        // Buffer with a byte value of 4 at offset 0; indirect resolution
        // reads that 4 and combines it with the operand using each op.
        // Buffer is 32 bytes so all results stay in-bounds.
        let buffer = &[0x04u8; 32];
        let cases: &[(&str, IndirectAdjustmentOp, i64, usize)] = &[
            ("Mul: 4 * 2 = 8", IndirectAdjustmentOp::Mul, 2, 8),
            ("Mul: 4 * 4 = 16", IndirectAdjustmentOp::Mul, 4, 16),
            ("Div: 4 / 2 = 2", IndirectAdjustmentOp::Div, 2, 2),
            ("Mod: 4 % 3 = 1", IndirectAdjustmentOp::Mod, 3, 1),
            ("And: 4 & 0x0F = 4", IndirectAdjustmentOp::And, 0x0F, 4),
            ("Or:  4 | 0x10 = 20", IndirectAdjustmentOp::Or, 0x10, 20),
            ("Xor: 4 ^ 0x0c = 8", IndirectAdjustmentOp::Xor, 0x0c, 8),
        ];
        for (name, op, operand, expected) in cases {
            let spec = indirect_with_op(
                0,
                TypeKind::Byte { signed: false },
                *operand,
                *op,
                Endianness::Little,
            );
            let result = resolve_indirect_offset(&spec, buffer)
                .unwrap_or_else(|e| panic!("{name}: unexpected error {e:?}"));
            assert_eq!(result, *expected, "{name}");
        }
    }

    #[test]
    fn test_resolve_indirect_offset_div_or_mod_by_zero() {
        // Div and Mod must reject a zero operand instead of panicking.
        let buffer = &[0x04u8; 8];
        for op in [IndirectAdjustmentOp::Div, IndirectAdjustmentOp::Mod] {
            let spec = indirect_with_op(
                0,
                TypeKind::Byte { signed: false },
                0,
                op,
                Endianness::Little,
            );
            let result = resolve_indirect_offset(&spec, buffer);
            assert!(
                matches!(
                    result,
                    Err(LibmagicError::EvaluationError(
                        EvaluationError::InvalidOffset { .. }
                    ))
                ),
                "Expected InvalidOffset for {op:?} by zero"
            );
        }
    }

    #[test]
    fn test_resolve_indirect_offset_mul_overflow() {
        // u64::MAX (0xFF...FF read as quad) * 2 overflows.
        let buffer = &[0xFFu8; 16];
        let spec = indirect_with_op(
            0,
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            },
            2,
            IndirectAdjustmentOp::Mul,
            Endianness::Little,
        );
        let result = resolve_indirect_offset(&spec, buffer);
        assert!(matches!(
            result,
            Err(LibmagicError::EvaluationError(
                EvaluationError::InvalidOffset { .. }
            ))
        ));
    }
}
