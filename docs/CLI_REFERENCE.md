# CLI Reference - rmagic

Command-line interface documentation for the `rmagic` file identification tool.

## Overview

`rmagic` is a pure-Rust implementation of the `file` command for file type identification using magic rules.

## Installation

```bash
# From crates.io (when published)
cargo install libmagic-rs

# From source
git clone https://github.com/EvilBit-Labs/libmagic-rs
cd libmagic-rs
cargo install --path .
```

## Synopsis

```text
rmagic [OPTIONS] <FILE>...
rmagic [OPTIONS] -
```

## Description

`rmagic` analyzes files and determines their types based on magic rules. It examines file contents rather than relying on file extensions, providing accurate identification for binary files, archives, executables, images, and more.

## Arguments

| Argument    | Description                  |
| ----------- | ---------------------------- |
| `<FILE>...` | One or more files to analyze |
| `-`         | Read from standard input     |

## Options

### Output Format

| Option       | Description                             |
| ------------ | --------------------------------------- |
| `-j, --json` | Output results in JSON format           |
| `--text`     | Output results in text format (default) |

**Note:** `--json` and `--text` are mutually exclusive.

### Magic File Selection

| Option                    | Description                        |
| ------------------------- | ---------------------------------- |
| `-m, --magic-file <FILE>` | Use custom magic file or directory |
| `-b, --use-builtin`       | Use built-in magic rules           |

**Note:** `--magic-file` and `--use-builtin` are mutually exclusive.

### Behavior

| Option                  | Description                          |
| ----------------------- | ------------------------------------ |
| `-s, --strict`          | Exit with non-zero code on any error |
| `-t, --timeout-ms <MS>` | Set evaluation timeout (1-300000ms)  |

### Help

| Option          | Description               |
| --------------- | ------------------------- |
| `-h, --help`    | Print help information    |
| `-V, --version` | Print version information |

## Exit Codes

| Code | Description                     |
| ---- | ------------------------------- |
| `0`  | Success                         |
| `1`  | General evaluation error        |
| `2`  | Invalid arguments (misuse)      |
| `3`  | File not found or access denied |
| `4`  | Magic file not found or invalid |
| `5`  | Evaluation timeout              |

## Output Formats

### Text Format (Default)

One line per file in the format:

```text
filename: description
```

**Examples:**

```text
document.pdf: PDF document
image.png: PNG image data
binary.exe: PE32 executable
```

### JSON Format

**Single file:** Pretty-printed JSON with full details.

```json
{
  "matches": [
    {
      "text": "ELF 64-bit LSB executable",
      "offset": 0,
      "value": "7f454c46",
      "tags": [
        "executable",
        "elf"
      ],
      "score": 90,
      "mime_type": "application/x-executable"
    }
  ]
}
```

**Multiple files:** JSON Lines format (compact, one JSON object per line).

```json
{"filename":"file1.bin","matches":[...]}
{"filename":"file2.bin","matches":[...]}
```

## Magic File Discovery

When no `--magic-file` is specified and `--use-builtin` is not used, `rmagic` searches for magic files in this order (OpenBSD-style, text-first):

### Text Directories (Highest Priority)

1. `/usr/share/file/magic/Magdir`
2. `/usr/share/file/magic`

### Text Files

3. `/usr/share/misc/magic`
4. `/usr/local/share/misc/magic`
5. `/etc/magic`
6. `/opt/local/share/file/magic`

### Binary Files (Fallback)

07. `/usr/share/file/magic.mgc`
08. `/usr/local/share/misc/magic.mgc`
09. `/opt/local/share/file/magic.mgc`
10. `/etc/magic.mgc`
11. `/usr/share/misc/magic.mgc`

### Development Fallbacks

12. `missing.magic` (current directory)
13. `third_party/magic.mgc`

**Note:** Binary `.mgc` files are currently unsupported. Use `--use-builtin` or a text magic file.

## Built-in Rules

The `--use-builtin` flag uses pre-compiled rules for common file types:

| Category    | Formats             |
| ----------- | ------------------- |
| Executables | ELF, PE/DOS (MZ)    |
| Archives    | ZIP, TAR, GZIP      |
| Images      | JPEG, PNG, GIF, BMP |
| Documents   | PDF                 |

## Examples

### Basic Usage

```bash
# Identify a single file
rmagic document.pdf

# Identify multiple files
rmagic *.bin

# Use built-in rules
rmagic --use-builtin image.png

# Read from stdin
cat unknown.bin | rmagic -
```

### JSON Output

```bash
# Single file with pretty JSON
rmagic --json executable.elf

# Multiple files with JSON Lines
rmagic --json file1.bin file2.bin file3.bin

# Parse JSON output with jq
rmagic --json binary.exe | jq '.matches[0].text'
```

### Custom Magic File

```bash
# Use specific magic file
rmagic --magic-file /path/to/custom.magic files/*

# Use magic directory (Magdir style)
rmagic --magic-file /usr/share/file/magic files/*
```

### Error Handling

```bash
# Strict mode - fail on first error
rmagic --strict *.bin

# With timeout protection
rmagic --timeout-ms 5000 large-file.bin

# Combine options
rmagic --strict --timeout-ms 10000 --json *.bin
```

### Pipeline Usage

```bash
# Find all ELF files
find . -type f -exec rmagic --use-builtin {} + | grep ELF

# Process files and output JSON
for f in *.bin; do
    rmagic --json "$f" >> results.jsonl
done

# Use with xargs
find . -name "*.dat" -print0 | xargs -0 rmagic --use-builtin
```

### Scripting

```bash
#!/bin/bash
# Check if file is an image

if rmagic --use-builtin "$1" | grep -q "image"; then
    echo "File is an image"
    exit 0
else
    echo "File is not an image"
    exit 1
fi
```

## Configuration

### Environment Variables

| Variable         | Description                                   |
| ---------------- | --------------------------------------------- |
| `CI`             | Enables CI mode (affects magic file fallback) |
| `GITHUB_ACTIONS` | Enables GitHub Actions mode                   |

### Platform-Specific Behavior

#### Unix (Linux, macOS, BSD)

- Full magic file discovery
- Memory-mapped file access
- Standard Unix exit codes

#### Windows

- Limited magic file locations
- Falls back to `%APPDATA%\Magic\magic`
- Uses `third_party/magic.mgc` in CI

## Troubleshooting

### Common Issues

#### Magic file not found

```bash
# Solution 1: Use built-in rules
rmagic --use-builtin file.bin

# Solution 2: Specify magic file path
rmagic --magic-file /path/to/magic file.bin

# Solution 3: Check available locations
ls -la /usr/share/misc/magic /usr/share/file/magic* 2>/dev/null
```

#### Unsupported format: binary .mgc

```bash
# Binary .mgc files are not supported
# Use --use-builtin or a text magic file

rmagic --use-builtin file.bin
```

#### Evaluation timeout

```bash
# Increase timeout
rmagic --timeout-ms 30000 large-file.bin

# Or use simpler rules
rmagic --use-builtin large-file.bin
```

#### Permission denied

```bash
# Check file permissions
ls -la file.bin

# Run with appropriate permissions
sudo rmagic file.bin
```

### Debug Tips

```bash
# Check which magic file is being used
rmagic --help  # Shows version

# Test with built-in rules first
rmagic --use-builtin test-file.bin

# Verbose error with strict mode
rmagic --strict file.bin
```

## Comparison with GNU file

| Feature             | rmagic      | GNU file         |
| ------------------- | ----------- | ---------------- |
| Binary .mgc support | No          | Yes              |
| Text magic files    | Yes         | Yes              |
| Built-in rules      | Yes         | No               |
| Memory safety       | Rust (safe) | C                |
| JSON output         | Native      | Requires wrapper |
| Timeout support     | Yes         | No               |

### Migration from file

```bash
# Before (GNU file)
file document.pdf

# After (rmagic)
rmagic document.pdf

# With options
file -i document.pdf      # MIME type
rmagic --json document.pdf | jq '.matches[0].mime_type'
```

## See Also

- [API Reference](API_REFERENCE.md) - Library API documentation
- [Architecture](ARCHITECTURE.md) - Internal design documentation
- [file(1)](https://man7.org/linux/man-pages/man1/file.1.html) - GNU file command
- [magic(5)](https://man7.org/linux/man-pages/man5/magic.5.html) - Magic file format

## License

Apache-2.0
