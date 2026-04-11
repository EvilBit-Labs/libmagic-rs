# Parser Implementation

The libmagic-rs parser is built using the [nom](https://github.com/Geal/nom) parser combinator library, providing a robust and efficient way to parse magic file syntax into our AST representation.

## Architecture Overview

The parser follows a modular design where individual components are implemented and tested separately, then composed into higher-level parsers:

```text
Magic File Text → Individual Parsers → Combined Parsers → Complete AST
                      ↓
              Numbers, Offsets, Operators, Values → Rules → Rule Hierarchies
```

## Implemented Components

### Number Parsing (`parse_number`)

Handles both decimal and hexadecimal number formats with comprehensive overflow protection:

```rust
// Decimal numbers
parse_number("123")    // Ok(("", 123))
parse_number("-456")   // Ok(("", -456))

// Hexadecimal numbers
parse_number("0x1a")   // Ok(("", 26))
parse_number("-0xFF")  // Ok(("", -255))
```

**Features:**

- ✅ Decimal and hexadecimal format support
- ✅ Signed and unsigned number handling
- ✅ Overflow protection with proper error reporting
- ✅ Comprehensive test coverage (15+ test cases)

### Offset Parsing (`parse_offset`)

Converts numeric values into `OffsetSpec::Absolute` variants:

```rust
// Basic offsets
parse_offset("0")      // Ok(("", OffsetSpec::Absolute(0)))
parse_offset("0x10")   // Ok(("", OffsetSpec::Absolute(16)))
parse_offset("-4")     // Ok(("", OffsetSpec::Absolute(-4)))

// With whitespace handling
parse_offset(" 123 ")  // Ok(("", OffsetSpec::Absolute(123)))
```

**Features:**

- ✅ Absolute offset parsing with full number format support
- ✅ Whitespace handling (leading and trailing)
- ✅ Negative offset support for relative positioning
- 📋 Indirect offset parsing (planned)
- 📋 Relative offset parsing (planned)

### Operator Parsing (`parse_operator`)

Parses comparison and bitwise operators with multiple syntax variants:

```rust
// Equality operators
parse_operator("=")    // Ok(("", Operator::Equal))
parse_operator("==")   // Ok(("", Operator::Equal))

// Inequality operators
parse_operator("!=")   // Ok(("", Operator::NotEqual))
parse_operator("<>")   // Ok(("", Operator::NotEqual))

// Comparison operators (v0.2.0+)
parse_operator("<")    // Ok(("", Operator::LessThan))
parse_operator(">")    // Ok(("", Operator::GreaterThan))
parse_operator("<=")   // Ok(("", Operator::LessEqual))
parse_operator(">=")   // Ok(("", Operator::GreaterEqual))

// Bitwise operators
parse_operator("&")    // Ok(("", Operator::BitwiseAnd))
parse_operator("^")    // Ok(("", Operator::BitwiseXor))
parse_operator("~")    // Ok(("", Operator::BitwiseNot))

// Any-value operator (always matches)
parse_operator("x")    // Ok(("", Operator::AnyValue))
```

**Features:**

- ✅ Multiple syntax variants for compatibility
- ✅ Precedence handling (longer operators matched first)
- ✅ Whitespace tolerance
- ✅ Invalid operator rejection with clear errors
- ✅ Ten comparison and bitwise operators supported, plus AnyValue (`x`)

**Note:** Comparison operators (`<`, `>`, `<=`, `>=`) were implemented in v0.2.0 via [#104](https://github.com/EvilBit-Labs/libmagic-rs/pull/104).

### Value Parsing (`parse_value`)

Handles multiple value types with intelligent type detection:

```rust
// String literals with escape sequences
parse_value("\"Hello\"")           // Value::String("Hello".to_string())
parse_value("\"Line1\\nLine2\"")   // Value::String("Line1\nLine2".to_string())

// Floating-point literals
parse_value("3.14")                // Value::Float(3.14)
parse_value("-1.0")                // Value::Float(-1.0)
parse_value("2.5e10")              // Value::Float(2.5e10)

// Numeric values
parse_value("123")                 // Value::Uint(123)
parse_value("-456")                // Value::Int(-456)
parse_value("0x1a")                // Value::Uint(26)

// Hex byte sequences
parse_value("\\x7f\\x45")          // Value::Bytes(vec![0x7f, 0x45])
parse_value("7f454c46")            // Value::Bytes(vec![0x7f, 0x45, 0x4c, 0x46])
```

**Features:**

- ✅ Quoted string parsing with escape sequence support
- ✅ Floating-point literal parsing with scientific notation support
- ✅ Numeric literal parsing (decimal and hexadecimal)
- ✅ Hex byte sequence parsing (with and without `\x` prefix)
- ✅ Intelligent type precedence to avoid parsing conflicts
- ✅ Comprehensive escape sequence handling (`\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`)

### Float and Double Type Parsing (`parse_float_value`)

Parses floating-point type specifiers and literals for IEEE 754 single (32-bit) and double-precision (64-bit) values:

```rust
// Float literals
parse_float_value("3.14")          // Ok(("", 3.14))
parse_float_value("-0.5")          // Ok(("", -0.5))
parse_float_value("1.0e-10")       // Ok(("", 1.0e-10))
parse_float_value("2.5E+3")        // Ok(("", 2.5e+3))
```

**Type Keywords:**

Six floating-point type keywords are supported, each mapping to `TypeKind::Float` or `TypeKind::Double` with an `Endianness` field:

- `float` - 32-bit IEEE 754, native endianness → `TypeKind::Float { endian: Endianness::Native }`
- `befloat` - 32-bit IEEE 754, big-endian → `TypeKind::Float { endian: Endianness::Big }`
- `lefloat` - 32-bit IEEE 754, little-endian → `TypeKind::Float { endian: Endianness::Little }`
- `double` - 64-bit IEEE 754, native endianness → `TypeKind::Double { endian: Endianness::Native }`
- `bedouble` - 64-bit IEEE 754, big-endian → `TypeKind::Double { endian: Endianness::Big }`
- `ledouble` - 64-bit IEEE 754, little-endian → `TypeKind::Double { endian: Endianness::Little }`

**Float Literal Grammar:**

The `parse_float_value` function recognizes standard floating-point notation with a **mandatory decimal point** to distinguish floats from integers:

```text
[-]digits.digits[{e|E}[{+|-}]digits]
```

Examples: `3.14`, `-0.5`, `1.0e-10`, `2.5E+3`

Parsed literals are stored as `Value::Float(f64)` in the AST, regardless of whether the rule uses `float` or `double` (the type determines buffer read size, not literal representation).

**Usage in Magic Rules:**

```rust
// Native-endian float comparison
0 float x        // Match any float value
0 float =3.14    // Match if float equals 3.14

// Big-endian double comparison
0 bedouble >1.5  // Match if big-endian double > 1.5
```

**Features:**

- ✅ Six type keywords for float and double with endianness variants
- ✅ Float literal parsing with decimal point, negative values, scientific notation
- ✅ `Value::Float(f64)` AST variant for floating-point literals
- ✅ Type precedence ensures floats parsed before integers (decimal point disambiguates)
- ✅ Comprehensive test coverage for all endianness variants and literal formats

**Note:** Float and double types do **not** have signed/unsigned variants. IEEE 754 handles sign internally via the sign bit, so all float types use a single `TypeKind` variant with only an `endian` field (no `signed: bool` field).

### Pascal String (pstring) Type

The parser supports Pascal-style length-prefixed strings through the `pstring` keyword with multiple length prefix width variants:

**Type Keyword:**

- `pstring` - Length-prefixed string → `TypeKind::PString { max_length: None, length_width: PStringLengthWidth::OneByte, length_includes_itself: false }`

**Length Prefix Width Variants:**

Pascal strings support multiple length prefix widths via suffix modifiers:

- `/B` - 1-byte length prefix (default) → `PStringLengthWidth::OneByte`
- `/H` - 2-byte big-endian length prefix → `PStringLengthWidth::TwoByteBE`
- `/h` - 2-byte little-endian length prefix → `PStringLengthWidth::TwoByteLE`
- `/L` - 4-byte big-endian length prefix → `PStringLengthWidth::FourByteBE`
- `/l` - 4-byte little-endian length prefix → `PStringLengthWidth::FourByteLE`

**Self-Inclusive Length Flag (`/J`):**

The `/J` flag indicates JPEG-style self-inclusive length, where the stored length value includes the length prefix bytes themselves. The evaluator subtracts the prefix width from the stored length to determine the actual string data length.

The `/J` flag can be combined with any width variant:

- `/J` - 1-byte self-inclusive (default width)
- `/BJ` - 1-byte self-inclusive (explicit)
- `/HJ` - 2-byte big-endian self-inclusive
- `/hJ` - 2-byte little-endian self-inclusive
- `/LJ` - 4-byte big-endian self-inclusive
- `/lJ` - 4-byte little-endian self-inclusive

**Format:**

Pascal strings store the length as a prefix (1, 2, or 4 bytes depending on the variant), followed by that many bytes of string data. Unlike C strings, they are not null-terminated. When the `/J` flag is used, the length value includes the prefix size itself.

**Parser Implementation:**

- Recognized by `parse_type_keyword()` in `src/parser/types.rs`
- Suffix parsing handled by `parse_pstring_suffix()` in `src/parser/grammar/mod.rs`
- Maps to `TypeKind::PString` in the AST with `length_width` and `length_includes_itself` fields
- Evaluator reads length prefix using appropriate byte order (`from_be_bytes` or `from_le_bytes`)
- Stored as `Value::String` for comparison with string operators
- Supports optional `max_length` field to cap the length value

**Usage in Magic Rules:**

```rust
// Basic pstring matching (1-byte length prefix)
0 pstring =Hello     // Match if pstring equals "Hello"
0 pstring x          // Match any pstring value

// Multi-byte length prefix variants
0 pstring/H =Test    // 2-byte big-endian length prefix
0 pstring/h =Test    // 2-byte little-endian length prefix
0 pstring/L =Test    // 4-byte big-endian length prefix
0 pstring/l =Test    // 4-byte little-endian length prefix

// JPEG-style self-inclusive length
0 pstring/J x        // 1-byte self-inclusive length
0 pstring/HJ =Data   // 2-byte big-endian self-inclusive length
0 pstring/lJ =Data   // 4-byte little-endian self-inclusive length

// With max_length constraint
0 pstring/H/64 x     // 2-byte prefix, limit read to 64 bytes
```

**Features:**

- ✅ Five length prefix width variants (1-byte, 2-byte BE/LE, 4-byte BE/LE)
- ✅ Self-inclusive length flag (`/J`) for JPEG-style length encoding
- ✅ Combinable suffix syntax (`/HJ`, `/lJ`, etc.)
- ✅ Bounds checking for both length prefix and string data
- ✅ Proper endianness handling via `from_be_bytes` / `from_le_bytes`
- ✅ UTF-8 validation with replacement character for invalid sequences
- ✅ Optional `max_length` parameter to limit string reads
- ✅ String comparison operators work with pstring values

### Date and Timestamp Types

The parser supports date and timestamp types for parsing Unix timestamps (signed seconds since epoch). There are 12 type keywords:

**32-bit timestamps (Date):**

- `date` - Native endian, UTC
- `ldate` - Native endian, local time
- `bedate` - Big-endian, UTC
- `beldate` - Big-endian, local time
- `ledate` - Little-endian, UTC
- `leldate` - Little-endian, local time

**64-bit timestamps (QDate):**

- `qdate` - Native endian, UTC
- `qldate` - Native endian, local time
- `beqdate` - Big-endian, UTC
- `beqldate` - Big-endian, local time
- `leqdate` - Little-endian, UTC
- `leqldate` - Little-endian, local time

The parser creates `TypeKind::Date` or `TypeKind::QDate` variants with appropriate endianness and UTC flags. During evaluation, timestamps are formatted as strings in the format "Www Mmm DD HH:MM:SS YYYY" to match GNU file output.

### Regex Type

The parser supports regular expression matching through the `regex` keyword, enabling POSIX-extended regex patterns against file contents:

**Type Keyword:**

- `regex` - Regular expression match → `TypeKind::Regex { flags, count }`

**Flag Support:**

Regex rules accept three modifier flags via the `/[csl]` suffix:

- `/c` - Case-insensitive matching → `RegexFlags::case_insensitive = true`
- `/s` - Advance anchor to match-start instead of match-end → `RegexFlags::start_offset = true`
- `/l` - Line-based counting (interpret count as line count) → `RegexFlags::line_based = true`

Flags can be combined in any order (`/cl`, `/lc`, `/csl` are all equivalent). The parser also accepts interleaved flag-and-count syntax matching GNU `file` semantics: `regex/1l` and `regex/l1` both parse identically.

**Optional Count Parameter:**

An optional decimal count controls the scan window:

- No count: scan 8192 bytes (default)
- `/N` (no `/l`): scan at most `N` bytes, capped at 8192
- `/Nl` (with `/l`): scan at most `N` lines, effective byte cap is `min(N * 80, 8192)`

The 8192-byte hard cap matches GNU `file`'s `FILE_REGEX_MAX` constant and prevents runaway regex scans against large buffers.

**Parsing Examples:**

```rust
// Plain regex (no flags, 8192-byte default scan window)
parse_type_and_operator("regex")
// → TypeKind::Regex { flags: RegexFlags::default(), count: None }

// Case-insensitive flag
parse_type_and_operator("regex/c")
// → TypeKind::Regex { flags: RegexFlags { case_insensitive: true, .. }, count: None }

// Line-based with explicit count
parse_type_and_operator("regex/1l")
// → TypeKind::Regex { flags: RegexFlags { line_based: true, .. }, count: Some(1) }

// Combined flags and count (interleaved order accepted)
parse_type_and_operator("regex/c256s")
// → TypeKind::Regex { flags: RegexFlags { case_insensitive: true, start_offset: true, .. }, count: Some(256) }
```

**Usage in Magic Rules:**

```rust
// Match lines starting with a digit
0 regex "^[0-9]" numeric prefix

// Case-insensitive JSON detection
0 regex/c "\\{.*\"[^\"]+\"" possible JSON

// Scan first line only for version string
>1 regex/1l "version [0-9]+" version line
```

**Regex Semantics:**

- Patterns are compiled with multi-line mode always enabled (matching libmagic's unconditional `REG_NEWLINE`), so `^` and `$` match at line boundaries and `.` does not match `\n`.
- The scan window is always capped at 8192 bytes regardless of the `count` value.
- Zero-width matches (`^`, `a*`, lookaheads) are preserved as `Value::String("")` and distinguished from genuine misses.
- Regex rules only support `Operator::Equal` and `Operator::NotEqual`; other comparison operators are rejected at evaluation time.

**Features:**

- ✅ `regex` keyword recognition with suffix parsing
- ✅ Three modifier flags (`/c`, `/s`, `/l`) with arbitrary combination order
- ✅ Optional numeric count parameter (interleaved with flags per GNU `file` semantics)
- ✅ 8192-byte scan window cap matching `FILE_REGEX_MAX`
- ✅ Bare `regex/` with no valid modifier is a parse error
- ✅ `regex/0` is rejected (zero count has no valid semantics)
- ✅ `RegexFlags` struct representation for clean flag management

### Search Type

The parser supports bounded literal byte sequence searching through the `search` keyword:

**Type Keyword:**

- `search` - Multi-byte pattern search within bounded range → `TypeKind::Search { range }`

**Mandatory Range Parameter:**

Search rules require a decimal range suffix specifying the scan window width in bytes:

- `/N` - Scan up to `N` bytes for the literal pattern, stored as `NonZeroUsize`

Per GNU `file` magic(5) specification, the range is **mandatory**. Bare `search` (no `/N` suffix) and `search/0` are both rejected at parse time.

**Parsing Examples:**

```rust
// 256-byte search window
parse_type_and_operator("search/256")
// → TypeKind::Search { range: NonZeroUsize(256) }

// Bare search is a parse error (range is mandatory)
parse_type_and_operator("search")
// → Err(...)

// Zero-range search is rejected
parse_type_and_operator("search/0")
// → Err(...)
```

**Usage in Magic Rules:**

```rust
// Scan up to 256 bytes for DOS MZ header
0 search/256 "MZ" DOS executable

// Look for ZIP signature within first 1024 bytes
0 search/1024 "PK\x03\x04" ZIP archive
```

**Search Semantics:**

- Unlike `TypeKind::String`, which only matches at the exact offset, `search` scans forward up to `range` bytes for the first occurrence of the literal pattern.
- The anchor advances to the end of the matched pattern (matching libmagic's `FILE_SEARCH` behavior in `softmagic.c::moffset()`).
- Search rules only support `Operator::Equal` and `Operator::NotEqual`; other comparison operators are rejected at evaluation time.

**Features:**

- ✅ `search` keyword recognition with mandatory `/N` suffix
- ✅ `NonZeroUsize` range representation (zero-width scan unrepresentable)
- ✅ Bare `search` and `search/0` rejected at parse time
- ✅ Binary-safe literal matching via `memchr::memmem::find`

## Parser Design Principles

### Error Handling

All parsers use nom's `IResult` type for consistent error handling:

```rust
pub fn parse_number(input: &str) -> IResult<&str, i64> {
    // Implementation with proper error propagation
}
```

**Error Categories:**

- **Syntax Errors**: Invalid characters or malformed input
- **Overflow Errors**: Numbers too large for target type
- **Format Errors**: Invalid hex digits, unterminated strings, etc.

### Memory Safety

All parsing operations are memory-safe with no unsafe code:

- **Bounds Checking**: All buffer access is bounds-checked
- **Overflow Protection**: Numeric parsing includes overflow detection
- **Resource Management**: No manual memory management required

### Performance Optimization

The parser is designed for efficiency:

- **Zero-Copy**: String slices used where possible to avoid allocations
- **Early Termination**: Parsers fail fast on invalid input
- **Minimal Backtracking**: Parser combinators designed to minimize backtracking

## Testing Strategy

Each parser component has comprehensive test coverage:

### Test Categories

1. **Basic Functionality**: Core parsing behavior
2. **Edge Cases**: Boundary values, empty input, etc.
3. **Error Conditions**: Invalid input handling
4. **Whitespace Handling**: Leading/trailing whitespace tolerance
5. **Remaining Input**: Proper handling of unconsumed input

### Example Test Structure

```rust
#[test]
fn test_parse_number_positive() {
    assert_eq!(parse_number("123"), Ok(("", 123)));
    assert_eq!(parse_number("0x1a"), Ok(("", 26)));
}

#[test]
fn test_parse_number_with_remaining_input() {
    assert_eq!(parse_number("123abc"), Ok(("abc", 123)));
    assert_eq!(parse_number("0xFF rest"), Ok((" rest", 255)));
}

#[test]
fn test_parse_number_edge_cases() {
    assert_eq!(parse_number("0"), Ok(("", 0)));
    assert_eq!(parse_number("-0"), Ok(("", 0)));
    assert!(parse_number("").is_err());
    assert!(parse_number("abc").is_err());
}
```

## Complete Magic File Parsing

The parser provides complete magic file parsing through the `parse_text_magic_file()` function:

```rust
use libmagic_rs::parser::parse_text_magic_file;

let magic_content = r#"
# ELF file format
0 string \x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
"#;

let rules = parse_text_magic_file(magic_content)?;
assert_eq!(rules.len(), 1);           // One root rule
assert_eq!(rules[0].children.len(), 2); // Two child rules
```

The parser distinguishes between signed and unsigned type variants (e.g., `byte` vs `ubyte`, `leshort` vs `uleshort`), mapping them to the `signed` field in `TypeKind::Byte { signed: bool }` and similar type variants. Unprefixed types default to signed in accordance with libmagic conventions. Float and double types do not have signed/unsigned variants; IEEE 754 handles sign internally.

### Format Detection

The parser automatically detects magic file formats:

```rust
use libmagic_rs::parser::{detect_format, MagicFileFormat};

match detect_format(path)? {
    MagicFileFormat::Text => // Parse as text magic file
    MagicFileFormat::Directory => // Load all files from Magdir
    MagicFileFormat::Binary => // Show helpful error (not yet supported)
}
```

## Current Limitations

### Not Yet Implemented

- **Indirect Offsets**: Pointer dereferencing patterns (e.g., `(0x3c.l)`)
- **Binary .mgc Format**: Compiled magic database format
- **Strength Modifiers**: `!:strength` parsing for rule priority

### Planned Enhancements

- **Better Error Messages**: More descriptive error reporting with source locations
- **Performance Optimization**: Specialized parsers for common patterns
- **Streaming Support**: Incremental parsing for large magic files

## Integration Points

The parser provides a complete pipeline from text to AST:

```rust
use libmagic_rs::parser::{parse_text_magic_file, detect_format, MagicFileFormat};

// Detect format and parse accordingly
let rules = match detect_format(path)? {
    MagicFileFormat::Text => {
        let content = std::fs::read_to_string(path)?;
        parse_text_magic_file(&content)?
    }
    MagicFileFormat::Directory => {
        // Load and merge all files in directory
        load_magic_directory(path)?
    }
    MagicFileFormat::Binary => {
        return Err(ParseError::UnsupportedFormat { ... });
    }
};
```

The hierarchical structure is automatically built from indentation levels (`>` prefixes), enabling parent-child rule relationships for detailed file type identification.
