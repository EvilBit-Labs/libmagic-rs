---
title: Pascal-string Anchor Poisoning via Attacker-Controlled Length Prefix
category: security-issues
date: 2026-04-07
tags: [evaluator, relative-offsets, pstring, bounds-check, attacker-input, anchor]
issue: '#38'
pr: '#211'
severity: medium
components: [evaluator/types/mod.rs, evaluator/engine/mod.rs]
---

# Pascal-string Anchor Poisoning via Attacker-Controlled Length Prefix

## Problem

When implementing the GNU `file` "previous match" anchor for `OffsetSpec::Relative` evaluation (issue #38), the helper that advances the anchor by the bytes a successful read consumed (`bytes_consumed`) read pstring length prefixes directly without bounding them against the actual buffer. A pstring rule with a 4-byte length prefix near `u32::MAX` (e.g., `\xFF\xFF\xFF\xFF`) caused `bytes_consumed` to return ~4 GB, advancing the anchor far past `buffer.len()`. Every subsequent `Relative` rule then resolved to a target `>= buffer.len()` and was silently skipped via the engine's graceful-skip arm — no error, no log loud enough to surface in normal operation, just incomplete classification.

A crafted file could deliberately trigger this on the first matching pstring rule, suppressing all following type-refinement rules and forcing the engine to report only the broad parent match (e.g., classify a malicious script as "data" instead of "shell script with dangerous interpreter").

Caught by the security and adversarial reviewers in `ce:review` autofix mode (PR #211, finding SEC-001 / ADV-001) before merge.

## Symptoms

- After a pstring rule with a large 4-byte length prefix matches, all subsequent sibling/child rules using `OffsetSpec::Relative` silently fail to match.
- The match list returned to the caller is missing entries that would have classified the file more specifically.
- No panic, no error, no test failure on benign inputs — only adversarial/fuzz inputs trigger it.
- Debug logs contain `Skipping rule '<name>': BufferOverrun` for each suppressed rule, but the root cause (anchor saturation from a previous pstring) is invisible without correlating across log lines.

## What Didn't Work

- **Documenting the gap.** First instinct (for the related fixed-width-types case from the same review) was to tighten the rustdoc to scope the "infallible" claim to variable-width types only, leaving the fixed-width branch unguarded. The Copilot reviewer rejected this in the next round: "Consider adding a guard ... so the function matches its documented defensive behavior and can't advance the anchor when misused." Documentation is not a substitute for invariants — if the contract says infallible, the function should *be* infallible regardless of caller discipline.
- **Adding a `warn!` log on `saturating_add` overflow at the engine site.** Considered as a debugging aid. Rejected because it adds noise without preventing the underlying bug — the anchor would still be poisoned, the rules would still be skipped, the user would still see incomplete classification. Fix the cause, not the symptom.
- **Trusting `read_pstring`'s upstream bounds check.** `read_pstring` itself checks `string_end <= buffer.len()` and errors if the payload would extend beyond the buffer, so a successful read implies the actual payload fit. But `bytes_consumed` re-reads the raw length prefix from the buffer (rather than receiving the byte count from the read function) and didn't apply the same bound. The two functions had divergent contracts that the engine relied on being equivalent.

## Solution

Clamp the pstring payload length against the remaining buffer in `pstring_bytes_consumed`, mirroring `read_pstring`'s own bounds enforcement:

```rust
// src/evaluator/types/mod.rs

let payload_length = if length_includes_itself {
    match stored_length.checked_sub(width) {
        Some(n) => n,
        None => return 0,
    }
} else {
    stored_length
};

// Clamp against remaining buffer bytes after the prefix. This defends
// against an attacker-controlled length prefix that exceeds the remaining
// buffer: read_pstring would have failed to actually read a payload that
// long, so a successful read implies the payload fit in the buffer.
// Mirroring that bound here keeps the anchor truthful.
let remaining_after_prefix = buffer.len().saturating_sub(prefix_end);
let bounded_payload = payload_length.min(remaining_after_prefix);
let actual_length = max_length.map_or(bounded_payload, |m| m.min(bounded_payload));
width.saturating_add(actual_length)
```

The same review pass also extended the *fixed-width* branch of `bytes_consumed` to bounds-check itself:

```rust
if let Some(bits) = type_kind.bit_width() {
    let width = (bits as usize) / 8;
    // Bounds-check the fixed-width path so a misuse cannot advance the
    // anchor past the buffer end. The engine guarantees a successful read
    // preceded the call, but the guard makes the contract self-consistent
    // for any future caller.
    return match offset.checked_add(width) {
        Some(end) if end <= buffer.len() => width,
        _ => 0,
    };
}
```

Regression tests pin both branches at `src/evaluator/types/tests.rs`:

- `test_bytes_consumed_pstring_clamps_oversized_prefix_be` — `\xFF\xFF\xFF\xFF` BE prefix on a buffer with 3 payload bytes → returns `4 + 3 = 7`, not `4 + u32::MAX`.
- `test_bytes_consumed_pstring_clamps_oversized_prefix_le` — same for LE prefix.
- `test_bytes_consumed_fixed_width_returns_zero_past_end` — fixed-width type at `offset == buf.len()`, beyond, and at `usize::MAX` overflow → all return 0.

## Why This Works

The fix restores an invariant the engine implicitly relied on: **the bytes the anchor advances by must equal the bytes the read function actually consumed from the buffer.** `read_pstring` already enforced `string_end <= buffer.len()`, so a successful read implies the payload fit in the remaining buffer. By applying the same bound in `bytes_consumed`, the helper becomes consistent with the read function under all inputs — including adversarial ones — without needing to plumb the byte count through the read function's return value.

The general principle: **when advancing internal state by an attacker-controlled byte count, clamp against the actual buffer reality, not the raw input.** Length prefixes, type-length-value structures, and any "this field is N bytes long" header field from untrusted input must all be bounded before being trusted.

## Prevention

- **Document the invariant.** `GOTCHAS.md` S3.8 now notes that "Pascal-string consumption is also clamped against the remaining buffer to prevent attacker-controlled length prefixes from poisoning the anchor to `usize::MAX`." Future contributors editing `bytes_consumed` see the constraint without needing to rediscover it from the security review.
- **Match read function consumption exactly.** If a helper function re-derives a value the corresponding read function already computed (here: payload length), make the helper apply *all* the same bounds checks — not a subset. Diverging contracts between two functions that the engine assumes are equivalent are a recurring class of subtle bugs.
- **Test with adversarial byte patterns.** Unit tests for variable-width type helpers should include `0xFF...FF` length prefixes, `usize::MAX` boundary cases, and `/J` flag underflow — not just typical inputs. The integration test suite should also exercise an attacker-controlled chain (e.g., a parent with an oversized prefix followed by a `Relative` child) end-to-end through `evaluate_rules` to confirm graceful skip rather than silent misclassification.
- **Treat `pub(crate)` boundaries as hints, not guarantees.** `bytes_consumed` was made `pub(crate)` in the same review pass, narrowing it to engine use only. But the visibility narrowing alone doesn't eliminate the bug — it only reduces the blast radius. Defensive bounds checking is still required because future internal callers may not respect the read-then-call invariant.
- **Keep dual-purpose helpers in sync.** When `read_pstring`/`read_string` change their bounds enforcement, `pstring_bytes_consumed`/`string_bytes_consumed` must change too. Add the file pair to `GOTCHAS.md` S2.1 (or similar) as a known coupling so refactors don't silently break the anchor.

## Related Issues

- Issue #38 — Evaluator: implement relative offset resolution
- PR #211 — feat(evaluator): implement relative offset resolution (#38)
- Related solution: [docs/solutions/logic-errors/indirect-offset-resolution.md](../logic-errors/indirect-offset-resolution.md) — sibling work that established the offset-resolver patterns this PR followed
- Related solution: [docs/solutions/logic-errors/indirect-offset-gnu-file-semantics.md](../logic-errors/indirect-offset-gnu-file-semantics.md) — same lesson about deriving expectations from GNU `file` source rather than from running the code
- `GOTCHAS.md` S3.8 — Relative Offsets: Anchor is Global-Monotonic, No Save/Restore (load-bearing context for any future contributor touching the evaluator's anchor)
