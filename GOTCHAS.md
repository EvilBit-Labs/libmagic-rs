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

Adding a variant to `TypeKind` requires updating exhaustive matches in 10+ files: `ast`, `grammar`, `types`, `codegen`, `strength`, `property_tests`, `evaluator/types/mod.rs` (`read_typed_value`, `coerce_value_to_type`), `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), and `grammar/tests.rs` (stale assertions). Note: `coerce_value_to_type` and output matches use catch-all `_ =>` so they compile without changes but may need semantic updates.

### 2.2 `Operator` Exhaustive Matches

Adding a variant to `Operator` requires updating: `ast`, `grammar`, `codegen`, `strength`, `property_tests`, `evaluator/operators/`.

### 2.3 `Value` Exhaustive Matches

Adding a variant to `Value` requires updating: `ast`, `codegen`, `strength`, `property_tests`, `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), `evaluator/operators/comparison.rs` (`compare_values`). Bitwise/equality operators use catch-all `_ =>` so they are safe.

- **Note:** `Value` no longer derives `Eq` (removed when `Value::Float(f64)` was added) -- no production code depends on `Value: Eq`.

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

### 3.6 `parse_value` Requires Quoted Strings

`parse_value()` does not accept bare unquoted strings. String values in magic file rules must be quoted (e.g., `string "MZ"` not `string MZ`). Integration tests writing magic files must use `r#"0 string "MZ" description"#` format.

### 3.7 Indirect Offset Pointer Specifiers Follow GNU `file` Semantics

Lowercase pointer specifiers (`.s`, `.l`, `.q`) map to **little-endian**, not native endian. Uppercase (`.S`, `.L`, `.Q`) map to big-endian. All numeric pointer types are **signed by default** (per S6.3). The adjustment is parsed **after** the closing paren: `(base.type)+adj`, not `(base.type+adj)`.

### 3.8 Relative Offsets: Anchor is Global-Monotonic, No Save/Restore

`OffsetSpec::Relative(N)` resolves against `EvaluationContext::last_match_end()`, which advances after every successful match in `evaluate_rules` and is **never saved/restored across child recursion**. This is intentional and matches GNU `file`: a sibling rule sees the anchor wherever the deepest descendant of the previous sibling left it. Do not wrap recursion in a save/restore pair "for safety" -- it would silently break sibling-after-nested chains. The recursion-depth pattern in the same loop *is* save/restore, and the asymmetry is correct.

The load-bearing invariant is that the anchor is updated *before recursing into children* (so children and their followers see the new anchor). The current code also happens to set the anchor before `matches.push(...)`, but the push ordering is incidental -- only the recursion ordering is required for correctness. `bytes_consumed()` (in `evaluator/types/mod.rs`) is the source of truth for advance distance; for variable-width types it re-derives consumption from the buffer rather than trusting `Value::String.len()` (which can drift from the original byte length via `from_utf8_lossy`). Pascal-string consumption is also clamped against the remaining buffer to prevent attacker-controlled length prefixes from poisoning the anchor to `usize::MAX`.

## 4. Module Visibility & Re-exports

### 4.1 Private Engine Module

`evaluator::engine` is private. Integration tests must import `libmagic_rs::evaluator::evaluate_rules` (re-exported), not `evaluator::engine::evaluate_rules`.

### 4.2 `MatchResult` Name Collision

`evaluator::MatchResult` was renamed to `evaluator::RuleMatch` (issue #60). `output::MatchResult` is the public-facing type. Do not confuse the two.

### 4.3 Glob Re-exports

Prefer `pub mod` over `pub use module::*` for submodules -- glob re-exports expand public API surface unintentionally.

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
