# Getting Started

## Installation

### Prerequisites

- **Rust 1.91+** (2024 edition)
- **Git** for cloning the repository
- **Cargo** (comes with Rust)

### From Source

libmagic-rs is published on [crates.io](https://crates.io/crates/libmagic-rs) at version 0.6.0. You can also build from source:

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
libmagic-rs = "0.6.0"
```

For the latest development version:

```toml
[dependencies]
libmagic-rs = { git = "https://github.com/EvilBit-Labs/libmagic-rs.git" }
```

**Note**: Version 0.6.0 includes breaking API changes from 0.5.x. If you are upgrading from 0.5.x, see the [migration guide](#migration-from-05x) for details on updating your code.

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

```text
libmagic-rs/
├── Cargo.toml              # Project configuration
├── src/
│   ├── lib.rs              # Library API (MagicDatabase, etc.)
│   ├── main.rs             # CLI implementation (rmagic binary)
│   ├── config.rs           # EvaluationConfig and validation logic
│   ├── error.rs            # Error types (LibmagicError, ParseError, EvaluationError)
│   ├── parser/
│   │   ├── mod.rs          # Magic file parser entry point
│   │   ├── ast.rs          # AST data structures
│   │   ├── grammar/        # nom-based parsing combinators
│   │   │   ├── mod.rs      # Main grammar parser
│   │   │   ├── numbers.rs  # Numeric literal parsing
│   │   │   └── value.rs    # Value literal parsing
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

The library covers the core libmagic feature set:

- Parsing the magic file DSL via a nom-based parser, including hierarchical (`>`, `>>`, …) rules.
- Evaluating rules against files with graceful error handling — buffer overruns, invalid offsets, and type-read failures get skipped instead of aborting the whole match.
- Memory-mapped file access with bounds checks on every read.
- Built-in rules for common formats: ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF.
- Optional MIME type mapping (off by default).
- Text and JSON output, with semantic tag enrichment.
- Strength-based rule ordering, honoring `!:strength` directives.
- Per-file evaluation timeouts you set on `EvaluationConfig`.
- Confidence scoring based on how deep into the rule hierarchy a match goes.

The CLI ships as `rmagic` with text/JSON output, stdin support, configurable timeouts, and a `--use-builtin` mode that needs no external magic file. The repo runs ~1,200 tests per change.

## Example Magic Rules

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

let parsed = parse_text_magic_file(magic_content)?;
assert_eq!(parsed.rules.len(), 1);
assert_eq!(parsed.rules[0].children.len(), 2);
```

### Working with AST Directly

```rust
use libmagic_rs::parser::ast::*;

// Create a simple ELF detection rule using builder methods
let elf_rule = MagicRule::new(
    OffsetSpec::Absolute(0),
    TypeKind::Byte,
    Operator::Equal,
    Value::Uint(0x7f), // First byte of ELF magic
    "ELF executable".to_string(),
).with_level(0);

// Serialize to JSON for inspection
let json = serde_json::to_string_pretty(&elf_rule)?;
println!("{}", json);
```

### Evaluating Rules

```rust
use libmagic_rs::evaluator::{evaluate_rules_with_config, EvaluationContext};
use libmagic_rs::parser::ast::*;
use libmagic_rs::EvaluationConfig;

// Create a rule to detect ELF files using builder methods
let rule = MagicRule::new(
    OffsetSpec::Absolute(0),
    TypeKind::Byte,
    Operator::Equal,
    Value::Uint(0x7f),
    "ELF magic".to_string(),
).with_level(0);

// Evaluate against a buffer
let buffer = &[0x7f, 0x45, 0x4c, 0x46]; // ELF magic bytes
let config = EvaluationConfig::default();
let matches = evaluate_rules_with_config(&[rule], buffer, &config)?;

assert_eq!(matches.len(), 1);
assert_eq!(matches[0].message, "ELF magic");
```

## Testing Your Setup

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

- [AST Data Structures](./ast-structures.md) covers the core types.
- [Architecture Overview](./architecture.md) explains how the pieces fit together.
- [Development Setup](./development.md) is where contributing guidelines live.

Watch the [GitHub repository](https://github.com/EvilBit-Labs/libmagic-rs) for updates.

## Getting Help

Run `cargo doc --open` for the full rustdoc. For bug reports and feature requests, use [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues). [Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions) is the place for open-ended questions.

## Migration from 0.5.x

Version 0.6.0 includes breaking API changes. Key changes to be aware of:

### `parse_text_magic_file` Return Type

The parser now returns `ParsedMagic { rules, name_table }` instead of `Vec<MagicRule>`:

```rust
// Before (0.5.x)
let rules = parse_text_magic_file(content)?;

// After (0.6.0)
let parsed = parse_text_magic_file(content)?;
let rules = parsed.rules;
```

### `MagicRule` Construction

`MagicRule` is now non-exhaustive and has new fields. Use builder methods instead of struct literals:

```rust
// Before (0.5.x)
let rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Byte,
    op: Operator::Equal,
    value: Value::Uint(0x7f),
    message: "ELF magic".to_string(),
    children: vec![],
    level: 0,
};

// After (0.6.0)
let rule = MagicRule::new(
    OffsetSpec::Absolute(0),
    TypeKind::Byte,
    Operator::Equal,
    Value::Uint(0x7f),
    "ELF magic".to_string(),
).with_level(0);
```

### `EvaluationConfig` Construction

`EvaluationConfig` is now non-exhaustive. Use builder methods:

```rust
// Before (0.5.x)
let config = EvaluationConfig {
    max_recursion_depth: 20,
    max_string_length: 1024,
    stop_at_first_match: true,
    mime_types: false,
    timeout_ms: Some(5000),
};

// After (0.6.0)
let config = EvaluationConfig::default()
    .with_max_recursion_depth(20)
    .with_max_string_length(1024)
    .with_stop_at_first_match(true)
    .with_mime_types(false)
    .with_timeout_ms(Some(5000));
```

### Enum Exhaustiveness

Many enums are now marked `#[non_exhaustive]`. Add wildcard patterns in match expressions:

```rust
// Before (0.5.x)
match error {
    LibmagicError::Io(_) => { /* ... */ }
    LibmagicError::Parse(_) => { /* ... */ }
    LibmagicError::Evaluation(_) => { /* ... */ }
}

// After (0.6.0)
match error {
    LibmagicError::Io(_) => { /* ... */ }
    LibmagicError::Parse(_) => { /* ... */ }
    LibmagicError::Evaluation(_) => { /* ... */ }
    _ => { /* handle other cases */ }
}
```

See the [CHANGELOG](https://github.com/EvilBit-Labs/libmagic-rs/blob/main/CHANGELOG.md) for the complete list of changes.
