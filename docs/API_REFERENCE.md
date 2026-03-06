# API Reference - libmagic-rs

A comprehensive reference for the libmagic-rs library API.

## Table of Contents

- [Core Types](#core-types)
- [MagicDatabase](#magicdatabase)
- [EvaluationConfig](#evaluationconfig)
- [EvaluationResult](#evaluationresult)
- [Error Handling](#error-handling)
- [Parser Module](#parser-module)
- [Evaluator Module](#evaluator-module)
- [Output Module](#output-module)

---

## Core Types

### MagicDatabase

The main interface for loading magic rules and evaluating files.

```rust
use libmagic_rs::MagicDatabase;
```

#### Constructor Methods

| Method                                     | Description                                  |
| ------------------------------------------ | -------------------------------------------- |
| `with_builtin_rules()`                     | Create database with built-in rules          |
| `with_builtin_rules_and_config(config)`    | Create with built-in rules and custom config |
| `load_from_file(path)`                     | Load rules from a file or directory          |
| `load_from_file_with_config(path, config)` | Load from file with custom config            |

#### Evaluation Methods

| Method                    | Description                        |
| ------------------------- | ---------------------------------- |
| `evaluate_file(path)`     | Evaluate a file and return results |
| `evaluate_buffer(buffer)` | Evaluate an in-memory buffer       |

#### Accessor Methods

| Method          | Return Type         | Description                     |
| --------------- | ------------------- | ------------------------------- |
| `config()`      | `&EvaluationConfig` | Get evaluation configuration    |
| `source_path()` | `Option<&Path>`     | Get path rules were loaded from |

#### Example

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

// Using built-in rules
let db = MagicDatabase::with_builtin_rules()?;
let result = db.evaluate_file("sample.bin")?;
println!("Type: {}", result.description);

// With custom configuration
let config = EvaluationConfig {
    timeout_ms: Some(5000),
    enable_mime_types: true,
    ..Default::default()
};
let db = MagicDatabase::with_builtin_rules_and_config(config)?;

// From file
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
```

---

### EvaluationConfig

Configuration for rule evaluation behavior.

```rust
use libmagic_rs::EvaluationConfig;
```

#### Fields

| Field                 | Type          | Default | Description                              |
| --------------------- | ------------- | ------- | ---------------------------------------- |
| `max_recursion_depth` | `u32`         | 20      | Maximum nesting depth for rules (1-1000) |
| `max_string_length`   | `usize`       | 8192    | Maximum string bytes to read (1-1MB)     |
| `stop_at_first_match` | `bool`        | `true`  | Stop after first match                   |
| `enable_mime_types`   | `bool`        | `false` | Map results to MIME types                |
| `timeout_ms`          | `Option<u64>` | `None`  | Evaluation timeout (1-300000ms)          |

#### Preset Configurations

```rust
// Default balanced settings
let config = EvaluationConfig::default();

// Optimized for speed
let config = EvaluationConfig::performance();
// - max_recursion_depth: 10
// - max_string_length: 1024
// - stop_at_first_match: true
// - timeout_ms: Some(1000)

// Optimized for completeness
let config = EvaluationConfig::comprehensive();
// - max_recursion_depth: 50
// - max_string_length: 32768
// - stop_at_first_match: false
// - enable_mime_types: true
// - timeout_ms: Some(30000)
```

#### Validation

```rust
let config = EvaluationConfig {
    max_recursion_depth: 25,
    max_string_length: 16384,
    ..Default::default()
};

// Validate configuration
config.validate()?;
```

---

### EvaluationResult

Result of magic rule evaluation.

```rust
use libmagic_rs::EvaluationResult;
```

#### Fields

| Field         | Type                 | Description                          |
| ------------- | -------------------- | ------------------------------------ |
| `description` | `String`             | Human-readable file type description |
| `mime_type`   | `Option<String>`     | MIME type (if enabled)               |
| `confidence`  | `f64`                | Confidence score (0.0-1.0)           |
| `matches`     | `Vec<MatchResult>`   | Individual match results             |
| `metadata`    | `EvaluationMetadata` | Evaluation diagnostics               |

#### Example

```rust
let result = db.evaluate_file("document.pdf")?;

println!("Description: {}", result.description);
println!("Confidence: {:.0}%", result.confidence * 100.0);

if let Some(mime) = &result.mime_type {
    println!("MIME Type: {}", mime);
}

println!("Evaluation time: {:.2}ms", result.metadata.evaluation_time_ms);
```

---

### EvaluationMetadata

Diagnostic information about the evaluation process.

```rust
use libmagic_rs::EvaluationMetadata;
```

#### Fields

| Field                | Type              | Description                    |
| -------------------- | ----------------- | ------------------------------ |
| `file_size`          | `u64`             | Size of analyzed file in bytes |
| `evaluation_time_ms` | `f64`             | Time taken in milliseconds     |
| `rules_evaluated`    | `usize`           | Number of rules tested         |
| `magic_file`         | `Option<PathBuf>` | Source magic file path         |
| `timed_out`          | `bool`            | Whether evaluation timed out   |

---

## Error Handling

### LibmagicError

Main error type for all library operations.

```rust
use libmagic_rs::LibmagicError;
```

#### Variants

| Variant                            | Description                 |
| ---------------------------------- | --------------------------- |
| `ParseError(ParseError)`           | Magic file parsing error    |
| `EvaluationError(EvaluationError)` | Rule evaluation error       |
| `IoError(std::io::Error)`          | File I/O error              |
| `Timeout { timeout_ms }`           | Evaluation timeout exceeded |

### ParseError

Errors during magic file parsing.

| Variant                                            | Description                     |
| -------------------------------------------------- | ------------------------------- |
| `InvalidSyntax { line, message }`                  | Invalid syntax in magic file    |
| `UnsupportedFeature { line, feature }`             | Unsupported feature encountered |
| `InvalidOffset { line, offset }`                   | Invalid offset specification    |
| `InvalidType { line, type_spec }`                  | Invalid type specification      |
| `InvalidOperator { line, operator }`               | Invalid operator                |
| `InvalidValue { line, value }`                     | Invalid value                   |
| `UnsupportedFormat { line, format_type, message }` | Unsupported file format         |
| `IoError(std::io::Error)`                          | I/O error during parsing        |

### EvaluationError

Errors during rule evaluation.

| Variant                                       | Description                        |
| --------------------------------------------- | ---------------------------------- |
| `BufferOverrun { offset }`                    | Read beyond buffer bounds          |
| `InvalidOffset { offset }`                    | Invalid offset calculation         |
| `UnsupportedType { type_name }`               | Unsupported type during evaluation |
| `RecursionLimitExceeded { depth }`            | Max recursion depth exceeded       |
| `StringLengthExceeded { length, max_length }` | String too long                    |
| `InvalidStringEncoding { offset }`            | Invalid string encoding            |
| `Timeout { timeout_ms }`                      | Evaluation timeout                 |
| `InternalError { message }`                   | Internal error (bug)               |

#### Example

```rust
use libmagic_rs::{MagicDatabase, LibmagicError, ParseError};

match MagicDatabase::load_from_file("invalid.magic") {
    Ok(db) => println!("Loaded successfully"),
    Err(LibmagicError::ParseError(ParseError::InvalidSyntax { line, message })) => {
        eprintln!("Syntax error at line {}: {}", line, message);
    }
    Err(LibmagicError::IoError(e)) => {
        eprintln!("I/O error: {}", e);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Parser Module

### AST Types

#### MagicRule

Represents a parsed magic rule.

```rust
use libmagic_rs::MagicRule;
```

| Field               | Type                       | Description                                            |
| ------------------- | -------------------------- | ------------------------------------------------------ |
| `offset`            | `OffsetSpec`               | Where to read data                                     |
| `typ`               | `TypeKind`                 | Type of data to read                                   |
| `op`                | `Operator`                 | Comparison operator                                    |
| `value`             | `Value`                    | Expected value                                         |
| `message`           | `String`                   | Description message                                    |
| `children`          | `Vec<MagicRule>`           | Nested rules                                           |
| `level`             | `u32`                      | Indentation level                                      |
| `strength_modifier` | `Option<StrengthModifier>` | Optional strength modifier from `!:strength` directive |

#### OffsetSpec

Offset specification for locating data.

```rust
use libmagic_rs::OffsetSpec;
```

| Variant                                                      | Description                     |
| ------------------------------------------------------------ | ------------------------------- |
| `Absolute(i64)`                                              | Absolute offset from file start |
| `Indirect { base_offset, pointer_type, adjustment, endian }` | Indirect through pointer        |
| `Relative(i64)`                                              | Relative to previous match      |
| `FromEnd(i64)`                                               | Offset from end of file         |

#### TypeKind

Data type specifications.

```rust
use libmagic_rs::TypeKind;
```

| Variant                    | Description                                              |
| -------------------------- | -------------------------------------------------------- |
| `Byte { signed }`          | Single byte with explicit signedness (changed in v0.2.0) |
| `Short { endian, signed }` | 16-bit integer                                           |
| `Long { endian, signed }`  | 32-bit integer                                           |
| `Quad { endian, signed }`  | 64-bit integer                                           |
| `String { max_length }`    | String data                                              |

##### 64-bit Integer Types

The `Quad` variant supports six endian-signedness combinations:

| Type Specifier | Endianness | Signedness | Description                           |
| -------------- | ---------- | ---------- | ------------------------------------- |
| `quad`         | Native     | Signed     | Native-endian signed 64-bit integer   |
| `uquad`        | Native     | Unsigned   | Native-endian unsigned 64-bit integer |
| `lequad`       | Little     | Signed     | Little-endian signed 64-bit integer   |
| `ulequad`      | Little     | Unsigned   | Little-endian unsigned 64-bit integer |
| `bequad`       | Big        | Signed     | Big-endian signed 64-bit integer      |
| `ubequad`      | Big        | Unsigned   | Big-endian unsigned 64-bit integer    |

**Version Note:** In v0.2.0, the `Byte` variant changed from a unit variant to a struct variant with a `signed` field.

#### Operator

Comparison operators.

```rust
use libmagic_rs::Operator;
```

| Variant               | Description                                               |
| --------------------- | --------------------------------------------------------- |
| `Equal`               | Equality comparison (`=` or `==`)                         |
| `NotEqual`            | Inequality comparison (`!=` or `<>`)                      |
| `LessThan`            | Less than comparison (`<`) (added in v0.2.0)              |
| `GreaterThan`         | Greater than comparison (`>`) (added in v0.2.0)           |
| `LessEqual`           | Less than or equal comparison (`<=`) (added in v0.2.0)    |
| `GreaterEqual`        | Greater than or equal comparison (`>=`) (added in v0.2.0) |
| `BitwiseAnd`          | Bitwise AND (`&`)                                         |
| `BitwiseAndMask(u64)` | Bitwise AND with mask value                               |
| `BitwiseXor`          | Bitwise XOR (`^`)                                         |
| `BitwiseNot`          | Bitwise NOT/complement (`~`)                              |
| `AnyValue`            | Match any value unconditionally (`x`)                     |

**Version Note:** The comparison operators `LessThan`, `GreaterThan`, `LessEqual`, and `GreaterEqual` were added in v0.2.0.

#### Value

Value types for matching.

```rust
use libmagic_rs::Value;
```

| Variant          | Description      |
| ---------------- | ---------------- |
| `Uint(u64)`      | Unsigned integer |
| `Int(i64)`       | Signed integer   |
| `Bytes(Vec<u8>)` | Byte sequence    |
| `String(String)` | String value     |

#### Endianness

Byte order specification.

```rust
use libmagic_rs::Endianness;
```

| Variant  | Description   |
| -------- | ------------- |
| `Little` | Little-endian |
| `Big`    | Big-endian    |
| `Native` | System native |

---

## Evaluator Module

### EvaluationContext

Maintains evaluation state during rule processing.

```rust
use libmagic_rs::EvaluationContext;
```

#### Methods

| Method                         | Description                        |
| ------------------------------ | ---------------------------------- |
| `new(config)`                  | Create new context                 |
| `current_offset()`             | Get current position               |
| `set_current_offset(offset)`   | Set current position               |
| `recursion_depth()`            | Get recursion depth                |
| `increment_recursion_depth()`  | Increment depth (with limit check) |
| `decrement_recursion_depth()`  | Decrement depth                    |
| `should_stop_at_first_match()` | Check stop behavior                |
| `max_string_length()`          | Get max string length              |
| `enable_mime_types()`          | Check MIME type setting            |
| `timeout_ms()`                 | Get timeout value                  |
| `reset()`                      | Reset to initial state             |

### MatchResult (Evaluator)

Result from internal evaluation.

```rust
use libmagic_rs::evaluator::MatchResult;
```

| Field        | Type     | Description       |
| ------------ | -------- | ----------------- |
| `message`    | `String` | Match description |
| `offset`     | `usize`  | Match offset      |
| `level`      | `u32`    | Rule level        |
| `value`      | `Value`  | Matched value     |
| `confidence` | `f64`    | Confidence score  |

---

## Output Module

### MatchResult (Output)

Structured match result for output formatting.

```rust
use libmagic_rs::output::MatchResult;
```

#### Fields

| Field        | Type             | Description           |
| ------------ | ---------------- | --------------------- |
| `message`    | `String`         | File type description |
| `offset`     | `usize`          | Match offset          |
| `length`     | `usize`          | Bytes examined        |
| `value`      | `Value`          | Matched value         |
| `rule_path`  | `Vec<String>`    | Rule hierarchy        |
| `confidence` | `u8`             | Confidence (0-100)    |
| `mime_type`  | `Option<String>` | MIME type             |

#### Methods

```rust
// Create basic result
let result = MatchResult::new(
    "PNG image".to_string(),
    0,
    Value::Bytes(vec![0x89, 0x50, 0x4e, 0x47])
);

// Create with full metadata
let result = MatchResult::with_metadata(
    "JPEG image".to_string(),
    0,
    2,
    Value::Bytes(vec![0xff, 0xd8]),
    vec!["image".to_string(), "jpeg".to_string()],
    85,
    Some("image/jpeg".to_string())
);

// Modify result
result.set_confidence(90);
result.add_rule_path("subtype".to_string());
result.set_mime_type(Some("image/jpeg".to_string()));
```

### JSON Output

```rust
use libmagic_rs::output::json::{format_json_output, format_json_line_output};

// Pretty-printed JSON (single file)
let json = format_json_output(&matches)?;

// JSON Lines (multiple files)
let json_line = format_json_line_output(path, &matches)?;
```

---

## Type Aliases

| Alias       | Definition                              | Description         |
| ----------- | --------------------------------------- | ------------------- |
| `Result<T>` | `std::result::Result<T, LibmagicError>` | Library result type |

---

## Re-exports

The following types are re-exported from the root module for convenience:

```rust
// AST types
pub use parser::ast::{Endianness, MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

// Evaluator types
pub use evaluator::{EvaluationContext, MatchResult};

// Error types
pub use error::{EvaluationError, LibmagicError, ParseError};
```

---

## Feature Flags

Currently, libmagic-rs does not have optional feature flags. All functionality is included by default.

---

## Thread Safety

- `MagicDatabase` is **not** `Send` or `Sync` by default due to internal state
- `EvaluationConfig` is `Send + Sync` (plain data)
- For multi-threaded use, create separate `MagicDatabase` instances per thread or use appropriate synchronization

---

## Version Compatibility

- **Minimum Rust Version**: 1.89
- **Edition**: 2024
- **License**: Apache-2.0
