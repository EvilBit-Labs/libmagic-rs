---
name: api-design
description: Rust library API design patterns including builder pattern, error handling, trait design, type safety, and CLI design with clap.
---

# API Design Patterns (Rust Library & CLI)

> Code samples below are generic Rust patterns -- placeholder types and
> simplified enum shapes for illustration. Do not treat them as a snapshot
> of the current libmagic-rs API. Actual AST variants (`OffsetSpec`,
> `TypeKind`, `Operator`, `Value`), config fields, and error variants live
> in `src/parser/ast.rs`, `src/config.rs`, and `src/error.rs` respectively.
> See [AGENTS.md](../../AGENTS.md) and [GOTCHAS.md](../../GOTCHAS.md) for
> the authoritative architecture and policy.

## When to Activate

- Designing or modifying public library API in `lib.rs`
- Adding new public types, traits, or functions
- Reviewing API ergonomics and consistency
- Designing CLI arguments and output formats
- Planning breaking vs non-breaking changes

## Library API Design

### Builder Pattern

Use for configuration types that have many optional fields. libmagic-rs
currently uses chainable `with_*` setters on `EvaluationConfig` (see
`src/config.rs`); a dedicated builder type (`MagicDatabase::builder()`)
is planned for v0.6.0 (see AGENTS.md "Development Phases"). General
shape:

```rust
// Illustrative -- the real EvaluationConfig uses chainable setters today
pub struct EvaluationConfig {
    pub timeout_ms: Option<u64>,
    pub max_recursion_depth: u32,
    pub stop_at_first_match: bool,
    // #[non_exhaustive]
}

impl EvaluationConfig {
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: Option<u64>) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}
```

When validation is required, a separate `Builder` with a `build() ->
Result<T, ConfigError>` step makes sense; for libmagic-rs today, validation
runs at construction time inside `EvaluationConfig::new` and the chainable
setters.

### Error Design

#### Three-Tier Error Hierarchy

```rust
// Top-level: user-facing errors -- the actual type is LibmagicError
pub enum LibmagicError {
    Parse(ParseError),
    Evaluation(EvaluationError),
    Config(ConfigError),
    Io(std::io::Error),
}

// Module-level: specific to subsystem.
// Note: ParseError::IoError holds String, not std::io::Error, because
// src/error.rs is shared with build.rs (see GOTCHAS S1.1).
pub enum ParseError {
    InvalidSyntax { line: usize, message: String },
    IoError(String),
}
```

Use `thiserror::Error` to derive `Display` and `Error`.

#### Error Guidelines

- Use `thiserror` for deriving `Error` implementations
- Errors should be actionable (include line numbers, offsets, context)
- Never expose internal paths or system details in public errors
- Implement `From` conversions for ergonomic `?` usage where the source
  error doesn't need additional context

### Type Safety

#### Newtype Pattern

```rust
// Wrap primitives when they carry semantics that shouldn't mix
pub struct Offset(i64);
pub struct Score(u32);

// Prevents accidentally passing a score where an offset is expected
fn evaluate_at(offset: Offset, buffer: &[u8]) -> Result<Score, EvaluationError> { todo!() }
```

#### Enum-Based Type Discrimination

```rust
// Use enums to make invalid states unrepresentable.
// libmagic-rs's actual OffsetSpec is richer than this -- see
// src/parser/ast.rs and GOTCHAS S3.7 for the real shape (Indirect carries
// pointer type + endian, adjustment + op, base/result relative flags,
// etc.). Don't copy this simplified version into code.
pub enum OffsetSpec {
    Absolute(i64),
    Indirect { /* multi-field, see ast.rs */ },
    Relative(i64),
    FromEnd(i64),
}
```

### Public API Surface

#### Minimize Exposure

```rust
// Only expose what users need
pub use crate::evaluator::EvaluationResult;
pub use crate::parser::MagicRule;

// Keep internals private
pub(crate) use crate::evaluator::EvaluationContext;
```

#### Document Everything Public

```rust
/// Evaluate magic rules against a file buffer.
///
/// # Arguments
/// * `buffer` -- file contents to identify
///
/// # Returns
/// The best matching result, or `None` if no rules match.
///
/// # Errors
/// Returns `EvaluationError` if evaluation fails due to invalid offsets
/// or corrupted rule definitions.
///
/// # Examples
/// ```
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use libmagic_rs::MagicDatabase;
///
/// let db = MagicDatabase::default();
/// let _ = db.evaluate_buffer(&[0x7f, 0x45, 0x4c, 0x46])?;
/// # Ok(()) }
/// ```
pub fn evaluate_buffer(&self, buffer: &[u8])
    -> Result<Option<EvaluationResult>, EvaluationError> { todo!() }
```

### Trait Design

```rust
// Small, focused traits
pub trait SafeBufferAccess {
    fn get_byte(&self, offset: usize) -> Option<u8>;
    fn get_slice(&self, offset: usize, len: usize) -> Option<&[u8]>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
}
```

## CLI Design (clap)

### Argument Structure

```rust
#[derive(clap::Parser)]
#[command(name = "rmagic", about = "Identify file types")]
struct Args {
    /// Files to identify
    #[arg(required = true)]
    files: Vec<std::path::PathBuf>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Use custom magic file
    #[arg(long, value_name = "FILE")]
    magic_file: Option<std::path::PathBuf>,
}
```

### CLI Conventions

- Follow GNU `file` command conventions where possible
- Short flags for common options
- Long flags for all options
- Positional arguments for files
- `--` to separate flags from file arguments
- Exit code 0 for success, 1 for errors

### Output Format Consistency

- Text output: `filename: description` (matches GNU `file`)
- JSON output: structured with `filename`, `matches`, `metadata`
- Errors to stderr, results to stdout
- Quiet mode suppresses non-essential output

## API Evolution

### Non-Breaking Changes (patch/minor version)

- Adding new enum variants when the enum is `#[non_exhaustive]`
- Adding new optional fields to `#[non_exhaustive]` structs
- Adding new methods to existing types
- Loosening input constraints

### Breaking Changes (major version)

- Removing or renaming public types/functions
- Changing function signatures
- Adding required fields to non-`#[non_exhaustive]` structs
- Tightening input constraints
- Changing error types

### Defensive Techniques

```rust
// Mark enums and structs as non-exhaustive so adding variants/fields
// later is not a breaking change. libmagic-rs uses this widely; see
// AGENTS.md "v1.0.0" milestone for the planned complete coverage.
#[non_exhaustive]
pub enum TypeKind {
    Byte,
    Short { /* ... */ },
    Long { /* ... */ },
    String { /* ... */ },
    // ... real variants live in src/parser/ast.rs
}
```

## API Review Checklist

Before exposing new public API:

- [ ] All public items have rustdoc with examples that pass `cargo test
      --doc`
- [ ] Error types are descriptive and actionable
- [ ] Builder pattern (or chainable `with_*` setters) used for types with
      >3 optional fields
- [ ] Types prevent invalid states at compile time where possible
- [ ] `#[non_exhaustive]` on enums and structs that may grow
- [ ] Consistent naming with existing API surface
- [ ] `From` / `Into` conversions for ergonomic use
- [ ] `Display` and `Debug` implemented for all public types
