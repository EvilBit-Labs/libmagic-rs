// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Per-entry test type and two-pass admission, mirroring GNU `file`'s
//! `apprentice.c::set_test_type` and the skip checks in `softmagic.c::match`.

use crate::output::ascmagic::{Utf8Look, looks_utf8};
use crate::parser::ast::{MagicRule, MetaType, TypeKind, Value};

/// Which passes an entry may run in, from its first line only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryTestType {
    pub(crate) bin: bool,
    pub(crate) text: bool,
}

/// One of the two top-level evaluation passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassMode {
    Bin,
    Text,
}

/// A pass plus whether the buffer was classified as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TopLevelPass {
    pub(crate) mode: PassMode,
    pub(crate) buffer_is_text: bool,
}

/// `(bin_test, text_test)` for the types upstream calls `IS_STRING` that
/// carry flags here; `None` when the rule has no `/b` `/t` hints.
fn string_hints(rule: &MagicRule) -> Option<(bool, bool)> {
    match &rule.typ {
        TypeKind::String { flags, .. } => Some((flags.bin_test, flags.text_test)),
        TypeKind::Search { flags, .. } => Some((flags.bin_test, flags.text_test)),
        _ => None,
    }
}

/// True if `s` holds the escape text `\xHH` (HH >= 0x80), walking escapes
/// left to right so `\\xff` is not one.
fn has_high_hex_escape(s: &str) -> bool {
    let mut rest = s.as_bytes();
    while let Some((&c, tail)) = rest.split_first() {
        if c == b'\\' {
            if let Some([b'x', h1, h2]) = tail.get(..3)
                && let Ok(hex) = std::str::from_utf8(&[*h1, *h2])
                && u8::from_str_radix(hex, 16).is_ok_and(|v| v >= 0x80)
            {
                return true;
            }
            // Skip the escaped character so `\\x..` is not read as an escape.
            rest = tail.get(1..).unwrap_or_default();
        } else {
            rest = tail;
        }
    }
    false
}

/// Upstream's `file_looks_utf8(pattern) > 0` for a regex/search operand.
fn pattern_is_text(rule: &MagicRule, is_regex: bool) -> bool {
    let bytes = match &rule.value {
        Value::String(s) if is_regex && has_high_hex_escape(s) => return false,
        Value::String(s) => s.as_bytes(),
        Value::Bytes(b) => b.as_slice(),
        _ => return false,
    };
    matches!(looks_utf8(bytes), Utf8Look::Ascii | Utf8Look::Multibyte)
}

/// Test type of an entry, from its first line (children are ignored).
pub(crate) fn entry_test_type(rule: &MagicRule) -> EntryTestType {
    let (bin, text) = match &rule.typ {
        TypeKind::String { flags, .. } => (!flags.text_test, flags.text_test),
        TypeKind::Regex { .. } | TypeKind::Search { .. } => {
            let is_regex = matches!(rule.typ, TypeKind::Regex { .. });
            let (b, t) = string_hints(rule).unwrap_or((false, false));
            if b || t {
                (b, t)
            } else if pattern_is_text(rule, is_regex) {
                (false, true)
            } else {
                (true, false)
            }
        }
        TypeKind::Meta(MetaType::Offset) => (true, false),
        TypeKind::Meta(_) => (false, false),
        // Numeric, float, date, pstring, string16: no /t hint carried.
        _ => (true, false),
    };
    EntryTestType { bin, text }
}

impl TopLevelPass {
    /// Mirrors upstream `match()`: the `/b` `/t` hint skip, then the pass check.
    pub(crate) fn admits(self, rule: &MagicRule) -> bool {
        if let Some((bin_test, text_test)) = string_hints(rule)
            && ((self.buffer_is_text && bin_test && !text_test)
                || (!self.buffer_is_text && text_test && !bin_test))
        {
            return false;
        }
        let t = entry_test_type(rule);
        match self.mode {
            PassMode::Bin => t.bin,
            PassMode::Text => t.text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{
        Endianness, MetaType, OffsetSpec, Operator, RegexCount, RegexFlags, TypeKind, Value,
    };
    use crate::parser::grammar::parse_magic_rule;

    fn parsed(line: &str) -> MagicRule {
        match parse_magic_rule(line) {
            Ok((_, rule)) => rule,
            Err(e) => panic!("parse failed for {line:?}: {e:?}"),
        }
    }

    fn ast(typ: TypeKind, value: Value) -> MagicRule {
        MagicRule::new(
            OffsetSpec::Absolute(0),
            typ,
            Operator::Equal,
            value,
            "m".into(),
        )
    }

    fn regex(value: Value) -> MagicRule {
        ast(
            TypeKind::Regex {
                flags: RegexFlags::default(),
                count: RegexCount::Default,
            },
            value,
        )
    }

    fn meta(m: MetaType) -> MagicRule {
        ast(TypeKind::Meta(m), Value::Uint(0))
    }

    const BIN: (bool, bool) = (true, false);
    const TEXT: (bool, bool) = (false, true);
    const BOTH: (bool, bool) = (true, true);
    const NEITHER: (bool, bool) = (false, false);

    fn s(v: &str) -> Value {
        Value::String(v.to_string())
    }

    #[test]
    fn entry_test_type_table() {
        let le = Endianness::Little;
        let mut with_child = parsed("0 byte 1 parent");
        with_child.children.push(parsed(">1 search/10/t ABC child"));
        let rows: Vec<(&str, MagicRule, (bool, bool))> = vec![
            ("byte", parsed("0 byte 1 m"), BIN),
            ("short", parsed("0 leshort 1 m"), BIN),
            ("long", parsed("0 belong 1 m"), BIN),
            ("quad", parsed("0 lequad 1 m"), BIN),
            (
                "id3",
                ast(TypeKind::Id3 { endian: le }, Value::Uint(1)),
                BIN,
            ),
            ("float", parsed("0 lefloat 1 m"), BIN),
            ("double", parsed("0 ledouble 1 m"), BIN),
            ("date", parsed("0 ledate 1 m"), BIN),
            ("qdate", parsed("0 leqdate 1 m"), BIN),
            ("offset pseudo-type", meta(MetaType::Offset), BIN),
            ("string", parsed("0 string ABC m"), BIN),
            ("string/t", parsed("0 string/t ABC m"), TEXT),
            ("string/b", parsed("0 string/b ABC m"), BIN),
            ("string/tb", parsed("0 string/tb ABC m"), TEXT),
            ("pstring", parsed("0 pstring ABC m"), BIN),
            ("lestring16", parsed("0 lestring16 ABC m"), BIN),
            ("regex", parsed("0 regex ABC m"), TEXT),
            ("regex/c", parsed("0 regex/c ABC m"), TEXT),
            ("search", parsed("0 search/100 ABC m"), TEXT),
            ("search/b", parsed("0 search/100/b ABC m"), BIN),
            ("search/t", parsed("0 search/100/t ABC m"), TEXT),
            ("search/bt", parsed("0 search/100/bt ABC m"), BOTH),
            ("regex NUL pattern", regex(s("A\0B")), BIN),
            ("regex escaped high byte", regex(s(r"A\xff")), BIN),
            (
                "regex escaped backslash then xff",
                regex(s(r"A\\xff")),
                TEXT,
            ),
            ("regex low hex escape", regex(s(r"A\x41")), TEXT),
            ("regex utf8 string", regex(s("caf\u{e9}")), TEXT),
            (
                "regex bytes utf8",
                regex(Value::Bytes(vec![0xc3, 0xa9])),
                TEXT,
            ),
            ("regex bytes invalid", regex(Value::Bytes(vec![0xff])), BIN),
            ("regex non-pattern operand", regex(Value::Uint(1)), BIN),
            ("first line wins over children", with_child, BIN),
            ("meta default", meta(MetaType::Default), NEITHER),
            ("meta clear", meta(MetaType::Clear), NEITHER),
            ("meta indirect", meta(MetaType::Indirect), NEITHER),
            (
                "meta use",
                meta(MetaType::Use {
                    name: "x".into(),
                    flip_endian: false,
                }),
                NEITHER,
            ),
            ("meta name", meta(MetaType::Name("x".into())), NEITHER),
        ];
        for (label, rule, (bin, text)) in rows {
            assert_eq!(
                entry_test_type(&rule),
                EntryTestType { bin, text },
                "test type mismatch for {label}"
            );
        }
    }

    #[test]
    fn admits_table() {
        use PassMode::{Bin, Text};
        // (label, rule, mode, buffer_is_text, expected)
        let rows: Vec<(&str, MagicRule, PassMode, bool, bool)> = vec![
            ("string/b", parsed("0 string/b A m"), Bin, true, false),
            ("string/b", parsed("0 string/b A m"), Text, true, false),
            ("string/b", parsed("0 string/b A m"), Bin, false, true),
            ("string/t", parsed("0 string/t A m"), Bin, false, false),
            ("string/t", parsed("0 string/t A m"), Text, false, false),
            ("string/t", parsed("0 string/t A m"), Text, true, true),
            ("string/t", parsed("0 string/t A m"), Bin, true, false),
            ("string", parsed("0 string A m"), Bin, true, true),
            ("string", parsed("0 string A m"), Bin, false, true),
            ("string", parsed("0 string A m"), Text, true, false),
            ("string", parsed("0 string A m"), Text, false, false),
            ("regex", parsed("0 regex A m"), Text, true, true),
            ("regex", parsed("0 regex A m"), Bin, true, false),
            ("search/bt", parsed("0 search/9/bt A m"), Bin, true, true),
            ("search/bt", parsed("0 search/9/bt A m"), Text, false, true),
            ("belong", parsed("0 belong 1 m"), Bin, true, true),
            ("belong", parsed("0 belong 1 m"), Bin, false, true),
            ("belong", parsed("0 belong 1 m"), Text, true, false),
            ("belong", parsed("0 belong 1 m"), Text, false, false),
        ];
        for (label, rule, mode, buffer_is_text, expected) in rows {
            let pass = TopLevelPass {
                mode,
                buffer_is_text,
            };
            assert_eq!(
                pass.admits(&rule),
                expected,
                "{label}: mode={mode:?} buffer_is_text={buffer_is_text}"
            );
        }
    }

    #[test]
    fn admits_rejects_meta_entries_everywhere() {
        let metas = [
            MetaType::Default,
            MetaType::Clear,
            MetaType::Indirect,
            MetaType::Use {
                name: "x".into(),
                flip_endian: false,
            },
            MetaType::Name("x".into()),
        ];
        for m in metas {
            let rule = meta(m.clone());
            for mode in [PassMode::Bin, PassMode::Text] {
                for buffer_is_text in [true, false] {
                    let pass = TopLevelPass {
                        mode,
                        buffer_is_text,
                    };
                    assert!(
                        !pass.admits(&rule),
                        "{m:?} admitted in {mode:?}/{buffer_is_text}"
                    );
                }
            }
        }
    }
}
