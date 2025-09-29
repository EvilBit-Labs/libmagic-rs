# API Reference

> [!NOTE]
> This API reference describes the planned interface. The current implementation has placeholder functionality.

Complete API documentation for libmagic-rs library components.

## Core Types

### MagicDatabase

Main interface for loading and using magic rules.

```rust
pub struct MagicDatabase {/* ... */}

impl MagicDatabase {
    /// Load magic rules from a file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self>;

    /// Evaluate magic rules against a file
    pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult>;

    /// Evaluate magic rules against a buffer
    pub fn evaluate_buffer(&self, buffer: &[u8]) -> Result<EvaluationResult>;
}
```

### EvaluationResult

Contains the results of file type identification.

```rust
pub struct EvaluationResult {
    /// Human-readable file type description
    pub description: String,

    /// Optional MIME type
    pub mime_type: Option<String>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}
```

### EvaluationConfig

Configuration options for rule evaluation.

```rust
pub struct EvaluationConfig {
    /// Maximum recursion depth for nested rules
    pub max_recursion_depth: u32,

    /// Maximum string length to read
    pub max_string_length: usize,

    /// Stop at first match or continue for all matches
    pub stop_at_first_match: bool,
}

impl Default for EvaluationConfig {
    /* ... */
}
```

## AST Types

### MagicRule

Represents a complete magic rule.

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
```

### OffsetSpec

Specifies where to read data in files.

```rust
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
```

### TypeKind

Defines how to interpret bytes.

```rust
pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
}
```

### Operator

Comparison and bitwise operators.

```rust
pub enum Operator {
    Equal,
    NotEqual,
    BitwiseAnd,
}
```

### Value

Expected values for matching.

```rust
pub enum Value {
    Uint(u64),
    Int(i64),
    Bytes(Vec<u8>),
    String(String),
}
```

### Endianness

Byte order specifications.

```rust
pub enum Endianness {
    Little,
    Big,
    Native,
}
```

## Error Types

### LibmagicError

Main error type for the library.

```rust
pub enum LibmagicError {
    ParseError { line: usize, message: String },
    EvaluationError(String),
    IoError(std::io::Error),
    InvalidFormat(String),
}
```

### Result Type

Convenience type alias.

```rust
pub type Result<T> = std::result::Result<T, LibmagicError>;
```

## Parser Module (Planned)

### Functions

```rust
/// Parse magic file into AST
pub fn parse_magic_file<P: AsRef<Path>>(path: P) -> Result<Vec<MagicRule>>;

/// Parse magic rules from string
pub fn parse_magic_string(input: &str) -> Result<Vec<MagicRule>>;
```

## Evaluator Module (Planned)

### Functions

```rust
/// Evaluate rules against buffer
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
    config: &EvaluationConfig,
) -> Result<Vec<Match>>;

/// Evaluate rules against file
pub fn evaluate_file<P: AsRef<Path>>(
    rules: &[MagicRule],
    path: P,
    config: &EvaluationConfig,
) -> Result<Vec<Match>>;
```

## Output Module (Planned)

### Functions

```rust
/// Format results as text
pub fn format_text(results: &[Match]) -> String;

/// Format results as JSON
pub fn format_json(results: &[Match]) -> Result<String>;
```

## I/O Module (Planned)

### FileBuffer

Memory-mapped file buffer.

```rust
pub struct FileBuffer {/* ... */}

impl FileBuffer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self>;
    pub fn as_slice(&self) -> &[u8];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

For complete API documentation with examples, run:

```bash
cargo doc --open
```
