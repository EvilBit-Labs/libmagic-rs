# Library API

> [!NOTE]
> The library API is currently in early development with placeholder functionality. This documentation describes the planned interface.

The libmagic-rs library provides a safe, efficient API for file type identification in Rust applications.

## Core Types

### MagicDatabase

The main interface for loading and using magic rules:

```rust
pub struct MagicDatabase {
    // Internal implementation
}

impl MagicDatabase {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self>;
    pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult>;
}
```

### EvaluationResult

Contains the results of file type identification:

```rust
pub struct EvaluationResult {
    pub description: String,
    pub mime_type: Option<String>,
    pub confidence: f64,
}
```

### EvaluationConfig

Configuration options for rule evaluation:

```rust
pub struct EvaluationConfig {
    pub max_recursion_depth: u32,
    pub max_string_length: usize,
    pub stop_at_first_match: bool,
}
```

## Basic Usage

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

// Load magic database
let db = MagicDatabase::load_from_file("magic.db")?;

// Evaluate a file
let result = db.evaluate_file("example.bin")?;

println!("File type: {}", result.description);
if let Some(mime) = result.mime_type {
    println!("MIME type: {}", mime);
}
```

## Error Handling

All operations return `Result` types with descriptive errors:

```rust
use libmagic_rs::LibmagicError;

match db.evaluate_file("missing.bin") {
    Ok(result) => println!("Type: {}", result.description),
    Err(LibmagicError::IoError(e)) => eprintln!("File error: {}", e),
    Err(LibmagicError::EvaluationError(e)) => eprintln!("Evaluation error: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Advanced Usage

Coming soon with full implementation.
