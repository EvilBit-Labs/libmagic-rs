---
name: verification-loop
description: Comprehensive verification for Rust projects. Runs cargo check, clippy, fmt, tests, coverage, and security audit in sequence.
---

# Verification Loop (Rust)

> The canonical entry point in this project is `mise exec -- just
> ci-check`. It runs the same checks pre-commit and CI run, in the right
> order. The per-phase breakdown below is for when you want finer-grained
> feedback (e.g., during refactoring you want only clippy + tests, not
> the whole suite).

## When to Use

- After completing a feature or significant code change
- Before creating a PR
- After refactoring
- When you want to ensure all quality gates pass

## Canonical command

```bash
mise exec -- just ci-check
```

This runs format, clippy, build, tests, mdformat, shellcheck,
actionlint, cargo-machete, cargo-audit, and Cargo.toml sort. If it
exits 0, you're green.

## Per-Phase Breakdown

### Phase 1: Build Verification

```bash
mise exec -- cargo check 2>&1 | tail -20
```

If build fails, STOP and fix before continuing.

### Phase 2: Format Check

```bash
mise exec -- cargo fmt -- --check 2>&1 | head -20
```

If formatting issues are found, run `mise exec -- cargo fmt` to fix.

### Phase 3: Lint Check

```bash
mise exec -- cargo clippy --all-targets -- -D warnings 2>&1 | head -30
```

All clippy warnings are errors in this project (`warnings = "deny"` in
workspace lints).

### Phase 4: Test Suite

```bash
# Run all tests with nextest (does not run doc tests).
mise exec -- cargo nextest run 2>&1 | tail -50

# Doc tests separately.
mise exec -- cargo test --doc 2>&1 | tail -20
```

Report:

- Total tests
- Passed
- Failed

### Phase 5: Coverage Check

```bash
mise exec -- cargo llvm-cov --summary-only 2>&1 | tail -10
```

Target: 85%+ coverage per AGENTS.md. If below threshold, identify
uncovered code.

### Phase 6: Security Audit

```bash
mise exec -- cargo audit 2>&1 | tail -20
mise exec -- cargo deny check 2>&1 | tail -20
```

`cargo deny check` uses the project's `deny.toml` -- do not pass a
custom config path.

### Phase 7: Diff Review

```bash
git diff --stat
git diff HEAD --name-only
```

Review each changed file for:

- Unintended changes
- Missing error handling (`.unwrap()`, direct indexing in non-test code)
- Files exceeding the 500--600 line guideline (800 max)
- Missing tests for new code paths

## Output Format

After running all phases, produce a verification report:

```
VERIFICATION REPORT
==================

Build:     [PASS/FAIL]
Format:    [PASS/FAIL]
Clippy:    [PASS/FAIL] (X warnings)
Tests:     [PASS/FAIL] (X/Y passed)
Doc Tests: [PASS/FAIL]
Coverage:  [X%] (target: 85%)
Audit:     [PASS/FAIL] (X advisories)
Deny:      [PASS/FAIL]
Diff:      [X files changed]

Overall:   [READY/NOT READY] for PR

Issues to Fix:
1. ...
2. ...
```
