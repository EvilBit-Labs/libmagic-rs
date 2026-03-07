// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type interpretation for reading and converting bytes from file buffers.
//!
//! This module exposes the public type-reading API and dispatches to focused
//! submodules for numeric and string handling.

mod float;
mod numeric;
mod string;

use crate::parser::ast::{TypeKind, Value};
use thiserror::Error;

pub use float::{read_double, read_float};
pub use numeric::{read_byte, read_long, read_quad, read_short};
pub use string::read_string;

/// Errors that can occur during type reading operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypeReadError {
    /// Buffer access beyond available data.
    #[error(
        "Buffer overrun: attempted to read at offset {offset} but buffer length is {buffer_len}"
    )]
    BufferOverrun {
        /// The offset that was attempted to be accessed.
        offset: usize,
        /// The actual length of the buffer.
        buffer_len: usize,
    },
    /// Unsupported type variant (reserved for future types not yet evaluatable,
    /// e.g., regex, float, date).
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// The name of the unsupported type.
        type_name: String,
    },
}

/// Reads bytes according to the specified `TypeKind`.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_typed_value;
/// use libmagic_rs::parser::ast::{Endianness, TypeKind, Value};
///
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x34, 0x12];
/// let byte_result = read_typed_value(buffer, 0, &TypeKind::Byte { signed: false }).unwrap();
/// assert_eq!(byte_result, Value::Uint(0x7f));
///
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
/// Returns `TypeReadError::BufferOverrun` when the requested value extends past
/// the buffer bounds.
pub fn read_typed_value(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
) -> Result<Value, TypeReadError> {
    match type_kind {
        TypeKind::Byte { signed } => read_byte(buffer, offset, *signed),
        TypeKind::Short { endian, signed } => read_short(buffer, offset, *endian, *signed),
        TypeKind::Long { endian, signed } => read_long(buffer, offset, *endian, *signed),
        TypeKind::Quad { endian, signed } => read_quad(buffer, offset, *endian, *signed),
        TypeKind::Float { endian } => read_float(buffer, offset, *endian),
        TypeKind::Double { endian } => read_double(buffer, offset, *endian),
        TypeKind::String { max_length } => read_string(buffer, offset, *max_length),
    }
}

/// Coerces a rule value to the signed width implied by `type_kind`.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::coerce_value_to_type;
/// use libmagic_rs::parser::ast::{TypeKind, Value};
///
/// let coerced = coerce_value_to_type(&Value::Uint(0xff), &TypeKind::Byte { signed: true });
/// assert_eq!(coerced, Value::Int(-1));
/// ```
#[must_use]
pub fn coerce_value_to_type(value: &Value, type_kind: &TypeKind) -> Value {
    match (value, type_kind) {
        (Value::Uint(v), TypeKind::Byte { signed: true }) if *v > i8::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Value::Int(i64::from(*v as u8 as i8))
        }
        (Value::Uint(v), TypeKind::Short { signed: true, .. }) if *v > i16::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Value::Int(i64::from(*v as u16 as i16))
        }
        (Value::Uint(v), TypeKind::Long { signed: true, .. }) if *v > i32::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Value::Int(i64::from(*v as u32 as i32))
        }
        (Value::Uint(v), TypeKind::Quad { signed: true, .. }) if *v > i64::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_wrap)]
            Value::Int(*v as i64)
        }
        // Round f64 expected value to f32 precision for TypeKind::Float so that
        // parsed f64 literals compare correctly against f32-widened file values.
        #[allow(clippy::cast_possible_truncation)]
        (Value::Float(v), TypeKind::Float { .. }) => Value::Float(f64::from(*v as f32)),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests;
