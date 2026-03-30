---
title: 'Fix indirect offset parser: endianness, signedness, and adjustment placement'
date: 2026-03-30
status: resolved
severity: high
category: logic-errors
tags:
  - parser
  - indirect-offset
  - gnu-file-semantics
  - endianness
  - signed-by-default
components:
  - src/parser/grammar/mod.rs
  - src/parser/grammar/tests.rs
  - tests/indirect_offset_integration.rs
  - GOTCHAS.md
  - AGENTS.md
symptoms:
  - (0x3c.l)+4 parsed as indirect with adjustment=0 and leftover +4, breaking parse_magic_rule()
  - Lowercase pointer specifiers (.s, .l, .q) produced Endianness::Native instead of Endianness::Little
  - Pointer types were unsigned, mismatching libmagic signed-by-default convention
root_causes:
  - pointer_specifier_to_type() mapped lowercase specifiers to Endianness::Native instead of Endianness::Little
  - 'pointer_specifier_to_type() set signed: false instead of signed: true'
  - parse_indirect_offset() consumed adjustment inside parentheses instead of after closing paren
references:
  - GOTCHAS.md S6.3 (signed-by-default types)
  - GOTCHAS.md S3.7 (added by this fix)
  - 'GNU file(1) man page: indirect offset syntax'
related_issues:
  - 37
---

# Fix Indirect Offset Parser: GNU `file` Semantics

## Problem

The indirect offset parser had three semantic errors that caused it to produce incorrect AST nodes. The code compiled and tests passed, but behavior was wrong relative to the GNU `file` specification:

1. **Endianness**: Lowercase specifiers (`.s`, `.l`, `.q`) mapped to `Endianness::Native` instead of `Endianness::Little`
2. **Signedness**: Pointer types set to `signed: false` instead of `signed: true` (GOTCHAS S6.3)
3. **Adjustment syntax**: Parsed inside parens `(0x3c.l+4)` instead of after them `(0x3c.l)+4`

The tests validated the wrong implementation rather than the specification -- a "tests match code but not spec" anti-pattern.

## Root Cause

The initial implementation followed incorrect assumptions:

- Lowercase = native endian (wrong: GNU `file` defines lowercase = little-endian)
- Pointer types = unsigned (wrong: libmagic types are signed by default per S6.3)
- Adjustment inside parens (wrong: GNU `file` syntax places adjustment after `)`)

Tests were written alongside the code, so they confirmed the implementation's behavior rather than the spec's requirements.

## Solution

Three changes in `src/parser/grammar/mod.rs`:

### Fix 1: Endianness mapping

```rust
// Before (wrong)
'l' => Some((TypeKind::Long { endian: Endianness::Native, signed: false }, Endianness::Native))

// After (correct -- GNU `file` lowercase = little-endian)
'l' => Some((TypeKind::Long { endian: Endianness::Little, signed: true }, Endianness::Little))
```

Applied to all lowercase specifiers (`b`, `s`, `l`, `q`). Uppercase specifiers were already correct (`Endianness::Big`).

### Fix 2: Signed-by-default

Changed all pointer types from `signed: false` to `signed: true` across every specifier arm.

### Fix 3: Adjustment after closing paren

```rust
// Before (wrong): adjustment consumed inside parens
let (input, adjustment) = parse_adjustment(input)?;
let (input, _) = char(')')(input)?;

// After (correct): close paren first, then adjustment
let (input, _) = char(')')(input)?;
let (input, adjustment) = parse_adjustment(input)?;
```

### Test corrections

- All parser unit tests updated to expect `Endianness::Little`, `signed: true`, and `(base.type)+adj` syntax
- Integration tests updated with little-endian byte layouts and lowercase `.l` specifier
- Added new test: `>(0x3c.l)+4` child rule with adjustment after paren

## Prevention Strategies

### Spec-first test writing

Write test expectations from the spec (GNU `file` man page, GOTCHAS.md) before implementing. Document the spec reference above each test case. In TDD, the RED phase must derive expected values from the spec, not from running the code.

### Cross-reference GOTCHAS.md for type mappings

Treat GOTCHAS.md as a mandatory checklist when adding type mappings:

- **S6.3**: Default to `signed: true` unless keyword has `u` prefix
- **S6.1**: Uppercase = big-endian, lowercase = little-endian
- **S3.7**: Indirect offset specifiers follow GNU `file` semantics

### Prefer deterministic endianness

`Endianness::Native` should never appear in indirect offset resolution. Every endianness value must be explicitly `Little` or `Big` per the spec. Tests must use explicit byte sequences, not `to_ne_bytes()`.

### Verify against real magic files

Extract test inputs from `/usr/share/misc/magic` or the upstream [file/file](https://github.com/file/file) repository rather than inventing syntax.

## Cross-References

- **Evaluator solution**: `docs/solutions/logic-errors/indirect-offset-resolution.md`
- **Parser-evaluator sync**: `docs/solutions/integration-issues/indirect-offset-parser-evaluator-sync.md`
- **Magic format spec**: `docs/MAGIC_FORMAT.md` (lines 106-126)
- **Gotchas**: `GOTCHAS.md` sections 3.5, 3.6, 3.7, 6.3
- **Issue**: #37
