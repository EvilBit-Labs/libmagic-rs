---
title: Implementing regex and search evaluator types in libmagic-rs
category: integration-issues
date: 2026-04-10
tags: [rust, evaluator, regex, search, typekind, libmagic, exhaustive-match]
severity: medium
components: [evaluator/types, parser/codegen, parser/grammar]
related_issues: [39]
---

## Problem

Implementing evaluator support for `TypeKind::Regex` and `TypeKind::Search` in libmagic-rs exposed five interlocking issues: a stale `regex` crate feature flag, a dispatch signature that could not carry pattern operands to the type-reading layer, a missing anchor-advance path for variable-width regex matches, a build-script exhaustive-match failure that surfaced before the library error, and a clippy `doc_markdown` lint on module-level docs.

## Root Cause

1. `regex` v1.12+ exposes `regex::bytes::RegexBuilder` unconditionally; declaring `features = ["bytes"]` references a feature that no longer exists, so cargo rejects the manifest.
2. `read_typed_value(buffer, offset, type_kind)` was designed for fixed-shape numeric and string types that need only the buffer and offset. Regex and Search are fundamentally different — they require the rule's *value operand* (the pattern) at read time to compile the regex or locate the needle.
3. `bytes_consumed` (the source of truth for advancing `EvaluationContext::last_match_end` per GOTCHAS.md S3.8) re-derives consumption from the buffer for variable-width types. Regex matches have buffer-dependent lengths, so the anchor advance cannot be computed without re-running the regex.
4. `src/parser/codegen.rs` is included by `build.rs` via `#[path]` (GOTCHAS.md S1.2). Adding `TypeKind` variants breaks `serialize_type_kind`'s exhaustive match, and cargo surfaces the build-script compilation failure *before* the library error — a trap not previously documented in S2.1.
5. Clippy's pedantic `doc_markdown` lint flags unquoted identifiers like `TypeKind` in rustdoc, and each identifier must be individually backticked.

## Solution

**Manifest fix:** Drop the nonexistent feature flag in `Cargo.toml`:

```toml
regex = "1.12.3"
```

**Dispatch threading:** Add `read_typed_value_with_pattern(buffer, offset, type_kind, pattern: Option<&Value>)` as a new entry point alongside the existing 3-arg `read_typed_value`, which becomes a thin wrapper that forwards `pattern: None`. The engine calls the pattern-aware form uniformly; the 3-arg convenience wrapper is retained so the ~30 existing call sites (`read_typed_value(buf, off, &kind)`) compile unchanged. Add a parallel `bytes_consumed_with_pattern` so the anchor-advance path can reach the pattern operand for `TypeKind::Regex` and `TypeKind::Search`.

Additionally, expose a `read_pattern_match(buffer, offset, type_kind, pattern) -> Result<Option<Value>, TypeReadError>` helper for the engine's pattern-bearing code path. `Option<Value>` is the structured "no match" signal: a genuine miss returns `None`, while a legitimate zero-width regex match (e.g., `^`, `a*`, lookaheads) returns `Some(Value::String(String::new()))`. `read_typed_value_with_pattern` collapses `None` to `Value::String(String::new())` for back-compat with the single-`Value` return shape; the engine path uses `read_pattern_match` directly and drives its own `Equal`/`NotEqual` decision from the `Option` discriminant.

**Regex reader** (`src/evaluator/types/regex.rs`) — uses a `build_regex` helper that wraps the pattern in `^(?:...)` when `/l` is set so bare, unanchored patterns cannot match mid-line:

```rust
fn build_regex(
    pattern: &str,
    case_insensitive: bool,
    start_of_line: bool,
) -> Result<Regex, regex::Error> {
    let owned;
    let effective_pattern: &str = if start_of_line {
        owned = format!("^(?:{pattern})");
        &owned
    } else {
        pattern
    };
    RegexBuilder::new(effective_pattern)
        .case_insensitive(case_insensitive)
        .multi_line(start_of_line)
        .build()
}

pub fn read_regex(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    case_insensitive: bool,
    start_of_line: bool,
) -> Result<Option<Value>, TypeReadError> {
    if offset >= buffer.len() { return Err(BufferOverrun { .. }); }
    let regex = build_regex(pattern, case_insensitive, start_of_line)
        .map_err(|e| UnsupportedType {
            type_name: format!("regex compile error: {e}"),
        })?;
    let remaining = &buffer[offset..];
    Ok(regex.find(remaining).map(|m| {
        Value::String(String::from_utf8_lossy(m.as_bytes()).into_owned())
    }))
}
```

**Search reader** (`src/evaluator/types/search.rs`):

```rust
pub fn read_search(
    buffer: &[u8],
    offset: usize,
    pattern: &[u8],
    range: Option<usize>,
) -> Result<Option<Value>, TypeReadError> {
    if offset >= buffer.len() { return Err(BufferOverrun { .. }); }
    let remaining = &buffer[offset..];
    let window_len = range.map_or(remaining.len(), |n| n.min(remaining.len()));
    let window = &remaining[..window_len];
    Ok(memchr::memmem::find(window, pattern).map(|_| {
        Value::String(String::from_utf8_lossy(pattern).into_owned())
    }))
}
```

`None` is the structured "no match" signal, which lets the engine distinguish a zero-width regex match from a genuine miss without reusing `Value::String(String::new())` as a sentinel.

**Anchor advance:** In `bytes_consumed_with_pattern`, the `Regex` arm re-runs the regex via `regex_bytes_consumed(...)` and returns `m.end()`. The `Search` arm re-runs `memchr::memmem::find` against the window and returns `match_idx + pattern.len()` — the byte just past the matched needle, matching GNU `file`'s `softmagic.c` `FILE_SEARCH` path where `ms->search.offset += idx` and then `moffset()` adds `vlen = m->vallen`. An earlier revision of this PR advanced by the full window size (`range`); that was wrong and caused relative-offset children to land far past the intended byte.

**Engine pattern-bearing code path:** In `evaluate_single_rule_with_anchor`, split the flow into two arms. For `TypeKind::Regex | Search`, call `read_pattern_match` and translate its `Option` result directly into `Equal` (`Some` → match) / `NotEqual` (`None` → match) — no `apply_operator` call. Any other operator on a pattern-bearing type is rejected as `TypeReadError::UnsupportedType` because it has no well-defined semantics (ordering a matched string against the pattern literal produces nonsense). For all other types, continue through `read_typed_value_with_pattern` + `coerce_value_to_type` + `apply_operator` as before.

**Codegen:** Add `Regex { .. }` and `Search { .. }` arms to `serialize_type_kind` in `src/parser/codegen.rs`. Verify `cargo check` against `build.rs` output, not just the library.

**Doc lint:** Backtick identifiers individually in module docs: `` //! Implements the `regex` `TypeKind`. ``

## Prevention

- **Verify crate features on docs.rs before adding them.** The `regex` crate dropped the `bytes` feature by v1.12 (`regex::bytes` is unconditional). Check `https://docs.rs/<crate>/<version>/` for the exact feature list before editing `Cargo.toml`. A wasted `cargo build` cycle is the cheap failure mode; a silently-disabled feature is the expensive one.
- **When adding a `TypeKind` variant, walk GOTCHAS S2.1 in order, then verify the build.rs pipeline.** The hidden site is `serialize_type_kind` in `src/parser/codegen.rs` — it is included via `#[path]` in `build.rs`, so omissions surface as confusing `E0004`/`E0599` errors from `build.rs` *before* any library file compiles. Run `cargo clean && cargo check` after editing `TypeKind` to shake these out early.
- **`bytes_consumed` is load-bearing for relative offsets.** Any variable-width variant (`Regex`, `Search`, `String`, `PString`, future additions) MUST have an explicit arm in `bytes_consumed` in `src/evaluator/types/mod.rs`. The catch-all `_ =>` arm fires a `debug_assert` in dev/test, but release builds will silently corrupt the GNU `file` anchor for any downstream `Relative(N)` sibling. Treat missing arms as a correctness bug, not a lint.
- **Sibling functions beat signature extensions when the new concern is narrow.** The earlier design in this solution suggested extending `read_typed_value` in place; the current implementation instead added a sibling `read_typed_value_with_pattern` and kept `read_typed_value` as a zero-cost wrapper. The sibling approach avoided updating ~30 existing call sites that would otherwise have to pass `None` for the new argument. When only a narrow slice of callers needs the new capability, a sibling function is cheaper and easier to review.
- **Do not overload `Value::String("")` as a "no match" sentinel.** A zero-width regex match (`^`, `a*`, lookaheads) returns a valid empty matched string that is not a miss. Use `Result<Option<Value>, _>` or a dedicated sentinel variant when the reader needs to distinguish "found nothing" from "found zero bytes." The engine path must work from the `Option`, not from `is_empty()`.
- **Search advances by match-end, not window-end.** The GNU `file` contract is `anchor += match_idx + pattern.len()`; the full search window size is only used as a bound on the scan. Getting this wrong silently corrupts relative-offset children of every successful search rule with no test failure for any rule that does not chain children.
- **Pattern-bearing types reject non-equality operators.** `regex < "foo"` and `search & 0xff` are magic-file semantic bugs. The engine should return a structured error rather than falling through to `apply_operator`, which produces garbage ordering comparisons against the pattern literal.
- **Backtick every Rust identifier individually in doc comments.** Clippy `doc_markdown` fires on bare `TypeKind` even inside a sentence like "extends `read_typed_value` for TypeKind::Regex". Write `` `TypeKind::Regex` `` as a separate backticked span.

## Testing

- **Unit tests for `read_regex` and `read_search`** (added this session): basic match, no-match, case-insensitive flag, start-of-line anchor, non-zero offset handling, bounded search range, invalid/unparseable pattern error path, and binary (non-UTF-8) buffer handling.
- **Start-of-line anchoring negative test.** With `/l` enabled, a bare (unanchored) pattern like `"line"` that appears only mid-line must return the empty-string no-match. The `build_regex` helper's `^(?:...)` wrapper is what makes this correct — test it explicitly so a future refactor does not regress.
- **Anchor-advance regression tests.** After a successful `Regex` or `Search` match at offset `O` consuming `N` bytes, assert `EvaluationContext::last_match_end() == O + N`. Add a parallel test for the no-match path (anchor must not advance).
- **Sibling-after-regex integration test.** Construct a `MagicRule` tree where a `Regex` parent match is followed by a sibling with `OffsetSpec::Relative(+K)`; verify the sibling reads from `anchor + K`, not from absolute `K`. Repeat for `Search` and for `Relative(-K)` to cover both directions.
- **Property test hook.** Add `Regex` and `Search` arms to `arb_type_kind` in `tests/property_tests.rs` so the codegen round-trip and strength-calculation invariants exercise the new variants automatically.

## Related Documentation

- `GOTCHAS.md` S2.1 — TypeKind exhaustive-match checklist across 10+ files (ast, grammar, types, codegen, strength, property_tests, evaluator/types, output, grammar/tests); catch-all arms in `bytes_consumed` will fire `debug_assert` for variable-width variants.
- `GOTCHAS.md` S3.1 — parser type-keyword split between `src/parser/types.rs` (`parse_type_keyword` / `type_keyword_to_kind`) and `src/parser/grammar/mod.rs` for suffixes.
- `GOTCHAS.md` S1.2 / S1.3 — build.rs / codegen serialization boundary and generated-import sync (`generate_builtin_rules` in `src/parser/codegen.rs`).
- `GOTCHAS.md` S3.8 — `bytes_consumed` as source of truth for `EvaluationContext::last_match_end` anchor advance.
- `GOTCHAS.md` S8.1 — `enum_variant_names` clippy guidance for same-suffix variants; S10.3 — public enum variants require `# Examples` rustdoc (clippy enforced).
- `AGENTS.md` "Adding New Type Support" — 7-step procedure for new `TypeKind` variants.
- GitHub issue **#39** — parent ticket tracking regex and search type evaluator support.

No prior solution doc specifically covers regex/search type matching, the build.rs/codegen indirect-error surface, or `clippy::doc_markdown` fixes.
