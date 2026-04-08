// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type interpretation for reading and converting bytes from file buffers.
//!
//! This module exposes the public type-reading API and dispatches to focused
//! submodules for numeric and string handling.

mod date;
mod float;
mod numeric;
mod string;

use crate::parser::ast::{TypeKind, Value};
use thiserror::Error;

use date::format_timestamp_value;
pub use date::{read_date, read_qdate};
pub use float::{read_double, read_float};
pub use numeric::{read_byte, read_long, read_quad, read_short};
pub use string::{read_pstring, read_string};

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
    /// e.g., regex, date, timestamp).
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// The name of the unsupported type.
        type_name: String,
    },
    /// Invalid pstring length prefix value (e.g., `/J` flag with stored length
    /// smaller than the prefix width).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::types::TypeReadError;
    /// let err = TypeReadError::InvalidPStringLength {
    ///     stored_length: 1,
    ///     prefix_width: 2,
    /// };
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Invalid pstring length prefix: stored length 1 is less than prefix width 2"
    /// );
    /// ```
    #[error(
        "Invalid pstring length prefix: stored length {stored_length} is less than prefix width {prefix_width}"
    )]
    InvalidPStringLength {
        /// The length value stored in the pstring prefix.
        stored_length: usize,
        /// The byte width of the length prefix field.
        prefix_width: usize,
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
        TypeKind::Date { endian, utc } => read_date(buffer, offset, *endian, *utc),
        TypeKind::QDate { endian, utc } => read_qdate(buffer, offset, *endian, *utc),
        TypeKind::String { max_length } => read_string(buffer, offset, *max_length),
        TypeKind::PString {
            max_length,
            length_width,
            length_includes_itself,
        } => read_pstring(
            buffer,
            offset,
            *max_length,
            *length_width,
            *length_includes_itself,
        ),
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
        // Normalize numeric expected values for date types into formatted timestamp
        // strings so they match the Value::String representation from read_date/read_qdate.
        (Value::Uint(v), TypeKind::Date { utc, .. } | TypeKind::QDate { utc, .. }) => {
            Value::String(format_timestamp_value(*v, *utc))
        }
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(v), TypeKind::Date { utc, .. } | TypeKind::QDate { utc, .. }) if *v >= 0 => {
            Value::String(format_timestamp_value(*v as u64, *utc))
        }
        _ => value.clone(),
    }
}

/// Returns the number of buffer bytes a successful `read_typed_value` would
/// consume for the given `type_kind` at `offset`.
///
/// This mirrors the consumption logic of the underlying read functions and is
/// used by the evaluation engine to advance the GNU `file` "previous match"
/// anchor for relative offset resolution. It is `pub(crate)` because no
/// external caller should depend on the anchor-advance contract -- the only
/// intended caller is `evaluate_rules` in the engine.
///
/// The function is intentionally infallible -- the engine only calls it after
/// a successful read, so the read shape is known to be valid. For variable-
/// width types with unexpected inputs (offset past end of buffer, malformed
/// pstring prefix, `/J` flag underflow), it returns `0` rather than
/// panicking; the anchor then stays put and the next relative offset will
/// bounds-fail gracefully. Fixed-width types do not bounds-check the offset
/// (they return `bit_width / 8` unconditionally) because the engine's
/// read-then-call invariant guarantees a successful read preceded the call.
/// Calling `bytes_consumed` for a fixed-width type at an out-of-bounds
/// offset will report a nonzero width that does not correspond to any
/// actual buffer bytes -- do not call this function outside the engine's
/// post-read flow.
///
/// # Semantics
///
/// - **Fixed-width types** (Byte, Short, Long, Quad, Float, Double, Date,
///   QDate): width derived from `TypeKind::bit_width()`. The engine
///   guarantees the offset is in-bounds; callers outside the engine must
///   uphold the same invariant.
/// - **C-string** (`TypeKind::String`): scans for the first NUL within
///   `max_length` bytes (or to the buffer end). Returns `length + 1` when a
///   NUL is found (the NUL is consumed), or `length` if the buffer ends or
///   `max_length` truncates first.
/// - **Pascal string** (`TypeKind::PString`): reads the length prefix (1, 2,
///   or 4 bytes, BE/LE), accounts for the `/J` flag (stored length includes
///   prefix width), caps by `max_length`, and returns `prefix_width +
///   actual_payload_bytes`. The result is also clamped against the remaining
///   buffer length so a malicious oversized length prefix cannot poison the
///   anchor.
#[must_use]
pub(crate) fn bytes_consumed(buffer: &[u8], offset: usize, type_kind: &TypeKind) -> usize {
    if let Some(bits) = type_kind.bit_width() {
        return (bits as usize) / 8;
    }

    match type_kind {
        TypeKind::String { max_length } => string_bytes_consumed(buffer, offset, *max_length),
        TypeKind::PString {
            max_length,
            length_width,
            length_includes_itself,
        } => pstring_bytes_consumed(
            buffer,
            offset,
            *max_length,
            *length_width,
            *length_includes_itself,
        ),
        // A new variable-width TypeKind variant was added without updating
        // this match. Returning 0 here would silently corrupt the GNU `file`
        // anchor for any rule using relative offsets after a match of the
        // new type. The debug assertion fires immediately in test/dev
        // builds; release builds keep the 0 fallback (graceful skip rather
        // than panic), but the asserting log highlights the gap.
        //
        // GOTCHAS S2.1 lists this match in the new-TypeKind-variant
        // checklist -- see that section if you are reading this comment
        // because the assertion just fired.
        _ => {
            debug_assert!(
                false,
                "bytes_consumed: unhandled variable-width TypeKind variant {type_kind:?} -- update bytes_consumed and GOTCHAS S2.1"
            );
            0
        }
    }
}

/// Compute the buffer bytes consumed by a successful c-string read.
///
/// Mirrors `read_string`: scans from `offset` for the first NUL within
/// `max_length` bytes (or to the end of the buffer when `max_length` is
/// `None`), and returns `length_to_nul + 1` when a NUL was found, or
/// `length_read` when no NUL was found (truncated by buffer end or
/// `max_length`).
fn string_bytes_consumed(buffer: &[u8], offset: usize, max_length: Option<usize>) -> usize {
    let Some(remaining) = buffer.get(offset..) else {
        return 0;
    };
    let search_len = max_length.map_or(remaining.len(), |m| m.min(remaining.len()));
    let Some(window) = remaining.get(..search_len) else {
        return 0;
    };
    match memchr::memchr(0, window) {
        Some(nul_idx) => nul_idx.saturating_add(1),
        None => search_len,
    }
}

/// Compute the buffer bytes consumed by a successful pstring read.
///
/// Mirrors `read_pstring`: reads the length prefix, applies the `/J` flag,
/// caps by `max_length`, and returns `prefix_width + payload_bytes`. Returns
/// `0` for any unexpected condition (offset past end, prefix bytes missing,
/// `/J` underflow), since the engine only calls this after a successful read.
fn pstring_bytes_consumed(
    buffer: &[u8],
    offset: usize,
    max_length: Option<usize>,
    length_width: crate::parser::ast::PStringLengthWidth,
    length_includes_itself: bool,
) -> usize {
    use crate::parser::ast::PStringLengthWidth;
    let width = length_width.byte_count();
    let Some(prefix_end) = offset.checked_add(width) else {
        return 0;
    };
    let Some(len_bytes) = buffer.get(offset..prefix_end) else {
        return 0;
    };
    let stored_length = match length_width {
        PStringLengthWidth::OneByte => usize::from(len_bytes[0]),
        PStringLengthWidth::TwoByteBE => {
            let arr: [u8; 2] = match len_bytes.try_into() {
                Ok(a) => a,
                Err(_) => return 0,
            };
            usize::from(u16::from_be_bytes(arr))
        }
        PStringLengthWidth::TwoByteLE => {
            let arr: [u8; 2] = match len_bytes.try_into() {
                Ok(a) => a,
                Err(_) => return 0,
            };
            usize::from(u16::from_le_bytes(arr))
        }
        PStringLengthWidth::FourByteBE => {
            let arr: [u8; 4] = match len_bytes.try_into() {
                Ok(a) => a,
                Err(_) => return 0,
            };
            u32::from_be_bytes(arr) as usize
        }
        PStringLengthWidth::FourByteLE => {
            let arr: [u8; 4] = match len_bytes.try_into() {
                Ok(a) => a,
                Err(_) => return 0,
            };
            u32::from_le_bytes(arr) as usize
        }
    };

    let payload_length = if length_includes_itself {
        match stored_length.checked_sub(width) {
            Some(n) => n,
            None => return 0,
        }
    } else {
        stored_length
    };

    // Clamp against remaining buffer bytes after the prefix. This defends
    // against an attacker-controlled 4-byte length prefix near u32::MAX
    // poisoning the anchor: read_pstring would have failed to actually read
    // a payload that long, so a successful read implies the payload fit in
    // the buffer. Mirroring that bound here keeps the anchor truthful.
    let remaining_after_prefix = buffer.len().saturating_sub(prefix_end);
    let bounded_payload = payload_length.min(remaining_after_prefix);
    let actual_length = max_length.map_or(bounded_payload, |m| m.min(bounded_payload));
    width.saturating_add(actual_length)
}

#[cfg(test)]
mod tests;
