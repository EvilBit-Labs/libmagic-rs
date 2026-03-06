# Evaluator Engine

The evaluator engine executes magic rules against file buffers to identify file types. It provides safe, efficient rule evaluation with hierarchical processing, graceful error recovery, and configurable resource limits.

## Overview

The evaluator processes magic rules hierarchically:

1. **Load file** into memory-mapped buffer
2. **Resolve offsets** (absolute, relative, from-end)
3. **Read typed values** from buffer with bounds checking
4. **Apply operators** for comparison
5. **Process children** if parent rule matches
6. **Collect results** with match metadata

## Architecture

```text
File Buffer → Offset Resolution → Type Reading → Operator Application → Results
     ↑              ↑                  ↑              ↑                    ↑
Memory Map    Context State      Endian Handling   Match Logic      Hierarchical
```

## Module Organization

The evaluator module separates public interface from implementation:

- **`evaluator/mod.rs`** - Public API surface: defines `EvaluationContext` and `RuleMatch` types, re-exports core evaluation functions from the engine submodule
- **`evaluator/engine/mod.rs`** - Core evaluation implementation: `evaluate_single_rule`, `evaluate_rules`, `evaluate_rules_with_config`
- **`evaluator/offset/mod.rs`** - Offset resolution
- **`evaluator/operators/mod.rs`** - Operator application
- **`evaluator/types/`** - Type reading and coercion (organized as submodules as of v0.4.2)
  - **`types/mod.rs`** - Public API surface: `read_typed_value`, `coerce_value_to_type`, re-exports type functions
  - **`types/numeric.rs`** - Numeric type handling: `read_byte`, `read_short`, `read_long`, `read_quad` with endianness and signedness support
  - **`types/string.rs`** - String type handling: `read_string` with null-termination and UTF-8 conversion
  - **`types/tests.rs`** - Module tests
- **`evaluator/strength.rs`** - Rule strength calculation

The refactoring improves organization by separating concerns: `mod.rs` handles the public API surface and data types, while `engine/` contains the core evaluation logic. The types module was refactored in v0.4.2 from a single 1,836-line file into focused submodules for numeric and string handling, improving maintainability without changing the public API. From a public API perspective, all types and functions are imported from the `evaluator` module as before -- the internal organization is transparent to library users.

## Core Components

### EvaluationContext

Maintains state during rule processing:

```rust
pub struct EvaluationContext {
    /// Current offset position for relative calculations
    current_offset: usize,
    /// Current recursion depth for safety limits
    recursion_depth: u32,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}
```

Note: Fields are private; use accessor methods like `current_offset()`, `recursion_depth()`, and `config()`.

**Key Methods:**

- `new()` - Create context with default configuration
- `current_offset()` / `set_current_offset()` - Track current buffer position
- `recursion_depth()` - Query current recursion depth
- `increment_recursion_depth()` / `decrement_recursion_depth()` - Track recursion safely
- `timeout_ms()` - Query configured timeout
- `reset()` - Reset context state for reuse

### RuleMatch

Represents a successful rule match:

```rust
pub struct RuleMatch {
    /// Human-readable description from the matched rule
    pub message: String,
    /// Offset where the match occurred
    pub offset: usize,
    /// Depth in the rule hierarchy (0 = root rule)
    pub level: u32,
    /// The matched value (parsed according to rule type)
    pub value: Value,
    /// Confidence score (0.0 to 1.0) based on rule hierarchy depth
    pub confidence: f64,
}
```

The `Value` type is from `parser::ast::Value` and represents the actual matched content according to the rule's type specification.

### Offset Resolution (`evaluator/offset.rs`)

Handles all offset types safely:

- **Absolute offsets**: Direct file positions (`0`, `0x100`)
- **Relative offsets**: Based on previous match positions (`&+4`)
- **From-end offsets**: Calculated from file size (`-4` from end)
- **Bounds checking**: All offset calculations are validated

```rust
pub fn resolve_offset(
    spec: &OffsetSpec,
    buffer: &[u8],
) -> Result<usize, LibmagicError>
```

### Type Reading (`evaluator/types/`)

Interprets bytes according to type specifications. The types module is organized into submodules for numeric and string type handling (refactored from a single file in v0.4.2):

- **Byte**: Single byte values (signed or unsigned)
- **Short**: 16-bit integers with endianness
- **Long**: 32-bit integers with endianness
- **Quad**: 64-bit integers with endianness
- **String**: Byte sequences with length limits
- **Bounds checking**: Prevents buffer overruns

```rust
pub fn read_typed_value(
    buffer: &[u8],
    offset: usize,
    type_kind: &TypeKind,
) -> Result<Value, TypeReadError>
```

The `read_byte` function signature changed in v0.2.0 to accept three parameters (`buffer`, `offset`, and `signed`) instead of two, allowing explicit control over signed vs unsigned byte interpretation.

### Operator Application (`evaluator/operators.rs`)

Applies comparison operations:

- **Equal** (`=`, `==`): Exact value matching
- **NotEqual** (`!=`, `<>`): Non-matching values
- **LessThan** (`<`): Less-than comparison (numeric or lexicographic) *(added in v0.2.0)*
- **GreaterThan** (`>`): Greater-than comparison (numeric or lexicographic) *(added in v0.2.0)*
- **LessEqual** (`<=`): Less-than-or-equal comparison (numeric or lexicographic) *(added in v0.2.0)*
- **GreaterEqual** (`>=`): Greater-than-or-equal comparison (numeric or lexicographic) *(added in v0.2.0)*
- **BitwiseAnd** (`&`): Pattern matching for flags
- **BitwiseAndMask**: AND with mask then compare

Comparison operators support numeric comparisons across different integer types using `i128` coercion for cross-type compatibility.

```rust
pub fn apply_operator(
    operator: &Operator,
    left: &Value,
    right: &Value,
) -> bool
```

**Example with comparison operators:**

```rust
use libmagic_rs::parser::ast::{Operator, Value};
use libmagic_rs::evaluator::operators::apply_operator;

// Less-than comparison (v0.2.0+)
assert!(apply_operator(
    &Operator::LessThan,
    &Value::Uint(5),
    &Value::Uint(10)
));

// Greater-than-or-equal comparison (v0.2.0+)
assert!(apply_operator(
    &Operator::GreaterEqual,
    &Value::Uint(10),
    &Value::Uint(10)
));

// Cross-type integer comparison (v0.2.0+)
assert!(apply_operator(
    &Operator::LessThan,
    &Value::Int(-1),
    &Value::Uint(0)
));
```

## Evaluation Algorithm

The evaluator uses a depth-first hierarchical algorithm:

```rust
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
) -> Result<Vec<RuleMatch>, EvaluationError>
```

**Algorithm:**

1. For each root rule:

   - Resolve offset from buffer
   - Read value at offset according to type
   - Apply operator to compare actual vs expected
   - If match: add to results, recursively evaluate children
   - If no match: skip children, continue to next rule

2. Child rules inherit context from parent match

3. Results accumulate hierarchically (parent message + child details)

### Hierarchical Processing

```mermaid
flowchart TD
    R[Root Rule<br/>e.g., "0 string \x7fELF"]
    R -->|match| C1[Child Rule 1<br/>e.g., ">4 byte 1"]
    R -->|match| C2[Child Rule 2<br/>e.g., ">4 byte 2"]
    C1 -->|match| G1[Result:<br/>ELF 32-bit]
    C2 -->|match| G2[Result:<br/>ELF 64-bit]

    style R fill:#e3f2fd
    style C1 fill:#fff3e0
    style C2 fill:#fff3e0
    style G1 fill:#c8e6c9
    style G2 fill:#c8e6c9
```

## Configuration

Evaluation behavior is controlled via `EvaluationConfig`:

```rust
pub struct EvaluationConfig {
    /// Maximum recursion depth for nested rules (default: 20)
    pub max_recursion_depth: u32,
    /// Maximum string length to read (default: 8192)
    pub max_string_length: usize,
    /// Stop at first match or continue for all matches (default: true)
    pub stop_at_first_match: bool,
    /// Enable MIME type mapping in results (default: false)
    pub enable_mime_types: bool,
    /// Timeout for evaluation in milliseconds (default: None)
    pub timeout_ms: Option<u64>,
}
```

**Preset Configurations:**

```rust
// Default balanced configuration
let config = EvaluationConfig::default();

// Optimized for speed
let config = EvaluationConfig::performance();

// Find all matches with full details
let config = EvaluationConfig::comprehensive();
```

## Safety Features

### Memory Safety

- **Bounds checking**: All buffer access is validated before reading
- **Integer overflow protection**: Safe arithmetic using `checked_*` and `saturating_*`
- **Resource limits**: Configurable limits prevent resource exhaustion

### Error Handling

The evaluator uses graceful degradation:

- **Invalid offsets**: Skip rule, continue with others
- **Type mismatches**: Skip rule, continue with others
- **Timeout exceeded**: Return error (partial results are not preserved)
- **Recursion limit**: Stop descent, continue siblings

```rust
pub enum EvaluationError {
    BufferOverrun { offset: usize },
    InvalidOffset { offset: i64 },
    UnsupportedType { type_name: String },
    RecursionLimitExceeded { depth: u32 },
    StringLengthExceeded { length: usize, max_length: usize },
    InvalidStringEncoding { offset: usize },
    Timeout { timeout_ms: u64 },
    TypeReadError(TypeReadError),
}
```

### Timeout Protection

```rust
// With 5 second timeout
let config = EvaluationConfig {
    timeout_ms: Some(5000),
    ..Default::default()
};

let result = evaluate_rules_with_config(&rules, buffer, &config)?;
```

## API Reference

### Primary Functions

```rust
/// Evaluate rules with context for recursion tracking
pub fn evaluate_rules(
    rules: &[MagicRule],
    buffer: &[u8],
    context: &mut EvaluationContext,
) -> Result<Vec<RuleMatch>, LibmagicError>;

/// Evaluate rules with custom configuration (creates context internally)
pub fn evaluate_rules_with_config(
    rules: &[MagicRule],
    buffer: &[u8],
    config: &EvaluationConfig,
) -> Result<Vec<RuleMatch>, LibmagicError>;

/// Evaluate a single rule (used internally and for testing)
pub fn evaluate_single_rule(
    rule: &MagicRule,
    buffer: &[u8],
) -> Result<Option<(usize, Value)>, LibmagicError>;
```

### Usage Example

```rust
use libmagic_rs::{evaluate_rules, EvaluationConfig};
use libmagic_rs::parser::parse_text_magic_file;

// Parse magic rules
let magic_content = r#"
0 string \x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
"#;
let rules = parse_text_magic_file(magic_content)?;

// Read target file
let buffer = std::fs::read("sample.bin")?;

// Evaluate with default config
let matches = evaluate_rules(&rules, &buffer)?;

for m in matches {
    println!("Match at offset {}: {}", m.offset, m.message);
}
```

**Example with comparison operators (v0.2.0+):**

```rust
use libmagic_rs::{evaluate_rules, EvaluationConfig};
use libmagic_rs::parser::parse_text_magic_file;

// Parse magic rule with comparison operator
let magic_content = r#"
0 leshort <100 Small value detected
0 leshort >=1000 Large value detected
"#;
let rules = parse_text_magic_file(magic_content)?;

let buffer = vec![0x0A, 0x00]; // Little-endian 10
let matches = evaluate_rules(&rules, &buffer)?;

// Matches first rule (<100)
assert_eq!(matches[0].message, "Small value detected");
```

## Implementation Status

- [x] Basic evaluation engine structure
- [x] Offset resolution (absolute, relative, from-end)
- [x] Type reading with endianness support (Byte, Short, Long, Quad, String)
- [x] Operator application (Equal, NotEqual, LessThan, GreaterThan, LessEqual, GreaterEqual, BitwiseAnd, BitwiseAndMask)
- [x] Hierarchical rule processing with child evaluation
- [x] Error handling with graceful degradation
- [x] Timeout protection
- [x] Recursion depth limiting
- [x] Comprehensive test coverage (100+ tests)
- [ ] Indirect offset support (pointer dereferencing)
- [ ] Regex type support
- [ ] Performance optimizations (rule ordering, caching)

## Performance Considerations

### Lazy Evaluation

- **Parent-first**: Only evaluate children if parent matches
- **Early termination**: Stop on first match when configured
- **Skip on error**: Continue evaluation after non-fatal errors

### Memory Efficiency

- **Memory mapping**: Files accessed via mmap, not loaded entirely
- **Zero-copy reads**: Slice references where possible
- **Bounded strings**: String reads limited to prevent memory exhaustion
