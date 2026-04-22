# Development Gotchas & Pitfalls

This document tracks non-obvious behaviors, common pitfalls, and architectural "gotchas" in the libmagic-rs codebase to assist future maintainers and contributors.

## 1. Build Script Boundary

### 1.1 `error.rs` is Shared with `build.rs`

`src/error.rs` is compiled by both the library and `build.rs`. It cannot reference lib-only types like `crate::io::IoError`.

- **Workaround:** `FileError(String)` wraps structured I/O errors as strings.
- **Rule:** Use `ParseError::IoError` for I/O errors in parser code, not `ParseError::invalid_syntax`.

### 1.2 Serialization Code Sharing

Serialization functions live in `src/parser/codegen.rs`, shared by both `build.rs` (via `#[path]` include) and `src/build_helpers.rs` (via `crate::parser::codegen`). Only `format_parse_error` remains duplicated because `ParseError` has different import paths in each context.

### 1.3 Generated Imports Must Stay in Sync

`generate_builtin_rules()` emits a `use crate::parser::ast::{...}` import line in generated code. When adding new AST types (e.g., `PStringLengthWidth`), this import must be updated or the build pipeline breaks on any rule that uses the new type.

- **Symptom:** Build fails with `E0433` ("cannot find type") in the generated `builtin_rules.rs`.
- **Fix:** Update the import in `generate_builtin_rules()` in `src/parser/codegen.rs`.

## 2. Adding Enum Variants

### 2.1 `TypeKind` Exhaustive Matches

Adding a variant to `TypeKind` requires updating exhaustive matches in 10+ files: `ast`, `grammar`, `types`, `codegen` (`serialize_type_kind` -- easy to forget; build.rs is a separate compilation unit so the error surfaces there first), `strength`, `property_tests`, `evaluator/types/mod.rs` (`read_typed_value`, `coerce_value_to_type`, **`bytes_consumed`** -- variable-width variants must be matched explicitly or relative-offset anchors will silently corrupt), `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), and `grammar/tests.rs` (stale assertions). Note: `coerce_value_to_type`, output matches, and `bytes_consumed` use catch-all `_ =>` so they compile without changes but may need semantic updates -- `bytes_consumed` will fire a `debug_assert` in test/dev builds for unhandled variable-width variants.

### 2.2 `Operator` Exhaustive Matches

Adding a variant to `Operator` requires updating: `ast`, `grammar`, `codegen`, `strength`, `property_tests`, `evaluator/operators/`.

### 2.3 `Value` Exhaustive Matches

Adding a variant to `Value` requires updating: `ast`, `codegen`, `strength`, `property_tests`, `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), `evaluator/operators/comparison.rs` (`compare_values`). Bitwise/equality operators use catch-all `_ =>` so they are safe.

- **Note:** `Value` no longer derives `Eq` (removed when `Value::Float(f64)` was added) -- no production code depends on `Value: Eq`.

### 2.4 Pattern-Bearing Types Bypass `apply_operator` in the Engine

`TypeKind::Regex` and `TypeKind::Search` are evaluated by **logical match** in `evaluate_single_rule_with_anchor` (`src/evaluator/engine/mod.rs`), not by string equality against `rule.value`. The engine calls `types::read_pattern_match`, which returns `Result<Option<Value>, _>`: `Some(v)` means the pattern matched (possibly zero-width) and `None` means it did not. The engine translates that `Option` directly into `Equal`/`NotEqual`. Comparing matched text to the pattern literal via `apply_operator` would fail for any regex with metacharacters (e.g., matched `"123"` vs pattern `"[0-9]+"`). **Non-equality operators on pattern-bearing types are rejected as `TypeReadError::UnsupportedType`** — an earlier revision fell through to `apply_operator` and silently produced lexicographic ordering comparisons against the pattern source text. If you add a new pattern-bearing `TypeKind` variant, add its arm to both `read_pattern_match` and `bytes_consumed_with_pattern`; the engine's special-case match is keyed on the `Regex | Search` pair so you must add new variants there too.

### 2.5 Zero-Width Regex Matches vs Misses

`read_regex` returns `Ok(Some(Value::String("")))` for a legitimate zero-width match (`^`, `a*`, `.{0}`) and `Ok(None)` for a genuine miss. (The Rust `regex` crate does not support look-around assertions such as lookaheads or lookbehinds -- they are excluded to preserve linear-time matching guarantees.) An earlier revision collapsed both cases to `Value::String(String::new())` and distinguished them by `is_empty()`, which broke every pattern that legitimately matches zero bytes. The structured `Option` is the invariant — do not re-flatten it. `read_typed_value_with_pattern` does collapse `None` to `Value::String(String::new())` for back-compat with its single-`Value` return shape, but the engine does not go through that function for pattern types; it calls `read_pattern_match` directly.

### 2.6 Search Anchor Advance Is Match-End, Not Window-End

`search_bytes_consumed` returns `match_idx + pattern.len()` — the byte just past the matched pattern — not `range` (the window size). This matches GNU `file` semantics: `src/softmagic.c` `FILE_SEARCH` in `moffset()` computes `o = ms->search.offset + vlen - offset` where `ms->search.offset` has already been advanced by `idx` (the match index inside the window) in `magiccheck`, and `vlen = m->vallen` (the pattern length). An earlier revision returned the full window size, which silently corrupted relative-offset children of every successful `search` rule (e.g., `search/256 "MAGIC"` at index 4 advanced the anchor by 256 instead of by 9). The fix threaded the pattern through `bytes_consumed_with_pattern` for `TypeKind::Search` so the scan can be re-run at anchor-advance time. Search does not currently support a `/s`-style start-offset flag; if one is added, match-end can become match-start.

### 2.7 Regex `/l` Is Encoded by `RegexCount::Lines`, Not a Flag

The `/l` suffix is **not** a `RegexFlags` field — it lives on the `RegexCount::Lines` variant of `TypeKind::Regex::count`, alongside `RegexCount::Default` and `RegexCount::Bytes(n)`. This collapses the previously-possible "line-mode without count" degenerate state into the type-enforced `RegexCount::Lines(None)` (the `regex/l` shorthand) and makes "byte-count scan" and "line-count scan" mutually exclusive at the type level. `compute_window` in `src/evaluator/types/regex.rs` dispatches on the `RegexCount` variant: `Lines(Some(n))` walks line terminators (LF, CRLF, or bare CR — each counting as one terminator) until the Nth is found, and `Lines(None)` / `Default` both return the full byte-capped window. The `/l` encoding does **not** toggle regex multi-line matching — libmagic always compiles with `REG_NEWLINE` (unconditional at `src/softmagic.c::alloc_regex` line 2123), so `^` and `$` match at line boundaries for every regex rule regardless of which `RegexCount` variant is chosen. An earlier revision of this crate wrapped line-based patterns in `^(?:...)` and only set `multi_line(true)` when `/l` was set; that was wrong on both counts and has been removed. `build_regex` now unconditionally sets `multi_line(true)` and `dot_matches_new_line(false)` for all patterns.

### 2.8 Regex Scan Window Is Always Capped at 8192 Bytes

Every regex rule is subject to the `REGEX_MAX_BYTES` (8192) hard cap, matching GNU `file`'s `FILE_REGEX_MAX` (`src/file.h:522`). This applies to every `RegexCount` variant:

- `RegexCount::Default` (plain `regex`) → scan the full 8192-byte window (or less if the buffer is shorter).
- `RegexCount::Bytes(n)` → `min(n, 8192, remaining)` — explicit counts larger than 8192 are clamped.
- `RegexCount::Lines(Some(n))` → walks line terminators to the Nth, but the walk is bounded by the byte-capped slice, so a buffer with fewer than N terminators in the first 8192 bytes stops at the 8192-byte boundary regardless of `n`.
- `RegexCount::Lines(None)` → same as `Default`: full 8192-byte capped window.

The cap is a DoS mitigation: without it, a malicious regex against a multi-GB buffer combined with `EvaluationConfig::default()` (no timeout — see S13.1) can hang the evaluator. `REGEX_MAX_BYTES` itself lives in `src/evaluator/types/regex.rs` (not in `parser::ast`) because it is runtime evaluation policy rather than AST shape — putting it in `ast.rs` would couple the build-script compilation unit to an evaluator-only concern per S1.1. Do not add a path that bypasses the cap, even for "trusted" rules — the cap is also what makes the regex evaluator's worst-case runtime bounded.

### 2.9 Regex `/s` Flag Affects Anchor Advance Only, Not Match Result

`RegexFlags::start_offset` (the `/s` suffix) controls *only* `regex_bytes_consumed`: when set, the anchor advance is `m.start()` (match-start) instead of `m.end()` (match-end). The match *result* (whether a pattern matches, and what matched text is returned) is unchanged. This matches libmagic's `REGEX_OFFSET_START` flag, which zeros the `rm_len` contribution in `moffset()` but does not alter the regex scan itself. Tests for `/s` must exercise `regex_bytes_consumed` directly or check the resolved offset of a `Relative(N)` child rule; checking `read_regex` alone won't detect a broken `/s` implementation.

### 2.10 `RegexCount::Lines(None)` Is Behaviorally Equivalent to `RegexCount::Default`

Both `RegexCount::Default` (plain `regex`) and `RegexCount::Lines(None)` (the `regex/l` shorthand with no explicit count) produce the full 8192-byte capped window. `compute_window` handles them with a shared match arm, and `calculate_default_strength` gives them the same strength score (20, no "constrained scan" bonus). The two variants are kept distinct at the AST level because the magic-file surface syntax distinguishes them — `regex` and `regex/l` parse to different `RegexCount` variants and round-trip through codegen as different Rust expressions — but no runtime path treats them differently. If you write a test that passes `Lines(None)` expecting different behavior from `Default`, the test is wrong, not the implementation. See `test_read_regex_lines_none_is_equivalent_to_default_on_buffer_with_terminators` in `src/evaluator/types/regex.rs` for the regression guard that pins this equivalence.

### 2.11 `MetaType` Exhaustive Matches

Adding a variant to the `MetaType` enum (nested inside `TypeKind::Meta`) requires updates in: `ast.rs` (variant definition + 3 test fixtures that iterate MetaType variants: `test_meta_type_variants_debug_clone_eq`, `test_meta_type_serde_roundtrip`, `test_type_kind_meta_bit_width_is_none`), `parser/types.rs` (`parse_type_keyword` tag + `type_keyword_to_kind` match arm + `test_roundtrip_all_keywords` array), `parser/codegen.rs` (`serialize_type_kind` -- the inner `TypeKind::Meta(meta)` arm), and `tests/property_tests.rs` (`arb_type_kind` `prop_oneof` branch). The evaluator does NOT need updates: `TypeKind::Meta(_)` is handled uniformly as a no-op via the wildcard arm in `engine::evaluate_single_rule_with_anchor`, `strength.rs::calculate_default_strength`, and `types/mod.rs::bytes_consumed_with_pattern`.

## 3. Parser Architecture

### 3.1 Type Keyword Parsing Split

Type keyword parsing lives in `src/parser/types.rs` (`parse_type_keyword` + `type_keyword_to_kind`); the grammar module in `src/parser/grammar/mod.rs` orchestrates and handles type-specific suffixes (e.g., pstring `/B`, `/H`, `/L`).

- **Gotcha:** `parse_type_keyword` only consumes the base keyword (e.g., `"pstring"`), leaving suffixes for the grammar layer.
- **Gotcha:** `parse_type_and_operator` consumes trailing whitespace after parsing type+suffix -- test expectations for remaining input must account for this.

### 3.2 `parse_number` Return Type

`parse_number` returns `i64` and is shared by offset + value parsing. Never widen its return type. Use the separate `parse_unsigned_number` (`u64`) path only in `parse_numeric_value`.

### 3.3 nom `tuple` Combinator

The nom `tuple` combinator is deprecated. Use bare tuple syntax `(a, b, c)` directly as `Parser` is implemented for tuples.

### 3.4 `type_keyword_to_kind` Line Length

`type_keyword_to_kind` has `#[allow(clippy::too_many_lines)]` because it exceeds 100 lines with all date keywords.

### 3.5 `parse_number` Does Not Handle `+` Prefix

`parse_number` handles `-` signs but not `+`. When parsing syntax like `+4` (e.g., indirect offset adjustments), consume the `+` character manually before calling `parse_number`.

### 3.6 `parse_value` Requires Quoted Strings (But `parse_magic_rule` Has a Bare-Word Fallback)

`parse_value()` itself does not accept bare unquoted strings -- `parse_value("xyz")` still returns `Err` (see `test_parse_value_invalid_input` in `grammar/tests/mod.rs`, which pins this behavior). However, `parse_magic_rule` adds a bare-word fallback (`parse_bare_string_value`) when the rule's type is string-family (`String`, `PString`, `Regex`, `Search`), so `0 string TEST` and `>0 search/12 ABC` parse successfully without quotes -- matching libmagic magic(5) surface syntax and allowing real-world fixtures like `third_party/tests/searchbug.magic` to load. For non-string-family types (byte/short/long/etc.), bare words still fail; integration tests exercising `parse_value` directly (as opposed to going through `parse_magic_rule`) must still quote string literals.

### 3.7 Indirect Offset Pointer Specifiers Follow GNU `file` Semantics

Lowercase pointer specifiers (`.s`, `.l`, `.q`) map to **little-endian**, not native endian. Uppercase (`.S`, `.L`, `.Q`) map to big-endian. All numeric pointer types are **signed by default** (per S6.3). The adjustment is parsed **after** the closing paren: `(base.type)+adj`, not `(base.type+adj)`.

### 3.8 Relative Offsets: Global Anchor, No Save/Restore

`OffsetSpec::Relative(N)` resolves against `EvaluationContext::last_match_end()`, which is updated after every successful match in `evaluate_rules` and is **never saved/restored across child recursion**. This is intentional and matches GNU `file`: a sibling rule sees the anchor wherever the deepest descendant of the previous sibling left it. The anchor is global/shared rather than stack-scoped, but its numeric value is not guaranteed to be non-decreasing -- a successful `Relative(-N)` rule (or any later rule that matches at a lower absolute position) can move it earlier. Do not wrap recursion in a save/restore pair "for safety" -- it would silently break sibling-after-nested chains. The recursion-depth pattern in the same loop *is* save/restore, and the asymmetry is correct.

The load-bearing invariant is that the anchor is updated *before recursing into children* (so children and their followers see the new anchor). The current code also happens to set the anchor before `matches.push(...)`, but the push-ordering relative to `set_last_match_end` is incidental for anchor correctness -- only the ordering before the `evaluate_rules` recursion call matters. (Future code that reads the anchor while iterating `matches` would make this ordering load-bearing, so do not "optimize" the order without checking call sites first.) `bytes_consumed()` (in `evaluator/types/mod.rs`) is the source of truth for advance distance; for variable-width types it re-derives consumption from the buffer rather than trusting `Value::String.len()` (which can drift from the original byte length via `from_utf8_lossy`). Pascal-string consumption is also clamped against the remaining buffer to prevent attacker-controlled length prefixes from poisoning the anchor to `usize::MAX`.

### 3.9 `parse_text_magic_file` is Fail-Fast, Not Skip-on-Error

`build_rule_hierarchy` propagates any `parse_magic_rule_line` error immediately, so a single unparseable rule (e.g., a child using unsupported `&+N` relative-offset syntax or an unquoted `$VAR` string value -- see S3.6) causes the **entire file load** to fail with `ParseError::InvalidSyntax`. There is no skip-and-continue mode. When writing corpus tests against third_party `.magic` files that mix supported and unsupported syntax, bypass the parser and build the equivalent `MagicRule` tree programmatically via the AST; the runtime evaluator can still be exercised end-to-end against the real testfile buffer. See `tests/evaluator_tests.rs::test_regex_eol_corpus` for a worked example.

## 4. Module Visibility & Re-exports

### 4.1 Private Engine Module

`evaluator::engine` is private. Integration tests must import `libmagic_rs::evaluator::evaluate_rules` (re-exported), not `evaluator::engine::evaluate_rules`.

### 4.2 `MatchResult` Name Collision

`evaluator::MatchResult` was renamed to `evaluator::RuleMatch` (issue #60). `output::MatchResult` is the public-facing type. Do not confuse the two.

### 4.3 Glob Re-exports

Prefer `pub mod` over `pub use module::*` for submodules -- glob re-exports expand public API surface unintentionally.

### 4.4 Parallel `EvaluationResult` / `EvaluationMetadata` Types

There are **two distinct** `EvaluationResult` and `EvaluationMetadata` types with overlapping names but different fields:

- `src/lib.rs`: the top-level library API types. `EvaluationResult` carries `description`, `mime_type`, `confidence`, `matches: Vec<evaluator::RuleMatch>`, `metadata`. `EvaluationMetadata` carries `file_size: u64`, `evaluation_time_ms: f64`, `rules_evaluated: usize`, `magic_file: Option<PathBuf>`, `timed_out: bool`.
- `src/output/mod.rs`: the output-formatting types. `EvaluationResult` carries `filename: PathBuf`, `matches: Vec<MatchResult>`, `metadata`, `error: Option<String>`. `EvaluationMetadata` carries `file_size: u64`, `evaluation_time_ms: f64`, `rules_evaluated: u32`, `rules_matched: u32`.

The two pairs are **not** interchangeable -- notice that `rules_evaluated` is `usize` in one and `u32` in the other, and `timed_out`/`magic_file` exist only on the lib.rs variant while `rules_matched`/`filename`/`error` exist only on the output variant. A one-way conversion exists: `output::EvaluationResult::from_library_result(...)` converts from the library type to the output type; there is no reverse conversion. When importing, qualify the path (`libmagic_rs::EvaluationResult` vs `libmagic_rs::output::EvaluationResult`) to make intent explicit. Do not "unify" them without coordinated API changes across the CLI, integration tests, and any downstream consumers.

## 5. Numeric Types & Arithmetic

### 5.1 `usize::from(u32)` Does Not Compile

There is no `From<u32> for usize` (fails on 32-bit targets). Use `as usize` for widening conversions from `u32`.

### 5.2 Checked Arithmetic for Offsets

Buffer offset calculations must use `checked_add` to prevent overflow panics from malicious/malformed input. Always test with `usize::MAX` -- otherwise regressions go undetected.

### 5.3 Float Epsilon Equality

`inf - inf = NaN`, so `(a - b).abs() <= epsilon` fails for infinities. Handle `is_infinite()` before the epsilon check. Use `#[allow(clippy::float_cmp)]` on the exact infinity comparison.

- **Note:** `apply_equal`/`apply_not_equal` use epsilon-aware comparison for `Value::Float`; ordering operators (`<`, `>`, `<=`, `>=`) use direct `partial_cmp` -- these are deliberately different semantics.

## 6. String & PString Types

### 6.1 Multi-Byte PString Length Prefixes

Uppercase pstring suffix letters indicate **big-endian** byte order, lowercase indicate **little-endian**: `/H` (2-byte BE), `/h` (2-byte LE), `/L` (4-byte BE), `/l` (4-byte LE). The `/J` flag means the stored length includes the length field itself (JPEG convention). Flags are combinable (e.g., `pstring/HJ`). Test data must use the correct byte order for the variant (e.g., `\x00\x05` for `TwoByteBE` length 5, `\x05\x00` for `TwoByteLE` length 5).

### 6.2 `medate`/`meldate` Not Supported

Middle-endian date keywords are NOT supported. They were removed until real middle-endian decoding is implemented end-to-end.

### 6.3 Signed-by-Default Types

libmagic types are signed by default (`byte`, `short`, `long`, `quad`). Unsigned variants use `u` prefix (`ubyte`, `ushort`, `ulong`, `uquad`, etc.).

### 6.4 `TypeKind::String { max_length: None }` Against Buffers Without NUL

`read_string` with `max_length: None` reads until the first NUL or end of buffer. On NUL-free buffers (raw ASCII text, JSON, log lines, etc.) it reads the *entire remaining buffer*, and equality comparison against a short target value then fails. Programmatic rules built against such buffers must set `max_length: Some(target_len)` explicitly. Text magic rules (`string "MZ"`) typically work anyway because real executable headers contain NULs within the first few bytes.

## 7. Testing

### 7.1 Doctest Import Paths

Doctests compile as external crates. Use `libmagic_rs::` imports, never `crate::`.

### 7.2 Heterogeneous Byte-Array Test Tables

Table-driven tests with byte-array buffers of different sizes need `&[(&[u8], ...)]` slice type annotation. Bare arrays can't hold heterogeneous-length byte literals in a `[T; N]`.

### 7.3 Doc Examples and Pattern Matching

`cargo test --doc` verifies doc examples. Ensure example strings don't accidentally match multiple patterns.

### 7.4 `clap` Args in Tests

Adding fields to the clap `Args` struct requires updating ALL manual `Args { ... }` constructions in unit tests (search for `Args {` in `src/main.rs`). `process_file` and `run_analysis` are called from unit tests with mocked stdin -- signature changes require updating all test call sites.

### 7.5 Reserved-for-Future Enum Variants

Reserved-for-future enum variants (e.g., `TypeReadError::UnsupportedType`) need explicit doc comments explaining intent -- otherwise every reviewer flags them as dead code.

## 8. Clippy & Formatting

### 8.1 Enum Variant Naming

Enum variants with a shared suffix (e.g., `OneByte`, `TwoByte`, `FourByte`) trigger `clippy::enum_variant_names`. Add `#[allow(clippy::enum_variant_names)]` when renaming would hurt readability.

### 8.2 `unsafe_code = "forbid"` is Absolute

`unsafe_code = "forbid"` in `Cargo.toml` workspace lints cannot be overridden with `#[allow(unsafe_code)]`. Use vetted crates (e.g., `chrono` for timezone) instead of libc FFI or subprocess calls.

### 8.3 Pre-commit Hook Reformats

Pre-commit hook runs `cargo fmt`. First commit attempt may fail if fmt modifies files -- just re-stage and retry.

## 9. Error Handling Patterns

### 9.1 Error Return Path Cleanup

On error return paths, use `let _ = cleanup()` not `cleanup()?` -- the `?` masks the original error with cleanup failures. Bind original errors with `e @` pattern instead of reconstructing after state changes (avoids data loss).

### 9.2 Use Correct Error Constructors

- Use `ParseError::IoError` for I/O errors in parser code, not `ParseError::invalid_syntax`
- Use `LibmagicError::ConfigError` for config validation, not `ParseError::invalid_syntax`

## 10. Documentation

### 10.1 Extracting Public Functions

When extracting public functions to new modules, verify `# Examples`, `# Arguments`, `# Returns` doc sections are preserved -- they silently disappear.

### 10.2 `# Returns` and `# Security` Accuracy

When refactoring docs, verify `# Returns` and `# Security` sections match actual behavior. For example, `from_utf8_lossy` never errors but docs may claim it does.

### 10.3 Public Enum Variant Docs

All public enum variants need `# Examples` rustdoc sections. Clippy enforces this.

## 11. Date/Timestamp Types

### 11.1 Date Value Representation

`read_date`/`read_qdate` return `Value::Uint(raw_secs)` for correct numeric comparisons. Use `format_timestamp_value(secs, utc)` at output time for GNU `file`-compatible display. There is no special date coercion in `coerce_value_to_type`.

### 11.2 Timezone Handling

`chrono` crate (no default features, `std` + `clock`) is a vetted dependency for timezone offset computation in `evaluator/types/date.rs`. Do not use libc FFI or subprocess calls (see S8.2).

## 12. Git & Release

### 12.1 Repository Identity

Repository is **EvilBit-Labs/libmagic-rs**. NEVER guess repo/org names -- always run `git remote -v` to verify.

### 12.2 Tag Signing

All tags and commits MUST be signed -- use `git tag -s` and `git commit -s -S`. If signing fails, STOP and troubleshoot. Never push unsigned tags/commits. Never delete or force-push tags without explicit user permission.

### 12.3 Auto-Generated Release Workflow

`.github/workflows/release.yml` is auto-generated by cargo-dist -- never modify manually.

## 13. Evaluation Configuration

### 13.1 `EvaluationConfig::default()` Has No Timeout

`EvaluationConfig::default()` sets `timeout_ms: None`, meaning library consumers who construct a config via `Default::default()` get **unbounded** evaluation time. On crafted or adversarial input, rules with expensive operations (deep nesting, large buffers, pathological backtracking in future regex/search types) can run indefinitely and expose callers to a denial-of-service vector.

- **Rule:** Library consumers embedding libmagic-rs in services or untrusted-input pipelines should **not** use `EvaluationConfig::default()`. Use `EvaluationConfig::performance()` (which sets `timeout_ms: Some(1000)`) as the safe preset, or construct a config explicitly with a non-`None` timeout sized for your workload.
- **Validation:** `timeout_ms` is clamped to `MAX_SAFE_TIMEOUT_MS` (5 minutes) by config validation and must be `> 0` if specified -- see the validation logic in `src/config.rs`.
- **Note:** `Default` cannot be changed to set a timeout without breaking API expectations of callers who deliberately want no timeout (e.g., CLI one-shot invocations). The gotcha is that the unsafe default is the ergonomic choice; document the tradeoff prominently in any new consumer-facing docs.

## 14. Output Formatting

### 14.1 `\b` (Backspace) Prefix in Rule Messages Suppresses Leading Space

`MagicDatabase::build_result` concatenates rule messages with a space separator, **except** when a message starts with `\u{0008}` (backspace / `\b`), in which case the backspace is stripped and no leading space is inserted. This mirrors GNU `file`'s description formatting (used by rules like `>&1 regex/1l ... \b, version %s` to produce `Ansible Vault text, version 1.1` instead of `Ansible Vault text , version 1.1`). Tests that manually simulate the concatenation path (e.g., corpus tests that bypass `load_from_file` -- see S3.9) must honor this convention or their assertions will diverge from the real evaluator output.

### 14.2 `%s` (and Other printf-Style Format Specifiers) Are Not Substituted

Magic rule messages like `\b, version %s` are passed through verbatim to the final concatenated description -- the evaluator does not implement printf-style format substitution. Captured values from regex/search/pattern matches live on `RuleMatch.value`, not embedded in `RuleMatch.message`. Tests or output checks that expect substituted text (e.g., "version 1.1") must either hardcode the expected token in the rule's message or assert against `RuleMatch.value` directly.
