# Getting Started

This guide will help you get up and running with libmagic-rs, whether you want to use it as a CLI tool or integrate it into your Rust applications.

## Installation

### Prerequisites

- **Rust 1.85+** (2024 edition)
- **Git** for cloning the repository
- **Cargo** (comes with Rust)

### From Source

Currently, libmagic-rs is only available from source as it's in early development:

```bash
# Clone the repository
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
cd libmagic-rs

# Build the project
cargo build --release

# Run tests to verify installation
cargo test
```

The compiled binary will be available at `target/release/rmagic`.

### Development Build

For development or testing the latest features:

```bash
# Clone and build in debug mode
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
cd libmagic-rs
cargo build

# The debug binary is at target/debug/rmagic
```

## Quick Start

### CLI Usage

> [!NOTE]
> The CLI is currently a placeholder implementation. Full functionality is coming soon.

```bash
# Basic file identification (placeholder output)
./target/release/rmagic example.bin

# JSON output format
./target/release/rmagic example.bin --json

# Help and options
./target/release/rmagic --help
```

**Current Output:**

```bash
$ ./target/release/rmagic README.md
README.md: data
```

### Library Usage

> [!NOTE]
> The library API is currently a placeholder. Full implementation is in progress.

Add libmagic-rs to your `Cargo.toml`:

```toml
[dependencies]
libmagic-rs = { git = "https://github.com/EvilBit-Labs/libmagic-rs.git" }
```

Basic usage example:

```rust
use libmagic_rs::{EvaluationConfig, LibmagicError, MagicDatabase};

fn main() -> Result<(), LibmagicError> {
    // Load magic rules (placeholder - returns empty database)
    let db = MagicDatabase::load_from_file("magic.db")?;

    // Evaluate a file (placeholder - returns "data")
    let result = db.evaluate_file("example.bin")?;

    println!("File type: {}", result.description);
    println!("Confidence: {}", result.confidence);

    if let Some(mime_type) = result.mime_type {
        println!("MIME type: {}", mime_type);
    }

    Ok(())
}
```

## Project Structure

Understanding the project layout will help you navigate the codebase:

```text
libmagic-rs/
├── Cargo.toml              # Project configuration
├── src/
│   ├── lib.rs              # Library API (placeholder)
│   ├── main.rs             # CLI implementation (basic)
│   ├── parser/
│   │   ├── mod.rs          # Parser module (placeholder)
│   │   └── ast.rs          # AST data structures (complete)
│   ├── evaluator/
│   │   └── mod.rs          # Evaluation engine (placeholder)
│   ├── output/
│   │   └── mod.rs          # Output formatting (placeholder)
│   └── io/
│       └── mod.rs          # I/O utilities (placeholder)
├── tests/                  # Integration tests
├── test_files/             # Test magic files and samples
└── docs/                   # This documentation
```

## Development Setup

If you want to contribute or modify the library:

### 1. Clone and Setup

```bash
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
cd libmagic-rs

# Install development dependencies
cargo install cargo-nextest  # Faster test runner
cargo install cargo-watch    # Auto-rebuild on changes
```

### 2. Development Workflow

```bash
# Check code without building
cargo check

# Run tests (fast)
cargo nextest run

# Run tests with coverage
cargo test

# Format code
cargo fmt

# Lint code (strict mode)
cargo clippy -- -D warnings

# Build documentation
cargo doc --open
```

### 3. Continuous Development

```bash
# Auto-rebuild and test on file changes
cargo watch -x check -x test

# Auto-run specific tests
cargo watch -x "test ast_structures"
```

## Current Capabilities

### What Works Now

- ✅ **AST Data Structures**: Complete implementation with full serialization
- ✅ **CLI Framework**: Basic argument parsing and structure
- ✅ **Build System**: Cargo configuration with strict linting
- ✅ **Testing**: Comprehensive unit tests for AST components
- ✅ **Documentation**: This guide and API documentation

### What's Coming Soon

- 🔄 **Magic File Parser**: nom-based parser for magic file DSL
- 🔄 **Rule Evaluator**: Engine for executing rules against files
- 🔄 **File I/O**: Memory-mapped file access
- 🔄 **Output Formatters**: Text and JSON result formatting

## Example Magic Rules

While the parser isn't implemented yet, you can work with AST structures directly:

```rust
use libmagic_rs::parser::ast::*;

// Create a simple ELF detection rule
let elf_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Long {
        endian: Endianness::Little,
        signed: false
    },
    op: Operator::Equal,
    value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]), // "\x7fELF"
    message: "ELF executable".to_string(),
    children: vec![],
    level: 0,
};

// Serialize to JSON for inspection
let json = serde_json::to_string_pretty(&elf_rule)?;
println!("{}", json);
```

## Testing Your Setup

Verify everything is working correctly:

```bash
# Run all tests
cargo test

# Run specific AST tests
cargo test ast_structures

# Check code quality
cargo clippy -- -D warnings

# Verify documentation builds
cargo doc

# Test CLI (placeholder functionality)
cargo run -- README.md
```

## Next Steps

1. **Explore the AST**: Check out [AST Data Structures](./ast-structures.md) to understand the core types
2. **Read the Architecture**: See [Architecture Overview](./architecture.md) for the big picture
3. **Follow Development**: Watch the [GitHub repository](https://github.com/EvilBit-Labs/libmagic-rs) for updates
4. **Contribute**: See [Development Setup](./development.md) for contribution guidelines

## Getting Help

- **Documentation**: This guide covers all current functionality
- **API Reference**: Run `cargo doc --open` for detailed API docs
- **Issues**: [Report bugs or request features](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- **Discussions**: [Ask questions or share ideas](https://github.com/EvilBit-Labs/libmagic-rs/discussions)

The project is in active development, so check back regularly for new features and capabilities!
