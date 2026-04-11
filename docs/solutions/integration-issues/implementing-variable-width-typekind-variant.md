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

**AST shape:** `RegexFlags { case_insensitive, start_offset }` captures the `/c` and `/s` modifiers; the `/l` line-based scan window lives on a separate `RegexCount` enum (see below) so "byte count" and "line count" are mutually exclusive at the type level. `TypeKind::Regex { flags: RegexFlags, count: RegexCount }` pairs them. `TypeKind::Search { range: NonZeroUsize }` takes a mandatory non-zero range (bare `search` and `search/0` are parse errors). The struct-of-flags + enum-of-counts shape keeps future flag additions from cascading through the ~10 exhaustive-match sites called out in GOTCHAS S2.1.

`RegexCount` has three variants:

```rust
pub enum RegexCount {
    /// Plain `regex`: default 8192-byte scan window.
    Default,
    /// `regex/N`: byte-bounded scan, clamped to 8192.
    Bytes(NonZeroU32),
    /// `regex/Nl` (Some) or `regex/l` (None): line-bounded scan.
    Lines(Option<NonZeroU32>),
}
```

`RegexCount::Lines(None)` is behaviorally equivalent to `RegexCount::Default` — both walk the full 8192-byte capped window — but the two variants are kept distinct at the AST level to preserve magic-file surface syntax round-tripping.

**Regex reader** (`src/evaluator/types/regex.rs`): `build_regex` unconditionally enables multi-line mode (matching libmagic's `REG_NEWLINE` in `softmagic.c::alloc_regex`) and disables `.`-matches-newline. The `RegexCount::Lines` variant does *not* toggle regex compilation — it controls only the scan window. The `compute_window` helper applies the 8192-byte `FILE_REGEX_MAX` cap unconditionally, then dispatches on the `RegexCount` variant: byte-bounded cases take the first `min(n, 8192)` bytes; line-bounded cases walk LF / CRLF / bare CR terminators until the Nth:

```rust
fn build_regex(pattern: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(true)
        .dot_matches_new_line(false)
        .build()
}

fn compute_window(buffer: &[u8], offset: usize, count: RegexCount) -> &[u8] {
    // min(requested count, 8192, remaining buffer) for byte mode;
    // Lines(Some(n)) walks LF / CRLF / bare CR terminators within that cap;
    // Default and Lines(None) share a match arm -- both return the full cap.
}

pub fn read_regex(
    buffer: &[u8],
    offset: usize,
    pattern: &str,
    flags: RegexFlags,
    count: RegexCount,
) -> Result<Option<Value>, TypeReadError> {
    // BufferOverrun guard, compile, scan compute_window(...) for first match.
    // Returns Some(Value::String(matched)) on hit, None on miss.
}
```

**Search reader** (`src/evaluator/types/search.rs`): takes `NonZeroUsize` range and returns `Option<Value>`. `None` is the structured "no match" signal:

```rust
pub fn read_search(
    buffer: &[u8],
    offset: usize,
    pattern: &[u8],
    range: NonZeroUsize,
) -> Result<Option<Value>, TypeReadError> {
    // BufferOverrun guard, window = &remaining[..min(range, remaining.len())],
    // memchr::memmem::find -> Ok(Some(pattern)) / Ok(None).
}
```

The `Option` is load-bearing: it lets the engine distinguish a zero-width regex match (e.g., `^`, `a*`, lookaheads) from a genuine miss. Both would otherwise collapse to `Value::String(String::new())`.

**Anchor advance:** In `bytes_consumed_with_pattern`, the `Regex` arm calls `regex_bytes_consumed(buffer, offset, pattern, flags, count: RegexCount)` which re-runs the compiled regex inside `compute_window` and returns `m.end()` by default, or `m.start()` when `flags.start_offset` is set (the `/s` flag, matching libmagic's `REGEX_OFFSET_START`). The `Search` arm re-runs `memchr::memmem::find` against the window and returns `match_idx + pattern.len()` — the byte just past the matched needle, matching GNU `file`'s `softmagic.c` `FILE_SEARCH` path where `ms->search.offset += idx` and then `moffset()` adds `vlen = m->vallen`. An earlier revision of this PR advanced by the full window size (`range`); that was wrong and caused relative-offset children to land far past the intended byte. Both `regex_bytes_consumed` and the `Regex`/`Search` arms in `bytes_consumed_with_pattern` fire `debug_assert!` on engine-invariant violations (missing pattern, invalid pattern variant) so dev/test builds catch caller bugs loudly.

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

- **Unit tests for `read_regex` and `read_search`**: basic match, no-match, case-insensitive flag, non-zero offset handling, bounded search range, invalid/unparseable pattern error path, and binary (non-UTF-8) buffer handling.
- **Line-based scan window tests**: single-line, multi-line, CRLF and bare-CR terminator handling, explicit count honored, count larger than available lines degrading to "scan to end of capped window".
- **8192-byte boundary tests**: pattern ending exactly at byte 8191 (must match), pattern starting at byte 8192 (must miss), pattern straddling the cap (must miss), line-based scan also respecting the cap. These guard against off-by-one regressions in the `FILE_REGEX_MAX` enforcement, which is security-critical (the cap is part of the DoS mitigation when `EvaluationConfig::default()` has no timeout, per GOTCHAS S13.1).
- **Zero-width match tests**: `read_regex` with pattern `^` must return `Some(Value::String(""))` on a non-empty buffer, not `None`. Pattern `a*` against `"xyz"` must match at position 0 with an empty match string.
- **`/s` flag tests**: `regex_bytes_consumed` with `start_offset: true` returns `m.start()` instead of `m.end()`, verified against a fixed buffer where match-start and match-end are known constants.
- **Non-equality operator rejection tests**: `regex < pattern`, `search & mask`, etc. must return `TypeReadError::UnsupportedType` rather than silently comparing matched bytes to the pattern literal.
- **Anchor-advance regression tests**: a child rule with `OffsetSpec::Relative(0)` after a successful `Regex` or `Search` parent must resolve to match-end, not window-end. Used as the end-to-end regression guard against the "search advances by window size" bug.
- **Parser last-wins rejection**: `regex/1l2l`, `regex/1c2l`, `regex/l1l2` must all be parse errors (we hard-reject duplicate counts rather than silently accepting the last one per libmagic's historical behavior).
- **Sibling-after-regex integration test.** Construct a `MagicRule` tree where a `Regex` parent match is followed by a sibling with `OffsetSpec::Relative(+K)`; verify the sibling reads from `anchor + K`, not from absolute `K`. Repeat for `Search` and for `Relative(-K)` to cover both directions.
- **Property test hook.** `arb_type_kind` in `tests/property_tests.rs` generates `RegexFlags { case_insensitive, start_offset }` and a `RegexCount` variant (Default / Bytes / Lines) for regex, and `NonZeroUsize` for search, so the codegen round-trip and strength-calculation invariants exercise the new variants automatically.
- **Corpus integration tests** (`tests/regex_search_corpus_tests.rs`): searchbug, json1, jsonlines1, cmd1, gedcom, regex-eol. Models the blocked corpus files from issue #39 by constructing equivalent rule trees either programmatically or (preferred where the syntax permits) via `parse_text_magic_file`.

## Related Documentation

- `GOTCHAS.md` S2.1 — TypeKind exhaustive-match checklist across 10+ files (ast, grammar, types, codegen, strength, property_tests, evaluator/types, output, grammar/tests); catch-all arms in `bytes_consumed` will fire `debug_assert` for variable-width variants.
- `GOTCHAS.md` S3.1 — parser type-keyword split between `src/parser/types.rs` (`parse_type_keyword` / `type_keyword_to_kind`) and `src/parser/grammar/mod.rs` for suffixes.
- `GOTCHAS.md` S1.2 / S1.3 — build.rs / codegen serialization boundary and generated-import sync (`generate_builtin_rules` in `src/parser/codegen.rs`).
- `GOTCHAS.md` S3.8 — `bytes_consumed` as source of truth for `EvaluationContext::last_match_end` anchor advance.
- `GOTCHAS.md` S8.1 — `enum_variant_names` clippy guidance for same-suffix variants; S10.3 — public enum variants require `# Examples` rustdoc (clippy enforced).
- `AGENTS.md` "Adding New Type Support" — 7-step procedure for new `TypeKind` variants.
- GitHub issue **#39** — parent ticket tracking regex and search type evaluator support.

No prior solution doc specifically covers regex/search type matching, the build.rs/codegen indirect-error surface, or `clippy::doc_markdown` fixes.
