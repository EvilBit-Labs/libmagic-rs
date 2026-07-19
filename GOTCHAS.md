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

Adding a variant to `TypeKind` requires updating exhaustive matches in 10+ files: `ast`, `grammar`, `types`, `codegen` (`serialize_type_kind` -- easy to forget; build.rs is a separate compilation unit so the error surfaces there first), `strength`, `property_tests`, `evaluator/types/mod.rs` (`read_typed_value`, `coerce_value_to_type`, **`bytes_consumed`** -- variable-width variants must be matched explicitly or relative-offset anchors will silently corrupt), `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), and `grammar/tests.rs` (stale assertions). For string-family variants (anything that semantically holds a byte sequence rather than a fixed-width number), also add the variant to `is_string_family_type` in `parser/grammar/mod.rs::parse_magic_rule` so bareword (unquoted) value parsing falls back through the string parser instead of failing strict `parse_value` -- `String16` was caught missing from this helper during the PR #233 review (see `docs/solutions/design-patterns/multi-agent-review-surfaces-cross-cutting-consistency-gaps-2026-04-25.md` Gap 5 bonus fix). Note: `coerce_value_to_type`, output matches, and `bytes_consumed` use catch-all `_ =>` so they compile without changes but may need semantic updates -- `bytes_consumed` will fire a `debug_assert` in test/dev builds for unhandled variable-width variants.

**`TypeReadError::UnsupportedType` propagation has exactly one narrow, allowlisted exception.** The top-level dispatch match in `evaluator/engine/mod.rs::evaluate_rules` carries the comment `TypeReadError::UnsupportedType is intentionally NOT caught here (except the narrow exception in the arm immediately below)`. That exception, and the equivalent arm in the other two engine catch sites (`evaluate_children_or_warn`, and the inline child-recursion match under a successful parent), is gated on `types::is_missing_pattern_operand(type_name)` (the `type_name` string is exactly one of `"regex without string pattern"`, `"search without string/bytes pattern"`, or `"string with flags requires string/bytes pattern"` -- an exhaustive allowlist, not a substring match) or `types::is_regex_compile_failure(type_name)` (`type_name.starts_with("regex compile error:")`, which includes the `REGEX_COMPILE_SIZE_LIMIT` CWE-1333 DoS guard). When either predicate matches, the rule is skipped (non-match) instead of aborting the whole file's evaluation, logged via the shared `log_pattern_operand_skip` helper: `debug!` for the ordinary missing-pattern case, `warn!` for a regex compile failure (so a malicious or pathological magic file's rejection stays visible in logs even though evaluation of the rest of the file continues). Any OTHER `UnsupportedType` cause -- an unwired `TypeKind` variant, a non-Equal/NotEqual operator applied to a pattern-bearing type (S2.4), a `Meta` variant read as a value -- still falls through and propagates fatally at all three sites. Do not widen the predicate to match on the variant alone; that would silently swallow genuine capability gaps this contract exists to surface.

**Meta variants (`TypeKind::Meta(MetaType::...)`)** are structurally different. They do not read bytes, so `read_typed_value`, `coerce_value_to_type`, and `bytes_consumed` treat them as no-ops (catch-all or zero-width arms). The live dispatch for control-flow directives happens in `evaluator/engine/mod.rs::evaluate_rules` -- the *loop body*, not in `evaluate_single_rule_with_anchor`. `MetaType::Use`, `MetaType::Default`, `MetaType::Clear`, and `MetaType::Indirect` are all handled inline there so subroutine / control-flow effects can be spliced into the caller's match vector; `MetaType::Name` is hoisted out of the rule list at parse time by `parser::name_table::extract_name_table` and should normally never reach the evaluator at all. The evaluator's leaked-`Name` arm uses `debug!` (not `debug_assert!`) because `prop_arbitrary_rule_evaluation_never_panics` synthesizes arbitrary `TypeKind` values and a panic there would break the never-panics invariant. **This applies to every inline-dispatched meta variant that has a "not configured" fallback** (currently `Use` and `Indirect` for the no-`RuleEnvironment` case): property tests run with `rule_env = None`, so any `debug_assert!` on that path will fire. When adding a new `MetaType` variant: decide whether it needs inline loop-level dispatch (add to `evaluate_rules`) or is a silent no-op (add to the `_` arm of the `Meta(_)` match in `evaluate_single_rule_with_anchor`). See `docs/solutions/integration-issues/meta-type-subroutine-dispatch-architecture.md` for the full pattern.

### 2.2 `Operator` Exhaustive Matches

Adding a variant to `Operator` requires updating: `ast`, `grammar`, `codegen`, `strength`, `property_tests`, `evaluator/operators/`.

### 2.3 `Value` Exhaustive Matches

Adding a variant to `Value` requires updating: `ast`, `codegen`, `strength`, `property_tests`, `output/mod.rs` (2 length matches), `output/json.rs` (`format_value_as_hex`), `evaluator/operators/comparison.rs` (`compare_values`). Bitwise/equality operators use catch-all `_ =>` so they are safe.

- **Note:** `Value` no longer derives `Eq` (removed when `Value::Float(f64)` was added) -- no production code depends on `Value: Eq`.
- **Cross-type `String`/`Bytes` equality is policy, not accident.** `apply_equal` deliberately compares `Value::String(s)` against `Value::Bytes(b)` by underlying byte sequence (`s.as_bytes() == b.as_slice()`) -- this is libmagic-compatible behavior. The parser produces `Value::Bytes` for backslash-escape patterns like `\177ELF` (via `parse_mixed_hex_ascii`), while `read_string_exact` returns `Value::String`; both must compare equal when their bytes match or rules with hex/octal escapes silently fail to match. Any new `Value` variant that can hold the same byte sequence as `Value::String` should extend this cross-equality, not lock into strict-variant comparison. See `docs/solutions/logic-errors/magic-string-rule-matching-3-bug-fix-2026-04-25.md` for the worked example.
- **Cross-type ordering must follow the same policy or trichotomy breaks.** `compare_values` in `evaluator/operators/comparison.rs` mirrors the cross-type `String`/`Bytes` equality with `Some(s.as_bytes().cmp(b.as_slice()))` (and the symmetric `Bytes`/`String` arm). Without this, `apply_equal(Bytes, String)` returns `true` for byte-equal pairs but `apply_less_than` and `apply_greater_than` both return `false` — neither `<`, `==`, nor `>` is true, breaking the trichotomy invariant. magic(5) rules using `>` or `<` against a `Value::Bytes` literal silently never fire even when equality with the same literal succeeds. When adding a new `Value` variant that participates in equality, audit `compare_values` for the matching arms; an asymmetric policy is worse than no policy. See `docs/solutions/design-patterns/multi-agent-review-surfaces-cross-cutting-consistency-gaps-2026-04-25.md` Gap 1 for the worked example. Regression test: `cross_type_string_bytes_ordering_is_byte_sequence` in the comparison module pins the trichotomy.

### 2.4 Pattern-Bearing Types Bypass `apply_operator` in the Engine

`TypeKind::Regex` and `TypeKind::Search` are evaluated by **logical match** in `evaluate_single_rule_with_anchor` (`src/evaluator/engine/mod.rs`), not by string equality against `rule.value`. The engine calls `types::read_pattern_match`, which returns `Result<Option<Value>, _>`: `Some(v)` means the pattern matched (possibly zero-width) and `None` means it did not. The engine translates that `Option` directly into `Equal`/`NotEqual`. Comparing matched text to the pattern literal via `apply_operator` would fail for any regex with metacharacters (e.g., matched `"123"` vs pattern `"[0-9]+"`). **Non-equality operators on pattern-bearing types are rejected as `TypeReadError::UnsupportedType`** — an earlier revision fell through to `apply_operator` and silently produced lexicographic ordering comparisons against the pattern source text. If you add a new pattern-bearing `TypeKind` variant, add its arm to both `read_pattern_match` and `bytes_consumed_with_pattern`; the engine's special-case match is keyed on the `Regex | Search` pair so you must add new variants there too.

**`Regex` and `Search` both accept `Some(Value::String(_))` and `Some(Value::Bytes(_))` as the pattern operand** — the two types are symmetric, not asymmetric. (An earlier revision had `Regex` accept only `Value::String`, rejecting `Value::Bytes` with `UnsupportedType { "regex without string pattern" }` while `Search` already accepted both; that gap is what let escape-heavy `regex` patterns like `\^[\040\t]{0,50}\\.asciiz` -- parsed as `Value::Bytes` by `parse_value`'s hex/mixed-ascii branch -- fatally abort evaluation of the entire system magic DB. See the fix-system-magic-regex-graceful plan.) Three call sites in `src/evaluator/types/mod.rs` were made symmetric: the `Regex` arms of `read_typed_value_with_pattern` and `read_pattern_match` both now match `Some(Value::Bytes(b)) => Cow::Owned(decode_regex_bytes_pattern(b))` alongside the `Value::String` fast path (`Cow::Borrowed`), and `bytes_consumed_with_pattern`'s `Regex` arm decodes the same way before re-running `regex_bytes_consumed` so the anchor-advance side agrees with the read side. `decode_regex_bytes_pattern` decodes via `String::from_utf8_lossy` and `warn!`s only when `str::from_utf8` on the same bytes actually fails (i.e., a real substitution occurred, not just that the input happened to be `Bytes`) — a lossy substitution silently desyncs the compiled regex (which now contains U+FFFD) from the target buffer (still matched by raw bytes), so the warning is the visibility guard for that divergence. In practice this backstop is now a floor-tier safety net rather than the common path: the parser's getstr resolver (see the new sub-point below) captures unquoted `regex` patterns as `Value::String` directly, so a `Value::Bytes` regex pattern should be rare going forward.

### 2.5 Zero-Width Regex Matches vs Misses

`read_regex` returns `Ok(Some(Value::String("")))` for a legitimate zero-width match (`^`, `a*`, `.{0}`) and `Ok(None)` for a genuine miss. (The Rust `regex` crate does not support look-around assertions such as lookaheads or lookbehinds -- they are excluded to preserve linear-time matching guarantees.) An earlier revision collapsed both cases to `Value::String(String::new())` and distinguished them by `is_empty()`, which broke every pattern that legitimately matches zero bytes. The structured `Option` is the invariant — do not re-flatten it. `read_typed_value_with_pattern` does collapse `None` to `Value::String(String::new())` for back-compat with its single-`Value` return shape, but the engine does not go through that function for pattern types; it calls `read_pattern_match` directly.

### 2.6 Search Anchor Advance Is Match-End, Not Window-End

`search_bytes_consumed` returns `match_idx + matched_len` by default — the byte just past the matched pattern — not `range` (the window size). This matches GNU `file` semantics: `src/softmagic.c` `FILE_SEARCH` in `moffset()` computes `o = ms->search.offset + vlen - offset` where `ms->search.offset` has already been advanced by `idx` (the match index inside the window) in `magiccheck`, and `vlen = m->vallen` (the pattern length). An earlier revision returned the full window size, which silently corrupted relative-offset children of every successful `search` rule (e.g., `search/256 "MAGIC"` at index 4 advanced the anchor by 256 instead of by 9). The fix threaded the pattern through `bytes_consumed_with_pattern` for `TypeKind::Search` so the scan can be re-run at anchor-advance time.

**`/s` start-anchor branch (issue #235).** When `SearchFlags.start_anchor` is set, `search_bytes_consumed` returns `match_idx` instead of `match_idx + matched_len` — relative-offset children resolve to the byte at match-START rather than match-END. This mirrors libmagic's `REGEX_OFFSET_START` flag (`src/file.h::STRING_SEARCHEND`) which zeros the matched-length contribution in `moffset()`. The flag affects ONLY anchor advance; the match decision itself is unchanged. Required for TGA-footer detection (`images:114`), sfnt name table parsing (`fonts:260`), and any rule whose children walk backwards from the matched signature. Tests at `src/evaluator/types/search.rs::test_search_with_start_anchor_returns_match_start_index` and `tests/search_flag_conformance_tests.rs::search_s_anchors_relative_child_at_match_start` pin the contract.

**`matched_len` policy under `/T`.** When `SearchFlags.trim` is set, the pattern is trimmed of leading/trailing ASCII whitespace at the dispatcher boundary and the comparator sees only the trimmed slice. `matched_len` reflects the trimmed length, not `pattern.len()` — using the raw length would advance the anchor past bytes the comparator never inspected. The flagged-string path (`read_pattern_match` in `evaluator/types/mod.rs`) follows the same trim-at-boundary contract; search.rs inlines its own `trim_ascii_whitespace` because the module-level helper is private.

**Anchor advance is clamped against remaining buffer.** `min(match_idx + matched_len, buffer.len() - offset)` protects against any `match_idx + matched_len` overflow on adversarial input, following the pattern at `docs/solutions/security-issues/pstring-anchor-poisoning.md`.

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

Adding a variant to the `MetaType` enum (nested inside `TypeKind::Meta`) requires updates in: `ast.rs` (variant definition + 3 test fixtures that iterate MetaType variants: `test_meta_type_variants_debug_clone_eq`, `test_meta_type_serde_roundtrip`, `test_type_kind_meta_bit_width_is_none`), `parser/types.rs` (`parse_type_keyword` tag + `type_keyword_to_kind` match arm + `test_roundtrip_all_keywords` array), `parser/codegen.rs` (`serialize_type_kind` -- the inner `TypeKind::Meta(meta)` arm), and `tests/property_tests.rs` (`arb_type_kind` `prop_oneof` branch).

**Evaluator decision rule.** The evaluator has two possible locations for a `MetaType` arm and the choice is load-bearing:

- **Inline loop-level dispatch in `evaluate_rules` (in `src/evaluator/engine/mod.rs`)** — use this when the variant has side-effects that the single-rule path cannot express. `MetaType::Default` (needs the per-level `sibling_matched` flag), `MetaType::Clear` (mutates `sibling_matched`), `MetaType::Use` (splices subroutine matches into the caller's match vector from `RuleEnvironment::name_table`), and `MetaType::Indirect` (recurses into `RuleEnvironment::root_rules` with a reset anchor and a sliced buffer) all live here. These variants are dispatched **before** the loop body calls `evaluate_single_rule_with_anchor`.
- **Silent no-op via the `Meta(_)` wildcard arm in `evaluate_single_rule_with_anchor`** — use this for variants that produce no match, do not mutate `sibling_matched`, and do not recurse. `MetaType::Name` falls here (it is also defensively scrubbed at load time by `parser::name_table::extract_name_table`, so the evaluator arm is a `debug!`-and-return safety net rather than a normal code path).

Similarly, `strength.rs::calculate_default_strength` and `types/mod.rs::bytes_consumed_with_pattern` treat all `Meta(_)` variants uniformly (zero strength contribution, zero bytes consumed) via catch-alls, so they do not need per-variant arms.

### 2.12 Unquoted `regex` Patterns Are getstr-Resolved to `Value::String`, Not Passed Through `parse_hex_bytes`

`parser/grammar/mod.rs::parse_magic_rule` special-cases `TypeKind::Regex` **ahead of** the generic `is_string_family_type` bareword fallback (S3.6): a quoted regex value (`regex/c "hello" ...`) still goes through `parse_value`'s existing `parse_quoted_string` path unchanged, but an unquoted (bareword) regex pattern is routed to `parser::grammar::getstr::parse_regex_getstr_value` instead of `parse_hex_bytes`/`parse_bare_string_value`. This exists because `parse_value`'s `alt(...)` tries the hex/mixed-ascii branch before any string interpretation, so a pattern beginning with a magic(5) escape (`\^`, `\040`, `\t`, `\x..`) was previously captured as `Value::Bytes` — the root cause of the system-magic-DB fatal abort this fix resolves (see S2.4). If the getstr resolver's own parse fails, `parse_magic_rule` falls back to `parse_value` so previously-working non-escaped and quoted forms are unaffected.

The resolver (`src/parser/grammar/getstr/mod.rs`) replicates GNU `file`'s `apprentice.c::getstr` escape table, verified against the upstream source (not inferred):

- `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v` resolve to their C control-character byte (`0x07`, `0x08`, `0x0C`, `0x0A`, `0x0D`, `0x09`, `0x0B`).
- `\0`-`\7` starts a 1-3 digit **octal** escape (`\NNN`), truncated to a byte (matches getstr's `CAST(char, val)`).
- `\x` starts a 0-2 digit **hex** escape (`\xNN`); with no valid hex digit following, getstr's quirky default is the literal byte `'x'` (`0x78`) — replicated here for fidelity, not "fixed."
- Every other escaped character — relation characters (`> < & ^ = !`), a literal backslash, an escaped `.`, an escaped space, an escaped literal tab character, and any otherwise-unrecognized escape such as `\^` — falls through getstr's `default:` case: **the backslash is dropped and the character is kept literally** (`\^` -> `^`, a regex anchor). `parse_bare_string_value` (used for non-regex string-family barewords) now follows the **same** drop-the-backslash rule for an unknown escape (`\<` -> `<`, `\^` -> `^`, `\ ` -> a literal space that continues the token), matching GNU `file`. An earlier revision had it **keep** the backslash, which broke real rules like sgml's `0 string \<?xml\ version=` (the value became the literal `\<?xml version=` and never matched an actual `<?xml ...` document). The two paths (getstr for regex, `parse_bare_string_value` for other string-family barewords) still differ in their `>= 0x80` handling, but neither flattens through `from_utf8_lossy` any longer -- getstr re-encodes a high byte as `\xHH` regex-native text (KTD3 above), while `parse_bare_string_value` returns `Value::Bytes` holding the raw bytes when the resolved token is not valid UTF-8 (mirroring `read_string_exact`; see S6.7). Both preserve the high byte, but in different representations -- getstr as regex-source text (required because the pattern must compile), `parse_bare_string_value` as raw bytes (correct for a plain `string` comparison) -- so regex must still not reuse `parse_bare_string_value` unchanged. The bullet list above is a summary of the notable named escapes, not an exhaustive taxonomy of this fallthrough bucket -- anything not named-control/octal/hex lands here, including plain punctuation that happens to be escaped for readability in a magic file.

**String-family bareword beginning with a backslash (now fixed -- third drop-backslash site).** A string-family bareword whose value *begins* with a backslash (e.g. sgml's `0 string \<?xml\ version=`) is routed through `parse_value`'s hex/mixed-ascii branch (S3.12) **before** the `parse_bare_string_value` fallback is ever reached, so `parse_bare_string_value`'s drop-backslash rule never fired for it. `parse_mixed_hex_ascii` (in `src/parser/grammar/value.rs`) now applies the **same** getstr drop-backslash policy in its unrecognized-escape fallback: after `parse_escape_sequence` (octal `\NNN`, `\n`/`\r`/`\t`/`\\`/`\"`/`\'`/`\0`) and `parse_hex_byte_with_prefix` (`\xNN`) both fail, a leading `\` followed by another character drops the backslash and keeps the character literally (`\<` -> `<`, `\^` -> `^`, `\ ` -> a literal space that CONTINUES the token instead of terminating it). A lone **trailing** backslash (nothing follows) still stays a literal `0x5c` byte -- required by the `0 regex \` recovery contract (`parse_magic_rule_regex_lone_trailing_backslash_pattern_recovers_via_value_fallback`). This resolves XML/plist detection: `\<?xml\ version=` now parses to the full `Value::Bytes(b"<?xml version=")` with a clean message, so `<?xml` documents and XML plists classify as "XML document text" instead of falling through to "ASCII text".

The output variant stays `Value::Bytes` **unconditionally** (never `Value::String`, even for all-ASCII content); recognized high-byte escapes (`\376`, `\xff`) are handled by the two branches above and reach `bytes` intact, and only an *unrecognized* escape hits the new fallback. (`parse_bare_string_value` now also preserves high bytes -- it returns `Value::Bytes` on non-UTF-8 tokens per S6.7 -- so the two paths no longer diverge on high-byte *preservation*; they differ only in that `parse_mixed_hex_ascii` always emits `Value::Bytes` while `parse_bare_string_value` emits `Value::String` for a valid-UTF-8 token.) This also brings `search` barewords into getstr parity with `regex`: the same escaped source text (`\^[\040\t]{0,50}\\.asciiz`) now resolves to identical bytes on both types (`Value::Bytes` for search, `Value::String` for regex) -- previously `search` kept the literal backslash on `\^`. Drop-backslash now lives in **three** sites with intentionally-divergent escape tables (getstr module, `parse_bare_string_value`, `parse_mixed_hex_ascii`); do not unify them without auditing each table (both string paths still omit `\a\b\f\v` that getstr resolves). Regression: `bareword_value_beginning_with_backslash_resolves_full_token_and_matches_xml` (parse + match) and `parse_magic_rule_search_with_same_escaped_pattern_drops_unknown_escape_backslash` (getstr-parity for search).

**XML version detail (now fixed -- see S14.3):** the resolved XML version detail previously rendered as `XML 1 document text` where GNU `file` prints `XML 1.0 document text`. The sgml rule is `>15 string/t >\0 %.3s document text`. This had TWO independent root causes, both now fixed: (1) the ordering-operator string read returned only the compared prefix (`pattern.len()` bytes = 1) as the display value; (2) `format_magic_message` discarded `%s` precision. See S14.3 for the compare/render decoupling and the precision support.

- getstr does **not** special-case `\d`, `\s`, or `\w` as PCRE-style character classes; they fall into the "everything else" bucket above and demote to the bare literal character (`\d` -> `d`, `\s` -> `s`, `\w` -> `w`). This is genuine upstream libmagic behavior, not a bug in the resolver.
- This resolver deliberately does **not** replicate getstr's `file_magwarn` lint diagnostics (e.g. "unnecessary escape" / "no such escape" warnings emitted by upstream `apprentice.c` while authoring a magic file). Those are magic-file-*authoring* diagnostics, orthogonal to escape *resolution* -- this module's job is producing the correct resolved byte sequence, not linting the source `.magic` file for style.

**`>= 0x80` re-encoding (KTD3):** a resolved byte `>= 0x80` cannot be pushed onto a Rust `String` as a raw `char` (not a valid standalone Unicode scalar value), so `push_resolved_byte` re-encodes it as a regex-native `\xHH` hex escape *in the output text* instead — the byte lives in the regex the engine compiles, never as a raw byte inside the `String`. This is required because `parse_text_magic_file` is fail-fast (S3.11): a raw `>= 0x80` byte pushed into a `String` would either panic or (via lossy handling) corrupt the pattern, and either way one such rule would abort the entire file load rather than just failing that rule. Named control escapes (`\a`/`\b`/`\f`/`\n`/`\r`/`\t`/`\v`) resolve to a single raw byte, not to escape *syntax*, so they are never reinterpreted by the downstream regex compiler as a shorthand (e.g. a resolved `0x08` byte is an ordinary literal, not the two-character sequence `\` + `b` that the `regex` crate would read as a word-boundary assertion) — no `\xHH` re-encoding is needed for these regardless of value.

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

`parse_value()` itself does not accept bare unquoted strings -- `parse_value("xyz")` still returns `Err` (see `test_parse_value_invalid_input` in `grammar/tests/mod.rs`, which pins this behavior). However, `parse_magic_rule` adds a bare-word fallback when the rule's type is string-family (`String`, `String16`, `PString`, `Regex`, `Search`), so `0 string TEST` and `>0 search/12 ABC` parse successfully without quotes -- matching libmagic magic(5) surface syntax and allowing real-world fixtures like `third_party/tests/searchbug.magic` to load. For non-string-family types (byte/short/long/etc.), bare words still fail; integration tests exercising `parse_value` directly (as opposed to going through `parse_magic_rule`) must still quote string literals.

**String-family barewords never parse as numbers (`parse_string_family_value`).** For a string-family type (except `Regex`, which has its own getstr branch -- S2.12), `parse_magic_rule` does **not** call `parse_value` at all. It calls `parse_string_family_value`, which runs a leading `multispace0` trim and then a three-way `alt`: quoted string (-> `Value::String`), hex/escape bytes (-> `Value::Bytes`, e.g. gzip's `\037\213`, `\177ELF`), then `parse_bare_string_value` (-> `Value::String`) for everything else. Crucially it **omits** `parse_value`'s float and integer branches. libmagic never treats a `string` comparison value as numeric: `0 string >0` compares against the literal ASCII byte `'0'` (0x30), and `>0.6.1` against the literal characters `0.6.1`. An earlier revision routed these through `parse_value` first, which captured bareword `0` as `Value::Uint(0)` and `0.6.1` as `Value::Float(0.6)` (leaking `.1` into the message); the subsequent `String`-vs-number comparison is incomparable (`compare_values` returns `None`), so the rule **silently never matched** -- breaking the pervasive `>0 ... %s` idiom (`\b, name %s`, `face %s`, `palette %s`) and version compares (`>0.6.1 ... version %s`). This applies to **both equality and ordering** operators -- `parse_string_family_value` never inspects the operator, so `0 string 000` also flips from `Value::Uint(0)` to `Value::String("000")` (correct per libmagic).

Two invariants must be preserved by anything that touches this function: (1) the leading `multispace0` is load-bearing -- it makes the hex branch see byte-identical input to what `parse_value` fed it, so an escape-heavy value cannot fall through to the lossy-UTF-8 `parse_bare_string_value` path and corrupt a high byte (see the high-byte-UTF-8 corruption class); and (2) hex-*letter* barewords (`>AB`, `cafebabe`) still resolve to `Value::Bytes` via the unchanged hex branch (S3.12) -- only the numeric subset changed. This composes with S14.3: a rule parsed as `0 string >0.6.1` now correctly renders the full field (`0.6.2 release`) while comparing only the `pattern.len()` prefix (`0.6.10` does NOT match, because prefix `0.6.1` equals the pattern).

**`search` + ordering-op interaction (S2.4).** `Search` is pattern-bearing, so a non-equality operator on it is an intentional `UnsupportedType` fatal gap (S2.4, not allowlisted). Before this change a numeric bareword like `0 search/16 >100` parsed to `Value::Uint(100)`, which -- having no String/Bytes pattern operand -- hit the *allowlisted* `"search without string/bytes pattern"` graceful skip. Now it parses to `Value::String("100")`, so the pattern operand IS present and the `>` operator trips the (deliberate) S2.4 fatal path instead. This is not a real-world regression: no magic rule in the GNU `file` DB uses `search`/`regex` with an ordering operator on a numeric/string bareword (the only real hit, lex's `search/1 >\0`, is a `\0`->`Value::Bytes` value on the unchanged hex branch, and was already on the S2.4 fatal path before and after this change). Do NOT "fix" the skip->fatal flip by widening the S2.1 allowlist to cover non-equality ops on pattern-bearing types -- S2.4 exists precisely to surface that capability gap. Regex is unaffected regardless (it uses its own getstr branch, S2.12, not `parse_string_family_value`).

Regressions: `test_string_family_bareword_numeric_value_parses_as_string` (covers equality + ordering; the `search` case uses an equality op deliberately), `test_string_family_value_preserves_quoted_and_hex_bytes`, and end-to-end `test_string_numeric_ordering_end_to_end_composes_with_full_field_render`. The system-DB integration tests (`system_magic_dir`, `compatibility_tests`) that load the real magic DB are the actual regression net; the ad-hoc 525-file `file`-differential showed zero *observable changes* (no regressions and no visible top-level improvements -- the affected `>0 %s` rules are deep children whose parents did not match in that sample), so the real-file proof is the synthetic before/after plus those passing DB-loading tests.

### 3.7 Indirect Offset Pointer Specifiers Follow GNU `file` Semantics

Lowercase pointer specifiers (`.s`, `.l`, `.q`) map to **little-endian**, not native endian. Uppercase (`.S`, `.L`, `.Q`) map to big-endian. All numeric pointer types are **signed by default** (per S6.3).

Adjustment is accepted in two forms:

- **Inside the parens** (canonical magic(5)): `(base.type+N)`, `(base.type-N)`, `(base.type*N)`, `(base.type/N)`, `(base.type%N)`, `(base.type&N)`, `(base.type|N)`, `(base.type^N)`. The full magic(5) operator set is supported here.
- **After the closing paren** (legacy/alternate): `(base.type)+N`, `(base.type)-N`. Only `+`/`-` are accepted in this form.

Only one form may be used per rule; combinations like `(19.b-1)+2` are not permitted. The operator selection is stored on `OffsetSpec::Indirect.adjustment_op` (`IndirectAdjustmentOp` enum). Subtraction is folded into `IndirectAdjustmentOp::Add` with a negative `adjustment` so the evaluator does not need a dedicated `Sub` variant.

The evaluator (`evaluator/offset/indirect.rs::apply_adjustment`) treats the operand as `i64` for `Add` (signed addition) and reinterprets it as `u64` for `Mul`/`Div`/`Mod`/`And`/`Or`/`Xor` to match libmagic's `apprentice.c::do_offset` raw-machine-word semantics. `Div`/`Mod` reject a zero operand with `EvaluationError::InvalidOffset`; `Mul` rejects integer overflow the same way. `IndirectAdjustmentOp` derives `Default = Add` so a missing field round-trips correctly through serde for older AST snapshots.

### 3.8 Relative Offsets: Global Anchor, No Save/Restore

`OffsetSpec::Relative(N)` resolves against `EvaluationContext::last_match_end()`, which is updated after every successful match in `evaluate_rules` and is **never saved/restored across child recursion**. This is intentional and matches GNU `file`: a sibling rule sees the anchor wherever the deepest descendant of the previous sibling left it. The anchor is global/shared rather than stack-scoped, but its numeric value is not guaranteed to be non-decreasing -- a successful `Relative(-N)` rule (or any later rule that matches at a lower absolute position) can move it earlier. Do not wrap recursion in a save/restore pair "for safety" -- it would silently break sibling-after-nested chains. The recursion-depth pattern in the same loop *is* save/restore, and the asymmetry is correct.

The load-bearing invariant is that the anchor is updated *before recursing into children* (so children and their followers see the new anchor). The current code also happens to set the anchor before `matches.push(...)`, but the push-ordering relative to `set_last_match_end` is incidental for anchor correctness -- only the ordering before the `evaluate_rules` recursion call matters. (Future code that reads the anchor while iterating `matches` would make this ordering load-bearing, so do not "optimize" the order without checking call sites first.) `bytes_consumed()` (in `evaluator/types/mod.rs`) is the source of truth for advance distance; for variable-width types it re-derives consumption from the buffer rather than trusting `Value::String.len()` (which can drift from the original byte length via `from_utf8_lossy`). Pascal-string consumption is also clamped against the remaining buffer to prevent attacker-controlled length prefixes from poisoning the anchor to `usize::MAX`.

**Continuation-sibling exception (issue #42):** Inside a child recursion (`recursion_depth > 0`), the anchor IS reset to the sibling list's entry value between iterations. Continuation rules at the same indentation level (`>>&0 ubyte ...; >>&0 offset ...`) all resolve `&N` against the parent-level anchor rather than chaining off each other. This matches libmagic's `ms->c.li[cont_level]` continuation-level model and is required for the GNU `file` `searchbug.magic` fixture to produce `at_offset 11` (not `at_offset 12`). Top-level siblings (`recursion_depth == 0`) keep the chaining behavior described above. The `reset_anchor_between_siblings` flag in `evaluate_rules` gates this. Tests that assert top-level sibling chaining (e.g. `relative_anchor_can_decrease_when_later_sibling_matches_at_lower_position`) must stay at depth 0; tests that assert continuation-sibling parent-anchor resolution (e.g. `test_offset_does_not_advance_anchor_for_continuation_siblings`) must go through a parent rule's children. The two semantics coexist deliberately; do not "unify" them without reading the entire GNU `file` `searchbug` test chain.

### 3.10 Subroutine Base Offset for `use` Bodies

Inside a `MetaType::Use` subroutine body, `OffsetSpec::Absolute(n)` with `n >= 0` resolves to `base_offset + n`, where `base_offset` is the use-site offset. `EvaluationContext::base_offset` tracks this; `evaluate_use_rule` saves and restores it around the subroutine call, alongside `last_match_end`. This matches magic(5) / libmagic semantics: a subroutine written as `>0 search/12 ABC` invoked via `>>64 use part2` scans starting at file position 64, not 0. Negative absolute offsets (`FromEnd`-style), `Indirect` pointer reads, and `Relative(&N)` offsets are unaffected -- they already have well-defined reference frames. Callers of `offset::resolve_offset_with_base` pass `context.base_offset()`; the `base_offset` field is gated behind `pub(crate)` accessors so external consumers cannot inject arbitrary offset bias.

### 3.11 Parsing Is Strict at Build Time, Line-Tolerant at Runtime

There are **two** hierarchy builders, differing only in how they treat a rule line that fails to parse:

- **Strict (`build_rule_hierarchy` / `parse_text_magic_file`)** -- propagates any `parse_magic_rule_line` error immediately, so a single unparseable rule causes the **entire** call to fail with `ParseError::InvalidSyntax`. This is used by **build-time codegen** (`src/build_helpers.rs`) so a malformed rule in the crate's OWN builtin magic fails the build loudly.
- **Tolerant (`build_rule_hierarchy_tolerant` / `parse_text_magic_file_tolerant`)** -- skips an unparseable rule **and its deeper-indented subtree** with a `warn!` and keeps the rest of the file, matching GNU `file`. This is used by the **runtime** loaders (`load_magic_file`, `load_magic_directory`). Real system magic databases routinely mix rules this parser fully supports with a handful using constructs it does not yet handle (`guid`, `der`, middle-endian and quad-date variants, some indirect-offset forms). Without tolerance, ONE such rule dropped the whole file -- e.g. losing all of `compress`'s gzip/bzip2 detection over its lone `ustring` XZ rule, or all of `jpeg`/`pdf`/`linux`/`windows`. A prior audit found ~21 real system files being discarded this way.

**Subtree drop, not single-line skip:** when a rule line fails, its descendants (lines with a deeper `>`-level, counted by `leading_indent_level`) are dropped too, so an orphaned child cannot silently re-attach to the wrong parent. A pending `!:strength` directive orphaned by the skip is discarded.

**Directory-level "all failed" contract preserved:** `load_magic_directory` still errors when a non-empty directory loads nothing usable. A file counts as success only if it contributes at least one rule or name-table entry; a content-bearing file whose rules were ALL skipped is tracked (via `has_rule_lines`) and, if every file in the directory is like that (or fails preprocessing), the load fails with the same truncated "All N ... failed to parse" report. Genuinely empty or comment-only files are valid and contribute nothing.

When writing corpus tests against third_party `.magic` files that mix supported and unsupported syntax, you can either use the tolerant runtime path (which now loads them) or bypass the parser and build the equivalent `MagicRule` tree programmatically via the AST. See `tests/evaluator_tests.rs::test_regex_eol_corpus` for the AST-build approach and `src/parser/loader.rs::tests::test_load_magic_file_tolerates_unparseable_rule_and_keeps_valid_ones` for the tolerant contract.

### 3.10 `parse_text_magic_file` Returns `ParsedMagic`, Not `Vec<MagicRule>`

`parse_text_magic_file`, `load_magic_file`, and `load_magic_directory` all return `Result<ParsedMagic { rules: Vec<MagicRule>, name_table: NameTable }, ParseError>`. Top-level `Meta(Name(id))` rules are hoisted *out* of the flat rule list at parse time by `parser::name_table::extract_name_table` and placed into the `name_table` field keyed by identifier. Duplicate names keep the first definition and emit a `warn!`; nested `Name` rules (not well-defined in magic(5)) are scrubbed with a warning during extraction.

- **Callers must destructure at the boundary.** Codegen consumers (`build.rs`, `src/build_helpers.rs`) use `parsed.rules` and discard the table. Runtime consumers (`MagicDatabase::load_from_file_with_config`) pattern-match `let ParsedMagic { rules, name_table } = ...;` and wrap both in `Arc`s before handing them to the database.
- **Directory loads merge name tables with first-wins semantics.** `load_magic_directory` merges per-file name tables alphabetically (same ordering as the rule merge); duplicate-name warnings during merge are distinct from per-file duplicate-name warnings during extraction.
- **Evaluator consumption is via `RuleEnvironment`** (`pub(crate)` in `evaluator/mod.rs`), threaded as an optional `Arc<RuleEnvironment>` on `EvaluationContext`. Low-level callers (`evaluate_rules`, `evaluate_rules_with_config`) leave this as `None` and `Use` rules no-op silently -- this preserves the low-level API for property tests and fuzz harnesses that construct rule trees by hand. `MagicDatabase::evaluate_buffer_internal` attaches the environment before dispatching.
- **Each subroutine in the name table must be strength-sorted** the same way top-level rules are (via `sort_rules_by_strength_recursive`), otherwise `use`-site evaluation is non-deterministic with respect to source order inside the `name` block. `MagicDatabase::load_from_file_with_config` iterates `name_table.values_mut()` to do this after the top-level sort.

See `docs/solutions/integration-issues/meta-type-subroutine-dispatch-architecture.md` for the full three-layer pattern (parse-time hoist, `ParsedMagic` return type, optional `RuleEnvironment`).

### 3.12 `parse_hex_bytes_no_prefix` Requires a Token Boundary After the Hex Run

`parse_hex_bytes_no_prefix` (in `src/parser/grammar/value.rs`, one of the three `alt(...)` branches inside `parse_hex_bytes`) is used for a bareword (no `\` prefix) value token that looks like a run of pure hex digit pairs. Before the U7 audit (fix-system-magic-regex-graceful plan), it captured hex digits via `take_while(is_ascii_hexdigit)` and stopped there, returning `Ok` with the truncated byte vector and leaking the remainder as the parser's "remaining input" -- e.g. `parse_hex_bytes("4a[42")` returned `Ok(("[42", vec![0x4a]))`, silently dropping to a 1-byte value and leaking `"[42"` back to the caller, where `parse_magic_rule` would misinterpret it as the start of the rule's message text. The fix adds `is_hex_token_boundary`: after the hex-digit run, the immediately-following character must be end-of-input, whitespace, or a closing quote (`"`), or the whole token is rejected (`Err`) rather than truncated. This only affects the no-prefix branch -- `parse_mixed_hex_ascii` (used when the token starts with `\`) already consumed the whole token via its `none_of(" \t\n\r")` fallback and needed no fix; see `src/parser/grammar/tests/hex_bytes_truncation.rs` for the confirmed-bug regression test and the differential coverage pinning `parse_mixed_hex_ascii`'s pre-existing whole-token behavior. `TypeKind::Regex` no longer routes through this code path at all (S2.12), but `TypeKind::Search` and any other string-family type whose bareword value happens to look like hex bytes still does.

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

### 6.4 `TypeKind::String` Has Two Read Modes -- Pick Consciously

`src/evaluator/types/string.rs` exposes **two** read functions and the dispatcher in `src/evaluator/types/mod.rs::read_typed_value_with_pattern` picks between them based on `(max_length, pattern)`:

- `read_string_exact(buffer, offset, length)` reads **exactly** `length` bytes with NO NUL truncation. Used for libmagic-compatible `string PATTERN` byte-for-byte comparison whenever a comparison value is supplied -- including patterns that legitimately contain embedded NULs (e.g. `0 string PNCIHISK\0 Norton Utilities disc image data`). Selecting this path is critical: a 9-byte pattern ending in `\0` compared against a 9-byte buffer ending in `\0` must read 9 bytes, not stop at 8.
- `read_string(buffer, offset, max_length)` reads until the **first NUL** or end of buffer, capped by `max_length` if set. Used for the `x` (any-value) operator path where the read is for printable-prefix output rather than equality comparison. Adding a new code path that needs string bytes must pick consciously: pattern comparison -> `read_string_exact`; printable scan -> `read_string`.

Historical note (NUL-free buffer behavior): when `read_string` runs with `max_length: None` on a NUL-free buffer (raw ASCII text, JSON, log lines), it reads the *entire remaining buffer*, which historically broke equality comparison against short target values. The pattern-aware dispatcher now routes those comparisons through `read_string_exact` so that failure mode no longer applies; programmatic rules constructed by hand without going through the dispatcher should still set `max_length: Some(target_len)` explicitly when targeting NUL-free buffers.

The full backstory of why both functions exist (and why the previous single-function design silently broke `0 string PNCIHISK\0 ...` and similar rules) is documented in `docs/solutions/logic-errors/magic-string-rule-matching-3-bug-fix-2026-04-25.md`.

### 6.5 `/c` and `/C` String Flags Are Asymmetric — Pattern Char Controls Fold Direction

Per libmagic `src/softmagic.c`, the case-insensitive string flags are **not** symmetric "both sides fold to lower". They are direction-controlled by the pattern character:

- **`/c` (`STRING_IGNORE_LOWERCASE`)** fires only when the pattern char is lowercase. When it fires, the file byte is `tolower`'d before comparison. Pattern `b"foo"` matches `b"FOO"`, `b"Foo"`, `b"foo"`. Pattern `b"FOO"` is compared literally regardless of `/c`.
- **`/C` (`STRING_IGNORE_UPPERCASE`)** fires only when the pattern char is uppercase. When it fires, the file byte is `toupper`'d before comparison. Pattern `b"FOO"` matches `b"foo"`, `b"Foo"`, `b"FOO"`. Pattern `b"foo"` is compared literally regardless of `/C`.

Mixed-case patterns work: `/c FoO` matches `b"FoO"`, `b"Foo"`, `b"FOO"` (the lowercase `o` positions accept any case) but **not** `b"fOO"` (the uppercase `F` position is literal and case-folding does not fire). Patterns with the "wrong" case for the flag fall through to literal byte comparison — this is the source of confused "why doesn't my pattern match?" bug reports.

Use `u8::to_ascii_lowercase` / `to_ascii_uppercase`, not full Unicode case-folding — libmagic uses C's `tolower`/`toupper` which are de-facto ASCII for magic-file processing. See `src/evaluator/types/string.rs::compare_string_with_flags` for the implementation and the table-driven tests for the asymmetry coverage.

### 6.6 `/B` Is Pstring-Only, Not a String Flag

magic(5) and libmagic `src/file.h` reserve `/B` (`CHAR_PSTRING_1_BE`) as a `pstring` length-width letter. It is **not** a `string` flag — `string/B` is rejected at parse time. An earlier draft of the grammar (PR #233) included `'B'` in the string-flag set; that was wrong and has been corrected. The pstring suffix parser (`src/parser/grammar/type_suffix.rs::parse_pstring_suffix`) is the only place `/B` should be accepted.

### 6.7 `parse_bare_string_value` Returns `Value::Bytes` on Non-UTF-8 (Parse-Side Mirror of `read_string_exact`)

`parse_bare_string_value` (`src/parser/grammar/mod.rs`) is the bareword fallback for string-family types (`string`, `pstring`, `search`; **not** `regex`, which has its own getstr branch -- S2.12). It resolves the token's magic(5) escape sequences into a `Vec<u8>` and then chooses the `Value` variant by UTF-8 validity: valid UTF-8 -> `Value::String` (so `%s` output renders normally), non-UTF-8 -> `Value::Bytes` holding the raw bytes. This exactly mirrors `read_string_exact` (S6.4), which was already fixed to return `Value::Bytes` on a non-UTF-8 slice.

**Why the old `from_utf8_lossy` -> `Value::String` was a silent-never-match bug.** A bareword whose resolved bytes contain a byte `>= 0x80` that isn't part of a valid UTF-8 sequence -- e.g. OS/2 INF's top-level signature `0 string HSP\x01\x9b\x00 OS/2 INF` (bytes `48 53 50 01 9b 00`, where `0x9b` is invalid UTF-8) -- was lossy-decoded to `"HSP\x01\u{FFFD}\x00"`. That does two things, both fatal to matching: (1) it **inflates the byte length** from 6 to 8 (U+FFFD is 3 bytes `EF BF BD`), so the dispatcher's `read_string_exact(buffer, offset, pattern.len())` reads 8 bytes instead of 6; and (2) it **changes the byte value** at that position (`9b` -> `EF`), so even the read prefix mismatches. The rule fell through to the ascmagic `data`/`ASCII text` fallback. The signature `HSP` begins with literal ASCII, so it does **not** start with `\` and therefore bypasses `parse_mixed_hex_ascii` (which unconditionally emits `Value::Bytes` -- S2.12) -- `parse_bare_string_value` was the only high-byte-preserving gap left after `read_string_exact` was fixed.

Cross-type `String`/`Bytes` equality and ordering (S2.3) compare by byte sequence, so the returned variant (`String` for the all-UTF-8 case, `Bytes` for the high-byte case) always compares correctly against the read value regardless of which variant the read side produced. The fix covers **both** escape forms that can carry a high byte in a bareword: `\x9b` (hex) and `\376` (octal). Regression: `parse_bare_string_value_high_byte_returns_bytes_not_lossy_string` (parse) and `os2_inf_high_byte_signature_matches` (end-to-end). See the high-byte-utf8-corruption recurring-bug class.

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

### 8.2 `unsafe_code = "deny"` Has Exactly One Vetted Exception

`unsafe_code = "deny"` in `Cargo.toml` workspace lints applies project-wide via `[lints] workspace = true` in the package section (without that opt-in the whole `[workspace.lints]` table is inert -- do not remove it). The level is `deny` rather than `forbid` because the memmap2 `map()` call in `src/io/mod.rs::create_memory_mapping` requires one `#[allow(unsafe_code)]` block: memory mapping is inherently unsafe (a concurrently-truncated file is UB at the OS level) and `forbid` would make the exception impossible. Any new `unsafe` block needs the same treatment: a SAFETY comment explaining the invariants (enforced by `clippy::undocumented_unsafe_blocks = "deny"`) and explicit review sign-off. For everything else, use vetted crates (e.g., `chrono` for timezone) instead of libc FFI or subprocess calls.

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

### 13.2 `EvaluationConfig::default()` Stops at First Top-Level Match -- But Only Once That Match Produces Output

`EvaluationConfig::default()` sets `stop_at_first_match: true`, so once any top-level rule produces a match **that carries usable description text**, the evaluator halts further sibling iteration. Integration tests that rely on later siblings running (e.g., `default`/`clear`/`default` chains, `use`-followed-by-sibling continuation, anything exercising the per-level `sibling_matched` flag across the full chain) must use `MagicDatabase::load_from_file_with_config` with `EvaluationConfig::default().with_stop_at_first_match(false)`. Unit tests that go through `evaluate_rules` directly hit the same issue -- override the config at construction time. The existing `Use` tests (`test_use_child_rules_evaluated_after_subroutine`, `test_use_stop_at_first_match_short_circuits_siblings`) document both sides of this contract.

**Refined contract (the "message-less shadowing" fix).** A top-level rule that matches but contributes NO usable description text -- an empty message, a whitespace-only message, or a `\b`-only message (see S14.1), and no message-bearing descendant match either -- does **not** count as the terminating match. Evaluation continues to the next top-level sibling instead. This matters because real-world magic files routinely use message-less top-level rules purely as gating conditions for children (e.g. `/usr/share/file/magic/c-lang`'s `0 search/8192 "#include"` -> `>0 regex \^#include c` chain). Under the OLD (pre-fix) contract, if such a gating rule happened to match a buffer on its own (no matching child) BEFORE a later, more specific rule was tried in strength order, the whole evaluation would halt right there and the CLI would print a **blank description** -- this was a real, maintainer-reported bug (an assembler-source file starting with a leading tab and `.asciiz`, and even plain ASCII text with no special content, both produced blank output against the real system magic DB). The fix is implemented via `has_message_bearing_match` / `is_message_bearing` in `src/evaluator/engine/mod.rs`, checked at every one of the five successful-match dispatch sites in `evaluate_rules` (the plain value-rule path, `Default`, `Indirect`, `Offset`, and `Use`) before the `should_stop_at_first_match()` break fires. `sibling_matched` (the `default`/`clear` dispatch flag) is UNAFFECTED by this refinement -- a message-less match still counts as "a sibling matched" for that purpose; only the `stop_at_first_match` early-exit is gated on message-bearing output.

See S13.3 for the companion text/data fallback that also had to be added for this fix to fully resolve the reported bug (a message-less rule not stopping evaluation is necessary but not sufficient -- if NO rule ever produces output, something must still replace the blank description).

### 13.3 Text/Data Fallback When No Rule Produces a Description

`MagicDatabase::build_result` (`src/lib.rs`) falls back to `crate::output::ascmagic::classify_fallback(buffer)` whenever rule evaluation produces no usable description -- either because `matches` is empty, or because every match's message is empty after concatenation (see S13.2's refined stop-at-first-match contract; the fallback is what makes "continue past message-less matches" actually resolve to a non-blank final answer instead of an empty string once every top-level rule is message-less or none match at all). This mirrors GNU `file`'s `file_ascmagic` basic content classification: `"empty"` for a zero-byte buffer, `"ASCII text"` for buffers containing only printable ASCII plus common text control characters (tab, LF, CR, VT, FF, and the legacy bell/backspace/escape bytes GNU `file` still treats as text), `"UTF-8 Unicode text"` for valid non-ASCII UTF-8, and `"data"` for anything else (binary content).

This is a deliberately narrow subset of GNU `file`'s real charset detection (no ISO-8859 variants, no UTF-16, no "with CRLF line terminators"/"with escape sequences" qualifiers -- see `src/output/ascmagic.rs`'s module doc for the full list of what is intentionally not replicated). Every classification it produces is a true subset of what GNU `file` would print for the same input (e.g. `file` says `"ASCII text, with CRLF line terminators"` where this crate says plain `"ASCII text"`), so substring-based differential tests against the real `file` binary still hold even though byte-for-byte parity is not attempted.

`MagicDatabase::concatenate_messages` also skips any match whose rendered (post `format_magic_message`) text is empty, rather than only special-casing the leading-position and `\b`-prefixed cases -- otherwise a message-less match sandwiched between two message-bearing ones would introduce a stray double space in the final description.

**`apply_bitwise_and` semantics fix (found while validating this fallback, fixed in the same change).** `src/evaluator/operators/bitwise.rs::apply_bitwise_and` (the `Operator::BitwiseAnd`, i.e. bare `&VALUE` relational test, as opposed to the `type&MASK` attached-mask form which uses `BitwiseAndMask`/`apply_bitwise_and_mask`) previously implemented `(value & operand) != 0` ("any masked bit set"). Empirical A/B testing against the real `file` binary on `/usr/share/file/magic/ibm6000`'s `4 belong &0x0feeddb0` entry (a bare mask with no explicit comparison -- magic(5) shorthand for `belong &0x0feeddb0 =0x0feeddb0`) proved the correct libmagic semantics is `(value & operand) == operand` ("all masked bits set", matching GNU `file`'s `magiccheck()` in `src/softmagic.c`): an all-zero masked value and a partial-bit-overlap (nonzero, but not equal to the full mask) value both correctly produce "data" from real `file`, but the old implementation spuriously matched the partial-overlap case -- and because the ibm6000 mask has ~17 bits set, this false-positive fired for the overwhelming majority of random 32-bit values at that offset (nearly any binary blob evaluated against the full system magic DB used to trigger a bogus "AIX core file" classification). `apply_bitwise_and` now implements `(left & right) == right` for all four `Value` integer-type combinations. Consequences worth knowing:

- **Not commutative in general.** `apply_bitwise_and(a, b)` asks "does `a` have every bit of `b` set" -- swapping operands generally changes the answer (`apply_bitwise_and(0xFF, 0x0F)` is true, `apply_bitwise_and(0x0F, 0xFF)` is false). It IS trivially true for self-AND (`a & a == a` always). The old "any bit set" semantics genuinely was commutative, so `test_apply_bitwise_and_symmetry` was renamed to `test_apply_bitwise_and_is_not_commutative_in_general` and rewritten to pin the asymmetry rather than assert it away.
- **A zero mask is vacuously true.** `(v & 0) == 0` always holds, regardless of `v` -- there are no required bits, so none can fail to be set. This matches libmagic's literal equality test; nobody writes real magic-file rules with a `&0` mask, but the semantics is still worth knowing.
- **Single-bit masks are unaffected.** For a mask with exactly one bit set, "any bit set" and "all bits set" coincide (the value is either 0 or the mask). The overwhelming majority of real magic-file bare-`&` usage is single-bit flag checks (`>6 leshort &0x0001 \b, encrypted`, `>3 byte &0x01 \b, ASCII text`, etc. -- see `docs/MAGIC_FORMAT.md`), so this fix does not change their behavior. Only bare `&` with a genuinely multi-bit mask (rare; the ibm6000 entry above is the corpus example that surfaced this) is affected, and only in the direction of becoming *more* correct (stricter -- it can only remove false-positive matches, never add new ones, since "all bits set" implies "some bit set" whenever the mask is nonzero).

### 13.4 `stop_at_first_match` Is Top-Level-Only, and Continuation Rules Are NOT Strength-Sorted (they interlock)

Two libmagic-parity rules that must be maintained together (both landed in the fix-system-magic-regex-graceful work; together they make multi-fragment descriptions like gzip's `gzip compressed data, ..., from Unix, original size modulo 2^32 N` render correctly and in order):

- **`stop_at_first_match` short-circuits only the TOP-LEVEL classification loop.** `evaluate_rules` computes `stop_at_first_match_applies = !is_child_sibling_list` at entry (`is_child_sibling_list = recursion_depth > 0 && !is_indirect_reentry`) and gates every one of the five `break` sites on it. Inside a child/continuation sibling list or a `use` subroutine body the break NEVER fires -- **all** matching siblings render, matching libmagic where continuation levels always evaluate every entry and only the outer `match()` loop stops at first success. An indirect re-entry is treated as a fresh top-level classification (it consumes `take_indirect_reentry()` so `is_child_sibling_list` is false), so its own sibling list DOES stop at first match, but the children of a rule matched inside it do not. An earlier revision applied the break at every recursion level, silently truncating a description after its first message-bearing child (dropping gzip's `max compression`, `from Unix`, `original size ...`). The `EvaluationConfig::stop_at_first_match` doc already promised top-level-only; the code now matches. Regression: `test_stop_at_first_match_does_not_truncate_child_siblings`.
- **`sort_rules_by_strength` is non-recursive; only top-level entries are sorted.** `MagicDatabase` load (`src/lib.rs`, both the builtin and from-file paths) calls `sort_rules_by_strength` (NOT `sort_rules_by_strength_recursive`) and does **not** sort `name`-block subroutine bodies at all. This mirrors libmagic's `apprentice_sort`, which orders whole magic entries by their first line's strength and never reorders continuation lines. Child rules and subroutine bodies stay in FILE order because their order is load-bearing: (a) `default`/`clear` fire based on whether an **earlier-in-file** sibling matched -- strength-sorting a comparison-bearing sibling ahead of a low-strength `default` wrongly suppresses the `default`; and (b) multi-fragment descriptions must render in source order. The two fixes interlock: once stop-at-first-match no longer truncates children, child evaluation order only affects fragment order + `default`/`clear`, and file order is exactly what makes both correct. The old `NameTable::sort_subroutines` machinery was removed (subroutine bodies must not be sorted). Regressions: `test_sort_rules_by_strength_preserves_child_file_order`, and the end-to-end `test_gzip_multipart_description_end_to_end`.

### 13.5 The `offset` Pseudo-Type Compares the Resolved Position; `-0` Is EOF (`FromEnd(0)`)

The magic(5) `offset` pseudo-type (`TypeKind::Meta(MetaType::Offset)`) treats the **resolved offset itself** as the read value and applies the rule's comparison operator against it -- it does NOT read bytes at that position. `offset x` is a bare AnyValue placeholder that always matches (used to report the position via `%lld`); `offset >48` / `offset <48` / `offset =N` compare the resolved position against N. gzip's `>>-0 offset >48` gates the trailing "original size modulo 2^32" child on the file being long enough. The engine's `Offset` dispatch (`src/evaluator/engine/mod.rs`) skips the rule as a non-match when the comparison fails (so the false branch and its children do not render). An earlier revision only supported `x` and routed everything else through the meta no-operand parse path, which turned `>48` into literal message text and made the compare a no-op.

**`-0` parses to `OffsetSpec::FromEnd(0)`, not `Absolute(0)`.** magic(5) `-0` means "0 bytes from end of file" = the EOF **position** (`buffer.len()`), one past the last readable byte -- but `-0 == 0` in a signed integer, so the sign is otherwise lost. `parse_offset` (`src/parser/grammar/mod.rs`) detects the leading `-` explicitly and, for a zero magnitude, emits `FromEnd(0)`; other negatives (`-4`) keep their `Absolute(-4)` encoding (which the evaluator already resolves from the end). The `FromEnd` resolver arm special-cases `FromEnd(0)` to return `buffer.len()` (valid as a position for the read-less `offset` type; `resolve_absolute_offset(0)` would wrongly send it through the positive path and yield start-of-file). Negative `FromEnd` deltas keep the shared from-end resolution. Regressions: `test_resolve_from_end_zero_is_eof_position`, `test_offset_from_end_zero_compares_against_file_size`, and the `parse_offset("-0") == FromEnd(0)` assertion in `test_parse_offset_edge_cases`.

## 14. Output Formatting

### 14.1 `\b` (Backspace) Prefix in Rule Messages Suppresses Leading Space

`MagicDatabase::build_result` (via `concatenate_messages`) concatenates rule messages with a space separator, **except** when a message starts with the backspace no-separator marker, in which case the marker is stripped and no leading space is inserted. This mirrors GNU `file`'s description formatting (used by rules like `>&1 regex/1l ... \b, version %s` to produce `Ansible Vault text, version 1.1` instead of `Ansible Vault text , version 1.1`).

**The marker has two forms, and both are stripped.** The parser (`parse_message`) preserves description text verbatim -- matching GNU `file`, which keeps the desc literal and special-cases a leading `\b` at print time -- so a magic-file message like `\b, for MS Windows` (msdos) or `\b]` (Mach-O universal binary) reaches concatenation as the **literal two-character sequence `\b` (backslash + `'b'`)**, NOT a `\u{0008}` byte. `concatenate_messages` strips a leading `\u{0008}` byte (for programmatically-constructed messages / older tests) *or* a leading literal `"\\b"`. An earlier revision only stripped the `\u{0008}` byte, so real system-DB rules rendered the literal marker into the output (`PE STUB    \b, for MS Windows`, `...architectures: \b]`). A leading `\b` in a *description* is always the marker (LaTeX-style `\begin` etc. appear only in string *values*, never descriptions). Tests that manually simulate the concatenation path (e.g., corpus tests that bypass `load_from_file` -- see S3.11) must honor this convention or their assertions will diverge from the real evaluator output.

### 14.2 Printf-Style Format Specifiers Are Substituted by `format_magic_message`

Magic rule messages like `at_offset %lld` or `followed_by 0x%02x` are substituted with the rule's `RuleMatch.value` at description-assembly time, via `src/output/format.rs::format_magic_message`. The supported subset covers `%d`, `%i`, `%u`, `%x`, `%X`, `%o`, `%s`, `%c`, and `%%`, plus width/padding modifiers (`%05d`, `%-5d`) and length modifiers (`l`, `ll`, `h`, `hh`, `j`, `z`, `t`) which are parsed and ignored (all numeric rendering uses `u64`/`i64` width).

Hex specifiers mask the value to the natural width of the rule's `TypeKind` -- a signed byte carrying `-1` renders as `ff`, not `ffffffffffffffff`. Unknown specifiers pass through literally with a `debug!` log, matching the evaluator's graceful-skip discipline.

Substitution runs BEFORE the backspace check in S14.1, so a rule emitting `\b, version %s` correctly composes with the preceding match without an intervening space after the value is substituted.

Tests that manually simulate the concatenation path must run their message strings through `format_magic_message` or construct `RuleMatch::message` strings that contain no `%` metacharacters. A literal `%` in user-facing data should be escaped as `%%`.

### 14.3 String Ordering Ops Render the Full Field; Comparison Stays Prefix-Limited (`%.Ns` Precision)

For a `string` rule compared with an **ordering** operator (`<`/`>`/`<=`/`>=`), libmagic **compares** only the first `pattern.len()` bytes (`src/softmagic.c::magiccheck` -> `file_strncmp(m->value.s, p->s, m->vallen, ...)`) but **renders** the full string field (`p->s`, read until NUL/EOF). These two are decoupled and BOTH matter -- the sgml XML rule `>15 string/t >\0 %.3s document text` is the canonical case: comparing `>\0` reads 1 byte, but `%.3s` must render the full field `1.0` (from `1.0" encoding=...`) to produce `XML 1.0 document text`.

- **The decoupling lives in `evaluate_value_rule` (`src/evaluator/engine/mod.rs`), NOT in the read dispatcher.** `matched` is computed first from the untouched exact-length read (`read_typed_value_with_pattern` -> `read_string_exact(pattern.len())`), so the match decision is byte-identical to before. Then `string_ordering_display_value` re-reads the full field via `read_string(buffer, offset, Some(max_string_length))` for the RETURNED display value only, when (and only when) the type is `TypeKind::String { .. }` AND the operator is one of `< > <= >=`. Every other type/operator returns the compared value unchanged.
- **Do NOT "simplify" this by reading the full field for the comparison too.** Verified against the real `file` binary (file-5.41): buffer `0.6.10` does NOT match `>0.6.1` because the compared prefix `0.6.1` equals the pattern (`file_strncmp` limited to `vallen=5`), even though the full string `0.6.10` sorts after `0.6.1`. A full-field comparison would spuriously match. Regression guard: `test_string_ordering_comparison_stays_prefix_limited`.
- **The full-field display read does not move the relative-offset anchor.** `bytes_consumed_with_pattern` re-derives the advance from the PATTERN (`pattern.len()`, +1 for a trailing NUL), never from the display value, so a `>>&N` child of a `string >\0` rule resolves to the same offset. Regression guard: `test_string_ordering_full_field_display_does_not_move_relative_anchor`.
- **On a display-side read error after a successful match, the compared value is returned** rather than propagating -- a matched rule must not abort on a display-only read.
- **Scope:** `TypeKind::String` only. `PString` (`read_pstring`) and `String16` (`read_string16`) already read their full field independent of `pattern.len()`. Equal/NotEqual display is left as the compared prefix (libmagic renders full-field there too, but that is unreported, rarer, and expands regression surface -- deferred).

**`%.Ns` precision** (`src/output/format.rs`): `Spec` now carries `precision: Option<usize>`, parsed from the `.<digits>` run (capped at `MAX_FORMAT_WIDTH`; a bare `.` is precision 0). It is honored **only for `%s`** (`Conv::Str`) via `render_str_spec`, which truncates the rendered string to at most `precision` **characters** (char-wise, not byte-wise: C's `%.Ns` is byte-wise, but our value is a Rust `String` and a byte cut could split a multi-byte UTF-8 sequence; identical for the ASCII version/name fields that use precision). Width padding is applied AFTER truncation, so `%4.4s`/`%-.4s` now render correctly (`%s` previously dropped width entirely). Numeric conversions still ignore precision. Regression guard: `test_string_precision_truncation`.

This is display-detail only -- the match DECISION is unchanged. The remaining `, ASCII text` suffix that real `file` appends (`XML 1.0 document text, ASCII text`) is combined magic + `file_ascmagic` text classification, a separate feature (see S13.3).

## 15. Offset Resolution at EOF

### 15.1 `offset == buffer.len()` Is a Valid Resolution Target (Width Enforced at Read Time)

`resolve_absolute_offset` (`src/evaluator/offset/absolute.rs`) permits `offset == buffer_len` -- the EOF position -- and rejects only `offset > buffer_len`. The positive branch uses `>` (not `>=`). This mirrors libmagic's structure: **offset resolution is permissive; each type read enforces its own width.** Do not "tighten" this back to `>=` -- it silently drops legitimate EOF-anchored children.

**Verified against real `file` (file-5.41)** with a custom magic file (`0 string XY PARENT` + `>2 byte x byte=%d` + `>2 string x [%s,`):

- On a 2-byte buffer (child offset 2 == EOF): `file` prints `PARENT [,` -- the **numeric** child (`byte x`) is **dropped** (its width-checked read has no bytes at EOF -> non-match), while the **`string x`** child renders an **empty** `%s`.
- On a 3-byte buffer (child offset 2 < EOF): both children render.

The motivating real-world case is LUKS: `luks:10` is `>8 string x [%s,`, and on a header truncated to exactly 8 bytes GNU `file` prints `LUKS encrypted file, ver 1 [,`. Before this fix, offset resolution rejected `offset == 8` and the trailing detail was silently dropped (rmagic printed only `LUKS encrypted file, ver 1`). The empty-field cascade continues at `>40`/`>72` for headers truncated exactly at those offsets (`[, ,`, `[, , ]`), matching `file` byte-for-byte.

**Why it is safe (blast radius):** the only two production callers are in `offset/mod.rs` (`Absolute(N)` and negative-`FromEnd`), and only the positive branch changed (`FromEnd` uses the negative branch, already `>`). At `offset == len`:

- **Fixed-width readers** (`byte`/`short`/`long`/`quad`/`float`/`double`/`date`/`qdate`) use bounds-safe `.get()` / `read_bytes_at` -> `BufferOverrun` -> non-match. Outcome is **unchanged** from before (the child was dropped either way).
- **`read_string`** (`src/evaluator/types/string.rs`) also relaxed its entry guard to `offset > buffer.len()`; at EOF `&buffer[len..]` is a valid empty slice -> `Value::String("")`. This is the ONLY reader whose *outcome* changes (empty render instead of drop).
- **`read_string16`** and **`read_pstring`** keep their `>=`/prefix-length guards, so they return `BufferOverrun` (non-match) at EOF -- they need at least one code unit / their length prefix, matching libmagic. Not relaxed (no observed rule needs it; YAGNI).
- **Anchor-advance** (`string_bytes_consumed`, GOTCHAS S3.8) uses `buffer.get(offset..)` and already returns `0` at `offset == len`, so the anchor lands exactly at `len` -- consistent with the 0-byte read. No divergent guard.

**Empty-buffer edge:** `resolve_absolute_offset(0, b"")` now returns `Ok(0)` (0 == 0). A top-level `0 string x` would therefore match an empty file with an empty string -- but **no system-DB rule uses a top-level `string`-family `x`** (verified by grep), and GNU `file` still classifies an empty file as `empty`. rmagic's ascmagic "empty" fallback (S13.3) is unaffected. A numeric top-level rule on an empty buffer still fails its width-checked read (non-match), so empty-file classification is unchanged.

Corpus differential (before/after, 394 small/truncated real files vs GNU `file`): exactly 2 rows changed (both truncated LUKS fixtures), both moving from a dropped detail to exact `file` parity. Zero regressions. The change is monotonic -- it can only *add* an empty string-child render at EOF, never remove or alter existing output. Regressions: `test_resolve_absolute_offset_at_eof_is_permitted` and the updated resolver tests (`absolute.rs`); `test_read_string_offset_at_buffer_end_is_empty_string` (`string.rs`); end-to-end `child_at_eof_numeric_dropped_string_renders_empty` and `luks_truncated_header_renders_empty_cipher_field` (`tests/parser_integration_tests.rs`).
