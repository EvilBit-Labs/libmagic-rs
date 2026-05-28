# Getting Started with libmagic-rs

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Library Usage](#library-usage)
- [CLI Usage](#cli-usage)
- [Common Patterns](#common-patterns)
- [Next Steps](#next-steps)

---

## Installation

### Add to your project

Add libmagic-rs to your `Cargo.toml`:

```toml
[dependencies]
libmagic-rs = "0.6.0"
```

#### Upgrading from earlier versions

**0.6.0**

- `EvaluationConfig` is now `#[non_exhaustive]`. Construct it with `EvaluationConfig::default()` and the `with_*` builder methods (`with_timeout_ms`, `with_max_recursion_depth`, `with_max_string_length`, `with_stop_at_first_match`, `with_mime_types`), or with `..Default::default()` syntax. Struct literals no longer compile.
- `MagicRule` gained a `value_transform` field.
- `OffsetSpec`, `LibmagicError`, `IoError`, `Operator`, `TypeReadError`, `ParseError`, `Value`, `TypeKind`, and `EvaluationError` are all `#[non_exhaustive]`. Pattern matches on these need a wildcard `_` arm.
- `parse_text_magic_file` returns `ParsedMagic { rules, name_table }` instead of `Vec<MagicRule>`.
- Several parser grammar functions moved from public to internal API.
- `evaluate_single_rule` signature changed.
- `MimeMapper` now implements `Copy`.

**0.5.0**

- `RuleMatch` has a new `type_kind` field; struct literals need updating.
- `Value` no longer derives `Eq`, which affects comparison operations.
- `TypeKind` gained `Float` and `Double` variants for floating-point types with endian variants. The `TypeKind::String` discriminant moved from 4 to 6, so exhaustive matches on `TypeKind` need updating.

**0.4.0**

- `Operator` gained `BitwiseXor`, `BitwiseNot`, and `AnyValue`. Exhaustive matches on `Operator` need updating.

**0.3.0**

- Added `TypeKind::Quad` for 64-bit quad integer types with endian variants. The `TypeKind::String` discriminant moved from 3 to 4.
- `evaluator::MatchResult` was renamed to `evaluator::RuleMatch` to avoid a collision with `output::MatchResult`. The public re-export is `RuleMatch`.

**0.2.0**

- `TypeKind::Byte` changed from a unit variant to a tuple variant.
- `Operator` gained `LessThan`, `GreaterThan`, `LessEqual`, and `GreaterEqual`.

### Build from Source

```bash
git clone https://github.com/EvilBit-Labs/libmagic-rs
cd libmagic-rs
cargo build --release
```

### Install CLI Tool

```bash
# From source
cargo install --path .

# Verify installation
rmagic --version
```

---

## Quick start

#### Step 1: Create a new project

```bash
cargo new my-file-analyzer
cd my-file-analyzer
```

#### Step 2: Add the dependency

Edit `Cargo.toml`:

```toml
[dependencies]
libmagic-rs = "0.6.0"
```

#### Step 3: Write the code

Edit `src/main.rs`:

```rust
use libmagic_rs::MagicDatabase;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load built-in magic rules
    let db = MagicDatabase::with_builtin_rules()?;

    // Analyze a file
    let result = db.evaluate_file("test.bin")?;

    // Print the result
    println!("File type: {}", result.description);

    Ok(())
}
```

#### Step 4: Create a test file

```bash
# Create a test ZIP file
echo "test content" > test.txt
zip test.bin test.txt
```

#### Step 5: Run

```bash
cargo run
# Output: File type: ZIP archive data
```

---

## Library Usage

### Loading Magic Rules

#### Option 1: Built-in Rules (Recommended for Simplicity)

```rust
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;
```

Built-in rules support: ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF.

#### Option 2: From File

```rust
use libmagic_rs::MagicDatabase;

// Load from text magic file
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;

// Load from directory (Magdir style)
let db = MagicDatabase::load_from_file("/usr/share/file/magic")?;
```

#### Option 3: With Custom Configuration

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig::default()
    .with_timeout_ms(Some(5000))        // 5 second timeout
    .with_mime_types(true)              // Get MIME types
    .with_max_string_length(16384);     // Larger string buffer

let db = MagicDatabase::with_builtin_rules_and_config(config)?;
```

### Evaluating Files

#### Evaluate a File Path

```rust
let result = db.evaluate_file("document.pdf")?;

println!("Type: {}", result.description);
println!("Confidence: {:.0}%", result.confidence * 100.0);

if let Some(mime) = &result.mime_type {
    println!("MIME: {}", mime);
}
```

#### Evaluate a Buffer (Memory Data)

```rust
// Useful for stdin, network data, or already-loaded content
let data = std::fs::read("document.pdf")?;
let result = db.evaluate_buffer(&data)?;

println!("Type: {}", result.description);
```

#### Evaluate Multiple Files

```rust
let files = vec!["file1.bin", "file2.bin", "file3.bin"];

for file in files {
    match db.evaluate_file(file) {
        Ok(result) => println!("{}: {}", file, result.description),
        Err(e) => eprintln!("{}: Error - {}", file, e),
    }
}
```

### Working with Results

#### Access Match Details

```rust
let result = db.evaluate_file("executable.elf")?;

// Primary description
println!("Description: {}", result.description);

// Individual matches
for match_result in &result.matches {
    println!("  Offset {}: {}", match_result.offset, match_result.message);
    println!("  Confidence: {:.0}%", match_result.confidence * 100.0);
}

// Evaluation metadata
println!("File size: {} bytes", result.metadata.file_size);
println!("Evaluation time: {:.2}ms", result.metadata.evaluation_time_ms);
```

#### Handle Unknown Files

```rust
let result = db.evaluate_file("unknown.dat")?;

if result.description == "data" {
    println!("Unknown file type");
} else {
    println!("Identified as: {}", result.description);
}
```

### Error Handling

#### Basic Error Handling

```rust
use libmagic_rs::{MagicDatabase, LibmagicError};

match MagicDatabase::load_from_file("magic.db") {
    Ok(db) => {
        // Use database
    }
    Err(LibmagicError::IoError(e)) => {
        eprintln!("File error: {}", e);
    }
    Err(LibmagicError::ParseError(e)) => {
        eprintln!("Parse error: {}", e);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

#### Comprehensive Error Handling

```rust
use libmagic_rs::{MagicDatabase, LibmagicError, ParseError, EvaluationError};

fn analyze_file(path: &str) -> Result<String, String> {
    let db = MagicDatabase::with_builtin_rules()
        .map_err(|e| format!("Failed to load rules: {}", e))?;

    match db.evaluate_file(path) {
        Ok(result) => Ok(result.description),
        Err(LibmagicError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("File not found: {}", path))
        }
        Err(LibmagicError::IoError(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(format!("Permission denied: {}", path))
        }
        Err(LibmagicError::EvaluationError(EvaluationError::Timeout { timeout_ms })) => {
            Err(format!("Timeout after {}ms", timeout_ms))
        }
        Err(e) => Err(format!("Evaluation failed: {}", e)),
    }
}
```

---

## CLI Usage

### Basic Commands

```bash
# Identify a file
rmagic document.pdf

# Multiple files
rmagic *.bin

# With built-in rules
rmagic --use-builtin image.png

# From stdin
cat unknown.bin | rmagic -
```

### Output Formats

```bash
# Text output (default)
rmagic file.bin
# Output: file.bin: ELF 64-bit executable

# JSON output (single file)
rmagic --json file.bin
# Output: {"matches": [...]}

# JSON Lines (multiple files)
rmagic --json file1.bin file2.bin
# Output: {"filename":"file1.bin",...}
#         {"filename":"file2.bin",...}
```

### Common Workflows

```bash
# Find all ELF executables
find . -type f -exec rmagic --use-builtin {} + | grep ELF

# Process with jq
rmagic --json file.bin | jq '.matches[0].text'

# Batch processing
for f in *.dat; do
    echo -n "$f: "
    rmagic --use-builtin "$f"
done
```

---

## Common Patterns

### Pattern 1: File Type Validator

```rust
use libmagic_rs::MagicDatabase;

fn is_image(path: &str) -> bool {
    let check = || -> Option<bool> {
        let db = MagicDatabase::with_builtin_rules().ok()?;
        let result = db.evaluate_file(path).ok()?;

        let desc = result.description.to_lowercase();
        Some(desc.contains("image") || desc.contains("jpeg") ||
             desc.contains("png") || desc.contains("gif"))
    };
    check().unwrap_or(false)
}
```

### Pattern 2: Safe Upload Handler

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

fn validate_upload(data: &[u8], allowed_types: &[&str]) -> Result<bool, String> {
    let config = EvaluationConfig::default()
        .with_timeout_ms(Some(1000));  // Short timeout for uploads

    let db = MagicDatabase::with_builtin_rules_and_config(config)
        .map_err(|e| e.to_string())?;

    let result = db.evaluate_buffer(data)
        .map_err(|e| e.to_string())?;

    let desc = result.description.to_lowercase();
    Ok(allowed_types.iter().any(|t| desc.contains(&t.to_lowercase())))
}

// Usage
let data = std::fs::read("upload.jpg")?;
let is_valid = validate_upload(&data, &["jpeg", "png", "gif"])?;
```

### Pattern 3: Batch Processor

```rust
use libmagic_rs::MagicDatabase;
use std::path::Path;

fn process_directory(dir: &Path) -> Vec<(String, String)> {
    let db = match MagicDatabase::with_builtin_rules() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to load rules: {}", e);
            return vec![];
        }
    };

    let mut results = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let type_str = match db.evaluate_file(&path) {
                    Ok(result) => result.description,
                    Err(_) => "error".to_string(),
                };
                results.push((path.display().to_string(), type_str));
            }
        }
    }

    results
}
```

### Pattern 4: JSON API Response

```rust
use libmagic_rs::MagicDatabase;
use serde::Serialize;

#[derive(Serialize)]
struct FileInfo {
    filename: String,
    file_type: String,
    mime_type: Option<String>,
    confidence: f64,
    size: u64,
}

fn get_file_info(path: &str) -> Result<FileInfo, String> {
    let config = libmagic_rs::EvaluationConfig::default()
        .with_mime_types(true);

    let db = MagicDatabase::with_builtin_rules_and_config(config)
        .map_err(|e| e.to_string())?;

    let result = db.evaluate_file(path)
        .map_err(|e| e.to_string())?;

    Ok(FileInfo {
        filename: path.to_string(),
        file_type: result.description,
        mime_type: result.mime_type,
        confidence: result.confidence,
        size: result.metadata.file_size,
    })
}
```

---

## Next steps

The deeper docs cover specific tasks:

- [API Reference](API_REFERENCE.md) — complete API documentation
- [Architecture Guide](ARCHITECTURE.md) — internals
- [CLI Reference](CLI_REFERENCE.md) — full CLI documentation
- [Magic File Format](MAGIC_FORMAT.md) — writing custom rules

Before shipping anything that touches untrusted input:

- Create one `MagicDatabase` and reuse it across many files. Do not reload it per call.
- Set `timeout_ms` on `EvaluationConfig`. The default is unbounded.
- If the result `description` is the literal string `"data"`, no rule matched. Handle that case.
- The built-in rules cover ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF. Use them first; add custom rules only when needed.

Bug reports go to [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues). API docs are at [docs.rs/libmagic-rs](https://docs.rs/libmagic-rs).

---

## Quick reference

```rust
// Load database
let db = MagicDatabase::with_builtin_rules()?;

// Evaluate file
let result = db.evaluate_file("file.bin")?;

// Get description
println!("{}", result.description);

// Check confidence
if result.confidence > 0.8 {
    println!("High confidence match");
}

// Handle unknown
if result.description == "data" {
    println!("Unknown file type");
}
```

```bash
# CLI quick reference
rmagic file.bin                    # Basic usage
rmagic --use-builtin file.bin      # Built-in rules
rmagic --json file.bin             # JSON output
rmagic --timeout-ms 5000 file.bin  # With timeout
rmagic - < file.bin                # From stdin
```
