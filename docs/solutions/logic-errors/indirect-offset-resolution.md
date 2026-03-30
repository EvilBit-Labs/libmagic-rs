---
title: Implementing Indirect Offset Resolution for Binary Format Detection
category: logic-errors
date: 2026-03-30
tags: [evaluator, offsets, indirect, binary-formats, pe-header, pointer-chasing]
issue: '#37'
severity: high
components: [evaluator/offset/indirect.rs, evaluator/offset/mod.rs]
---

# Implementing Indirect Offset Resolution

## Problem

Indirect offsets (`OffsetSpec::Indirect`) were parsed into the AST but evaluation returned "not yet implemented." This blocked detection of complex binary formats like PE executables, where a pointer at offset `0x3C` must be read and dereferenced to locate the PE header.

Syntax: `(0x3c.l)` -- read a 32-bit long at offset 0x3C, use that value as the actual offset.

## Root Cause

The evaluator's `resolve_offset()` dispatcher in `offset/mod.rs` had a stub for `OffsetSpec::Indirect` that returned `UnsupportedType`. The implementation required a multi-step pointer dereference pipeline that did not exist.

## Solution

Implemented a 4-step pipeline in `evaluator/offset/indirect.rs`:

1. **Resolve base offset** to absolute position (reuses `resolve_absolute_offset`, supports negative/from-end)
2. **Read pointer value** at that position using the specified numeric type and endianness
3. **Apply adjustment** with checked arithmetic (`checked_add`/`checked_sub`)
4. **Validate final offset** against buffer bounds

### Key Design Decisions

**Signed pointer reinterpretation**: Signed negative pointer values (e.g., `i32(-1)` from `[0xFF, 0xFF, 0xFF, 0xFF]`) are reinterpreted as raw unsigned (`u64::MAX`) via `extract_raw_unsigned()`. This matches libmagic's behavior where the bit pattern is what matters, not the signed interpretation. The bounds check at step 4 catches these enormous values.

**Separated concerns**: `read_pointer()` handles type dispatch and endianness, `extract_raw_unsigned()` handles signed-to-unsigned conversion, `apply_adjustment()` handles arithmetic with overflow protection. Each is independently testable.

**`i64::MIN` edge case**: `apply_adjustment` explicitly handles `i64::MIN` because `-i64::MIN` overflows. Returns an error rather than panicking.

```rust
// Core pipeline
let abs_base = resolve_absolute_offset(base_offset, buffer)?;
let pointer_value = read_pointer(buffer, abs_base, pointer_type, endian)?;
let final_offset = apply_adjustment(pointer_value, adjustment)?;
if final_offset >= buffer.len() { return Err(BufferOverrun) }
```

### Dispatcher Update

`offset/mod.rs` line 71 changed from stub to:

```rust
OffsetSpec::Indirect { .. } => indirect::resolve_indirect_offset(spec, buffer),
```

## Prevention Tips

- When adding new offset types, follow the same pattern: resolve base, read value, apply adjustment, validate bounds. The 4-step pipeline is the established pattern.
- Always use `checked_add`/`checked_sub` for offset arithmetic -- malicious files can craft values targeting overflow.
- Signed pointer values must be treated as raw bit patterns (reinterpret as unsigned), not as mathematical negatives. This is a libmagic compatibility requirement.

## Test Coverage

35 unit tests covering:

- All pointer types (byte, short, long, quad) with both endiannesses
- Signed and unsigned pointer values
- Positive and negative adjustments
- From-end base offsets
- Pointer read buffer overruns
- Final offset buffer overruns
- Arithmetic overflow and underflow
- Unsupported pointer types (string, float, double)
- PE-header-style real-world scenario (0x3C pointer)
- 32-bit platform awareness (conditional assertions)

## Related

- Issue #38: Relative offset resolution (next offset type to implement)
- `evaluator/offset/absolute.rs`: Reused for base offset resolution
- `evaluator/types/`: `read_byte`, `read_short`, `read_long`, `read_quad` reused for pointer reading
- GOTCHAS.md S5.1: `usize::from(u32)` does not compile on 32-bit targets
