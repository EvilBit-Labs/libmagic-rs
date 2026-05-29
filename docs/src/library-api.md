# Library API

The `libmagic_rs` crate exposes a Rust API for identifying file types by evaluating magic rules against file contents.

## Quick Start

The fastest way to get started is with built-in rules, which require no external files:

```rust,no_run
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;
let result = db.evaluate_file("sample.bin")?;
println!("File type: {}", result.description);
println!("Confidence: {:.0}%", result.confidence * 100.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Built-in rules are compiled into the binary at build time and detect common file types including ELF, PE/DOS, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, and PDF.

## MagicDatabase

`MagicDatabase` is the main entry point. It holds parsed rules, an evaluation configuration, and a cached MIME mapper. Four constructors are available:

```rust,no_run
use libmagic_rs::{MagicDatabase, EvaluationConfig};

// Built-in rules with default config
let db = MagicDatabase::with_builtin_rules()?;

// Built-in rules with custom config
let db = MagicDatabase::with_builtin_rules_and_config(EvaluationConfig::performance())?;

// Load from a file or directory (auto-detects format)
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;

// Load from a file or directory with custom config
let config = EvaluationConfig::comprehensive();
let db = MagicDatabase::load_from_file_with_config("/usr/share/misc/magic.d", config)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

When a directory path is given, all magic files within it are loaded (the Magdir pattern). Binary `.mgc` files are not supported; the library returns a descriptive error if one is encountered.

### Evaluation

```rust,no_run
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;

// Evaluate a file on disk (uses memory-mapped I/O internally)
let result = db.evaluate_file("document.pdf")?;
println!("{}", result.description);

// Evaluate an in-memory buffer (useful for stdin or pre-loaded data)
let result = db.evaluate_buffer(b"\x7fELF\x02\x01\x01\x00")?;
println!("{}", result.description);
# Ok::<(), Box<dyn std::error::Error>>(())
```

When no rules match, the description defaults to `"data"` with confidence `0.0`.

### Accessors

- `config() -> &EvaluationConfig` -- returns the active configuration.
- `source_path() -> Option<&Path>` -- returns the path rules were loaded from, or `None` for built-in rules.

## EvaluationConfig

Controls evaluation behavior with these fields:

| Field                 | Type          | Default | Description                                                                                                                                                               |
| --------------------- | ------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `max_recursion_depth` | `u32`         | `20`    | Maximum depth for nested rule evaluation                                                                                                                                  |
| `max_string_length`   | `usize`       | `8192`  | Caps bytes read for `TypeKind::String` reads (both unflagged and with `/c`/`/C`/`/w`/`/W`/`/T`/`/f` flags); does NOT apply to `TypeKind::PString` or `TypeKind::String16` |
| `stop_at_first_match` | `bool`        | `true`  | Stop after the first matching rule                                                                                                                                        |
| `enable_mime_types`   | `bool`        | `false` | Map descriptions to MIME types                                                                                                                                            |
| `timeout_ms`          | `Option<u64>` | `None`  | Evaluation timeout in milliseconds                                                                                                                                        |

### Presets

```rust
use libmagic_rs::EvaluationConfig;

// Default values (same as EvaluationConfig::default())
let default = EvaluationConfig::new();

// Speed: lower limits, 1s timeout, stop at first match
let fast = EvaluationConfig::performance();
assert_eq!(fast.max_recursion_depth, 10);
assert_eq!(fast.timeout_ms, Some(1000));

// Completeness: higher limits, MIME enabled, find all matches, 30s timeout
let full = EvaluationConfig::comprehensive();
assert!(!full.stop_at_first_match);
assert!(full.enable_mime_types);
```

### Custom Configuration

```rust
use libmagic_rs::EvaluationConfig;

let config = EvaluationConfig {
    max_recursion_depth: 30,
    max_string_length: 16384,
    stop_at_first_match: false,
    enable_mime_types: true,
    timeout_ms: Some(5000),
};
```

### Security Validation

Call `validate()` to check that values are within safe bounds. All `MagicDatabase` constructors call `validate()` automatically.

```rust
use libmagic_rs::EvaluationConfig;

let config = EvaluationConfig::default();
assert!(config.validate().is_ok());

let bad = EvaluationConfig { max_recursion_depth: 0, ..EvaluationConfig::default() };
assert!(bad.validate().is_err());
```

Enforced limits:

- `max_recursion_depth`: 1--1000 (prevents stack overflow)
- `max_string_length`: 1--1,048,576 bytes (prevents memory exhaustion)
- `timeout_ms`: if set, 1--300,000 ms (prevents denial of service)
- High recursion (>100) combined with large strings (>65,536) is rejected

## EvaluationResult

```rust,ignore
pub struct EvaluationResult {
    pub description: String,          // Human-readable file type (or "data" if unknown)
    pub mime_type: Option<String>,    // MIME type (only when enable_mime_types is true)
    pub confidence: f64,              // 0.0 to 1.0, based on match depth
    pub matches: Vec<RuleMatch>,      // Individual rule matches with offset/level/value
    pub metadata: EvaluationMetadata, // Diagnostics (timing, file size, etc.)
}
```

### Working with Results

```rust,no_run
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig {
    enable_mime_types: true,
    stop_at_first_match: false,
    ..EvaluationConfig::default()
};
let db = MagicDatabase::with_builtin_rules_and_config(config)?;
let result = db.evaluate_file("photo.jpg")?;

println!("Description: {}", result.description);
if let Some(ref mime) = result.mime_type {
    println!("MIME type: {}", mime);
}
println!("Confidence: {:.1}%", result.confidence * 100.0);
for m in &result.matches {
    println!("  offset={}, level={}, message={}", m.offset, m.level, m.message);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## EvaluationMetadata

```rust,ignore
pub struct EvaluationMetadata {
    pub file_size: u64,              // Size of evaluated file/buffer in bytes
    pub evaluation_time_ms: f64,     // Wall-clock evaluation time
    pub rules_evaluated: usize,      // Number of top-level rules in the database
    pub magic_file: Option<PathBuf>, // Source path, or None for built-in rules
    pub timed_out: bool,             // Whether evaluation hit the timeout
}
```

```rust,no_run
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig { timeout_ms: Some(2000), ..EvaluationConfig::default() };
let db = MagicDatabase::with_builtin_rules_and_config(config)?;
let result = db.evaluate_buffer(b"\x89PNG\r\n\x1a\n")?;

let meta = &result.metadata;
println!("Size: {} bytes, Time: {:.3} ms, Timed out: {}",
         meta.file_size, meta.evaluation_time_ms, meta.timed_out);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## MIME Type Mapping

MIME type detection is opt-in via `enable_mime_types`. When enabled, descriptions are mapped to standard MIME types using an internal lookup table.

```rust,no_run
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig { enable_mime_types: true, ..EvaluationConfig::default() };
let db = MagicDatabase::with_builtin_rules_and_config(config)?;
let result = db.evaluate_buffer(b"%PDF-1.4")?;

if let Some(mime) = &result.mime_type {
    println!("MIME: {}", mime); // e.g., "application/pdf"
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

When `enable_mime_types` is `false` (the default), `mime_type` is always `None`.

## Error Handling

All fallible operations return `Result<T, LibmagicError>` (aliased as `libmagic_rs::Result<T>`).

### LibmagicError Variants

| Variant                            | When it occurs                                             |
| ---------------------------------- | ---------------------------------------------------------- |
| `ParseError(ParseError)`           | Invalid magic file syntax during loading                   |
| `EvaluationError(EvaluationError)` | Rule evaluation failure (buffer overrun, unsupported type) |
| `IoError(std::io::Error)`          | File system errors (not found, permission denied)          |
| `Timeout { timeout_ms }`           | Evaluation exceeded the configured timeout                 |
| `ConfigError { reason }`           | Invalid configuration values                               |
| `FileError(String)`                | Structured file I/O error with path and operation context  |

### Matching on Errors

```rust,no_run
use libmagic_rs::{MagicDatabase, LibmagicError};

let db = MagicDatabase::with_builtin_rules()?;
match db.evaluate_file("missing.bin") {
    Ok(result) => println!("Type: {}", result.description),
    Err(LibmagicError::IoError(e)) => eprintln!("File error: {}", e),
    Err(LibmagicError::Timeout { timeout_ms }) => {
        eprintln!("Timed out after {} ms", timeout_ms);
    }
    Err(LibmagicError::EvaluationError(e)) => eprintln!("Evaluation failed: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Validating Configuration Early

```rust
use libmagic_rs::{EvaluationConfig, LibmagicError};

let config = EvaluationConfig { max_recursion_depth: 5000, ..EvaluationConfig::default() };
match config.validate() {
    Ok(()) => println!("Config is valid"),
    Err(LibmagicError::ConfigError { reason }) => eprintln!("Bad config: {}", reason),
    Err(e) => eprintln!("Unexpected: {}", e),
}
```

## Reading from Standard Input

Use `evaluate_buffer` to process data piped through stdin:

```rust,no_run
use libmagic_rs::MagicDatabase;
use std::io::Read;

let db = MagicDatabase::with_builtin_rules()?;
let mut buffer = Vec::new();
std::io::stdin().read_to_end(&mut buffer)?;
println!("{}", db.evaluate_buffer(&buffer)?.description);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Evaluating Multiple Files

A single `MagicDatabase` can evaluate any number of files. Rules are parsed once and reused:

```rust,no_run
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;
for path in &["image.png", "archive.tar.gz", "binary.elf"] {
    match db.evaluate_file(path) {
        Ok(result) => println!("{}: {}", path, result.description),
        Err(e) => eprintln!("{}: error: {}", path, e),
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```
