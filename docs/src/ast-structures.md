# AST Data Structures

The Abstract Syntax Tree (AST) is the core representation of magic rules in libmagic-rs. This chapter provides detailed documentation of the fully implemented AST data structures with comprehensive test coverage (29 unit tests) and their usage patterns.

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
    typ: TypeKind::Byte { signed: false },
    op: Operator::Equal,
    value: Value::Uint(0x7f),
    message: "ELF".to_string(),
    children: vec![
        MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: false },
            op: Operator::Equal,
            value: Value::Uint(1),
            message: "32-bit".to_string(),
            children: vec![],
            level: 1,
        },
        MagicRule {
            offset: OffsetSpec::Absolute(4),
            typ: TypeKind::Byte { signed: false },
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

> **Breaking Change in v0.2.0:** The `Byte` variant changed from a unit variant (`Byte`) to a struct variant (`Byte { signed: bool }`). Code that pattern-matches exhaustively on `TypeKind` requires updates.

### Numeric Types

```rust
pub enum TypeKind {
    /// Single byte (8-bit)
    Byte { signed: bool },

    /// 16-bit integer
    Short { endian: Endianness, signed: bool },

    /// 32-bit integer
    Long { endian: Endianness, signed: bool },

    /// 64-bit integer
    Quad { endian: Endianness, signed: bool },

    /// String data
    String {
        max_length: Option<usize>,
        flags: StringFlags,
    },

    /// Pascal string (length-prefixed)
    PString {
        max_length: Option<usize>,
        length_width: PStringLengthWidth,
        length_includes_itself: bool,
    },

    /// Regular expression pattern matching
    Regex {
        flags: RegexFlags,
        count: RegexCount,
    },

    /// Bounded literal byte sequence search
    Search {
        range: NonZeroUsize,
    },
}
```

**Examples:**

```rust
// Single unsigned byte
let byte_type = TypeKind::Byte { signed: false };

// Single signed byte
let signed_byte_type = TypeKind::Byte { signed: true };

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

// 64-bit little-endian unsigned integer
let quad_le = TypeKind::Quad {
    endian: Endianness::Little,
    signed: false
};

// 64-bit big-endian signed integer
let quad_be = TypeKind::Quad {
    endian: Endianness::Big,
    signed: true
};

// Null-terminated string, max 256 bytes
let string_type = TypeKind::String {
    max_length: Some(256),
    flags: StringFlags::default(),
};
```

### PString (Pascal String)

Pascal-style length-prefixed strings where the length prefix can be 1, 2, or 4 bytes depending on the `length_width` field.

**Structure:**

- Length prefix: 1, 2, or 4 bytes indicating string length, with configurable endianness
- String data: The number of bytes specified by the length prefix

**Example:**

```text
0    pstring    JPEG
0    pstring/H  JPEG
```

The first line reads a 1-byte length prefix (default), then reads that many bytes as a string. The second line reads a 2-byte big-endian length prefix.

**Behavior:**

- Returns `Value::String` containing the string data (without the length prefix)
- Performs bounds checking on both the length prefix and the string data
- Supports all string comparison operators
- Length prefix width controlled by `PStringLengthWidth` enum
- Optional `/J` flag indicates JPEG-style self-inclusive length (stored length includes the prefix itself)

### PStringLengthWidth Enum

The `PStringLengthWidth` enum specifies the width and endianness of the length prefix:

```rust
pub enum PStringLengthWidth {
    /// 1-byte length prefix (default, `/B` suffix)
    OneByte,
    /// 2-byte big-endian length prefix (`/H` suffix)
    TwoByteBE,
    /// 2-byte little-endian length prefix (`/h` suffix)
    TwoByteLE,
    /// 4-byte big-endian length prefix (`/L` suffix)
    FourByteBE,
    /// 4-byte little-endian length prefix (`/l` suffix)
    FourByteLE,
}
```

**Suffix conventions:**

- `/B` - 1-byte length prefix (default if no suffix specified)
- `/H` - 2-byte big-endian length prefix
- `/h` - 2-byte little-endian length prefix
- `/L` - 4-byte big-endian length prefix
- `/l` - 4-byte little-endian length prefix
- `/J` - Length includes the prefix width itself (combinable: `/HJ`, `/lJ`, etc.)

**Examples:**

```rust
use libmagic_rs::parser::ast::{TypeKind, PStringLengthWidth};

// 1-byte length prefix (default)
let pstring_default = TypeKind::PString {
    max_length: None,
    length_width: PStringLengthWidth::OneByte,
    length_includes_itself: false,
};

// 2-byte big-endian length prefix
let pstring_be = TypeKind::PString {
    max_length: None,
    length_width: PStringLengthWidth::TwoByteBE,
    length_includes_itself: false,
};

// 4-byte little-endian length prefix
let pstring_le = TypeKind::PString {
    max_length: None,
    length_width: PStringLengthWidth::FourByteLE,
    length_includes_itself: false,
};

// 2-byte big-endian with /J flag (JPEG-style self-inclusive length)
let pstring_jpeg = TypeKind::PString {
    max_length: None,
    length_width: PStringLengthWidth::TwoByteBE,
    length_includes_itself: true,
};

// Maximum 64-byte limit with 1-byte prefix
let limited_pstring = TypeKind::PString {
    max_length: Some(64),
    length_width: PStringLengthWidth::OneByte,
    length_includes_itself: false,
};
```

### Regex (Regular Expression Pattern Matching)

The `Regex` variant matches POSIX-extended regular expression patterns against file buffers. Patterns are binary-safe and always compiled with multi-line mode enabled (matching `^` and `$` at line boundaries). The scan window is capped at 8192 bytes (`FILE_REGEX_MAX`) regardless of the `count` variant.

**Structure:**

```rust
Regex {
    flags: RegexFlags,
    count: RegexCount,
}
```

**Fields:**

- `flags`: Modifier flags from the `/c` and `/s` suffixes (case-insensitive, start-offset). The `/l` suffix is NOT a flag — it selects the `RegexCount::Lines` variant below.
- `count`: Scan window specifier, one of three variants (see `RegexCount` below).

**Example:**

```text
0    regex     [0-9]+      Numeric content
0    regex/1l  ^#!/        Shebang on first line
0    regex/cs  json        Case-insensitive "json", anchor at match-start
```

**Behavior:**

- Returns `Value::String` containing the matched text
- Scan window capped at 8192 bytes (GNU `file` `FILE_REGEX_MAX`)
- Multi-line mode unconditional (`^`/`$` match line boundaries, `.` does not match newlines)
- Zero-width matches (e.g., `^`, `a*`) return `Value::String("")` and are distinguished from no-match
- Only supports `Equal` and `NotEqual` operators; other comparison operators return `TypeReadError::UnsupportedType`

### RegexFlags Struct

The `RegexFlags` struct specifies regex behavior modifiers. All flags default to `false` via `RegexFlags::default`.

```rust
pub struct RegexFlags {
    /// `/c` - case-insensitive matching
    pub case_insensitive: bool,
    /// `/s` - advance anchor to match-start instead of match-end
    pub start_offset: bool,
}
```

The `/l` modifier is encoded by the `RegexCount::Lines` variant rather than a flag field, so "line count" and "byte count" are mutually exclusive at the type level.

### RegexCount Enum

The `RegexCount` enum specifies the scan window for a regex rule:

```rust
pub enum RegexCount {
    /// Plain `regex` with no suffix: full 8192-byte default window.
    Default,
    /// `regex/N`: scan at most `N` bytes, capped at 8192.
    Bytes(NonZeroU32),
    /// `regex/Nl` or `regex/l`: scan N line terminators (or the full
    /// capped window if `None`), capped at 8192 bytes.
    Lines(Option<NonZeroU32>),
}
```

**Variant mapping:**

- `regex` → `RegexCount::Default`
- `regex/N` → `RegexCount::Bytes(N)`
- `regex/Nl` → `RegexCount::Lines(Some(N))`
- `regex/l` → `RegexCount::Lines(None)` (behaviorally equivalent to `Default`: both walk the full 8192-byte capped window)

**Examples:**

```rust
use libmagic_rs::parser::ast::{RegexCount, RegexFlags, TypeKind};
use std::num::NonZeroU32;

// Plain regex with 8192-byte default scan window
let plain_regex = TypeKind::Regex {
    flags: RegexFlags::default(),
    count: RegexCount::Default,
};

// First line only (1 line, capped at 8192 bytes)
let first_line = TypeKind::Regex {
    flags: RegexFlags::default(),
    count: RegexCount::Lines(NonZeroU32::new(1)),
};

// Case-insensitive with anchor at match-start
let case_start = TypeKind::Regex {
    flags: RegexFlags {
        case_insensitive: true,
        start_offset: true,
    },
    count: RegexCount::Default,
};
```

### StringFlags Struct

The `StringFlags` struct specifies string comparison behavior modifiers. All flags default to `false` via `StringFlags::default()`.

```rust
pub struct StringFlags {
    pub compact_whitespace: bool,
    pub compact_optional_whitespace: bool,
    pub ignore_lowercase: bool,
    pub ignore_uppercase: bool,
    pub text_test: bool,
    pub trim: bool,
    pub bin_test: bool,
    pub full_word: bool,
}
```

**Flag meanings:**

- `/W` (`compact_whitespace`) — pattern whitespace requires at least one whitespace byte in the file, then consumes remaining whitespace greedily
- `/w` (`compact_optional_whitespace`) — pattern whitespace matches zero or more whitespace bytes in the file
- `/c` (`ignore_lowercase`) — when the pattern character is lowercase, the file byte is compared case-insensitively; uppercase pattern characters require exact match (asymmetric)
- `/C` (`ignore_uppercase`) — when the pattern character is uppercase, the file byte is compared case-insensitively; lowercase pattern characters require exact match (asymmetric)
- `/t` (`text_test`) — hint that this rule applies to text files (captured for MIME output integration)
- `/T` (`trim`) — trim leading and trailing ASCII whitespace from the pattern before comparison
- `/b` (`bin_test`) — hint that this rule applies to binary files (captured for MIME output integration)
- `/f` (`full_word`) — post-match check that the byte after the matched region is either end-of-buffer or a non-word character

**Examples:**

```rust
use libmagic_rs::parser::ast::{StringFlags, TypeKind};

// Plain string with byte-exact comparison
let plain_string = TypeKind::String {
    max_length: Some(32),
    flags: StringFlags::default(),
};

// Case-insensitive string matching
let case_insensitive = TypeKind::String {
    max_length: Some(32),
    flags: StringFlags::default().with_ignore_lowercase(true),
};

// Whitespace-flexible string matching
let whitespace_flex = TypeKind::String {
    max_length: Some(64),
    flags: StringFlags::default()
        .with_compact_optional_whitespace(true),
};

// Combined flags (case-insensitive + whitespace-optional)
let combined = TypeKind::String {
    max_length: Some(128),
    flags: StringFlags::default()
        .with_ignore_lowercase(true)
        .with_compact_optional_whitespace(true),
};
```

`StringFlags::default()` constructs a value with every flag set to `false`. The `is_empty(self) -> bool` method (takes `self` by value; `StringFlags` is `Copy`) returns `true` for such a value, and the engine uses it as the fast-path predicate: rules with default flags take the byte-exact comparison path, while any non-default flag routes the rule through `compare_string_with_flags`.

**Note:** `/B` is NOT a string flag — it is the `pstring` 1-byte length-width letter. `string/B` is rejected at parse time.

### Meta-types (Control Directives)

The `Meta` variant represents pseudo-types that do not read bytes from the buffer. They encode control-flow directives inherited from the libmagic magic(5) format.

**Structure:**

```rust
/// Control-flow directives that do not read bytes from the buffer.
Meta(MetaType),
```

`TypeKind::Meta(_)` returns `None` from `bit_width()` because meta-types consume zero on-disk bytes.

**MetaType Enum:**

```rust
pub enum MetaType {
    /// `default` — fires only when no sibling at the same level has matched.
    Default,
    /// `clear` — resets the per-level sibling-matched flag.
    Clear,
    /// `name <id>` — defines a named subroutine (hoisted out of the rule list at load time).
    Name(String),
    /// `use <id>` — invokes a named subroutine at the resolved offset.
    Use(String),
    /// `indirect` — re-applies the full rule database at the resolved offset.
    Indirect,
    /// `offset` — emits the resolved file position as `Value::Uint` for
    /// printf-style format substitution (e.g. `%lld`).
    Offset,
}
```

**Examples:**

```rust
use libmagic_rs::parser::ast::{TypeKind, MetaType};

// Default fallback rule
let default_rule = TypeKind::Meta(MetaType::Default);

// Clear sibling-matched flag
let clear_rule = TypeKind::Meta(MetaType::Clear);

// Named subroutine declaration
let name_rule = TypeKind::Meta(MetaType::Name("part2".to_string()));

// Subroutine invocation
let use_rule = TypeKind::Meta(MetaType::Use("part2".to_string()));

// Re-entry into root rules
let indirect_rule = TypeKind::Meta(MetaType::Indirect);

// Report the resolved file offset for format substitution
let offset_rule = TypeKind::Meta(MetaType::Offset);
```

**Parse-time Name Extraction:**

Top-level `name <id>` rules are hoisted out of `ParsedMagic::rules` by `parser::name_table::extract_name_table` and placed into the `name_table: NameTable` field of `ParsedMagic` keyed by identifier. As a result, `MetaType::Name` variants in the final parsed rule list are expected only as an internal intermediate representation — `name` rules do not survive past the load boundary in normal operation.

### Search (Bounded Literal Byte Sequence Search)

The `Search` variant scans for a literal byte pattern within a bounded range. Unlike `String`, which matches only at the exact offset, `Search` scans forward up to `range` bytes for the first occurrence.

**Structure:**

```rust
Search {
    range: NonZeroUsize,
}
```

**Fields:**

- `range`: Mandatory scan window width in bytes (must be non-zero per GNU `file` magic(5) specification)

**Example:**

```text
0    search/256    PK\003\004    ZIP archive within first 256 bytes
```

**Behavior:**

- Returns `Value::String` containing the matched bytes if found within range
- Anchor advances to `match_idx + pattern.len()` (the byte position just past the matched needle), matching GNU `file`'s `softmagic.c` `FILE_SEARCH` path where `ms->search.offset += idx` and then `moffset()` adds `vlen = m->vallen`. An earlier implementation incorrectly advanced by the full window size (`range`), but this caused relative-offset children to land far past the intended byte.
- Only supports `Equal` and `NotEqual` operators
- Range is mandatory; `search/0` or bare `search` are parse errors

**Examples:**

```rust
use libmagic_rs::parser::ast::TypeKind;
use std::num::NonZeroUsize;

// Scan up to 256 bytes for the pattern
let bounded_search = TypeKind::Search {
    range: NonZeroUsize::new(256).unwrap(),
};

// Scan up to 1024 bytes
let wide_search = TypeKind::Search {
    range: NonZeroUsize::new(1024).unwrap(),
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

The `Operator` enum defines comparison and bitwise operations:

```rust
pub enum Operator {
    Equal,          // == (equality comparison)
    NotEqual,       // != (inequality comparison)
    LessThan,       // < (less-than comparison)
    GreaterThan,    // > (greater-than comparison)
    LessEqual,      // <= (less-than-or-equal comparison)
    GreaterEqual,   // >= (greater-than-or-equal comparison)
    BitwiseAnd,     // & (bitwise AND for pattern matching)
    BitwiseAndMask(u64), // & (bitwise AND with mask value)
}
```

> **Added in v0.2.0:** The comparison operators `LessThan`, `GreaterThan`, `LessEqual`, and `GreaterEqual` were added. This is a breaking change for exhaustive matches on `Operator`.

**Usage Examples:**

```rust
// Exact match
let equal_op = Operator::Equal;

// Not equal
let not_equal_op = Operator::NotEqual;

// Less than comparison
let less_op = Operator::LessThan;

// Greater than comparison
let greater_op = Operator::GreaterThan;

// Less than or equal
let less_equal_op = Operator::LessEqual;

// Greater than or equal
let greater_equal_op = Operator::GreaterEqual;

// Bitwise AND (useful for flag checking)
let bitwise_op = Operator::BitwiseAnd;

// Bitwise AND with mask
let bitwise_mask_op = Operator::BitwiseAndMask(0xFF00);
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

All AST types implement `Serialize` and `Deserialize` for caching and interchange with comprehensive test coverage:

```rust
use serde_json;

// Serialize a rule to JSON (fully tested)
let rule = MagicRule { /* ... */ };
let json = serde_json::to_string(&rule)?;

// Deserialize from JSON (fully tested)
let rule: MagicRule = serde_json::from_str(&json)?;

// All edge cases are tested including:
// - Empty collections (Vec::new(), String::new())
// - Extreme values (u64::MAX, i64::MIN, i64::MAX)
// - Complex nested structures with multiple levels
// - All enum variants and their serialization round-trips
```

**Implementation Status:**

- ✅ **Complete serialization** for all AST types
- ✅ **Comprehensive testing** with edge cases and boundary values
- ✅ **JSON compatibility** for rule caching and interchange
- ✅ **Round-trip validation** ensuring data integrity

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
                typ: TypeKind::Byte { signed: false },
                op: Operator::Equal,
                value: Value::Uint(1),
                message: "32-bit".to_string(),
                children: vec![],
                level: 1,
            },
            MagicRule {
                offset: OffsetSpec::Absolute(4),
                typ: TypeKind::Byte { signed: false },
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
use libmagic_rs::parser::ast::{MagicRule, OffsetSpec, TypeKind, Operator, Value, StringFlags};

let script_rule = MagicRule {
    offset: OffsetSpec::Absolute(0),
    typ: TypeKind::String {
        max_length: Some(32),
        flags: StringFlags::default(),
    },
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

1. **Use `Byte { signed }`** for single-byte values and flags, specifying signedness
2. **Use `Short/Long/Quad`** with explicit endianness and signedness for multi-byte integers
3. **Use `String`** with length limits for text patterns at exact offsets
4. **Use `PString`** for Pascal-style length-prefixed strings
5. **Use `Regex`** for pattern matching (complex patterns, line-based checks, case-insensitive matching)
6. **Use `Search`** for simple substring matching within a bounded range (faster than regex for literal patterns)
7. **Use `Bytes`** for exact binary sequences

### Performance Considerations

1. **Prefer absolute offsets** over indirect when possible
2. **Use bitwise AND** for flag checking instead of multiple equality rules
3. **Limit string lengths** to prevent excessive reading
4. **Structure hierarchies** to fail fast on non-matches

The AST provides a flexible, type-safe foundation for representing magic rules while maintaining compatibility with existing magic file formats.
