---
title: Multi-Agent PR Review Catches 25 Issues in TODO Cleanup
category: developer-experience
date: 2026-04-09
tags:
  - code-review
  - pr-review
  - documentation-accuracy
  - api-design
  - non-exhaustive
  - rust
  - configuration
  - error-handling
severity: high
components:
  - config.rs
  - main.rs
  - mime.rs
  - evaluator/mod.rs
  - evaluator/types/mod.rs
  - io/mod.rs
  - parser/grammar/mod.rs
  - GOTCHAS.md
  - docs/src/compatibility.md
---

# Multi-Agent PR Review Catches 25 Issues in TODO Cleanup

## Problem

After resolving 30 TODO items across 7 commits on branch `todo_cleanup` (PR #212, 51 files changed, 2727 insertions / 2295 deletions), the PR passed `just ci-check` (1120 tests, zero clippy warnings) but a comprehensive 6-agent parallel review identified 5 High, 9 Important, and 11 Suggestion-level issues that CI could not catch.

**Agents used:** code-reviewer, pr-test-analyzer, silent-failure-hunter, type-design-analyzer, comment-analyzer, CodeRabbit.

**Issue categories:** doc-code drift, broken rustdoc links, missing unit tests, swallowed errors, unnecessary allocations, inconsistent attribute application, overly broad API visibility, stale compatibility statuses, missing security documentation.

## Root Cause

CI validates compilation, linting, and test correctness but cannot detect:

- Documentation that references non-existent methods or wrong error types
- Line numbers in cross-file references that drift after refactoring
- Missing tests on new modules (config.rs had 307 lines, 4 validators, 0 tests)
- `if let Ok(...)` patterns silently swallowing filesystem errors
- Return types that allocate needlessly (`Option<String>` vs `Option<&'static str>`)
- Inconsistent `#[non_exhaustive]` application across public enums
- Internal methods exposed as `pub` when `pub(crate)` is appropriate

## Solution

All 12 distinct issues were fixed in a single commit:

### 1. Catch-all wildcard in error handler (main.rs)

`#[non_exhaustive]` requires a wildcard arm even within the defining crate. Restored `_ =>` with descriptive message (`"Error: {error}"`) instead of the misleading `"Unknown error: {error}"`.

### 2. Swallowed metadata errors (main.rs:399-412)

`if let Ok(metadata)` silently discarded permission/symlink errors. Fixed by propagating with `map_err` to produce `LibmagicError::IoError` with the magic file path for actionable diagnostics.

```rust
// Before (silent failure)
if let Ok(metadata) = std::fs::metadata(&magic_file_path) { ... }

// After (explicit error)
let metadata = std::fs::metadata(&magic_file_path).map_err(|e| {
    LibmagicError::IoError(std::io::Error::new(
        e.kind(),
        format!("Cannot access magic file {}: {e}", magic_file_path.display()),
    ))
})?;
```

### 3. Doc error type mismatch (config.rs:200)

Doc claimed `LibmagicError::InvalidFormat` but code returns `LibmagicError::ConfigError`. Updated doc to match implementation.

### 4. Broken rustdoc links (lib.rs, output/mod.rs)

Referenced non-existent `MagicDatabase::evaluate`. Updated to `evaluate_file` / `evaluate_buffer`.

### 5. Stale GOTCHAS.md line references

Removed hard-coded line numbers that drifted after config extraction. Referenced by module path only. Updated `src/lib.rs:386` to `src/config.rs`.

### 6. MIME mapper allocation (mime.rs)

Changed `get_mime_type` return from `Option<String>` to `Option<&'static str>`. Updated 40+ test assertions. Added `.map(String::from)` at the single callsite in lib.rs.

### 7. Missing config tests (config.rs)

Added 19 boundary tests covering all 4 validators: recursion depth (0, 1, 1000, 1001), string length (0, 1, 1M, 1M+1), timeout (None, 0, 1, 300K, 300K+1), resource combination (4 corners), and `evaluate_rules_with_config` rejection of invalid configs.

### 8. Overly broad visibility (evaluator/mod.rs)

Downgraded `increment_recursion_depth` and `decrement_recursion_depth` from `pub` to `pub(crate)`. `RecursionGuard` is the intended public mechanism.

### 9. Missing `#[non_exhaustive]` (evaluator/types/mod.rs)

Added to `TypeReadError` for consistency with other public error enums.

### 10. Stale compatibility statuses (docs/src/compatibility.md)

Updated hierarchical rules, indirect offsets, date/time, and float/double from "Planned" / "In Progress" to "Complete".

### 11. Missing security documentation (io/mod.rs)

Added `# Security` section to `from_path_and_metadata` documenting the deliberate canonicalization skip and recommending alternatives for adversarial environments.

### 12. Unsafe raw slice (parser/grammar/mod.rs)

Changed `input[1..]` to `input.get(1..).is_some_and(...)` for consistency with the safe `.get()` pattern used elsewhere.

## Prevention

### Documentation must be symbol-based, not line-based

Line numbers in cross-file references break on every refactoring. Reference by module path and symbol name instead.

### New modules need tests from day one

Any module with validators or config builders should have boundary tests for all numeric limits (zero, one, max, max+1) before merge.

### Apply `#[non_exhaustive]` consistently

For pre-1.0 libraries, all public enums likely to grow should have `#[non_exhaustive]`. Audit for consistency when adding the attribute to any type.

### Propagate errors explicitly

Avoid `if let Ok(...)` for operations that can fail meaningfully. Use `?` or explicit `match` with error context. Document intentional suppression with a comment explaining why.

### Prefer `&'static str` over `String` for static data

Lookup tables backed by static data should return references, not owned strings. Convert to `String` only at the boundary where ownership is needed.

### Multi-agent review catches what CI cannot

Run parallel review agents (code, test, silent-failure, type-design, comment) on large PRs before merge. CI validates correctness; review validates design.

## Related

- GOTCHAS.md S4.4 (parallel EvaluationResult types)
- GOTCHAS.md S9.1 (error return path cleanup)
- GOTCHAS.md S13.1 (EvaluationConfig::default() has no timeout)
- PR #212: the TODO cleanup PR that triggered this review
