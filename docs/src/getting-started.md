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

```bash
# Basic file identification
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

Add libmagic-rs to your `Cargo.toml`:

```toml
[dependencies]
libmagic-rs = { git = "https://github.com/EvilBit-Labs/libmagic-rs.git" }
```

Basic usage example:

```rust
use libmagic_rs::{EvaluationConfig, LibmagicError, MagicDatabase};

fn main() -> Result<(), LibmagicError> {
    // Load magic rules from a magic file or directory
    let db = MagicDatabase::load_from_file("magic.db")?;

    // Evaluate a file against the loaded rules
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
├── CONTRIBUTING.md         # Contribution guidelines
├── src/
│   ├── lib.rs              # Library API with EvaluationConfig
│   ├── main.rs             # CLI implementation (basic)
│   ├── error.rs            # Error types (ParseError, EvaluationError, etc.)
│   ├── parser/
│   │   ├── mod.rs          # Magic file parser ✅ Complete
│   │   ├── ast.rs          # AST data structures ✅ Complete
│   │   └── grammar.rs      # nom-based parsing combinators ✅ Complete
│   ├── evaluator/
│   │   ├── mod.rs          # Evaluation engine ✅ Complete
│   │   ├── offset.rs       # Offset resolution ✅ Complete
│   │   ├── operators.rs    # Comparison operators ✅ Complete
│   │   └── types.rs        # Type interpretation ✅ Complete
│   ├── output/
│   │   └── mod.rs          # Output formatting
│   └── io/
│       └── mod.rs          # Memory-mapped I/O ✅ Complete
├── tests/                  # Integration tests
├── third_party/            # Canonical libmagic tests and magic files
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
- ✅ **Magic File Parser**: nom-based parser for magic file DSL with hierarchical rules
- ✅ **Rule Evaluator**: Engine for executing rules against files with graceful error handling
- ✅ **Memory-Mapped I/O**: Efficient file access with comprehensive bounds checking
- ✅ **CLI Framework**: Basic argument parsing and structure
- ✅ **Build System**: Cargo configuration with strict linting
- ✅ **Testing**: Comprehensive unit tests for all modules
- ✅ **Documentation**: This guide, API documentation, and architecture docs

### What's Coming Soon

- 🔄 **Indirect Offsets**: Support for offset indirection in magic rules
- 🔄 **Output Formatters**: Text and JSON result formatting
- 🔄 **MIME Type Mapping**: Automatic MIME type detection
- 🔄 **Rule Caching**: Pre-compiled rule database support

## Example Magic Rules

You can parse magic rules from text or work with AST structures directly:

### Parsing Magic Files

```rust
use libmagic_rs::parser::parse_text_magic_file;

// Parse a simple magic file
let magic_content = r#"
# ELF file format
0 string \x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
"#;

let rules = parse_text_magic_file(magic_content)?;
assert_eq!(rules.len(), 1);
assert_eq!(rules[0].children.len(), 2);
```

### Working with AST Directly

```rust
use libmagic_rs::parser::ast::*;

// Create a simple ELF detection rule
let elf_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Byte,
    op: Operator::Equal,
    value: Value::Uint(0x7f), // First byte of ELF magic
    message: "ELF executable".to_string(),
    children: vec![],
    level: 0,
};

// Serialize to JSON for inspection
let json = serde_json::to_string_pretty(&elf_rule)?;
println!("{}", json);
```

### Evaluating Rules

```rust
use libmagic_rs::evaluator::{evaluate_rules_with_config, EvaluationContext};
use libmagic_rs::parser::ast::*;
use libmagic_rs::EvaluationConfig;

// Create a rule to detect ELF files
let rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Byte,
    op: Operator::Equal,
    value: Value::Uint(0x7f),
    message: "ELF magic".to_string(),
    children: vec![],
    level: 0,
};

// Evaluate against a buffer
let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
let config = EvaluationConfig::default();
let matches = evaluate_rules_with_config(&[rule], buffer, &config)?;

assert_eq!(matches.len(), 1);
assert_eq!(matches[0].message, "ELF magic");
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

# Test CLI
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
