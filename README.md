# libmagic-rs

A pure-Rust clean-room implementation of libmagic, the library that powers the `file` command for identifying file types. This project provides a memory-safe, efficient alternative to the C-based libmagic library.

> **Note**: This is a clean-room implementation inspired by the original [libmagic](https://www.darwinsys.com/file/) project. We respect and acknowledge the original work by Ian Darwin and the current maintainers led by Christos Zoulas.

## Overview

libmagic-rs is designed to replace libmagic with a safe, efficient Rust implementation that:

- **Memory Safety**: Pure Rust with no unsafe code (except vetted crates)
- **Performance**: Uses memory-mapped I/O for efficient file reading
- **Compatibility**: Supports common magic file syntax (offsets, types, operators, nesting)
- **Extensibility**: Designed for modern use cases (PE resources, Mach-O, Go build info)
- **Multiple Output Formats**: Classic text output and structured JSON

## Features

### Core Capabilities

- Parse magic files (DSL for byte-level file type detection)
- Evaluate magic rules against file buffers to identify file types
- Support for absolute, indirect, and relative offset specifications
- Multiple data types: byte, short, long, string, regex patterns
- Hierarchical rule evaluation with proper nesting
- Memory-mapped file I/O for efficient processing

### Output Formats

**Text Output (Default)**:

```text
ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

**JSON Output**:

```json
{
  "filename": "example.bin",
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

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/libmagic-rs.git
cd libmagic-rs

# Build the project
cargo build --release

# Run tests
cargo test
```

### CLI Usage

```bash
# Basic file identification
./target/release/rmagic file.bin

# JSON output
./target/release/rmagic file.bin --json

# Use custom magic file
./target/release/rmagic file.bin --magic-file custom.magic

# Multiple files
./target/release/rmagic file1.bin file2.exe file3.pdf
```

### Library Usage

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

// Load magic rules
let db = MagicDatabase::load_from_file("magic/standard.magic")?;

// Configure evaluation
let config = EvaluationConfig {
    max_recursion_depth: 10,
    stop_at_first_match: true,
    enable_mime_types: true,
    ..Default::default()
};

// Identify file type
let result = db.evaluate_file("example.bin", &config)?;
println!("File type: {}", result.primary_match().message);
```

## Architecture

The project follows a parser-evaluator architecture:

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Core Modules

- **Parser** (`src/parser/`): Magic file DSL parsing into Abstract Syntax Tree
- **Evaluator** (`src/evaluator/`): Rule evaluation engine with offset resolution
- **Output** (`src/output/`): Text and JSON output formatting
- **IO** (`src/io/`): Memory-mapped file access and buffer management

### Key Data Structures

```rust
pub struct MagicRule {
    pub offset: OffsetSpec,
    pub typ: TypeKind,
    pub op: Operator,
    pub value: Value,
    pub message: String,
    pub children: Vec<MagicRule>,
}

pub enum OffsetSpec {
    Absolute(i64),
    Indirect { base_offset: i64, pointer_type: TypeKind, adjustment: i64 },
    Relative(i64),
    FromEnd(i64),
}

pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { encoding: StringEncoding, max_length: Option<usize> },
    Regex { flags: RegexFlags },
}
```

## Development

### Prerequisites

- Rust 1.85+ (2024)
- Cargo
- Git

### Building

```bash
# Development build
cargo build

# Release build with optimizations
cargo build --release

# Check without building
cargo check
```

### Testing

```bash
# Run all tests
cargo test

# Run with nextest (faster test runner)
cargo nextest run

# Run specific test
cargo test test_name

# Run integration tests
cargo test --test integration

# Run compatibility tests against original file project
cargo test --test compatibility
```

### Compatibility Testing

We maintain strict compatibility with the original [file project](https://github.com/file/file/blob/7ed3febfcd616804a2ec6495b3e5f9ccb6fc5f8f/tests/README) by testing against their complete test suite. This ensures our implementation produces identical results to the original libmagic library.

The compatibility test suite includes:

- All test files from the original file project
- Expected output validation against GNU file command
- Performance regression testing
- Edge case handling verification

### Code Quality

```bash
# Format code
cargo fmt

# Lint code (strict mode)
cargo clippy -- -D warnings

# Generate documentation
cargo doc --open

# Run benchmarks
cargo bench
```

### Project Structure

```text
libmagic-rs/
├── Cargo.toml              # Project manifest and dependencies
├── src/
│   ├── lib.rs              # Library root and public API
│   ├── main.rs             # CLI binary entry point
│   ├── parser/              # Magic file parser module
│   ├── evaluator/           # Rule evaluation engine
│   ├── output/              # Output formatting
│   ├── io/                  # Memory-mapped file I/O
│   └── error.rs             # Error types and handling
├── tests/                   # Integration tests
├── benches/                 # Performance benchmarks
├── magic/                   # Magic file databases
└── docs/                    # Documentation
```

## Performance

The implementation is optimized for performance with:

- **Memory-mapped I/O**: Efficient file access without loading entire files
- **Zero-copy operations**: Minimize allocations during evaluation
- **Aho-Corasick indexing**: Fast multi-pattern string search
- **Rule caching**: Compiled magic rules for repeated use
- **Early termination**: Stop evaluation at first match when appropriate

### Benchmarks

Performance targets:

- Match or exceed libmagic performance within 10%
- Memory usage comparable to libmagic
- Fast startup with large magic databases

## Compatibility

### Magic File Support

- Standard magic file syntax (offsets, types, operators)
- Hierarchical rule nesting with indentation
- Endianness handling for multi-byte types
- String matching and regex patterns
- Indirect offset resolution

### Migration from libmagic

The library provides a migration path from C-based libmagic:

- Similar API patterns where possible
- Comprehensive migration guide in documentation
- Compatibility testing with GNU `file` command results
- Performance parity validation

## Security

- **Memory Safety**: No unsafe code except in vetted dependencies
- **Bounds Checking**: All buffer access protected by bounds checking
- **Safe File Handling**: Graceful handling of truncated/corrupted files
- **Fuzzing Integration**: Robustness testing with malformed inputs

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests and ensure they pass (`cargo test`)
5. Run clippy to check for issues (`cargo clippy -- -D warnings`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Development Guidelines

- Follow Rust naming conventions
- Add tests for new functionality
- Update documentation for API changes
- Ensure all code passes `cargo clippy -- -D warnings`
- Maintain >85% test coverage

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Roadmap

### Phase 1: MVP (v0.1)

- Basic magic file parsing and evaluation
- CLI interface with text/JSON output
- Memory-mapped file I/O
- Core data types (byte, short, long, string)

### Phase 2: Enhanced Features (v0.2)

- Indirect offset resolution
- Regex support with binary-safe matching
- Compiled rule caching
- Additional operators and type support

### Phase 3: Performance & Compatibility (v0.3)

- Performance optimizations
- Full libmagic syntax compatibility
- Comprehensive test suite
- MIME type mapping

### Phase 4: Production Ready (v1.0)

- Stable API
- Complete documentation
- Migration guide
- Performance parity validation

## Support

- **Documentation**: [Project Documentation](docs/)
- **Issues**: [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)

## Acknowledgments

- [Ian Darwin](https://www.darwinsys.com/file/) for the original file command and libmagic implementation
- [Christos Zoulas](https://www.darwinsys.com/file/) and the current libmagic maintainers
- The original libmagic project for establishing the magic file format standard
- Rust community for excellent tooling and ecosystem
- Contributors and testers who help improve the project
