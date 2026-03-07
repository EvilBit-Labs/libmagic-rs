---
name: libmagic-rs-patterns
description: Coding patterns extracted from libmagic-rs repository
version: 1.0.0
source: local-git-analysis
analyzed_commits: 34
---

# libmagic-rs Development Patterns

## Commit Conventions

This project uses **conventional commits** (77% of commits follow the pattern):

- `feat:` - New features (most common, ~30%)
- `chore:` / `chore(deps):` / `chore(ci):` - Maintenance tasks
- `fix:` - Bug fixes
- `refactor:` - Code restructuring
- `docs:` - Documentation updates
- `test:` - Test additions/changes

PR references use `(#N)` suffix format.

## Architecture

### Module Structure

```
src/
  lib.rs             # Public API (MagicDatabase), re-exports
  main.rs            # CLI binary (rmagic), clap-based
  error.rs           # Error types (LibmagicError, ParseError, EvaluationError)
  mime.rs            # MIME type detection
  tags.rs            # Tag-based classification
  build_helpers.rs   # Testable build script logic
  builtin_rules.rs   # Built-in magic rules (compiled at build time)
  builtin_rules.magic # Magic rule definitions
  parser/
    mod.rs           # Parser public interface
    ast.rs           # AST node definitions (OffsetSpec, TypeKind, Operator, etc.)
    grammar.rs       # nom-based parser combinators
  evaluator/
    mod.rs           # Main evaluation engine
    offset.rs        # Offset resolution (absolute, indirect, relative)
    operators.rs     # Comparison and bitwise operations
    types.rs         # Type interpretation with endianness
    strength.rs      # Rule strength calculation
  io/
    mod.rs           # Memory-mapped file I/O
  output/
    mod.rs           # Output formatting interface
    json.rs          # JSON output format
    text.rs          # Text output format
```

### Pipeline Pattern

```
Magic File -> Parser -> AST -> Evaluator -> Match Results -> Output Formatter
                                  ^
Target File -> Memory Mapper -> File Buffer
```

### Core Types

- `MagicDatabase` - Main entry point for loading rules and evaluating files
- `MagicRule` - A single magic rule with offset, type, operator, value, description
- `OffsetSpec` - Absolute, Indirect, Relative, or FromEnd offsets
- `TypeKind` - Byte, Short, Long, String, Regex with endianness
- `Operator` - Equal, NotEqual, Greater, Less, BitwiseAnd, Xor
- `LibmagicError` / `ParseError` / `EvaluationError` - Error hierarchy

## Co-Change Patterns (Files That Change Together)

| File Group | Frequency | Reason |
|------------|-----------|--------|
| `src/lib.rs` + `src/main.rs` | 8x | API changes require CLI updates |
| `src/evaluator/mod.rs` + `src/lib.rs` | 8x | Evaluator changes exposed through lib API |
| `Cargo.toml` + `src/lib.rs` | 6x | Dependency changes affect library code |
| `src/lib.rs` + `src/parser/ast.rs` | 5x | AST changes re-exported through lib |
| `src/main.rs` + `tests/cli_integration.rs` | 4x | CLI changes require test updates |

## Clippy Configuration

Extremely strict clippy setup in `Cargo.toml`:

- `unsafe_code = "forbid"` - No unsafe code allowed
- `warnings = "deny"` - Zero warnings policy
- `unwrap_used = "deny"` - Never use `.unwrap()` in production
- `panic = "deny"` - Never panic in production
- `pedantic`, `nursery`, `cargo` lint groups enabled
- Security-focused lints: `indexing_slicing`, `arithmetic_side_effects`, `string_slice`

## Error Handling Pattern

Use `thiserror` with constructor methods:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid syntax at line {line}: {message}")]
    InvalidSyntax { line: usize, message: String },
}

impl ParseError {
    #[must_use]
    pub fn invalid_syntax(line: usize, message: impl Into<String>) -> Self {
        Self::InvalidSyntax { line, message: message.into() }
    }
}
```

- Every error variant has a named constructor method
- Constructors use `impl Into<String>` for ergonomic creation
- All constructors marked `#[must_use]`
- Error messages include contextual info (line numbers, offsets)

## Testing Patterns

### Test Organization

- **Unit tests**: `#[cfg(test)] mod tests` in each source file
- **Integration tests**: `tests/*.rs` directory
- **Compatibility tests**: `tests/compatibility_tests.rs` against real libmagic test suite
- **Property tests**: `tests/property_tests.rs` using `proptest`
- **Benchmarks**: `benches/*.rs` using `criterion`
- **Snapshots**: `insta` for CLI output snapshots

### Test File Naming

- CLI tests: `tests/cli_integration.rs`
- JSON output: `tests/json_integration_test.rs`
- Parser: `tests/parser_integration_tests.rs`
- Properties: `tests/property_tests.rs`

### Commands

```bash
cargo nextest run --workspace --no-capture    # Standard test run
just ci-check                                  # Full CI parity check
just coverage                                  # Generate coverage report
just test-compatibility                        # Test against original libmagic
```

## Build Script Pattern

Build logic extracted into testable library module:

```rust
// src/build_helpers.rs - testable parsing and code generation
#[cfg(any(test, doc))]
pub mod build_helpers { ... }

// build.rs - minimal, delegates to build_helpers
fn main() { ... }
```

## Tooling

- **mise**: Tool version manager (replaces asdf/rtx)
- **just**: Task runner (justfile)
- **cargo-nextest**: Fast test runner
- **cargo-llvm-cov**: Code coverage
- **insta**: Snapshot testing
- **criterion**: Benchmarking
- **pre-commit**: Git hooks
- **actionlint**: GitHub Actions validation
- **mdbook**: Documentation site
- **prettier**: JSON/YAML formatting
- **mdformat**: Markdown formatting
- **cargo-dist**: Binary distribution

## Rust Edition & Style

- **Edition**: 2024
- **MSRV**: 1.89
- **rustfmt**: `style_edition = "2024"`
- All derive macros: `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`
- Public API types derive `Serialize` + `Deserialize`
- Doc comments on all public items with examples
- `#[non_exhaustive]` on public enums

## Safety Rules

1. Use `.get()` for buffer access, never direct indexing
2. Use `strip_prefix()`/`strip_suffix()` instead of `&str[n..]`
3. No `unwrap()` or `panic!()` in library code
4. Bounds checking on all byte reads
5. Configurable timeouts for evaluation
6. No non-ASCII literals in code/comments
