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

**Unit Tests**: Located in source files with `#[cfg(test)]` modules

**Integration Tests**: Located in `tests/` directory:

- `tests/cli_integration.rs` - CLI subprocess tests using `assert_cmd`
- `tests/integration_tests.rs` - End-to-end evaluation tests
- `tests/evaluator_tests.rs` - Evaluator component tests
- `tests/parser_integration_tests.rs` - Parser integration tests
- `tests/json_integration_test.rs` - JSON output format tests
- `tests/compatibility_tests.rs` - GNU `file` compatibility tests
- `tests/directory_loading_tests.rs` - Magic directory loading tests
- `tests/mime_tests.rs` - MIME type detection tests
- `tests/tags_tests.rs` - Tag extraction tests
- `tests/property_tests.rs` - Property-based tests using `proptest`

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test cli_integration
cargo test --test property_tests
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

- `test_type_kind_byte` - Single byte type handling with signedness
- `test_type_kind_short` - 16-bit integer types with endianness
- `test_type_kind_long` - 32-bit integer types with endianness
- `test_type_kind_quad` - 64-bit integer types with endianness
- `test_type_kind_string` - String types with length limits
- `test_type_kind_serialization` - All type serialization including signed/unsigned variants
- `test_serialize_type_kind_quad` - Quad type serialization (build_helpers.rs)

**Operator Tests:**

- `test_operator_variants` - All operator types (Equal, NotEqual, LessThan, GreaterThan, LessEqual, GreaterEqual, BitwiseAnd, BitwiseAndMask)
- `test_operator_serialization` - Operator serialization including comparison operators

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
- `test_parse_operator_comparison` - Comparison operators (\<, >, \<=, >=)
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

#### Evaluator Component Tests

**Type Reading Tests:**

- `test_read_byte` - Single byte reading with signedness
- `test_read_short_endianness_and_signedness` - 16-bit reading with all endian/sign combinations
- `test_read_short_extreme_values` - 16-bit boundary values
- `test_read_long_endianness_and_signedness` - 32-bit reading with all endian/sign combinations
- `test_read_long_buffer_overrun` - 32-bit buffer boundary checking
- `test_read_quad_endianness_and_signedness` - 64-bit reading with all endian/sign combinations
- `test_read_quad_buffer_overrun` - 64-bit buffer boundary checking
- `test_read_quad_at_offset` - 64-bit reading at non-zero offsets
- `test_read_string` - Null-terminated string reading
- `test_read_typed_value` - Dispatch to correct type reader

**Value Coercion Tests:**

- `test_coerce_value_to_type` - Type conversion including quad overflow handling

**Strength Calculation Tests:**

- `test_strength_type_byte` - Byte type strength
- `test_strength_type_short` - 16-bit type strength
- `test_strength_type_long` - 32-bit type strength
- `test_strength_type_quad` - 64-bit type strength
- `test_strength_type_string` - String type strength with/without max_length
- `test_strength_operator_equal` - Operator strength calculations

#### Integration Tests

**End-to-End Evaluation Tests:**

- `test_quad_lequad_matches_little_endian_value` - LE quad pattern matching
- `test_quad_bequad_matches_big_endian_value` - BE quad pattern matching
- `test_quad_signed_negative_one` - Signed 64-bit negative value matching
- `test_quad_nested_child_rule_with_offset` - Quad types in hierarchical rules

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

### Integration Tests

CLI integration tests are located in `tests/cli_integration.rs` and use the `assert_cmd` crate for subprocess-based testing. This approach provides natural process isolation and eliminates the need for fragile fd manipulation.

**Running CLI integration tests:**

```bash
# Run all CLI integration tests
cargo test --test cli_integration

# Run specific test
cargo test --test cli_integration test_builtin_elf_detection

# Run with output
cargo test --test cli_integration -- --nocapture
```

**Test organization in `tests/cli_integration.rs`:**

- **Builtin Flag Tests**: Test `--use-builtin` with various file formats (ELF, PNG, JPEG, PDF, ZIP, GIF)
- **Stdin Tests**: Test stdin input handling, truncation warnings, and format detection
- **Multiple File Tests**: Test sequential processing, partial failures, and strict mode behavior
- **Error Handling Tests**: Test file not found, directory errors, magic file errors, and invalid arguments
- **Timeout Tests**: Test `--timeout-ms` argument parsing and validation
- **Output Format Tests**: Test text and JSON output formats
- **Shell Completion Tests**: Test `--generate-completion` for bash, zsh, and fish
- **Custom Magic File Tests**: Test custom magic file loading and fallback behavior
- **Edge Cases**: Test file names with spaces, Unicode, empty files, and small files
- **CLI Argument Parsing**: Test multiple files, strict mode, and flag combinations

**Example CLI integration test:**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Helper to create a Command for the rmagic binary
fn rmagic_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("rmagic"))
}

#[test]
fn test_builtin_elf_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.elf");
    std::fs::write(&test_file, b"\x7fELF\x02\x01\x01\x00")
        .expect("Failed to create test file");

    rmagic_cmd()
        .args(["--use-builtin", test_file.to_str().expect("Invalid path")])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}
```

**Parser integration tests** are also located in the `tests/` directory:

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

### Property Tests

Property-based testing using `proptest` is implemented in `tests/property_tests.rs`:

```bash
# Run property tests
cargo test --test property_tests

# Run with more test cases
PROPTEST_CASES=1000 cargo test --test property_tests
```

The property tests verify:

- **Serialization roundtrips**: AST types serialize and deserialize correctly
- **Evaluation safety**: Evaluation never panics on arbitrary input
- **Configuration validation**: Invalid configurations are rejected
- **Known pattern detection**: ELF, ZIP, PDF patterns are correctly detected

Example property test:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_evaluation_never_panics(buffer in prop::collection::vec(any::<u8>(), 0..1024)) {
        let db = MagicDatabase::with_builtin_rules().expect("should load");
        // Should not panic regardless of buffer contents
        let result = db.evaluate_buffer(&buffer);
        prop_assert!(result.is_ok());
    }
}
```

### Compatibility Tests

Compatibility tests validate against GNU `file` command using the canonical test suite from the file project. Test data is located in `third_party/tests/`.

```bash
# Run compatibility tests (requires test files)
cargo test test_compatibility_with_original_libmagic -- --ignored

# Or use the just recipe
just test-compatibility
```

The compatibility workflow runs automatically in CI on pushes to main/develop.

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

**Testing Signed vs Unsigned Byte Behavior:**

```rust
#[test]
fn test_signed_unsigned_byte_handling() {
    // Test signed byte interpretation
    let signed_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: true },
        op: Operator::GreaterThan,
        value: Value::Int(0),
        message: "Positive signed byte".to_string(),
        children: vec![],
        level: 0,
    };

    // 0x7f = 127 as signed (positive)
    // 0x80 = -128 as signed (negative)

    // Test unsigned byte interpretation
    let unsigned_rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Byte { signed: false },
        op: Operator::GreaterThan,
        value: Value::Uint(127),
        message: "Large unsigned byte".to_string(),
        children: vec![],
        level: 0,
    };

    // Both 0x7f and 0x80 are > 127 when interpreted as unsigned
}
```

**Testing 64-bit Integer (Quad) Types:**

```rust
#[test]
fn test_read_quad_endianness_and_signedness() {
    // Little-endian unsigned
    let buffer = &[0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12];
    let result = read_quad(buffer, 0, Endianness::Little, false).unwrap();
    assert_eq!(result, Value::Uint(0x1234_5678_90ab_cdef));

    // Big-endian signed negative
    let buffer = &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let result = read_quad(buffer, 0, Endianness::Big, true).unwrap();
    assert_eq!(result, Value::Int(-1));
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
        typ: TypeKind::Byte { signed: true },
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

## CLI Testing

### CLI Integration Tests

CLI functionality is tested using the `assert_cmd` crate in `tests/cli_integration.rs`. This subprocess-based approach provides:

- **Process isolation**: Each test runs `rmagic` as a separate process
- **Realistic testing**: Tests actual CLI behavior including exit codes and output
- **Reliable coverage**: Works correctly under `llvm-cov` for coverage reporting
- **Cross-platform compatibility**: No platform-specific fd manipulation required

### Running CLI Tests

```bash
# Run all CLI integration tests
cargo test --test cli_integration

# Run specific CLI test
cargo test --test cli_integration test_builtin_elf_detection

# Run with verbose output
cargo test --test cli_integration -- --nocapture
```

### Test Categories in cli_integration.rs

| Category                | Description                                               |
| ----------------------- | --------------------------------------------------------- |
| Builtin Flag Tests      | Test `--use-builtin` with ELF, PNG, JPEG, PDF, ZIP, GIF   |
| Stdin Tests             | Test `-` input, truncation warnings, format detection     |
| Multiple File Tests     | Test sequential processing, strict mode, partial failures |
| Error Handling Tests    | Test file not found, directory errors, invalid arguments  |
| Timeout Tests           | Test `--timeout-ms` parsing and validation                |
| Output Format Tests     | Test `--json` and `--text` output formats                 |
| Shell Completion Tests  | Test `--generate-completion` for various shells           |
| Custom Magic File Tests | Test `--magic-file` loading and fallback                  |
| Edge Cases              | Test Unicode filenames, empty files, small files          |

### Best Practices

1. **Use `assert_cmd`**: All CLI tests use `rmagic_cmd()` helper (wrapping `cargo_bin!("rmagic")` macro) for subprocess testing
2. **Use `predicates`**: Check stdout/stderr with predicate matchers for readable assertions
3. **Use `tempfile`**: Create temporary test files with `TempDir` for isolation
4. **Derive from config**: Use `EvaluationConfig::default()` for thresholds instead of hardcoding

## Benchmarks

Performance benchmarks are implemented using Criterion in the `benches/` directory:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench parser
cargo bench evaluation
cargo bench io

# Generate HTML benchmark report
cargo bench -- --noplot
```

### Available Benchmarks

| Benchmark          | Description                                |
| ------------------ | ------------------------------------------ |
| `parser_bench`     | Magic file parsing performance             |
| `evaluation_bench` | Rule evaluation against various file types |
| `io_bench`         | Memory-mapped I/O operations               |

### Benchmark CI

Benchmarks run automatically:

- **Weekly**: Scheduled runs on Sunday at 3 AM UTC
- **On PR**: When performance-critical code changes (src/evaluator, src/parser, src/io, benches)
- **Manual**: Via workflow_dispatch

The CI compares PR benchmarks against the main branch and reports regressions.

## Future Testing Plans

### Fuzzing Integration (Phase 2)

- **Parser Fuzzing**: Use cargo-fuzz for parser robustness
- **Evaluator Fuzzing**: Test evaluation engine with malformed files
- **Continuous Fuzzing**: Integrate with OSS-Fuzz for ongoing testing

The comprehensive testing strategy ensures libmagic-rs maintains high quality, reliability, and compatibility while enabling confident refactoring and feature development.
