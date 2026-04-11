# GitHub Copilot Instructions for libmagic-rs

## Project Overview

libmagic-rs is a **pure-Rust implementation of libmagic** for file type identification. The project follows a **parser-evaluator architecture** with strict memory safety guarantees and zero unsafe code.

### Development Stage: v0.5.0 (Active Development)

- ✅ **Parser**: Complete with AST structures, grammar parsing, type handling, and hierarchy support
- ✅ **Evaluator**: Fully implemented with offset resolution, type interpretation, operator application, and strength calculation
- ✅ **Output**: Text and JSON formatters with comprehensive metadata
- ✅ **CLI**: Full-featured `rmagic` binary with multiple file support, stdin, built-in rules, and custom magic files
- ✅ **Offsets**: Absolute, from-end, indirect, and relative offset resolution all implemented
- ✅ **Advanced types**: Regex (`regex::bytes`) with `/c`, `/s`, `/l` flags and `RegexCount` variants, and bounded literal `search` with mandatory `NonZeroUsize` range

## Architecture Patterns

### Parser-Evaluator Flow

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Module Structure (Follow This Pattern)

- **`src/parser/`**: Complete parsing system
  - `ast.rs`: AST node definitions (MagicRule, TypeKind, Operator, Value, OffsetSpec)
  - `grammar/`: nom-based parser combinators for magic file syntax
  - `types.rs`: Type keyword parsing and validation
  - `hierarchy.rs`: Hierarchical rule structure handling
  - `loader.rs`: Magic file loading and preprocessing
  - `codegen.rs`: Serialization for build-time rule compilation
- **`src/evaluator/`**: Rule evaluation engine
  - `engine/`: Core evaluation logic and rule matching
  - `offset/`: Offset resolution (absolute, from-end, indirect, relative) -- all fully implemented
  - `operators/`: Operator application (equality, comparison, bitwise)
  - `types/`: Type interpretation with endianness handling (includes `regex` and `search` submodules)
  - `strength.rs`: Confidence scoring and strength modifiers
- **`src/io/`**: Memory-mapped FileBuffer with SafeBufferAccess trait for bounds checking
- **`src/output/`**: Result formatting (text.rs, json.rs) with metadata support

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
cargo test         # Run 1,068+ unit tests (currently all passing)
cargo nextest run  # Faster test execution (preferred)
cargo clippy -- -D warnings  # Required - zero warnings policy
cargo fmt          # Code formatting
```

### Testing Patterns (Follow src/parser/grammar.rs)

- **Unit tests**: Use `#[cfg(test)]` modules alongside source
- **Property testing**: Use `proptest` for fuzzing-style tests
- **Error case testing**: Validate all `Result<T, E>` error paths
- **Serialization testing**: All AST types use serde, test round-trip
- **Table-driven tests**: Consolidate related test cases with descriptive failure messages

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

### Supported Syntax (Currently Implemented in v0.5.0)

- **Offsets**: Absolute, from-end, indirect, and relative (all fully evaluated; relative offsets use GNU `file` previous-match anchor semantics)
- **Types**: `byte`, `short`, `long`, `quad`, `float`, `double`, `string`, `pstring`, `regex`, `search` with endianness and flag support. Unsigned variants, signed variants, date/timestamp variants as documented in AGENTS.md.
- **Operators**: `=` (equal), `!=` (not equal), `<` (less than), `>` (greater than), `<=` (less equal), `>=` (greater equal), `&` (bitwise AND with optional mask), `^` (bitwise XOR), `~` (bitwise NOT), `x` (any value)
- **Nesting**: Hierarchical rules with proper indentation handling
- **String Matching**: Exact string matching with null-termination and Pascal string (length-prefixed) support
- **Regex**: Binary-safe matching via `regex::bytes::Regex`. `/c` and `/s` live on `RegexFlags`; `/l` is encoded by the `RegexCount::Lines` variant of `TypeKind::Regex::count`. Scan window dispatches on `RegexCount::Default` (plain `regex`, 8192-byte cap), `RegexCount::Bytes(NonZeroU32)` (`regex/N`), or `RegexCount::Lines(Option<NonZeroU32>)` (`regex/Nl` or `regex/l`). All variants capped at `evaluator::types::regex::REGEX_MAX_BYTES` (8192).
- **Search**: Bounded literal scan via `memchr::memmem::find`; mandatory `NonZeroUsize` range; match-end anchor advance
- **Directives**: `!:strength` modifier (parsed and applied)

### Planned Features (v1.0+)

- Additional directives: `!:mime`, `!:ext`, `!:apple`
- Named tests (`use`/`name` directives)
- Aho-Corasick multi-pattern optimization for search rules
- Compiled-regex caching

### Binary-Safe Regex

Regex matching is implemented via `regex::bytes::Regex` (see `src/evaluator/types/regex.rs`). `regex::bytes` handles null bytes and non-UTF8 data natively; matched bytes are converted to `Value::String` via `String::from_utf8_lossy` so binary matches surface U+FFFD replacement characters in the display.

## Current Implementation Status

### Completed (Don't Reimplement)

- ✅ **AST structures** (`src/parser/ast.rs`) - fully tested with serde
- ✅ **Parser components** (`src/parser/grammar/`) - complete magic file syntax parsing
- ✅ **Type system** (`src/parser/types.rs`) - byte, short, long, quad, float, double, string, pstring, regex, search, date types
- ✅ **File I/O** (`src/io/mod.rs`) - memory-mapped FileBuffer with bounds checking
- ✅ **CLI framework** (`src/main.rs`) - clap-based argument parsing with JSON output
- ✅ **Evaluator engine** (`src/evaluator/`) - complete rule evaluation with strength calculation
- ✅ **Output formatters** (`src/output/`) - text and JSON formatters with metadata

### Recently Completed

- ✅ **Indirect offsets** (`src/evaluator/offset/indirect.rs`) - fully implemented (#37)
- ✅ **Relative offsets** (`src/evaluator/offset/relative.rs`) - fully implemented with previous-match anchor (#38)
- ✅ **Regex type** (`src/evaluator/types/regex.rs`) - binary-safe via `regex::bytes` with `/c`, `/s`, `/l` flags (#39)
- ✅ **Search type** (`src/evaluator/types/search.rs`) - bounded literal scan via `memchr::memmem::find` with mandatory `NonZeroUsize` range (#39)
- ✅ **Pascal strings** - implemented (#43)

### Active Development (Contribute Here)

- 📋 **Additional directives**: `!:mime`, `!:ext`, `!:apple`
- 📋 **Named tests**: `use`/`name` directives
- 📋 **Aho-Corasick optimization** for multi-pattern search
- 📋 **Compiled-regex caching** for repeated evaluation

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

## Common Tasks and Patterns

### Adding New Type Support

> **Note:** Currently implemented types are `Byte`, `Short`, `Long`, `Quad`, `Float`, `Double`, `String`, `PString`, `Regex`, `Search`, and date/timestamp variants.

1. Extend `TypeKind` enum in `src/parser/ast.rs`
2. Add keyword parsing in `src/parser/types.rs` (`parse_type_keyword` and `type_keyword_to_kind`)
3. Add value/operator parsing in `src/parser/grammar/mod.rs` if needed
4. Implement reading logic in `src/evaluator/types/` submodules
5. Update `serialize_type_kind()` in `src/parser/codegen.rs`
6. Add tests for the new type
7. Update documentation

### Adding New Operators

> **Note:** Currently implemented operators are `Equal`, `NotEqual`, `LessThan`, `GreaterThan`, `LessEqual`, `GreaterEqual`, `BitwiseAnd` (with `BitwiseAndMask`), `BitwiseXor`, `BitwiseNot`, and `AnyValue`.

1. Extend `Operator` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar/mod.rs`
3. Implement operator logic in `src/evaluator/operators/` submodule
4. Update `serialize_operator()` in `src/parser/codegen.rs`
5. Update strength calculation match in `src/evaluator/strength.rs`
6. Update `arb_operator()` in `tests/property_tests.rs`
7. Add tests for the new operator
8. Update documentation

This guide ensures AI agents understand the project's strict safety requirements, current development focus, and established patterns for immediate productivity.
