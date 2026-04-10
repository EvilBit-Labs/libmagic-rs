# Architecture Overview

The libmagic-rs library is designed around a clean separation of concerns, following a parser-evaluator architecture that promotes maintainability, testability, and performance.

## High-Level Architecture

```mermaid
flowchart LR
    subgraph Input
        MF[Magic File]
        TF[Target File]
    end

    subgraph Processing
        P[Parser]
        AST[AST]
        FB[File Buffer]
        E[Evaluator]
    end

    subgraph Output
        R[Results]
        F[Formatter]
        O[Output]
    end

    MF --> P --> AST --> E
    TF --> FB --> E
    E --> R --> F --> O

    style MF fill:#1a3a5c,stroke:#4a9eff,color:#e0e0e0
    style TF fill:#1a3a5c,stroke:#4a9eff,color:#e0e0e0
    style P fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style AST fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style FB fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style E fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style R fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
    style F fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
    style O fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
```

## Core Components

### 1. Parser Module (`src/parser/`)

The parser is responsible for converting magic files (text-based DSL) into an Abstract Syntax Tree (AST).

**Key Files:**

- `ast.rs`: Core data structures representing magic rules (✅ Complete)
- `grammar/`: nom-based parsing components for magic file syntax (✅ Complete)
  - `mod.rs`: Main grammar dispatcher (796 lines)
  - `numbers.rs`: Numeric type parsing (decimal/hex, signed/unsigned)
  - `value.rs`: Value literal parsing (strings, floats, hex bytes)
- `mod.rs`: Parser interface, format detection, and hierarchical rule building (✅ Complete)

**Responsibilities:**

- Parse magic file syntax into structured data (✅ Complete)
- Handle hierarchical rule relationships (✅ Complete)
- Validate syntax and report meaningful errors (✅ Complete)
- Detect file format (text, directory, binary) (✅ Complete)
- Support incremental parsing for large magic databases (📋 Planned)

**Current Implementation Status:**

- ✅ **Number parsing**: Decimal and hexadecimal with overflow protection
- ✅ **Offset parsing**: Absolute offsets with comprehensive validation
- ✅ **Operator parsing**: Equality (`=`, `==`), inequality (`!=`, `<>`), comparison (`<`, `>`, `<=`, `>=`), bitwise (`&`, `^`, `~`), and any-value (`x`) operators
- ✅ **Value parsing**: Strings, numbers, and hex byte sequences with escape sequences
- ✅ **PString suffixes**: `/B`, `/H`, `/h`, `/L`, `/l` (length prefix width), `/J` (self-inclusive length)
- ✅ **Error handling**: Comprehensive nom error handling with meaningful messages
- ✅ **Rule parsing**: Complete rule parsing via `parse_magic_rule()`
- ✅ **File parsing**: Complete magic file parsing with `parse_text_magic_file()`
- ✅ **Hierarchy building**: Parent-child relationships via `build_rule_hierarchy()`
- ✅ **Format detection**: Text, directory, and binary format detection
- ✅ **Indirect offsets**: Pointer dereferencing patterns

### 2. AST Data Structures (`src/parser/ast.rs`)

The AST provides a complete representation of magic rules in memory.

**Core Types:**

```rust
pub struct MagicRule {
    pub offset: OffsetSpec,       // Where to read data
    pub typ: TypeKind,            // How to interpret bytes
    pub op: Operator,             // Comparison operation
    pub value: Value,             // Expected value
    pub message: String,          // Human-readable description
    pub children: Vec<MagicRule>, // Nested rules
    pub level: u32,               // Indentation level
}

pub enum TypeKind {
    Byte { signed: bool },        // Single byte with explicit signedness
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    Quad { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
    PString {
        max_length: Option<usize>,
        length_width: PStringLengthWidth,
        length_includes_itself: bool,
    }, // Pascal string (length-prefixed)
}

pub enum Operator {
    Equal,                        // = or ==
    NotEqual,                     // != or <>
    LessThan,                     // <
    GreaterThan,                  // >
    LessEqual,                    // <=
    GreaterEqual,                 // >=
    BitwiseAnd,                   // &
    BitwiseAndMask(u64),          // & with mask
    BitwiseXor,                   // ^
    BitwiseNot,                   // ~
    AnyValue,                     // x (always matches)
}

pub enum PStringLengthWidth {
    OneByte,                      // 1-byte prefix (default, /B)
    TwoByteBE,                    // 2-byte big-endian prefix (/H)
    TwoByteLE,                    // 2-byte little-endian prefix (/h)
    FourByteBE,                   // 4-byte big-endian prefix (/L)
    FourByteLE,                   // 4-byte little-endian prefix (/l)
}
```

**Design Principles:**

- **Immutable by default**: Rules don't change after parsing
- **Serializable**: Full serde support for caching
- **Self-contained**: No external dependencies in AST nodes
- **Type-safe**: Rust's type system prevents invalid rule combinations
- **Explicit signedness**: `TypeKind::Byte` and integer types (Short, Long, Quad) distinguish signed from unsigned interpretations

**PString Length Prefix Support:**

The `PString` type supports multiple length prefix formats through the `length_width` field:

- **OneByte** (`/B`): Default 1-byte length prefix (0-255 range)
- **TwoByteBE** (`/H`): 2-byte big-endian prefix
- **TwoByteLE** (`/h`): 2-byte little-endian prefix
- **FourByteBE** (`/L`): 4-byte big-endian prefix
- **FourByteLE** (`/l`): 4-byte little-endian prefix

The `length_includes_itself` field (controlled by the `/J` suffix) indicates JPEG-style self-inclusive length, where the stored length value includes the length field itself. This can be combined with any width variant (e.g., `/HJ` for 2-byte big-endian with self-inclusive length).

### 3. Evaluator Module (`src/evaluator/`)

The evaluator executes magic rules against file buffers to identify file types. (✅ Complete)

**Structure:**

- `mod.rs`: Public API surface (~720 lines) with `EvaluationContext`, `RuleMatch` types, and re-exports
- `engine/`: Core evaluation engine submodule
  - `mod.rs`: `evaluate_single_rule`, `evaluate_rules`, and `evaluate_rules_with_config` functions
  - `tests.rs`: Engine unit tests
- `types/`: Type interpretation submodule
  - `mod.rs`: Public API surface with `read_typed_value`, `coerce_value_to_type`, and type re-exports
  - `numeric.rs`: Numeric type handling (`read_byte`, `read_short`, `read_long`, `read_quad`) with endianness and signedness support
  - `string.rs`: String type handling (`read_string`, `read_pstring`) with null-termination, UTF-8 conversion, and multi-byte length prefix support
  - `tests.rs`: Module tests
- `offset/`: Offset resolution submodule
  - `mod.rs`: Dispatcher (`resolve_offset`) and re-exports
  - `absolute.rs`: `OffsetError`, `resolve_absolute_offset`
  - `indirect.rs`: `resolve_indirect_offset` for indirect pointer-based offset resolution (issue #37, shipped)
  - `relative.rs`: `resolve_relative_offset` with GNU `file` semantics (issue #38, PR #211)
- `operators/`: Operator application submodule
  - `mod.rs`: Dispatcher (`apply_operator`) and re-exports
  - `equality.rs`: `apply_equal`, `apply_not_equal`
  - `comparison.rs`: `compare_values`, `apply_less_than`/`greater_than`/`less_equal`/`greater_equal`
  - `bitwise.rs`: `apply_bitwise_and`, `apply_bitwise_and_mask`, `apply_bitwise_xor`, `apply_bitwise_not`

**Organization Note:** The evaluator module has been refactored to split monolithic files into focused submodules. The initial refactoring split a 2,638-line `mod.rs` into `engine/` submodules, and a subsequent refactoring reorganized the 1,836-line `types.rs` into `types/` submodules for numeric and string handling. The public API surface remains in `mod.rs` with core logic distributed across focused submodules. This maintains the same public API through re-exports (no breaking changes) while improving code organization and staying within the 500-600 line module guideline.

**Implemented Features:**

- ✅ **Hierarchical Evaluation**: Parent rules must match before children
- ✅ **Lazy Evaluation**: Only process rules when necessary
- ✅ **Bounds Checking**: Safe buffer access with overflow protection
- ✅ **Context Preservation**: Maintain state across rule evaluations
- ✅ **Graceful Degradation**: Skip problematic rules, continue evaluation
- ✅ **Timeout Protection**: Configurable time limits
- ✅ **Recursion Limiting**: Prevent stack overflow from deep nesting
- ✅ **Signedness Coercion**: Automatic value coercion for signed type comparisons (e.g., `0xff` → `-1` for signed byte)
- ✅ **Comparison Operators**: Full support for `<`, `>`, `<=`, `>=` with numeric and lexicographic ordering
- ✅ **Relative Offsets**: Resolution against previous-match anchor using GNU `file` semantics (issue #38, PR #211)
- ✅ **Indirect Offsets**: Pointer dereferencing (implemented)

### 4. Configuration Module (`src/config.rs`)

Extracted from `lib.rs`, this module defines `EvaluationConfig` (307 lines) for controlling rule evaluation behavior.

**Responsibilities:**

- **Security limits**: Maximum recursion depth, string length, and evaluation timeout
- **Matching strategy**: Stop-at-first-match or collect-all-matches mode
- **MIME type mapping**: Enable/disable MIME type lookup for results
- **Configuration presets**: `default()`, `performance()`, and `comprehensive()` constructors
- **Validation**: Comprehensive security checks for configuration values

**Key Types:**

```rust
pub struct EvaluationConfig {
    pub max_recursion_depth: u32,
    pub max_string_length: usize,
    pub stop_at_first_match: bool,
    pub enable_mime_types: bool,
    pub timeout_ms: Option<u64>,
}
```

The `validate()` method enforces safe limits on all fields (recursion depth ≤ 1000, string length ≤ 1MB, timeout ≤ 5 minutes, and resource-combination checks) to prevent stack overflow, memory exhaustion, and denial-of-service attacks.

### 5. I/O Module (`src/io/`)

Provides efficient file access through memory-mapped I/O. (✅ Complete)

**Implemented Features:**

- **FileBuffer**: Memory-mapped file buffers using `memmap2`
- **Safe buffer access**: Comprehensive bounds checking with `safe_read_bytes` and `safe_read_byte`
- **Error handling**: Structured IoError types for all failure scenarios
- **Resource management**: RAII patterns with automatic cleanup
- **File validation**: Size limits, empty file detection, and metadata validation
- **Overflow protection**: Safe arithmetic in all buffer operations

**Key Components:**

```rust
pub struct FileBuffer {
    mmap: Mmap,
    path: PathBuf,
}

pub fn safe_read_bytes(buffer: &[u8], offset: usize, length: usize) -> Result<&[u8], IoError>
pub fn safe_read_byte(buffer: &[u8], offset: usize) -> Result<u8, IoError>
pub fn validate_buffer_access(buffer_size: usize, offset: usize, length: usize) -> Result<(), IoError>
```

### 6. Output Module (`src/output/`)

Formats evaluation results into different output formats.

**Planned Formatters:**

- `text.rs`: Human-readable output (GNU `file` compatible)
- `json.rs`: Structured JSON output with metadata
- `mod.rs`: Format selection and coordination

## Data Flow

### 1. Magic File Loading

```mermaid
flowchart LR
    A[Magic File\ntext] --> B[Parser]
    B --> C[AST]
    C --> D[Validation]
    D --> E[Cached Rules]

    style A fill:#1a3a5c,stroke:#4a9eff,color:#e0e0e0
    style E fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
```

1. **Parsing**: Convert text DSL to structured AST
2. **Validation**: Check rule consistency and dependencies
3. **Optimization**: Reorder rules for evaluation efficiency
4. **Caching**: Serialize compiled rules for reuse

### 2. File Evaluation

```mermaid
flowchart LR
    A[Target File] --> B[Memory Map]
    B --> C[Buffer]
    C --> D[Rule Evaluation]
    D --> E[Results]
    E --> F[Formatting]

    style A fill:#1a3a5c,stroke:#4a9eff,color:#e0e0e0
    style F fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
```

1. **File Access**: Create memory-mapped buffer
2. **Rule Matching**: Execute rules hierarchically
3. **Result Collection**: Gather matches and metadata
4. **Output Generation**: Format results as text or JSON

## Design Patterns

### Parser-Evaluator Separation

The clear separation between parsing and evaluation provides:

- **Independent Testing**: Each component can be tested in isolation
- **Performance Optimization**: Rules can be pre-compiled and cached
- **Flexible Input**: Support for different magic file formats
- **Error Isolation**: Parse errors vs. evaluation errors are distinct

### Hierarchical Rule Processing

Magic rules form a tree structure where:

- **Parent rules** define broad file type categories
- **Child rules** provide specific details and variants
- **Evaluation stops** when a definitive match is found
- **Context flows** from parent to child evaluations

```mermaid
flowchart TD
    R["Root Rule<br/>e.g., 0 string PK"]
    R -->|match| C1["Child Rule 1<br/>e.g., #gt;4 ubyte 0x14"]
    R -->|match| C2["Child Rule 2<br/>e.g., #gt;4 ubyte 0x06"]
    C1 -->|match| G1["Grandchild<br/>ZIP archive v2.0"]
    C2 -->|match| G2["Grandchild<br/>ZIP archive v1.0"]

    style R fill:#1a3a5c,stroke:#4a9eff,color:#e0e0e0
    style C1 fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style C2 fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style G1 fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
    style G2 fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
```

**Operator Support:**

The evaluator supports all comparison, bitwise, and special matching operators:

- **Equality**: `=` or `==` (exact match)
- **Inequality**: `!=` or `<>` (not equal)
- **Less-than**: `<` (numeric or lexicographic)
- **Greater-than**: `>` (numeric or lexicographic)
- **Less-equal**: `<=` (numeric or lexicographic)
- **Greater-equal**: `>=` (numeric or lexicographic)
- **Bitwise AND**: `&` (bit pattern matching)
- **Bitwise XOR**: `^` (exclusive OR pattern matching)
- **Bitwise NOT**: `~` (bitwise complement comparison)
- **Any-value**: `x` (unconditional match, always succeeds)

Comparison operators support both numeric comparisons (with automatic type coercion between signed and unsigned integers via `i128`) and lexicographic comparisons for strings and byte sequences.

### Memory-Safe Buffer Access

All buffer operations use safe Rust patterns:

```rust
// Safe buffer access with bounds checking
fn read_bytes(buffer: &[u8], offset: usize, length: usize) -> Option<&[u8]> {
    buffer.get(offset..offset.saturating_add(length))
}
```

### Error Handling Strategy

The library uses Result types with nested error enums throughout:

```rust
pub type Result<T> = std::result::Result<T, LibmagicError>;

#[derive(Debug, thiserror::Error)]
pub enum LibmagicError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),

    #[error("Evaluation error: {0}")]
    EvaluationError(#[from] EvaluationError),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Evaluation timeout exceeded after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid syntax at line {line}: {message}")]
    InvalidSyntax { line: usize, message: String },

    #[error("Unsupported format at line {line}: {format_type}")]
    UnsupportedFormat { line: usize, format_type: String, message: String },
    // ... additional variants
}

#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    #[error("Buffer overrun at offset {offset}")]
    BufferOverrun { offset: usize },

    #[error("Recursion limit exceeded (depth: {depth})")]
    RecursionLimitExceeded { depth: u32 },
    // ... additional variants
}
```

## Performance Considerations

### Memory Efficiency

- **Zero-copy operations** where possible
- **Memory-mapped I/O** to avoid loading entire files
- **Lazy evaluation** to skip unnecessary work
- **Rule caching** to avoid re-parsing magic files

### Computational Efficiency

- **Early termination** when definitive matches are found
- **Optimized rule ordering** based on match probability
- **Efficient string matching** using algorithms like Aho-Corasick
- **Minimal allocations** in hot paths

### Scalability

- **Parallel evaluation** for multiple files (future)
- **Streaming support** for large files (future)
- **Incremental parsing** for large magic databases
- **Resource limits** to prevent runaway evaluations

## Module Dependencies

```mermaid
flowchart TD
    L[lib.rs<br/>Public API and coordination<br/>624 lines]
    C[config.rs<br/>EvaluationConfig<br/>307 lines]
    L --> C
    L --> P[parser/<br/>Magic file parsing]
    L --> E[evaluator/<br/>Rule evaluation engine]
    L --> O[output/<br/>Result formatting]
    L --> I[io/<br/>File I/O utilities]
    L --> ER[error.rs<br/>Error types]

    P --> ER
    E --> P
    E --> C
    E --> I
    E --> ER
    O --> ER

    style L fill:#2a1a4a,stroke:#b39ddb,color:#e0e0e0
    style C fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
    style P fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style E fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style O fill:#4a3000,stroke:#ffb74d,color:#e0e0e0
    style I fill:#1b3d1b,stroke:#66bb6a,color:#e0e0e0
    style ER fill:#4a1a1a,stroke:#ef5350,color:#e0e0e0
```

**Dependency Rules:**

- **No circular dependencies** between modules
- **Clear interfaces** with well-defined responsibilities
- **Minimal coupling** between components
- **Testable boundaries** for each module

This architecture ensures the library is maintainable, performant, and extensible while providing a clean API for both CLI and library usage.
