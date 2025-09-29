# AI Assistant Guidelines for libmagic-rs

This document provides comprehensive guidelines for AI assistants working on the libmagic-rs project, ensuring consistent, high-quality development practices and project understanding.

## Project Overview

**libmagic-rs** is a pure-Rust implementation of libmagic, designed to replace the C-based library with a memory-safe, efficient alternative for file type detection.

### Core Mission

- **Memory Safety**: Pure Rust implementation with no unsafe code (except vetted dependencies)
- **Performance**: Memory-mapped I/O with zero-copy operations where possible
- **Compatibility**: Support for common libmagic syntax patterns
- **Extensibility**: AST-based design for easy addition of new rule types

## Development Principles

### 1. Memory Safety First

- **No unsafe code** except in vetted dependencies (memmap2, byteorder, etc.)
- **Bounds checking** for all buffer access using `.get()` methods
- **Safe resource management** with RAII patterns
- **Graceful error handling** for malformed inputs

### 2. Zero-Warnings Policy

- All code must pass `cargo clippy -- -D warnings` with no exceptions
- Preserve all `deny` attributes and `-D warnings` flags
- Fix clippy suggestions unless they conflict with project requirements
- Use `cargo fmt` for consistent code formatting

### 3. Performance Critical

- Use memory-mapped I/O (`memmap2`) for efficient file access
- Implement zero-copy operations where possible
- Use Aho-Corasick indexing for multi-pattern string searches
- Cache compiled magic rules for performance
- Profile with `cargo bench` for performance regressions

### 4. Testing Required

- Target >85% test coverage with `cargo llvm-cov`
- All code changes must include comprehensive tests
- Use `cargo nextest` for faster, more reliable test execution
- Include property tests with `proptest` for fuzzing
- Benchmark critical path components with `criterion`

## Architecture Patterns

### Parser-Evaluator Design

The project follows a clear separation of concerns:

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Module Organization

```rust
// Core data structures in lib.rs
pub struct MagicRule { /* ... */ }
pub enum TypeKind { Byte, Short, Long, String, /* ... */ }
pub enum Operator { Equal, NotEqual, Greater, /* ... */ }

// Parser module structure
parser/
├── mod.rs      // Public parser interface
├── ast.rs      // AST node definitions
└── grammar.rs  // Magic file DSL parsing (nom/pest)

// Evaluator module structure
evaluator/
├── mod.rs       // Main evaluation engine
├── offset.rs    // Offset resolution (absolute, indirect, relative)
├── types.rs     // Type interpretation with endianness
└── operators.rs // Comparison and bitwise operations
```

## Code Quality Standards

### File Size Limits

- Keep source files under 500-600 lines
- Split larger files into focused modules
- Use clear, descriptive module names

### Error Handling Patterns

```rust
// Library errors should be descriptive and actionable
#[derive(Debug, thiserror::Error)]
pub enum MagicError {
    #[error("Failed to parse magic file at line {line}: {reason}")]
    ParseError { line: usize, reason: String },

    #[error("IO error reading file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid offset specification: {offset}")]
    InvalidOffset { offset: String },
}

// Use Result types consistently
pub fn evaluate_magic_rules(
    rules: &[MagicRule],
    data: &[u8]
) -> Result<Option<String>, MagicError> {
    // Implementation
}
```

### Naming Conventions

- **Files**: snake_case (e.g., `magic_rule.rs`)
- **Types**: PascalCase (e.g., `MagicRule`, `TypeKind`)
- **Functions**: snake_case (e.g., `resolve_offset`, `evaluate_rule`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_BUFFER_SIZE`)
- **Modules**: snake_case (e.g., `evaluator`, `output`)

## Development Workflow

### Standard Commands

```bash
# Development cycle
cargo check        # Fast syntax/type checking
cargo build        # Build project
cargo test         # Run all tests
cargo clippy       # Linting with strict warnings
cargo fmt          # Format code

# Performance and quality
cargo bench        # Run benchmarks
cargo doc          # Generate documentation
cargo test --doc   # Test documentation examples
```

### Testing Strategy

- **Unit Tests**: Alongside source files with `#[cfg(test)]`
- **Integration Tests**: In `tests/` directory with real magic files
- **Compatibility Tests**: Complete test suite from [original file project](https://raw.githubusercontent.com/file/file/refs/heads/master/tests/README)
- **Property Tests**: Use `proptest` for fuzzing magic rule evaluation
- **Benchmarks**: Critical path performance tests with `criterion`
- **Coverage**: Target >85% with `cargo llvm-cov`

## Magic File Compatibility

### Supported Syntax

- **Offsets**: Absolute, indirect, relative, and from-end specifications
- **Types**: byte, short, long, string, regex with endianness support
- **Operators**: =, !=, >, <, & (bitwise AND), ^ (XOR)
- **Nested Rules**: Hierarchical rule evaluation with proper indentation
- **String Matching**: Both exact and regex pattern matching

### Binary-Safe Regex Handling

```rust
// Use regex crate with bytes feature for binary-safe matching
pub trait BinaryRegex {
    fn find_at(&self, haystack: &[u8], start: usize) -> Option<Match>;
}

impl BinaryRegex for regex::bytes::Regex { /* ... */ }
```

## Performance Requirements

### Critical Optimizations

- **Memory Mapping**: Use `mmap` to avoid loading entire files into memory
- **Zero-Copy**: Minimize allocations during rule evaluation
- **Aho-Corasick**: Use for multi-pattern string searches when beneficial
- **Rule Caching**: Cache compiled magic rules for repeated use
- **Early Exit**: Stop evaluation as soon as a definitive match is found

### Benchmarking

```rust
// Example benchmark structure
#[bench]
fn bench_magic_evaluation(b: &mut Bencher) {
    let rules = load_magic_rules("tests/fixtures/standard.magic");
    let file_data = include_bytes!("../tests/fixtures/sample.bin");

    b.iter(|| {
        evaluate_rules(&rules, file_data)
    });
}
```

## Output Formats

### Text Output (Default)

```text
sample.bin: ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

### JSON Output (Structured)

```json
{
  "filename": "sample.bin",
  "matches": [
    {
      "text": "ELF 64-bit LSB executable",
      "offset": 0,
      "value": "7f454c46",
      "tags": ["executable", "elf"],
      "score": 90,
      "mime_type": "application/x-executable"
    }
  ],
  "metadata": {
    "file_size": 8192,
    "evaluation_time_ms": 2.3,
    "rules_evaluated": 45
  }
}
```

## Common Tasks and Patterns

### Adding New Type Support

1. Extend `TypeKind` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar.rs`
3. Implement reading logic in `src/evaluator/types.rs`
4. Add tests for the new type
5. Update documentation

### Adding New Operators

1. Extend `Operator` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar.rs`
3. Implement operator logic in `src/evaluator/operators.rs`
4. Add tests for the new operator
5. Update documentation

### Performance Optimization

1. Profile with `cargo bench` to identify bottlenecks
2. Use memory-mapped I/O for file access
3. Implement caching for compiled rules
4. Use Aho-Corasick for multi-pattern searches
5. Minimize allocations in hot paths

## Error Recovery Strategy

### Parse Errors

- Continue parsing after syntax errors
- Collect all errors for batch reporting
- Provide clear error messages with line numbers

### Evaluation Errors

- Graceful degradation
- Skip problematic rules and continue with others
- Maintain evaluation context for nested rules

### IO Errors

- Proper resource cleanup
- Clear error messages for file access issues
- Handle truncated and corrupted files safely

## Security Considerations

### Memory Safety

- No unsafe code except in vetted dependencies
- Bounds checking for all buffer access
- Safe handling of malformed input
- Fuzzing integration for robustness testing

### Input Validation

- Validate magic file syntax before parsing
- Check file size limits and resource usage
- Handle malicious or malformed input gracefully
- Implement timeouts for long-running evaluations

## Documentation Requirements

### API Documentation

- All public APIs require rustdoc with examples
- Include error conditions and recovery strategies
- Provide usage examples for common patterns
- Document performance characteristics

### Code Comments

- Explain complex algorithms and optimizations
- Document magic file syntax support
- Include references to libmagic compatibility
- Explain design decisions and trade-offs

## CI/CD Integration

### Automated Checks

The project includes automated CI checks via `.kiro/hooks/ci-auto-fix.kiro.hook`:

1. **Formatting**: `cargo fmt` for consistent code style
2. **Linting**: `cargo clippy -- -D warnings` for best practices
3. **Compilation**: `cargo check` and `cargo build` for error detection
4. **Testing**: `cargo test` and `cargo nextest run` for validation
5. **Security**: `cargo audit` for vulnerability detection
6. **License Compliance**: Verify dependency licenses

### Quality Gates

- All code must pass clippy with `-D warnings`
- Test coverage must be >85%
- No compilation warnings or errors
- All tests must pass
- Security audit must pass
- Performance benchmarks must not regress

## Project Context

### Current Status

- **Phase**: Early development (MVP)
- **Focus**: Core parser and evaluator implementation
- **Priority**: Memory safety and basic functionality
- **Next Steps**: Enhanced features and performance optimization

### Key Dependencies

- `memmap2`: Memory-mapped file I/O
- `byteorder`: Endianness handling
- `nom`: Parser combinators
- `serde`: Serialization
- `clap`: CLI argument parsing
- `regex`: Pattern matching
- `aho-corasick`: Multi-pattern search

### Development Phases

1. **MVP (v0.1)**: Basic parsing and evaluation
2. **Enhanced Features (v0.2)**: Indirect offsets, regex, caching
3. **Performance & Compatibility (v0.3)**: Optimizations, full compatibility
4. **Production Ready (v1.0)**: Stable API, complete documentation

## Best Practices

### Code Organization

- Keep modules focused and cohesive
- Use clear, descriptive names
- Minimize coupling between modules
- Maximize cohesion within modules

### Error Handling

- Use `Result<T, E>` patterns consistently
- Avoid panics in library code
- Provide actionable error messages
- Implement graceful degradation

### Testing

- Write tests alongside implementation
- Include edge cases and error conditions
- Use property-based testing for complex logic
- Benchmark performance-critical code

### Documentation

- Document public APIs thoroughly
- Include usage examples
- Explain design decisions
- Keep documentation up-to-date

## Troubleshooting

### Common Issues

- **Compilation errors**: Check for missing dependencies and syntax issues
- **Test failures**: Verify test logic and expected behavior
- **Performance issues**: Profile with `cargo bench` and optimize hot paths
- **Memory issues**: Check for bounds violations and resource leaks

### Debugging Tips

- Use `cargo test -- --nocapture` for test output
- Enable debug logging with `RUST_LOG=debug`
- Use `cargo clippy` to catch potential issues
- Profile with `cargo bench` for performance analysis

This guide ensures consistent, high-quality development practices for the libmagic-rs project while maintaining focus on memory safety, performance, and compatibility.
