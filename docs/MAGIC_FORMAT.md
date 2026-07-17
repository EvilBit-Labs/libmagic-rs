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

```text
offset  type  value  message
```

Example:

```text
0       string  PK    ZIP archive data
```

This rule matches files starting with "PK" and labels them as "ZIP archive data".

---

## Basic Syntax

### Rule Structure

```text
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

```text
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

```text
0       string  \x7fELF   ELF executable
16      short   2         (shared object)
```

### Hexadecimal Offset

Use `0x` prefix for hex offsets:

```text
0x0     string  MZ        DOS executable
0x3c    long    >0        (PE offset present)
```

### Negative Offset (From End)

Read from end of file:

```text
-4      string  .ZIP      ZIP file (end marker)
```

### Indirect Offset

Read pointer value and use as offset:

```text
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

```text
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

```text
0       byte      0x7f      (byte match)
0       leshort   0x5a4d    DOS MZ signature
0       belong    0xcafebabe Java class file
0       lequad    0x1234567890abcdef  (64-bit little-endian)
8       uquad     >0x8000000000000000 (unsigned 64-bit check)
```

### String Types

Match literal string data:

```text
0       string    %PDF      PDF document
0       string    GIF89a    GIF image data
```

String escape sequences:

- `\x00` - hex byte
- `\n` - newline
- `\t` - tab
- `\\` - backslash

**Pascal String (pstring)**

Length-prefixed string type where a length prefix (1, 2, or 4 bytes) specifies the number of bytes of string data that follow. Unlike C strings, Pascal strings are not null-terminated.

The length prefix width is controlled by suffix flags:

| Suffix | Length Prefix Width | Byte Order    |
| ------ | ------------------- | ------------- |
| `/B`   | 1 byte (default)    | N/A           |
| `/H`   | 2 bytes             | big-endian    |
| `/h`   | 2 bytes             | little-endian |
| `/L`   | 4 bytes             | big-endian    |
| `/l`   | 4 bytes             | little-endian |

The `/J` flag indicates JPEG-style self-inclusive length where the stored length value includes the size of the length prefix itself. This flag can be combined with any width suffix (`/HJ`, `/lJ`, etc.) or used alone (`/J` defaults to 1-byte width).

Examples:

```text
0       pstring   =JPEG         JPEG image (1-byte prefix, default)
0       pstring/B =JPEG         JPEG image (1-byte prefix, explicit)
0       pstring/H =JPEG         JPEG image (2-byte big-endian prefix)
0       pstring/h =JPEG         JPEG image (2-byte little-endian prefix)
0       pstring/L =JPEG         JPEG image (4-byte big-endian prefix)
0       pstring/l =JPEG         JPEG image (4-byte little-endian prefix)
0       pstring/HJ =JPEG        JPEG image (2-byte BE, self-inclusive length)
```

The `pstring` magic-file surface syntax does not accept a `max_length` value -- only the `/B`, `/H`, `/h`, `/L`, `/l`, and `/J` width/flag suffixes. The AST field `max_length: Option<usize>` is reserved for programmatic rule construction (e.g., callers building `TypeKind::PString { max_length: Some(N), ... }` directly) and for future grammar extensions. When set, it caps the length value to guard against attacker-controlled length-prefix saturation attacks where malicious files specify extreme length values; rules loaded from `.magic` text always have `max_length: None`.

**UCS-2 Strings (lestring16 / bestring16)**

Wide-character strings encoded as 2 bytes per character with little-endian (`lestring16`) or big-endian (`bestring16`) byte order. Each string is null-terminated (U+0000) and capped at 8192 characters. Invalid surrogate halves are replaced with U+FFFD.

Examples:

```text
0       lestring16  =WORD      Word document (UTF-16LE)
0       bestring16  =WORD      Word document (UTF-16BE)
```

### String Flags

Flags for `string` type modify comparison behavior per libmagic `src/softmagic.c`:

| Flag | Description                                                                                                            |
| ---- | ---------------------------------------------------------------------------------------------------------------------- |
| `/c` | Case-insensitive match (lowercase pattern chars fold file bytes to lower; uppercase pattern chars are literal)         |
| `/C` | Case-insensitive match (uppercase pattern chars fold file bytes to upper; lowercase pattern chars are literal)         |
| `/w` | Whitespace-optional (pattern whitespace matches zero or more file whitespace)                                          |
| `/W` | Whitespace-required-compact (pattern whitespace requires at least one file whitespace; additional whitespace consumed) |
| `/T` | Trim leading/trailing ASCII whitespace from pattern before comparison                                                  |
| `/f` | Full-word match (post-match word-boundary check; next byte must be EOF or non-word char)                               |
| `/b` | Force binary test (MIME-output hint; no effect on comparison)                                                          |
| `/t` | Force text test (MIME-output hint; no effect on comparison)                                                            |

**`/c` vs `/C` asymmetry:** The pattern character controls fold direction. `/c` with lowercase pattern chars folds the file byte to lowercase; uppercase pattern chars in the same pattern are compared literally. Mixed-case patterns work intuitively: `/c FoO` matches `FoO`, `Foo`, `FOO` but not `fOO` (the uppercase `F` is literal). See GOTCHAS S6.5 for details.

**`/B` is not a string flag** — it is the `pstring` 1-byte length-width letter. `string/B` is rejected at parse time. See GOTCHAS S6.6.

Examples:

```text
0       string/c  <!doctype  HTML document
0       string/w  foo bar    whitespace-flexible match
0       string/T  \tdata     leading/trailing whitespace trimmed
0       string/f  int        full-word boundary check
0       string/b  FTCOMP     binary-file hint
```

Flags for `pstring` type are documented in the Pascal String section above.

### Floating-Point Types

Match 32-bit (float) or 64-bit (double) IEEE 754 floating-point values.

| Type       | Size    | Endianness    |
| ---------- | ------- | ------------- |
| `float`    | 4 bytes | native        |
| `befloat`  | 4 bytes | big-endian    |
| `lefloat`  | 4 bytes | little-endian |
| `double`   | 8 bytes | native        |
| `bedouble` | 8 bytes | big-endian    |
| `ledouble` | 8 bytes | little-endian |

Examples:

```text
0       lefloat   3.14159    (32-bit little-endian float)
0       bedouble  >1.0       (64-bit big-endian double)
```

### Date/Timestamp Types

Date and timestamp types read Unix timestamps (signed seconds since epoch) and format them as human-readable strings.

**32-bit timestamps (4 bytes):**

| Type      | Size    | Endianness    | Timezone   |
| --------- | ------- | ------------- | ---------- |
| `date`    | 4 bytes | native        | UTC        |
| `ldate`   | 4 bytes | native        | local time |
| `bedate`  | 4 bytes | big-endian    | UTC        |
| `beldate` | 4 bytes | big-endian    | local time |
| `ledate`  | 4 bytes | little-endian | UTC        |
| `leldate` | 4 bytes | little-endian | local time |

**64-bit timestamps (8 bytes):**

| Type       | Size    | Endianness    | Timezone   |
| ---------- | ------- | ------------- | ---------- |
| `qdate`    | 8 bytes | native        | UTC        |
| `qldate`   | 8 bytes | native        | local time |
| `beqdate`  | 8 bytes | big-endian    | UTC        |
| `beqldate` | 8 bytes | big-endian    | local time |
| `leqdate`  | 8 bytes | little-endian | UTC        |
| `leqldate` | 8 bytes | little-endian | local time |

All timestamp values are formatted as strings in the format `"Www Mmm DD HH:MM:SS YYYY"` to match GNU file output.

Example:

```text
0       ldate   x   Unix timestamp: %s
```

### Regex Pattern Type

Match byte patterns using regular expressions. The `regex` type uses `regex::bytes::Regex` for pattern matching.

**Syntax:**

```text
offset  regex[/count[unit]][flags]  pattern  message
```

| Component | Required | Description                                        |
| --------- | -------- | -------------------------------------------------- |
| `/count`  | No       | Numeric cap: bytes scanned (default)               |
| `unit`    | No       | `l` suffix = line count cap instead of byte count  |
| `/c`      | No       | Case-insensitive matching                          |
| `/s`      | No       | Anchor advance to match-start (default: match-end) |
| `/l`      | No       | Line-bounded scan window (stops at newline)        |

**Flags:**

- `/c` - case-insensitive matching
- `/s` - anchor advance to match-start (not match-end)
- `/l` - line-bounded scan window (stops at first newline)

**Count semantics:**

- `regex/100` - scan up to 100 bytes
- `regex/10l` - scan up to 10 lines
- Bare `regex` or `regex/0` are parse errors (range is mandatory per GNU `file` magic(5))

Every scan window is capped at 8192 bytes (`FILE_REGEX_MAX`). Multi-line matching is always enabled (matching libmagic's unconditional `REG_NEWLINE`). Anchor advance follows GNU `file` semantics (match-end, not window-end).

**Bareword (unquoted) pattern escape handling:**

Bareword regex patterns undergo escape resolution using GNU `file`'s getstr escape table before the pattern is compiled. Quoted patterns (`"..."`) preserve escapes verbatim without getstr resolution.

Supported bareword escape sequences:

- `\t`, `\n`, `\r`, `\b`, `\f`, `\v` - standard C control sequences (tab, newline, carriage return, backspace, form feed, vertical tab)
- `\NNN` - octal escape sequences (three digits; e.g. `\040` resolves to space)
- `\xNN` - hexadecimal escape sequences (two hex digits; e.g. `\x20` resolves to space)
- `\\` - escaped backslash (resolves to a single backslash)
- Unrecognized escape sequences drop the backslash (e.g. `\^` becomes `^`)

Bytes >= 0x80 produced by escape resolution are re-encoded as `\xHH` format for the regex engine. Regex shorthand sequences like `\d`, `\s`, `\w` that are not recognized by getstr are demoted to literals (e.g. `\d` becomes `d`).

This behavior matches GNU `file`'s handling of bareword regex patterns and ensures compatibility with system magic databases like `/usr/share/file/magic/`.

Examples:

```text
0       regex/100      [A-Z]+            Found uppercase letters
0       regex/10l/c    error             Found "error" (case-insensitive, 10-line cap)
0       regex/500/s    ^BEGIN            Found BEGIN at start (anchor advances to match-start)
0       regex/50       \^[\040\t]{0,50}\\.asciiz    assembler source text (getstr-resolved pattern)
```

### Search Type

Bounded literal pattern scan. Searches for a literal byte pattern within a specified range using `memchr::memmem::find`.

**Syntax:**

```text
offset  search/range[/flags]  pattern  message
```

The range is MANDATORY (`NonZeroUsize`). Bare `search` and `search/0` are parse errors per GNU `file` magic(5). Anchor advance follows GNU `file` semantics (match-end, not window-end) unless `/s` is set.

**Search Flags:**

Flags for `search` type modify comparison and anchor behavior. Most flags share semantics with `string` type flags; `/s` is search-specific.

| Flag | Description                                                                                                            |
| ---- | ---------------------------------------------------------------------------------------------------------------------- |
| `/s` | Anchor advance lands at match-START instead of match-END (required for TGA footer, sfnt name table)                    |
| `/c` | Case-insensitive match (lowercase pattern chars fold file bytes to lower; uppercase pattern chars are literal)         |
| `/C` | Case-insensitive match (uppercase pattern chars fold file bytes to upper; lowercase pattern chars are literal)         |
| `/w` | Whitespace-optional (pattern whitespace matches zero or more file whitespace)                                          |
| `/W` | Whitespace-required-compact (pattern whitespace requires at least one file whitespace; additional whitespace consumed) |
| `/T` | Trim leading/trailing ASCII whitespace from pattern before comparison                                                  |
| `/f` | Full-word match (post-match word-boundary check; next byte must be EOF or non-word char)                               |
| `/t` | Force text test (MIME-output hint; no effect on comparison)                                                            |
| `/b` | Force binary test (MIME-output hint; no effect on comparison)                                                          |

**`/c` vs `/C` asymmetry:** The pattern character controls fold direction. `/c` with lowercase pattern chars folds the file byte to lowercase; uppercase pattern chars in the same pattern are compared literally. See String Flags section above for details.

**`/s` — Start anchor:** When set, the anchor for relative-offset child rules lands at the match-START position rather than match-END. This is required for file formats that place magic strings in footers or trailers (TGA, sfnt name table).

**MIME hints (`/t`, `/b`):** These flags are captured but do not currently alter match decisions. They are deferred to MIME-output integration (issue #51).

Examples:

```text
0       search/1024         MARKER                      Found marker within 1024 bytes
0       search/4096         \x00\x00                    Found null bytes
0       search/4261301/s    TRUEVISION-XFILE.\0         TGA footer (anchor at match-start)
0       search/1/w          #!\040/usr/bin/python       Python shebang (flexible whitespace)
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
| `x`      | Match any value       | `4 byte x`           |

### Bitwise AND with Mask

Test specific bits:

```text
# Check if bit 7 is set
4       byte    &0x80     (compressed)

# Check if lower nibble is 0x0f
4       byte    &0x0f=0x0f (all bits set)
```

### Negation

Prefix operator with `!` for negation:

```text
# Match if NOT equal to zero
4       long    !0        (non-zero)
```

---

## Values

### Numeric Values

```text
# Decimal
0       long    1234

# Hexadecimal
0       long    0x4d5a

# Octal
0       byte    0177
```

### String Values

```text
# Plain string
0       string  RIFF

# With escape sequences
0       string  PK\x03\x04

# Unicode (as bytes)
0       string  \xff\xfe
```

### Any-Value Operator

The `x` operator matches unconditionally at the given offset. It is typically used in child rules to extract and format a value without testing it:

Example:

```text
0       string  PK        ZIP archive
>4      short   x         version %d
```

The `x` value matches anything and `%d` formats the matched value.

---

## Nested Rules

Rules can be nested to create hierarchical matches. Deeper matches indicate more specific identification.

### Indentation Levels

Use `>` prefix for nested rules:

```text
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

```text
0       string  \x7fELF       ELF
>4      byte    2             64-bit
>>5     byte    1             LSB
>>>16   short   2             (shared object)
>>>16   short   3             (executable)
```

### Continuation Messages

Use `\b` (backspace) to suppress space before message:

```text
0       string  GIF8      GIF image data
>4      byte    7a        \b, version 87a
>4      byte    9a        \b, version 89a
```

Output: `GIF image data, version 89a`

---

## Examples

### ELF Executable

```text
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

```text
# ZIP archive
0       string  PK\x03\x04    ZIP archive data
>4      leshort x             \b, version %d.%d to extract
>6      leshort &0x0001       \b, encrypted
>6      leshort &0x0008       \b, with data descriptor
```

### JPEG Image

```text
# JPEG
0       string  \xff\xd8\xff  JPEG image data
>3      byte    0xe0          \b, JFIF standard
>3      byte    0xe1          \b, Exif format
```

### PDF Document

```text
# PDF
0       string  %PDF-         PDF document
>5      string  1.            \b, version 1.x
>5      string  2.            \b, version 2.x
```

### PE Executable

```text
# DOS MZ executable with PE header
0       string  MZ            DOS executable
>0x3c   lelong  >0            (PE offset)
>(0x3c.l) string PE\0\0       PE executable
```

### GZIP Compressed

```text
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

```text
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

## Meta-types / Control Directives

Meta-types are pseudo-types that do not read bytes from the buffer. Instead, they control the evaluation flow: defining named subroutines, invoking them, providing fallbacks when no sibling matched, resetting per-level match state, or re-applying the entire rule database at a resolved offset.

| Keyword     | Syntax                      | Description                                                                             |
| ----------- | --------------------------- | --------------------------------------------------------------------------------------- |
| `name <id>` | `0 name part2`              | Defines a named subroutine block; children are the subroutine body                      |
| `use <id>`  | `>0 use part2`              | Invokes a named subroutine at the resolved offset                                       |
| `default`   | `0 default x Fallback`      | Fires only when no sibling at the same level has matched                                |
| `clear`     | `0 clear`                   | Resets the per-level sibling-matched flag                                               |
| `indirect`  | `8 indirect x`              | Re-applies the full rule database at the resolved offset                                |
| `offset`    | `>>&0 offset x at_off %lld` | Emits the resolved file position as `Value::Uint` for printf-style message substitution |

### `name` and `use` — Named Subroutines

`name <id>` defines a named subroutine block at the top level; its children are the subroutine body. `use <id>` invokes that subroutine at a given offset.

```text
# Define a reusable subroutine
0       name    part2
>0      search/64    ABC       found_ABC
>>&0    byte    x            followed_by 0x%x

# Top-level rule that invokes the subroutine
0       string  TEST          Testfmt
>0      use     part2
>64     use     part2
```

Top-level `name` blocks are hoisted out of the flat rule list at parse time into a `NameTable` keyed by identifier. Duplicate names retain the first definition and emit a warning. `name` rules nested inside another rule's children are not well-defined in magic(5) and are scrubbed at load time.

### `default` — Fallback Rule

A `default` rule at a given level fires only when none of its siblings at the same level have matched. The operator is conventionally `x` (any-value), and the value column is ignored.

```text
0       byte    0xAA    Real-Match
0       default x       DEFAULT-FALLBACK
```

Against a buffer starting with `0xAA`, only `Real-Match` fires. Against a buffer starting with any other byte, `DEFAULT-FALLBACK` fires.

### `clear` — Reset Sibling-Matched Flag

A `clear` directive resets the per-level "sibling matched" flag, so a subsequent `default` at the same level can fire again even after an earlier sibling matched. Pair with `EvaluationConfig::with_stop_at_first_match(false)` to walk all top-level siblings.

```text
0       byte    0xAA    Match-A
0       default x       DEFAULT-SKIPPED
0       clear
0       default x       DEFAULT-FIRES
```

Against a buffer starting with `0xAA`: `Match-A` fires, `DEFAULT-SKIPPED` is suppressed (a sibling matched), `clear` resets the flag, and `DEFAULT-FIRES` fires.

### `indirect` — Re-apply Root Rules at a Resolved Offset

An `indirect` rule resolves its offset, slices the buffer at that point, and re-applies the full rule database against the sub-buffer. Recursion is bounded by `EvaluationConfig::max_recursion_depth`.

```text
0       byte    0x42    Inner-Match
8       indirect x
```

Against a 16-byte buffer with `buf[8] = 0x42`: the top-level `byte` rule at offset 0 does not match, and the `indirect` rule re-applies the root rules at offset 8 — where `buf[8] = 0x42` matches the inner `byte` rule, producing `Inner-Match`.

---

## Best Practices

### 1. Order Rules by Specificity

Put more specific rules first:

```text
# Good: Specific before general
0       string  PK\x03\x04   ZIP archive
0       string  PK           (generic PK signature)

# Bad: General catches all
0       string  PK           (generic PK signature)
0       string  PK\x03\x04   ZIP archive  # Never reached
```

### 2. Use Nested Rules for Details

```text
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

```text
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

```text
# Good: Match exact size needed
0       leshort 0x5a4d   DOS executable

# Bad: Over-reading
0       lelong  x        (reads 4 bytes when 2 needed)
```

### 6. Handle Endianness Explicitly

```text
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
- Floating-point types (`float`, `befloat`, `lefloat`, `double`, `bedouble`, `ledouble`)
- String types (`string`, `pstring`, `lestring16`, `bestring16`)
- Regex patterns (`regex` type with `/c`, `/s`, `/l` flags and byte/line count caps)
- Search type (`search` bounded literal pattern scan)
- Date and timestamp types (32-bit and 64-bit Unix timestamps)
- Comparison operators (`=`, `!`, `<`, `>`, `<=`, `>=`)
- Bitwise AND operator
- Nested rules
- Comments

### Not Yet Supported

- 128-bit integer types

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
