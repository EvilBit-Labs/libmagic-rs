---
inclusion: always
---

# Rust Libmagic Development Guidelines

## AI Assistant Rules

### Core Principles

- **Memory Safety First**: Pure Rust implementation with no `unsafe` code except in vetted dependencies
- **Zero-Warnings Policy**: All code must pass `cargo clippy -- -D warnings` with no exceptions
- **Performance Critical**: Use memory-mapped I/O, zero-copy operations, and efficient algorithms
- **Testing Required**: All code changes must include comprehensive tests with >85% coverage
- **File Size Limits**: Keep source files under 500-600 lines; split larger files into focused modules
- **No Auto-Commits**: Never commit code without explicit user permission

### Code Quality Standards

- **Linting**: Preserve all `deny` attributes and `-D warnings` flags
- **Formatting**: Use `cargo fmt` with project defaults
- **Documentation**: All public APIs require rustdoc with examples and error conditions
- **Error Handling**: Use `Result<T, E>` patterns; avoid panics in library code
- **Naming**: Follow Rust conventions (snake_case functions, PascalCase types)

## Architecture Patterns

### Parser-Evaluator Design

- **AST-based Processing**: Parse magic files into Abstract Syntax Tree first
- **Hierarchical Matching**: Parent rules must match before evaluating children
- **Lazy Evaluation**: Only evaluate rules when necessary for performance
- **Memory Mapping**: Use `memmap2` for efficient file access without loading entire files

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
- **Property Tests**: Use `proptest` for fuzzing magic rule evaluation
- **Benchmarks**: Critical path performance tests with `criterion`
- **Coverage**: Target >85% with `cargo llvm-cov`

### Magic File Compatibility

- **Standard Format**: Support common magic file syntax (offsets, types, operators)
- **Nested Rules**: Implement proper hierarchical rule evaluation
- **Endianness**: Handle big-endian, little-endian, and native byte order
- **String Matching**: Support both exact and regex pattern matching
- **Indirect Offsets**: Resolve pointer-based offset calculations

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

    b.iter(|| evaluate_rules(&rules, file_data));
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
  "mime_type": "application/x-executable",
  "description": "ELF 64-bit LSB executable, x86-64, version 1 (SYSV)",
  "confidence": 1.0,
  "matched_rules": [
    "elf",
    "elf64",
    "x86_64"
  ]
}
```

## Error Handling Patterns

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
    data: &[u8],
) -> Result<Option<String>, MagicError> {
    // Implementation
}
```
