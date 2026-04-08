---
title: Rust Test Visibility Boundary — tests/ vs src/.../tests.rs
category: developer-experience
date: 2026-04-07
tags: [rust, testing, pub-crate, integration-tests, unit-tests, visibility]
issue: '#38'
pr: '#211'
severity: low
components: [testing]
---

# Rust Test Visibility Boundary — `tests/` vs `src/.../tests.rs`

## Context

When adding tests that need to exercise `pub(crate)` items directly — for example, injecting internal state via a crate-private setter to verify a graceful-skip contract — the test file location matters. The two test locations in a Rust crate have different visibility semantics, and this is not always obvious until you hit a compile error.

Encountered during PR #211 (relative offset evaluation) when adding a test that needed `EvaluationContext::set_last_match_end()` — a `pub(crate)` setter — to inject a near-saturation anchor value (`usize::MAX`) and verify that subsequent `OffsetSpec::Relative` rules skip gracefully without panicking. The initial instinct was to put the test in the existing integration test file at `tests/relative_offset_evaluation.rs`, but that failed to compile because `tests/` compiles as an external crate.

## Guidance

**Tests that need `pub(crate)` items must live in `src/.../tests.rs`, not in `tests/`.**

The two test locations in a Rust crate are:

| Location                                                                   | Compiles as    | Can access `pub(crate)` items?       | Can access `pub` items? |
| -------------------------------------------------------------------------- | -------------- | ------------------------------------ | ----------------------- |
| `tests/foo.rs` (integration tests)                                         | External crate | **No** — fails with E0603/E0624      | Yes                     |
| `src/foo/tests.rs` (module-adjacent unit tests, gated with `#[cfg(test)]`) | Same crate     | **Yes**                              | Yes                     |
| Doctests in rustdoc                                                        | External crate | **No** — same constraint as `tests/` | Yes                     |

This matches the project convention already used in `libmagic-rs`:

- `src/evaluator/tests.rs`, `src/evaluator/engine/tests.rs`, `src/evaluator/types/tests.rs` — unit tests in `#[cfg(test)] mod tests` peer files, full access to crate-private APIs
- `tests/evaluator_tests.rs`, `tests/relative_offset_evaluation.rs` — integration tests that only exercise the public API through `libmagic_rs::...` paths

## Why This Matters

Mixing the two up produces compile errors rather than runtime failures, so you find out quickly — but the error message (`error[E0603]: function 'set_last_match_end' is private`) doesn't immediately suggest moving the test to a different file. The instinct is often to widen visibility (change `pub(crate)` to `pub`), which defeats the point of the crate boundary and bloats the public API surface that gets locked in at semver time.

Picking the right location is also the right answer for **doctests**: rustdoc examples compile as external crates, so `/// use crate::...` never works in doctests on published items. Use `/// use libmagic_rs::...` (or the actual crate name) in doctests.

## When to Apply

- Writing a unit test that needs to inject internal state, call a `pub(crate)` constructor, or inspect a private field → put it in `src/.../tests.rs`
- Writing an integration test that exercises the crate's public API through the same entry points an external consumer would use → put it in `tests/`
- Writing a rustdoc example for a `pub` function → use the full external path (e.g., `libmagic_rs::evaluator::evaluate_rules`), not `crate::`
- Tempted to widen `pub(crate)` to `pub` just to write a test → **stop**, move the test to `src/.../tests.rs` instead

## Examples

**Wrong location — compile error:**

```rust
// tests/my_integration_test.rs
use libmagic_rs::evaluator::EvaluationContext;
use libmagic_rs::EvaluationConfig;

#[test]
fn test_anchor_injection() {
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX); // error[E0624]: method is private
    // ...
}
```

**Right location — compiles fine:**

```rust
// src/evaluator/engine/tests.rs
use super::*;
use crate::evaluator::EvaluationContext;
use crate::EvaluationConfig;

#[test]
fn test_evaluate_rules_anchor_near_saturation_skips_relative_child_gracefully() {
    let mut ctx = EvaluationContext::new(EvaluationConfig::default());
    ctx.set_last_match_end(usize::MAX); // Fine — same crate
    // ... assert evaluate_rules skips Relative rules gracefully
}
```

The second form compiled on the first try and now pins a safety contract that would be impossible to test from the integration layer.

## Related

- PR #211 — `test_evaluate_rules_anchor_near_saturation_skips_relative_child_gracefully` is the concrete case that prompted this learning
- `docs/solutions/security-issues/pstring-anchor-poisoning.md` — the security fix this test regression-guards
- Project GOTCHAS.md §7.1 — "Doctest Import Paths" (use `libmagic_rs::` not `crate::`) — same root cause, different surface
- AGENTS.md §4.1 — `evaluator::engine` is private; integration tests must import the re-exported `libmagic_rs::evaluator::evaluate_rules`, not `evaluator::engine::evaluate_rules` (related but distinct: module visibility, not test file visibility)
