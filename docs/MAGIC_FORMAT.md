# Magic File Format Guide

A comprehensive guide to the magic file format used by libmagic-rs.

## Table of Contents

- [Overview](#overview)
- [Basic Syntax](#basic-syntax)
- [Offset Specifications](#offset-specifications)
- [Type Specifications](#type-specifications)
- [Operators](#operators)
- [Values](#values)
- [Nested Rules](#nested-rules)
- [Examples](#examples)
- [Best Practices](#best-practices)

---

## Overview

Magic files contain rules that describe file formats by specifying byte patterns at specific offsets. Each rule consists of:

1. **Offset** - Where to look in the file
2. **Type** - How to interpret the bytes
3. **Value** - What to match against
4. **Message** - Description to display on match

### Basic Format

```
offset  type  value  message
```

Example:

```
0       string  PK    ZIP archive data
```

This rule matches files starting with "PK" and labels them as "ZIP archive data".

---

## Basic Syntax

### Rule Structure

```
[level>]offset    type    [operator]value    message
```

| Component  | Required | Description                        |
| ---------- | -------- | ---------------------------------- |
| `level>`   | No       | Indentation level for nested rules |
| `offset`   | Yes      | Where to read data                 |
| `type`     | Yes      | Data type to read                  |
| `operator` | No       | Comparison operator (default: `=`) |
| `value`    | Yes      | Expected value                     |
| `message`  | Yes      | Description text                   |

### Comments

Lines starting with `#` are comments:

```
# This is a comment
0  string  PK  ZIP archive
```

### Whitespace

- Fields are separated by whitespace (spaces or tabs)
- Leading whitespace indicates rule nesting level
- Trailing whitespace is ignored

---

## Offset Specifications

### Absolute Offset

Direct byte position from file start:

```
0       string  \x7fELF   ELF executable
16      short   2         (shared object)
```

### Hexadecimal Offset

Use `0x` prefix for hex offsets:

```
0x0     string  MZ        DOS executable
0x3c    long    >0        (PE offset present)
```

### Negative Offset (From End)

Read from end of file:

```
-4      string  .ZIP      ZIP file (end marker)
```

### Indirect Offset

Read pointer value and use as offset:

```
# Read 4-byte pointer at offset 60, then check that location
(0x3c.l)   string  PE\0\0  PE executable
```

Indirect offset syntax:

- `(base.type)` - Read pointer at base, interpret as type
- `(base.type+adj)` - Add adjustment to pointer value

Types for indirect offsets:

- `.b` - byte (1 byte)
- `.s` - short (2 bytes)
- `.l` - long (4 bytes)
- `.q` - quad (8 bytes)

### Relative Offset

Offset relative to previous match:

```
0       string  PK\x03\x04   ZIP archive
&2      short   >0           (with data)
```

The `&` prefix indicates relative offset.

---

## Type Specifications

### Integer Types

| Type      | Size    | Endianness    |
| --------- | ------- | ------------- |
| `byte`    | 1 byte  | N/A           |
| `short`   | 2 bytes | native        |
| `leshort` | 2 bytes | little-endian |
| `beshort` | 2 bytes | big-endian    |
| `long`    | 4 bytes | native        |
| `lelong`  | 4 bytes | little-endian |
| `belong`  | 4 bytes | big-endian    |
| `quad`    | 8 bytes | native        |
| `lequad`  | 8 bytes | little-endian |
| `bequad`  | 8 bytes | big-endian    |

All integer types have unsigned variants prefixed with `u`:

- `ubyte`, `ushort`, `uleshort`, `ubeshort`
- `ulong`, `ulelong`, `ubelong`
- `uquad`, `ulequad`, `ubequad`

Examples:

```
0       byte      0x7f      (byte match)
0       leshort   0x5a4d    DOS MZ signature
0       belong    0xcafebabe Java class file
0       lequad    0x1234567890abcdef  (64-bit little-endian)
8       uquad     >0x8000000000000000 (unsigned 64-bit check)
```

### String Type

Match literal string data:

```
0       string    %PDF      PDF document
0       string    GIF89a    GIF image data
```

String escape sequences:

- `\x00` - hex byte
- `\n` - newline
- `\t` - tab
- `\\` - backslash

### String Flags

| Flag | Description            |
| ---- | ---------------------- |
| `/c` | Case-insensitive match |
| `/w` | Whitespace-insensitive |
| `/b` | Match at word boundary |

Example:

```
0       string/c  <!doctype  HTML document
```

---

## Operators

### Comparison Operators

| Operator | Description           | Example              |
| -------- | --------------------- | -------------------- |
| `=`      | Equal (default)       | `0 long =0xcafebabe` |
| `!`      | Not equal             | `4 byte !0`          |
| `<`      | Less than             | `8 long <100`        |
| `>`      | Greater than          | `8 long >1000`       |
| `<=`     | Less than or equal    | `8 long <=100`       |
| `>=`     | Greater than or equal | `8 long >=1000`      |
| `&`      | Bitwise AND           | `4 byte &0x80`       |
| `^`      | Bitwise XOR           | `4 byte ^0xff`       |
| `~`      | Bitwise NOT           | `4 byte ~0xff`       |

### Bitwise AND with Mask

Test specific bits:

```
# Check if bit 7 is set
4       byte    &0x80     (compressed)

# Check if lower nibble is 0x0f
4       byte    &0x0f=0x0f (all bits set)
```

### Negation

Prefix operator with `!` for negation:

```
# Match if NOT equal to zero
4       long    !0        (non-zero)
```

---

## Values

### Numeric Values

```
# Decimal
0       long    1234

# Hexadecimal
0       long    0x4d5a

# Octal
0       byte    0177
```

### String Values

```
# Plain string
0       string  RIFF

# With escape sequences
0       string  PK\x03\x04

# Unicode (as bytes)
0       string  \xff\xfe
```

### Special Values

| Value | Description                   |
| ----- | ----------------------------- |
| `x`   | Match any value (always true) |

Example:

```
0       string  PK        ZIP archive
>4      short   x         version %d
```

The `x` value matches anything and `%d` formats the matched value.

---

## Nested Rules

Rules can be nested to create hierarchical matches. Deeper matches indicate more specific identification.

### Indentation Levels

Use `>` prefix for nested rules:

```
0       string  \x7fELF   ELF
>4      byte    1         32-bit
>4      byte    2         64-bit
>5      byte    1         LSB
>5      byte    2         MSB
```

Evaluation:

1. Check offset 0 for ELF magic
2. If matched, check offset 4 for bit size
3. If matched, check offset 5 for endianness

### Multiple Nesting Levels

```
0       string  \x7fELF       ELF
>4      byte    2             64-bit
>>5     byte    1             LSB
>>>16   short   2             (shared object)
>>>16   short   3             (executable)
```

### Continuation Messages

Use `\b` (backspace) to suppress space before message:

```
0       string  GIF8      GIF image data
>4      byte    7a        \b, version 87a
>4      byte    9a        \b, version 89a
```

Output: `GIF image data, version 89a`

---

## Examples

### ELF Executable

```
# ELF (Executable and Linkable Format)
0       string  \x7fELF       ELF
>4      byte    1             32-bit
>4      byte    2             64-bit
>5      byte    1             LSB
>5      byte    2             MSB
>16     leshort 2             (executable)
>16     leshort 3             (shared object)
```

### ZIP Archive

```
# ZIP archive
0       string  PK\x03\x04    ZIP archive data
>4      leshort x             \b, version %d.%d to extract
>6      leshort &0x0001       \b, encrypted
>6      leshort &0x0008       \b, with data descriptor
```

### JPEG Image

```
# JPEG
0       string  \xff\xd8\xff  JPEG image data
>3      byte    0xe0          \b, JFIF standard
>3      byte    0xe1          \b, Exif format
```

### PDF Document

```
# PDF
0       string  %PDF-         PDF document
>5      string  1.            \b, version 1.x
>5      string  2.            \b, version 2.x
```

### PE Executable

```
# DOS MZ executable with PE header
0       string  MZ            DOS executable
>0x3c   lelong  >0            (PE offset)
>(0x3c.l) string PE\0\0       PE executable
```

### GZIP Compressed

```
# GZIP
0       string  \x1f\x8b      gzip compressed data
>2      byte    8             \b, deflated
>3      byte    &0x01         \b, ASCII text
>3      byte    &0x02         \b, with header CRC
>3      byte    &0x04         \b, with extra field
>3      byte    &0x08         \b, with original name
>3      byte    &0x10         \b, with comment
```

### PNG Image

```
# PNG
0       string  \x89PNG\r\n\x1a\n   PNG image data
>16     belong  x                   \b, %d x
>20     belong  x                   %d
>24     byte    0                   \b, grayscale
>24     byte    2                   \b, RGB
>24     byte    3                   \b, palette
>24     byte    4                   \b, grayscale+alpha
>24     byte    6                   \b, RGBA
```

---

## Best Practices

### 1. Order Rules by Specificity

Put more specific rules first:

```
# Good: Specific before general
0       string  PK\x03\x04   ZIP archive
0       string  PK           (generic PK signature)

# Bad: General catches all
0       string  PK           (generic PK signature)
0       string  PK\x03\x04   ZIP archive  # Never reached
```

### 2. Use Nested Rules for Details

```
# Good: Hierarchical structure
0       string  \x7fELF   ELF
>4      byte    2         64-bit
>>5     byte    1         LSB

# Bad: Flat rules
0       string  \x7fELF           ELF
4       byte    2                 64-bit
5       byte    1                 LSB
```

### 3. Document Complex Rules

```
# JPEG with Exif metadata
# The Exif APP1 marker (0xFFE1) contains camera metadata
0       string  \xff\xd8\xff    JPEG image data
>3      byte    0xe1            \b, Exif format
```

### 4. Test Edge Cases

Consider:

- Empty files
- Truncated files
- Minimum valid file size
- Maximum offset values

### 5. Use Appropriate Types

```
# Good: Match exact size needed
0       leshort 0x5a4d   DOS executable

# Bad: Over-reading
0       lelong  x        (reads 4 bytes when 2 needed)
```

### 6. Handle Endianness Explicitly

```
# Good: Explicit endianness
0       lelong  0xcafebabe   (little-endian)
0       belong  0xcafebabe   (big-endian)

# Risky: Native endianness
0       long    0xcafebabe   (platform-dependent)
```

---

## Supported Features

### Currently Supported

- Absolute offsets
- Relative offsets
- Indirect offsets (basic)
- Byte, short, long, quad types (8-bit, 16-bit, 32-bit, 64-bit integers)
- String type
- Comparison operators (`=`, `!`, `<`, `>`, `<=`, `>=`)
- Bitwise AND operator
- Nested rules
- Comments

### Not Yet Supported

- Regex patterns
- Date/time types
- Float types
- 128-bit integer types
- Use/name directives
- Default rules

### Recently Added

- **Comparison operators**: Full support for `<`, `>`, `<=`, `>=` operators
- **Strength modifiers**: The `!:strength` directive for adjusting rule priority
- **64-bit integers**: `quad` type family (`quad`, `uquad`, `lequad`, `ulequad`, `bequad`, `ubequad`)

---

## Troubleshooting

### Rule Not Matching

1. Check offset is correct (0-indexed)
2. Verify endianness matches file format
3. Test with `hexdump -C file | head`
4. Ensure no conflicting rules

### Unexpected Results

1. Check rule order (first match wins)
2. Verify nested rule levels
3. Test with simpler rules first

### Performance Issues

1. Avoid unnecessary string searches
2. Use specific offsets over searches
3. Order rules by likelihood of match

---

## See Also

- [magic(5)](https://man7.org/linux/man-pages/man5/magic.5.html) - Original magic format
- [file(1)](https://man7.org/linux/man-pages/man1/file.1.html) - GNU file command
- [API Reference](API_REFERENCE.md) - libmagic-rs API documentation
