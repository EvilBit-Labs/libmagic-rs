# GitHub Copilot Instructions for libmagic-rs

## Project Overview

libmagic-rs is a **pure-Rust implementation of libmagic** for file type identification. The project follows a **parser-evaluator architecture** with strict memory safety guarantees and zero unsafe code.

### Development Stage: MVP Phase (v0.1)

- ✅ **Core AST and parser components** are complete with 98 unit tests
- 🔄 **Currently implementing**: Complete magic file rule parsing (`src/parser/mod.rs`)
- 📋 **Next**: Rule evaluation engine (`src/evaluator/`) and output formatters (`src/output/`)

## Architecture Patterns

### Parser-Evaluator Flow

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Module Structure (Follow This Pattern)

- **`src/parser/`**: `ast.rs` (complete), `grammar.rs` (nom parsers), `mod.rs` (rule integration)
- **`src/io/`**: Memory-mapped FileBuffer with comprehensive bounds checking (complete)
- **`src/evaluator/`**: Offset resolution, type interpretation, operators (planned)
- **`src/output/`**: Text and JSON formatters (planned)

## Critical Development Practices

### Memory Safety First

- **NEVER use unsafe code** except in vetted dependencies (`memmap2`, `byteorder`)
- **Always use `.get()` methods** for buffer access, never direct indexing
- Use `SafeBufferAccess` trait pattern from `src/io/mod.rs` for bounds checking

### Zero-Warnings Policy

```bash
cargo clippy -- -D warnings  # Must pass with NO warnings
cargo fmt                    # Required before commit
```

### Quality Standards

- **File size limit**: Keep source files under 500-600 lines
- **Test coverage**: Target >85% with `cargo llvm-cov`
- **All public APIs** require rustdoc with examples
- **Comprehensive error handling** with `thiserror::Error` patterns

## Key Data Structures (src/parser/ast.rs)

### Core AST Types

```rust
pub struct MagicRule {
    pub offset: OffsetSpec,       // Absolute, Indirect, Relative
    pub typ: TypeKind,            // Byte, Short, Long, String, Regex
    pub op: Operator,             // Equal, NotEqual, Greater, Less, BitwiseAnd
    pub value: Value,             // Number, String, Regex with escaping
    pub message: String,          // Output text for matches
    pub children: Vec<MagicRule>, // Hierarchical nesting
    pub level: u32,               // Indentation level
}
```

### Offset Resolution Patterns

- **Absolute**: `OffsetSpec::Absolute(0x10)` for direct file positions
- **Indirect**: Pointer dereferencing with `base_offset`, `pointer_type`, `adjustment`
- **Relative**: `RelativeFrom::Start(pos)` or `RelativeFrom::LastMatch(offset)`

## Development Workflow

### Standard Development Cycle

```bash
cargo check        # Fast syntax/type checking (use frequently)
cargo test         # Run 98 unit tests (currently all passing)
cargo nextest run  # Faster test execution (preferred)
cargo clippy -- -D warnings  # Required - zero warnings policy
cargo fmt          # Code formatting
```

### Testing Patterns (Follow src/parser/grammar.rs)

- **Unit tests**: Use `#[cfg(test)]` modules alongside source
- **Property testing**: Use `proptest` for fuzzing-style tests
- **Error case testing**: Validate all `Result<T, E>` error paths
- **Serialization testing**: All AST types use serde, test round-trip

### Performance Focus

- **Memory-mapped I/O**: Use `FileBuffer` from `src/io/mod.rs` for file access
- **Zero-copy operations**: Minimize allocations during parsing/evaluation
- **Early termination**: Stop evaluation at first match when appropriate

## Parser Implementation Specifics

### nom Parser Patterns (src/parser/grammar.rs)

```rust
use nom::{IResult, bytes::complete::tag, character::complete::digit1};

// Always include overflow protection for numbers
fn parse_decimal_number(input: &str) -> IResult<&str, i64> {
    let (input, digits) = digit1(input)?;
    if digits.len() > 19 { /* handle overflow */ }
    // ... safe parsing
}
```

### String Handling with Escapes

- Support C-style escape sequences: `\n`, `\t`, `\xFF`, `\x20`
- Use `parse_string_content` patterns from `grammar.rs`
- Handle both quoted strings and regex patterns with binary safety

## Error Handling Patterns

### Structured Errors (Follow src/lib.rs)

```rust
#[derive(Debug, thiserror::Error)]
pub enum LibmagicError {
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### Error Recovery

- **Parse errors**: Continue parsing, collect all errors for batch reporting
- **Evaluation errors**: Skip problematic rules, continue with others
- **IO errors**: Proper resource cleanup with RAII patterns

## Magic File Compatibility

### Supported Syntax (Implement in parser/mod.rs)

- **Offsets**: `0x10`, `(0x20.l+4)`, `&0x30`
- **Types**: `byte`, `short`, `long`, `string`, `regex` with endianness (`be`, `le`)
- **Operators**: `=`, `!=`, `>`, `<`, `&` (bitwise AND), `^` (XOR)
- **Nesting**: Hierarchical rules with proper indentation handling

### Binary-Safe Regex

```rust
// Use regex crate with bytes feature for binary-safe matching
use regex::bytes::Regex;
// Handle null bytes and non-UTF8 data properly
```

## Current Implementation Status

### Completed (Don't Reimplement)

- ✅ **AST structures** (`src/parser/ast.rs`) - fully tested with serde
- ✅ **Parser components** (`src/parser/grammar.rs`) - numbers, offsets, operators, values
- ✅ **File I/O** (`src/io/mod.rs`) - memory-mapped FileBuffer with bounds checking
- ✅ **CLI framework** (`src/main.rs`) - clap-based argument parsing

### Active Development (Contribute Here)

- 🔄 **Rule parsing** (`src/parser/mod.rs`) - integrate components into complete rules
- 📋 **Evaluator engine** (`src/evaluator/mod.rs`) - offset resolution, type interpretation
- 📋 **Output formatters** (`src/output/mod.rs`) - text and JSON result formatting

## Code Quality Enforcement

### Linting Configuration (Cargo.toml)

```toml
[workspace.lints.rust]
unsafe_code = "forbid" # Zero unsafe code policy
warnings = "deny"      # Zero warnings policy

[workspace.lints.clippy]
correctness = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
```

### CI/CD Integration

- **Security audits**: `cargo audit` runs daily
- **Dependency scanning**: CodeQL and security workflows
- **Documentation**: mdbook with mermaid diagrams in `docs/`

## Integration Points

### External Dependencies (Key Patterns)

- **memmap2**: Memory-mapped file I/O (safe wrapper usage only)
- **nom**: Parser combinators (follow overflow protection patterns)
- **serde**: Serialization (all AST types implement Serialize/Deserialize)
- **thiserror**: Error handling (structured error types with context)

### Cross-Component Communication

- **Parser → AST**: Clean separation, all parsing returns AST nodes
- **AST → Evaluator**: Rules contain all evaluation context
- **FileBuffer → Evaluator**: Safe buffer access through trait methods
- **Results → Output**: Structured match results for formatters

This guide ensures AI agents understand the project's strict safety requirements, current development focus, and established patterns for immediate productivity.
