# Issue #235 — Implement Search-Type Flag Semantics

## Issue Summary

GitHub: [EvilBit-Labs/libmagic-rs#235](https://github.com/EvilBit-Labs/libmagic-rs/issues/235) Milestone: v0.3.0 — Advanced Features Labels: enhancement, parser, evaluator, compatibility, testing, priority:normal Epic: #54 (Type System Expansion) Sibling work: #234 (string flags — landed); GOTCHAS S2.6 (search anchor advance).

`search/N/<flags>` flag letters are currently parsed-and-dropped (warn-logged) by `parser::grammar::type_suffix::parse_search_suffix`. The evaluator ignores them, which silently corrupts relative-offset child resolution for any `search/N/s` rule and mis-matches `/c`/`/C`/`/w`/`/W`/`/b`/`/B`/`/t`/`/T` rules against real magic files.

## Problem Statement

magic(5) defines flag suffixes on `search` rules that alter scan and anchor-advance semantics. The currently-affected real-world rules include:

| File        | Line | Rule fragment                             | Flag impact                                                        |
| ----------- | ---- | ----------------------------------------- | ------------------------------------------------------------------ |
| `images`    | 114  | `search/4261301/s TRUEVISION-XFILE.\0`    | `/s` — TGA footer; children walk backwards from match-START        |
| `python`    | 219  | `search/1/w #!\040/usr/bin/python`        | `/w` — shebang detection with whitespace flexibility               |
| `macintosh` | 17   | `search/2652/b (This\ file\ `             | `/b` — BinHex blank-handling hint                                  |
| `fonts`     | 260  | `search/432/s name`                       | `/s` — sfnt name table                                             |
| `archive`   | 1427 | `>>30 search/100/b !application/epub+zip` | `/b` — parsed/stored; comparison-time EPUB negation depends on #51 |

`/s` is the most load-bearing for correctness because it changes the previous-match anchor from match-END (`match_idx + pattern.len()`) to match-START (`match_idx`), so relative-offset children of the search rule read from the wrong position when `/s` is dropped. The other flags share semantics with `string` (#234) and reuse `compare_string_with_flags`.

## Technical Approach

Mirror the `RegexFlags` / `StringFlags` pattern that already exists in `parser::ast`. Specifically:

1. Add a new `SearchFlags` struct (bool-per-flag, `Default`, `Copy`, `Serialize`, `Deserialize`, builder setters) shaped like `RegexFlags`. Use the **same eight fields as `StringFlags`** plus one search-specific `start_anchor` field for `/s`. The name `start_anchor` is deliberate — magic(5) describes `/s` as the "search-start" flag, where match-START becomes the new anchor. `RegexFlags` uses `start_offset` for the parallel concept; the two are intentionally different surface names because regex's `/s` lives next to byte/line counts ("offset") while search's `/s` lives next to a single scan window ("anchor"). Resolved at review time, 2026-05-27. Add a `SearchFlags::to_string_flags() -> StringFlags` accessor that drops `start_anchor` and forwards the remaining eight fields. The two structs must stay structurally parallel — when `StringFlags` grows a ninth field, `SearchFlags` gains the same field and `to_string_flags()` is extended in lockstep. This is the committed handoff between the search reader and `compare_string_with_flags`; no comparator refactor, no shim duplication.
2. Extend `TypeKind::Search { range, flags }`.
3. Have `parse_search_suffix` *return* the parsed flags instead of consuming and warning-then-dropping them.
4. Route the literal-pattern match through `compare_string_with_flags` from `evaluator::types::string` (via `SearchFlags::to_string_flags()`) — the eight shared flag semantics are already implemented and table-tested.
5. In `search_bytes_consumed`, gate the return value on `flags.start_anchor`: match-START (`match_idx`) when set, match-END (`match_idx + matched_len`) when clear. `matched_len` is the length of the pattern slice the comparator actually inspected — equal to `pattern.len()` when `/T` is clear, and the trimmed length when `/T` is set. This mirrors the existing flagged-string convention in `read_pattern_match` (`types/mod.rs`), which trims at the boundary and reports trimmed length for `bytes_consumed`. Per GOTCHAS S2.6 the engine threads the pattern through `bytes_consumed_with_pattern` for `TypeKind::Search`, so the re-scan stays cheap.
6. Update codegen, strength, property-test arbitrary, and all exhaustive matches per GOTCHAS S2.1.

**Fast-path vs slow-path selection.** Split the dispatcher on flag *category*, not on `SearchFlags::is_empty()`. Two categories matter:

- **Comparison-altering flags** (`/c`, `/C`, `/w`, `/W`, `/T`, `/f`) — change which buffer bytes count as a match. Force the byte-by-byte scan calling `compare_string_with_flags` at each candidate offset.
- **Anchor-only flags** (`/s`, `/t`, `/b`) — do not change the match decision. `/s` only changes anchor-advance; `/t`/`/b` are MIME hints. Keep the SIMD-accelerated `memchr::memmem::find` fast path; only the anchor-advance step reads `flags.start_anchor`.

This preserves the TGA-footer fast path (`search/4261301/s`, 4 MiB window) that motivates the entire rewrite. `SearchFlags::needs_byte_compare()` returns `true` when any comparison-altering flag is set; the dispatcher uses it to choose the path.

## Implementation Plan

Each phase is independently testable and small enough for a focused PR. TDD: write the test for the behavior being added, watch it fail, then add the production code.

### Phase 1 — AST + serialization (Red → Green)

1. Add `SearchFlags` struct to `src/parser/ast.rs` (next to `StringFlags`). Nine bool fields: the eight from `StringFlags` plus `start_anchor`.
2. Extend `TypeKind::Search { range, flags: SearchFlags }`.
3. Update `serialize_type_kind` in `src/parser/codegen.rs` to emit the new field.
4. Update `arb_type_kind` in `tests/property_tests.rs` to generate non-default flags.
5. Add `#[allow(clippy::struct_excessive_bools)]` on `SearchFlags` with the same design-note comment used by `StringFlags`.

### Phase 2 — Parser

1. Rewrite the flag-letter loop in `parse_search_suffix` to set the matching `SearchFlags` field instead of just consuming the letter. Accept duplicate flag letters idempotently — `search/256/cc` sets `ignore_lowercase` twice with no side effect, matching libmagic's per-letter `STRING_*` bitfield accumulation. Reject unknown letters the same way it does today.
2. Remove the `warn!(... "parsed but not yet evaluated")` log.
3. Update the suffix tests in `src/parser/grammar/type_suffix.rs` to assert that each individual letter sets the right field, and that combinations like `search/256/cs` and `search/256/Ww` round-trip correctly.
4. Verify the trailing-junk and operator-boundary rules (`=`, `!`, `<`, `>`, `&`, `^`, `~`, `x`) still gate parsing.

### Phase 3 — Evaluator (the load-bearing work)

1. In `src/evaluator/types/search.rs`, extend `read_search` to accept `SearchFlags`. Two paths, selected by `SearchFlags::needs_byte_compare()`:

   - `false` (no comparison-altering flags; `/s`/`/t`/`/b` may still be set) → existing `memchr::memmem::find` fast path. Untouched for performance.
   - `true` (`/c`, `/C`, `/w`, `/W`, `/T`, or `/f` set) → byte-by-byte scan over the window, calling `compare_string_with_flags(buf, &searchflags.to_string_flags())` at each candidate offset. Track the start index of the matching slice so anchor-advance can use it.

2. Extend `search_bytes_consumed` to take `SearchFlags` and the matched-slice length. Return `match_idx` when `flags.start_anchor`, else `match_idx + matched_len`. `matched_len` is computed at the boundary: when `/T` is set, trim the pattern with `trim_ascii_whitespace` and use the trimmed length; otherwise use `pattern.len()`. The same trimmed slice feeds both `compare_string_with_flags` and the anchor-advance computation, so the two cannot disagree. This mirrors `read_pattern_match` in `src/evaluator/types/mod.rs`, which already enforces trim-at-boundary for flagged `string` rules.

   **Pre-merge checkpoint:** confirm the `matched_len` policy against GNU `file` `softmagic.c::moffset` before merging Phase 3.

3. Thread `flags` through call sites in `src/evaluator/types/mod.rs` (`read_pattern_match`, `bytes_consumed_with_pattern`) and `src/evaluator/engine/mod.rs` (the `TypeKind::Regex | TypeKind::Search` arm).

4. `/t` and `/b` remain MIME-output hints with no comparison effect today, same as `StringFlags::text_test` / `StringFlags::bin_test`. Document with a doc comment that the parser captures them for the future `!:mime` work (#51).

### Phase 4 — Tests

Each flag combination needs positive and negative coverage. The matrix is:

| Flag               | Positive test                                             | Negative test                                                           |
| ------------------ | --------------------------------------------------------- | ----------------------------------------------------------------------- |
| default (no flags) | byte-exact match returns `Some(Value::String(...))`       | non-match returns `Ok(None)`                                            |
| `/s`               | relative-offset child resolves at match-START             | clearing `/s` returns to match-END                                      |
| `/c`               | lowercase pattern matches uppercase buffer                | uppercase-letter positions stay literal (asymmetric — see GOTCHAS S6.5) |
| `/C`               | uppercase pattern matches lowercase buffer                | lowercase-letter positions stay literal                                 |
| `/w`               | pattern whitespace matches zero-or-more buffer whitespace | non-whitespace mismatch still fails                                     |
| `/W`               | pattern whitespace requires ≥1 buffer whitespace          | zero buffer whitespace fails                                            |
| `/T`               | leading/trailing whitespace in pattern is trimmed         | trim does not move the match index in the buffer                        |
| `/b`               | parses, captured, no comparison change                    | (no comparison-side regression)                                         |
| `/t`               | parses, captured, no comparison change                    | (no comparison-side regression)                                         |

Conformance tests against GNU `file` for the four real-world rules listed in the Problem Statement. The archive:1427 EPUB negation test unblocks #51 and should be added even though full ZIP support lands separately.

### Phase 5 — Cleanup

1. Remove the `issue #235` reference comment from `parse_search_suffix`.
2. Update AGENTS.md "Currently Implemented" section for `TypeKind::Search` to list the new flag set.
3. Update the GOTCHAS S2.6 note: the match-end vs window-end fix stays, and the new `/s` axis adds match-start. Document the `flags.start_anchor` decision point.
4. Add a `docs/solutions/design-patterns/` entry once the PR has run through CI to capture the parallel-with-StringFlags pattern for future flag-bearing types.

## Test Plan

- [ ] **Unit tests** added to `src/evaluator/types/search.rs` for each row in the matrix above.
- [ ] **Integration tests** in `tests/evaluator_tests.rs` covering: - [ ] TGA footer (`search/N/s ... TRUEVISION-XFILE.\0`) with relative-offset children reading correct bytes from match-START - [ ] Python shebang (`search/1/w`) matching `#! /usr/bin/python` (note the extra space) - [ ] BinHex (`search/2652/b`) loads and matches - [ ] sfnt name table (`search/432/s`) relative child resolves correctly
- [ ] **Property test** that any `SearchFlags` round-trips through codegen and `serde` (extend `arb_type_kind`).
- [ ] **`cargo nextest run`** passes with zero failures.
- [ ] **`cargo llvm-cov --fail-under-lines 85`** stays green.
- [ ] **`cargo clippy -- -D warnings`** stays green.
- [ ] **`just ci-check`** passes (per memory: always before commit).
- [ ] **Conformance**: pipe the four real-world fixtures through `libmagic-rs file` and confirm the output matches GNU `file` for the same input.

## Files to Modify

| Path                                 | Change                                                                                                           |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| `src/parser/ast.rs`                  | Add `SearchFlags` struct; extend `TypeKind::Search`                                                              |
| `src/parser/grammar/type_suffix.rs`  | `parse_search_suffix` returns `(NonZeroUsize, SearchFlags)`; per-letter assignment + tests                       |
| `src/parser/grammar/mod.rs`          | Wire the new return shape into the type-keyword dispatch (any call site that destructures `parse_search_suffix`) |
| `src/parser/codegen.rs`              | `serialize_type_kind` for the new field                                                                          |
| `src/evaluator/types/search.rs`      | `read_search` + `search_bytes_consumed` take and act on `SearchFlags`; flag-aware byte-walk                      |
| `src/evaluator/types/mod.rs`         | Thread `flags` through `read_pattern_match` and `bytes_consumed_with_pattern` for `TypeKind::Search`             |
| `src/evaluator/engine/mod.rs`        | Pass `flags` into the pattern-bearing dispatch arm                                                               |
| `src/evaluator/strength.rs`          | Confirm strength match arm still compiles; no scoring change unless a flag clearly warrants it                   |
| `src/build_helpers.rs`               | Update the `TypeKind::Search` construction site so build-time codegen stays in sync                              |
| `tests/property_tests.rs`            | `arb_type_kind` generates non-default `SearchFlags`                                                              |
| `tests/evaluator_tests.rs`           | New integration tests for the four real-world rules                                                              |
| `src/evaluator/types/tests.rs`       | Update 3 `TypeKind::Search { ... }` struct-literal sites for the new `flags` field                               |
| `src/evaluator/engine/tests/mod.rs`  | Update 4 `TypeKind::Search { ... }` struct-literal sites                                                         |
| `src/parser/grammar/tests/mod.rs`    | Update 2 `TypeKind::Search { ... }` struct-literal sites                                                         |
| `tests/regex_search_corpus_tests.rs` | Update 1 `TypeKind::Search { ... }` struct-literal site                                                          |
| `AGENTS.md`                          | Update "Currently Implemented" `Search type` paragraph                                                           |
| `GOTCHAS.md`                         | Extend S2.6 with the `/s` match-start branch                                                                     |

## Files to Create

| Path                                                                          | Purpose                                                                                       |
| ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `docs/solutions/design-patterns/search-flags-parallel-stringflags-2026-05.md` | After merge — capture the parallel-with-StringFlags pattern for future flag-bearing TypeKinds |

## Success Criteria

- [ ] `TypeKind::Search` carries a `SearchFlags` field, fully serialized through `serde` and codegen.
- [ ] `search/N/s` rules resolve `&N` children against match-START. The TGA footer fixture matches GNU `file` output.
- [ ] `search/N/c`, `/C`, `/w`, `/W`, `/T` produce the same match decisions as GNU `file` on the four real-world fixtures plus the eight unit-test cases from the matrix above.
- [ ] `/b` and `/t` parse, are captured on `SearchFlags`, and produce no comparison-time regression versus default-flag rules.
- [ ] Zero clippy warnings; `just ci-check` is green.
- [ ] Test coverage ≥85% per project policy.
- [ ] AGENTS.md and GOTCHAS.md reflect the implemented behavior (not aspirational).

## Out of Scope

- **`!:mime` / `!:ext` / `!:apple` directive evaluation** — issue #51 / v0.6.0. `/t` and `/b` are wired as MIME-output hints only; surfacing them through output metadata is a separate task.
- **Aho-Corasick multi-pattern optimization** — v1.0.0 work; unrelated to flag semantics.
- **ZIP/OOXML/OASIS/HWPX/EPUB/JAR full detection** — issue #51. The `/b` flag is parsed and stored under this change, but its comparison-time semantics remain deferred to `!:mime` evaluation. The archive:1427 rule loads successfully after this change; its EPUB-vs-generic-ZIP negation depends on full ZIP-content inspection landing separately in v0.6.0.
- **Magic-file `&+N` / `&-N` relative-offset *parsing*** — separate work item; evaluator already supports relative offsets programmatically.
- **Changing the `memchr::memmem::find` fast path for default-flag rules** — the parallel-walk is reserved for non-default `SearchFlags` to keep the common-case scan time unchanged.
