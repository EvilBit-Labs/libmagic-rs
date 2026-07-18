---
title: Loading the real system magic DB without fatal aborts -- getstr fidelity, raw-byte regex, and narrow graceful-skip
date: 2026-07-17
category: integration-issues
module: parser-grammar/getstr + evaluator-types/regex + evaluator-engine
problem_type: integration_issue
component: parser_evaluator_boundary
severity: high
applies_when:
  - loading a real-world third-party magic file / directory (e.g. /usr/share/file/magic) into the strict parser-evaluator
  - a bareword (unquoted) pattern value begins with a magic(5) escape sequence
  - compiling a user-supplied pattern for binary-safe byte matching via regex::bytes::Regex
  - deciding whether an evaluator capability gap should abort a whole file or skip one rule
  - a message-less gating rule can win a stop_at_first_match race against a more specific rule
  - implementing magic(5) bare `&MASK` relational tests
related_components:
  - parser-grammar
  - evaluator-types
  - evaluator-engine
  - evaluator-operators
  - output-formatting
tags:
  - libmagic-compatibility
  - getstr-escape-resolution
  - regex-bytes-unicode-false
  - graceful-degradation
  - stop-at-first-match
  - bitwise-and-semantics
  - verify-against-canonical-source
---

# Loading the real system magic DB without fatal aborts

## Context

Loading the macOS/Linux system magic database (`/usr/share/file/magic/`) into libmagic-rs fatally aborted with `TypeReadError::UnsupportedType { "regex without string pattern" }`. A single unparseable/unsupported rule aborts the **entire** file load (`parse_text_magic_file` is fail-fast, GOTCHAS S3.11) or the entire evaluation, so one escape-heavy `regex` rule took the whole DB down. The root cause was not one bug but a chain of four distinct correctness gaps between "a strict, internally-consistent parser+evaluator" and "the messy, real-world magic files GNU `file` actually ships." Each gap is individually reusable.

The driving example: the assembler magic file's `0 regex \^[\040\t]{0,50}\.asciiz assembler source text` -- an unquoted regex whose pattern begins with the magic(5) escape `\^`.

## Guidance

### 1. Bareword pattern values that begin with an escape are captured as `Value::Bytes`, not `Value::String`

`parse_value`'s `alt(...)` tries the hex/mixed-ASCII branch (`parse_mixed_hex_ascii`) **before** any string interpretation. A bareword regex pattern like `\^[\040\t]{0,50}\.asciiz` starts with `\`, so it was captured as `Value::Bytes` -- and the evaluator's `Regex` arm only accepted `Value::String`, so it fatally aborted. Two independent fixes were needed:

- **Parser (primary):** special-case `TypeKind::Regex` in `parse_magic_rule` *ahead of* the generic `is_string_family_type` bareword fallback, routing unquoted regex patterns through a dedicated getstr resolver (see #2) that produces a `Value::String`. Quoted regex values are unaffected (they already go through `parse_quoted_string`). If the resolver's own parse fails, fall back to `parse_value` so previously-working forms are untouched. (GOTCHAS S2.12)
- **Evaluator (floor-tier backstop):** make the `Regex` arms symmetric with `Search` -- accept **both** `Value::String` and `Value::Bytes` at all three call sites (`read_typed_value_with_pattern`, `read_pattern_match`, `bytes_consumed_with_pattern`), decoding `Bytes` via `String::from_utf8_lossy` with a `warn!` **only** when a real lossy substitution occurred (`str::from_utf8` actually fails). The warn is the visibility guard for a genuine desync: a lossy substitution puts U+FFFD in the compiled regex while the target buffer still holds raw bytes. (GOTCHAS S2.4)

The lesson: when a parser has an ordered `alt(...)` where an earlier branch can greedily capture a token the later branch was meant to own, the type-specific dispatch must run **before** the generic `alt`, not rely on it.

### 2. Replicate GNU `file`'s `getstr` escape table exactly -- verified against upstream, not inferred

The regex getstr resolver (`src/parser/grammar/getstr/mod.rs`) replicates `apprentice.c::getstr` **verified against the upstream C source**, including its quirks:

- Named control escapes `\a \b \f \n \r \t \v` -> their C control byte.
- `\0`-`\7` -> 1-3 digit **octal**, truncated to a byte.
- `\x` -> 0-2 digit **hex**; `\x` with *no* following hex digit is the literal byte `'x'` (0x78) -- getstr's quirky `default`, replicated for fidelity, not "fixed."
- **Every other escaped char drops the backslash and keeps the char literally** (`\^` -> `^`, a regex anchor). This is the *opposite* of `parse_bare_string_value` (non-regex string-family barewords), which *keeps* the backslash on an unknown escape.
- getstr does **not** special-case `\d`/`\s`/`\w` as PCRE classes; they fall into the "everything else" bucket and demote to bare `d`/`s`/`w`. **This is genuine upstream libmagic behavior, not a resolver bug** -- a reviewer will flag it; the answer is "yes, real `file` does this too."

The reusable meta-lesson: for compatibility work, escape-resolution quirks must be copied from the canonical implementation and each surprising one annotated with "verified upstream," or the next reviewer (or you, six months later) will "fix" a deliberate quirk back into a bug.

### 3. `regex::bytes::Regex` matches `\xHH` against the UTF-8 *encoding* unless you set `unicode(false)`

A resolved byte `>= 0x80` cannot be pushed onto a Rust `String` as a raw `char` (not a valid scalar value), so the getstr resolver re-encodes it as regex-native `\xHH` text (GOTCHAS S2.12, KTD3). **But** `regex::bytes::RegexBuilder` defaults to `unicode(true)`, under which the pattern `\xff` matches the *two-byte UTF-8 encoding* of U+00FF (`0xC3 0xBF`), **not** the raw byte `0xff`. For binary-safe byte matching against a file buffer you MUST call `.unicode(false)` on the builder (`build_regex` in `src/evaluator/types/regex.rs`). This is a silent correctness bug: the regex compiles fine and matches *some* inputs, so tests that only use ASCII patterns never catch it. Regression coverage must include a `>= 0x80` byte pattern matched against the raw byte.

### 4. Keep the graceful-skip predicate *narrow* -- an allowlist, never a variant-wildcard

To stop one bad rule from aborting the whole DB, the engine skips (as a non-match) exactly two classes at three catch sites: `is_missing_pattern_operand(type_name)` (an **exhaustive string allowlist** of `"regex without string pattern"`, `"search without string/bytes pattern"`, `"string with flags requires string/bytes pattern"`) and `is_regex_compile_failure(type_name)` (`starts_with("regex compile error:")`, which includes the CWE-1333 compile-size DoS guard). Everything else -- an unwired `TypeKind`, a non-equality operator on a pattern-bearing type, a `Meta` variant read as a value -- **still propagates fatally**. Do not widen the predicate to match on the error *variant* alone; that silently swallows genuine capability gaps this contract exists to surface. Log `debug!` for the ordinary missing-pattern case, `warn!` for a compile failure (so a pathological/malicious file's rejection stays visible). (GOTCHAS S2.1)

### 5. A message-less gating rule must not shadow a specific rule under `stop_at_first_match`

Once #1-#4 let the DB load and evaluate, a *second* class of bug surfaced: real magic files use message-less top-level rules purely as gating conditions for children (e.g. c-lang's `0 search/8192 "#include"`). Under `stop_at_first_match: true` (the CLI default), if such a gating rule matched a buffer on its own -- before a later, more specific rule was tried in strength order -- evaluation halted and the CLI printed a **blank description**. Fix: gate every `should_stop_at_first_match()` break site on `has_message_bearing_match` (the match, or a descendant, must carry real description text). The `default`/`clear` `sibling_matched` flag is **unaffected** -- a message-less match still counts as "a sibling matched"; only the early-exit is gated. Companion: a `classify_fallback` (`empty`/`ASCII text`/`UTF-8 Unicode text`/`data`, a strict subset of GNU `file`'s `file_ascmagic`) so a readable file never yields blank output. (GOTCHAS S13.2, S13.3)

### 6. magic(5) bare `&MASK` is "all bits set," not "any bit set" -- found by differential testing

While building the differential-parity test, a random binary blob spuriously matched an `AIX core file` rule (`4 belong &0x0feeddb0`, a ~17-bit bare mask). `apply_bitwise_and` implemented `(v & l) != 0` ("any masked bit set"); real libmagic's `magiccheck()` is `(v & l) == l` ("all masked bits set"). Verified by A/B testing against the real `file` binary. The fix is strictly stricter (removes false positives, never adds matches); single-bit flag rules -- the overwhelming majority of real bare-`&` usage -- are provably unaffected since the two interpretations coincide for a one-bit mask. **This is why differential parity tests must run against un-doctored inputs:** a fixture engineered to dodge a latent bug proves nothing. (GOTCHAS S13.3)

## Discovery mechanism

Gaps #1-#4 were the planned fix; #5 surfaced only when the DB actually evaluated end-to-end against real files (macOS host caught it, an integration test pinned it); #6 surfaced only when a differential-parity test was run against the *real* `file` binary with an un-doctored blob. The meta-lesson: **strict-vs-real-world integration bugs come in chains** -- fixing the load-abort merely exposes the next layer (blank output), which exposes the next (false-positive matches). Budget for the chain, and use differential testing against the canonical binary as the floor.

## Cross-references

- GOTCHAS S2.1 (narrow graceful-skip allowlist), S2.4 (Regex/Search Bytes symmetry), S2.12 (getstr resolver + `>= 0x80` re-encoding), S3.11 (fail-fast loader), S13.2 (message-bearing stop-at-first-match), S13.3 (text/data fallback + bitwise-AND)
- `src/parser/grammar/getstr/mod.rs`, `src/evaluator/types/regex.rs::build_regex`, `src/evaluator/engine/mod.rs` (graceful-skip + `has_message_bearing_match`), `src/output/ascmagic.rs`, `src/evaluator/operators/bitwise.rs`
- `tests/system_magic_dir.rs` (differential parity against GNU `file`), `tests/regex_getstr_fixtures.rs` (getstr + `unicode(false)` fixtures)
