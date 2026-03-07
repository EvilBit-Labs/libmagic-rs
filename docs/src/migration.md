# Migration from libmagic

This guide helps you migrate from the C-based libmagic library to libmagic-rs, covering API differences, compatibility considerations, and best practices.

## API Comparison

### C libmagic API

```c
#include <magic.h>

magic_t magic = magic_open(MAGIC_MIME_TYPE);
magic_load(magic, NULL);
const char* result = magic_file(magic, "example.bin");
printf("MIME type: %s\n", result);
magic_close(magic);
```

### libmagic-rs API

```rust,no_run
use libmagic_rs::MagicDatabase;

// Using built-in rules (no external magic file needed)
let db = MagicDatabase::with_builtin_rules()?;
let result = db.evaluate_file("example.bin")?;
println!("File type: {}", result.description);

// Or load from a magic file / directory
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
let result = db.evaluate_file("example.bin")?;
println!("File type: {}", result.description);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API Mapping

| C libmagic                      | libmagic-rs                                        | Notes                         |
| ------------------------------- | -------------------------------------------------- | ----------------------------- |
| `magic_open(flags)`             | `MagicDatabase::with_builtin_rules()`              | No manual flag management     |
| `magic_load(magic, path)`       | `MagicDatabase::load_from_file(path)`              | Auto-detects format           |
| `magic_file(magic, path)`       | `db.evaluate_file(path)`                           | Returns structured result     |
| `magic_buffer(magic, buf, len)` | `db.evaluate_buffer(buf)`                          | Safe slice, no length needed  |
| `magic_error(magic)`            | `Result<T, LibmagicError>`                         | Typed errors, no global state |
| `magic_close(magic)`            | (automatic)                                        | RAII cleanup on drop          |
| `MAGIC_MIME_TYPE`               | `EvaluationConfig { enable_mime_types: true, .. }` | Opt-in configuration          |

## Key Differences

### Memory Safety

- **C libmagic**: Manual memory management, potential for leaks/corruption
- **libmagic-rs**: Automatic memory management, compile-time safety guarantees

### Error Handling

- **C libmagic**: Error codes and global error state
- **libmagic-rs**: `Result` types with structured errors (`ParseError`, `EvaluationError`, `ConfigError`, `Timeout`)

### Thread Safety

- **C libmagic**: Requires careful synchronization
- **libmagic-rs**: `MagicDatabase` is safe to share across threads via `Arc`

## Migration Strategies

### Direct Replacement

For simple use cases, libmagic-rs can be a drop-in replacement:

```rust
// Before (C)
// const char* type = magic_file(magic, path);

// After (Rust)
let result = db.evaluate_file(path)?;
let type_str = &result.description;
```

### Gradual Migration

For complex applications:

1. **Start with new code**: Use libmagic-rs for new features
2. **Wrap existing code**: Create Rust wrappers around C libmagic calls
3. **Replace incrementally**: Migrate modules one at a time
4. **Remove C dependency**: Complete the migration

## Compatibility Notes

### Magic File Format

- **Supported**: Standard magic file syntax
- **Extensions**: Additional features planned (regex, etc.)
- **Compatibility**: Existing magic files should work

### Output Format

- **Text mode**: Compatible with GNU `file` command
- **JSON mode**: New structured format for modern applications
- **MIME types**: Similar to `file --mime-type`

### Performance

- **Memory usage**: Comparable to C libmagic
- **Speed**: Target within 10% of C performance
- **Startup**: Faster with compiled rule caching

## Common Migration Issues

### Error Handling Patterns

**C libmagic:**

```c
if (magic_load(magic, NULL) != 0) {
    fprintf(stderr, "Error: %s\n", magic_error(magic));
    return -1;
}
```

**libmagic-rs:**

```rust,ignore
use libmagic_rs::{MagicDatabase, LibmagicError};

let db = match MagicDatabase::load_from_file("magic.db") {
    Ok(db) => db,
    Err(LibmagicError::IoError(e)) => {
        eprintln!("File error: {}", e);
        return Err(LibmagicError::IoError(e));
    }
    Err(LibmagicError::ParseError(e)) => {
        eprintln!("Parse error: {}", e);
        return Err(LibmagicError::ParseError(e));
    }
    Err(e) => return Err(e),
};
```

### Resource Management

**C libmagic:**

```c
magic_t magic = magic_open(flags);
// ... use magic ...
magic_close(magic);  // Manual cleanup required
```

**libmagic-rs:**

```rust
{
    let db = MagicDatabase::load_from_file("magic.db")?;
    // ... use db ...
}  // Automatic cleanup when db goes out of scope
```

## Best Practices

### Error Handling

- Use `?` operator for error propagation
- Match on specific error types when needed
- Provide context with error messages

### Performance

- Reuse `MagicDatabase` instances when possible
- Consider caching for frequently accessed files
- Use appropriate configuration for your use case

### Testing

- Test with your existing magic files
- Verify output compatibility with your applications
- Benchmark performance for your workload

## Future Compatibility

libmagic-rs aims to maintain compatibility with:

- **Standard magic file format**: Core syntax will remain supported
- **GNU file output**: Text output format compatibility
- **Common use cases**: Drop-in replacement for most applications

## Migrating from v0.1.x to v0.2.0

Version 0.2.0 introduces breaking changes to support comparison operators and improve type handling. Update your code as follows:

### TypeKind::Byte Variant Change

The `Byte` variant changed from a unit variant to a struct variant with a `signed` field.

**Before (v0.1.x):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte => {
        // Handle byte type
    }
    _ => {}
}

// Constructing
let byte_type = TypeKind::Byte;
```

**After (v0.2.0):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte { signed } => {
        // Handle byte type, check signedness if needed
        if signed {
            // Handle signed byte
        } else {
            // Handle unsigned byte
        }
    }
    _ => {}
}

// Constructing
let signed_byte = TypeKind::Byte { signed: true };
let unsigned_byte = TypeKind::Byte { signed: false };
```

### New Operator Variants

The `Operator` enum added four comparison operators. Exhaustive matches must handle these variants.

**Before (v0.1.x):**

```rust,ignore
use libmagic_rs::parser::ast::Operator;

match operator {
    Operator::Equal => { /* ... */ }
    Operator::NotEqual => { /* ... */ }
    // Other existing variants
}
```

**After (v0.2.0):**

```rust,ignore
use libmagic_rs::parser::ast::Operator;

match operator {
    Operator::Equal => { /* ... */ }
    Operator::NotEqual => { /* ... */ }
    Operator::LessThan => { /* ... */ }
    Operator::GreaterThan => { /* ... */ }
    Operator::LessEqual => { /* ... */ }
    Operator::GreaterEqual => { /* ... */ }
    // Other existing variants
}
```

### read_byte Function Signature

The `libmagic_rs::evaluator::types::read_byte` function signature changed from 2 to 3 parameters.

**Before (v0.1.x):**

```rust,ignore
use libmagic_rs::evaluator::types::read_byte;

let value = read_byte(buffer, offset)?;
```

**After (v0.2.0):**

```rust,ignore
use libmagic_rs::evaluator::types::read_byte;

// The third parameter indicates signedness
let signed_value = read_byte(buffer, offset, true)?;
let unsigned_value = read_byte(buffer, offset, false)?;
```

## Migrating from v0.2.x to v0.3.0

Version 0.3.0 adds support for 64-bit quad integers and renames a core evaluator type. Update your code as follows:

### New TypeKind::Quad Variant

A new `Quad` variant was added to the `TypeKind` enum for 64-bit integer types. Exhaustive matches on `TypeKind` must handle the new variant.

**Before (v0.2.x):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte { signed } => { /* ... */ }
    TypeKind::Short { endian, signed } => { /* ... */ }
    TypeKind::Long { endian, signed } => { /* ... */ }
    TypeKind::String { max_length } => { /* ... */ }
}
```

**After (v0.3.0):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte { signed } => { /* ... */ }
    TypeKind::Short { endian, signed } => { /* ... */ }
    TypeKind::Long { endian, signed } => { /* ... */ }
    TypeKind::Quad { endian, signed } => {
        // Handle 64-bit quad integer type
    }
    TypeKind::String { max_length } => { /* ... */ }
}
```

### TypeKind Variant Discriminant Changes

The addition of the `Quad` variant changed the discriminant value of the `String` variant from 3 to 4. Code using numeric casts on `TypeKind` variants must be updated.

**Before (v0.2.x):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

let type_kind = TypeKind::String { max_length: None };
let discriminant = type_kind as isize;  // Returns 3
```

**After (v0.3.0):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

let type_kind = TypeKind::String { max_length: None };
let discriminant = type_kind as isize;  // Returns 4
```

**Recommendation:** Avoid relying on enum discriminant values. Use pattern matching or the `std::mem::discriminant` function instead.

### MatchResult Renamed to RuleMatch

The `MatchResult` struct in `libmagic_rs::evaluator` was renamed to `RuleMatch` for clarity.

**Before (v0.2.x):**

```rust,ignore
use libmagic_rs::evaluator::MatchResult;

let match_result = MatchResult {
    message: "ELF executable".to_string(),
    offset: 0,
    level: 0,
    value: Value::Uint(0x7f),
    confidence: MatchResult::calculate_confidence(0),
};
```

**After (v0.3.0):**

```rust,ignore
use libmagic_rs::evaluator::RuleMatch;

let match_result = RuleMatch {
    message: "ELF executable".to_string(),
    offset: 0,
    level: 0,
    value: Value::Uint(0x7f),
    confidence: RuleMatch::calculate_confidence(0),
};
```

Update all references from `MatchResult` to `RuleMatch` in type annotations, function signatures, and construction sites.

## Migrating from v0.3.x to v0.4.0

Version 0.4.0 adds three new operator variants to the `Operator` enum for extended bitwise operations and pattern matching capabilities.

### New Operator Enum Variants

Three variants were added to the `Operator` enum:

- `BitwiseXor` (magic file symbol: `^`)
- `BitwiseNot` (magic file symbol: `~`)
- `AnyValue` (magic file symbol: `x`)

**Impact:** Since the `Operator` enum is exhaustive (not marked with `#[non_exhaustive]`), any code with exhaustive pattern matching on `Operator` must be updated to handle these variants.

**Before (v0.3.x):**

```rust,ignore
use libmagic_rs::parser::ast::Operator;

match operator {
    Operator::Equal => { /* ... */ }
    Operator::NotEqual => { /* ... */ }
    Operator::BitwiseAnd => { /* ... */ }
    Operator::BitwiseOr => { /* ... */ }
    // ... other existing variants
}
```

**After (v0.4.0):**

```rust,ignore
use libmagic_rs::parser::ast::Operator;

match operator {
    Operator::Equal => { /* ... */ }
    Operator::NotEqual => { /* ... */ }
    Operator::BitwiseAnd => { /* ... */ }
    Operator::BitwiseOr => { /* ... */ }
    Operator::BitwiseXor => { /* handle XOR */ }
    Operator::BitwiseNot => { /* handle NOT */ }
    Operator::AnyValue => { /* handle any value x */ }
    // ... other existing variants
}
```

**Alternative:** If your code does not need to handle all operators specifically, use a wildcard pattern:

```rust,ignore
match operator {
    Operator::Equal => { /* specific handling */ }
    _ => { /* generic handling for all other operators */ }
}
```

These operators enable fuller support for libmagic file format specifications and extend bitwise operation capabilities.

## Migrating from v0.4.x to v0.5.0

Version 0.5.0 adds support for floating-point types in magic file parsing. This enables detection of file formats that use IEEE 754 float and double values.

### RuleMatch Struct Field Addition

The `RuleMatch` struct gained a new `type_kind` field that carries the source `TypeKind` used to read the matched value. Code constructing `RuleMatch` instances with struct literals must add this field.

**Before (v0.4.x):**

```rust,ignore
use libmagic_rs::evaluator::RuleMatch;
use libmagic_rs::parser::ast::Value;

let match_result = RuleMatch {
    message: "ELF executable".to_string(),
    offset: 0,
    level: 0,
    value: Value::Uint(0x7f),
    confidence: RuleMatch::calculate_confidence(0),
};
```

**After (v0.5.0):**

```rust,ignore
use libmagic_rs::evaluator::RuleMatch;
use libmagic_rs::parser::ast::{Value, TypeKind, Endianness};

let match_result = RuleMatch {
    message: "ELF executable".to_string(),
    offset: 0,
    level: 0,
    value: Value::Uint(0x7f),
    type_kind: TypeKind::Byte { signed: false },
    confidence: RuleMatch::calculate_confidence(0),
};
```

The `type_kind` field allows output formatters and other consumers to determine the on-disk width of the matched value.

### Value Enum Changes

The `Value` enum added a `Float(f64)` variant for floating-point values and no longer derives the `Eq` trait (it still implements `PartialEq`).

**New Float Variant:**

```rust,ignore
use libmagic_rs::parser::ast::Value;

// New floating-point variant
let float_value = Value::Float(3.14);
let double_value = Value::Float(2.71828);
```

**Eq Trait Removal:**

Code using `Eq` as a trait bound or relying on exact equality semantics must be updated:

```rust,ignore
// Before (v0.4.x) - Eq was available
fn compare_values<T: Eq>(a: T, b: T) -> bool {
    a == b
}

// After (v0.5.0) - Use PartialEq instead
fn compare_values<T: PartialEq>(a: T, b: T) -> bool {
    a == b
}
```

Exact equality comparisons on `Value::Float` follow IEEE 754 semantics (NaN != NaN).

### TypeKind Enum Extensions

Two new variants were added to `TypeKind` for floating-point types. Exhaustive pattern matches must handle these variants.

**New Variants:**

- `Float { endian: Endianness }` - 32-bit IEEE 754 floating-point
- `Double { endian: Endianness }` - 64-bit IEEE 754 double-precision

**Before (v0.4.x):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte { signed } => { /* ... */ }
    TypeKind::Short { endian, signed } => { /* ... */ }
    TypeKind::Long { endian, signed } => { /* ... */ }
    TypeKind::Quad { endian, signed } => { /* ... */ }
    TypeKind::String { max_length } => { /* ... */ }
}
```

**After (v0.5.0):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

match type_kind {
    TypeKind::Byte { signed } => { /* ... */ }
    TypeKind::Short { endian, signed } => { /* ... */ }
    TypeKind::Long { endian, signed } => { /* ... */ }
    TypeKind::Quad { endian, signed } => { /* ... */ }
    TypeKind::Float { endian } => {
        // Handle 32-bit float type
    }
    TypeKind::Double { endian } => {
        // Handle 64-bit double type
    }
    TypeKind::String { max_length } => { /* ... */ }
}
```

**String Variant Discriminant Change:**

The addition of `Float` and `Double` changed the discriminant value of the `String` variant from 4 to 6. Code casting `TypeKind` variants to integers must be updated.

**Before (v0.4.x):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

let type_kind = TypeKind::String { max_length: None };
let discriminant = type_kind as isize;  // Returns 4
```

**After (v0.5.0):**

```rust,ignore
use libmagic_rs::parser::ast::TypeKind;

let type_kind = TypeKind::String { max_length: None };
let discriminant = type_kind as isize;  // Returns 6
```

**Recommendation:** Avoid relying on enum discriminant values. Use pattern matching or the `std::mem::discriminant` function instead.

## Getting Help

If you encounter migration issues:

- Check the [troubleshooting guide](./troubleshooting.md)
- Search [existing issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- Ask questions in [discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)
- Report bugs with minimal reproduction cases
