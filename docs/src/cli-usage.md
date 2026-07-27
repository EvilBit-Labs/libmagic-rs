# CLI Usage

`rmagic` is a Rust-native alternative to the GNU `file` command. It identifies file types by evaluating magic rules against a file's contents.

## Basic Usage

```bash
# Identify a single file
rmagic document.pdf

# Identify multiple files
rmagic file1.bin file2.exe file3.pdf

# Read from stdin
cat unknown.bin | rmagic -

# Use built-in rules (no external magic file required)
rmagic --use-builtin archive.tar.gz

# Get help
rmagic --help
```

## Arguments and Flags

### Positional Arguments

| Argument  | Description                                                          |
| --------- | -------------------------------------------------------------------- |
| `FILE...` | One or more files to analyze (required). Use `-` to read from stdin. |

### Output Format Flags

| Flag         | Description                                             |
| ------------ | ------------------------------------------------------- |
| `--text`     | Output results in text format. This is the default.     |
| `-j, --json` | Output results in JSON format. Conflicts with `--text`. |

These two flags are mutually exclusive. Passing both `--json` and `--text` produces an error.

### Magic File Flags

| Flag                    | Description                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------ |
| `-m, --magic-file FILE` | Use a custom magic file instead of the system default.                                     |
| `-b, --use-builtin`     | Use built-in magic rules compiled into the binary. Mutually exclusive with `--magic-file`. |

The built-in rules cover common file types: ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, and PDF. They are compiled at build time and require no external files.

### Behavior Flags

| Flag                  | Description                                                                                                                                              |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-s, --strict`        | Exit with a non-zero code on processing failures (I/O, parse, or evaluation errors). A "data" result (unknown file type) is **not** considered an error. |
| `-t, --timeout-ms MS` | Per-file evaluation timeout in milliseconds. Valid range: 1--300000 (5 minutes).                                                                         |
| `-L, --dereference`   | Follow symlinks and report the target's type. This is already the default; the flag exists for GNU `file` compatibility.                                 |
| `--no-dereference`    | Do not follow symlinks; report `symbolic link to <target>` instead of classifying the target.                                                            |

## Output Formats

### Text Output (Default)

Text output prints one line per file in the format `filename: description`:

```bash
$ rmagic image.png document.pdf
image.png: PNG image data
document.pdf: PDF document
```

When a file type cannot be determined, the description is `data`:

```bash
$ rmagic unknown.bin
unknown.bin: data
```

### JSON Output

JSON output varies based on the number of files being analyzed.

**Single file** -- pretty-printed JSON with a matches array:

```bash
rmagic --json image.png
```

```json
{
  "matches": [
    {
      "description": "PNG image data",
      "offset": 0,
      "tags": [
        "image",
        "png"
      ],
      "mime_type": "image/png",
      "score": 90
    }
  ]
}
```

**Multiple files** -- JSON Lines format with one compact JSON object per line:

```bash
$ rmagic --json file1.bin file2.txt
{"filename":"file1.bin","matches":[...]}
{"filename":"file2.txt","matches":[...]}
```

Each line is a self-contained JSON object, making it straightforward to parse with line-oriented tools such as `jq`.

## Stdin Support

Use `-` as the filename to read input from stdin:

```bash
cat sample.bin | rmagic -
```

Stdin input is truncated to the configured `max_string_length` (8192 bytes by default). When truncation occurs, a warning is printed to stderr:

```text
Warning: stdin input truncated to 8192 bytes
```

Output for stdin uses `stdin` as the filename:

```bash
$ echo "hello" | rmagic -
stdin: data
```

Stdin can be combined with regular file arguments:

```bash
rmagic --use-builtin file1.bin - file2.txt < input.dat
```

## Exit Codes

| Code | Meaning                                                 |
| ---- | ------------------------------------------------------- |
| 0    | Success                                                 |
| 1    | General error (evaluation failure, configuration error) |
| 2    | Invalid arguments (bad command-line usage)              |
| 3    | File not found or access denied                         |
| 4    | Magic file not found or invalid                         |
| 5    | Evaluation timeout                                      |

### Strict Mode and Exit Codes

Without `--strict`, processing errors for individual files are printed to stderr but do not affect the exit code. The tool continues processing remaining files and exits 0 if at least the overall invocation succeeded.

With `--strict`, the first processing error (I/O, parse, or evaluation) causes a non-zero exit code. The tool still processes all files and prints errors as they occur, but returns the exit code corresponding to the first error.

A "data" result (unknown file type) is never treated as an error, even in strict mode.

A **broken symlink is** treated as an error by strict mode. Its classification still goes to stdout and the default (non-strict) run still exits 0, but the path was unreadable, which is the category `--strict` exists to catch. `--no-dereference` is not an escape hatch -- a dangling link stays broken under both flags. Not passing `--strict` is. Note that `--strict` over a real filesystem tree will therefore exit non-zero on a healthy machine wherever expected dangling links exist.

A **directory is not** an error: `rmagic <dir>` prints `<dir>: directory` and exits 0 under `--strict` too, because that is a successful detection rather than an I/O failure.

## Symlinks

By default rmagic follows symlinks and reports the type of the target, matching GNU `file`:

```bash
$ rmagic good.link          # link -> real.elf
good.link: ELF 64-bit LSB

$ rmagic broken.link        # link -> a target that does not exist
broken.link: broken symbolic link to missing.txt
$ echo $?
0

$ rmagic --no-dereference good.link
good.link: symbolic link to real.elf
```

Three properties are worth knowing:

- **The target is reproduced verbatim**, exactly as stored in the link. It is never canonicalized or joined against the link's directory, so a relative target prints as a relative path.
- **A missing target, a symlink cycle, and a target inside an unreadable directory all report the same** `broken symbolic link to <target>`. This matches GNU `file`, which does not distinguish them either.
- **Control bytes in a target are escaped only when stdout is a terminal.** Symlink targets have no character restrictions, so a planted link can contain raw escape sequences that would otherwise reach your terminal. Redirected and piped output passes bytes through unchanged, keeping captured output byte-identical to `file`.

`--no-dereference` is the flag to reach for when scanning untrusted trees: under it rmagic never reads or classifies the *content* of a link's target, so a planted link cannot induce it to disclose an attacker-chosen file. See [Security Assurance](security-assurance.md) for the limits of that guarantee.

```bash
# Without strict: exits 0 even if some files fail
$ rmagic file1.bin nonexistent.bin file2.txt
file1.bin: data
Error processing nonexistent.bin: ...
file2.txt: data
$ echo $?
0

# With strict: exits with error code from first failure
$ rmagic --strict file1.bin nonexistent.bin file2.txt
file1.bin: data
Error processing nonexistent.bin: ...
file2.txt: data
$ echo $?
3
```

## Magic File Discovery

When `--use-builtin` is not specified and no `--magic-file` is provided, `rmagic` searches for a magic file in standard system locations. The search follows an OpenBSD-inspired approach, preferring human-readable text files over compiled binary `.mgc` files.

### Search Order (Unix)

Text directories and files are checked first. If a text-format file or directory is found, it is used immediately. If only binary `.mgc` files exist, the first one found is used as a fallback.

| Priority | Path                              | Format              |
| -------- | --------------------------------- | ------------------- |
| 1        | `/usr/share/file/magic/Magdir`    | Text directory      |
| 2        | `/usr/share/file/magic`           | Text directory/file |
| 3        | `/usr/share/misc/magic`           | Text file           |
| 4        | `/usr/local/share/misc/magic`     | Text file           |
| 5        | `/etc/magic`                      | Text file           |
| 6        | `/opt/local/share/file/magic`     | Text file           |
| 7        | `/usr/share/file/magic.mgc`       | Binary              |
| 8        | `/usr/local/share/misc/magic.mgc` | Binary              |
| 9        | `/opt/local/share/file/magic.mgc` | Binary              |
| 10       | `/etc/magic.mgc`                  | Binary              |
| 11       | `/usr/share/misc/magic.mgc`       | Binary              |

If none of these paths exist, `rmagic` falls back to `/usr/share/file/magic.mgc`.

### Windows

On Windows, the tool checks `%APPDATA%\Magic\magic` first, then falls back to the bundled `third_party/magic.mgc`.

## Timeout Configuration

The `--timeout-ms` flag sets a per-file timeout for magic rule evaluation. Each file gets its own independent timeout window. If evaluation exceeds the specified duration, the file is skipped with an error.

```bash
# Set a 500ms timeout per file
$ rmagic --timeout-ms 500 large_file.bin

# Combine with strict mode to fail on timeout
$ rmagic --strict --timeout-ms 1000 *.bin
```

Valid values range from 1 to 300000 (5 minutes).

## Multiple File Processing

When multiple files are provided, each file is processed sequentially with independent error handling. A failure in one file does not prevent processing of subsequent files.

```bash
$ rmagic --use-builtin image.png archive.zip README.md
image.png: PNG image data
archive.zip: Zip archive data
README.md: data
```

Errors for individual files are printed to stderr with the filename for context:

```bash
$ rmagic --use-builtin good.png /nonexistent bad_perms.bin
good.png: PNG image data
Error processing /nonexistent: ...
Error processing bad_perms.bin: ...
```

## Examples

### Identify files with built-in rules

```bash
$ rmagic --use-builtin photo.jpg
photo.jpg: JPEG image data, JFIF standard
```

### JSON output for scripting

```bash
$ rmagic --use-builtin --json binary.elf | jq '.matches[0].mime_type'
"application/x-executable"
```

### Process files from a directory listing

```bash
ls *.bin | xargs rmagic --use-builtin --strict
```

### Custom magic file

```bash
$ rmagic --magic-file /path/to/custom.magic firmware.img
firmware.img: ARM firmware image
```

### Pipeline with stdin

```bash
$ curl -sL https://example.com/file | rmagic --use-builtin -
stdin: Zip archive data
```

### Strict mode in CI

```bash
#!/bin/bash
rmagic --use-builtin --strict --json artifacts/*.bin
if [ $? -ne 0 ]; then
    echo "File identification failed" >&2
    exit 1
fi
```

## Related Chapters

- [Getting Started](./getting-started.md) -- installation and first steps
- [Configuration](./configuration.md) -- evaluation configuration options
- [Error Handling](./error-handling.md) -- detailed error type documentation
- [Command Reference](./cli-reference.md) -- complete flag and option reference
