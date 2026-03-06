---
name: verification-loop
description: Comprehensive verification for Rust projects. Runs cargo check, clippy, fmt, tests, coverage, and security audit in sequence.
---

# Verification Loop (Rust)

## When to Use

- After completing a feature or significant code change
- Before creating a PR
- After refactoring
- When you want to ensure all quality gates pass

## Verification Phases

### Phase 1: Build Verification

```bash
cargo check 2>&1 | tail -20
```

If build fails, STOP and fix before continuing.

### Phase 2: Format Check

```bash
cargo fmt -- --check 2>&1 | head -20
```

If formatting issues found, run `cargo fmt` to fix.

### Phase 3: Lint Check

```bash
cargo clippy -- -D warnings 2>&1 | head -30
```

All clippy warnings are errors in this project. Fix before continuing.

### Phase 4: Test Suite

```bash
# Run all tests with nextest
cargo nextest run 2>&1 | tail -50

# Also run doc tests (nextest doesn't run these)
cargo test --doc 2>&1 | tail -20
```

Report:

- Total tests: X
- Passed: X
- Failed: X

### Phase 5: Coverage Check

```bash
cargo llvm-cov --summary-only 2>&1 | tail -10
```

Target: 85%+ coverage. If below threshold, identify uncovered code.

### Phase 6: Security Audit

```bash
cargo audit 2>&1 | tail -20
```

### Phase 7: Diff Review

```bash
git diff --stat
git diff HEAD --name-only
```

Review each changed file for:

- Unintended changes
- Missing error handling (`.unwrap()`, direct indexing)
- Files exceeding 500-600 line guideline
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
Diff:      [X files changed]

Overall:   [READY/NOT READY] for PR

Issues to Fix:
1. ...
2. ...
```

## Quick Verification (Pre-commit)

For fast checks during development:

```bash
just ci-check
```

This runs the project's standard CI validation suite.

## Full CI Verification

Matches what runs in GitHub Actions:

```bash
cargo fmt -- --check && cargo clippy -- -D warnings && cargo nextest run && cargo test --doc && cargo audit
```
