# Magic File Format

Magic files define rules for identifying file types through byte-level patterns. This chapter documents the magic file format supported by libmagic-rs.

## Basic Syntax

Magic files consist of rules with the following format:

```text
offset  type  operator  value  message
```

### Example Rules

```text
# ELF files
0       string    \x7fELF         ELF
>4      byte      1               32-bit
>4      byte      2               64-bit

# ZIP archives
0       string    PK\003\004     ZIP archive

# JPEG images
0       string    \xff\xd8\xff   JPEG image
```

## Offset Specifications

### Absolute Offsets

```text
0       # Start of file
16      # Byte 16
0x10    # Hexadecimal offset
```

### Relative Offsets (Hierarchical)

```text
0       string    \x7fELF    ELF
>4      byte      1          32-bit    # 4 bytes after ELF magic
>5      byte      1          LSB       # 5 bytes after ELF magic
```

### Indirect Offsets

```text
(0x20.l)     # Read 32-bit value at 0x20, use as offset
(0x20.l+4)   # Same, but add 4 to the result
```

## Data Types

### Numeric Types

- `byte` - 8-bit value
- `short` - 16-bit value
- `long` - 32-bit value
- `leshort` - Little-endian 16-bit
- `beshort` - Big-endian 16-bit
- `lelong` - Little-endian 32-bit
- `belong` - Big-endian 32-bit

### String Types

- `string` - Null-terminated string
- `pstring` - Pascal string (length-prefixed)

## Operators

- `=` or no operator - Equality (default)
- `!=` - Inequality
- `&` - Bitwise AND
- `>` - Greater than
- `<` - Less than

## Value Formats

### Numeric Values

```text
42          # Decimal
0x2a        # Hexadecimal
0377        # Octal
```

### String Values

```text
hello                    # Plain string
"hello world"           # Quoted string
\x7fELF                 # Escape sequences
PK\003\004              # Mixed format
```

### Byte Sequences

```text
\x7f\x45\x4c\x46       # Hex bytes
\177ELF                 # Mixed octal/ASCII
```

## Comments and Organization

```text
# This is a comment
# Comments can appear anywhere

# Group related rules
# ELF files
0    string    \x7fELF    ELF
>4   byte      1          32-bit

# ZIP files
0    string    PK         ZIP-based format
```

## Advanced Features (Planned)

### Regular Expressions

```text
0    regex    ^#!/bin/.*sh    Shell script
```

### Conditional Logic

```text
0    string    \x7fELF         ELF
>4   byte      1               32-bit
>>16 leshort   >0              executable
```

### MIME Type Mapping

```text
0    string    \x7fELF    ELF    application/x-executable
```

This format provides a flexible, human-readable way to define file type detection rules while maintaining compatibility with existing magic file databases.
