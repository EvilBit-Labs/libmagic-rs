# API Reference

Complete API documentation for libmagic-rs library components.

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

// With custom configuration using builder methods (required since v0.6.0)
let config = EvaluationConfig::default()
    .with_timeout_ms(Some(5000))
    .with_mime_types(true);
let db = MagicDatabase::with_builtin_rules_and_config(config)?;

// From file
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
```

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

#### Builder Methods

Since v0.6.0, `EvaluationConfig` is `#[non_exhaustive]`. Use builder-style setters to construct configurations:

```rust
let config = EvaluationConfig::default()
    .with_max_recursion_depth(25)
    .with_max_string_length(16384)
    .with_stop_at_first_match(false)
    .with_mime_types(true)
    .with_timeout_ms(Some(5000));
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

## AST Types

### MagicRule

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
| `value_transform`   | (type unspecified)         | Value transformation (added in v0.6.0)                 |

### StrengthModifier

Optional modifier for rule strength calculation.

```rust
use libmagic_rs::StrengthModifier;
```

| Variant         | Description                 |
| --------------- | --------------------------- |
| `Add(i32)`      | Add to base strength        |
| `Subtract(i32)` | Subtract from base strength |
| `Multiply(i32)` | Multiply base strength      |
| `Divide(i32)`   | Divide base strength        |
| `Set(i32)`      | Set strength to fixed value |

### OffsetSpec

Offset specification for locating data.

```rust
use libmagic_rs::OffsetSpec;
```

| Variant                                                                                                     | Description                     |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------- |
| `Absolute(i64)`                                                                                             | Absolute offset from file start |
| `Indirect { base_offset, pointer_type, adjustment, endian, base_relative, adjustment_op, result_relative }` | Indirect through pointer        |
| `Relative(i64)`                                                                                             | Relative to previous match      |
| `FromEnd(i64)`                                                                                              | Offset from end of file         |

#### Changes in v0.6.0

`OffsetSpec::Indirect` added three fields:
- `base_relative: bool` - whether `base_offset` is relative to the previous match
- `adjustment_op: Option<AdjustmentOp>` - operation to apply between pointer value and adjustment
- `result_relative: bool` - whether the final computed offset is relative to the previous match

### TypeKind

Data type specifications.

```rust
use libmagic_rs::TypeKind;
```

| Variant                    | Description                                                                                 |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| `Byte { signed }`          | Single byte with explicit signedness (struct variant since v0.2.0; previously unit variant) |
| `Short { endian, signed }` | 16-bit integer                                                                              |
| `Long { endian, signed }`  | 32-bit integer                                                                              |
| `Float { endian }`         | 32-bit IEEE 754 floating-point (added in v0.5.0)                                            |
| `Double { endian }`        | 64-bit IEEE 754 double-precision floating-point (added in v0.5.0)                           |
| `String { max_length }`    | String data (discriminant changed from 4 to 6 in v0.5.0)                                    |
| `PString { max_length }`   | Pascal string - length-prefixed byte followed by string data (returns `Value::String`)      |
| `Meta(MetaType)`           | Control flow and subroutine directives for conditional execution and code reuse             |

### MetaType

Control-flow directives carried by `TypeKind::Meta`.

```rust
use libmagic_rs::parser::ast::MetaType;
```

| Variant        | Description                                                                                              |
| -------------- | -------------------------------------------------------------------------------------------------------- |
| `Default`      | Fires when no sibling at the same indentation level matched at the current offset                        |
| `Clear`        | Resets the sibling-matched flag so a later `default` sibling can fire even if an earlier sibling matched |
| `Name(String)` | Declares a named subroutine that can be invoked later via `Use`                                          |
| `Use(String)`  | Invokes a named subroutine previously declared via `Name`                                                |
| `Indirect`     | Re-applies the entire magic database at the resolved offset                                              |
| `Offset`       | Reports the current file offset as `Value::Uint(position)` rather than reading a typed value             |

#### Examples

```rust
use libmagic_rs::{TypeKind, parser::ast::MetaType};

// A default fallback rule (fires when no sibling matched)
let default_type = TypeKind::Meta(MetaType::Default);

// Define a named subroutine
let name_type = TypeKind::Meta(MetaType::Name("riff_header".to_string()));

// Invoke that subroutine at a given offset
let use_type = TypeKind::Meta(MetaType::Use("riff_header".to_string()));

// Re-enter the root rule set at a resolved offset (ZIP-in-DOCX etc.)
let indirect_type = TypeKind::Meta(MetaType::Indirect);

// Emit the current file offset as a match value for printf substitution
let offset_type = TypeKind::Meta(MetaType::Offset);
```

### Operator

Comparison and bitwise operators for magic rule matching.

```rust
use libmagic_rs::Operator;
```

| Variant               | Description                                                               |
| --------------------- | ------------------------------------------------------------------------- |
| `Equal`               | Equality comparison (`=`)                                                 |
| `NotEqual`            | Inequality comparison (`!=`)                                              |
| `LessThan`            | Less than comparison (`<`) (added in v0.2.0)                              |
| `GreaterThan`         | Greater than comparison (`>`) (added in v0.2.0)                           |
| `LessEqual`           | Less than or equal comparison (`<=`) (added in v0.2.0)                    |
| `GreaterEqual`        | Greater than or equal comparison (`>=`) (added in v0.2.0)                 |
| `BitwiseAnd`          | Bitwise AND (`&`) - matches if any bits overlap                           |
| `BitwiseAndMask(u64)` | Bitwise AND with mask - masked comparison of file data                    |
| `BitwiseXor`          | Bitwise XOR (`^`) - matches if XOR result is non-zero                     |
| `BitwiseNot`          | Bitwise NOT (`~`) - compares complement of file value with expected value |
| `AnyValue`            | Match any value (`x`) - unconditional match                               |

#### Examples

```rust
use libmagic_rs::{MagicDatabase, Operator};

// Equal operator (default)
// Note: parse_magic_rule and parser::grammar module removed in v0.6.0
// This example is for conceptual reference only

// Bitwise AND - check if bit is set
// 0 byte &0x80

// Bitwise XOR - check for difference
// 0 byte ^0xFF

// Bitwise NOT - check complement
// 0 byte ~0xFF

// Any value - always matches
// 0 byte x
```

### Value

Value types for matching.

```rust
use libmagic_rs::Value;
```

| Variant          | Description                            |
| ---------------- | -------------------------------------- |
| `Uint(u64)`      | Unsigned integer                       |
| `Int(i64)`       | Signed integer                         |
| `Float(f64)`     | Floating-point value (added in v0.5.0) |
| `Bytes(Vec<u8>)` | Byte sequence                          |
| `String(String)` | String value                           |

The `Value` enum derives `PartialEq` but no longer derives `Eq` (removed in v0.5.0 to support floating-point values).

### Endianness

Byte order specification.

```rust
use libmagic_rs::Endianness;
```

| Variant  | Description   |
| -------- | ------------- |
| `Little` | Little-endian |
| `Big`    | Big-endian    |
| `Native` | System native |

## Error Types

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

## Evaluator Module

### EvaluationContext

Maintains evaluation state during rule processing.

```rust
use libmagic_rs::EvaluationContext;
```

#### Methods

| Method                        | Description                   |
| ----------------------------- | ----------------------------- |
| `new(config)`                 | Create new context            |
| `current_offset()`            | Get current position          |
| `set_current_offset(offset)`  | Set current position          |
| `recursion_depth()`           | Get recursion depth           |
| `should_stop_at_first_match()` | Check stop behavior           |
| `max_string_length()`         | Get max string length         |
| `enable_mime_types()`         | Check MIME type setting       |
| `timeout_ms()`                | Get timeout value             |
| `reset()`                     | Reset to initial state        |

#### Removed in v0.6.0

- `increment_recursion_depth()` - removed
- `decrement_recursion_depth()` - removed

### MatchResult (Evaluator)

Result from internal evaluation.

```rust
use libmagic_rs::evaluator::MatchResult;
```

| Field        | Type       | Description                                                                                                                                                                                                                                                                                                               |
| ------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `message`    | `String`   | Match description. Printf-style format specifiers (`%d`, `%i`, `%u`, `%x`, `%X`, `%o`, `%s`, `%c`, plus width/padding modifiers) are substituted with the rule's read value at output time. **Literal `%` must be escaped as `%%`** -- unescaped `%` is interpreted as a format specifier (breaking change since v0.5.0). |
| `offset`     | `usize`    | Match offset                                                                                                                                                                                                                                                                                                              |
| `level`      | `u32`      | Rule level                                                                                                                                                                                                                                                                                                                |
| `value`      | `Value`    | Matched value                                                                                                                                                                                                                                                                                                             |
| `type_kind`  | `TypeKind` | Type used to read value (added in v0.5.0)                                                                                                                                                                                                                                                                                 |
| `confidence` | `f64`      | Confidence score                                                                                                                                                                                                                                                                                                          |

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

## Type Aliases

| Alias       | Definition                              | Description         |
| ----------- | --------------------------------------- | ------------------- |
| `Result<T>` | `std::result::Result<T, LibmagicError>` | Library result type |

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

## Thread Safety

- `MagicDatabase` is `Send + Sync` (since v0.6.0) and can be shared across threads with appropriate synchronization
- `EvaluationConfig` is `Send + Sync` (plain data)
- For multi-threaded use, wrap `MagicDatabase` in `Arc<MagicDatabase>` to share a single instance, or create separate instances per thread

## Version Compatibility

- **Minimum Rust Version**: 1.85
- **Edition**: 2024
- **License**: Apache-2.0
- **Current Version**: 0.6.0

### Breaking Changes in v0.6.0

- `EvaluationConfig` is now `#[non_exhaustive]` - struct literal construction is no longer supported for external crates; use builder methods (`with_max_recursion_depth`, `with_max_string_length`, `with_stop_at_first_match`, `with_mime_types`, `with_timeout_ms`) or `..Default::default()`
- `MagicRule` has a new `value_transform` field
- `MagicDatabase` now implements `Send + Sync` (see Thread Safety section)
- Multiple enums are now `#[non_exhaustive]`: `OffsetSpec`, `LibmagicError`, `IoError`, `Operator`, `TypeReadError`, `ParseError`, `Value`, `TypeKind`, `EvaluationError` - pattern matching must include wildcard arms
- `OffsetSpec::Indirect` has new fields: `base_relative`, `adjustment_op`, `result_relative`
- `EvaluationContext` methods `increment_recursion_depth()` and `decrement_recursion_depth()` removed
- `parser::grammar` module removed along with functions like `parse_magic_rule`, `parse_offset`, `parse_number`, `parse_value`, `parse_operator`, `parse_type`, `parse_type_and_operator`, `parse_message`, `parse_comment`, `is_empty_line`, `is_comment_line`, `has_continuation`, `is_strength_directive`, `parse_strength_directive`, `parse_rule_offset`
- `parse_text_magic_file` return type changed from `Result<Vec<MagicRule>, ParseError>` to `Result<ParsedMagic, ParseError>`; callers must destructure `ParsedMagic { rules, name_table }`. `load_magic_file` and `load_magic_directory` return the same new type
- `evaluate_single_rule` parameter count changed from 2 to 3

### Breaking Changes in v0.5.0

- `TypeKind` enum: Added `Float { endian }` and `Double { endian }` variants for IEEE 754 floating-point support
- `TypeKind::String` discriminant changed from 4 to 6 to accommodate new float types
- `Value` enum: Added `Float(f64)` variant for floating-point values
- `Value` enum: No longer derives `Eq` trait (only `PartialEq` is available due to floating-point values)
- `RuleMatch` struct: Added `type_kind: TypeKind` field to indicate the type used for matching

### Breaking Changes (post-0.5.0, pre-0.6.0)

- Parser functions (`parse_text_magic_file`, `load_magic_file`, `load_magic_directory`) now return `ParsedMagic { rules, name_table }` instead of `Vec<MagicRule>`. External consumers can only access the public `rules` field — `name_table` is `pub(crate)` and managed internally by `MagicDatabase`. Typical usage: `let parsed = parse_text_magic_file(&source)?; /* use parsed.rules */`. The library wires `name_table` through `MagicDatabase::load_from_file` automatically; direct access is not required (or supported) for external code.
- Rule messages are now rendered through printf-style format substitution: specifiers like `%d`, `%x`, `%02x`, `%s`, `%lld` are replaced with the rule's read value at output time. **Literal `%` in rule messages must be escaped as `%%`.** Messages that were previously emitted verbatim with bare `%` characters will now be interpreted as format specifiers — this is a visible behavior change for existing magic files that used `%` for non-formatting purposes.

### Breaking Changes in v0.2.0

- `TypeKind::Byte` changed from a unit variant to a struct variant `Byte { signed }` to support explicit signedness
- Added comparison operators: `LessThan`, `GreaterThan`, `LessEqual`, `GreaterEqual` to the `Operator` enum (breaking change due to exhaustive enum)

For complete API documentation with examples, run:

```bash
cargo doc --open
```
