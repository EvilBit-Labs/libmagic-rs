---
title: RAII scope guards for error-safe save/restore of evaluator context state
date: 2026-04-22
status: resolved
severity: high
category: logic-errors
problem_type: logic_error
root_cause: scope_issue
resolution_type: code_fix
components:
  - evaluator/engine
  - evaluator/mod
  - output/format
tags:
  - rust
  - evaluator
  - raii
  - drop-guard
  - error-safety
  - save-restore
  - meta-types
  - issue-42
issue: '#42'
pr: '#230'
branch: 42-parser-implement-default-clear-name-use-and-indirect-meta-types
applies_when:
  - Adding a new `EvaluationContext` field that needs scoped save/restore around a subroutine, indirect re-entry, or other nested evaluation
  - Introducing fallible (`?`-returning) operations inside a block that has previously mutated context state
  - Reviewing any `let saved_x = context.x(); ... context.set_x(new); ...?; ... context.set_x(saved_x);` sequence
  - Extending an existing RAII guard (`AnchorScope`, `RecursionGuard`) to cover additional fields
solution_files:
  - src/evaluator/engine/mod.rs
  - src/evaluator/mod.rs
  - src/output/format.rs
  - src/evaluator/offset/mod.rs
related_gotchas:
  - S2.1 TypeKind exhaustive-match discipline (analogous "every site must be updated" pattern)
  - S3.8 Relative offsets global-anchor discipline (the anchor field this guard restores)
  - S3.10 Subroutine base_offset (the second field this guard restores)
  - S14.2 Printf-style format specifiers (the adjacent UTF-8 byte-preservation fix)
---

# RAII scope guards for error-safe save/restore of evaluator context state

## Context

During the close-out of issue #42 (libmagic meta-type directives, PR #230), a post-commit code review pass surfaced three findings. The first two were real bugs with the same structural shape; the third was a false positive worth documenting because it recurred independently across two reviewers. All three surfaced only after `just ci-check` was green and the full 1348-test suite passed — they were invisible to the test harness because no existing test case exercised the trigger conditions.

The bug class at the heart of this learning is **manual save/restore of shared mutable state that silently becomes a no-op when `?` short-circuits the restore**. The codebase already had a working fix pattern (`AnchorScope` for `Indirect` dispatch), but it had not been applied to a newer code path that grew organically from one saved field to two. The learning is less "we shipped a bug" and more "when a RAII pattern exists, extending state requires extending the RAII guard, not adding a parallel manual save/restore pair."

## Symptoms

A future developer debugging this class of bug would see behavior that looks like non-determinism or misidentification of file types when `use` directives are present in magic rules:

- After a `RecursionLimitExceeded` or `Timeout` error returned from `evaluate_use_rule`, subsequent calls to `evaluate_rules` on the same `EvaluationContext` (without an intervening `context.reset()`) would resolve relative offsets against the use-site offset rather than the caller-level anchor. Rules using `&+N` / `&-N` would resolve at wrong file positions.
- Base-offset-biased rules inside the next evaluation would silently compute offsets relative to the stale use-site rather than zero (or the caller's correct base). This produced matches at wrong byte positions, missed matches, or `BufferOverrun` errors on otherwise-valid files.
- The corruption was intermittent — it only manifested when an error occurred during `use` rule evaluation, and only affected *subsequent* evaluations on a reused context. Tests that reset between calls, or that never exercised the timeout / recursion-limit paths on subroutine bodies, would not catch it.
- The `EvaluationContext::base_offset` doc comment **referenced a `BaseOffsetScope` RAII guard that did not exist in the codebase** — a ghost reference from a planned-but-not-implemented design. This is a symptom worth searching for directly: any `// ... restored via FooScope` comment where `FooScope` does not grep is a latent manual-restore bug.

## What Didn't Work: Manual Save/Restore

The original pattern in `evaluate_use_rule` saved the anchor and base offset at the top, modified both, then restored them at the bottom:

```rust
let saved_anchor = context.last_match_end();
let saved_base = context.base_offset();
context.set_last_match_end(absolute_offset);
context.set_base_offset(absolute_offset);

let subroutine_matches = {
    let mut guard = RecursionGuard::enter(context)?;   // <- can return Err
    evaluate_rules(&subroutine_rules, buffer, guard.context())?  // <- can return Err
};

context.set_last_match_end(saved_anchor);  // <- skipped on any Err above
context.set_base_offset(saved_base);       // <- skipped on any Err above
```

This passed review, passed `just ci-check`, and passed all 1348 tests — because no test had a `Use` rule whose subroutine body exceeded `max_recursion_depth` or exceeded the configured timeout. The error-path corruption was data-dependent on a condition the test corpus never triggered.

The reason the restore is bypassable is structural. Rust's `?` operator is syntactic sugar for early return on `Err`: execution jumps immediately out of the function's stack frame, skipping any remaining lines. The manual restore lines sit below the `?` operators, so they are unreachable on any error path. This is not a logic mistake — it is a fundamental property of using `?` without RAII.

The session history on this branch (session history) adds a telling detail: `AnchorScope` was introduced earlier in a prior session specifically to guard `MetaType::Indirect` dispatch, which needed to save and restore only `last_match_end`. `AnchorScope` was not reused for `Use` when that path was built. Later, when `base_offset` was added to `EvaluationContext` to implement subroutine-relative absolute offsets (magic(5) semantics — see GOTCHAS S3.10), its doc comment named a `BaseOffsetScope` RAII guard as the intended implementation. The actual implementation shipped a manual save/restore pair. The ghost guard name in the doc comment was written as future-tense design intent, and was never reconciled when the real guard (named `SubroutineScope`) was finally built during PR review. The two-field save/restore problem thus grew organically from a one-field fix with no re-examination of the pattern at each step.

## Solution: RAII Guard

Introduce `SubroutineScope<'a>` in `src/evaluator/engine/mod.rs` — a struct that holds a mutable reference to the context along with the two saved values, and restores both in its `Drop` implementation:

```rust
struct SubroutineScope<'a> {
    context: &'a mut EvaluationContext,
    saved_anchor: usize,
    saved_base: usize,
}

impl<'a> SubroutineScope<'a> {
    fn enter(context: &'a mut EvaluationContext, use_site: usize) -> Self {
        let saved_anchor = context.last_match_end();
        let saved_base = context.base_offset();
        context.set_last_match_end(use_site);
        context.set_base_offset(use_site);
        Self {
            context,
            saved_anchor,
            saved_base,
        }
    }

    fn context(&mut self) -> &mut EvaluationContext {
        self.context
    }
}

impl Drop for SubroutineScope<'_> {
    fn drop(&mut self) {
        self.context.set_last_match_end(self.saved_anchor);
        self.context.set_base_offset(self.saved_base);
    }
}
```

The call site becomes:

```rust
let subroutine_matches = {
    let mut scope = SubroutineScope::enter(context, absolute_offset);
    let mut guard = RecursionGuard::enter(scope.context())?;
    evaluate_rules(&subroutine_rules, buffer, guard.context())?
};
```

If `RecursionGuard::enter` returns `Err`, the `?` exits the block; `guard` is not yet constructed, but `scope` has been, and `scope` drops — restoring both fields. If `evaluate_rules` returns `Err`, `guard` drops first (decrementing recursion depth), then `scope` drops (restoring anchor and base). If both succeed, the block completes and the temporary bindings drop at the closing brace. In all three cases the restore happens.

## Why This Works

Rust's `Drop` trait is invoked unconditionally when a value goes out of scope, regardless of whether the exit is normal, an early return, a `?` propagation, or a panic unwind. (Panics in library code are separately forbidden by project policy — `unsafe_code = "forbid"` is a workspace lint — but the Drop guarantee holds regardless of the project-level rule.) This means the RAII guard eliminates the entire category of "forgot to restore" bugs: there is no code path through which the fields can be left modified, because the restore is tied to the object's lifetime rather than to a specific line of code. The compiler enforces that `scope` lives exactly as long as its enclosing block; it cannot be moved past the block, dropped early, or accidentally omitted by a refactor.

The same principle is why `RecursionGuard` was already implemented as RAII. Once the pattern is established for one piece of scoped state, each additional piece that participates in the same save/restore discipline needs to be either folded into an existing guard or given its own guard. The maintenance burden is not "write a Drop impl for each field" — it is "recognize the state-mutation pattern and reach for the existing tool."

## Prevention

The canonical smell is a three-part sequence: `let saved_x = context.x()`, followed by `context.set_x(new_value)` followed by any `?` operator (directly or via a nested block that returns via `?`), followed by `context.set_x(saved_x)` at a later position. When reviewing code for this pattern, the single checklist question is: **"Is mutable shared state modified before a fallible operation, and restored manually afterward?"** If the answer is yes, the code is already buggy or one refactor away from being buggy.

Process habits that catch this class:

1. **When adding a new field to a context type that participates in scoped evaluation** — where a callee should see a modified value but the caller must see the original — the first question is "does a RAII guard already exist for this kind of state?" If yes, the new field goes into the existing guard. If no, a new guard is built before the manual save/restore is written.
2. **Treat ghost references in doc comments as red flags.** If a doc comment names a type (`...restored via BaseOffsetScope`), grep for that type. If it does not exist, the doc was written as design intent and the implementation shipped the weaker form. Reconcile immediately.
3. **Asymmetry between neighboring save/restore sites is a planning failure.** `AnchorScope` (one field, RAII) and `evaluate_use_rule` (two fields, manual) are the same problem at different scales. Any time a reviewer sees one site using RAII and an adjacent site using manual save/restore, the question is whether the manual site is about to break or has already broken.

Mechanical detection is possible via a custom Semgrep rule matching the three-part sequence `let saved_$X = ...; ...; ...?; ...; ..set_$X(saved_$X)`. Clippy does not have a built-in lint for this shape. The project's `just ci-check` pipeline does not catch it either; detection relies on review judgment or targeted tests that exercise the error paths.

Direct regression guards added in PR #230 (`test_use_subroutine_absolute_offset_biased_by_use_site`, `test_use_subroutine_relative_offset_unaffected_by_use_site`) cover the happy path for `base_offset` propagation but intentionally do not exercise `RecursionLimitExceeded` inside a `Use` body, because that would require a fixture with deep nesting and would be fragile. A future reviewer who changes `SubroutineScope` should verify the Drop semantics manually or write a `max_recursion_depth = 1` regression test against a mutually-recursive `use` chain.

## Secondary Fix: Non-ASCII Template Bytes in `format_magic_message`

The same PR-review pass caught an independent bug in `src/output/format.rs::format_magic_message`. The original implementation iterated `template.as_bytes()` and pushed each non-`%` byte with `out.push(b as char)`. In Rust, casting a `u8` to `char` produces the Unicode scalar value with that code point — which for bytes in the range 0x80–0xFF yields Latin-1 characters rather than UTF-8 continuation bytes. A two-byte UTF-8 sequence like `é` (0xC3 0xA9) emitted as two separate Latin-1 characters (`Ã` and `©`), corrupting any template containing non-ASCII text.

The fix tracks a `plain_start` index and copies plain-text runs as string slices (`&template[plain_start..i]`) rather than byte by byte. This is safe because `%` is ASCII (0x25) and cannot appear as a UTF-8 continuation byte (which is always 0x80–0xBF), so scanning for `%` at byte granularity cannot split a multi-byte code point. The slice copy preserves the original UTF-8 byte sequences verbatim. A regression guard (`test_non_ascii_template_preserved`) pins the fix with `café`, `→ ok ←`, and `über`.

The relationship to the primary learning is the same: this bug shipped a unit-test-green green-on-`just ci-check` implementation that nothing in the test corpus could exercise. The fix pattern is structurally analogous — replace ad-hoc byte-by-byte work with a higher-level primitive (here, string slicing) that preserves the invariant by construction rather than by discipline.

## False Positive Postmortem: `AtomicBool::swap` Return Value

Two independent reviewers (the `correctness-reviewer` and the `silent-failure-hunter`) both flagged `USE_WITHOUT_RULE_ENV_WARNED.swap(true, Ordering::Relaxed)` as having inverted logic, and both recommended negating the condition. Both tracings were accurate up to the penultimate step and then concluded the opposite direction.

The code is:

```rust
if USE_WITHOUT_RULE_ENV_WARNED.swap(true, Ordering::Relaxed) {
    debug!("use directive '{name}' evaluated without a rule environment; no-op");
} else {
    warn!(
        "use directive '{name}' evaluated without a rule environment; treating as no-op (subsequent occurrences suppressed)"
    );
}
```

`swap` returns the **previous** value. On the first call the previous value is `false` (the `AtomicBool::new(false)` initialization), so the condition takes the `else` branch and emits the `warn!`. On subsequent calls the previous value is `true`, so the condition takes the `if` branch and emits the `debug!`. The code is correct — first call warns, subsequent calls debug.

The shared mental-model error among the reviewers was reading `swap(true)` as "set to true and return true." The session history shows this finding recurred multiple times across independent review passes. The lesson for future reviews is that `AtomicBool::swap` is subtly different from `fetch_or` or `compare_exchange` in how its return value relates to intent. When the intent is "do X only on the first call," the idiomatic reading is "was it already true before I set it?" — `false` means no (first call), `true` means yes (subsequent calls).

Writing the branch against a named variable makes the intent hard to misread:

```rust
let already_warned = USE_WITHOUT_RULE_ENV_WARNED.swap(true, Ordering::Relaxed);
if already_warned {
    debug!(...)
} else {
    warn!(...)
}
```

The production code preserves the inline form because it is correct, the comment above it documents the first-call-vs-subsequent semantics, and rewriting every subsequent review flag would be noise. But new uses of `AtomicBool::swap` as a once-guard in this codebase should prefer the named-variable form to eliminate the inversion risk at the source.

## Related

- [`integration-issues/meta-type-subroutine-dispatch-architecture.md`](../integration-issues/meta-type-subroutine-dispatch-architecture.md) — sibling doc covering the three-layer parse-time / `ParsedMagic` / optional `RuleEnvironment` pattern that this RAII guard sits inside. That doc describes the `use` dispatch's save/restore contract in prose but predates the `SubroutineScope` fix; after this learning ships, that doc should be updated to point readers at `SubroutineScope` as the canonical implementation.
- [`security-issues/pstring-anchor-poisoning.md`](../security-issues/pstring-anchor-poisoning.md) — a different failure mode of the same `EvaluationContext::last_match_end` field (attacker-controlled length prefixes poisoning the anchor). Shares the anchor-state-as-shared-mutable-concern framing.
- [`integration-issues/implementing-variable-width-typekind-variant.md`](../integration-issues/implementing-variable-width-typekind-variant.md) — `bytes_consumed` as the source of truth for advancing the anchor; the precondition that makes a corrupted anchor from a `?`-bypassed restore consequential.
- GOTCHAS.md S3.8 (relative-offset anchor discipline), S3.10 (subroutine base_offset semantics), S14.2 (printf format substitution — relevant to the non-ASCII template fix).
- GitHub: issue #42 (parent), PR #230 (where this landed).
