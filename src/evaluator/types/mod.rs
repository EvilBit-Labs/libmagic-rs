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
pub(crate) use date::{read_date, read_qdate};
pub(crate) use float::{read_double, read_float};
pub(crate) use numeric::{read_byte, read_long, read_quad, read_short};
pub(crate) use regex::read_regex;
pub(crate) use search::read_search;
use string::string16_bytes_consumed;
pub(crate) use string::{read_pstring, read_string, read_string_exact, read_string16};

/// Swap the declared endianness of an endian-bearing [`TypeKind`] for the
/// magic(5) `use \^name` endian flip (issue #236).
///
/// This mirrors libmagic's `cvt_flip` in `src/softmagic.c` exactly: only the
/// explicit little/big-endian numeric, float, and date families are swapped
/// (`short`/`long`/`quad`/`float`/`double` and the `date`/`ldate`/`qdate`/
/// `qldate` families). `Endianness::Native` is left untouched (libmagic has
/// no `FILE_SHORT`/`FILE_LONG` case in `cvt_flip`), `String16` is deliberately
/// NOT flipped (also absent from `cvt_flip`), and every non-endian type is
/// returned unchanged. The `signed`/`utc` attributes are preserved.
///
/// The evaluator calls this at read time only when the `\^` flip is active for
/// the current subroutine body, so the common (unflipped) path never allocates
/// a flipped clone.
pub(crate) fn flip_type_endian(typ: &TypeKind) -> TypeKind {
    use crate::parser::ast::Endianness;

    /// Swap Little<->Big; leave Native alone (matches `cvt_flip`, which only
    /// has explicit BE/LE cases).
    const fn swap(e: Endianness) -> Endianness {
        match e {
            Endianness::Little => Endianness::Big,
            Endianness::Big => Endianness::Little,
            Endianness::Native => Endianness::Native,
        }
    }

    match *typ {
        TypeKind::Short { endian, signed } => TypeKind::Short {
            endian: swap(endian),
            signed,
        },
        TypeKind::Long { endian, signed } => TypeKind::Long {
            endian: swap(endian),
            signed,
        },
        TypeKind::Quad { endian, signed } => TypeKind::Quad {
            endian: swap(endian),
            signed,
        },
        TypeKind::Float { endian } => TypeKind::Float {
            endian: swap(endian),
        },
        TypeKind::Double { endian } => TypeKind::Double {
            endian: swap(endian),
        },
        TypeKind::Date { endian, utc } => TypeKind::Date {
            endian: swap(endian),
            utc,
        },
        TypeKind::QDate { endian, utc } => TypeKind::QDate {
            endian: swap(endian),
            utc,
        },
        // Byte, String, String16, PString, Regex, Search, Meta: unchanged.
        // (`String16` is intentionally absent from libmagic's `cvt_flip`.)
        ref other => other.clone(),
    }
}

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
    /// Genuine evaluator capability gap that must abort evaluation: a
    /// non-equality operator on a pattern-bearing type, a `Meta` variant read
    /// as a value, an unwired `TypeKind`, or a future gap. The `type_name`
    /// field carries a free-form description; callers treat it as an opaque
    /// diagnostic string. This variant is intentionally **not** part of the
    /// narrow graceful-skip allowlist (GOTCHAS S2.1) -- the two skippable
    /// conditions have their own variants ([`Self::MissingPatternOperand`],
    /// [`Self::RegexCompileError`]) so the engine matches variants, not
    /// strings.
    #[error("Unsupported type: {type_name}")]
    UnsupportedType {
        /// Free-form description of the offending type or failure condition.
        type_name: String,
    },
    /// A pattern-bearing type (`Regex`, `Search`, or a flagged `String`) was
    /// evaluated without a usable `String`/`Bytes` pattern operand. This is a
    /// narrow, allowlisted **non-match** condition (GOTCHAS S2.1/S2.4): the
    /// engine skips the rule (logged at `debug!`) rather than aborting the
    /// whole file. The `type_name` field names which type lacked the operand.
    /// Dedicated variant (issue #391 item 2) replacing the earlier
    /// string-keyed `UnsupportedType` allowlist so the skip contract is
    /// compiler-enforced.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::types::TypeReadError;
    /// let err = TypeReadError::MissingPatternOperand {
    ///     type_name: "regex without string pattern".to_string(),
    /// };
    /// assert_eq!(err.to_string(), "regex without string pattern");
    /// ```
    #[error("{type_name}")]
    MissingPatternOperand {
        /// Description of the pattern-bearing type that lacked an operand.
        type_name: String,
    },
    /// A `Regex` rule's pattern failed to compile, including the
    /// `REGEX_COMPILE_SIZE_LIMIT` (CWE-1333) denial-of-service guard. Narrow,
    /// allowlisted **non-match** condition (GOTCHAS S2.1): the engine skips
    /// the rule (logged at `warn!`, so a malicious or pathological pattern's
    /// rejection stays visible) rather than aborting. The `detail` field
    /// carries the underlying compiler error. Dedicated variant (issue #391
    /// item 2) replacing the earlier `"regex compile error:"` string prefix
    /// check.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::evaluator::types::TypeReadError;
    /// let err = TypeReadError::RegexCompileError {
    ///     detail: "regex parse error".to_string(),
    /// };
    /// assert_eq!(err.to_string(), "regex compile error: regex parse error");
    /// ```
    #[error("regex compile error: {detail}")]
    RegexCompileError {
        /// The underlying regex compiler error message.
        detail: String,
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

impl TypeReadError {
    /// Whether this error is one of the two narrow, allowlisted graceful-skip
    /// conditions (GOTCHAS S2.1): a missing pattern operand
    /// ([`Self::MissingPatternOperand`]) or a regex compile failure
    /// ([`Self::RegexCompileError`]). Every other cause -- notably
    /// [`Self::UnsupportedType`] (an unwired `TypeKind`, a non-equality
    /// operator on a pattern-bearing type, a `Meta` read as a value) -- is a
    /// genuine capability gap that MUST propagate and abort evaluation. The
    /// engine keys its narrow skip on this method, so widening it silently
    /// widens the S2.1 contract; do not add variants here without the same
    /// review discipline the string allowlist required.
    #[must_use]
    pub(crate) fn is_pattern_skip(&self) -> bool {
        matches!(
            self,
            Self::MissingPatternOperand { .. } | Self::RegexCompileError { .. }
        )
    }

    /// Whether this is specifically the regex-compile-failure skip condition
    /// (including the `REGEX_COMPILE_SIZE_LIMIT` CWE-1333 guard), which the
    /// engine logs at `warn!` rather than `debug!` so a malicious or
    /// pathological pattern's rejection stays visible (KTD5).
    #[must_use]
    pub(crate) fn is_regex_compile_failure(&self) -> bool {
        matches!(self, Self::RegexCompileError { .. })
    }
}

/// Diagnostic string for `TypeKind::Regex` evaluated without a usable
/// `String`/`Bytes` pattern operand. Carried as the `type_name` of
/// [`TypeReadError::MissingPatternOperand`] by every construction site so the
/// message stays single-sourced.
pub(crate) const REGEX_MISSING_PATTERN_MSG: &str = "regex without string pattern";

/// Diagnostic string for `TypeKind::Search` evaluated without a usable
/// `String`/`Bytes` pattern operand. See [`REGEX_MISSING_PATTERN_MSG`] for
/// the single-source-of-truth rationale.
pub(crate) const SEARCH_MISSING_PATTERN_MSG: &str = "search without string/bytes pattern";

/// Diagnostic string for a flagged `TypeKind::String` evaluated without a
/// usable `String`/`Bytes` pattern operand. See
/// [`REGEX_MISSING_PATTERN_MSG`] for the single-source-of-truth rationale.
pub(crate) const FLAGGED_STRING_MISSING_PATTERN_MSG: &str =
    "string with flags requires string/bytes pattern";

/// Default `max_string_length` used by [`read_typed_value`] when callers
/// do not supply an explicit cap. Matches
/// `EvaluationConfig::default().max_string_length` so call sites that
/// invoke `read_typed_value` directly see the same scan-mode bound the
/// engine applies at evaluation time. The engine call path
/// (`evaluate_value_rule`) threads the user-configured cap, so this
/// constant only governs internal helper / test usage.
#[allow(dead_code)]
pub(crate) const DEFAULT_MAX_STRING_LENGTH: usize = 8192;

/// Reads bytes according to the specified `TypeKind`.
///
/// This is the internal dispatch entry point for type reading for
/// non-pattern-bearing types. Fixed-width numeric, float, date, string,
/// and pstring types need no pattern operand, so the hot path stays
/// ergonomic.
///
/// For pattern-bearing types (`TypeKind::Regex`, `TypeKind::Search`) this
/// function will return `TypeReadError::MissingPatternOperand` because the
/// pattern operand is mandatory. Callers that need to evaluate regex/search
/// rules should use [`read_typed_value_with_pattern`] and thread the rule
/// value operand through as `pattern`.
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` when the requested value extends
/// past the buffer bounds, `TypeReadError::MissingPatternOperand` when a
/// pattern-bearing type is evaluated without a pattern, or
/// `TypeReadError::InvalidPStringLength` for a malformed Pascal string
/// length prefix.
///
/// This three-argument form defaults `max_string_length` to
/// [`DEFAULT_MAX_STRING_LENGTH`] (8192 bytes, matching
/// `EvaluationConfig::default()`). The engine's value-rule path supplies
/// the user-configured cap via [`read_typed_value_with_pattern`] directly,
/// so this helper exists for internal callers (tests, future fuzz
/// harnesses) that want a one-shot type-read without constructing a
/// context. The lib build doesn't currently call it; the `dead_code`
/// allow keeps the helper available for `#[cfg(test)]` modules without
/// gating its visibility, so a future fuzz harness can reuse it.
#[allow(dead_code)]
pub(crate) fn read_typed_value(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
) -> Result<Value, TypeReadError> {
    read_typed_value_with_pattern(buffer, offset, type_kind, None, DEFAULT_MAX_STRING_LENGTH)
}

/// Decodes a `Value::Bytes` regex pattern operand into a `String` for
/// compilation (KTD1/KTD6 of the fix-system-magic-regex-graceful plan).
///
/// Regex patterns are normally captured by the parser as `Value::String`,
/// but escape-heavy patterns (e.g. `\^[\040\t]{0,50}\\.asciiz`) can
/// currently be miscategorized as `Value::Bytes` by `parse_value`'s
/// hex/mixed-ascii branch. This backstop lets such patterns compile
/// instead of fatally erroring (GOTCHAS S2.4), mirroring the existing
/// `TypeKind::Search` arms' dual `String`/`Bytes` acceptance.
///
/// `String::from_utf8_lossy` never panics, but on a real substitution --
/// any byte `>= 0x80` that is not valid UTF-8 -- it silently replaces the
/// byte with U+FFFD while the target buffer is still matched against its
/// *raw* bytes. The two sides then diverge silently: the compiled regex
/// no longer represents the same bytes the file comparison expects. Detect
/// a real substitution with `str::from_utf8` first and `warn!` so the
/// divergence is visible in logs rather than a silent wrong answer.
fn decode_regex_bytes_pattern(bytes: &[u8]) -> String {
    if std::str::from_utf8(bytes).is_err() {
        // The pattern originates from an untrusted magic DB and can be
        // arbitrarily long, so log only its length and a bounded preview
        // rather than the whole buffer -- otherwise a pathological pattern
        // could flood logs / churn memory (CWE-117-adjacent). 32 bytes is
        // enough to identify the offending rule without dumping it.
        const PREVIEW_LEN: usize = 32;
        let preview = bytes.get(..PREVIEW_LEN.min(bytes.len())).unwrap_or(bytes);
        let truncated = if bytes.len() > PREVIEW_LEN { "..." } else { "" };
        log::warn!(
            "regex pattern given as {} raw bytes (preview: {preview:?}{truncated}) is not \
             valid UTF-8; lossily reinterpreting as text before compiling (bytes >= 0x80 \
             become U+FFFD and will not byte-match the target buffer)",
            bytes.len()
        );
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Reads bytes according to the specified `TypeKind`, threading a
/// `pattern` operand through for non-pattern-bearing types whose
/// dispatch arm consults the rule's value operand (e.g. `TypeKind::String`
/// equality matches against the literal pattern bytes).
///
/// This is the internal dispatch entry point for value-rule evaluation.
/// Pattern-bearing types (`TypeKind::Regex`, `TypeKind::Search`, and
/// flagged `TypeKind::String`) are routed through [`read_pattern_match`]
/// by the engine instead; this function returns
/// `TypeReadError::MissingPatternOperand` if called with those variants
/// without a pattern so a programmatic caller mis-routing them surfaces
/// immediately rather than silently producing wrong results.
///
/// # Errors
///
/// Returns `TypeReadError::BufferOverrun` when the requested value extends
/// past the buffer bounds, `TypeReadError::MissingPatternOperand` when a
/// pattern-bearing type is evaluated through this path without a pattern
/// operand, or `TypeReadError::InvalidPStringLength` for a
/// malformed Pascal string length prefix.
///
/// `max_string_length` bounds the scan-mode string read on the
/// `(None, _)` arm of [`TypeKind::String`]. Without it, `string x` rules
/// against an attacker-controlled NUL-free buffer could allocate up to
/// the full buffer length (CWE-770). The cap is wired from
/// `EvaluationContext::max_string_length` at the engine call site.
pub(crate) fn read_typed_value_with_pattern(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
    pattern: Option<&Value>,
    max_string_length: usize,
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
        TypeKind::String {
            max_length,
            flags: _,
        } => {
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
            //
            // Note: `flags` is bound and ignored here. Flagged-string
            // dispatch is handled separately via the pattern-bearing-type
            // branch in `read_pattern_match` (see GOTCHAS S2.4 for the
            // contract). When the flags are non-default, the engine
            // bypasses this read path and calls
            // `compare_string_with_flags` directly.
            match (max_length, pattern) {
                (Some(n), _) => read_string_exact(buffer, offset, *n),
                (None, Some(Value::String(p))) => read_string_exact(buffer, offset, p.len()),
                (None, Some(Value::Bytes(b))) => read_string_exact(buffer, offset, b.len()),
                // 2A-H1: thread the configured cap into the scan-mode read.
                // Without this, `string x` rules against attacker-controlled
                // NUL-free buffers could allocate up to the full buffer
                // length, defeating the CWE-770 control documented in
                // `EvaluationConfig::max_string_length`.
                (None, _) => read_string(buffer, offset, Some(max_string_length)),
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
            // Dual `String`/`Bytes` acceptance mirrors the `Search` arm
            // directly below (GOTCHAS S2.4). The `String` fast path stays
            // allocation-free via `Cow::Borrowed`; `Bytes` goes through
            // `decode_regex_bytes_pattern` (KTD6 lossy-substitution guard).
            let pattern_str: Cow<'_, str> = match pattern {
                Some(Value::String(s)) => Cow::Borrowed(s.as_str()),
                Some(Value::Bytes(b)) => Cow::Owned(decode_regex_bytes_pattern(b)),
                _ => {
                    return Err(TypeReadError::MissingPatternOperand {
                        type_name: REGEX_MISSING_PATTERN_MSG.to_string(),
                    });
                }
            };
            // Collapse `None` (no match) to `Value::String(String::new())`
            // for back-compat with callers using the single-Value return
            // shape. The engine path goes through `read_pattern_match`
            // directly and preserves the `Option` so it can distinguish a
            // zero-width match from a miss.
            Ok(read_regex(buffer, offset, &pattern_str, *flags, *count)?
                .unwrap_or_else(|| Value::String(String::new())))
        }
        TypeKind::Search { range, flags } => {
            let pattern_bytes: &[u8] = match pattern {
                Some(Value::String(s)) => s.as_bytes(),
                Some(Value::Bytes(b)) => b.as_slice(),
                _ => {
                    return Err(TypeReadError::MissingPatternOperand {
                        type_name: SEARCH_MISSING_PATTERN_MSG.to_string(),
                    });
                }
            };
            Ok(read_search(buffer, offset, pattern_bytes, *range, *flags)?
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
/// * `MissingPatternOperand` if the pattern operand is missing or has the
///   wrong `Value` variant for a pattern-bearing type
/// * `UnsupportedType` if `type_kind` is not pattern-bearing
/// * `RegexCompileError` (via [`read_regex`]) if a regex pattern fails to
///   compile
pub(crate) fn read_pattern_match(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
    pattern: Option<&Value>,
    max_string_length: usize,
) -> Result<Option<Value>, TypeReadError> {
    // Match the documented BufferOverrun contract uniformly across all
    // pattern-bearing paths. `read_regex` and `read_search` enforce this
    // guard internally; the flagged `TypeKind::String` arm below delegates
    // to `compare_string_with_flags`, which would silently return `None`
    // (no-match) for `offset >= buffer.len()`. Under `Operator::NotEqual`
    // an out-of-bounds read would then be reported as a successful match
    // -- a correctness hazard. Returning `BufferOverrun` here keeps the
    // three paths semantically aligned and lets the engine dispatcher
    // (`evaluate_pattern_rule`) reject the rule rather than infer truth
    // from an unread region.
    if offset >= buffer.len() {
        return Err(TypeReadError::BufferOverrun {
            offset,
            buffer_len: buffer.len(),
        });
    }
    match type_kind {
        TypeKind::Regex { flags, count } => {
            // Dual `String`/`Bytes` acceptance mirrors the `Search` arm
            // directly below (GOTCHAS S2.4); see `decode_regex_bytes_pattern`
            // for the KTD6 lossy-substitution `warn!` guard.
            let pattern_str: Cow<'_, str> = match pattern {
                Some(Value::String(s)) => Cow::Borrowed(s.as_str()),
                Some(Value::Bytes(b)) => Cow::Owned(decode_regex_bytes_pattern(b)),
                _ => {
                    return Err(TypeReadError::MissingPatternOperand {
                        type_name: REGEX_MISSING_PATTERN_MSG.to_string(),
                    });
                }
            };
            read_regex(buffer, offset, &pattern_str, *flags, *count)
        }
        TypeKind::Search { range, flags } => {
            let pattern_bytes: &[u8] = match pattern {
                Some(Value::String(s)) => s.as_bytes(),
                Some(Value::Bytes(b)) => b.as_slice(),
                _ => {
                    return Err(TypeReadError::MissingPatternOperand {
                        type_name: SEARCH_MISSING_PATTERN_MSG.to_string(),
                    });
                }
            };
            read_search(buffer, offset, pattern_bytes, *range, *flags)
        }
        // Flagged `string` rules go through the pattern-bearing-type
        // contract (GOTCHAS S2.4): on hit return `Some(Value::Bytes(
        // matched_bytes))`, on miss return `None`. The engine dispatcher
        // (`evaluate_pattern_rule`) translates Some/None into Equal/NotEqual
        // and rejects other operators on the rule.
        //
        // The value variant is `Value::Bytes` (not `Value::String`) because
        // libmagic semantics are byte-exact -- `from_utf8_lossy` would
        // silently replace invalid UTF-8 with U+FFFD and break `%s`
        // substitution. The cross-type String/Bytes equality policy in
        // GOTCHAS S2.3 keeps downstream comparisons consistent.
        //
        // Pattern can be `Value::String` (the common case) or `Value::Bytes`
        // (parser-emitted for backslash-escape literals like `\177ELF`).
        // The trim flag (`/T`) is honored here at evaluation time so the AST
        // construction stays unchanged.
        TypeKind::String { max_length, flags } if !flags.is_empty() => {
            let pattern_bytes: &[u8] = match pattern {
                Some(Value::String(s)) => s.as_bytes(),
                Some(Value::Bytes(b)) => b.as_slice(),
                _ => {
                    return Err(TypeReadError::MissingPatternOperand {
                        type_name: FLAGGED_STRING_MISSING_PATTERN_MSG.to_string(),
                    });
                }
            };
            let trimmed: &[u8] = if flags.trim {
                trim_ascii_whitespace(pattern_bytes)
            } else {
                pattern_bytes
            };
            // An empty (post-trim) pattern would silently match *any* file
            // because `compare_string_with_flags(b"", ...)` returns
            // `Some(0)` -- the same hazard documented in GOTCHAS S2.5 for
            // regex. Treat it as "no match" with a `warn!` so the
            // malformed rule surfaces in logs without breaking evaluation
            // of subsequent rules. Most commonly hit by `string/T "   "`
            // where the pattern is pure whitespace.
            if trimmed.is_empty() {
                log::warn!(
                    "flagged string rule has empty pattern (after /T trim); \
                     treating as no-match to avoid catastrophic over-matching"
                );
                return Ok(None);
            }
            // `max_length: Some(n)` caps the scan window to `n` bytes from
            // `offset`, matching the unflagged path's behavior. Without
            // this, flagged matches can read past the configured window
            // (e.g., `/w` chewing through whitespace beyond `n`). When the
            // window is shorter than the pattern, the comparison naturally
            // produces no match via `compare_string_with_flags`'s EOF
            // handling -- no special case needed.
            //
            // CWE-770: When AST `max_length` is `None`, fall back to the
            // configured `max_string_length` cap rather than passing the
            // full buffer. The cap is applied to the buffer's UPPER bound
            // (not pre-sliced from `offset`) because
            // `compare_string_with_flags` slices internally via
            // `buffer.get(offset..)?` -- pre-slicing would double-offset
            // and silently produce no-match at any non-zero offset.
            //
            // `end` is constructed with `saturating_add` then `.min(buffer.len())`
            // so the slice always satisfies `end <= buffer.len()`. We use
            // `buffer.get(..end).ok_or(BufferOverrun)` rather than direct
            // indexing to satisfy the project-wide ".get() for buffer access"
            // rule (AGENTS.md "Memory Safety First") while preserving the
            // SF-2 fail-loud posture: if a future refactor breaks the clamp
            // invariant, we surface a typed `BufferOverrun` to the engine
            // instead of silently falling back to the uncapped buffer --
            // which would defeat the CWE-770 control. The `ok_or` arm is
            // structurally unreachable under the current invariant; it
            // exists as defense-in-depth.
            let scan_buffer: &[u8] = {
                let cap = max_length.unwrap_or(max_string_length);
                let end = offset.saturating_add(cap).min(buffer.len());
                buffer.get(..end).ok_or(TypeReadError::BufferOverrun {
                    offset,
                    buffer_len: buffer.len(),
                })?
            };
            match string::compare_string_with_flags(trimmed, scan_buffer, offset, *flags) {
                Some(consumed) => {
                    let matched = scan_buffer
                        .get(offset..offset.saturating_add(consumed))
                        .unwrap_or(&[]);
                    Ok(Some(Value::Bytes(matched.to_vec())))
                }
                None => Ok(None),
            }
        }
        TypeKind::Meta(meta) => Err(TypeReadError::UnsupportedType {
            type_name: format!("meta-type {meta:?} cannot be read as a pattern match"),
        }),
        _ => Err(TypeReadError::UnsupportedType {
            type_name: format!("read_pattern_match called on non-pattern type: {type_kind:?}"),
        }),
    }
}

/// Anchor-advance count for a flagged `string` rule.
///
/// Flagged string rules go through the pattern-bearing-type contract (see
/// `read_pattern_match`), so their anchor advance is whatever
/// `compare_string_with_flags` consumed -- which can exceed `pattern.len()`
/// when `/w` or `/W` let the file have additional whitespace. Re-running
/// the comparison here recovers the consumed-bytes count without storing
/// it on the match value, matching the regex/search precedent.
///
/// **NUL-terminator inclusion**: when the byte immediately after the
/// matched region is `0x00`, the consumed count includes that NUL so
/// relative-offset children land *after* the terminator. This mirrors
/// the unflagged-string path in `bytes_consumed_with_pattern` and is the
/// behavior `relative_after_string_parent_includes_nul_terminator` pins
/// for the byte-exact path.
///
/// **`max_length` cap**: when `max_length: Some(n)` is set, the scan is
/// bounded to `n` bytes from `offset`, matching the unflagged path. The
/// NUL-terminator inclusion is also clamped to this window so we cannot
/// advance past the configured boundary.
fn flagged_string_bytes_consumed(
    buffer: &[u8],
    offset: usize,
    max_length: Option<usize>,
    flags: crate::parser::ast::StringFlags,
    pattern: Option<&Value>,
) -> usize {
    let pattern_bytes: &[u8] = match pattern {
        Some(Value::String(s)) => s.as_bytes(),
        Some(Value::Bytes(b)) => b.as_slice(),
        _ => {
            // The dispatcher (`bytes_consumed_with_pattern`) only routes here
            // when a string/bytes pattern is present (the Equal/NotEqual
            // pattern-match path), so this arm is not normally reachable. It is
            // handled defensively rather than with a `debug_assert!(false)`: a
            // panic in library code is forbidden by the no-panic policy, and a
            // flagged string reaching the consume side without a string pattern
            // (e.g. an AnyValue/ordering operator misrouted by a future change)
            // must degrade -- warn and return 0 (buffer-safe; the relative-
            // offset anchor simply does not advance for that rule) rather than
            // crash.
            log::warn!(
                "flagged_string_bytes_consumed: missing string/bytes pattern ({pattern:?}); \
                 relative-offset anchor will not advance for this rule"
            );
            return 0;
        }
    };
    let effective: &[u8] = if flags.trim {
        trim_ascii_whitespace(pattern_bytes)
    } else {
        pattern_bytes
    };
    let scan_buffer: &[u8] = if let Some(n) = max_length {
        let end = offset.saturating_add(n).min(buffer.len());
        buffer.get(..end).unwrap_or(buffer)
    } else {
        buffer
    };
    let consumed =
        string::compare_string_with_flags(effective, scan_buffer, offset, flags).unwrap_or(0);
    if consumed == 0 {
        return 0;
    }
    // Mirror the unflagged path: peek the byte immediately after the
    // matched region. If it is NUL, include it in the anchor advance so
    // relative-offset children resolve past the terminator. Bounded by
    // the same scan window, so a `max_length`-clamped match cannot
    // accidentally cross the cap.
    let after = offset.saturating_add(consumed);
    match scan_buffer.get(after) {
        Some(&0) => consumed.saturating_add(1),
        _ => consumed,
    }
}

/// Trim leading and trailing ASCII whitespace from a byte slice.
///
/// Used for the `/T` (`STRING_TRIM`) flag on `string` rules. ASCII-only
/// trim matches libmagic's `isspace`-based contract; full Unicode
/// whitespace handling is out of scope.
// Slicing is invariant-safe: `start <= end <= s.len()` by construction
// (`position`/`rposition` results).
#[allow(clippy::indexing_slicing)]
fn trim_ascii_whitespace(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    &s[start..end]
}

/// Coerces a rule value to the signed width implied by `type_kind`.
///
/// Returns a [`Cow::Borrowed`] when no coercion is needed (the hot path for
/// most rule evaluations, e.g. string matching), and a [`Cow::Owned`] only
/// when the value must be transformed. This avoids an allocation on every
/// rule evaluation for `Value::String` and other pass-through cases.
///
#[must_use]
pub(crate) fn coerce_value_to_type<'a>(value: &'a Value, type_kind: &TypeKind) -> Cow<'a, Value> {
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
        // `bit_width()` returns multiples of 8, so the division is exact.
        #[allow(clippy::integer_division)]
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
        TypeKind::String { max_length, flags } => {
            // Route to the flag-aware consumer ONLY when there is a string/bytes
            // pattern to walk -- i.e. the Equal/NotEqual pattern-match path.
            // For an AnyValue (`x`) or ordering operator on a flagged string
            // (e.g. `0 string/b x`, `>15 string/t >\0`), the engine reads the
            // string via the plain value path (the /t, /b, /c... flags do not
            // change how many bytes are consumed), so fall through to the normal
            // string logic. Without this guard the pattern walker hit its
            // missing-pattern branch and panicked in debug builds. Mirrors the
            // engine's `TypeKind::String { flags }` operator-based dispatch.
            if !flags.is_empty() && matches!(pattern, Some(Value::String(_) | Value::Bytes(_))) {
                return flagged_string_bytes_consumed(buffer, offset, *max_length, *flags, pattern);
            }
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
                // `Value::Bytes` patterns reach this arm for backslash-escape
                // values like `\177ELF` (parsed via `parse_mixed_hex_ascii`).
                // The read path uses `read_string_exact(buffer, offset,
                // b.len())`, so the consume side must match -- otherwise the
                // relative-offset anchor mis-advances by the NUL-scan length
                // (which on a NUL-free ELF header is hundreds of bytes past
                // the actual match end). This is the dual-purpose-helper-
                // sync rule documented in GOTCHAS S6.4 / docs/solutions/
                // logic-errors/magic-string-rule-matching-3-bug-fix.
                (None, Some(Value::Bytes(b))) => {
                    let blen = b.len();
                    offset
                        .checked_add(blen)
                        .map_or(0, |end| if end > buffer.len() { 0 } else { blen })
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
            // U1 (fix-system-magic-regex-graceful): `Value::Bytes` regex
            // patterns are now accepted by `read_typed_value_with_pattern`/
            // `read_pattern_match` (GOTCHAS S2.4), so this consume-side
            // helper must mirror that acceptance -- otherwise a
            // successful Bytes-pattern regex match would hit the
            // `debug_assert` below and silently fail to advance the
            // anchor. Decode via the same `decode_regex_bytes_pattern`
            // helper used by the read paths so both sides agree on what
            // the pattern text is.
            Some(Value::Bytes(b)) => {
                let decoded = decode_regex_bytes_pattern(b);
                regex::regex_bytes_consumed(buffer, offset, &decoded, *flags, *count)
            }
            // Invariant: the engine only calls `bytes_consumed_with_pattern`
            // after a successful `read_typed_value_with_pattern`/`read_pattern_match`,
            // which requires `Some(Value::String(_) | Value::Bytes(_))` for
            // regex, so this arm is not normally reachable. Handle it
            // defensively rather than with a `debug_assert!(false)` (which
            // would panic in dev/test, violating the no-panic policy and the
            // `prop_arbitrary_rule_evaluation_never_panics` invariant): `warn!`
            // so a release build's silent stale-anchor is visible in logs, and
            // return 0 (buffer-safe; the relative-offset anchor simply does not
            // advance for this rule). Mirrors `flagged_string_bytes_consumed`.
            other => {
                log::warn!(
                    "bytes_consumed_with_pattern: TypeKind::Regex without Value::String/Bytes pattern ({other:?}); \
                     relative-offset anchor will not advance for this rule"
                );
                0
            }
        },
        TypeKind::Search { range, flags } => match pattern {
            Some(Value::String(s)) => {
                search::search_bytes_consumed(buffer, offset, s.as_bytes(), *range, *flags)
            }
            Some(Value::Bytes(b)) => {
                search::search_bytes_consumed(buffer, offset, b.as_slice(), *range, *flags)
            }
            // Same invariant and defensive rationale as the `Regex` arm above:
            // `warn!` + return 0 rather than `debug_assert!(false)`, so a
            // release build's silent stale-anchor is visible in logs.
            other => {
                log::warn!(
                    "bytes_consumed_with_pattern: TypeKind::Search without Value::String/Bytes pattern ({other:?}); \
                     relative-offset anchor will not advance for this rule"
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
// Indexing is invariant-safe: `len_bytes` is exactly `width >= 1` bytes,
// validated by the `checked_add` + `get` above.
#[allow(clippy::indexing_slicing)]
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
