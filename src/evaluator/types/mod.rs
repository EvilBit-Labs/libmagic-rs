// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Type interpretation for reading and converting bytes from file buffers.
//!
//! This module exposes the public type-reading API and dispatches to focused
//! submodules for numeric and string handling.

mod date;
mod float;
mod numeric;
pub(crate) mod regex;
mod search;
mod string;

use crate::parser::ast::{TypeKind, Value};
use std::borrow::Cow;
use thiserror::Error;

use date::format_timestamp_value;
pub use date::{read_date, read_qdate};
pub use float::{read_double, read_float};
pub use numeric::{read_byte, read_long, read_quad, read_short};
pub use regex::read_regex;
pub use search::read_search;
use string::string16_bytes_consumed;
pub use string::{read_pstring, read_string, read_string_exact, read_string16};

/// Reads a fixed-size byte array from the buffer at the given offset.
///
/// This is a shared helper for numeric, date, and float type readers that
/// need to extract exactly `N` bytes starting at `offset`. It performs a
/// bounds check (with overflow-safe addition) and returns a
/// `TypeReadError::BufferOverrun` with the original offset and buffer
/// length if the read cannot be satisfied.
pub(super) fn read_bytes_at<const N: usize>(
    buffer: &[u8],
    offset: usize,
) -> Result<[u8; N], TypeReadError> {
    let end = offset.checked_add(N).ok_or(TypeReadError::BufferOverrun {
        offset,
        buffer_len: buffer.len(),
    })?;
    buffer
        .get(offset..end)
        .and_then(|s| s.try_into().ok())
        .ok_or(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        })
}

/// Errors that can occur during type reading operations.
#[non_exhaustive]
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
    /// Type-level capability failure: regex pattern compile error, missing
    /// pattern operand on a pattern-bearing type, non-equality operator on
    /// a pattern-bearing type, or a future capability gap. The `type_name`
    /// field carries a free-form description of the offending type or
    /// condition; callers should treat this as an opaque diagnostic string.
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// Free-form description of the offending type or failure condition.
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
/// This is the public dispatch entry point for type reading for non
/// pattern-bearing types. It preserves the original three-argument
/// signature used by external consumers -- fixed-width numeric, float,
/// date, string, and pstring types need no pattern operand, so the hot
/// path stays ergonomic.
///
/// For pattern-bearing types (`TypeKind::Regex`, `TypeKind::Search`) this
/// function will return `TypeReadError::UnsupportedType` because the
/// pattern operand is mandatory. Callers that need to evaluate regex/search
/// rules should use [`read_typed_value_with_pattern`] and thread the rule
/// value operand through as `pattern`.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_typed_value;
/// use libmagic_rs::parser::ast::{Endianness, TypeKind, Value};
///
/// let buffer = &[0x7f, 0x45, 0x4c, 0x46, 0x34, 0x12];
/// let byte_result =
///     read_typed_value(buffer, 0, &TypeKind::Byte { signed: false }).unwrap();
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
/// Returns `TypeReadError::BufferOverrun` when the requested value extends
/// past the buffer bounds, `TypeReadError::UnsupportedType` when a
/// pattern-bearing type is evaluated without a pattern, or
/// `TypeReadError::InvalidPStringLength` for a malformed Pascal string
/// length prefix.
pub fn read_typed_value(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
) -> Result<Value, TypeReadError> {
    read_typed_value_with_pattern(buffer, offset, type_kind, None)
}

/// Reads bytes according to the specified `TypeKind`, threading a
/// `pattern` operand through for pattern-bearing types (`Regex`, `Search`).
///
/// This is the internal dispatch entry point used by the evaluation engine
/// to evaluate pattern-bearing types. The engine threads the rule's value
/// operand through as `pattern` so the regex and search readers can
/// compile/locate it against the buffer. For fixed-width and non-pattern
/// types (numeric, float, date, string, pstring), the `pattern` parameter
/// is ignored; external callers for those types should prefer the simpler
/// three-argument [`read_typed_value`] wrapper.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::read_typed_value_with_pattern;
/// use libmagic_rs::parser::ast::{RegexCount, RegexFlags, TypeKind, Value};
///
/// let haystack = b"abc123def";
/// let regex_type = TypeKind::Regex {
///     flags: RegexFlags::default(),
///     count: RegexCount::Default,
/// };
/// let pattern = Value::String("[0-9]+".to_string());
/// let regex_result =
///     read_typed_value_with_pattern(haystack, 0, &regex_type, Some(&pattern)).unwrap();
/// assert_eq!(regex_result, Value::String("123".to_string()));
/// ```
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` when the requested value extends
/// past the buffer bounds, `TypeReadError::UnsupportedType` when a regex
/// pattern fails to compile or a pattern-bearing type is evaluated without
/// a pattern, or `TypeReadError::InvalidPStringLength` for a malformed
/// Pascal string length prefix.
pub fn read_typed_value_with_pattern(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
    pattern: Option<&Value>,
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
        TypeKind::String { max_length } => {
            // libmagic semantics: `string PATTERN` compares the first
            // `len(PATTERN)` bytes of the buffer against the literal
            // pattern -- byte-for-byte, with NO NUL truncation. This
            // matters for patterns that legitimately contain NUL bytes
            // (e.g. `0 string PNCIHISK\0 ...`): if we stop at the
            // pattern's NUL we read 8 bytes from a 9-byte buffer and the
            // comparison fails even though the file matches exactly.
            //
            // Three behaviors selected by `(max_length, pattern)`:
            // - `(Some(n), _)`: read exactly `n` bytes (legacy explicit
            //   max-length path). Used for programmatic AST construction.
            // - `(None, Some(Value::String(p)))`: read exactly `p.len()`
            //   bytes for byte-exact comparison. This is the path that
            //   real magic-file rules go through.
            // - `(None, _)`: scan-mode read until NUL/EOF. Used for the
            //   `x` (any-value) operator, format substitution like
            //   `string x %s`, and any caller that wants the printable
            //   prefix rather than a fixed-length buffer slice.
            match (max_length, pattern) {
                (Some(n), _) => read_string_exact(buffer, offset, *n),
                (None, Some(Value::String(p))) => read_string_exact(buffer, offset, p.len()),
                (None, Some(Value::Bytes(b))) => read_string_exact(buffer, offset, b.len()),
                (None, _) => read_string(buffer, offset, None),
            }
        }
        TypeKind::String16 { endian } => read_string16(buffer, offset, *endian),
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
        TypeKind::Regex { flags, count } => {
            let pattern_str = match pattern {
                Some(Value::String(s)) => s.as_str(),
                _ => {
                    return Err(TypeReadError::UnsupportedType {
                        type_name: "regex without string pattern".to_string(),
                    });
                }
            };
            // Collapse `None` (no match) to `Value::String(String::new())`
            // for back-compat with callers using the single-Value return
            // shape. The engine path goes through `read_pattern_match`
            // directly and preserves the `Option` so it can distinguish a
            // zero-width match from a miss.
            Ok(read_regex(buffer, offset, pattern_str, *flags, *count)?
                .unwrap_or_else(|| Value::String(String::new())))
        }
        TypeKind::Search { range } => {
            let pattern_bytes: &[u8] = match pattern {
                Some(Value::String(s)) => s.as_bytes(),
                Some(Value::Bytes(b)) => b.as_slice(),
                _ => {
                    return Err(TypeReadError::UnsupportedType {
                        type_name: "search without string/bytes pattern".to_string(),
                    });
                }
            };
            Ok(read_search(buffer, offset, pattern_bytes, *range)?
                .unwrap_or_else(|| Value::String(String::new())))
        }
        TypeKind::Meta(meta) => Err(TypeReadError::UnsupportedType {
            type_name: format!("meta-type {meta:?} cannot be read as a value"),
        }),
    }
}

/// Engine entry point for pattern-bearing types (`Regex`, `Search`).
///
/// Returns `Ok(None)` on a genuine "no match" outcome and `Ok(Some(value))`
/// on a successful match -- including zero-width matches (e.g., regex `^`,
/// `a*`, or `.{0}`). This is the contract the evaluator needs to
/// distinguish a real miss from a zero-width hit; [`read_typed_value_with_pattern`]
/// collapses both cases to `Value::String(String::new())` for back-compat.
///
/// # Errors
///
/// Returns [`TypeReadError`] for:
///
/// * `BufferOverrun` when `offset >= buffer.len()`
/// * `UnsupportedType` if `type_kind` is not pattern-bearing, if the
///   pattern operand is missing, or if the pattern has the wrong
///   `Value` variant for the type
/// * `UnsupportedType` (via [`read_regex`]) if a regex pattern fails to
///   compile
pub(crate) fn read_pattern_match(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
    pattern: Option<&Value>,
) -> Result<Option<Value>, TypeReadError> {
    match type_kind {
        TypeKind::Regex { flags, count } => {
            let pattern_str = match pattern {
                Some(Value::String(s)) => s.as_str(),
                _ => {
                    return Err(TypeReadError::UnsupportedType {
                        type_name: "regex without string pattern".to_string(),
                    });
                }
            };
            read_regex(buffer, offset, pattern_str, *flags, *count)
        }
        TypeKind::Search { range } => {
            let pattern_bytes: &[u8] = match pattern {
                Some(Value::String(s)) => s.as_bytes(),
                Some(Value::Bytes(b)) => b.as_slice(),
                _ => {
                    return Err(TypeReadError::UnsupportedType {
                        type_name: "search without string/bytes pattern".to_string(),
                    });
                }
            };
            read_search(buffer, offset, pattern_bytes, *range)
        }
        TypeKind::Meta(meta) => Err(TypeReadError::UnsupportedType {
            type_name: format!("meta-type {meta:?} cannot be read as a pattern match"),
        }),
        _ => Err(TypeReadError::UnsupportedType {
            type_name: format!("read_pattern_match called on non-pattern type: {type_kind:?}"),
        }),
    }
}

/// Coerces a rule value to the signed width implied by `type_kind`.
///
/// Returns a [`Cow::Borrowed`] when no coercion is needed (the hot path for
/// most rule evaluations, e.g. string matching), and a [`Cow::Owned`] only
/// when the value must be transformed. This avoids an allocation on every
/// rule evaluation for `Value::String` and other pass-through cases.
///
/// # Examples
///
/// ```
/// use libmagic_rs::evaluator::types::coerce_value_to_type;
/// use libmagic_rs::parser::ast::{TypeKind, Value};
///
/// let coerced = coerce_value_to_type(&Value::Uint(0xff), &TypeKind::Byte { signed: true });
/// assert_eq!(*coerced, Value::Int(-1));
/// ```
#[must_use]
pub fn coerce_value_to_type<'a>(value: &'a Value, type_kind: &TypeKind) -> Cow<'a, Value> {
    match (value, type_kind) {
        (Value::Uint(v), TypeKind::Byte { signed: true }) if *v > i8::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Cow::Owned(Value::Int(i64::from(*v as u8 as i8)))
        }
        (Value::Uint(v), TypeKind::Short { signed: true, .. }) if *v > i16::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Cow::Owned(Value::Int(i64::from(*v as u16 as i16)))
        }
        (Value::Uint(v), TypeKind::Long { signed: true, .. }) if *v > i32::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Cow::Owned(Value::Int(i64::from(*v as u32 as i32)))
        }
        (Value::Uint(v), TypeKind::Quad { signed: true, .. }) if *v > i64::MAX as u64 =>
        {
            #[allow(clippy::cast_possible_wrap)]
            Cow::Owned(Value::Int(*v as i64))
        }
        // Round f64 expected value to f32 precision for TypeKind::Float so that
        // parsed f64 literals compare correctly against f32-widened file values.
        #[allow(clippy::cast_possible_truncation)]
        (Value::Float(v), TypeKind::Float { .. }) => Cow::Owned(Value::Float(f64::from(*v as f32))),
        // Normalize numeric expected values for date types into formatted timestamp
        // strings so they match the Value::String representation from read_date/read_qdate.
        (Value::Uint(v), TypeKind::Date { utc, .. } | TypeKind::QDate { utc, .. }) => {
            Cow::Owned(Value::String(format_timestamp_value(*v, *utc)))
        }
        #[allow(clippy::cast_sign_loss)]
        (Value::Int(v), TypeKind::Date { utc, .. } | TypeKind::QDate { utc, .. }) if *v >= 0 => {
            Cow::Owned(Value::String(format_timestamp_value(*v as u64, *utc)))
        }
        _ => Cow::Borrowed(value),
    }
}

/// Returns the anchor-advance distance for `type_kind` at `offset`, threading
/// the rule's value operand through for pattern-bearing types.
///
/// This value is used by the evaluation engine to advance the GNU `file`
/// "previous match" anchor for relative offset resolution. It reflects how
/// far the anchor should move after a successful match, which may include
/// framing bytes such as c-string NUL terminators or pstring length
/// prefixes even when the underlying read helper (`read_string`,
/// `read_pstring`) does not return those bytes as part of the typed value.
/// Callers should not equate this with "bytes `read_typed_value` returned"
/// -- it is specifically the anchor-movement distance, which is a
/// superset for variable-width types. It is `pub(crate)` because no
/// external caller should depend on this anchor-advance contract -- the
/// only intended caller is `evaluate_rules` in the engine.
///
/// The function is intentionally infallible. For unexpected inputs (offset
/// past end of buffer, malformed pstring prefix, `/J` flag underflow), it
/// returns `0` rather than panicking; the anchor then stays put and the
/// next relative offset will bounds-fail gracefully. The engine only calls
/// it after a successful read, so the defensive paths are belt-and-braces
/// for any future caller that breaks that invariant.
///
/// For `TypeKind::Regex` and `TypeKind::Search`, the pattern is required
/// at anchor-advance time to re-run the match and compute `m.end()` (or
/// `match_idx + pattern.len()` for search), matching GNU `file`'s
/// `softmagic.c` `FILE_REGEX` / `FILE_SEARCH` / `moffset()` semantics:
/// the anchor advances past the **matched bytes**, not past the entire
/// scan window. For regex, `flags.start_offset` (the `/s` flag) further
/// changes the advance to `m.start()` (match-start) instead of match-end.
/// When the pattern is unavailable or has the wrong `Value` variant, the
/// function returns `0` and fires a `debug_assert!` in dev/test builds
/// -- the engine invariant is that `bytes_consumed_with_pattern` is
/// called only after a successful `read_pattern_match`, which requires
/// a `Value::String`/`Value::Bytes` pattern. Non-pattern types should
/// pass `pattern: None`.
///
/// # Semantics
///
/// - **Fixed-width types** (Byte, Short, Long, Quad, Float, Double, Date,
///   QDate): returns `bit_width / 8` when the type's full width fits
///   inside the buffer at `offset`; returns `0` if `offset + width` would
///   exceed `buffer.len()`. This guard mirrors the variable-width path so
///   the anchor cannot advance past the end of the buffer regardless of
///   how the function is called.
/// - **C-string** (`TypeKind::String`): scans for the first NUL within a
///   window of `max_length` bytes (or to the buffer end if `max_length` is
///   `None`). When a NUL is found inside the window, returns
///   `nul_index + 1` -- the NUL byte is counted as consumed, so the next
///   relative offset reads the byte *after* the NUL. When no NUL is found
///   inside the window, returns the window size (no implicit terminator
///   byte is added). The NUL inclusion is intentional and matches GNU
///   `file` semantics: a `Relative(0)` rule following a NUL-terminated
///   string match reads the first byte after the terminator.
/// - **Pascal string** (`TypeKind::PString`): reads the length prefix (1, 2,
///   or 4 bytes, BE/LE), accounts for the `/J` flag (stored length includes
///   prefix width), caps by `max_length`, and returns `prefix_width +
///   actual_payload_bytes`. The result is also clamped against the remaining
///   buffer length so a malicious oversized length prefix cannot poison the
///   anchor.
#[must_use]
pub(crate) fn bytes_consumed_with_pattern(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
    pattern: Option<&Value>,
) -> usize {
    if let Some(bits) = type_kind.bit_width() {
        let width = (bits as usize) / 8;
        // Bounds-check the fixed-width path so a misuse (offset past end of
        // buffer, broken read-then-call invariant) cannot advance the
        // anchor past the buffer end. The engine guarantees a successful
        // read preceded the call, but the guard makes the contract
        // self-consistent for any future caller.
        return match offset.checked_add(width) {
            Some(end) if end <= buffer.len() => width,
            _ => 0,
        };
    }

    match type_kind {
        TypeKind::String { max_length } => {
            // For the (`max_length: None`, string literal pattern)
            // combination we now compare exactly `pattern.len()` bytes
            // in `read_typed_value_with_pattern` (libmagic semantics).
            // Keep the NUL-terminator inclusion that the chained-record
            // tests rely on by peeking at the byte immediately after
            // the pattern window: if it is NUL, consume one extra
            // byte; otherwise stop at the pattern boundary. Explicit
            // `max_length` rules and non-string patterns keep the
            // original NUL-scan behavior.
            match (max_length, pattern) {
                (Some(n), _) => string_bytes_consumed(buffer, offset, Some(*n)),
                (None, Some(Value::String(p))) => {
                    let plen = p.len();
                    let base = offset
                        .checked_add(plen)
                        .map_or(0, |end| if end > buffer.len() { 0 } else { plen });
                    if base == 0 {
                        0
                    } else {
                        match buffer.get(offset.saturating_add(plen)) {
                            Some(&0) => base.saturating_add(1),
                            _ => base,
                        }
                    }
                }
                (None, _) => string_bytes_consumed(buffer, offset, None),
            }
        }
        TypeKind::String16 { endian } => string16_bytes_consumed(buffer, offset, *endian),
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
        TypeKind::Regex { flags, count } => match pattern {
            Some(Value::String(s)) => {
                regex::regex_bytes_consumed(buffer, offset, s.as_str(), *flags, *count)
            }
            // Invariant: the engine only calls `bytes_consumed_with_pattern`
            // after a successful `read_typed_value_with_pattern`/`read_pattern_match`,
            // which requires `Some(Value::String(_))` for regex. If we land
            // here the invariant is broken by a new caller and the anchor
            // would silently stall instead of advancing. Fire a debug_assert
            // so the mismatch is caught in dev/test builds.
            other => {
                debug_assert!(
                    false,
                    "bytes_consumed_with_pattern: TypeKind::Regex without Value::String pattern ({other:?}) -- engine invariant violated"
                );
                0
            }
        },
        TypeKind::Search { range } => match pattern {
            Some(Value::String(s)) => {
                search::search_bytes_consumed(buffer, offset, s.as_bytes(), *range)
            }
            Some(Value::Bytes(b)) => {
                search::search_bytes_consumed(buffer, offset, b.as_slice(), *range)
            }
            other => {
                debug_assert!(
                    false,
                    "bytes_consumed_with_pattern: TypeKind::Search without Value::String/Bytes pattern ({other:?}) -- engine invariant violated"
                );
                0
            }
        },
        // Fixed-width variants are handled by the `bit_width()` fast
        // path above. Listing them here explicitly (rather than using
        // a `_ =>` wildcard) turns any future addition of a
        // variable-width `TypeKind` variant into a compile error
        // instead of a silent anchor corruption (review finding
        // S-M3/L5). `TypeKind` is `#[non_exhaustive]`, so this match
        // is only exhaustive inside this crate -- external callers
        // cannot add variants. When adding a new `TypeKind` variant,
        // either add it to the fixed-width `bit_width()` path or add
        // it to this match; GOTCHAS S2.1 catalogs the full checklist.
        TypeKind::Byte { .. }
        | TypeKind::Short { .. }
        | TypeKind::Long { .. }
        | TypeKind::Quad { .. }
        | TypeKind::Float { .. }
        | TypeKind::Double { .. }
        | TypeKind::Date { .. }
        | TypeKind::QDate { .. } => {
            debug_assert!(
                false,
                "bytes_consumed_with_pattern: fixed-width TypeKind variant {type_kind:?} should have been handled by the bit_width() fast path"
            );
            0
        }
        // Meta-type directives do not consume buffer bytes; the anchor
        // should not advance when a meta rule is encountered. Per the
        // GOTCHAS S2.1 checklist, listing them explicitly (rather than
        // relying on a `_ =>` wildcard) keeps the match exhaustive so
        // any future `TypeKind` variant triggers a compile error.
        TypeKind::Meta(_) => 0,
    }
}

/// Compute the anchor-advance distance for a successful c-string match.
///
/// Uses the same scan logic as `read_string`: it searches from `offset` for
/// the first NUL within `max_length` bytes (or to the end of the buffer
/// when `max_length` is `None`). Unlike the `Value::String` returned by
/// `read_string` (which excludes the NUL terminator from its length), this
/// helper counts the NUL terminator as consumed when one is found, so it
/// returns `length_to_nul + 1`. When no NUL is found (truncated by buffer
/// end or `max_length`), it returns `length_read` with no implicit
/// terminator byte added.
///
/// Counting the terminator is intentional for relative-offset anchoring: a
/// `Relative(0)` rule following a NUL-terminated string match resolves to
/// the byte *immediately after* the NUL terminator, not the NUL itself.
/// This matches GNU `file` semantics for chained record parsing. Do not
/// "fix" this to align with `read_string`'s byte count -- the asymmetry is
/// the point.
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
