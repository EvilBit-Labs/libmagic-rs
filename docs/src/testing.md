# Testing and Quality Assurance

The libmagic-rs project maintains high quality standards through comprehensive testing, strict linting, and continuous integration. This chapter covers the testing strategy, current test coverage, and quality assurance practices.

## Testing Philosophy

### Comprehensive Coverage

The project aims for comprehensive test coverage across all components:

- **Unit Tests**: Test individual functions and methods in isolation
- **Integration Tests**: Test component interactions and workflows
- **Property Tests**: Use property-based testing for edge cases
- **Compatibility Tests**: Validate against GNU `file` command results
- **Performance Tests**: Benchmark critical path performance

### Quality Gates

All code must pass these quality gates:

1. **Zero Warnings**: `cargo clippy -- -D warnings` must pass
2. **All Tests Pass**: Complete test suite must pass
3. **Code Coverage**: Target >85% coverage for new code
4. **Documentation**: All public APIs must be documented
5. **Memory Safety**: No unsafe code except in vetted dependencies

## Current Test Coverage

### Test Statistics

**Total Tests**: 79 passing unit tests

```bash
$ cargo test
running 79 tests
test result: ok. 79 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Test Distribution

#### AST Structure Tests (29 tests)

**OffsetSpec Tests:**

- `test_offset_spec_absolute` - Basic absolute offset creation
- `test_offset_spec_indirect` - Complex indirect offset structures
- `test_offset_spec_relative` - Relative offset handling
- `test_offset_spec_from_end` - End-relative offset calculations
- `test_offset_spec_serialization` - JSON serialization round-trips
- `test_all_offset_spec_variants` - Comprehensive variant testing
- `test_endianness_variants` - Endianness handling in all contexts

**Value Tests:**

- `test_value_uint` - Unsigned integer values including extremes
- `test_value_int` - Signed integer values including boundaries
- `test_value_bytes` - Byte sequence handling and comparison
- `test_value_string` - String values including Unicode
- `test_value_comparison` - Cross-type comparison behavior
- `test_value_serialization` - Complete serialization testing
- `test_value_serialization_edge_cases` - Boundary and extreme values

**TypeKind Tests:**

- `test_type_kind_byte` - Single byte type handling
- `test_type_kind_short` - 16-bit integer types with endianness
- `test_type_kind_long` - 32-bit integer types with endianness
- `test_type_kind_string` - String types with length limits
- `test_type_kind_serialization` - All type serialization

**Operator Tests:**

- `test_operator_variants` - All operator types
- `test_operator_serialization` - Operator serialization

**MagicRule Tests:**

- `test_magic_rule_creation` - Basic rule construction
- `test_magic_rule_with_children` - Hierarchical rule structures
- `test_magic_rule_serialization` - Complete rule serialization

#### Parser Component Tests (50 tests)

**Number Parsing Tests:**

- `test_parse_decimal_number` - Basic decimal parsing
- `test_parse_hex_number` - Hexadecimal parsing with 0x prefix
- `test_parse_number_positive` - Positive number handling
- `test_parse_number_negative` - Negative number handling
- `test_parse_number_edge_cases` - Boundary values and error conditions
- `test_parse_number_with_remaining_input` - Partial parsing behavior

**Offset Parsing Tests:**

- `test_parse_offset_absolute_positive` - Positive absolute offsets
- `test_parse_offset_absolute_negative` - Negative absolute offsets
- `test_parse_offset_with_whitespace` - Whitespace tolerance
- `test_parse_offset_with_remaining_input` - Partial parsing
- `test_parse_offset_edge_cases` - Error conditions and boundaries
- `test_parse_offset_common_magic_file_values` - Real-world patterns
- `test_parse_offset_boundary_values` - Extreme values

**Operator Parsing Tests:**

- `test_parse_operator_equality` - Equality operators (= and ==)
- `test_parse_operator_inequality` - Inequality operators (!= and \<>)
- `test_parse_operator_bitwise_and` - Bitwise AND operator (&)
- `test_parse_operator_with_remaining_input` - Partial parsing
- `test_parse_operator_precedence` - Operator precedence handling
- `test_parse_operator_invalid_input` - Error condition handling
- `test_parse_operator_edge_cases` - Boundary conditions
- `test_parse_operator_common_magic_file_patterns` - Real patterns

**Value Parsing Tests:**

- `test_parse_quoted_string_simple` - Basic string parsing
- `test_parse_quoted_string_with_escapes` - Escape sequence handling
- `test_parse_quoted_string_with_whitespace` - Whitespace handling
- `test_parse_quoted_string_invalid` - Error conditions
- `test_parse_hex_bytes_with_backslash_x` - \\x prefix hex bytes
- `test_parse_hex_bytes_without_prefix` - Raw hex byte sequences
- `test_parse_hex_bytes_mixed_case` - Case insensitive hex
- `test_parse_numeric_value_positive` - Positive numeric values
- `test_parse_numeric_value_negative` - Negative numeric values
- `test_parse_value_string_literals` - String literal parsing
- `test_parse_value_numeric_literals` - Numeric literal parsing
- `test_parse_value_hex_byte_sequences` - Hex byte parsing
- `test_parse_value_type_precedence` - Type detection precedence
- `test_parse_value_edge_cases` - Boundary conditions
- `test_parse_value_invalid_input` - Error handling

## Test Categories

### Unit Tests

Located alongside source code using `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        let result = parse_number("123");
        assert_eq!(result, Ok(("", 123)));
    }

    #[test]
    fn test_error_conditions() {
        let result = parse_number("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values
        assert_eq!(parse_number("0"), Ok(("", 0)));
        assert_eq!(parse_number("-0"), Ok(("", 0)));

        // Test extreme values
        let max_val = i64::MAX.to_string();
        assert_eq!(parse_number(&max_val), Ok(("", i64::MAX)));
    }
}
```

### Integration Tests (Planned)

Will be located in `tests/` directory:

```rust
// tests/parser_integration.rs
use libmagic_rs::parser::*;

#[test]
fn test_complete_rule_parsing() {
    let magic_line = "0 string \\x7fELF ELF executable";
    let rule = parse_magic_rule(magic_line).unwrap();

    assert_eq!(rule.offset, OffsetSpec::Absolute(0));
    assert_eq!(rule.message, "ELF executable");
}

#[test]
fn test_hierarchical_rules() {
    let magic_content = r#"
0 string \x7fELF ELF
>4 byte 1 32-bit
>4 byte 2 64-bit
"#;
    let rules = parse_magic_file_content(magic_content).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].children.len(), 2);
}
```

### Property Tests (Planned)

Using `proptest` for fuzz-like testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_number_parsing_roundtrip(n in any::<i64>()) {
        let s = n.to_string();
        let (remaining, parsed) = parse_number(&s).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(parsed, n);
    }

    #[test]
    fn test_offset_parsing_never_panics(s in ".*") {
        // Should never panic, even on invalid input
        let _ = parse_offset(&s);
    }
}
```

### Compatibility Tests (Planned)

Validate against GNU `file` command:

```rust
#[test]
fn test_elf_detection_compatibility() {
    let gnu_result = run_gnu_file("test_files/elf64_sample");
    let our_result = evaluate_file("test_files/elf64_sample");

    assert_eq!(extract_file_type(&gnu_result), our_result.description);
}
```

## Test Utilities and Helpers

### Common Test Patterns

**Whitespace Testing Helper:**

```rust
fn test_with_whitespace_variants<T, F>(input: &str, expected: &T, parser: F)
where
    T: Clone + PartialEq + std::fmt::Debug,
    F: Fn(&str) -> IResult<&str, T>,
{
    let variants = vec![
        format!(" {}", input),  // Leading space
        format!("  {}", input), // Leading spaces
        format!("\t{}", input), // Leading tab
        format!("{} ", input),  // Trailing space
        format!("{}  ", input), // Trailing spaces
        format!("{}\t", input), // Trailing tab
        format!(" {} ", input), // Both sides
    ];

    for variant in variants {
        assert_eq!(
            parser(&variant),
            Ok(("", expected.clone())),
            "Failed with whitespace: '{}'",
            variant
        );
    }
}
```

**Error Testing Patterns:**

```rust
#[test]
fn test_parser_error_conditions() {
    let error_cases = vec![
        ("", "empty input"),
        ("abc", "invalid characters"),
        ("0xGG", "invalid hex digits"),
        ("--123", "double negative"),
    ];

    for (input, description) in error_cases {
        assert!(
            parse_number(input).is_err(),
            "Should fail on {}: '{}'",
            description,
            input
        );
    }
}
```

### Test Data Management

**Test Fixtures:**

```rust
// Common test data
const ELF_MAGIC: &[u8] = &[0x7f, 0x45, 0x4c, 0x46];
const ZIP_MAGIC: &[u8] = &[0x50, 0x4b, 0x03, 0x04];
const PDF_MAGIC: &str = "%PDF-";

fn create_test_rule() -> MagicRule {
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte,
        op: Operator::Equal,
        value: Value::Uint(0x7f),
        message: "Test rule".to_string(),
        children: vec![],
        level: 0,
    }
}
```

## Running Tests

### Basic Test Execution

```bash
# Run all tests
cargo test

# Run specific test module
cargo test parser::grammar::tests

# Run specific test
cargo test test_parse_number_positive

# Run tests with output
cargo test -- --nocapture

# Run ignored tests (if any)
cargo test -- --ignored
```

### Enhanced Test Running

```bash
# Use nextest for faster execution
cargo nextest run

# Run tests with coverage
cargo llvm-cov --html

# Run tests in release mode
cargo test --release

# Test documentation examples
cargo test --doc
```

### Continuous Testing

```bash
# Auto-run tests on file changes
cargo watch -x test

# Auto-run specific tests
cargo watch -x "test parser"

# Run checks and tests together
cargo watch -x check -x test
```

## Code Coverage

### Coverage Tools

```bash
# Install coverage tool
cargo install cargo-llvm-cov

# Generate HTML coverage report
cargo llvm-cov --html

# Generate lcov format for CI
cargo llvm-cov --lcov --output-path coverage.lcov

# Show coverage summary
cargo llvm-cov --summary-only
```

### Coverage Targets

- **Overall Coverage**: Target >85% for the project
- **New Code**: Require >90% coverage for new features
- **Critical Paths**: Require 100% coverage for parser and evaluator
- **Public APIs**: Require 100% coverage for all public functions

### Coverage Exclusions

Some code is excluded from coverage requirements:

```rust
// Debug/development code
#[cfg(debug_assertions)]
fn debug_helper() { /* ... */
}

// Error handling that's hard to trigger
#[cfg_attr(coverage, coverage(off))]
fn handle_system_error() { /* ... */
}
```

## Quality Assurance

### Automated Checks

All code must pass these automated checks:

```bash
# Formatting check
cargo fmt -- --check

# Linting with strict rules
cargo clippy -- -D warnings

# Documentation generation
cargo doc --document-private-items

# Security audit
cargo audit

# Dependency check
cargo tree --duplicates
```

### Manual Review Checklist

For code reviews:

- [ ] **Functionality**: Does the code work as intended?
- [ ] **Tests**: Are there comprehensive tests covering the changes?
- [ ] **Documentation**: Are public APIs documented with examples?
- [ ] **Error Handling**: Are errors handled gracefully?
- [ ] **Performance**: Are there any performance implications?
- [ ] **Memory Safety**: Is all buffer access bounds-checked?
- [ ] **Compatibility**: Does this maintain API compatibility?

### Performance Testing

```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph --bench parser_bench

# Memory usage analysis
valgrind --tool=massif target/release/rmagic large_file.bin
```

## Future Testing Plans

### Integration Testing

- **Complete Workflow Tests**: End-to-end magic file parsing and evaluation
- **File Format Tests**: Comprehensive testing against known file formats
- **Error Recovery Tests**: Graceful handling of malformed inputs

### Compatibility Testing

- **GNU file Compatibility**: Validate results against original implementation
- **Magic File Compatibility**: Test with real-world magic databases
- **Performance Parity**: Ensure comparable performance to libmagic

### Fuzzing Integration

- **Parser Fuzzing**: Use cargo-fuzz for parser robustness
- **Evaluator Fuzzing**: Test evaluation engine with malformed files
- **Continuous Fuzzing**: Integrate with OSS-Fuzz for ongoing testing

The comprehensive testing strategy ensures libmagic-rs maintains high quality, reliability, and compatibility while enabling confident refactoring and feature development.
