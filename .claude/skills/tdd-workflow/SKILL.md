---
name: tdd-workflow
description: Enforces test-driven development for Rust. Write tests first with cargo test/nextest, implement to pass, refactor, verify >85% coverage with cargo llvm-cov.
---

# Test-Driven Development Workflow (Rust)

## When to Activate

- Writing new features or functionality
- Fixing bugs or issues
- Refactoring existing code
- Adding new magic rule types or operators
- Extending parser, evaluator, or output modules

## Core Principles

### 1. Tests BEFORE Code

ALWAYS write tests first, then implement code to make tests pass.

### 2. Coverage Requirements

- Minimum 85% coverage (project target per AGENTS.md)
- All edge cases covered
- Error scenarios tested
- Boundary conditions verified
- Doc examples verified with `cargo test --doc`

### 3. Test Types

#### Unit Tests

- Inline `#[cfg(test)]` modules alongside source
- Individual functions, parsers, evaluators
- Pure logic and data transformations

#### Integration Tests

- In `tests/` directory with real magic files
- End-to-end rule parsing and evaluation
- CLI argument handling and output formatting

#### Property Tests

- Use `proptest` for fuzzing magic rule evaluation
- Random input generation for parser robustness
- Boundary value exploration

#### Benchmarks

- Use `criterion` for performance-critical code
- Evaluator hot paths, parser throughput
- Memory-mapped I/O performance

## TDD Workflow Steps

### Step 1: Define the Behavior

```
Given [a magic rule with specific offset/type/operator],
When [evaluated against a file buffer with known contents],
Then [the evaluator should return the expected match result].
```

### Step 2: Write Failing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_feature_basic() {
        // Arrange
        let rule = MagicRule { /* ... */ };
        let buffer = &[0x7f, 0x45, 0x4c, 0x46];

        // Act
        let result = evaluate_rule(&rule, buffer);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().description, "ELF");
    }

    #[test]
    fn test_new_feature_edge_case() {
        // Empty buffer should not panic
        let rule = MagicRule { /* ... */ };
        let result = evaluate_rule(&rule, &[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_new_feature_error_case() {
        // Invalid offset should return error, not panic
        let rule = MagicRule { offset: OffsetSpec::Absolute(-1), /* ... */ };
        let result = evaluate_rule(&rule, &[0x00]);
        assert!(result.is_err());
    }
}
```

### Step 3: Run Tests (They Should Fail)

```bash
cargo test test_new_feature -- --nocapture
# Tests should fail -- we haven't implemented yet
```

### Step 4: Implement Code

Write minimal code to make tests pass. Follow project patterns:

- Use `.get()` for bounds-checked buffer access
- Return `Result<T, MagicError>` consistently
- No `unsafe`, no `.unwrap()`, no `panic!`

### Step 5: Run Tests Again

```bash
cargo nextest run
# All tests should pass
```

### Step 6: Refactor

Improve code quality while keeping tests green:

- Remove duplication
- Improve naming
- Extract modules if file exceeds 500 lines
- Ensure clippy compliance

### Step 7: Verify Coverage

```bash
cargo llvm-cov --html
# Verify 85%+ coverage achieved
# Open target/llvm-cov/html/index.html to inspect
```

## Testing Patterns

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_input(input in ".*") {
        // Parser should return Ok or Err, never panic
        let _ = parse_magic_line(&input);
    }

    #[test]
    fn evaluator_handles_any_buffer(
        buffer in prop::collection::vec(any::<u8>(), 0..1024)
    ) {
        let rule = create_test_rule();
        let _ = evaluate_rule(&rule, &buffer);
    }
}
```

### Parameterized Tests

```rust
#[test]
fn test_endianness_variants() {
    let cases = vec![
        (Endianness::Big, &[0x00, 0x01u8] as &[u8], 1u64),
        (Endianness::Little, &[0x01, 0x00u8] as &[u8], 1u64),
    ];

    for (endian, buffer, expected) in cases {
        let result = read_short(buffer, endian);
        assert_eq!(result, Ok(expected), "Failed for {:?}", endian);
    }
}
```

### Test Fixtures

```rust
fn create_elf_header() -> Vec<u8> {
    vec![0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00]
}

fn create_test_rule() -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::String { max_length: None },
        op: Operator::Equal,
        value: Value::String("ELF".to_string()),
        message: "ELF file".to_string(),
        children: vec![],
        level: 0,
    }
}
```

## Test Organization

```
src/
  parser/
    mod.rs          # #[cfg(test)] mod tests { ... }
    ast.rs          # #[cfg(test)] mod tests { ... }
    grammar.rs      # #[cfg(test)] mod tests { ... }
  evaluator/
    mod.rs          # #[cfg(test)] mod tests { ... }
    types.rs        # #[cfg(test)] mod tests { ... }
    operators.rs    # #[cfg(test)] mod tests { ... }
  io/
    mod.rs          # #[cfg(test)] mod tests { ... }
  output/
    mod.rs          # #[cfg(test)] mod tests { ... }
tests/
  compatibility.rs  # Integration tests against GNU file
  integration.rs    # End-to-end rule evaluation
benches/
  evaluation.rs     # Performance benchmarks
```

## Common Testing Mistakes to Avoid

### WRONG: Testing internal state

```rust
assert_eq!(parser.line_number, 5); // Implementation detail
```

### CORRECT: Test observable behavior

```rust
let rules = parse_magic_file(input)?;
assert_eq!(rules.len(), 5);
assert_eq!(rules[0].message, "ELF");
```

### WRONG: Ignoring error paths

```rust
let result = evaluate_rule(&rule, buffer).unwrap(); // Will panic
```

### CORRECT: Test both success and error

```rust
assert!(evaluate_rule(&rule, buffer).is_ok());
assert!(evaluate_rule(&rule, &[]).is_ok()); // Empty buffer
assert!(evaluate_rule(&bad_rule, buffer).is_err()); // Bad rule
```

## Quick Reference

```bash
# Run all tests
cargo nextest run

# Run specific module tests
cargo test parser::grammar::tests

# Run with output visible
cargo test -- --nocapture

# Run doc tests
cargo test --doc

# Coverage report
cargo llvm-cov --html

# Benchmarks
cargo bench
```
