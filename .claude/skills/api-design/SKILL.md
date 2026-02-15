---
name: api-design
description: Rust library API design patterns including builder pattern, error handling, trait design, type safety, and CLI design with clap.
---

# API Design Patterns (Rust Library & CLI)

## When to Activate

- Designing or modifying public library API in `lib.rs`
- Adding new public types, traits, or functions
- Reviewing API ergonomics and consistency
- Designing CLI arguments and output formats
- Planning breaking vs non-breaking changes

## Library API Design

### Builder Pattern
```rust
// For types with many optional configuration fields
pub struct EvaluationConfig {
    timeout: Duration,
    max_rules: usize,
    follow_symlinks: bool,
}

impl EvaluationConfig {
    pub fn builder() -> EvaluationConfigBuilder {
        EvaluationConfigBuilder::default()
    }
}

pub struct EvaluationConfigBuilder { /* ... */ }

impl EvaluationConfigBuilder {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<EvaluationConfig, ConfigError> {
        // Validate configuration
        Ok(EvaluationConfig { /* ... */ })
    }
}
```

### Error Design

#### Three-Tier Error Hierarchy
```rust
// Top-level: user-facing errors
pub enum LibmagicError {
    Parse(ParseError),
    Evaluation(EvaluationError),
    Config(ConfigError),
    Io(std::io::Error),
}

// Module-level: specific to subsystem
pub enum ParseError {
    InvalidSyntax { line: usize, reason: String },
    IoError(String),
}

// Always implement std::error::Error + Display
impl std::fmt::Display for LibmagicError { /* ... */ }
impl std::error::Error for LibmagicError { /* ... */ }
```

#### Error Guidelines
- Use `thiserror` for deriving Error implementations
- Errors should be actionable (include line numbers, context)
- Never expose internal paths or system details in public errors
- Implement `From` conversions for ergonomic `?` usage

### Type Safety

#### Newtype Pattern
```rust
// Wrap primitives to prevent misuse
pub struct Offset(i64);
pub struct Score(u32);
pub struct Level(u32);

// Prevents accidentally passing a score where an offset is expected
fn evaluate_at(offset: Offset, buffer: &[u8]) -> Result<Score, EvaluationError>;
```

#### Enum-Based Type Discrimination
```rust
// Use enums to make invalid states unrepresentable
pub enum OffsetSpec {
    Absolute(i64),
    Indirect { base: i64, pointer_type: TypeKind },
    Relative(i64),
    FromEnd(i64),
}
// Cannot have both Absolute and Relative -- the type system prevents it
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
/// * `rules` - Parsed magic rules to evaluate
/// * `buffer` - File contents to identify
///
/// # Returns
/// The best matching result, or `None` if no rules match.
///
/// # Errors
/// Returns `EvaluationError` if evaluation fails due to
/// invalid offsets or corrupted rule definitions.
///
/// # Examples
/// ```
/// use libmagic_rs::MagicDatabase;
///
/// let db = MagicDatabase::default();
/// let result = db.evaluate_buffer(&[0x7f, 0x45, 0x4c, 0x46])?;
/// ```
pub fn evaluate_buffer(&self, buffer: &[u8]) -> Result<Option<EvaluationResult>, EvaluationError>;
```

### Trait Design
```rust
// Small, focused traits
pub trait SafeBufferAccess {
    fn get_byte(&self, offset: usize) -> Option<u8>;
    fn get_slice(&self, offset: usize, len: usize) -> Option<&[u8]>;
    fn len(&self) -> usize;
}

// Implement for multiple types
impl SafeBufferAccess for FileBuffer { /* ... */ }
impl SafeBufferAccess for &[u8] { /* ... */ }
```

## CLI Design (clap)

### Argument Structure
```rust
#[derive(Parser)]
#[command(name = "rmagic", about = "Identify file types")]
struct Args {
    /// Files to identify
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Use custom magic file
    #[arg(long, value_name = "FILE")]
    magic_file: Option<PathBuf>,
}
```

### CLI Conventions
- Follow GNU `file` command conventions where possible
- Short flags for common options (`-j` for JSON)
- Long flags for all options (`--json`)
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
- Adding new enum variants (if `#[non_exhaustive]`)
- Adding new optional fields to builders
- Adding new methods to existing types
- Loosening input constraints

### Breaking Changes (major version)
- Removing or renaming public types/functions
- Changing function signatures
- Adding required fields to structs
- Tightening input constraints
- Changing error types

### Defensive Techniques
```rust
// Mark enums as non-exhaustive for future extension
#[non_exhaustive]
pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
    // Future: Quad, Float, Regex, etc.
}
```

## API Review Checklist

Before exposing new public API:
- [ ] All public items have rustdoc with examples
- [ ] Error types are descriptive and actionable
- [ ] Builder pattern used for types with >3 optional fields
- [ ] Types prevent invalid states at compile time
- [ ] `#[non_exhaustive]` on enums that may grow
- [ ] Consistent naming with existing API surface
- [ ] `From`/`Into` conversions for ergonomic use
- [ ] `Display` and `Debug` implemented for all public types
