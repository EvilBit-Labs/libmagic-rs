---
title: Magic-file string rules silently failed -- escape parsing + NUL truncation + cross-type compare
date: 2026-04-25
category: logic-errors
module: parser/grammar + evaluator/types/string + evaluator/operators/equality
problem_type: logic_error
component: tooling
severity: critical
symptoms:
  - Magic rules parse successfully but never match real on-disk bytes (rmagic returns "data" for buffers GNU file identifies correctly)
  - Bareword string values containing escape sequences like \0, \n, \xNN are stored as literal token text instead of decoded bytes
  - read_string truncates the buffer read at the first NUL even when the comparison value itself contains a trailing NUL
  - ELF detection silently fails because parser produces Value::Bytes for \177ELF but read_string_exact returns Value::String, and apply_equal returned false on cross-type comparisons
  - Synthetic-fixture conformance against GNU file was 0/5 before the fix (Norton, Squashfs LE, Squashfs BE, ELF all failed)
root_cause: logic_error
resolution_type: code_fix
related_components:
  - parser-grammar
  - evaluator-types-string
  - evaluator-operators-equality
tags:
  - magic-file-parser
  - string-comparison
  - nul-truncation
  - escape-sequences
  - value-coercion
---

# Magic-file string rules silently failed -- escape parsing + NUL truncation + cross-type compare

## Problem

Three intertwined bugs in libmagic-rs caused every magic-file string rule containing backslash escapes (`\0`, `\177ELF`, etc.) to silently fail to match real on-disk files, even after the magic file parsed successfully without errors. The bugs spanned the parser (escape interpretation), evaluator (NUL-truncated reads), and operator (strict-type equality) layers, conspiring to make `rmagic --magic-file /usr/share/file/magic/filesystems` return `"data"` for files that GNU `file` correctly identified.

## Symptoms

- `rmagic --magic-file just-norton.magic /tmp/norton` returned `"data"` with exit code 0 for a buffer (`PNCIHISK\x00`) that exactly matched the rule `0 string PNCIHISK\0 Norton Utilities disc image data`. Same magic file fed to GNU `file` correctly returned the Norton description.
- ELF binaries (`\x7fELF...`) returned `"data"` instead of `"ELF 64-bit ..."` even though the parser was successfully ingesting the `0 string \177ELF` rule from `/usr/share/file/magic/elf`.
- After ~30 prior parser fixes (commits `d805d4d`, `e90265d`, `4c4b46b`) made the entire `/usr/share/file/magic/` directory load without errors, the loader claimed success but **zero** real-file rules actually matched. The system silently degraded to "everything is `data`".
- The parser's `Value::String("PNCIHISK\\0")` (10 chars: `P`, `N`, `C`, `I`, `H`, `I`, `S`, `K`, `\`, `0`) had length 10 in the AST, while the buffer-side read of `PNCIHISK\x00` (9 bytes) returned an 8-byte string (`"PNCIHISK"`, NUL-truncated). Lengths mismatched, equality always returned false.
- For the ELF case, the parser produced `Value::Bytes([0x7f, 0x45, 0x4c, 0x46])` via `parse_mixed_hex_ascii`, while the evaluator's `read_string` produced `Value::String("\x7fELF")`. Even when both contained the same 4 bytes, `apply_equal` returned `false` due to strict variant matching.
- No error messages, no warnings, no logs at any level. The first signal of failure was the user reporting "it doesn't actually do anything correctly with it."

## What Didn't Work

- **Trusting parse success as functional success.** After shipping the previous commits that resolved every parse error in the GNU magic corpus, the working assumption was that the loader was "done" -- the magic file loaded, the rules tree was built, the evaluator ran without panicking. Only end-to-end testing against real binaries with `rmagic` vs `file` revealed that none of those parsed rules were actually firing.
- **Hypothesizing the wrong layer first.** The initial diagnosis blamed `apply_operator` / `apply_equal` for over-strict comparison. A side-channel test that printed the parsed `MagicRule.value` showed `String("PNCIHISK\\0")` -- the literal 10-character sequence with `\` and `0` as separate chars -- disproving the operator-first theory. The bug was upstream in the parser; `parse_bare_string_value` in `src/parser/grammar/mod.rs` was using `take_while` to grab the raw token without ever processing escape sequences.
- **Fixing only the parser side.** After making `parse_bare_string_value` walk character-by-character and interpret `\0`, `\xNN`, and `\NNN` escapes, the AST correctly stored 9 bytes ending in NUL -- but Norton **still** returned `"data"`. The buffer-side `read_string` in `src/evaluator/types/string.rs` was using `memchr::memchr(0, remaining_buffer)` to find the first NUL and truncate there, returning only the first 8 bytes. The compared lengths were 9 (expected) vs 8 (read), so equality failed by length-mismatch before even comparing bytes.
- **Assuming `read_string` had a single semantic.** The first attempt to fix the truncation tried to remove the NUL-stop behavior outright. That broke the `string x` (any-value) scan path, which legitimately wants to read up to the first NUL or end-of-buffer when no comparison pattern is given. The correct fix needed two semantics: exact-length read when a pattern is supplied (whose length defines the comparison window), and printable-prefix scan when no pattern is supplied.
- **Not anticipating cross-variant `Value` types.** Once Norton matched, ELF still didn't. The parser dispatches `\177ELF` through `parse_mixed_hex_ascii` (because the leading `\1` could begin a `\xNN` hex byte), which emits `Value::Bytes([0x7f, 0x45, 0x4c, 0x46])`, while bareword `PNCIHISK\0` goes through `parse_bare_string_value` and emits `Value::String`. The new `read_string_exact` always returns `Value::String`. `apply_equal` was delegating to `compare_values` which is type-strict -- `Value::Bytes` vs `Value::String` returned `None` from `partial_cmp`, never `Some(Equal)`.
- **Missing the recurring "format-modify-then-edit" friction during the fix.** While landing the three changes, four `cargo fmt` reformatting passes between the skeleton read and the subsequent edit caused "file modified since read" tool errors and one stale `old_string` failure (session history). This pattern is documented in [GOTCHAS.md](../../../GOTCHAS.md) S8.3; the recurrence here suggests the gotcha is undertreated. (session history)

## Solution

The fix landed in three coordinated changes spanning parser, evaluator types, and operator logic. All three were required together -- a fix to any one alone left the other two failures undiagnosed.

### Fix 1: Interpret escapes in bareword string values

`src/parser/grammar/mod.rs::parse_bare_string_value` was walking the input with `take_while` and storing the literal token. The new version walks character-by-character, dispatches `\xNN` hex escapes via `value::parse_hex_byte_with_prefix` (which `parse_escape_sequence` does NOT recognize), then octal/control escapes via `value::parse_escape_sequence`, falling through to a literal backslash if neither matches:

```rust
fn parse_bare_string_value(input: &str) -> IResult<&str, Value> {
    // ... whitespace handling elided ...
    let mut bytes: Vec<u8> = Vec::new();
    let mut remaining = input;
    while let Some(ch) = remaining.chars().next() {
        if ch.is_whitespace() || ch == '\n' || ch == '\r' { break; }
        if ch == '\\' {
            // \xNN hex first -- parse_escape_sequence doesn't handle it
            if let Ok((rest, b)) = value::parse_hex_byte_with_prefix(remaining) {
                bytes.push(b);
                remaining = rest;
                continue;
            }
            // Octal / control escapes (\0, \n, \t, \177, etc.)
            if let Ok((rest, esc)) = value::parse_escape_sequence(remaining) {
                let code = esc as u32;
                if let Ok(byte) = u8::try_from(code) {
                    bytes.push(byte);
                } else {
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(esc.encode_utf8(&mut buf).as_bytes());
                }
                remaining = rest;
                continue;
            }
            // Lone `\` not followed by recognised escape: literal backslash
            bytes.push(b'\\');
            remaining = &remaining[1..];
            continue;
        }
        // Plain UTF-8 character
        let mut buf = [0u8; 4];
        bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        remaining = &remaining[ch.len_utf8()..];
    }
    if bytes.is_empty() { return Err(...); }
    let value = String::from_utf8_lossy(&bytes).into_owned();
    Ok((remaining, Value::String(value)))
}
```

After this fix, `0 string PNCIHISK\0 ...` parses to `Value::String` of length 9, with byte 8 being `0x00`.

### Fix 2: Add `read_string_exact` and route pattern-supplied reads through it

The existing `read_string` is correct for `string x` (any-value scans where we want the printable prefix), but wrong when a comparison pattern is supplied. The new helper in `src/evaluator/types/string.rs`:

```rust
pub fn read_string_exact(
    buffer: &[u8],
    offset: usize,
    length: usize,
) -> Result<Value, TypeReadError> {
    let end = offset.checked_add(length).ok_or(TypeReadError::BufferOverrun {...})?;
    let slice = buffer.get(offset..end).ok_or(TypeReadError::BufferOverrun {...})?;
    Ok(Value::String(bytes_to_string_fast(slice)))
}
```

Dispatch in `src/evaluator/types/mod.rs` selects between the two based on whether a pattern is supplied and what its length is:

```rust
TypeKind::String { max_length } => {
    match (max_length, pattern) {
        (Some(n), _) => read_string_exact(buffer, offset, *n),
        (None, Some(Value::String(p))) => read_string_exact(buffer, offset, p.len()),
        (None, Some(Value::Bytes(b))) => read_string_exact(buffer, offset, b.len()),
        (None, _) => read_string(buffer, offset, None),  // any-value scan path
    }
}
```

This preserves the original `read_string` semantics for the `x` operator (where there's no pattern) while making pattern-comparison reads exact-length, NUL-tolerant.

### Fix 3: Cross-type equality between `Value::String` and `Value::Bytes`

`src/evaluator/operators/equality.rs::apply_equal` was delegating to `compare_values`, which only returns `Some(Ordering::Equal)` when both operands share a `Value` variant. The fix adds a libmagic-compatible cross-variant byte comparison before falling through:

```rust
pub fn apply_equal(left: &Value, right: &Value) -> bool {
    if let (Value::Float(a), Value::Float(b)) = (left, right) {
        return floats_equal(*a, *b);
    }
    // libmagic-compatible cross-type equality: parser produces Value::Bytes
    // for `\177ELF` via parse_mixed_hex_ascii, evaluator returns Value::String.
    // Compare by raw byte sequence so DSL surface semantics ignore the internal split.
    match (left, right) {
        (Value::String(s), Value::Bytes(b)) | (Value::Bytes(b), Value::String(s)) => {
            return s.as_bytes() == b.as_slice();
        }
        _ => {}
    }
    compare_values(left, right) == Some(Ordering::Equal)
}
```

### Test updates

Four pre-existing tests pinned the old strict-type behavior and were updated to the new libmagic-compatible policy:

- `test_apply_equal_bytes_vs_string` (in `src/evaluator/operators/equality.rs`)
- `test_apply_equal_edge_cases` (same file)
- `test_apply_operator_edge_cases` (in `src/evaluator/operators/mod.rs`)
- `test_parse_magic_rule_meta_name_use_reject_malformed_identifiers` (regression for the new escape-interpreting parser path, since `\` characters in identifier-rejection fixtures now parse differently)

All 1148 lib tests pass. `just ci-check` green.

## Why This Works

The three bugs formed a chain where each layer's contract assumed the others were correct, and all three contracts were wrong. The DSL surface promise is "byte-for-byte exact equality on the first N bytes of the buffer against the rule's pattern" -- magic(5) `string PATTERN` semantics. To honor that promise, three things must all hold simultaneously:

1. **The pattern stored in the AST must equal the byte sequence the magic-file author wrote**, including NULs encoded as `\0`, control bytes encoded as `\NNN` octal, and arbitrary bytes encoded as `\xNN` hex. The old `parse_bare_string_value` was treating the input as opaque text, leaving `\0` as two characters. Fix 1 makes the parser honor the same escape grammar that quoted strings already used.

2. **The buffer-side read must produce exactly `len(pattern)` bytes**, regardless of what those bytes are. The old `read_string` was scanning for a NUL terminator because that's correct for the `x` (any-value) operator, where the "value read" is meant for display formatting and should be a printable prefix. But for pattern comparison, NUL is just another byte -- the read length is dictated by the pattern, not by buffer content. Fix 2 splits the two semantics into `read_string` (NUL-stopping scan) and `read_string_exact` (length-determined slice), and routes pattern-comparison reads through the latter.

3. **Equality must compare bytes, not Value-discriminants.** The DSL doesn't distinguish "this rule's value was written as `PNCIHISK\0` (parsed to Bytes)" from "this rule's value was written as `\"PNCIHISK\\0\"` (parsed to String)." From the user's perspective, both should match a buffer containing those 9 bytes. The internal split between `Value::Bytes` and `Value::String` is an artifact of which parser path the source text happened to take (`parse_mixed_hex_ascii` for inputs starting with hex-looking escapes vs `parse_bare_string_value` for plain barewords). Fix 3 makes `apply_equal` look through that internal distinction.

The bugs went undetected because each layer had unit tests that verified its **local** contract, and those local contracts were internally consistent. The parser tests verified `parse_bare_string_value("PNCIHISK\\0")` returned `Value::String("PNCIHISK\\0")` -- which it did. The evaluator tests verified `read_string` stopped at NUL -- which it did. The operator tests verified `apply_equal(Bytes, String)` returned `false` -- which it did. Every test passed because every test was checking the wrong thing. End-to-end conformance against GNU `file` would have caught all three immediately.

The Norton failure (`string PNCIHISK\0`) exercised bugs 1 and 2; the ELF failure (`string \177ELF`) exercised bugs 1 and 3 (no NUL in the ELF magic, so bug 2 doesn't fire). A naive fix to either bug alone would have left the other two failures undiagnosed.

### Historical context (session history)

The latent gaps had distinct origin stories that map onto each fix:

- `parse_bare_string_value` was introduced during the `39-parser-implement-regex-and-search-types` work as a narrowly-scoped fallback for unquoted ASCII identifiers like `MZ` and `ABC` (GOTCHAS S3.6). The third-party fixtures in scope at the time (`third_party/tests/searchbug.magic`) did not use escape sequences, so escape interpretation was YAGNI'd out -- not a deliberate exclusion, just out-of-scope for the immediate problem. (session history)
- `read_string`'s NUL-stop behavior was deliberate and correct for the original quoted-string path (`parse_value` already decoded escapes into `Value::String` and the comparison was C-string-style). The combination of bareword + NUL-truncation only became broken when bareword values started containing real NULs after Fix 1 -- which was after the fact. (session history)
- `Value::Bytes` exists specifically because `parse_mixed_hex_ascii` interleaves raw hex bytes with ASCII text that cannot round-trip through `String`. `apply_equal`'s strict-variant assumption was never explicitly designed; it's the default behavior of a match arm that simply had no `(Value::Bytes, Value::String)` case. (session history)

This isn't a story about poor decisions -- it's a story about local correctness drifting away from end-to-end correctness when the test corpus expanded.

## Prevention

- **Add a GNU `file` conformance harness to the test suite.** The session built `/tmp/conformance_focused.sh` which runs `rmagic --magic-file <fixture> <buffer>` and `file --magic-file <fixture> <buffer>` against synthetic fixtures (Norton with NUL, ELF with `\177`, Squashfs little/big-endian, etc.) and asserts the output prefixes match. Promote it to `tests/system_magic_conformance.sh` (or a `#[test]` shelling out to the system `file` when present). This catches the entire class of "rule loads but doesn't match" silent failures, not just the three bugs fixed here.

- **For any DSL value type, write a property test that round-trips parser-output through evaluator-input.** For each `(rule_text, matching_buffer)` pair, assert that parsing the rule and evaluating it against the buffer produces a match. The Norton bug would have failed this immediately: `("0 string PNCIHISK\\0 desc", b"PNCIHISK\x00")` parses but doesn't match. Property-test the inverse (`(rule_text, non_matching_buffer)` produces no match) too, to guard against the new `read_string_exact` over-matching.

- **Document the dual semantics of `string` reads in `GOTCHAS.md`.** Add a section to S6 (String & PString Types) explicitly noting: "`read_string` is for `string x` any-value scans (NUL-stops at printable boundary); `read_string_exact` is for pattern comparison (exact-length slice). Adding a new code path that needs string bytes must pick consciously." This prevents the next contributor from re-introducing the NUL-truncation bug when adding (e.g.) wide-string or case-folded variants.

- **Document cross-Value equality as policy, not accident.** Add a section to S2.3 (`Value` Exhaustive Matches) noting that `apply_equal` deliberately compares `Value::String` and `Value::Bytes` by underlying byte sequence -- this is libmagic-compatible behavior and any new `Value` variant carrying byte data should extend this cross-equality, not lock into strict-variant comparison. Otherwise the next contributor adding `Value::Cstr` or similar will reintroduce the ELF mismatch.

- **Treat "parser corpus loads cleanly" as necessary but not sufficient.** The pre-existing CI gate "every file in `/usr/share/file/magic/` parses without error" gave false confidence here. The next gate to add: "every rule loaded from the corpus, when fed its own canonical sample fixture, produces non-`data` output." Even a small fixture set (one per major format family -- ELF, Mach-O, ZIP, PNG, JPEG, ext4, squashfs) would have caught the `data`-for-everything regression.

- **Investigate the parser-emits-`Value::Bytes` path more carefully.** `parse_mixed_hex_ascii` exists because some byte sequences (`\xff\xfe` BOMs, `\177ELF`, etc.) can't be losslessly stored as Rust `String` (UTF-8 invalid byte sequences). The choice of `Value::Bytes` for these is technically correct, but it forces every comparison/coercion path to handle the cross-variant case. Consider whether `Value::String` should be replaced or augmented with a byte-string variant uniformly, so the parser emits a single value type for all string-family rules and the operator/evaluator layers don't need cross-variant fallbacks. This is a v0.6.0 surface change (per the `Value` pattern refactor mentioned in `AGENTS.md`).

- **Add an end-to-end regression fixture for the bareword-escape path.** `tests/fixtures/bareword_escapes.magic` containing `0 string \0\0\0\0 four-nuls`, `0 string \xff\xfe utf16-le-bom`, `0 string \177ELF elf-magic` plus matching buffers, asserted via `MagicDatabase::evaluate_buffer`. This freezes the contract that all three escape forms (octal, hex, NUL) survive the parser and produce matches. Without it, a future "optimization" to `parse_bare_string_value` could reintroduce the literal-token bug silently.

## Related

- [`docs/solutions/integration-issues/implementing-variable-width-typekind-variant.md`](../integration-issues/implementing-variable-width-typekind-variant.md) -- introduced the `read_typed_value_with_pattern` dispatch architecture that this fix extends; shares the "structured `Option` vs. empty-string sentinel" prevention rule.
- [`docs/solutions/security-issues/pstring-anchor-poisoning.md`](../security-issues/pstring-anchor-poisoning.md) -- codifies the "keep dual-purpose helpers in sync" rule. After this fix, that rule extends to the `read_string` \<-> `read_string_exact` \<-> `string_bytes_consumed` triad, not just `read_pstring` \<-> `pstring_bytes_consumed`.
- [`GOTCHAS.md`](../../../GOTCHAS.md) S2.3 (`Value` Exhaustive Matches) -- the cross-type `Value::Bytes` vs `Value::String` equality policy added by Fix 3 is exactly the kind of cross-variant interaction that section warns about. Needs an addendum noting the policy.
- [`GOTCHAS.md`](../../../GOTCHAS.md) S6.4 -- currently states `read_string with max_length: None reads until the first NUL...` which is now incomplete; either add a forward-reference to `read_string_exact` and the pattern-bearing path, or update S6.4 to reflect the dispatcher's new branching.
- [`AGENTS.md`](../../../AGENTS.md) "Currently Implemented" / string section -- needs an update to mention escape sequence interpretation in bareword string values.
- GitHub #47 -- *Parser: report warnings for skipped invalid magic rules*: the loader work on the parent branch is the visibility counterpart to this fix's correctness work.
- GitHub #54 -- *Epic: Type System Expansion*: the string-family escape gap closed here logically rolls up.
- GitHub #106 -- *Evaluate and implement fuzzing tests*: the bareword-escape path is exactly the kind of input that adversarial fuzzing should exercise.
