# CLI Usage

> [!NOTE]
> The CLI is currently in early development with placeholder functionality. This documentation describes the planned interface.

The `rmagic` command-line tool provides a drop-in replacement for the GNU `file` command, with additional features for modern workflows.

## Basic Usage

```bash
# Identify a single file
rmagic file.bin

# Identify multiple files
rmagic file1.bin file2.exe file3.pdf

# Get help
rmagic --help
```

## Output Formats

### Text Output (Default)

```bash
rmagic example.bin
# Output: example.bin: ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

### JSON Output

```bash
rmagic example.bin --json
```

```json
{
  "filename": "example.bin",
  "description": "ELF 64-bit LSB executable, x86-64, version 1 (SYSV)",
  "mime_type": "application/x-executable",
  "confidence": 0.95
}
```

## Command-Line Options

### Input Options

- `FILE...` - Files to analyze (required)
- `--magic-file FILE` - Use custom magic file database

### Output Options

- `--text` - Text output format (default)
- `--json` - JSON output format
- `--mime` - Output MIME type only

### Behavior Options

- `--brief` - Don't prepend filenames to output lines
- `--no-buffer` - Don't buffer output (useful for pipes)

## Examples

Coming soon with full implementation.

## Exit Codes

- `0` - Success
- `1` - Error processing files
- `2` - Invalid command-line arguments
