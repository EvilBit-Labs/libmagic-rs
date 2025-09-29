# AST Data Structures

The Abstract Syntax Tree (AST) is the core representation of magic rules in libmagic-rs. This chapter provides detailed documentation of the AST data structures and their usage patterns.

## Overview

The AST consists of several key types that work together to represent magic rules:

- **`MagicRule`**: The main rule structure containing all components
- **`OffsetSpec`**: Specifies where to read data in files
- **`TypeKind`**: Defines how to interpret bytes
- **`Operator`**: Comparison and bitwise operations
- **`Value`**: Expected values for matching
- **`Endianness`**: Byte order specifications

## MagicRule Structure

The `MagicRule` struct is the primary AST node representing a complete magic rule:

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
```

### Example Usage

```rust
use libmagic_rs::parser::ast::*;

// ELF magic number rule
let elf_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Long {
        endian: Endianness::Little,
        signed: false
    },
    op: Operator::Equal,
    value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]), // "\x7fELF"
    message: "ELF executable".to_string(),
    children: vec![],
    level: 0,
};
```

### Hierarchical Rules

Magic rules can contain child rules that are evaluated when the parent matches:

```rust
let parent_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Byte,
    op: Operator::Equal,
    value: Value::Uint(0x7f),
    message: "ELF".to_string(),
    children: vec![
        MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "32-bit".to_string(),
            children: vec![],
            level: 1,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte,
            op: Operator::Equal,
            value: Value::Uint(2),
            message: "64-bit".to_string(),
            children: vec![],
            level: 1,
        },
    ],
    level: 0,
};
```

## OffsetSpec Variants

The `OffsetSpec` enum defines where to read data within a file:

### Absolute Offsets

```rust
pub enum OffsetSpec {
    /// Absolute offset from file start
    Absolute(i64),
    // ... other variants
}
```

**Examples:**

```rust
// Read at byte 0 (file start)
let start = OffsetSpec::Absolute(0);

// Read at byte 16
let offset_16 = OffsetSpec::Absolute(16);

// Read 4 bytes before current position (negative offset)
let relative_back = OffsetSpec::Absolute(-4);
```

### Indirect Offsets

Indirect offsets read a pointer value and use it as the actual offset:

```rust
Indirect {
    base_offset: i64,        // Where to read the pointer
    pointer_type: TypeKind,  // How to interpret the pointer
    adjustment: i64,         // Value to add to pointer
    endian: Endianness,      // Byte order for pointer
}
```

**Example:**

```rust
// Read a 32-bit little-endian pointer at offset 0x20,
// then read data at (pointer_value + 4)
let indirect = OffsetSpec::Indirect {
    base_offset: 0x20,
    pointer_type: TypeKind::Long {
        endian: Endianness::Little,
        signed: false
    },
    adjustment: 4,
    endian: Endianness::Little,
};
```

### Relative and FromEnd Offsets

```rust
// Relative to previous match position
Relative(i64),

// Relative to end of file
FromEnd(i64),
```

**Examples:**

```rust
// 8 bytes after previous match
let relative = OffsetSpec::Relative(8);

// 16 bytes before end of file
let from_end = OffsetSpec::FromEnd(-16);
```

## TypeKind Variants

The `TypeKind` enum specifies how to interpret bytes at the given offset:

### Numeric Types

```rust
pub enum TypeKind {
    /// Single byte (8-bit)
    Byte,

    /// 16-bit integer
    Short { endian: Endianness, signed: bool },

    /// 32-bit integer
    Long { endian: Endianness, signed: bool },

    /// String data
    String { max_length: Option<usize> },
}
```

**Examples:**

```rust
// Single byte
let byte_type = TypeKind::Byte;

// 16-bit little-endian unsigned integer
let short_le = TypeKind::Short {
    endian: Endianness::Little,
    signed: false
};

// 32-bit big-endian signed integer
let long_be = TypeKind::Long {
    endian: Endianness::Big,
    signed: true
};

// Null-terminated string, max 256 bytes
let string_type = TypeKind::String {
    max_length: Some(256)
};
```

### Endianness Options

```rust
pub enum Endianness {
    Little, // Little-endian (x86, ARM in little mode)
    Big,    // Big-endian (network byte order, PowerPC)
    Native, // Host system byte order
}
```

## Operator Types

The `Operator` enum defines comparison operations:

```rust
pub enum Operator {
    Equal,      // ==
    NotEqual,   // !=
    BitwiseAnd, // & (bitwise AND for pattern matching)
}
```

**Usage Examples:**

```rust
// Exact match
let equal_op = Operator::Equal;

// Not equal
let not_equal_op = Operator::NotEqual;

// Bitwise AND (useful for flag checking)
let bitwise_op = Operator::BitwiseAnd;
```

## Value Types

The `Value` enum represents expected values for comparison:

```rust
pub enum Value {
    Uint(u64),      // Unsigned integer
    Int(i64),       // Signed integer
    Bytes(Vec<u8>), // Byte sequence
    String(String), // String value
}
```

**Examples:**

```rust
// Unsigned integer value
let uint_val = Value::Uint(0x464c457f);

// Signed integer value
let int_val = Value::Int(-1);

// Byte sequence (magic numbers)
let bytes_val = Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]); // ZIP signature

// String value
let string_val = Value::String("#!/bin/sh".to_string());
```

## Serialization Support

All AST types implement `Serialize` and `Deserialize` for caching and interchange:

```rust
use serde_json;

// Serialize a rule to JSON
let rule = MagicRule { /* ... */ };
let json = serde_json::to_string(&rule)?;

// Deserialize from JSON
let rule: MagicRule = serde_json::from_str(&json)?;
```

## Common Patterns

### ELF File Detection

```rust
let elf_rules = vec![
    MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Long { endian: Endianness::Little, signed: false },
        op: Operator::Equal,
        value: Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46]),
        message: "ELF".to_string(),
        children: vec![
            MagicRule {
                offset: OffsetSpec::Absolute(4),
                typ: TypeKind::Byte,
                op: Operator::Equal,
                value: Value::Uint(1),
                message: "32-bit".to_string(),
                children: vec![],
                level: 1,
            },
            MagicRule {
                offset: OffsetSpec::Absolute(4),
                typ: TypeKind::Byte,
                op: Operator::Equal,
                value: Value::Uint(2),
                message: "64-bit".to_string(),
                children: vec![],
                level: 1,
            },
        ],
        level: 0,
    }
];
```

### ZIP Archive Detection

```rust
let zip_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::Long { endian: Endianness::Little, signed: false },
    op: Operator::Equal,
    value: Value::Bytes(vec![0x50, 0x4b, 0x03, 0x04]),
    message: "ZIP archive".to_string(),
    children: vec![],
    level: 0,
};
```

### Script Detection with String Matching

```rust
let script_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::String { max_length: Some(32) },
    op: Operator::Equal,
    value: Value::String("#!/bin/bash".to_string()),
    message: "Bash script".to_string(),
    children: vec![],
    level: 0,
};
```

## Best Practices

### Rule Organization

1. **Start with broad patterns** and use child rules for specifics
2. **Order rules by probability** of matching (most common first)
3. **Use appropriate types** for the data being checked
4. **Minimize indirection** for performance

### Type Selection

1. **Use `Byte`** for single-byte values and flags
2. **Use `Short/Long`** with explicit endianness for multi-byte integers
3. **Use `String`** with length limits for text patterns
4. **Use `Bytes`** for exact binary sequences

### Performance Considerations

1. **Prefer absolute offsets** over indirect when possible
2. **Use bitwise AND** for flag checking instead of multiple equality rules
3. **Limit string lengths** to prevent excessive reading
4. **Structure hierarchies** to fail fast on non-matches

The AST provides a flexible, type-safe foundation for representing magic rules while maintaining compatibility with existing magic file formats.
