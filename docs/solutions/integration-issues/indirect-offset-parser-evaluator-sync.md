---
title: Implement indirect offset parsing in magic file grammar
date: 2026-03-30
status: resolved
severity: high
category: integration-issues
components:
  - parser/grammar
  - evaluator/offset
  - integration
tags:
  - parser
  - indirect-offset
  - nom
  - magic-file-syntax
  - pointer-specifier
issue: '#37'
branch: 37-evaluator-implement-indirect-offset-resolution
symptoms:
  - parse_offset("(0x3c.l)") fails with parse error
  - Magic files containing indirect offset syntax cannot be loaded via MagicDatabase::load_from_file()
  - resolve_indirect_offset() is unreachable dead code from text-magic loading path
root_cause: parse_offset() had no branch for '('-prefixed input; always delegated to parse_number() which only handles numeric literals
solution_files:
  - src/parser/grammar/mod.rs
  - src/parser/grammar/tests.rs
  - tests/indirect_offset_integration.rs
related_gotchas:
  - parse_number() handles '-' prefix but not '+'; positive adjustments need manual '+' consumption
  - parse_value() requires quoted strings; bare string literals cause integration test failures
---

# Indirect Offset Parser-Evaluator Sync

## Problem

The evaluator for indirect offsets (`resolve_indirect_offset()` in `src/evaluator/offset/indirect.rs`) was fully implemented with 35 unit tests, but the parser in `src/parser/grammar/mod.rs` could not produce `OffsetSpec::Indirect` AST nodes. The `parse_offset()` function only handled absolute numeric offsets and had no branch for `(`-prefixed indirect offset syntax like `(0x3c.l)` or `(0x3c.l+4)`.

This meant the feature was unreachable through the public `MagicDatabase::load_from_file()` API -- the primary way users load text magic files.

## Root Cause

`parse_offset()` unconditionally delegated to `parse_number()`, which only parses numeric literals. Input starting with `(` was rejected as a parse error. The evaluator code was effectively dead code from the text-magic loading path.

## Solution

### 1. Added `pointer_specifier_to_type()` helper

Maps single-character pointer specifiers to `(TypeKind, Endianness)` per libmagic convention:

| Specifier  | Width  | Endianness |
| ---------- | ------ | ---------- |
| `.b`, `.B` | 1 byte | Native     |
| `.s`       | 2 byte | Native     |
| `.S`       | 2 byte | Big        |
| `.l`       | 4 byte | Native     |
| `.L`       | 4 byte | Big        |
| `.q`       | 8 byte | Native     |
| `.Q`       | 8 byte | Big        |

All pointer types are unsigned (`signed: false`). Lowercase = native endian, uppercase = big-endian.

### 2. Added `parse_indirect_offset()` function

Parses `(base.type)` and `(base.type+/-adj)` syntax:

1. Consume `(`
2. Parse base offset via `parse_number()`
3. Consume `.` and type specifier character
4. Optionally parse adjustment (see gotcha below)
5. Consume `)`
6. Return `OffsetSpec::Indirect { base_offset, pointer_type, adjustment, endian }`

### 3. Updated `parse_offset()` to branch on leading `(`

```rust
pub fn parse_offset(input: &str) -> IResult<&str, OffsetSpec> {
    let (input, _) = multispace0(input)?;
    if input.starts_with('(') {
        let (input, spec) = parse_indirect_offset(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, spec))
    } else {
        let (input, offset_value) = parse_number(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, OffsetSpec::Absolute(offset_value)))
    }
}
```

### 4. No changes needed to `parse_rule_offset()`

It delegates to `parse_offset()`, so hierarchical forms like `>(0x3c.l)` work automatically.

## Gotchas Discovered

### `parse_number()` does not handle `+` prefix

`parse_number()` handles `-` internally but not `+`. For `+N` adjustments, the `+` must be consumed manually:

```rust
let (input, adjustment) = if input.starts_with('+') {
    let (input, _) = char('+')(input)?;
    parse_number(input)?
} else if input.starts_with('-') {
    parse_number(input)?
} else {
    (input, 0)
};
```

Do NOT modify `parse_number()` globally -- it is shared by offset and value parsing, and adding `+` support would change semantics elsewhere.

### `parse_value()` requires quoted strings

Integration tests initially failed because `parse_value()` does not accept bare strings. Magic file string values must be quoted:

```text
# Correct
0 string "MZ" DOS executable

# Wrong -- parse_value() rejects bare "MZ"
0 string MZ DOS executable
```

### Use big-endian specifiers in cross-platform tests

Prefer `.L` (big-endian long) over `.l` (native) in integration test magic files so byte buffers are deterministic across architectures.

## Prevention Strategies

### Parser-Evaluator Parity Checklist

When adding a new AST variant, ensure:

1. **Parser produces it** -- unit test parses raw syntax, asserts correct AST node
2. **Evaluator consumes it** -- unit test constructs AST node, asserts evaluation result
3. **End-to-end test exists** -- integration test through `MagicDatabase::load_from_file()` proves the full pipeline works
4. **Codegen handles it** -- if it can appear in built-in rules, update `src/parser/codegen.rs`
5. **Strength calculation covers it** -- update `src/evaluator/strength.rs` if scoring changes

### Integration Test Template

```rust
#[test]
fn test_feature_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let magic_path = temp_dir.path().join("test.magic");
    let mut f = fs::File::create(&magic_path).unwrap();
    writeln!(f, r#"0 string "MAGIC" Test match"#).unwrap();

    let db = MagicDatabase::load_from_file(&magic_path).unwrap();
    let result = db.evaluate_buffer(b"MAGIC\x00data").unwrap();
    assert!(result.description.contains("Test match"));
}
```

## Cross-References

- **Evaluator solution**: `docs/solutions/logic-errors/indirect-offset-resolution.md`
- **Magic format spec**: `docs/MAGIC_FORMAT.md` (lines 106-126, indirect offset section)
- **Gotchas**: `GOTCHAS.md` sections 3.5 (`parse_number` `+` limitation) and 3.6 (quoted strings)
- **Architecture**: `AGENTS.md` offset specifications section
- **Issue**: #37 (indirect offset resolution)
- **Related gotchas**: S2 (enum variant checklists), S3 (parser architecture split), S5 (numeric type pitfalls)
