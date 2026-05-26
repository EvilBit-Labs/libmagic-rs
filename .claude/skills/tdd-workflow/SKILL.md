---
name: tdd-workflow
description: Enforces test-driven development for Rust. Write tests first with cargo test/nextest, implement to pass, refactor, verify >85% coverage with cargo llvm-cov.
---

# Test-Driven Development Workflow (Rust)

> Code samples below use placeholder types (`MagicRule`, `evaluate_rule`,
> `parse_magic_line`) for illustration. The current AST types are
> `#[non_exhaustive]` -- literal-construction from outside the crate
> won't compile. Construct test fixtures via crate-internal helpers or
> the public builder APIs. See [AGENTS.md](../../AGENTS.md) and
> [GOTCHAS.md](../../GOTCHAS.md) for authoritative project structure.

## When to Activate

- Writing new features or functionality
- Fixing bugs or issues
- Refactoring existing code
- Adding new magic rule types or operators
- Extending parser, evaluator, or output modules

## Core Principles

### 1. Tests BEFORE Code

Always write tests first, then implement code to make tests pass.

### 2. Coverage Requirements

- Minimum 85% line coverage per AGENTS.md
- All edge cases covered
- Error scenarios tested
- Boundary conditions verified
- Doc examples verified with `cargo test --doc`

### 3. Test Types

#### Unit Tests

- Inline `#[cfg(test)]` modules alongside source
- Individual functions, parsers, evaluators
- Pure logic and data transformations
- `.unwrap()` / `.expect()` are acceptable here (see
  `.claude/hookify.warn-panic-in-lib.md`)

#### Integration Tests

- In `tests/` directory with real magic files
- End-to-end rule parsing and evaluation
- CLI argument handling and output formatting

#### Property Tests

- Use `proptest` for fuzzing magic rule evaluation (`tests/property_tests.rs`)
- Random input generation for parser robustness
- Boundary value exploration

#### Benchmarks

- Use `criterion` for performance-critical code (`benches/*.rs`)
- Evaluator hot paths, parser throughput
- Memory-mapped I/O performance

## TDD Workflow Steps

### Step 1: Define the Behavior

```
Given [a magic rule with specific offset/type/operator],
When [evaluated against a file buffer with known contents],
Then [the evaluator returns the expected match result].
```

### Step 2: Write Failing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_feature_basic() {
        // Arrange -- construct via helpers, not literal struct exprs
        // (MagicRule and most AST types are #[non_exhaustive]).
        let rule = make_test_rule();
        let buffer: &[u8] = &[0x7f, 0x45, 0x4c, 0x46];

        // Act
        let result = evaluate_rule(&rule, buffer);

        // Assert -- .unwrap() is fine in tests; it's denied only in
        // library code per .claude/hookify.warn-panic-in-lib.md.
        assert_eq!(result.unwrap().description, "ELF");
    }

    #[test]
    fn test_new_feature_edge_case() {
        // Empty buffer must not panic
        let rule = make_test_rule();
        assert!(evaluate_rule(&rule, &[]).unwrap().is_none());
    }

    #[test]
    fn test_new_feature_error_case() {
        // Invalid offset returns error, not panic
        let rule = make_bad_offset_rule();
        assert!(evaluate_rule(&rule, &[0x00]).is_err());
    }
}
```

### Step 3: Run Tests (They Should Fail)

```bash
mise exec -- cargo nextest run -E 'test(test_new_feature)' --no-capture
```

### Step 4: Implement Code

Write minimal code to make tests pass. Follow project patterns:

- Use `.get()` for bounds-checked buffer access
- Return `Result<T, LibmagicError>` (or a more-specific variant)
- No `unsafe` -- workspace lint forbids it
- No `.unwrap()`, `.expect()`, or `panic!` in library code (test code is
  exempt)

### Step 5: Run Tests Again

```bash
mise exec -- cargo nextest run
```

### Step 6: Refactor

Improve code quality while keeping tests green:

- Remove duplication
- Improve naming
- Extract submodules if a file exceeds the 500--600-line guideline
- Ensure `cargo clippy -- -D warnings` is clean

### Step 7: Verify Coverage

```bash
mise exec -- cargo llvm-cov --html
# Open target/llvm-cov/html/index.html to inspect coverage
```

## Testing Patterns

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_input(input in ".*") {
        // Parser returns Ok or Err -- never panics
        let _ = parse_magic_line(&input);
    }

    #[test]
    fn evaluator_handles_any_buffer(
        buffer in prop::collection::vec(any::<u8>(), 0..1024)
    ) {
        let rule = make_test_rule();
        let _ = evaluate_rule(&rule, &buffer);
    }
}
```

### Table-Driven Tests

Prefer consolidating related cases into a single test with descriptive
failure messages, rather than one assertion per function (AGENTS.md
"Test style").

```rust
#[test]
fn test_endianness_variants() {
    let cases: &[(_, &[u8], u64)] = &[
        (Endianness::Big,    &[0x00, 0x01],  1),
        (Endianness::Little, &[0x01, 0x00],  1),
    ];

    for &(endian, buffer, expected) in cases {
        let actual = read_short(buffer, endian).unwrap();
        assert_eq!(actual, expected, "endian={endian:?} buffer={buffer:?}");
    }
}
```

## Test Organization

Authoritative module tree lives in AGENTS.md "Module Organization". The
parser and evaluator are both directory modules (`parser/grammar/`,
`evaluator/engine/`, `evaluator/types/`, `evaluator/operators/`,
`evaluator/offset/`), not flat files. Inline `#[cfg(test)] mod tests`
lives alongside the source in each submodule.

Integration tests live in `tests/*.rs` (e.g.,
`tests/compatibility_tests.rs`, `tests/integration_tests.rs`,
`tests/property_tests.rs`, `tests/cli_integration.rs`,
`tests/json_integration_test.rs`).

Benchmarks live in `benches/*.rs`.

## Common Testing Mistakes

### WRONG -- testing internal state

```rust
assert_eq!(parser.line_number, 5); // implementation detail
```

### CORRECT -- testing observable behavior

```rust
let rules = parse_magic_file(input).unwrap();
assert_eq!(rules.len(), 5);
assert_eq!(rules[0].message, "ELF");
```

### WRONG -- only happy path

```rust
let result = evaluate_rule(&rule, buffer).unwrap();
// .unwrap() is fine in tests, but if this is the only assertion,
// you have no coverage of the error path.
```

### CORRECT -- success and error paths both covered

```rust
assert!(evaluate_rule(&rule,     buffer).is_ok());
assert!(evaluate_rule(&rule,         &[]).is_ok()); // empty buffer
assert!(evaluate_rule(&bad_rule, buffer).is_err()); // malformed rule
```

## Quick Reference

```bash
# All tests via project's pinned toolchain
mise exec -- cargo nextest run

# Specific module tests
mise exec -- cargo test parser::grammar::tests

# With stdout visible
mise exec -- cargo test -- --nocapture

# Doc tests
mise exec -- cargo test --doc

# Coverage report
mise exec -- cargo llvm-cov --html

# Benchmarks
mise exec -- cargo bench

# Full pre-commit parity
mise exec -- just ci-check
```
