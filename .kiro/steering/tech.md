# Technical Stack

## Language & Toolchain

- **Language**: Rust (latest stable)
- **Build System**: Cargo
- **Package Manager**: Cargo

## Core Dependencies

- `memmap2`: Memory-mapped file I/O for efficient reads
- `byteorder`: Endianness conversions for multi-byte values
- `bstr`: Binary-safe string handling
- `regex` or `onig`: Regex matching (prefer binary-safe options)
- `aho-corasick`: Fast multi-pattern string search
- `serde` + `serde_json`: Serialization for JSON output and compiled rules
- `nom` or `pest`: Parser combinators for magic file DSL

## Architecture Patterns

- **Parser-Evaluator Pattern**: Separate parsing of magic files from rule evaluation
- **AST-based Processing**: Magic files parsed into Abstract Syntax Tree
- **Memory-mapped I/O**: Use mmap for efficient file access
- **Zero-copy Where Possible**: Minimize allocations during evaluation
- **Hierarchical Rule Matching**: Parent rules must match before children are evaluated

## Testing & Build Tools

- **Rust** - Primary language for performance and memory safety
- **Cargo** - Build system for Rust projects
- **cargo-nextest** - Test runner for faster, more reliable test execution
- **llvm-cov** - for coverage measurement and reporting (target: >85%)
- **insta** - for deterministic CLI output validation
- **criterion** - Performance benchmarks for critical path components

## Common Commands

### Development

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run with optimizations
cargo build --release

# Format code
cargo fmt

# Lint code
cargo clippy

# Check without building
cargo check
```

### CLI Usage (Future)

```bash
# Basic file identification
rmagic file.bin

# JSON output
rmagic file.bin --json

# Text output (default)
rmagic file.bin --text

# Use custom magic file
rmagic file.bin --magic-file custom.magic
```

## Performance Considerations

- Use `mmap` for file access to avoid loading entire files into memory
- Implement rule caching for compiled magic files
- Consider Aho-Corasick indexing for string patterns
- Avoid unnecessary allocations in hot paths
- Profile with `cargo bench` for performance regressions

## Safety Requirements

- No `unsafe` code except in vetted dependencies
- Bounds checking for all buffer access
- Safe handling of truncated/corrupted files
- Fuzzing integration for robustness testing
