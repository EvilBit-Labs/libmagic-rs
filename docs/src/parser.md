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
- ✅ Numeric literal parsing (decimal and hexadecimal)
- ✅ Hex byte sequence parsing (with and without `\x` prefix)
- ✅ Intelligent type precedence to avoid parsing conflicts
- ✅ Comprehensive escape sequence handling (`\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`)

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

The parser distinguishes between signed and unsigned type variants (e.g., `byte` vs `ubyte`, `leshort` vs `uleshort`), mapping them to the `signed` field in `TypeKind::Byte { signed: bool }` and similar type variants. Unprefixed types default to signed in accordance with libmagic conventions.

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
- **Regex Support**: Regular expression matching in rules
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
