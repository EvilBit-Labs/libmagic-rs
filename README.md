# libmagic-rs

[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/11947/badge)](https://www.bestpractices.dev/projects/11947)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/EvilBit-Labs/libmagic-rs/badge)](https://scorecard.dev/viewer/?uri=github.com/EvilBit-Labs/libmagic-rs)
[![Crates.io](https://img.shields.io/crates/v/libmagic-rs)](https://crates.io/crates/libmagic-rs)
[![License](https://img.shields.io/crates/l/libmagic-rs)](https://github.com/EvilBit-Labs/libmagic-rs/blob/main/LICENSE)

A pure-Rust implementation of libmagic, the library that powers the `file` command for identifying file types. This project provides a memory-safe, efficient alternative to the C-based libmagic library.

> [!NOTE]
> This is a clean-room implementation inspired by the original [libmagic](https://www.darwinsys.com/file/) project. We respect and acknowledge the original work by Ian Darwin and the current maintainers led by Christos Zoulas.

## Project Status

**Active Development (Phase 1 MVP)** - The core file identification pipeline is functional. You can identify common file types using text magic files today.

**Current Metrics:**

- 17,000+ lines of Rust code
- 650+ tests with comprehensive coverage
- Zero unsafe code with memory safety guarantees
- Zero warnings with strict clippy linting

### What Works Today

- **File type identification** - Identify files using text magic file databases
- **Text and JSON output** - Both output formats supported via `--json` flag
- **Custom magic files** - Use `--magic-file` to specify your own rules
- **Memory-mapped I/O** - Efficient file reading with bounds checking
- **Hierarchical rule matching** - Full nested rule evaluation
- **Platform detection** - Automatic magic file discovery on Unix systems

### In Progress (Phase 1 Completion)

- Multiple file support - Process multiple files in one command
- Stdin input - Pipe data via `rmagic -`
- Built-in fallback rules - Work without external magic files via `--use-builtin`
- Magdir directory loading - Load all files from a magic directory
- Compatibility testing - Validation against GNU `file` command output

### Phase 1 Goals

- 95%+ compatibility with GNU `file` for common file types
- >85% test coverage across all modules
- Complete documentation with rustdoc and mdbook site

## Overview

libmagic-rs is designed to replace libmagic with a safe, efficient Rust implementation that:

- **Memory Safety**: Pure Rust with no unsafe code (except vetted crates)
- **Performance**: Uses memory-mapped I/O for efficient file reading
- **Compatibility**: Supports common magic file syntax (offsets, types, operators, nesting)
- **Extensibility**: Designed for modern use cases (PE resources, Mach-O, Go build info)
- **Multiple Output Formats**: Classic text output and structured JSON

## Features

### Core Capabilities

- Parse text magic files (DSL for byte-level file type detection)
- Evaluate magic rules against file buffers to identify file types
- Absolute offset specifications (indirect/relative in Phase 2)
- Multiple data types: byte, short, long, quad, string
- Hierarchical rule evaluation with proper nesting
- Memory-mapped file I/O for efficient processing
- Confidence scoring based on match depth

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
      "tags": [
        "executable",
        "elf"
      ],
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
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
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

# JSON output with metadata
./target/release/rmagic file.bin --json

# Use custom magic file
./target/release/rmagic file.bin --magic-file custom.magic
```

> [!NOTE]
> Multiple file support (`rmagic file1.bin file2.bin`) and stdin input (`cat file | rmagic -`) are planned for Phase 1 completion.

### Library Usage

```rust
use libmagic_rs::MagicDatabase;

// Load magic rules from a text magic file
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;

// Identify file type
let result = db.evaluate_file("example.bin")?;
println!("File type: {}", result.description);
println!("Confidence: {:.0}%", result.confidence * 100.0);

// Or evaluate an in-memory buffer
let buffer = std::fs::read("example.bin")?;
let result = db.evaluate_buffer(&buffer)?;
if let Some(mime) = result.mime_type {
    println!("MIME type: {}", mime);
}
```

> [!NOTE]
> The library currently supports text-format magic files. Binary `.mgc` format support is planned for Phase 2, following the proven OpenBSD approach of parsing text format directly.

## Architecture

The project follows a parser-evaluator architecture:

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Core Modules

- **Parser** (`src/parser/`): Magic file DSL parsing into Abstract Syntax Tree
  - `ast.rs`: Core AST data structures
  - `grammar.rs`: nom-based parsing components
  - `mod.rs`: Parser interface with text magic file support
- **Evaluator** (`src/evaluator/`): Rule evaluation engine
  - Offset resolution (absolute offsets supported, indirect in Phase 2)
  - Type interpretation with endianness handling
  - Comparison and bitwise operations
  - Confidence scoring based on match depth
- **Output** (`src/output/`): Result formatting
  - Text formatter (GNU `file` compatible)
  - JSON formatter with metadata
- **IO** (`src/io/`): File access utilities
  - Memory-mapped file buffers with FileBuffer
  - Safe bounds checking with comprehensive error handling
  - Resource management with RAII patterns

### Key Data Structures

```rust
pub struct MagicRule {
    pub offset: OffsetSpec,
    pub typ: TypeKind,
    pub op: Operator,
    pub value: Value,
    pub message: String,
    pub children: Vec<MagicRule>,
    pub level: u32,
}

pub enum OffsetSpec {
    Absolute(i64),
    Indirect {
        base_offset: i64,
        pointer_type: TypeKind,
        adjustment: i64,
        endian: Endianness,
    },
    Relative(i64),
    FromEnd(i64),
}

pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
}

pub enum Value {
    Uint(u64),
    Int(i64),
    Bytes(Vec<u8>),
    String(String),
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
# Run all tests (650+ tests)
cargo test

# Run with nextest (faster test runner)
cargo nextest run

# Run specific test module
cargo test parser::grammar::tests
cargo test parser::ast::tests

# Test with coverage reporting
cargo llvm-cov --html

# Run compatibility tests against GNU file
cargo test --test compatibility
```

**Current Test Coverage:**

- 650+ tests covering parser, evaluator, I/O, and CLI components
- Parser testing for numbers, offsets, operators, values, and rule hierarchies
- Evaluator testing for rule matching and confidence scoring
- I/O testing for FileBuffer, memory mapping, and error handling
- CLI testing for argument parsing and output formatting
- Compatibility testing against GNU `file` command output
- Target: >85% test coverage for Phase 1 completion

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

The implementation includes:

- **Memory-mapped I/O**: Efficient file access without loading entire files
- **Zero-copy operations**: Minimize allocations during evaluation
- **Early termination**: Stop evaluation at first match when appropriate

**Planned optimizations (Phase 2+):**

- Aho-Corasick indexing for fast multi-pattern string search
- Compiled rule caching for repeated use
- Performance benchmarking against libmagic

### Benchmarks

Performance targets (Phase 3):

- Match or exceed libmagic performance within 10%
- Memory usage comparable to libmagic
- Fast startup with large magic databases

## Compatibility

### Magic File Support

**Supported (Phase 1):**

- Text magic file format (the stable, documented format)
- Hierarchical rule nesting with indentation levels
- Absolute offset specifications
- Core types: byte, short, long, quad, string
- Core operators: `=`, `!=`, `&`, `<`, `>`
- Endianness handling for multi-byte types
- Magdir-style directory loading

**Phase 2:**

- Binary `.mgc` compiled format
- Indirect offset resolution
- Regex patterns

### Text-First Approach

libmagic-rs follows the **OpenBSD approach**: parse text magic files directly, prioritizing simplicity and correctness over binary format complexity. This is the same strategy used by OpenBSD's `file` implementation and other successful reimplementations like PolyFile.

**Why text format first?**

- Text magic format is stable across libmagic versions
- Binary `.mgc` has version lock-in issues (format changes between releases)
- Simpler codebase (~1,500 lines vs ~3,000 for binary parsing)
- Easier debugging and testing

### Migration from libmagic

The library provides a migration path from C-based libmagic:

- Similar API patterns where possible
- Compatibility testing with GNU `file` command results
- Text magic files work unchanged from system installations

## Security

- **Memory Safety**: No unsafe code except in vetted dependencies
- **Bounds Checking**: All buffer access protected by bounds checking
- **Safe File Handling**: Graceful handling of truncated/corrupted files
- **Fuzzing Integration**: Robustness testing with malformed inputs

### Verifying Releases

All release artifacts are cryptographically signed via [Sigstore](https://www.sigstore.dev/) using GitHub Attestations. To verify a downloaded artifact:

```bash
gh attestation verify <artifact> --repo EvilBit-Labs/libmagic-rs
```

See the [release verification guide](https://evilbitlabs.io/libmagic-rs/release-verification.html) for details.

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

See [ROADMAP.md](ROADMAP.md) for the full roadmap with linked issues, or [GitHub Milestones](https://github.com/EvilBit-Labs/libmagic-rs/milestones) for detailed issue tracking.

| Milestone | Focus |
|-----------|-------|
| **v0.1.0** (current) | MVP: parser, evaluator, CLI, built-in rules, 94%+ test coverage |
| **v0.2.0** | Comparison operators, bitwise XOR/NOT, indirect/relative offsets, 64-bit integers |
| **v0.3.0** | Regex, float/double, date/timestamp, pascal strings, meta-types |
| **v0.4.0** | Builder API, JSON metadata, parse warnings, improved errors |
| **v1.0.0** | 95%+ GNU `file` compatibility, stable API, crates.io publication |

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
