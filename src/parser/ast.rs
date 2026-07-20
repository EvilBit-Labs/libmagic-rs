// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Abstract Syntax Tree definitions for magic rules
//!
//! This module contains the core data structures that represent parsed magic rules
//! and their components, including offset specifications, type kinds, operators, and values.

use serde::{Deserialize, Serialize};
use std::num::{NonZeroU32, NonZeroUsize};

/// The width of the length prefix for Pascal strings.
///
/// Uppercase suffix letters (`/H`, `/L`) indicate big-endian byte order.
/// Lowercase suffix letters (`/h`, `/l`) indicate little-endian byte order.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::PStringLengthWidth;
/// let width = PStringLengthWidth::OneByte;
/// assert_eq!(width.byte_count(), 1);
///
/// let width = PStringLengthWidth::TwoByteBE;
/// assert_eq!(width.byte_count(), 2);
///
/// let width = PStringLengthWidth::FourByteLE;
/// assert_eq!(width.byte_count(), 4);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
pub enum PStringLengthWidth {
    /// 1-byte length prefix (default, `/B` suffix)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::PStringLengthWidth;
    /// let width = PStringLengthWidth::OneByte;
    /// assert_eq!(width.byte_count(), 1);
    /// ```
    OneByte,
    /// 2-byte big-endian length prefix (`/H` suffix)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::PStringLengthWidth;
    /// let width = PStringLengthWidth::TwoByteBE;
    /// assert_eq!(width.byte_count(), 2);
    /// ```
    TwoByteBE,
    /// 2-byte little-endian length prefix (`/h` suffix)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::PStringLengthWidth;
    /// let width = PStringLengthWidth::TwoByteLE;
    /// assert_eq!(width.byte_count(), 2);
    /// ```
    TwoByteLE,
    /// 4-byte big-endian length prefix (`/L` suffix)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::PStringLengthWidth;
    /// let width = PStringLengthWidth::FourByteBE;
    /// assert_eq!(width.byte_count(), 4);
    /// ```
    FourByteBE,
    /// 4-byte little-endian length prefix (`/l` suffix)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::PStringLengthWidth;
    /// let width = PStringLengthWidth::FourByteLE;
    /// assert_eq!(width.byte_count(), 4);
    /// ```
    FourByteLE,
}

impl PStringLengthWidth {
    /// Returns the number of bytes used for the length prefix.
    #[must_use]
    pub fn byte_count(self) -> usize {
        match self {
            Self::OneByte => 1,
            Self::TwoByteBE | Self::TwoByteLE => 2,
            Self::FourByteBE | Self::FourByteLE => 4,
        }
    }
}

/// Arithmetic operation applied to the value read at an indirect offset's
/// `base_offset` before the result is used as the final file offset.
///
/// magic(5) supports `+`, `-`, `*`, `/`, `%`, `&`, `|`, and `^` between the
/// pointer-type specifier and the operand inside the parentheses. Addition
/// and subtraction collapse to [`IndirectAdjustmentOp::Add`] with a signed
/// `adjustment` (so `(N.X-1)` is `Add(-1)` rather than a separate `Sub`
/// variant); the remaining operators each have a dedicated variant.
///
/// The default is [`IndirectAdjustmentOp::Add`]; an indirect offset with no
/// arithmetic — just `(base.type)` — is encoded as `Add` with `adjustment:
/// 0`, preserving backwards compatibility.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
///
/// assert_eq!(IndirectAdjustmentOp::default(), IndirectAdjustmentOp::Add);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum IndirectAdjustmentOp {
    /// Addition (also covers subtraction via negative `adjustment`).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// assert_eq!(IndirectAdjustmentOp::default(), IndirectAdjustmentOp::Add);
    /// ```
    #[default]
    Add,
    /// Multiplication: `pointer_value * adjustment`.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::Mul;
    /// assert_eq!(op, IndirectAdjustmentOp::Mul);
    /// ```
    Mul,
    /// Truncating integer division: `pointer_value / adjustment`. Division
    /// by zero is rejected by the evaluator with an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::Div;
    /// assert_eq!(op, IndirectAdjustmentOp::Div);
    /// ```
    Div,
    /// Remainder: `pointer_value % adjustment`. Modulo by zero is rejected
    /// by the evaluator with an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::Mod;
    /// assert_eq!(op, IndirectAdjustmentOp::Mod);
    /// ```
    Mod,
    /// Bitwise AND: `pointer_value & adjustment`.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::And;
    /// assert_eq!(op, IndirectAdjustmentOp::And);
    /// ```
    And,
    /// Bitwise OR: `pointer_value | adjustment`.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::Or;
    /// assert_eq!(op, IndirectAdjustmentOp::Or);
    /// ```
    Or,
    /// Bitwise XOR: `pointer_value ^ adjustment`.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::IndirectAdjustmentOp;
    /// let op = IndirectAdjustmentOp::Xor;
    /// assert_eq!(op, IndirectAdjustmentOp::Xor);
    /// ```
    Xor,
}

/// Offset specification for locating data in files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum OffsetSpec {
    /// Absolute offset from file start (or from file end if negative)
    ///
    /// Positive values are offsets from the start of the file.
    /// Negative values are offsets from the end of the file (same as `FromEnd`).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let offset = OffsetSpec::Absolute(0x10); // Read at byte 16 from start
    /// let from_end = OffsetSpec::Absolute(-4); // 4 bytes before end of file
    /// ```
    Absolute(i64),

    /// Indirect offset through pointer dereferencing
    ///
    /// Reads a pointer value at `base_offset`, interprets it according to `pointer_type`
    /// and `endian`, then combines `adjustment` with the pointer value using
    /// `adjustment_op` to get the final offset. The default `adjustment_op`
    /// is [`IndirectAdjustmentOp::Add`], so `(base.type)` and
    /// `(base.type+N)` / `(base.type-N)` use addition (subtraction is
    /// encoded as `Add` with a negative `adjustment`). magic(5) also
    /// supports multiplicative and bitwise forms inside the parens, e.g.
    /// `(0x200.s*2)` ([`IndirectAdjustmentOp::Mul`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{OffsetSpec, TypeKind, Endianness, IndirectAdjustmentOp};
    ///
    /// let indirect = OffsetSpec::Indirect {
    ///     base_offset: 0x20,
    ///     base_relative: false,
    ///     pointer_type: TypeKind::Long { endian: Endianness::Little, signed: false },
    ///     adjustment: 4,
    ///     adjustment_op: IndirectAdjustmentOp::Add,
    ///     result_relative: false,
    ///     endian: Endianness::Little,
    /// };
    /// ```
    Indirect {
        /// Base offset to read pointer from. When `base_relative` is
        /// `true`, this value is added to the current anchor (last-match
        /// position) rather than being treated as an absolute file
        /// position.
        base_offset: i64,
        /// If `true`, `base_offset` is relative to the current anchor
        /// (i.e., `(&N.X)` syntax in magic files). Defaults to `false`
        /// for backwards compatibility with existing AST snapshots; the
        /// serde `default` attribute lets older serialized AST round-trip.
        #[serde(default)]
        base_relative: bool,
        /// Type of pointer value
        pointer_type: TypeKind,
        /// Operand combined with the pointer value via `adjustment_op`.
        ///
        /// For `IndirectAdjustmentOp::Add`, the operand is signed (negative
        /// values encode subtraction). For multiplicative and bitwise ops
        /// the operand is interpreted as `i64` but typically magic files
        /// supply non-negative literals.
        adjustment: i64,
        /// Arithmetic operation applied to the pointer value with
        /// `adjustment` as the operand. Defaults to
        /// [`IndirectAdjustmentOp::Add`] for legacy AST consumers via
        /// serde's `default` attribute.
        #[serde(default)]
        adjustment_op: IndirectAdjustmentOp,
        /// If `true`, the resolved offset is added to the current anchor
        /// instead of being treated as an absolute file position. This
        /// corresponds to magic-file `&(...)` syntax wrapping an indirect
        /// spec, e.g., `&(0x10.l)`.
        #[serde(default)]
        result_relative: bool,
        /// Endianness for pointer reading
        endian: Endianness,
    },

    /// Relative offset from previous match position
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let relative = OffsetSpec::Relative(8); // 8 bytes after previous match
    /// ```
    Relative(i64),

    /// Offset from end of file (negative values move towards start)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::OffsetSpec;
    ///
    /// let from_end = OffsetSpec::FromEnd(-16); // 16 bytes before end of file
    /// ```
    FromEnd(i64),
}

/// Control-flow directive carried by [`TypeKind::Meta`].
///
/// These are not value-reading types -- they correspond to magic(5)
/// control-flow keywords (`default`, `clear`, `name`, `use`, `indirect`,
/// `offset`) that modify how a rule set is traversed rather than reading
/// bytes from the buffer. All six variants are fully evaluated by the
/// engine: `default`/`clear` manage per-level sibling-matched state;
/// `name`/`use` implement subroutine dispatch; `indirect` re-applies the
/// root rule database at a resolved offset; and `offset` emits the
/// current file position as `Value::Uint` for printf-style formatting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetaType {
    /// `default` directive: fires when no sibling at the same indentation
    /// level has matched at the current offset. See magic(5) for the
    /// "default" type semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Default;
    /// assert_eq!(meta, MetaType::Default);
    /// ```
    Default,
    /// `clear` directive: resets the sibling-matched flag so a later
    /// `default` sibling can fire even if an earlier sibling matched.
    /// See magic(5) for the "clear" type semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Clear;
    /// assert_eq!(meta, MetaType::Clear);
    /// ```
    Clear,
    /// `name <identifier>` directive: declares a named subroutine that
    /// can be invoked later via [`MetaType::Use`]. See magic(5) for the
    /// "name" type semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Name("part2".to_string());
    /// assert_eq!(meta, MetaType::Name("part2".to_string()));
    /// ```
    Name(String),
    /// `use <identifier>` directive: invokes a named subroutine
    /// previously declared via [`MetaType::Name`]. See magic(5) for the
    /// "use" type semantics.
    ///
    /// `flip_endian` records the magic(5) `\^` prefix (`use \^name`),
    /// which flips the endianness of every endian-bearing read inside the
    /// invoked subroutine (libmagic `softmagic.c` `cvt_flip`; the flip
    /// toggles and propagates into nested `use` calls). A plain `use name`
    /// has `flip_endian: false`. See the evaluator's `flip_type_endian`
    /// and the `SubroutineScope` guard for the runtime semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Use { name: "part2".to_string(), flip_endian: false };
    /// assert_eq!(
    ///     meta,
    ///     MetaType::Use { name: "part2".to_string(), flip_endian: false }
    /// );
    /// ```
    Use {
        /// Identifier of the named subroutine to invoke.
        name: String,
        /// Whether to flip endianness of reads inside the subroutine
        /// (the magic(5) `\^` prefix).
        flip_endian: bool,
    },
    /// `indirect` directive: re-applies the entire magic database at the
    /// resolved offset. See magic(5) for the "indirect" type semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Indirect;
    /// assert_eq!(meta, MetaType::Indirect);
    /// ```
    Indirect,
    /// `offset` type keyword: reports the current file offset rather than
    /// reading a typed value from the buffer. See magic(5) for the
    /// "offset" type semantics.
    ///
    /// Evaluation: the engine resolves the rule's offset specification
    /// to an absolute position and emits a `RuleMatch` whose `value` is
    /// `Value::Uint(position)`. Message templates can reference that
    /// value through printf-style format specifiers (e.g. `%lld`),
    /// which are substituted by
    /// [`crate::output::format::format_magic_message`] at description-
    /// assembly time. The only supported operator is `x` (`AnyValue`);
    /// any other operator is `debug!`-logged and skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::MetaType;
    /// let meta = MetaType::Offset;
    /// assert_eq!(meta, MetaType::Offset);
    /// ```
    Offset,
}

/// Data type specifications for interpreting bytes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeKind {
    /// Single byte
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::TypeKind;
    ///
    /// let byte = TypeKind::Byte { signed: true };
    /// assert_eq!(byte, TypeKind::Byte { signed: true });
    /// ```
    Byte {
        /// Whether value is signed
        signed: bool,
    },
    /// 16-bit integer
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let short = TypeKind::Short { endian: Endianness::Little, signed: true };
    /// assert_eq!(short, TypeKind::Short { endian: Endianness::Little, signed: true });
    /// ```
    Short {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// 32-bit integer
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let long = TypeKind::Long { endian: Endianness::Big, signed: false };
    /// assert_eq!(long, TypeKind::Long { endian: Endianness::Big, signed: false });
    /// ```
    Long {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// 64-bit integer
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let quad = TypeKind::Quad { endian: Endianness::Big, signed: true };
    /// assert_eq!(quad, TypeKind::Quad { endian: Endianness::Big, signed: true });
    /// ```
    Quad {
        /// Byte order
        endian: Endianness,
        /// Whether value is signed
        signed: bool,
    },
    /// 32-bit IEEE 754 floating-point
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let float = TypeKind::Float { endian: Endianness::Big };
    /// assert_eq!(float, TypeKind::Float { endian: Endianness::Big });
    /// ```
    Float {
        /// Byte order
        endian: Endianness,
    },
    /// 64-bit IEEE 754 double-precision floating-point
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let double = TypeKind::Double { endian: Endianness::Big };
    /// assert_eq!(double, TypeKind::Double { endian: Endianness::Big });
    /// ```
    Double {
        /// Byte order
        endian: Endianness,
    },
    /// 32-bit Unix timestamp (seconds since epoch)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let date = TypeKind::Date { endian: Endianness::Big, utc: true };
    /// assert_eq!(date, TypeKind::Date { endian: Endianness::Big, utc: true });
    /// ```
    Date {
        /// Byte order
        endian: Endianness,
        /// true = UTC, false = local time
        utc: bool,
    },
    /// 64-bit Unix timestamp (seconds since epoch)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let qdate = TypeKind::QDate { endian: Endianness::Little, utc: false };
    /// assert_eq!(qdate, TypeKind::QDate { endian: Endianness::Little, utc: false });
    /// ```
    QDate {
        /// Byte order
        endian: Endianness,
        /// true = UTC, false = local time
        utc: bool,
    },
    /// String data
    ///
    /// The `flags` field carries the modifier flags parsed from the
    /// `/[cCwWtTbf]` suffix on a `string` rule. Default flags (all
    /// `false`) preserve the existing byte-exact comparison path; any
    /// non-default flag routes the rule through
    /// `compare_string_with_flags` in `src/evaluator/types/string.rs`.
    /// See [`StringFlags`] for per-flag semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{StringFlags, TypeKind};
    ///
    /// let s = TypeKind::String { max_length: None, flags: StringFlags::default() };
    /// assert_eq!(s, TypeKind::String { max_length: None, flags: StringFlags::default() });
    ///
    /// let case_insensitive = TypeKind::String {
    ///     max_length: None,
    ///     flags: StringFlags::default().with_ignore_lowercase(true),
    /// };
    /// assert!(matches!(case_insensitive, TypeKind::String { flags, .. } if flags.ignore_lowercase));
    /// ```
    String {
        /// Maximum length to read
        max_length: Option<usize>,
        /// Modifier flags from the `/[cCwWtTbf]` suffix
        flags: StringFlags,
    },
    /// UCS-2 (16-bit Unicode) string with explicit byte order.
    ///
    /// Backs the magic(5) `lestring16` (little-endian) and `bestring16`
    /// (big-endian) keywords. Each character occupies two bytes in the
    /// file; the reader stops at a U+0000 terminator (encoded as the
    /// 2-byte sequence `0x00 0x00`) or at the end of the buffer. The
    /// decoded value is returned as a Rust `String` (so non-ASCII
    /// characters are preserved when valid UCS-2).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, Endianness};
    ///
    /// let le = TypeKind::String16 { endian: Endianness::Little };
    /// assert_eq!(le, TypeKind::String16 { endian: Endianness::Little });
    ///
    /// let be = TypeKind::String16 { endian: Endianness::Big };
    /// assert_eq!(be, TypeKind::String16 { endian: Endianness::Big });
    /// ```
    String16 {
        /// Endianness for the 16-bit code units.
        endian: Endianness,
    },
    /// Pascal string (length-prefixed, supports 1/2/4-byte prefix, with optional max length)
    ///
    /// Pascal strings store the length as a prefix (1, 2, or 4 bytes, with configurable endianness), followed by
    /// that many bytes of string data. Unlike C strings, they are not null-terminated.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{TypeKind, PStringLengthWidth};
    ///
    /// let pstring = TypeKind::PString { max_length: None, length_width: PStringLengthWidth::OneByte, length_includes_itself: false };
    /// assert_eq!(pstring, TypeKind::PString { max_length: None, length_width: PStringLengthWidth::OneByte, length_includes_itself: false });
    ///
    /// let limited = TypeKind::PString { max_length: Some(64), length_width: PStringLengthWidth::TwoByteBE, length_includes_itself: false };
    /// assert_eq!(limited, TypeKind::PString { max_length: Some(64), length_width: PStringLengthWidth::TwoByteBE, length_includes_itself: false });
    ///
    /// // /J flag: stored length includes the length field itself
    /// let jpeg = TypeKind::PString { max_length: None, length_width: PStringLengthWidth::TwoByteBE, length_includes_itself: true };
    /// assert_eq!(jpeg, TypeKind::PString { max_length: None, length_width: PStringLengthWidth::TwoByteBE, length_includes_itself: true });
    /// ```
    PString {
        /// Maximum length to read (caps the length value)
        max_length: Option<usize>,
        /// Width of the length prefix
        length_width: PStringLengthWidth,
        /// Whether the stored length includes the length field itself (`/J` flag)
        length_includes_itself: bool,
    },
    /// Regular expression matching against file contents
    ///
    /// Regex rules match a POSIX-extended regular expression pattern against the
    /// file buffer. Patterns are compiled with multi-line mode always enabled
    /// (matching libmagic's unconditional `REG_NEWLINE`), so `^` and `$` match
    /// at line boundaries and `.` does not match `\n`. The `flags` control
    /// case sensitivity and anchor advance semantics; the `count` field
    /// controls the scan window (byte or line bounds). The scan window is
    /// always capped at 8192 bytes (matching GNU `file`'s `FILE_REGEX_MAX`;
    /// enforced in the evaluator).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{RegexCount, RegexFlags, TypeKind};
    /// use std::num::NonZeroU32;
    ///
    /// // Plain `regex` -- no flags, default 8192-byte scan window.
    /// let plain = TypeKind::Regex {
    ///     flags: RegexFlags::default(),
    ///     count: RegexCount::Default,
    /// };
    ///
    /// // `regex/1l` -- scan the first line only.
    /// let first_line = TypeKind::Regex {
    ///     flags: RegexFlags::default(),
    ///     count: RegexCount::Lines(NonZeroU32::new(1)),
    /// };
    ///
    /// // `regex/cs` -- case-insensitive, anchor advances to match-start.
    /// let case_insensitive_start = TypeKind::Regex {
    ///     flags: RegexFlags::default()
    ///         .with_case_insensitive(true)
    ///         .with_start_offset(true),
    ///     count: RegexCount::Default,
    /// };
    /// ```
    Regex {
        /// Modifier flags from the `/[cs]` suffix (`/c` case-insensitive,
        /// `/s` start-offset anchor). Line-mode is encoded by the
        /// [`RegexCount::Lines`] variant of `count`, not a flag.
        flags: RegexFlags,
        /// Scan window specifier: default 8192 bytes, explicit byte
        /// count, or explicit line count. See [`RegexCount`] for the
        /// three cases.
        count: RegexCount,
    },
    /// Multi-byte pattern search within a bounded (or open) range
    ///
    /// Search rules look for a literal byte pattern starting at the offset.
    /// Unlike [`TypeKind::String`], which only matches at the exact offset,
    /// `search` scans forward for the first occurrence. The window is
    /// controlled by `range`:
    ///
    /// * `Some(n)` -- the `search/N` form -- scans at most `n` bytes. `n` is
    ///   a [`NonZeroUsize`] so `search/0` is unrepresentable.
    /// * `None` -- bare `search` with no `/N` suffix -- scans from the offset
    ///   to end-of-buffer. magic(5) documents the count as required, but the
    ///   reference `file` binary accepts the bare form and treats it as
    ///   `str_range == 0` (scan-to-EOF), so we follow the implementation.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::TypeKind;
    /// use std::num::NonZeroUsize;
    ///
    /// // `search/256` -- scan up to 256 bytes for the literal pattern.
    /// let bounded = TypeKind::Search {
    ///     range: NonZeroUsize::new(256),
    ///     flags: libmagic_rs::parser::ast::SearchFlags::default(),
    /// };
    ///
    /// // bare `search` -- scan the whole remaining buffer.
    /// let open = TypeKind::Search {
    ///     range: None,
    ///     flags: libmagic_rs::parser::ast::SearchFlags::default(),
    /// };
    /// ```
    Search {
        /// Scan window width in bytes starting at the rule's offset;
        /// `None` means scan to end-of-buffer (bare `search`).
        range: Option<NonZeroUsize>,
        /// Modifier flags from the `/[sCcWwTtBbf]` suffix on a `search`
        /// rule. The `/s` flag controls anchor advance (match-START vs
        /// match-END); the eight `StringFlags`-shared letters alter how
        /// the literal pattern is compared against the file bytes. See
        /// [`SearchFlags`] for the per-flag semantics.
        flags: SearchFlags,
    },
    /// Control-flow directive (`default`, `clear`, `name`, `use`,
    /// `indirect`, `offset`).
    ///
    /// These magic(5) keywords do not read or compare bytes; they modify
    /// how a rule set is traversed. All six variants are fully evaluated:
    /// `default` fires as a fallback when no sibling at the same level
    /// has matched; `clear` resets that flag; `name`/`use` support
    /// subroutine definition and invocation; `indirect` re-enters the
    /// rule set at a resolved offset; `offset` emits the resolved file
    /// position as `Value::Uint` for printf-style message substitution.
    /// See [`MetaType`] for the individual variants.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{MetaType, TypeKind};
    /// let default_rule = TypeKind::Meta(MetaType::Default);
    /// assert_eq!(default_rule, TypeKind::Meta(MetaType::Default));
    /// ```
    Meta(MetaType),
}

/// Regex modifier flags parsed from the `/[cs]` suffix on a `regex` rule.
///
/// The `/l` "line-based window" modifier is **not** represented here; it
/// lives on [`RegexCount::Lines`] so that the type-level encoding makes
/// "line count" and "byte count" mutually exclusive. An earlier design
/// used two separate fields (`line_based: bool` + `count: Option<u32>`)
/// which admitted the cross-field state `line_based: true, count: None`;
/// under the current encoding that case is expressed explicitly as
/// [`RegexCount::Lines(None)`](RegexCount::Lines) -- the `regex/l`
/// shorthand -- and is behaviorally equivalent to [`RegexCount::Default`]
/// (both walk the full 8192-byte capped window).
///
/// All flags default to `false` via [`RegexFlags::default`], equivalent
/// to a plain `regex` with no `/c` or `/s` suffix.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::RegexFlags;
///
/// let plain = RegexFlags::default();
/// assert!(!plain.case_insensitive);
/// assert!(!plain.start_offset);
///
/// let case_and_start = RegexFlags::default()
///     .with_case_insensitive(true)
///     .with_start_offset(true);
/// assert!(case_and_start.case_insensitive);
/// assert!(case_and_start.start_offset);
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct RegexFlags {
    /// `/c` -- case-insensitive matching. When `true`, ASCII letter
    /// casing is ignored during pattern matching.
    pub case_insensitive: bool,
    /// `/s` -- advance the GNU `file` previous-match anchor to the start
    /// of the matched region instead of its end. Matches libmagic's
    /// `REGEX_OFFSET_START` flag, which zeros the length contribution in
    /// `moffset()` for `FILE_REGEX`. Useful for chaining child rules that
    /// need to re-match from the position where the parent regex began.
    pub start_offset: bool,
}

impl RegexFlags {
    /// Builder-style setter for [`RegexFlags::case_insensitive`] (`/c`).
    ///
    /// Chain after [`RegexFlags::default()`] to construct `RegexFlags`
    /// values without exhaustive struct literals. If a new flag is
    /// added to `RegexFlags` in the future, callers using the builder
    /// form keep compiling; callers using struct literals would need
    /// an update.
    #[must_use]
    pub const fn with_case_insensitive(mut self, value: bool) -> Self {
        self.case_insensitive = value;
        self
    }

    /// Builder-style setter for [`RegexFlags::start_offset`] (`/s`).
    ///
    /// Chain after [`RegexFlags::default()`] to construct `RegexFlags`
    /// values without exhaustive struct literals.
    #[must_use]
    pub const fn with_start_offset(mut self, value: bool) -> Self {
        self.start_offset = value;
        self
    }
}

/// String modifier flags parsed from the `/[cCwWtTbf]` suffix on a `string`
/// rule.
///
/// Mirrors libmagic's `STRING_*` flag bits from `src/file.h`. Each flag
/// alters how `compare_string_with_flags` walks the pattern and buffer in
/// parallel. The default (all `false`) preserves byte-exact comparison.
///
/// **`/c` vs `/C` are asymmetric**: the pattern character controls
/// direction. With `/c`, only lowercase pattern chars trigger case-folding
/// (the file byte is `tolower`'d). With `/C`, only uppercase pattern chars
/// trigger folding (the file byte is `toupper`'d). Mixed-case patterns
/// behave intuitively: `/c FoO` matches `FoO`, `Foo`, `FOO` but not
/// `fOO` (the uppercase `F` is literal). See GOTCHAS S6.5 for the
/// rationale and `src/softmagic.c` for the canonical libmagic contract.
///
/// **`/B` is NOT a string flag** -- it is the `pstring` 1-byte length-width
/// letter (`PSTRING_1_BE`). `string/B` is rejected at parse time. See
/// GOTCHAS S6.6.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::StringFlags;
///
/// let plain = StringFlags::default();
/// assert!(!plain.ignore_lowercase);
///
/// let case_insensitive = StringFlags::default().with_ignore_lowercase(true);
/// assert!(case_insensitive.ignore_lowercase);
///
/// let compound = StringFlags::default()
///     .with_ignore_lowercase(true)
///     .with_compact_optional_whitespace(true);
/// assert!(compound.ignore_lowercase);
/// assert!(compound.compact_optional_whitespace);
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
// libmagic's contract is naturally a bitfield: each flag is a distinct
// magic(5) letter (/c, /C, /w, /W, /t, /T, /b, /f) with its own STRING_*
// constant in libmagic src/file.h. Flags compose freely (string/cw is
// /c plus /w; string/wcCtTbf sets all eight). Folding pairs into enums
// is possible (whitespace: none|optional|required; case: none|lower|upper)
// but would obscure the libmagic mapping and produce verbose match arms
// in every consumer. The bool-per-flag layout mirrors `RegexFlags` and
// the libmagic source -- the clippy lint is overruled by the design.
#[allow(clippy::struct_excessive_bools)]
#[non_exhaustive]
pub struct StringFlags {
    /// `/W` -- `STRING_COMPACT_WHITESPACE`. Pattern whitespace requires at
    /// least one whitespace byte in the file, then any further whitespace
    /// in the file is consumed greedily.
    pub compact_whitespace: bool,
    /// `/w` -- `STRING_COMPACT_OPTIONAL_WHITESPACE`. Pattern whitespace
    /// matches zero or more whitespace bytes in the file.
    pub compact_optional_whitespace: bool,
    /// `/c` -- `STRING_IGNORE_LOWERCASE`. When the pattern char is
    /// lowercase, the file byte is `to_ascii_lowercase`'d before
    /// comparison. Uppercase pattern chars are compared literally.
    pub ignore_lowercase: bool,
    /// `/C` -- `STRING_IGNORE_UPPERCASE`. When the pattern char is
    /// uppercase, the file byte is `to_ascii_uppercase`'d before
    /// comparison. Lowercase pattern chars are compared literally.
    pub ignore_uppercase: bool,
    /// `/t` -- `STRING_TEXTTEST`. Hint that this rule applies to text
    /// files. Captured for MIME-output integration; does not currently
    /// alter comparison.
    pub text_test: bool,
    /// `/T` -- `STRING_TRIM`. Trim leading and trailing ASCII whitespace
    /// from the pattern before comparison. The trim is applied at
    /// evaluation time (in `read_pattern_match`) so the AST keeps the
    /// original pattern bytes; the comparison function receives the
    /// trimmed slice.
    pub trim: bool,
    /// `/b` -- `STRING_BINTEST`. Hint that this rule applies to binary
    /// files. Captured for MIME-output integration; does not currently
    /// alter comparison.
    pub bin_test: bool,
    /// `/f` -- `STRING_FULL_WORD`. Post-match check that the byte after
    /// the matched region is either end-of-buffer or a non-word
    /// character (ASCII alphanumeric or `_`).
    pub full_word: bool,
}

impl StringFlags {
    /// Returns `true` when every flag is `false` (the byte-exact fast
    /// path). The evaluator dispatcher uses this to skip the
    /// parallel-walk comparison when no flags are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.compact_whitespace
            && !self.compact_optional_whitespace
            && !self.ignore_lowercase
            && !self.ignore_uppercase
            && !self.text_test
            && !self.trim
            && !self.bin_test
            && !self.full_word
    }

    /// Builder-style setter for `compact_whitespace` (`/W`).
    #[must_use]
    pub const fn with_compact_whitespace(mut self, value: bool) -> Self {
        self.compact_whitespace = value;
        self
    }

    /// Builder-style setter for `compact_optional_whitespace` (`/w`).
    #[must_use]
    pub const fn with_compact_optional_whitespace(mut self, value: bool) -> Self {
        self.compact_optional_whitespace = value;
        self
    }

    /// Builder-style setter for `ignore_lowercase` (`/c`).
    #[must_use]
    pub const fn with_ignore_lowercase(mut self, value: bool) -> Self {
        self.ignore_lowercase = value;
        self
    }

    /// Builder-style setter for `ignore_uppercase` (`/C`).
    #[must_use]
    pub const fn with_ignore_uppercase(mut self, value: bool) -> Self {
        self.ignore_uppercase = value;
        self
    }

    /// Builder-style setter for `text_test` (`/t`).
    #[must_use]
    pub const fn with_text_test(mut self, value: bool) -> Self {
        self.text_test = value;
        self
    }

    /// Builder-style setter for `trim` (`/T`).
    #[must_use]
    pub const fn with_trim(mut self, value: bool) -> Self {
        self.trim = value;
        self
    }

    /// Builder-style setter for `bin_test` (`/b`).
    #[must_use]
    pub const fn with_bin_test(mut self, value: bool) -> Self {
        self.bin_test = value;
        self
    }

    /// Builder-style setter for `full_word` (`/f`).
    #[must_use]
    pub const fn with_full_word(mut self, value: bool) -> Self {
        self.full_word = value;
        self
    }
}

/// Search modifier flags parsed from the `/[sCcWwTtBbf]` suffix on a
/// `search` rule.
///
/// Mirrors [`StringFlags`] for the eight `STRING_*` letters that alter
/// the literal-pattern comparison (`/c`, `/C`, `/w`, `/W`, `/t`, `/T`,
/// `/b`, `/f`), plus a search-only `start_anchor` field for `/s` which
/// shifts the GNU `file` previous-match anchor to the START of the
/// matched region. The default (all `false`) preserves byte-exact
/// comparison and match-END anchor advance.
///
/// `SearchFlags` is structurally parallel to `StringFlags`: when one
/// struct grows a field, the other gains the same field in lockstep
/// so that [`SearchFlags::to_string_flags`] can keep handing off to
/// `compare_string_with_flags` without a generic refactor. The
/// search-only `start_anchor` field has no analog in `string` rules.
///
/// **`/c` vs `/C` are asymmetric** in the same way as [`StringFlags`]:
/// the pattern character controls fold direction. See [`StringFlags`]
/// and GOTCHAS S6.5 for the rationale.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::SearchFlags;
///
/// let plain = SearchFlags::default();
/// assert!(!plain.start_anchor);
/// assert!(plain.is_empty());
/// assert!(!plain.needs_byte_compare());
///
/// let start = SearchFlags::default().with_start_anchor(true);
/// assert!(start.start_anchor);
/// assert!(!start.is_empty());
/// // /s is anchor-only -- does not force the byte-compare slow path.
/// assert!(!start.needs_byte_compare());
///
/// let case = SearchFlags::default().with_ignore_lowercase(true);
/// assert!(case.needs_byte_compare());
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
// libmagic's contract is naturally a bitfield: each flag is a distinct
// magic(5) letter with its own STRING_*/SEARCH_* constant in libmagic
// src/file.h. Flags compose freely (search/cs is /c plus /s; search/sWcT
// sets four). Folding pairs into enums is possible but would obscure
// the libmagic mapping and produce verbose match arms in every consumer.
// The bool-per-flag layout mirrors `StringFlags` and `RegexFlags` and the
// libmagic source -- the clippy lint is overruled by the design.
#[allow(clippy::struct_excessive_bools)]
#[non_exhaustive]
pub struct SearchFlags {
    /// `/W` -- `STRING_COMPACT_WHITESPACE`. Pattern whitespace requires at
    /// least one whitespace byte in the file, then any further whitespace
    /// in the file is consumed greedily.
    pub compact_whitespace: bool,
    /// `/w` -- `STRING_COMPACT_OPTIONAL_WHITESPACE`. Pattern whitespace
    /// matches zero or more whitespace bytes in the file.
    pub compact_optional_whitespace: bool,
    /// `/c` -- `STRING_IGNORE_LOWERCASE`. When the pattern char is
    /// lowercase, the file byte is `to_ascii_lowercase`'d before
    /// comparison. Uppercase pattern chars are compared literally.
    pub ignore_lowercase: bool,
    /// `/C` -- `STRING_IGNORE_UPPERCASE`. When the pattern char is
    /// uppercase, the file byte is `to_ascii_uppercase`'d before
    /// comparison. Lowercase pattern chars are compared literally.
    pub ignore_uppercase: bool,
    /// `/t` -- `STRING_TEXTTEST`. Hint that this rule applies to text
    /// files. Captured for MIME-output integration; does not currently
    /// alter comparison.
    pub text_test: bool,
    /// `/T` -- `STRING_TRIM`. Trim leading and trailing ASCII whitespace
    /// from the pattern before comparison.
    pub trim: bool,
    /// `/b` -- `STRING_BINTEST`. Hint that this rule applies to binary
    /// files. Captured for MIME-output integration; does not currently
    /// alter comparison.
    pub bin_test: bool,
    /// `/f` -- `STRING_FULL_WORD`. Post-match check that the byte after
    /// the matched region is either end-of-buffer or a non-word
    /// character (ASCII alphanumeric or `_`).
    pub full_word: bool,
    /// `/s` -- magic(5) "search-start" flag. When `true`, the GNU `file`
    /// previous-match anchor advance lands on the match-START index
    /// rather than match-END (the default). Mirrors libmagic's
    /// `FILE_SEARCH` anchor handling in `src/softmagic.c::moffset`. The
    /// dispatch happens in
    /// `src/evaluator/types/search.rs::search_bytes_consumed`.
    pub start_anchor: bool,
}

impl SearchFlags {
    /// Returns `true` when every flag is `false` (default-constructed).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.compact_whitespace
            && !self.compact_optional_whitespace
            && !self.ignore_lowercase
            && !self.ignore_uppercase
            && !self.text_test
            && !self.trim
            && !self.bin_test
            && !self.full_word
            && !self.start_anchor
    }

    /// Returns `true` when any flag alters the literal-pattern
    /// comparison, forcing the byte-walk slow path through
    /// `compare_string_with_flags`. The anchor-only / metadata-only
    /// flags (`/s`, `/t`, `/b`) do **not** trigger byte-compare;
    /// they preserve the `memchr::memmem::find` fast path.
    #[must_use]
    pub const fn needs_byte_compare(self) -> bool {
        self.compact_whitespace
            || self.compact_optional_whitespace
            || self.ignore_lowercase
            || self.ignore_uppercase
            || self.trim
            || self.full_word
    }

    /// Project the eight shared flag fields onto a [`StringFlags`] for
    /// handoff to `compare_string_with_flags`. The search-only
    /// `start_anchor` field is dropped (it is anchor-advance policy,
    /// not comparison policy).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::SearchFlags;
    ///
    /// let sf = SearchFlags::default()
    ///     .with_ignore_lowercase(true)
    ///     .with_trim(true)
    ///     .with_start_anchor(true);
    /// let projected = sf.to_string_flags();
    /// assert!(projected.ignore_lowercase);
    /// assert!(projected.trim);
    /// // /s has no analog in StringFlags.
    /// ```
    #[must_use]
    pub const fn to_string_flags(self) -> StringFlags {
        StringFlags {
            compact_whitespace: self.compact_whitespace,
            compact_optional_whitespace: self.compact_optional_whitespace,
            ignore_lowercase: self.ignore_lowercase,
            ignore_uppercase: self.ignore_uppercase,
            text_test: self.text_test,
            trim: self.trim,
            bin_test: self.bin_test,
            full_word: self.full_word,
        }
    }

    /// Builder-style setter for `compact_whitespace` (`/W`).
    #[must_use]
    pub const fn with_compact_whitespace(mut self, value: bool) -> Self {
        self.compact_whitespace = value;
        self
    }

    /// Builder-style setter for `compact_optional_whitespace` (`/w`).
    #[must_use]
    pub const fn with_compact_optional_whitespace(mut self, value: bool) -> Self {
        self.compact_optional_whitespace = value;
        self
    }

    /// Builder-style setter for `ignore_lowercase` (`/c`).
    #[must_use]
    pub const fn with_ignore_lowercase(mut self, value: bool) -> Self {
        self.ignore_lowercase = value;
        self
    }

    /// Builder-style setter for `ignore_uppercase` (`/C`).
    #[must_use]
    pub const fn with_ignore_uppercase(mut self, value: bool) -> Self {
        self.ignore_uppercase = value;
        self
    }

    /// Builder-style setter for `text_test` (`/t`).
    #[must_use]
    pub const fn with_text_test(mut self, value: bool) -> Self {
        self.text_test = value;
        self
    }

    /// Builder-style setter for `trim` (`/T`).
    #[must_use]
    pub const fn with_trim(mut self, value: bool) -> Self {
        self.trim = value;
        self
    }

    /// Builder-style setter for `bin_test` (`/b`).
    #[must_use]
    pub const fn with_bin_test(mut self, value: bool) -> Self {
        self.bin_test = value;
        self
    }

    /// Builder-style setter for `full_word` (`/f`).
    #[must_use]
    pub const fn with_full_word(mut self, value: bool) -> Self {
        self.full_word = value;
        self
    }

    /// Builder-style setter for `start_anchor` (`/s`).
    #[must_use]
    pub const fn with_start_anchor(mut self, value: bool) -> Self {
        self.start_anchor = value;
        self
    }
}

/// Scan window specifier for a [`TypeKind::Regex`] rule.
///
/// Encodes the three mutually-exclusive scan modes in a single enum so
/// that the "byte count" and "line count" cases cannot be confused. The
/// `regex/l` shorthand (line mode with no explicit count) is represented
/// explicitly as [`RegexCount::Lines(None)`](RegexCount::Lines), which
/// is behaviorally equivalent to [`RegexCount::Default`] -- both walk
/// the full 8192-byte capped window -- but preserves the magic-file
/// surface syntax of the original rule. The 8192-byte hard cap
/// (matching GNU `file`'s `FILE_REGEX_MAX`) is applied by the evaluator
/// on every variant.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::RegexCount;
/// use std::num::NonZeroU32;
///
/// // Plain `regex` (no suffix): default 8192-byte window.
/// assert_eq!(RegexCount::default(), RegexCount::Default);
///
/// // `regex/100`: scan at most 100 bytes.
/// let hundred_bytes = RegexCount::Bytes(NonZeroU32::new(100).unwrap());
///
/// // `regex/1l`: scan the first line.
/// let one_line = RegexCount::Lines(NonZeroU32::new(1));
///
/// // `regex/l`: line-mode with no explicit count (walks terminators
/// // to the end of the 8192-byte capped window).
/// let unbounded_lines = RegexCount::Lines(None);
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RegexCount {
    /// No scan bound (plain `regex` with no suffix). Scans the default
    /// 8192-byte window from the rule's offset.
    #[default]
    Default,
    /// Byte-bounded scan (`regex/N` with no `/l` flag). The window is
    /// `min(n, 8192, remaining_buffer)` bytes long. `NonZeroU32` makes
    /// a zero-byte scan unrepresentable.
    Bytes(NonZeroU32),
    /// Line-bounded scan (`regex/Nl` or `regex/l`). The window walks
    /// LF / CRLF / bare CR line terminators from the offset. With
    /// `Some(n)`, the walk stops after the Nth terminator (inclusive).
    /// With `None` (the `regex/l` shorthand), the walk continues to
    /// the end of the 8192-byte capped window. Either way the
    /// effective byte window is capped at 8192.
    Lines(Option<NonZeroU32>),
}

impl TypeKind {
    /// Returns the bit width of integer types, or `None` for non-integer types (e.g., String).
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::{Endianness, StringFlags, TypeKind};
    ///
    /// assert_eq!(TypeKind::Byte { signed: false }.bit_width(), Some(8));
    /// assert_eq!(TypeKind::Short { endian: Endianness::Native, signed: true }.bit_width(), Some(16));
    /// assert_eq!(TypeKind::Long { endian: Endianness::Native, signed: true }.bit_width(), Some(32));
    /// assert_eq!(TypeKind::Quad { endian: Endianness::Native, signed: true }.bit_width(), Some(64));
    /// assert_eq!(TypeKind::Float { endian: Endianness::Native }.bit_width(), Some(32));
    /// assert_eq!(TypeKind::Double { endian: Endianness::Native }.bit_width(), Some(64));
    /// assert_eq!(TypeKind::String { max_length: None, flags: StringFlags::default() }.bit_width(), None);
    /// ```
    #[must_use]
    pub const fn bit_width(&self) -> Option<u32> {
        match self {
            Self::Byte { .. } => Some(8),
            Self::Short { .. } => Some(16),
            Self::Long { .. } | Self::Float { .. } | Self::Date { .. } => Some(32),
            Self::Quad { .. } | Self::Double { .. } | Self::QDate { .. } => Some(64),
            Self::String { .. }
            | Self::String16 { .. }
            | Self::PString { .. }
            | Self::Regex { .. }
            | Self::Search { .. }
            | Self::Meta(_) => None,
        }
    }
}

/// Comparison and bitwise operators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum Operator {
    /// Equality comparison (`=` or `==`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::Equal;
    /// assert_eq!(op, Operator::Equal);
    /// ```
    Equal,
    /// Inequality comparison (`!=` or `<>`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::NotEqual;
    /// assert_eq!(op, Operator::NotEqual);
    /// ```
    NotEqual,
    /// Less-than comparison (`<`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::LessThan;
    /// assert_eq!(op, Operator::LessThan);
    /// ```
    LessThan,
    /// Greater-than comparison (`>`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::GreaterThan;
    /// assert_eq!(op, Operator::GreaterThan);
    /// ```
    GreaterThan,
    /// Less-than-or-equal comparison (`<=`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::LessEqual;
    /// assert_eq!(op, Operator::LessEqual);
    /// ```
    LessEqual,
    /// Greater-than-or-equal comparison (`>=`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::GreaterEqual;
    /// assert_eq!(op, Operator::GreaterEqual);
    /// ```
    GreaterEqual,
    /// Bitwise AND operation without mask (`&`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::BitwiseAnd;
    /// assert_eq!(op, Operator::BitwiseAnd);
    /// ```
    BitwiseAnd,
    /// Bitwise AND operation with mask value (`&` with a mask operand)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::BitwiseAndMask(0xFF00);
    /// assert_eq!(op, Operator::BitwiseAndMask(0xFF00));
    /// ```
    BitwiseAndMask(u64),
    /// Bitwise XOR operation (`^`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::BitwiseXor;
    /// assert_eq!(op, Operator::BitwiseXor);
    /// ```
    BitwiseXor,
    /// Bitwise NOT/complement operation (`~`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::BitwiseNot;
    /// assert_eq!(op, Operator::BitwiseNot);
    /// ```
    BitwiseNot,
    /// Match any value; condition always succeeds (`x`)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Operator;
    ///
    /// let op = Operator::AnyValue;
    /// assert_eq!(op, Operator::AnyValue);
    /// ```
    AnyValue,
}

/// Value types for rule matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Value {
    /// Unsigned integer value
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let val = Value::Uint(0xDEAD_BEEF);
    /// assert_eq!(val, Value::Uint(0xDEAD_BEEF));
    /// ```
    Uint(u64),
    /// Signed integer value
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let val = Value::Int(-42);
    /// assert_eq!(val, Value::Int(-42));
    /// ```
    Int(i64),
    /// Floating-point value (used for `float` and `double` types)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let val = Value::Float(3.14);
    /// assert_eq!(val, Value::Float(3.14));
    /// ```
    Float(f64),
    /// Byte sequence
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let val = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
    /// assert_eq!(val, Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]));
    /// ```
    Bytes(Vec<u8>),
    /// String value
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Value;
    ///
    /// let val = Value::String("MZ".to_string());
    /// assert_eq!(val, Value::String("MZ".to_string()));
    /// ```
    String(String),
}

/// Endianness specification for multi-byte values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Endianness {
    /// Little-endian byte order (least significant byte first)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Endianness;
    ///
    /// let e = Endianness::Little;
    /// assert_eq!(e, Endianness::Little);
    /// ```
    Little,
    /// Big-endian byte order (most significant byte first)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Endianness;
    ///
    /// let e = Endianness::Big;
    /// assert_eq!(e, Endianness::Big);
    /// ```
    Big,
    /// Native system byte order (matches target architecture)
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::parser::ast::Endianness;
    ///
    /// let e = Endianness::Native;
    /// assert_eq!(e, Endianness::Native);
    /// ```
    Native,
}

/// Strength modifier for magic rules
///
/// Strength modifiers adjust the default strength calculation for a rule.
/// They are specified using the `!:strength` directive in magic files.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::StrengthModifier;
///
/// let add = StrengthModifier::Add(10);      // !:strength +10
/// let sub = StrengthModifier::Subtract(5);  // !:strength -5
/// let mul = StrengthModifier::Multiply(2);  // !:strength *2
/// let div = StrengthModifier::Divide(2);    // !:strength /2
/// let set = StrengthModifier::Set(50);      // !:strength =50
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrengthModifier {
    /// Add to the default strength: `!:strength +N`
    Add(i32),
    /// Subtract from the default strength: `!:strength -N`
    Subtract(i32),
    /// Multiply the default strength: `!:strength *N`
    Multiply(i32),
    /// Divide the default strength: `!:strength /N`
    Divide(i32),
    /// Set strength to an absolute value: `!:strength =N` or `!:strength N`
    Set(i32),
}

/// Arithmetic operation applied to a value read from the file *before* the
/// rule's comparison operator is evaluated.
///
/// magic(5) supports `+`, `-`, `*`, `/`, `%`, `|`, and `^` between the type
/// keyword and the comparison value (e.g., `lelong+1 x volume %d` reads a
/// long, adds 1, and formats the transformed value into the message).
/// Bitwise AND (`&MASK`) is *not* part of this enum because it is already
/// represented at the operator level via [`Operator::BitwiseAndMask`].
///
/// The operand is signed (`i64`) so that subtraction and negative multipliers
/// round-trip cleanly. Bitwise ops reinterpret the operand as a `u64` bit
/// pattern at evaluation time, matching libmagic's `apprentice.c::mconvert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ValueTransformOp {
    /// Addition (`type+N`).
    Add,
    /// Subtraction (`type-N`).
    Sub,
    /// Multiplication (`type*N`).
    Mul,
    /// Truncating integer division (`type/N`). Division by zero is rejected
    /// at evaluation time.
    Div,
    /// Remainder (`type%N`). Modulo by zero is rejected at evaluation time.
    Mod,
    /// Bitwise AND (`type&N`).
    ///
    /// magic(5) `&MASK` was historically encoded at the operator level
    /// via [`Operator::BitwiseAndMask`] (which combines mask+equal in
    /// one step). That encoding cannot represent rules like `lelong&0xff
    /// x %d` (mask + any-value, with the masked value used in format
    /// substitution). The parser promotes `&MASK` to this `BitAnd`
    /// transform when followed by another operator (`x`, `>`, `!=`, ...)
    /// so the read value is masked before comparison and before printf
    /// substitution. The legacy `&MASK VALUE` form (mask + implicit
    /// equal) keeps using `Operator::BitwiseAndMask` for backwards
    /// compatibility.
    BitAnd,
    /// Bitwise OR (`type|N`).
    Or,
    /// Bitwise XOR (`type^N`).
    Xor,
}

/// A pre-comparison value transform: `(op, operand)`.
///
/// Applied to the value read from the file before the rule's comparison
/// operator runs. See [`ValueTransformOp`] for the supported operations.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::{ValueTransform, ValueTransformOp};
///
/// // `lelong+1` -> add 1 to the read value
/// let t = ValueTransform::new(ValueTransformOp::Add, 1);
/// assert_eq!(t.op, ValueTransformOp::Add);
/// assert_eq!(t.operand, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValueTransform {
    /// Operation to apply.
    pub op: ValueTransformOp,
    /// Operand to combine with the read value.
    pub operand: i64,
}

impl ValueTransform {
    /// Construct a new `ValueTransform` from an op and an operand.
    #[must_use]
    pub const fn new(op: ValueTransformOp, operand: i64) -> Self {
        Self { op, operand }
    }
}

/// Magic rule representation in the AST
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MagicRule {
    /// Offset specification for where to read data
    pub offset: OffsetSpec,
    /// Type of data to read and interpret
    pub typ: TypeKind,
    /// Comparison operator to apply
    pub op: Operator,
    /// Expected value for comparison
    pub value: Value,
    /// Human-readable message for this rule
    pub message: String,
    /// Child rules that are evaluated if this rule matches
    pub children: Vec<MagicRule>,
    /// Indentation level for hierarchical rules
    pub level: u32,
    /// Optional strength modifier from `!:strength` directive
    pub strength_modifier: Option<StrengthModifier>,
    /// Optional pre-comparison value transform from a magic-file
    /// type-suffix like `lelong+1` or `ulequad/1073741824`. When set,
    /// the read value is transformed *before* `op` is evaluated and
    /// before the message's `%`-format substitution, so format
    /// specifiers see the post-transform number.
    ///
    /// `#[serde(default)]` keeps existing serialized AST snapshots
    /// (which never had this field) round-tripping correctly: missing
    /// fields deserialize to `None`, which means "no transform" --
    /// the historical behavior.
    #[serde(default)]
    pub value_transform: Option<ValueTransform>,
}

/// Validation errors returned by [`MagicRule::validate`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum MagicRuleValidationError {
    /// Rule message is empty. Messages are user-facing and required
    /// for meaningful output.
    #[error("rule message must not be empty")]
    EmptyMessage,

    /// The child rule at `child_index` has `level <= self.level`,
    /// violating the "children must nest deeper than the parent"
    /// invariant of the hierarchical indentation-based DSL.
    #[error(
        "child rule at index {child_index} has level {child_level}, \
         must be greater than parent level {parent_level}"
    )]
    InvalidChildLevel {
        /// Index of the offending child in `self.children`.
        child_index: usize,
        /// Level of the child rule.
        child_level: u32,
        /// Level of the parent rule.
        parent_level: u32,
    },

    /// Rule `level` exceeds the maximum supported depth. The limit is a
    /// hardening mechanism against stack overflow during deep recursion;
    /// libmagic files in the wild rarely go beyond 10 levels.
    #[error("rule level {level} exceeds maximum supported depth {max}")]
    LevelTooDeep {
        /// The invalid level value.
        level: u32,
        /// The maximum allowed depth.
        max: u32,
    },
}

impl MagicRule {
    /// Hard structural ceiling on rule `level`.
    ///
    /// This is a conservative upper bound enforced by
    /// [`MagicRule::validate`] to keep the AST shape sane: real
    /// magic files in the wild rarely exceed ~10 levels of nesting,
    /// so rejecting rules with `level > 1000` catches obviously
    /// pathological input at construction time without constraining
    /// any legitimate rule.
    ///
    /// This ceiling is **independent of** the evaluator's
    /// `EvaluationConfig::max_recursion_depth` (default 20), which
    /// is the *runtime* recursion guard applied during rule
    /// evaluation. The evaluator limit is the first one that fires
    /// in practice -- a rule tree with 50 levels passes this
    /// structural check but is aborted by the evaluator long before
    /// reaching `MAX_LEVEL`. The two limits serve different purposes:
    /// `MAX_LEVEL` is an AST-shape sanity check, and
    /// `max_recursion_depth` is a per-evaluation resource bound.
    pub const MAX_LEVEL: u32 = 1000;

    /// Construct a top-level rule with no children and no strength
    /// modifier.
    ///
    /// This is the most common constructor for programmatically building
    /// rules outside the parser. To add children, mutate
    /// [`MagicRule::children`] directly, or use [`MagicRule::with_children`].
    /// To set a strength modifier, use
    /// [`MagicRule::with_strength_modifier`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};
    ///
    /// let rule = MagicRule::new(
    ///     OffsetSpec::Absolute(0),
    ///     TypeKind::Byte { signed: false },
    ///     Operator::Equal,
    ///     Value::Uint(0x7f),
    ///     "ELF magic byte".to_string(),
    /// );
    /// assert_eq!(rule.level, 0);
    /// assert!(rule.children.is_empty());
    /// assert!(rule.validate().is_ok());
    /// ```
    #[must_use]
    pub fn new(
        offset: OffsetSpec,
        typ: TypeKind,
        op: Operator,
        value: Value,
        message: String,
    ) -> Self {
        Self {
            offset,
            typ,
            op,
            value,
            message,
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        }
    }

    /// Replace `self.children` with the given children and return the
    /// modified rule. Builder-style for chaining.
    #[must_use]
    pub fn with_children(mut self, children: Vec<MagicRule>) -> Self {
        self.children = children;
        self
    }

    /// Set `self.strength_modifier` to the given value and return the
    /// modified rule. Builder-style for chaining.
    #[must_use]
    pub const fn with_strength_modifier(mut self, modifier: StrengthModifier) -> Self {
        self.strength_modifier = Some(modifier);
        self
    }

    /// Set `self.level` to the given value and return the modified rule.
    /// Builder-style for chaining; typically used only when constructing
    /// child rules programmatically.
    #[must_use]
    pub const fn with_level(mut self, level: u32) -> Self {
        self.level = level;
        self
    }

    /// Validate structural invariants of the rule.
    ///
    /// This checks invariants that the parser enforces automatically but
    /// that programmatic constructors (especially via serde deserialize)
    /// can violate:
    ///
    /// * Message must not be empty.
    /// * `level` must not exceed [`Self::MAX_LEVEL`].
    /// * Every child's `level` must be strictly greater than
    ///   `self.level`, and each child must recursively validate.
    ///
    /// This does *not* validate that `value` is shape-compatible with
    /// `typ` (e.g., a `Value::Uint` against a `TypeKind::String`); such
    /// mismatches are coerced or rejected by the evaluator at match time.
    ///
    /// # Errors
    ///
    /// Returns [`MagicRuleValidationError`] describing the first
    /// invariant violation encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libmagic_rs::{MagicRule, OffsetSpec, Operator, TypeKind, Value};
    ///
    /// let rule = MagicRule::new(
    ///     OffsetSpec::Absolute(0),
    ///     TypeKind::Byte { signed: false },
    ///     Operator::Equal,
    ///     Value::Uint(0),
    ///     "zero byte".to_string(),
    /// );
    /// assert!(rule.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), MagicRuleValidationError> {
        if self.message.is_empty() {
            return Err(MagicRuleValidationError::EmptyMessage);
        }
        if self.level > Self::MAX_LEVEL {
            return Err(MagicRuleValidationError::LevelTooDeep {
                level: self.level,
                max: Self::MAX_LEVEL,
            });
        }
        for (child_index, child) in self.children.iter().enumerate() {
            if child.level <= self.level {
                return Err(MagicRuleValidationError::InvalidChildLevel {
                    child_index,
                    child_level: child.level,
                    parent_level: self.level,
                });
            }
            child.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Restriction lints without an allow-*-in-tests config option;
    // non-ASCII test data exercises string value handling.
    #![allow(clippy::non_ascii_literal)]

    use super::*;

    #[test]
    fn test_magic_rule_new_defaults() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0x7f),
            "ELF".to_string(),
        );
        assert_eq!(rule.level, 0);
        assert!(rule.children.is_empty());
        assert!(rule.strength_modifier.is_none());
        assert!(rule.validate().is_ok());
    }

    #[test]
    fn test_magic_rule_builder_chain() {
        let child = MagicRule::new(
            OffsetSpec::Absolute(4),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(2),
            "64-bit".to_string(),
        )
        .with_level(1);
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0x7f),
            "ELF".to_string(),
        )
        .with_children(vec![child])
        .with_strength_modifier(StrengthModifier::Add(10));
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.strength_modifier, Some(StrengthModifier::Add(10)));
        assert!(parent.validate().is_ok());
    }

    #[test]
    fn test_magic_rule_validate_empty_message_rejected() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0),
            String::new(),
        );
        assert_eq!(rule.validate(), Err(MagicRuleValidationError::EmptyMessage));
    }

    #[test]
    fn test_magic_rule_validate_child_level_must_be_deeper() {
        let child_same_level = MagicRule::new(
            OffsetSpec::Absolute(4),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(2),
            "child".to_string(),
        ); // level = 0, same as parent
        let parent = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0x7f),
            "parent".to_string(),
        )
        .with_children(vec![child_same_level]);
        assert_eq!(
            parent.validate(),
            Err(MagicRuleValidationError::InvalidChildLevel {
                child_index: 0,
                child_level: 0,
                parent_level: 0,
            })
        );
    }

    #[test]
    fn test_magic_rule_validate_level_too_deep() {
        let rule = MagicRule::new(
            OffsetSpec::Absolute(0),
            TypeKind::Byte { signed: false },
            Operator::Equal,
            Value::Uint(0),
            "deep".to_string(),
        )
        .with_level(MagicRule::MAX_LEVEL + 1);
        assert_eq!(
            rule.validate(),
            Err(MagicRuleValidationError::LevelTooDeep {
                level: MagicRule::MAX_LEVEL + 1,
                max: MagicRule::MAX_LEVEL,
            })
        );
    }

    #[test]
    fn test_offset_spec_absolute() {
        let offset = OffsetSpec::Absolute(42);
        assert_eq!(offset, OffsetSpec::Absolute(42));

        // Test negative offset
        let negative = OffsetSpec::Absolute(-10);
        assert_eq!(negative, OffsetSpec::Absolute(-10));
    }

    #[test]
    fn test_offset_spec_indirect() {
        let indirect = OffsetSpec::Indirect {
            base_offset: 0x20,
            base_relative: false,
            pointer_type: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            adjustment: 4,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: Endianness::Little,
        };

        match indirect {
            OffsetSpec::Indirect {
                base_offset,
                adjustment,
                ..
            } => {
                assert_eq!(base_offset, 0x20);
                assert_eq!(adjustment, 4);
            }
            _ => panic!("Expected Indirect variant"),
        }
    }

    #[test]
    fn test_offset_spec_relative() {
        let relative = OffsetSpec::Relative(8);
        assert_eq!(relative, OffsetSpec::Relative(8));

        // Test negative relative offset
        let negative_relative = OffsetSpec::Relative(-4);
        assert_eq!(negative_relative, OffsetSpec::Relative(-4));
    }

    #[test]
    fn test_offset_spec_from_end() {
        let from_end = OffsetSpec::FromEnd(-16);
        assert_eq!(from_end, OffsetSpec::FromEnd(-16));

        // Test positive from_end (though unusual)
        let positive_from_end = OffsetSpec::FromEnd(8);
        assert_eq!(positive_from_end, OffsetSpec::FromEnd(8));
    }

    #[test]
    fn test_offset_spec_debug() {
        let offset = OffsetSpec::Absolute(100);
        let debug_str = format!("{offset:?}");
        assert!(debug_str.contains("Absolute"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_offset_spec_clone() {
        let original = OffsetSpec::Indirect {
            base_offset: 0x10,
            base_relative: false,
            pointer_type: TypeKind::Short {
                endian: Endianness::Big,
                signed: true,
            },
            adjustment: -2,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: Endianness::Big,
        };

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_offset_spec_serialization() {
        let offset = OffsetSpec::Absolute(42);

        // Test JSON serialization
        let json = serde_json::to_string(&offset).expect("Failed to serialize");
        let deserialized: OffsetSpec = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(offset, deserialized);
    }

    #[test]
    fn test_offset_spec_indirect_serialization() {
        let indirect = OffsetSpec::Indirect {
            base_offset: 0x100,
            base_relative: false,
            pointer_type: TypeKind::Long {
                endian: Endianness::Native,
                signed: false,
            },
            adjustment: 12,
            adjustment_op: IndirectAdjustmentOp::Add,
            result_relative: false,
            endian: Endianness::Native,
        };

        // Test JSON serialization for complex variant
        let json = serde_json::to_string(&indirect).expect("Failed to serialize");
        let deserialized: OffsetSpec = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(indirect, deserialized);
    }

    #[test]
    fn test_all_offset_spec_variants() {
        let variants = [
            OffsetSpec::Absolute(0),
            OffsetSpec::Absolute(-100),
            OffsetSpec::Indirect {
                base_offset: 0x20,
                base_relative: false,
                pointer_type: TypeKind::Byte { signed: true },
                adjustment: 0,
                adjustment_op: IndirectAdjustmentOp::Add,
                result_relative: false,
                endian: Endianness::Little,
            },
            OffsetSpec::Relative(50),
            OffsetSpec::Relative(-25),
            OffsetSpec::FromEnd(-8),
            OffsetSpec::FromEnd(4),
        ];

        // Test that all variants can be created and are distinct
        for (i, variant) in variants.iter().enumerate() {
            for (j, other) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        variant, other,
                        "Variants at indices {i} and {j} should be different"
                    );
                }
            }
        }
    }

    #[test]
    fn test_endianness_variants() {
        let endianness_values = vec![Endianness::Little, Endianness::Big, Endianness::Native];

        for endian in endianness_values {
            let indirect = OffsetSpec::Indirect {
                base_offset: 0,
                base_relative: false,
                pointer_type: TypeKind::Long {
                    endian,
                    signed: false,
                },
                adjustment: 0,
                adjustment_op: IndirectAdjustmentOp::Add,
                result_relative: false,
                endian,
            };

            // Verify the endianness is preserved
            match indirect {
                OffsetSpec::Indirect {
                    endian: actual_endian,
                    ..
                } => {
                    assert_eq!(endian, actual_endian);
                }
                _ => panic!("Expected Indirect variant"),
            }
        }
    }

    // Value enum tests
    #[test]
    fn test_value_uint() {
        let value = Value::Uint(42);
        assert_eq!(value, Value::Uint(42));

        // Test large values
        let large_value = Value::Uint(u64::MAX);
        assert_eq!(large_value, Value::Uint(u64::MAX));
    }

    #[test]
    fn test_value_int() {
        let positive = Value::Int(100);
        assert_eq!(positive, Value::Int(100));

        let negative = Value::Int(-50);
        assert_eq!(negative, Value::Int(-50));

        // Test extreme values
        let max_int = Value::Int(i64::MAX);
        let min_int = Value::Int(i64::MIN);
        assert_eq!(max_int, Value::Int(i64::MAX));
        assert_eq!(min_int, Value::Int(i64::MIN));
    }

    #[test]
    fn test_value_bytes() {
        let empty_bytes = Value::Bytes(vec![]);
        assert_eq!(empty_bytes, Value::Bytes(vec![]));

        let some_bytes = Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]);
        assert_eq!(some_bytes, Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]));

        // Test that different byte sequences are not equal
        let other_bytes = Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]);
        assert_ne!(some_bytes, other_bytes);
    }

    #[test]
    fn test_value_string() {
        let empty_string = Value::String(String::new());
        assert_eq!(empty_string, Value::String(String::new()));

        let hello = Value::String("Hello, World!".to_string());
        assert_eq!(hello, Value::String("Hello, World!".to_string()));

        // Test Unicode strings
        let unicode = Value::String("🦀 Rust".to_string());
        assert_eq!(unicode, Value::String("🦀 Rust".to_string()));
    }

    #[test]
    fn test_value_comparison() {
        // Test that different value types are not equal
        let uint_val = Value::Uint(42);
        let int_val = Value::Int(42);
        let float_val = Value::Float(42.0);
        let bytes_val = Value::Bytes(vec![42]);
        let string_val = Value::String("42".to_string());

        assert_ne!(uint_val, int_val);
        assert_ne!(uint_val, float_val);
        assert_ne!(uint_val, bytes_val);
        assert_ne!(uint_val, string_val);
        assert_ne!(int_val, float_val);
        assert_ne!(int_val, bytes_val);
        assert_ne!(int_val, string_val);
        assert_ne!(float_val, bytes_val);
        assert_ne!(float_val, string_val);
        assert_ne!(bytes_val, string_val);
    }

    #[test]
    fn test_value_debug() {
        let uint_val = Value::Uint(123);
        let debug_str = format!("{uint_val:?}");
        assert!(debug_str.contains("Uint"));
        assert!(debug_str.contains("123"));

        let string_val = Value::String("test".to_string());
        let debug_str = format!("{string_val:?}");
        assert!(debug_str.contains("String"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_value_clone() {
        let original = Value::Bytes(vec![1, 2, 3, 4]);
        let cloned = original.clone();
        assert_eq!(original, cloned);

        // Verify they are independent copies
        match (original, cloned) {
            (Value::Bytes(orig_bytes), Value::Bytes(cloned_bytes)) => {
                assert_eq!(orig_bytes, cloned_bytes);
                // They should have the same content but be different Vec instances
            }
            _ => panic!("Expected Bytes variants"),
        }
    }

    #[test]
    fn test_value_float() {
        let value = Value::Float(3.125);
        assert_eq!(value, Value::Float(3.125));

        let negative = Value::Float(-1.5);
        assert_eq!(negative, Value::Float(-1.5));

        let zero = Value::Float(0.0);
        assert_eq!(zero, Value::Float(0.0));
    }

    #[test]
    fn test_value_serialization() {
        let values = vec![
            Value::Uint(42),
            Value::Int(-100),
            Value::Float(3.125),
            Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            Value::String("ELF executable".to_string()),
        ];

        for value in values {
            // Test JSON serialization
            let json = serde_json::to_string(&value).expect("Failed to serialize Value");
            let deserialized: Value =
                serde_json::from_str(&json).expect("Failed to deserialize Value");
            assert_eq!(value, deserialized);
        }
    }

    #[test]
    fn test_value_serialization_edge_cases() {
        // Test empty collections
        let empty_bytes = Value::Bytes(vec![]);
        let json = serde_json::to_string(&empty_bytes).expect("Failed to serialize empty bytes");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize empty bytes");
        assert_eq!(empty_bytes, deserialized);

        let empty_string = Value::String(String::new());
        let json = serde_json::to_string(&empty_string).expect("Failed to serialize empty string");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize empty string");
        assert_eq!(empty_string, deserialized);

        // Test extreme values
        let max_uint = Value::Uint(u64::MAX);
        let json = serde_json::to_string(&max_uint).expect("Failed to serialize max uint");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize max uint");
        assert_eq!(max_uint, deserialized);

        let min_int = Value::Int(i64::MIN);
        let json = serde_json::to_string(&min_int).expect("Failed to serialize min int");
        let deserialized: Value =
            serde_json::from_str(&json).expect("Failed to deserialize min int");
        assert_eq!(min_int, deserialized);
    }

    // TypeKind tests
    #[test]
    fn test_type_kind_byte() {
        let byte_type = TypeKind::Byte { signed: true };
        assert_eq!(byte_type, TypeKind::Byte { signed: true });
    }

    #[test]
    fn test_type_kind_short() {
        let short_little_endian = TypeKind::Short {
            endian: Endianness::Little,
            signed: false,
        };
        let short_big_endian = TypeKind::Short {
            endian: Endianness::Big,
            signed: true,
        };

        assert_ne!(short_little_endian, short_big_endian);
        assert_eq!(short_little_endian, short_little_endian.clone());
    }

    #[test]
    fn test_type_kind_long() {
        let long_native = TypeKind::Long {
            endian: Endianness::Native,
            signed: true,
        };

        match long_native {
            TypeKind::Long { endian, signed } => {
                assert_eq!(endian, Endianness::Native);
                assert!(signed);
            }
            _ => panic!("Expected Long variant"),
        }
    }

    #[test]
    fn test_type_kind_string() {
        let unlimited_string = TypeKind::String {
            max_length: None,
            flags: StringFlags::default(),
        };
        let limited_string = TypeKind::String {
            max_length: Some(256),
            flags: StringFlags::default(),
        };

        assert_ne!(unlimited_string, limited_string);
        assert_eq!(unlimited_string, unlimited_string.clone());
    }

    #[test]
    fn test_type_kind_serialization() {
        let types = vec![
            TypeKind::Byte { signed: true },
            TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            TypeKind::Long {
                endian: Endianness::Big,
                signed: true,
            },
            TypeKind::Quad {
                endian: Endianness::Little,
                signed: false,
            },
            TypeKind::Quad {
                endian: Endianness::Big,
                signed: true,
            },
            TypeKind::Float {
                endian: Endianness::Native,
            },
            TypeKind::Float {
                endian: Endianness::Big,
            },
            TypeKind::Double {
                endian: Endianness::Little,
            },
            TypeKind::Double {
                endian: Endianness::Native,
            },
            TypeKind::Date {
                endian: Endianness::Big,
                utc: true,
            },
            TypeKind::Date {
                endian: Endianness::Little,
                utc: false,
            },
            TypeKind::QDate {
                endian: Endianness::Native,
                utc: true,
            },
            TypeKind::QDate {
                endian: Endianness::Big,
                utc: false,
            },
            TypeKind::String {
                max_length: None,
                flags: StringFlags::default(),
            },
            TypeKind::String {
                max_length: Some(128),
                flags: StringFlags::default(),
            },
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false,
            },
            TypeKind::PString {
                max_length: Some(64),
                length_width: PStringLengthWidth::OneByte,
                length_includes_itself: false,
            },
            TypeKind::PString {
                max_length: None,
                length_width: PStringLengthWidth::TwoByteBE,
                length_includes_itself: true,
            },
            TypeKind::PString {
                max_length: Some(128),
                length_width: PStringLengthWidth::FourByteLE,
                length_includes_itself: false,
            },
        ];

        for typ in types {
            let json = serde_json::to_string(&typ).expect("Failed to serialize TypeKind");
            let deserialized: TypeKind =
                serde_json::from_str(&json).expect("Failed to deserialize TypeKind");
            assert_eq!(typ, deserialized);
        }
    }

    // Operator tests
    #[test]
    fn test_operator_variants() {
        let operators = [
            Operator::Equal,
            Operator::NotEqual,
            Operator::BitwiseAnd,
            Operator::BitwiseXor,
            Operator::BitwiseNot,
            Operator::AnyValue,
        ];

        for (i, op) in operators.iter().enumerate() {
            for (j, other) in operators.iter().enumerate() {
                if i == j {
                    assert_eq!(op, other);
                } else {
                    assert_ne!(op, other);
                }
            }
        }
    }

    #[test]
    fn test_operator_serialization() {
        let operators = vec![
            Operator::Equal,
            Operator::NotEqual,
            Operator::BitwiseAnd,
            Operator::BitwiseXor,
            Operator::BitwiseNot,
            Operator::AnyValue,
        ];

        for op in operators {
            let json = serde_json::to_string(&op).expect("Failed to serialize Operator");
            let deserialized: Operator =
                serde_json::from_str(&json).expect("Failed to deserialize Operator");
            assert_eq!(op, deserialized);
        }
    }

    // MagicRule tests
    #[test]
    fn test_magic_rule_creation() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        };

        assert_eq!(rule.message, "ELF magic");
        assert_eq!(rule.level, 0);
        assert!(rule.children.is_empty());
    }

    #[test]
    fn test_magic_rule_with_children() {
        let child_rule = MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "32-bit".to_string(),
            children: vec![],
            level: 1,
            strength_modifier: None,
            value_transform: None,
        };

        let parent_rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Long {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::Equal,
            value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
            message: "ELF executable".to_string(),
            children: vec![child_rule],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        };

        assert_eq!(parent_rule.children.len(), 1);
        assert_eq!(parent_rule.children[0].level, 1);
        assert_eq!(parent_rule.children[0].message, "32-bit");
    }

    #[test]
    fn test_magic_rule_serialization() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(16),
            typ: TypeKind::Short {
                endian: Endianness::Little,
                signed: false,
            },
            op: Operator::NotEqual,
            value: Value::Uint(0),
            message: "Non-zero short value".to_string(),
            children: vec![],
            level: 2,
            strength_modifier: None,
            value_transform: None,
        };

        let json = serde_json::to_string(&rule).expect("Failed to serialize MagicRule");
        let deserialized: MagicRule =
            serde_json::from_str(&json).expect("Failed to deserialize MagicRule");

        assert_eq!(rule.message, deserialized.message);
        assert_eq!(rule.level, deserialized.level);
        assert_eq!(rule.children.len(), deserialized.children.len());
    }

    // StrengthModifier tests
    #[test]
    fn test_strength_modifier_variants() {
        let add = StrengthModifier::Add(10);
        let sub = StrengthModifier::Subtract(5);
        let mul = StrengthModifier::Multiply(2);
        let div = StrengthModifier::Divide(2);
        let set = StrengthModifier::Set(50);

        // Test that each variant has the correct inner value
        assert_eq!(add, StrengthModifier::Add(10));
        assert_eq!(sub, StrengthModifier::Subtract(5));
        assert_eq!(mul, StrengthModifier::Multiply(2));
        assert_eq!(div, StrengthModifier::Divide(2));
        assert_eq!(set, StrengthModifier::Set(50));

        // Test that different variants are not equal
        assert_ne!(add, sub);
        assert_ne!(mul, div);
        assert_ne!(set, add);
    }

    #[test]
    fn test_strength_modifier_negative_values() {
        let add_negative = StrengthModifier::Add(-10);
        let sub_negative = StrengthModifier::Subtract(-5);
        let set_negative = StrengthModifier::Set(-50);

        assert_eq!(add_negative, StrengthModifier::Add(-10));
        assert_eq!(sub_negative, StrengthModifier::Subtract(-5));
        assert_eq!(set_negative, StrengthModifier::Set(-50));
    }

    #[test]
    fn test_strength_modifier_serialization() {
        let modifiers = vec![
            StrengthModifier::Add(10),
            StrengthModifier::Subtract(5),
            StrengthModifier::Multiply(2),
            StrengthModifier::Divide(3),
            StrengthModifier::Set(100),
        ];

        for modifier in modifiers {
            let json =
                serde_json::to_string(&modifier).expect("Failed to serialize StrengthModifier");
            let deserialized: StrengthModifier =
                serde_json::from_str(&json).expect("Failed to deserialize StrengthModifier");
            assert_eq!(modifier, deserialized);
        }
    }

    #[test]
    fn test_strength_modifier_debug() {
        let modifier = StrengthModifier::Add(25);
        let debug_str = format!("{modifier:?}");
        assert!(debug_str.contains("Add"));
        assert!(debug_str.contains("25"));
    }

    #[test]
    fn test_strength_modifier_clone() {
        let original = StrengthModifier::Multiply(4);
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_magic_rule_with_strength_modifier() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: Some(StrengthModifier::Add(20)),
            value_transform: None,
        };

        assert_eq!(rule.strength_modifier, Some(StrengthModifier::Add(20)));

        // Test serialization with strength_modifier
        let json = serde_json::to_string(&rule).expect("Failed to serialize MagicRule");
        let deserialized: MagicRule =
            serde_json::from_str(&json).expect("Failed to deserialize MagicRule");
        assert_eq!(rule.strength_modifier, deserialized.strength_modifier);
    }

    #[test]
    fn test_magic_rule_without_strength_modifier() {
        let rule = MagicRule {
            offset: OffsetSpec::Absolute(0),
            typ: TypeKind::Byte { signed: true },
            op: Operator::Equal,
            value: Value::Uint(0x7f),
            message: "ELF magic".to_string(),
            children: vec![],
            level: 0,
            strength_modifier: None,
            value_transform: None,
        };

        assert_eq!(rule.strength_modifier, None);
    }

    // MetaType tests
    #[test]
    fn test_meta_type_variants_debug_clone_eq() {
        let cases = [
            MetaType::Default,
            MetaType::Clear,
            MetaType::Indirect,
            MetaType::Offset,
            MetaType::Name("part2".to_string()),
            MetaType::Use {
                name: "part2".to_string(),
                flip_endian: false,
            },
            // A `\^`-flipped use must be distinct from the plain form so the
            // endian-flip flag participates in equality/serde/round-trip.
            MetaType::Use {
                name: "part2".to_string(),
                flip_endian: true,
            },
        ];

        for (i, variant) in cases.iter().enumerate() {
            // Debug formatting is non-empty
            let debug_str = format!("{variant:?}");
            assert!(
                !debug_str.is_empty(),
                "Debug format must be non-empty for variant at index {i}"
            );

            // Clone round-trip preserves equality
            let cloned = variant.clone();
            assert_eq!(
                variant, &cloned,
                "Clone must preserve equality for variant at index {i}"
            );

            // Distinct variants are not equal
            for (j, other) in cases.iter().enumerate() {
                if i == j {
                    assert_eq!(variant, other);
                } else {
                    assert_ne!(
                        variant, other,
                        "Variants at indices {i} and {j} must differ"
                    );
                }
            }
        }
    }

    #[test]
    fn test_meta_type_serde_roundtrip() {
        let cases = [
            MetaType::Default,
            MetaType::Clear,
            MetaType::Indirect,
            MetaType::Offset,
            MetaType::Name("foo".to_string()),
            MetaType::Use {
                name: "bar".to_string(),
                flip_endian: false,
            },
            MetaType::Use {
                name: "bar".to_string(),
                flip_endian: true,
            },
        ];

        for variant in cases {
            let json = serde_json::to_string(&variant).expect("serialize MetaType");
            let deserialized: MetaType = serde_json::from_str(&json).expect("deserialize MetaType");
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_type_kind_meta_bit_width_is_none() {
        let cases = [
            MetaType::Default,
            MetaType::Clear,
            MetaType::Indirect,
            MetaType::Offset,
            MetaType::Name("x".to_string()),
            MetaType::Use {
                name: "x".to_string(),
                flip_endian: false,
            },
            MetaType::Use {
                name: "x".to_string(),
                flip_endian: true,
            },
        ];
        for meta in cases {
            let kind = TypeKind::Meta(meta);
            assert_eq!(
                kind.bit_width(),
                None,
                "TypeKind::Meta must have no bit width: {kind:?}"
            );
        }
    }
}
