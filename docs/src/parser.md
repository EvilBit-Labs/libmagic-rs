# Parser Implementation

> [!NOTE]
> The parser is currently in development. This documentation describes the planned implementation.

The parser module is responsible for converting magic files (text-based DSL) into the AST representation used by the evaluator.

## Overview

The parser uses the `nom` crate for parsing combinators, providing:

- **Robust error handling** with detailed error messages
- **Incremental parsing** for large magic databases
- **Memory efficiency** through zero-copy parsing where possible
- **Extensible grammar** for future magic file features

## Magic File Format

Magic files use a simple DSL to describe file type detection rules:

```text
# Comments start with #
0    string    \x7fELF         ELF
>4   byte      1               32-bit
>4   byte      2               64-bit
>5   byte      1               LSB
>5   byte      2               MSB

# ZIP files
0    string    PK\003\004     ZIP archive
```

## Parser Architecture

```text
Magic File Text → Lexer → Tokens → Parser → AST
```

### Lexer (Planned)

Converts text into tokens:

- **Offsets**: `0`, `>4`, `(0x20.l+4)`
- **Types**: `byte`, `short`, `long`, `string`
- **Operators**: `=`, `!=`, `&`
- **Values**: Numbers, strings, byte sequences

### Parser (Planned)

Combines tokens into AST nodes:

- **Rule parsing**: Complete magic rule structures
- **Hierarchy handling**: Indentation-based nesting
- **Error recovery**: Continue parsing after errors
- **Validation**: Check rule consistency

## Implementation Status

- [ ] Basic nom parser setup
- [ ] Offset parsing (absolute, indirect, relative)
- [ ] Type parsing with endianness
- [ ] Operator parsing
- [ ] Value parsing (strings, numbers, bytes)
- [ ] Rule hierarchy parsing
- [ ] Error handling and reporting

## Planned API

```rust
pub fn parse_magic_file<P: AsRef<Path>>(path: P) -> Result<Vec<MagicRule>>;
pub fn parse_magic_string(input: &str) -> Result<Vec<MagicRule>>;
```
