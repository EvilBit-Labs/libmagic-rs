# Appendix C: Magic File Examples

This appendix provides comprehensive examples of magic file syntax and patterns, demonstrating how to create effective file type detection rules.

## Basic Magic File Syntax

### Simple Pattern Matching

```magic
# ELF executable files
0    string    \x7fELF    ELF

# PDF documents
0    string    %PDF-      PDF document

# PNG images
0    string    \x89PNG    PNG image data

# ZIP archives
0    string    PK\x03\x04    ZIP archive data
```

### Numeric Value Matching

```magic
# JPEG images (using hex values)
0    beshort    0xffd8    JPEG image data

# Windows PE executables
0    string    MZ        MS-DOS executable
>60  lelong    >0
>>60 string    PE\0\0    PE32 executable

# ELF with specific architecture
0    string    \x7fELF    ELF
>16  leshort   2         executable
>18  leshort   62        x86-64
```

## Hierarchical Rules

### Parent-Child Relationships

```magic
# ELF files with detailed classification
0    string    \x7fELF    ELF
>4   byte      1         32-bit
>>16 leshort   2         executable
>>16 leshort   3         shared object
>>16 leshort   1         relocatable
>4   byte      2         64-bit
>>16 leshort   2         executable
>>16 leshort   3         shared object
>>16 leshort   1         relocatable
```

### Multiple Levels of Nesting

```magic
# Detailed PE analysis
0    string    MZ        MS-DOS executable
>60  lelong    >0
>>60 string    PE\0\0    PE32
>>>88 leshort  0x010b    PE32 executable
>>>>92 leshort 1         (native)
>>>>92 leshort 2         (GUI)
>>>>92 leshort 3         (console)
>>>88 leshort  0x020b    PE32+ executable
>>>>92 leshort 1         (native)
>>>>92 leshort 2         (GUI)
>>>>92 leshort 3         (console)
```

## Data Types and Endianness

### Integer Types

```magic
# Little-endian integers
0    leshort   0x5a4d    MS-DOS executable (little-endian short)
0    lelong    0x464c457f    ELF (little-endian long)

# Big-endian integers
0    beshort   0x4d5a    MS-DOS executable (big-endian short)
0    belong    0x7f454c46    ELF (big-endian long)

# Native endian (system default)
0    short     0x5a4d    MS-DOS executable (native endian)
0    long      0x464c457f    ELF (native endian)
```

### String Matching

```magic
# Fixed-length strings
0    string    #!/bin/sh    shell script
0    string    #!/usr/bin/python    Python script

# Variable-length strings with limits
0    string/32    #!/    script text executable
16   string/256   This program    self-describing executable

# Case-insensitive matching (planned)
0    istring   html    HTML document
0    istring   <html   HTML document
```

## Advanced Offset Specifications

### Indirect Offsets

```magic
# PE section table access
0    string    MZ        MS-DOS executable
>60  lelong    >0
>>60 string    PE\0\0    PE32
>>>(60.l+24)  leshort   >0    sections
>>>>(60.l+24) leshort   x     \b, %d sections
```

### Relative Offsets

```magic
# ZIP file entries
0    string    PK\x03\x04    ZIP archive data
>26  leshort   x         \b, compressed size %d
>28  leshort   x         \b, uncompressed size %d
>30  leshort   >0
>>(30.s+46)   string    x    \b, first entry: "%.64s"
```

### Search Patterns

```magic
# Search for patterns within a range
0      string    \x7fELF    ELF
>0     search/1024    .note.gnu.build-id    \b, with build-id
>0     search/1024    .debug_info    \b, with debug info
```

## Bitwise Operations

### Flag Testing

```magic
# ELF program header flags
0    string    \x7fELF    ELF
>16  leshort   2         executable
>36  lelong    &0x1      \b, executable
>36  lelong    &0x2      \b, writable
>36  lelong    &0x4      \b, readable
```

### Mask Operations

```magic
# File permissions in Unix archives
0    string    070707    cpio archive
>6   long      &0170000
>>6  long      0100000   \b, regular file
>>6  long      0040000   \b, directory
>>6  long      0120000   \b, symbolic link
>>6  long      0060000   \b, block device
>>6  long      0020000   \b, character device
```

## Complex File Format Examples

### JPEG Image Analysis

```magic
# JPEG with EXIF data
0    beshort   0xffd8    JPEG image data
>2   beshort   0xffe1    \b, EXIF standard
>>10 string    Exif\0\0
>>>14 beshort  0x4d4d    \b, big-endian
>>>14 beshort  0x4949    \b, little-endian
>2   beshort   0xffe0    \b, JFIF standard
>>10 string    JFIF
>>>14 byte     x         \b, version %d
>>>15 byte     x         \b.%d
```

### Archive Format Detection

```magic
# TAR archives
257  string    ustar\0   POSIX tar archive
257  string    ustar\040\040\0    GNU tar archive

# RAR archives
0    string    Rar!      RAR archive data
>4   byte      0x1a      \b, version 1.x
>4   byte      0x07      \b, version 5.x

# 7-Zip archives
0    string    7z\xbc\xaf\x27\x1c    7-zip archive data
>6   byte      x         \b, version %d
>7   byte      x         \b.%d
```

### Executable Format Analysis

```magic
# Mach-O executables (macOS)
0    belong    0xfeedface    Mach-O executable (32-bit)
>4   belong    7            i386
>4   belong    18           x86_64
>12  belong    2            executable
>12  belong    6            shared library
>12  belong    8            bundle

0    belong    0xfeedfacf    Mach-O executable (64-bit)
>4   belong    0x01000007   x86_64
>4   belong    0x0100000c   arm64
>12  belong    2            executable
>12  belong    6            shared library
```

## Script and Text File Detection

### Shebang Detection

```magic
# Shell scripts
0    string    #!/bin/sh         POSIX shell script
0    string    #!/bin/bash       Bash shell script
0    string    #!/bin/csh        C shell script
0    string    #!/bin/tcsh       TC shell script
0    string    #!/bin/zsh        Z shell script

# Interpreted languages
0    string    #!/usr/bin/python    Python script
0    string    #!/usr/bin/perl      Perl script
0    string    #!/usr/bin/ruby      Ruby script
0    string    #!/usr/bin/node      Node.js script
0    string    #!/usr/bin/php       PHP script
```

### Text Format Detection

```magic
# Configuration files
0    string    [Desktop\ Entry]    Desktop configuration
0    string    # Configuration      configuration text
0    regex     ^[a-zA-Z_][a-zA-Z0-9_]*\s*=    configuration text

# Source code detection
0    regex     ^#include\s*<       C source code
0    regex     ^package\s+         Java source code
0    regex     ^class\s+\w+:       Python source code
0    regex     ^function\s+        JavaScript source code
```

## Database and Structured Data

### Database Files

```magic
# SQLite databases
0    string    SQLite\ format\ 3    SQLite 3.x database
>13  byte      x                   \b, version %d

# MySQL databases
0    string    \xfe\x01\x00\x00    MySQL table data
0    string    \x00\x00\x00\x00    MySQL ISAM compressed data

# PostgreSQL
0    belong    0x00061561          PostgreSQL custom database dump
>4   belong    x                   \b, version %d
```

### Structured Text Formats

```magic
# JSON files
0    regex     ^\s*[\{\[]          JSON data
>0   search/64 "version"          \b, with version info
>0   search/64 "name"             \b, with name field

# XML files
0    string    <?xml               XML document
>5   search/256 version
>>5  regex     version="([^"]*)"   \b, version \1
>5   search/256 encoding
>>5  regex     encoding="([^"]*)"  \b, encoding \1

# YAML files
0    regex     ^---\s*$            YAML document
0    regex     ^[a-zA-Z_][^:]*:    YAML configuration
```

## Multimedia File Examples

### Audio Formats

```magic
# MP3 files
0    string    ID3                 MP3 audio file with ID3
>3   byte      <0xff               version 2
>>3  byte      x                   \b.%d
0    beshort   0xfffb              MP3 audio file
0    beshort   0xfff3              MP3 audio file
0    beshort   0xffe3              MP3 audio file

# WAV files
0    string    RIFF                Microsoft RIFF
>8   string    WAVE                \b, WAVE audio
>>20 leshort   1                   \b, PCM
>>20 leshort   85                  \b, MPEG Layer 3
>>22 leshort   1                   \b, mono
>>22 leshort   2                   \b, stereo
```

### Video Formats

```magic
# AVI files
0    string    RIFF                Microsoft RIFF
>8   string    AVI\040             \b, AVI video
>>12 string    LIST
>>>20 string   hdrlavih

# MP4/QuickTime
4    string    ftyp                ISO Media
>8   string    isom                \b, MP4 Base Media v1
>8   string    mp41                \b, MP4 v1
>8   string    mp42                \b, MP4 v2
>8   string    qt                  \b, QuickTime movie
```

## Best Practices Examples

### Efficient Rule Ordering

```magic
# Order by probability - most common formats first
0    string    \x7fELF             ELF
0    string    MZ                  MS-DOS executable
0    string    \x89PNG             PNG image data
0    string    \xff\xd8\xff        JPEG image data
0    string    PK\x03\x04          ZIP archive data
0    string    %PDF-               PDF document

# Less common formats later
0    string    \x00\x00\x01\x00    Windows icon
0    string    \x00\x00\x02\x00    Windows cursor
```

### Error-Resistant Patterns

```magic
# Validate magic numbers with additional checks
0    string    \x7fELF             ELF
>4   byte      1                   32-bit
>4   byte      2                   64-bit
>4   byte      >2                  invalid class
>5   byte      1                   little-endian
>5   byte      2                   big-endian
>5   byte      >2                  invalid encoding
```

### Performance Optimizations

```magic
# Use specific offsets instead of searches when possible
0    string    \x7fELF             ELF
>16  leshort   2                   executable
>18  leshort   62                  x86-64

# Prefer shorter patterns for initial matching
0    beshort   0xffd8              JPEG image data
>2   beshort   0xffe0              \b, JFIF standard
>2   beshort   0xffe1              \b, EXIF standard
```

## Testing and Validation

### Test File Creation

```bash
# Create test files for magic rules
echo -e '\x7fELF\x02\x01\x01\x00' > test_elf64.bin
echo -e 'PK\x03\x04\x14\x00' > test_zip.bin
echo '%PDF-1.4' > test_pdf.txt
```

### Rule Validation

```magic
# Include validation comments
# Test: echo -e '\x7fELF\x02\x01\x01\x00' | rmagic -
# Expected: ELF 64-bit LSB executable
0    string    \x7fELF             ELF
>4   byte      2                   64-bit
>5   byte      1                   LSB
>6   byte      1                   current version
```

This comprehensive collection of magic file examples demonstrates the flexibility and power of the magic file format for accurate file type detection.
