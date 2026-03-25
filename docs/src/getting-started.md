# Getting Started

This guide will help you get up and running with libmagic-rs, whether you want to use it as a CLI tool or integrate it into your Rust applications.

## Installation

### Prerequisites

- **Rust 1.91+** (2024 edition)
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
# Identify files using built-in rules (no external magic file needed)
./target/release/rmagic --use-builtin example.bin

# JSON output format
./target/release/rmagic --use-builtin --json example.bin

# Use a custom magic file
./target/release/rmagic --magic-file /usr/share/misc/magic example.bin

# Multiple files
./target/release/rmagic --use-builtin file1.bin file2.pdf file3.zip

# Read from stdin
echo -ne '\x7fELF' | ./target/release/rmagic --use-builtin -

# Help and options
./target/release/rmagic --help
```

### Troubleshooting Detection Issues

Enable debug logging to see when rules are skipped due to evaluation errors (buffer overruns, invalid offsets, type read failures) versus simply not matching:

```bash
# CLI: Enable debug logs for a single invocation
RUST_LOG=debug cargo run -- --use-builtin example.bin

# Or with the compiled binary
RUST_LOG=debug ./target/release/rmagic --use-builtin example.bin
```

Debug output shows which rules the evaluator skips and why, helping diagnose unexpected file detection behavior.

### Library Usage

Add libmagic-rs to your `Cargo.toml`:

```toml
[dependencies]
libmagic-rs = { git = "https://github.com/EvilBit-Labs/libmagic-rs.git" }
```

Basic usage with built-in rules (no external files needed):

```rust,no_run
use libmagic_rs::{LibmagicError, MagicDatabase};

fn main() -> Result<(), LibmagicError> {
    // Use built-in rules compiled into the binary
    let db = MagicDatabase::with_builtin_rules()?;

    // Evaluate a file
    let result = db.evaluate_file("example.bin")?;
    println!("File type: {}", result.description);
    println!("Confidence: {}", result.confidence);

    // Evaluate an in-memory buffer
    let buffer = b"\x7fELF\x02\x01\x01\x00";
    let result = db.evaluate_buffer(buffer)?;
    println!("Buffer type: {}", result.description);

    Ok(())
}
```

## Project Structure

Understanding the project layout will help you navigate the codebase:

```text
libmagic-rs/
├── Cargo.toml              # Project configuration
├── src/
│   ├── lib.rs              # Library API (MagicDatabase, EvaluationConfig, etc.)
│   ├── main.rs             # CLI implementation (rmagic binary)
│   ├── error.rs            # Error types (LibmagicError, ParseError, EvaluationError)
│   ├── parser/
│   │   ├── mod.rs          # Magic file parser entry point
│   │   ├── ast.rs          # AST data structures
│   │   ├── grammar.rs      # nom-based parsing combinators
│   │   ├── loader.rs       # File/directory loading with format detection
│   │   └── format.rs       # Magic file format detection
│   ├── evaluator/
│   │   ├── mod.rs          # Evaluation engine
│   │   ├── offset.rs       # Offset resolution
│   │   ├── operators.rs    # Comparison operators with cross-type coercion
│   │   └── types.rs        # Type interpretation with endianness
│   ├── output/
│   │   ├── mod.rs          # Output types and conversion
│   │   └── json.rs         # JSON/JSON Lines formatting
│   ├── io/
│   │   └── mod.rs          # Memory-mapped I/O (FileBuffer)
│   ├── mime.rs             # MIME type mapping
│   ├── tags.rs             # Semantic tag extraction
│   └── builtin_rules.rs    # Pre-compiled magic rules
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

- **AST Data Structures**: Complete implementation with full serialization
- **Magic File Parser**: nom-based parser for magic file DSL with hierarchical rules
- **Rule Evaluator**: Engine for executing rules against files with graceful error handling
- **Memory-Mapped I/O**: Efficient file access with comprehensive bounds checking
- **CLI Tool (`rmagic`)**: Full-featured CLI with text/JSON output, stdin, timeouts, and built-in rules
- **Built-in Rules**: Pre-compiled detection for common file types (ELF, ZIP, PDF, JPEG, PNG, etc.)
- **MIME Type Mapping**: Opt-in MIME type detection
- **Output Formatters**: Text and JSON output with tag enrichment
- **Strength Calculation**: Rule priority scoring with `!:strength` directives
- **Confidence Scoring**: Match confidence based on rule hierarchy depth
- **Timeout Protection**: Configurable per-file evaluation timeouts
- **Build System**: Cargo configuration with strict clippy pedantic linting
- **Testing**: 940+ comprehensive tests across all modules
- **Documentation**: This guide, API documentation, and architecture docs

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
