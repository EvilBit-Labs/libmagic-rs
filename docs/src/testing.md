# Testing and Quality

libmagic-rs maintains high code quality through comprehensive testing, strict linting, and continuous integration.

## Testing Strategy

### Unit Tests

Every module includes comprehensive unit tests:

- **AST structures**: Serialization, comparison, edge cases
- **Parser components**: Grammar rules, error handling
- **Evaluator logic**: Offset resolution, type reading, operators
- **I/O utilities**: Buffer access, memory mapping, error conditions

### Integration Tests

End-to-end testing of complete workflows:

- **File type detection**: Real files with expected results
- **Magic file parsing**: Complete magic file processing
- **CLI functionality**: Command-line interface testing
- **Error scenarios**: Graceful handling of invalid inputs

### Property-Based Testing

Using `proptest` for fuzzing and edge case discovery:

- **Parser robustness**: Random magic file inputs
- **Evaluator safety**: Random file buffers and offsets
- **Serialization consistency**: Round-trip testing

### Compatibility Testing

Ensuring compatibility with existing tools:

- **GNU file comparison**: Identical results for common files
- **Magic file compatibility**: Support for existing magic databases
- **Performance parity**: Speed and memory usage comparison

## Code Quality Standards

### Linting Configuration

Strict clippy configuration enforces:

- **Security**: No unsafe code, bounds checking
- **Correctness**: No panics, proper error handling
- **Performance**: Efficient algorithms, minimal allocations
- **Style**: Consistent formatting, clear naming

### Coverage Requirements

- **Minimum coverage**: 85% line coverage
- **Critical paths**: 100% coverage for safety-critical code
- **Documentation**: All public APIs must have examples

### Static Analysis

- **Clippy**: Comprehensive linting with pedantic rules
- **Rustfmt**: Consistent code formatting
- **Cargo audit**: Security vulnerability scanning
- **Dependency analysis**: Minimal and vetted dependencies

## Running Tests

### Basic Testing

```bash
# Run all tests
cargo test

# Run with nextest (faster)
cargo nextest run

# Run specific test modules
cargo test ast_structures
cargo test parser
```

### Coverage Analysis

```bash
# Install coverage tools
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

### Property Testing

```bash
# Run property-based tests
cargo test proptest

# Run with more iterations
PROPTEST_CASES=10000 cargo test proptest
```

### Benchmarking

```bash
# Install criterion
cargo install criterion

# Run benchmarks
cargo bench

# Compare with baseline
cargo bench -- --save-baseline main
```

## Continuous Integration

GitHub Actions workflow ensures:

- **Multi-platform testing**: Linux, macOS, Windows
- **Multiple Rust versions**: Stable, beta, nightly
- **Dependency checking**: Security audits, license compliance
- **Documentation**: Ensure docs build and examples work

### CI Checks

```bash
# Format check
cargo fmt -- --check

# Linting
cargo clippy -- -D warnings

# Tests
cargo test --all-features

# Documentation
cargo doc --document-private-items

# Security audit
cargo audit
```

## Quality Metrics

### Code Coverage

- **Current target**: 85% line coverage
- **Critical components**: 95%+ coverage
- **Public APIs**: 100% coverage with examples

### Performance Benchmarks

- **Regression testing**: Prevent performance degradation
- **Comparison baselines**: Track improvements over time
- **Memory usage**: Monitor allocation patterns

### Security Analysis

- **Dependency scanning**: Regular security audits
- **Fuzzing**: Continuous robustness testing
- **Static analysis**: Automated vulnerability detection

## Test Organization

### Directory Structure

```text
tests/
├── integration/           # End-to-end tests
├── fixtures/             # Test files and data
│   ├── magic/           # Sample magic files
│   ├── samples/         # Test binary files
│   └── expected/        # Expected output files
└── compatibility/        # GNU file comparison tests
```

### Test Categories

- **Unit tests**: In-module `#[cfg(test)]` blocks
- **Integration tests**: `tests/` directory
- **Documentation tests**: Examples in rustdoc comments
- **Benchmark tests**: `benches/` directory

## Contributing to Tests

### Writing Tests

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

    #[test]
    fn test_error_conditions() {
        let result = function_that_should_fail();
        assert!(result.is_err());

        match result {
            Err(ExpectedError::SpecificVariant) => (),
            _ => panic!("Expected specific error variant"),
        }
    }
}
```

### Property Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_serialization_roundtrip(rule in any::<MagicRule>()) {
        let json = serde_json::to_string(&rule)?;
        let deserialized: MagicRule = serde_json::from_str(&json)?;
        prop_assert_eq!(rule, deserialized);
    }
}
```

### Integration Tests

```rust
#[test]
fn test_elf_detection() {
    let db = MagicDatabase::load_from_file("tests/fixtures/magic/elf.magic")?;
    let result = db.evaluate_file("tests/fixtures/samples/hello_world_elf")?;

    assert_eq!(result.description, "ELF 64-bit LSB executable");
    assert_eq!(
        result.mime_type,
        Some("application/x-executable".to_string())
    );
}
```

This comprehensive testing strategy ensures libmagic-rs maintains high quality, reliability, and compatibility throughout development.
