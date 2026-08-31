// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Meta-type directive parsing tests
//
// Covers `default`, `clear`, `name`, `use`, `indirect`, and `offset`. Exercises
// the optional `x` (AnyValue) placeholder strip, `name`/`use` identifier
// validation, end-to-end text-magic-file hoisting into the name table, and
// the `searchbug.magic` fixture as a single-file acceptance check.

#[test]
fn test_parse_magic_rule_meta_types() {
    // Table: (input, expected_level, expected_typ, expected_message)
    let cases: &[(&str, u32, TypeKind, &str)] = &[
        // `x` is the AnyValue operator; for meta types the parser strips
        // it (with surrounding whitespace) before taking the rest of the
        // line as the message. See `strip_optional_x_operator` in
        // `parser/grammar/mod.rs`. Without that strip, rules like
        // `>>&0 offset x at_offset %lld` would render as
        // `x\tat_offset 11` and diverge from GNU `file` output.
        (
            "0 default x msg",
            0,
            TypeKind::Meta(MetaType::Default),
            "msg",
        ),
        // And a message without a leading `x` passes through unchanged.
        ("0 default msg", 0, TypeKind::Meta(MetaType::Default), "msg"),
        ("0 clear", 0, TypeKind::Meta(MetaType::Clear), ""),
        (
            "0 offset x pos=%lld",
            0,
            TypeKind::Meta(MetaType::Offset),
            "pos=%lld",
        ),
        ("0 indirect x", 0, TypeKind::Meta(MetaType::Indirect), ""),
        (
            "0 name part2",
            0,
            TypeKind::Meta(MetaType::Name("part2".to_string())),
            "",
        ),
        (
            "0 use part2",
            0,
            TypeKind::Meta(MetaType::Use {
                name: "part2".to_string(),
                flip_endian: false,
            }),
            "",
        ),
        ("0 indirect", 0, TypeKind::Meta(MetaType::Indirect), ""),
        (
            ">0 use part2",
            1,
            TypeKind::Meta(MetaType::Use {
                name: "part2".to_string(),
                flip_endian: false,
            }),
            "",
        ),
    ];

    for (input, expected_level, expected_typ, expected_message) in cases {
        let (remaining, rule) =
            parse_magic_rule(input).unwrap_or_else(|e| panic!("parse failed for {input:?}: {e:?}"));
        assert_eq!(remaining, "", "remaining mismatch for {input:?}");
        assert_eq!(rule.level, *expected_level, "level mismatch for {input:?}");
        assert_eq!(rule.typ, *expected_typ, "typ mismatch for {input:?}");
        assert_eq!(
            rule.message, *expected_message,
            "message mismatch for {input:?}"
        );
    }

    // Bare `name` / `use` with no identifier must be a parse error.
    assert!(
        parse_magic_rule("0 name").is_err(),
        "bare `name` with no identifier must fail"
    );
    assert!(
        parse_magic_rule("0 use").is_err(),
        "bare `use` with no identifier must fail"
    );
}

#[test]
fn test_parse_magic_rule_meta_name_use_reject_malformed_identifiers() {
    // Operator-adjacent continuation must reject the truncated identifier
    // (`part2=foo`, `part2!bar`, etc.) rather than silently dropping the
    // operator text into the message slot.
    let operator_cases = [
        "0 use part2=foo",
        "0 use part2!=bar",
        "0 use part<foo",
        "0 use part>foo",
        "0 name part&foo",
        "0 name part^foo",
        "0 name part~foo",
        "0 name part|foo",
    ];
    for input in operator_cases {
        assert!(
            parse_magic_rule(input).is_err(),
            "operator-adjacent identifier must fail: {input:?}"
        );
    }

    // Identifiers followed by whitespace + descriptive text are accepted:
    // real-world magic files use this for human-readable annotations. The
    // identifier always ends at the first non-id character (space, tab), so
    // `expected_id` is the first token. The trailing text handling then
    // DIVERGES by directive, matching GNU `file`:
    //   - `name`: the trailing text IS the subroutine's own description and
    //     is preserved as the rule message (emitted at the `use` site, e.g.
    //     `0 name xbase-prf dBase Printer Form`).
    //   - `use`: a use-site has no message slot; the trailing text is
    //     dropped (`0 use foo bar` renders no `bar`).
    let trailing_text_cases = [
        ("0 name part 2", "part", "2"),
        ("0 name my id", "my", "id"),
        (
            "0 name xbase-prf dBase Printer Form",
            "xbase-prf",
            "dBase Printer Form",
        ),
        // Mach-O universal subroutine: the `\b [` no-separator marker + `[`
        // must survive parsing verbatim (the leading `\b` is a literal
        // backslash-b, preserved per GOTCHAS S14.1).
        ("0 name mach-o \\b [", "mach-o", "\\b ["),
        ("0 use part2 extra", "part2", ""),
        ("0 use foo bar", "foo", ""),
        // A lone no-separator marker on a `use` site is a spacing control,
        // not a description, so it survives (GOTCHAS S14.4). Every `use`
        // site in the system magic DB that carries trailing text carries
        // exactly this.
        ("0 use mach-o-cpu \\b", "mach-o-cpu", "\\b"),
        // Trailing whitespace after the marker still counts as lone.
        ("0 use mach-o-cpu \\b   ", "mach-o-cpu", "\\b"),
        // A marker followed by real text is NOT lone: the whole trailing
        // string drops, exactly as before.
        ("0 use foo \\b extra", "foo", ""),
        // The helper accepts both marker forms, so cover the raw U+0008 byte
        // as well as the literal `\b` above.
        ("0 use mach-o-cpu \u{0008}", "mach-o-cpu", "\u{0008}"),
        ("0 use mach-o-cpu \u{0008}   ", "mach-o-cpu", "\u{0008}"),
        ("0 use foo \u{0008} extra", "foo", ""),
    ];
    for (input, expected_id, expected_message) in trailing_text_cases {
        let (_, rule) = parse_magic_rule(input)
            .unwrap_or_else(|e| panic!("trailing text after id should parse {input:?}: {e:?}"));
        match &rule.typ {
            TypeKind::Meta(MetaType::Name(id) | MetaType::Use { name: id, .. }) => {
                assert_eq!(
                    id, expected_id,
                    "identifier should stop at first whitespace for {input:?}"
                );
            }
            other => panic!("expected Name/Use meta, got {other:?}"),
        }
        assert_eq!(
            rule.message, expected_message,
            "message mismatch for {input:?} (name preserves trailing text, use drops it)"
        );
    }

    // Sanity check: an identifier followed only by trailing whitespace still parses.
    let (_, rule) = parse_magic_rule("0 name part2   ").expect("trailing ws is ok");
    assert_eq!(
        rule.typ,
        TypeKind::Meta(MetaType::Name("part2".to_string()))
    );
    let (_, rule) = parse_magic_rule("0 use part2\t").expect("trailing tab is ok");
    assert_eq!(
        rule.typ,
        TypeKind::Meta(MetaType::Use {
            name: "part2".to_string(),
            flip_endian: false
        })
    );
}

#[test]
fn test_parse_use_caret_prefix_sets_flip_endian() {
    // magic(5) `use \^name` (the `\^` endian-flip prefix, issue #236) parses
    // to `MetaType::Use { flip_endian: true }`; the `\^` is consumed and the
    // bare identifier is preserved. A plain `use name` stays `flip_endian:
    // false`. This is the real `images` TIFF `>(4.L) use \^tiff_ifd` shape.
    let (_, flipped) = parse_magic_rule(">0 use \\^tiff_ifd").expect("flip use parses");
    assert_eq!(
        flipped.typ,
        TypeKind::Meta(MetaType::Use {
            name: "tiff_ifd".to_string(),
            flip_endian: true,
        }),
        "`use \\^name` must set flip_endian and strip the \\^ prefix"
    );

    let (_, plain) = parse_magic_rule(">0 use tiff_ifd").expect("plain use parses");
    assert_eq!(
        plain.typ,
        TypeKind::Meta(MetaType::Use {
            name: "tiff_ifd".to_string(),
            flip_endian: false,
        }),
        "a plain `use name` must leave flip_endian false"
    );
}

#[test]
fn test_parse_magic_rule_meta_rejects_attached_operator() {
    // Meta-type directives (`default`, `clear`, `indirect`, `offset`) have
    // no operand, so an attached operator like `default&0xf` is malformed.
    // Before the fix for RUs, `parse_attached_operator` consumed the `&`
    // (and optional mask) and `parse_magic_rule` then silently dropped the
    // captured operator on the floor, producing a rule whose `op` field
    // would be `AnyValue` even though the source text contained a mask.
    // `name`/`use` short-circuit in `parse_type_and_operator` and cannot
    // reach the attached-op path, so they are not exercised here. Only
    // `&`-attached forms round-trip through `parse_attached_operator`;
    // other operator-adjacent glyphs (`^`, `~`, `>`, etc.) fall through
    // to `parse_message` and are covered by message-parsing tests, not
    // here.
    let malformed = [
        "0 default&0xf msg",
        "0 default& msg",
        "0 clear&0xff",
        "0 indirect&0x1",
        "0 offset&0xf0 pos",
    ];
    for input in malformed {
        assert!(
            parse_magic_rule(input).is_err(),
            "meta-type with attached operator must fail: {input:?}"
        );
    }
}

#[test]
fn test_parse_text_magic_file_meta_roundtrip() {
    // Build a small magic file that uses the six meta-types. The `name`
    // block is a level-1 subroutine invoked by the top-level `use`, and
    // `indirect` / `default` / `clear` / `offset` appear as sibling
    // directives to exercise the parse path for each variant.
    //
    // NOTE: all rules use the SAME top-level indentation so
    // build_rule_hierarchy treats them as siblings. Child rules would
    // require a preceding parent match, which meta-types do not produce.
    let magic = "\
0 name subroutine
0 use subroutine
0 default default-msg
0 clear
0 indirect
";
    let parsed =
        crate::parser::parse_text_magic_file(magic).expect("meta-type magic file should parse");
    // Only the `name` declaration is hoisted into the name table; the
    // other four meta-types remain as top-level rules in document order.
    let rules = parsed.rules;
    assert_eq!(
        rules.len(),
        4,
        "expected 4 top-level rules after name hoist, got {rules:?}"
    );
    assert!(
        parsed.name_table.get("subroutine").is_some(),
        "name subroutine should be extracted into the name table"
    );

    assert_eq!(
        rules[0].typ,
        TypeKind::Meta(MetaType::Use {
            name: "subroutine".to_string(),
            flip_endian: false,
        })
    );
    assert_eq!(rules[1].typ, TypeKind::Meta(MetaType::Default));
    assert_eq!(rules[2].typ, TypeKind::Meta(MetaType::Clear));
    assert_eq!(rules[3].typ, TypeKind::Meta(MetaType::Indirect));
}

#[test]
fn test_parse_text_magic_file_searchbug_fixture() {
    // Regression: the canonical GNU `file` testfile `searchbug.magic`
    // exercises the `offset` keyword, `&N` relative-offset syntax, the
    // `name`/`use` subroutine machinery, and `search/N` -- every piece of
    // this phase's acceptance surface in a single fixture. Previously the
    // parser rejected the file on the unknown `offset` type keyword.
    let magic = std::fs::read_to_string("third_party/tests/searchbug.magic")
        .expect("searchbug.magic fixture must exist");
    let parsed = crate::parser::parse_text_magic_file(&magic)
        .expect("searchbug.magic must parse end-to-end");
    assert!(
        !parsed.rules.is_empty(),
        "searchbug.magic must produce at least one top-level rule"
    );
}
