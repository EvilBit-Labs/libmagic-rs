# Appendix B: Command Reference

This appendix provides a comprehensive reference for all command-line options and usage patterns of the `rmagic` tool.

## Command Syntax

```bash
rmagic [OPTIONS] <FILE>...
```

## Options

### Basic Options

#### `<FILE>`

- **Type**: Positional argument (required)
- **Description**: Path to the file(s) to analyze
- **Multiple**: Yes, can specify multiple files
- **Examples**:
  ```bash
  rmagic file.bin
  rmagic file1.exe file2.pdf file3.zip
  rmagic /path/to/directory/*
  ```

#### `--help`, `-h`

- **Description**: Display help information and exit
- **Example**:
  ```bash
  rmagic --help
  ```

#### `--version`, `-V`

- **Description**: Display version information and exit
- **Example**:
  ```bash
  rmagic --version
  ```

### Output Format Options

#### `--json`

- **Description**: Output results in JSON format instead of text
- **Default**: Text format
- **Example**:
  ```bash
  rmagic --json file.bin
  ```
- **Output Example**:
  ```json
  {
    "filename": "file.bin",
    "description": "ELF 64-bit LSB executable",
    "mime_type": "application/x-executable",
    "confidence": 1.0
  }
  ```

#### `--text`

- **Description**: Output results in text format (default behavior)
- **Default**: Enabled
- **Example**:
  ```bash
  rmagic --text file.bin
  # Output: file.bin: ELF 64-bit LSB executable
  ```

### Magic Database Options

#### `--magic-file <FILE>`

- **Description**: Use a custom magic file instead of the default
- **Type**: Path to magic file
- **Default**: Built-in magic database
- **Example**:
  ```bash
  rmagic --magic-file custom.magic file.bin
  rmagic --magic-file /usr/share/misc/magic file.bin
  ```

### Advanced Options (Planned)

#### `--mime-type`, `-i`

- **Description**: Output MIME type instead of description
- **Status**: 📋 Planned
- **Example**:
  ```bash
  rmagic --mime-type file.bin
  # Output: application/x-executable
  ```

#### `--mime-encoding`, `-e`

- **Description**: Output MIME encoding
- **Status**: 📋 Planned
- **Example**:
  ```bash
  rmagic --mime-encoding text.txt
  # Output: us-ascii
  ```

#### `--brief`, `-b`

- **Description**: Brief output (no filename prefix)
- **Status**: 📋 Planned
- **Example**:
  ```bash
  rmagic --brief file.bin
  # Output: ELF 64-bit LSB executable
  ```

#### `--raw`, `-r`

- **Description**: Raw output (no pretty formatting)
- **Status**: 📋 Planned

#### `--follow-symlinks`, `-L`

- **Description**: Follow symbolic links
- **Status**: 📋 Planned

#### `--no-follow-symlinks`, `-h`

- **Description**: Don't follow symbolic links (default)
- **Status**: 📋 Planned

#### `--compress`, `-z`

- **Description**: Try to look inside compressed files
- **Status**: 📋 Planned

#### `--uncompress`, `-Z`

- **Description**: Try to look inside compressed files (same as -z)
- **Status**: 📋 Planned

#### `--exclude <PATTERN>`

- **Description**: Exclude files matching pattern
- **Status**: 📋 Planned

#### `--include <PATTERN>`

- **Description**: Only include files matching pattern
- **Status**: 📋 Planned

## Usage Examples

### Basic File Identification

```bash
# Single file
rmagic document.pdf
# Output: document.pdf: PDF document, version 1.4

# Multiple files
rmagic *.bin
# Output:
# file1.bin: ELF 64-bit LSB executable
# file2.bin: data
# file3.bin: PNG image data, 1920 x 1080, 8-bit/color RGBA
```

### JSON Output

```bash
# Single file JSON output
rmagic --json executable.elf
```

```json
{
  "filename": "executable.elf",
  "description": "ELF 64-bit LSB executable, x86-64, version 1 (SYSV)",
  "mime_type": "application/x-executable",
  "confidence": 1.0,
  "matches": [
    {
      "offset": 0,
      "rule": "ELF magic",
      "value": "7f454c46",
      "message": "ELF"
    },
    {
      "offset": 4,
      "rule": "ELF class",
      "value": "02",
      "message": "64-bit"
    }
  ]
}
```

### Custom Magic Files

```bash
# Use custom magic database
rmagic --magic-file /path/to/custom.magic file.bin

# Use multiple magic files (planned)
rmagic --magic-file magic1.db --magic-file magic2.db file.bin
```

### Batch Processing

```bash
# Process all files in directory
rmagic /path/to/files/*

# Process with JSON output for scripting
rmagic --json /path/to/files/* > results.json

# Process recursively (planned)
rmagic --recursive /path/to/directory/
```

## Exit Codes

| Code | Meaning                                                         |
| ---- | --------------------------------------------------------------- |
| 0    | Success - all files processed successfully                      |
| 1    | Error - general error (file not found, permission denied, etc.) |
| 2    | Usage error - invalid command line arguments                    |
| 3    | Magic file error - invalid or missing magic file                |

## Environment Variables

### `MAGIC`

- **Description**: Default magic file path
- **Default**: Built-in magic database
- **Example**:
  ```bash
  export MAGIC=/usr/local/share/magic
  rmagic file.bin  # Uses /usr/local/share/magic
  ```

### `RMAGIC_DEBUG`

- **Description**: Enable debug output
- **Values**: `0` (off), `1` (basic), `2` (verbose)
- **Example**:
  ```bash
  RMAGIC_DEBUG=1 rmagic file.bin
  ```

## Configuration Files (Planned)

### Global Configuration

- **Path**: `/etc/rmagic.conf`
- **Format**: TOML
- **Purpose**: System-wide defaults

### User Configuration

- **Path**: `~/.config/rmagic/config.toml`
- **Format**: TOML
- **Purpose**: User-specific settings

### Example Configuration

```toml
[output]
format = "json"
brief = false

[magic]
default_file = "/usr/local/share/magic"
search_paths = [
  "/usr/share/misc/magic",
  "/usr/local/share/magic",
  "~/.local/share/magic",
]

[performance]
max_file_size = "100MB"
timeout = "30s"
```

## Compatibility with GNU file

The `rmagic` command aims for compatibility with GNU `file` command:

### Compatible Options

- Basic file analysis
- JSON output format
- Custom magic file specification
- Multiple file processing

### Differences

- JSON output format may differ in structure
- Some advanced GNU `file` options not yet implemented
- Performance characteristics may vary
- Error messages may differ

### Migration Guide

```bash
# GNU file command
file -i document.pdf
file --mime-type document.pdf

# rmagic equivalent (planned)
rmagic --mime-type document.pdf
rmagic -i document.pdf
```

## Performance Considerations

### Large Files

- Files are memory-mapped for efficiency
- Only necessary portions are read
- Configurable size limits prevent excessive memory usage

### Batch Processing

- Multiple files processed efficiently
- Parallel processing planned for future versions
- Progress reporting for large batches

### Memory Usage

- Constant memory usage regardless of file size
- Magic database cached in memory
- Minimal allocations during evaluation

## Troubleshooting

### Common Issues

#### "File not found"

```bash
rmagic nonexistent.file
# Error: File not found: nonexistent.file
```

**Solution**: Check file path and permissions

#### "Permission denied"

```bash
rmagic /root/private.file
# Error: Permission denied: /root/private.file
```

**Solution**: Check file permissions or run with appropriate privileges

#### "Invalid magic file"

```bash
rmagic --magic-file broken.magic file.bin
# Error: Parse error in magic file at line 42: Invalid offset specification
```

**Solution**: Validate magic file syntax

### Debug Mode

```bash
# Enable debug output
RMAGIC_DEBUG=1 rmagic file.bin

# Verbose debug output
RMAGIC_DEBUG=2 rmagic file.bin
```

This command reference provides comprehensive documentation for all current and planned features of the `rmagic` command-line tool.
