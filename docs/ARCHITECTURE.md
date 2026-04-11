# Architecture Guide - libmagic-rs

A comprehensive guide to the architecture and design of libmagic-rs.

## Table of Contents

- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Module Organization](#module-organization)
- [Data Flow](#data-flow)
- [Key Components](#key-components)
- [Design Decisions](#design-decisions)
- [Security Architecture](#security-architecture)

---

## Overview

libmagic-rs is a pure-Rust implementation of the libmagic library for file type identification. It follows a parser-evaluator architecture that separates concerns between magic file parsing and rule evaluation.

### Core Principles

1. **Memory Safety**: Pure Rust with no unsafe code (except vetted dependencies)
2. **Performance**: Memory-mapped I/O with zero-copy operations
3. **Compatibility**: Support for common libmagic syntax patterns
4. **Extensibility**: AST-based design for easy feature additions

---

## System Architecture

```text
+-------------------+     +-------------------+     +-------------------+
|   Magic File(s)   |     |   Target File     |     |   Configuration   |
+-------------------+     +-------------------+     +-------------------+
         |                         |                         |
         v                         v                         v
+-------------------+     +-------------------+     +-------------------+
|      Parser       |     |   Memory Mapper   |     | EvaluationConfig  |
|   (nom-based)     |     |    (memmap2)      |     |                   |
+-------------------+     +-------------------+     +-------------------+
         |                         |                         |
         v                         |                         |
+-------------------+              |                         |
|       AST         |              |                         |
|   (MagicRule)     |              |                         |
+-------------------+              |                         |
         |                         |                         |
         +-------------------------+-------------------------+
                                   |
                                   v
                    +----------------------------+
                    |        Evaluator           |
                    |  (offset, types, operators)|
                    +----------------------------+
                                   |
                                   v
                    +----------------------------+
                    |      Match Results         |
                    +----------------------------+
                                   |
                                   v
                    +----------------------------+
                    |    Output Formatter        |
                    |    (text, JSON)            |
                    +----------------------------+
```

---

## Module Organization

```text
libmagic-rs/
├── src/
│   ├── lib.rs              # Public API, MagicDatabase (624 lines)
│   ├── main.rs             # CLI binary (rmagic)
│   ├── config.rs           # EvaluationConfig with security limits (307 lines)
│   ├── error.rs            # Error types (LibmagicError, ParseError, EvaluationError)
│   ├── builtin_rules.rs    # Pre-compiled magic rules
│   ├── builtin_rules.magic # Built-in rule definitions
│   ├── build_helpers.rs    # Build script utilities
│   │
│   ├── parser/             # Magic file parsing
│   │   ├── mod.rs          # Parser interface
│   │   ├── ast.rs          # AST definitions (MagicRule, TypeKind::Byte { signed: bool }, etc.)
│   │   ├── grammar/        # nom-based parsing combinators
│   │   │   ├── mod.rs      # Rule and type parsing (796 lines)
│   │   │   ├── numbers.rs  # Decimal/hex number parsing
│   │   │   └── value.rs    # Value-literal parsing
│   │   └── loader.rs       # Magic file loading and format detection
│   │
│   ├── evaluator/          # Rule evaluation engine
│   │   ├── mod.rs          # Public API surface with re-exports, EvaluationContext, RuleMatch
│   │   ├── engine.rs       # Core evaluation logic (evaluate_single_rule, evaluate_rules, evaluate_rules_with_config)
│   │   ├── offset.rs       # Offset resolution
│   │   ├── types/          # Type reading subsystem
│   │   │   ├── mod.rs      # Type dispatch and pattern matching
│   │   │   ├── numeric.rs  # Byte, Short, Long, Quad
│   │   │   ├── float.rs    # Float, Double
│   │   │   ├── date.rs     # Date, QDate
│   │   │   ├── string.rs   # String, PString
│   │   │   ├── regex.rs    # Regex pattern matching
│   │   │   └── search.rs   # Search literal scanning
│   │   ├── operators.rs    # Comparison operations
│   │   └── strength.rs     # Strength calculation and sorting
│   │
│   ├── io/                 # I/O utilities
│   │   └── mod.rs          # FileBuffer, SafeBufferAccess
│   │
│   ├── output/             # Output formatting
│   │   ├── mod.rs          # MatchResult, EvaluationResult
│   │   ├── text.rs         # Text output formatter
│   │   └── json.rs         # JSON output formatter
│   │
│   ├── mime.rs             # MIME type mapping
│   └── tags.rs             # Tag extraction
│
├── tests/                  # Integration tests
│   ├── compatibility/      # libmagic compatibility tests
│   └── ...
│
└── benches/                # Performance benchmarks
```

---

## Data Flow

### 1. Magic File Loading

```text
Magic File Path
       |
       v
+------------------+
| detect_format()  |  Determine: Text, Directory, or Binary
+------------------+
       |
       v
+------------------+
| load_magic_file()|  Unified loading interface
+------------------+
       |
       +--------+---------+
       |        |         |
       v        v         v
    Text    Directory  Binary
    File      (Magdir)   (.mgc)
       |        |         |
       v        v         v
   parse     merge     (error:
   rules     files     unsupported)
       |        |
       +--------+
       |
       v
+------------------+
|   Vec<MagicRule> |  Parsed AST
+------------------+
```

### 2. Rule Evaluation

```text
+------------------+     +------------------+
|   Vec<MagicRule> |     |   File Buffer    |
+------------------+     +------------------+
         |                        |
         +------------------------+
                    |
                    v
        +------------------------+
        |  evaluate_rules_with_  |
        |  config()              |
        +------------------------+
                    |
    +---------------+---------------+
    |               |               |
    v               v               v
+--------+    +----------+    +----------+
| Offset |    |  Type    |    | Operator |
| Resolve|    |  Read    |    | Compare  |
+--------+    +----------+    +----------+
    |               |               |
    +---------------+---------------+
                    |
                    v
        +------------------------+
        |  Child Rule Evaluation |
        |  (if parent matched)   |
        +------------------------+
                    |
                    v
        +------------------------+
        |   Vec<MatchResult>     |
        +------------------------+
```

### 3. Output Generation

```text
+------------------+     +------------------+
|  EvaluationResult|     |   OutputFormat   |
+------------------+     +------------------+
         |                        |
         +------------------------+
                    |
                    v
        +------------------------+
        |   Format Selection     |
        +------------------------+
                    |
         +----------+----------+
         |                     |
         v                     v
    +----------+         +----------+
    |   Text   |         |   JSON   |
    | Formatter|         | Formatter|
    +----------+         +----------+
         |                     |
         v                     v
    "file: type"         { "matches": [...] }
```

---

## Key Components

### MagicDatabase

The main entry point for users. Manages rule loading and evaluation.

```rust
pub struct MagicDatabase {
    rules: Vec<MagicRule>,      // Parsed magic rules
    config: EvaluationConfig,   // Evaluation settings
    source_path: Option<PathBuf>, // Where rules came from
}
```

**Responsibilities:**

- Load rules from files, directories, or built-in
- Coordinate evaluation with configuration
- Present results in a user-friendly format

### EvaluationConfig

Controls evaluation behavior with security-focused defaults. Extracted from `lib.rs` to `config.rs` (307 lines) in PR #212.

```rust
pub struct EvaluationConfig {
    max_recursion_depth: u32,   // Prevent stack overflow
    max_string_length: usize,   // Prevent memory exhaustion
    stop_at_first_match: bool,  // Performance optimization
    enable_mime_types: bool,    // MIME type mapping
    timeout_ms: Option<u64>,    // DoS protection
}
```

**Security Limits:**

- Recursion depth: 1-1000 (default: 20)
- String length: 1-1MB (default: 8192)
- Timeout: 1-300000ms (5 minutes max)

**Configuration Presets:**

- `EvaluationConfig::new()` - Default balanced configuration
- `EvaluationConfig::performance()` - Fast evaluation (depth 10, string 1024, 1s timeout)
- `EvaluationConfig::comprehensive()` - Find all matches (depth 50, string 32768, 30s timeout)

### MagicRule (AST)

Represents a single magic rule in the abstract syntax tree.

```rust
pub struct MagicRule {
    offset: OffsetSpec,     // Where to read
    typ: TypeKind,          // What to read
    op: Operator,           // How to compare
    value: Value,           // Expected value
    message: String,        // Description
    children: Vec<MagicRule>, // Nested rules
    level: u32,             // Indentation level
    strength_modifier: Option<StrengthModifier>, // Strength adjustment
}
```

**TypeKind Variants:**

- `Byte { signed: bool }` - 8-bit integer
- `Short { endian: Endianness, signed: bool }` - 16-bit integer
- `Long { endian: Endianness, signed: bool }` - 32-bit integer
- `Quad { endian: Endianness, signed: bool }` - 64-bit integer
- `String { max_length: Option<usize> }` - Null-terminated string
- `Regex { flags: RegexFlags, count: RegexCount }` - Regular expression matching (see `RegexCount` for the `Default` / `Bytes(n)` / `Lines(Option<n>)` variants)
- `Search { range: NonZeroUsize }` - Bounded literal pattern search

**Hierarchical Structure:**

- Top-level rules (level 0) are entry points
- Child rules are evaluated only if parent matches
- Deeper matches = higher confidence

**Operator Support:**

- Supports comparison operators (`<`, `>`, `<=`, `>=`) in addition to equality (`=`, `!=`) and bitwise operators (`&`)

### EvaluationContext

Tracks state during rule evaluation.

```rust
pub struct EvaluationContext {
    current_offset: usize,  // Current position in buffer
    recursion_depth: u32,   // Nesting level
    config: EvaluationConfig, // Settings
}
```

**State Management:**

- Offset tracking for relative offsets
- Recursion depth monitoring
- Configuration access

### FileBuffer

Memory-mapped file access with safety guarantees.

```rust
pub struct FileBuffer {
    mmap: Mmap,             // Memory-mapped region
    size: usize,            // File size
}

pub trait SafeBufferAccess {
    fn get(&self, offset: usize) -> Option<u8>;
    fn get_range(&self, start: usize, end: usize) -> Option<&[u8]>;
}
```

**Safety Features:**

- Bounds checking on all accesses
- No direct indexing
- Empty file handling

---

## Design Decisions

### 1. Parser-Evaluator Separation

**Decision:** Separate parsing from evaluation with an AST intermediary.

**Rationale:**

- Allows rule caching and reuse
- Enables different evaluation strategies
- Simplifies testing and debugging
- Supports future optimizations (rule compilation)

### 2. nom for Parsing

**Decision:** Use nom parser combinators for magic file parsing.

**Rationale:**

- Zero-copy parsing where possible
- Composable parser fragments
- Strong error handling
- Well-tested in production

### 3. Memory-Mapped I/O

**Decision:** Use memmap2 for file access.

**Rationale:**

- Efficient for large files
- Lazy loading (only read what's needed)
- OS-managed caching
- Zero-copy buffer access

### 4. Bounds-Checked Access

**Decision:** All buffer access through `.get()` methods.

**Rationale:**

- Prevents buffer overruns
- No panic on invalid offsets
- Safe handling of truncated files
- Required for fuzzing compatibility

### 5. Configuration Validation

**Decision:** Validate configuration at creation time.

**Rationale:**

- Fail fast on invalid settings
- Prevent security issues
- Clear error messages
- Resource limit enforcement

### 6. Text-First Magic File Discovery

**Decision:** Prefer text magic files over binary .mgc files.

**Rationale:**

- Text files are debuggable
- Better for version control
- Easier development workflow
- Binary .mgc parsing is complex

---

## Security Architecture

### Threat Model

| Threat                              | Mitigation                  |
| ----------------------------------- | --------------------------- |
| Stack overflow via deep nesting     | `max_recursion_depth` limit |
| Memory exhaustion via large strings | `max_string_length` limit   |
| DoS via infinite evaluation         | `timeout_ms` limit          |
| Buffer overrun                      | Bounds checking everywhere  |
| Malformed input                     | Graceful error handling     |
| Integer overflow                    | Checked arithmetic          |

### Security Layers

```text
+----------------------------------+
|     Configuration Validation     |  Layer 1: Prevent bad configs
+----------------------------------+
              |
              v
+----------------------------------+
|     Input Validation             |  Layer 2: Validate magic files
+----------------------------------+
              |
              v
+----------------------------------+
|     Bounds Checking              |  Layer 3: Safe buffer access
+----------------------------------+
              |
              v
+----------------------------------+
|     Resource Limits              |  Layer 4: Runtime protection
+----------------------------------+
              |
              v
+----------------------------------+
|     Error Handling               |  Layer 5: Graceful degradation
+----------------------------------+
```

### Code Safety

- `#![deny(unsafe_code)]` - No unsafe code in library
- `#![deny(clippy::all)]` - Comprehensive linting
- `#[forbid(unsafe_code)]` in workspace - Project-wide safety

### Dependency Safety

Vetted dependencies with minimal unsafe:

- `memmap2` - Memory mapping (audited)
- `nom` - Parsing (no unsafe)
- `thiserror` - Error handling (no unsafe)
- `regex` - Pattern matching (production dependency)
- `memchr` - Fast byte searching (production dependency)

---

## Performance Considerations

### Hot Path Optimization

The evaluation hot path is optimized for:

1. Minimal allocations
2. Zero-copy buffer access
3. Early exit on mismatch
4. Efficient type reading

### Caching Strategy

- Parsed rules cached in `MagicDatabase`
- Reuse database for multiple files
- One parse, many evaluations

### Memory Efficiency

- Memory-mapped files avoid full loading
- Streaming evaluation possible
- Bounded string reading

---

## Extension Points

### Adding New Types

1. Add variant to `TypeKind` enum (`ast.rs`)
2. Add parsing logic (`grammar/mod.rs`)
3. Add reading logic in `evaluator/types/` (as a submodule for complex types or in `types/mod.rs` for simple ones)
4. Add serialization support (`build_helpers.rs`)
5. Add tests
6. Update documentation

#### Example: Quad Type Implementation

The `Quad` type (64-bit integer) demonstrates the type system extension pattern. The implementation includes:

- `TypeKind::Quad { endian: Endianness, signed: bool }` variant in the AST
- `read_quad()` function for safe buffer access with bounds checking
- Parsing support for `quad`, `uquad`, `lequad`, `ulequad`, `bequad`, `ubequad` type names
- Strength calculation (specificity score of 16, highest among numeric types)
- Serialization for build-time rule compilation

### Adding New Operators

1. Add variant to `Operator` enum (`ast.rs`)
2. Add parsing logic (`grammar/mod.rs`)
3. Add comparison logic (`operators.rs`)
4. Add serialization for build-time (`build.rs` and `build_helpers.rs`)
5. Add tests
6. Update documentation

**Implemented Operators:**

- `Equal` (`=`, `==`)
- `NotEqual` (`!=`, `<>`)
- `LessThan` (`<`)
- `GreaterThan` (`>`)
- `LessEqual` (`<=`)
- `GreaterEqual` (`>=`)
- `BitwiseAnd` (`&`)
- `BitwiseAndMask` (`&` with mask)

### Adding Output Formats

1. Create new module in `output/`
2. Implement formatting functions
3. Add to CLI options
4. Add tests
5. Update documentation

---

## Diagram: Component Interaction

```text
                    User Application
                           |
                           v
    +--------------------------------------------------+
    |                  MagicDatabase                    |
    |  +--------------------------------------------+  |
    |  |              Public API                     |  |
    |  |  - with_builtin_rules()                    |  |
    |  |  - load_from_file()                        |  |
    |  |  - evaluate_file()                         |  |
    |  |  - evaluate_buffer()                       |  |
    |  +--------------------------------------------+  |
    |                      |                           |
    |      +---------------+---------------+           |
    |      |                               |           |
    |      v                               v           |
    |  +--------+                   +------------+     |
    |  | Parser |                   | Evaluator  |     |
    |  +--------+                   +------------+     |
    |      |                               |           |
    |      v                               v           |
    |  +--------+                   +------------+     |
    |  |  AST   |------------------>| Type Reader|     |
    |  +--------+                   +------------+     |
    |                                      |           |
    |                                      v           |
    |                               +------------+     |
    |                               |  Operators |     |
    |                               +------------+     |
    |                                      |           |
    |                                      v           |
    |                               +------------+     |
    |                               |  Results   |     |
    |                               +------------+     |
    +--------------------------------------------------+
                           |
                           v
                    Output Formatter
                           |
                           v
                    Text / JSON
```

---

## Future Architecture Considerations

1. **Rule Compilation**: Compile rules to optimized bytecode
2. **Parallel Evaluation**: Evaluate independent rules concurrently
3. **Rule Indexing**: Aho-Corasick for multi-pattern matching
4. **Streaming API**: Process files without full loading
5. **WebAssembly Support**: Browser-based file identification
