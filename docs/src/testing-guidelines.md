# Testing Guidelines

Comprehensive testing guidelines for libmagic-rs to ensure code quality, reliability, and maintainability.

## Testing Philosophy

libmagic-rs follows a comprehensive testing strategy:

- **Unit tests**: Test individual functions and methods in isolation
- **Integration tests**: Test complete workflows and component interactions
- **Property tests**: Use fuzzing to discover edge cases and ensure robustness
- **Compatibility tests**: Verify compatibility with existing magic files and GNU file output
- **Performance tests**: Ensure performance requirements are met

## Test Organization

### Directory Structure

```text
libmagic-rs/
├── src/
│   ├── lib.rs              # Unit tests in #[cfg(test)] modules
│   ├── parser/
│   │   ├── mod.rs          # Parser unit tests
│   │   └── ast.rs          # AST unit tests
│   └── evaluator/
│       └── mod.rs          # Evaluator unit tests
├── tests/
│   ├── integration/        # Integration tests
│   ├── compatibility/      # GNU file compatibility tests
│   └── fixtures/           # Test data and expected outputs
│       ├── magic/          # Sample magic files
│       ├── samples/        # Test binary files
│       └── expected/       # Expected output files
└── benches/                # Performance benchmarks
```

### Test Categories

#### Unit Tests

Located in `#[cfg(test)]` modules within source files:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_output);
    }
}
```

#### Integration Tests

Located in `tests/` directory:

```rust
// tests/integration/basic_workflow.rs
use libmagic_rs::{MagicDatabase, EvaluationConfig};

#[test]
fn test_complete_file_analysis_workflow() {
    let db = MagicDatabase::load_from_file("tests/fixtures/magic/basic.magic")
        .expect("Failed to load magic database");

    let result = db.evaluate_file("tests/fixtures/samples/elf64")
        .expect("Failed to evaluate file");

    assert_eq!(result.description, "ELF 64-bit LSB executable");
}
```

## Writing Effective Tests

### Test Naming

Use descriptive names that explain the scenario being tested:

```rust
// Good: Descriptive test names
#[test]
fn test_parse_absolute_offset_with_positive_decimal_value() { }

#[test]
fn test_parse_absolute_offset_with_hexadecimal_value() { }

#[test]
fn test_parse_offset_returns_error_for_invalid_syntax() { }

// Bad: Generic test names
#[test]
fn test_parse_offset() { }

#[test]
fn test_error_case() { }
```

### Test Structure

Follow the Arrange-Act-Assert pattern:

```rust
#[test]
fn test_magic_rule_evaluation_with_matching_bytes() {
    // Arrange
    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte,
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "ELF magic".to_string(),
        children: vec![],
        level: 0,
    };
    let buffer = vec![0x7f, 0x45, 0x4c, 0x46]; // ELF magic

    // Act
    let result = evaluate_rule(&rule, &buffer);

    // Assert
    assert!(result.is_ok());
    assert!(result.unwrap());
}
```

### Assertion Best Practices

Use specific assertions with helpful error messages:

```rust
// Good: Specific assertions
assert_eq!(result.description, "ELF executable");
assert!(result.confidence > 0.8);

// Good: Custom error messages
assert_eq!(
    parsed_offset,
    OffsetSpec::Absolute(42),
    "Parser should correctly handle decimal offset values"
);

// Good: Pattern matching for complex types
match result {
    Ok(OffsetSpec::Indirect { base_offset, adjustment, .. }) => {
        assert_eq!(base_offset, 0x20);
        assert_eq!(adjustment, 4);
    }
    _ => panic!("Expected indirect offset specification"),
}

// Avoid: Generic assertions
assert!(result.is_ok());
assert_ne!(value, 0);
```

### Error Testing

Test error conditions thoroughly:

```rust
#[test]
fn test_parse_magic_file_with_invalid_syntax() {
    let invalid_magic = "0 invalid_type value message";

    let result = parse_magic_string(invalid_magic);

    assert!(result.is_err());
    match result {
        Err(LibmagicError::ParseError { line, message }) => {
            assert_eq!(line, 1);
            assert!(message.contains("invalid_type"));
        }
        _ => panic!("Expected ParseError for invalid syntax"),
    }
}

#[test]
fn test_file_evaluation_with_missing_file() {
    let db = MagicDatabase::load_from_file("tests/fixtures/magic/basic.magic").unwrap();

    let result = db.evaluate_file("nonexistent_file.bin");

    assert!(result.is_err());
    match result {
        Err(LibmagicError::IoError(_)) => (), // Expected
        _ => panic!("Expected IoError for missing file"),
    }
}
```

### Edge Case Testing

Test boundary conditions and edge cases:

```rust
#[test]
fn test_offset_parsing_edge_cases() {
    // Test zero offset
    let result = parse_offset("0");
    assert_eq!(result.unwrap(), OffsetSpec::Absolute(0));

    // Test maximum positive offset
    let result = parse_offset(&i64::MAX.to_string());
    assert_eq!(result.unwrap(), OffsetSpec::Absolute(i64::MAX));

    // Test negative offset
    let result = parse_offset("-1");
    assert_eq!(result.unwrap(), OffsetSpec::Absolute(-1));

    // Test empty input
    let result = parse_offset("");
    assert!(result.is_err());
}
```

## Property-Based Testing

Use `proptest` for fuzzing and property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_magic_rule_serialization_roundtrip(rule in any::<MagicRule>()) {
        // Property: serialization should be reversible
        let json = serde_json::to_string(&rule)?;
        let deserialized: MagicRule = serde_json::from_str(&json)?;
        prop_assert_eq!(rule, deserialized);
    }

    #[test]
    fn test_offset_resolution_never_panics(
        offset in any::<OffsetSpec>(),
        buffer in prop::collection::vec(any::<u8>(), 0..1024)
    ) {
        // Property: offset resolution should never panic
        let _ = resolve_offset(&offset, &buffer, 0);
        // If we reach here without panicking, the test passes
    }
}
```

## Test Data Management

### Fixture Organization

Organize test data systematically:

```text
tests/fixtures/
├── magic/
│   ├── basic.magic         # Simple rules for testing
│   ├── complex.magic       # Complex hierarchical rules
│   └── invalid.magic       # Invalid syntax for error testing
├── samples/
│   ├── elf32               # 32-bit ELF executable
│   ├── elf64               # 64-bit ELF executable
│   ├── zip_archive.zip     # ZIP file
│   └── text_file.txt       # Plain text file
└── expected/
    ├── elf32.txt           # Expected output for elf32
    ├── elf64.json          # Expected JSON output for elf64
    └── compatibility.txt   # GNU file compatibility results
```

### Creating Test Fixtures

```rust
// Helper function for creating test data
fn create_elf_magic_rule() -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long {
            endian: Endianness::Little,
            signed: false
        },
        op: Operator::Equal,
        value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
        message: "ELF executable".to_string(),
        children: vec![],
        level: 0,
    }
}

// Helper for creating test buffers
fn create_elf_buffer() -> Vec<u8> {
    let mut buffer = vec![0x7f, 0x45, 0x4c, 0x46]; // ELF magic
    buffer.extend_from_slice(&[0x02, 0x01, 0x01, 0x00]); // 64-bit, little-endian
    buffer.resize(64, 0); // Pad to minimum ELF header size
    buffer
}
```

## Compatibility Testing

### GNU File Comparison

Test compatibility with GNU `file` command:

```rust
#[test]
fn test_gnu_file_compatibility() {
    use std::process::Command;

    let sample_file = "tests/fixtures/samples/elf64";

    // Get GNU file output
    let gnu_output = Command::new("file")
        .arg("--brief")
        .arg(sample_file)
        .output()
        .expect("Failed to run GNU file command");

    let gnu_result = String::from_utf8(gnu_output.stdout)
        .expect("Invalid UTF-8 from GNU file")
        .trim();

    // Get libmagic-rs output
    let db = MagicDatabase::load_from_file("tests/fixtures/magic/standard.magic").unwrap();
    let result = db.evaluate_file(sample_file).unwrap();

    // Compare results (allowing for minor differences)
    assert!(
        results_are_compatible(&result.description, gnu_result),
        "libmagic-rs output '{}' not compatible with GNU file output '{}'",
        result.description,
        gnu_result
    );
}

fn results_are_compatible(rust_output: &str, gnu_output: &str) -> bool {
    // Implement compatibility checking logic
    // Allow for minor differences in formatting, version numbers, etc.
    rust_output.contains("ELF") && gnu_output.contains("ELF")
}
```

## Performance Testing

### Benchmark Tests

Use `criterion` for performance benchmarks:

```rust
// benches/evaluation_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use libmagic_rs::{MagicDatabase, EvaluationConfig};

fn bench_file_evaluation(c: &mut Criterion) {
    let db = MagicDatabase::load_from_file("tests/fixtures/magic/standard.magic")
        .expect("Failed to load magic database");

    c.bench_function("evaluate_elf_file", |b| {
        b.iter(|| {
            db.evaluate_file(black_box("tests/fixtures/samples/elf64"))
                .expect("Evaluation failed")
        })
    });
}

criterion_group!(benches, bench_file_evaluation);
criterion_main!(benches);
```

### Performance Regression Testing

```rust
#[test]
fn test_evaluation_performance() {
    use std::time::Instant;

    let db = MagicDatabase::load_from_file("tests/fixtures/magic/standard.magic").unwrap();

    let start = Instant::now();
    let _result = db.evaluate_file("tests/fixtures/samples/large_file.bin").unwrap();
    let duration = start.elapsed();

    // Ensure evaluation completes within reasonable time
    assert!(
        duration.as_millis() < 100,
        "File evaluation took too long: {}ms",
        duration.as_millis()
    );
}
```

## Test Execution

### Running Tests

```bash
# Run all tests
cargo test

# Run with nextest (faster, better output)
cargo nextest run

# Run specific test modules
cargo test ast_structures
cargo test integration

# Run tests with output
cargo test -- --nocapture

# Run ignored tests
cargo test -- --ignored

# Run property tests with more cases
PROPTEST_CASES=10000 cargo test proptest
```

### Coverage Analysis

```bash
# Install coverage tools
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --html --open

# Coverage for specific tests
cargo llvm-cov --html --tests integration
```

### Continuous Integration

Ensure tests run in CI with multiple configurations:

```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable, beta]

steps:
  - name: Run tests
    run: cargo nextest run --all-features

  - name: Run property tests
    run: cargo test proptest
    env:
      PROPTEST_CASES: 1000

  - name: Check compatibility
    run: cargo test compatibility
    if: matrix.os == 'ubuntu-latest'
```

## Test Maintenance

### Keeping Tests Updated

- **Update fixtures**: When adding new file format support
- **Maintain compatibility**: Update compatibility tests when GNU file changes
- **Performance baselines**: Update performance expectations as optimizations are added
- **Documentation**: Keep test documentation current with implementation

### Test Debugging

```rust
// Use debug output for failing tests
#[test]
fn debug_failing_test() {
    let result = function_under_test();
    println!("Debug output: {:?}", result);
    assert_eq!(result, expected_value);
}

// Use conditional compilation for debug tests
#[cfg(test)]
#[cfg(feature = "debug-tests")]
mod debug_tests {
    #[test]
    fn verbose_test() {
        // Detailed debugging test
    }
}
```

This comprehensive testing approach ensures libmagic-rs maintains high quality, reliability, and compatibility throughout its development lifecycle.
