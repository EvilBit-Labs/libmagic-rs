# Error Handling

libmagic-rs uses Rust's `Result` type system for comprehensive, type-safe error handling.

## Error Types

### LibmagicError

The main error enum covers all library operations:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibmagicError {
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid magic file format: {0}")]
    InvalidFormat(String),
}
```

### Result Type Alias

For convenience, the library provides a type alias:

```rust
pub type Result<T> = std::result::Result<T, LibmagicError>;
```

## Error Handling Patterns

### Basic Error Handling

```rust
use libmagic_rs::{MagicDatabase, LibmagicError};

match MagicDatabase::load_from_file("magic.db") {
    Ok(db) => {
        // Use the database
        println!("Loaded magic database successfully");
    }
    Err(e) => {
        eprintln!("Failed to load magic database: {}", e);
        return;
    }
}
```

### Using the ? Operator

```rust
fn analyze_file(path: &str) -> Result<String> {
    let db = MagicDatabase::load_from_file("magic.db")?;
    let result = db.evaluate_file(path)?;
    Ok(result.description)
}
```

### Matching Specific Errors

```rust
use libmagic_rs::LibmagicError;

match db.evaluate_file("example.bin") {
    Ok(result) => println!("File type: {}", result.description),
    Err(LibmagicError::IoError(e)) => {
        eprintln!("File access error: {}", e);
    }
    Err(LibmagicError::EvaluationError(msg)) => {
        eprintln!("Evaluation failed: {}", msg);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Error Context

### Adding Context with `map_err`

```rust
use libmagic_rs::LibmagicError;

fn load_custom_magic(path: &str) -> Result<MagicDatabase> {
    MagicDatabase::load_from_file(path).map_err(|e| {
        LibmagicError::InvalidFormat(format!(
            "Failed to load custom magic file '{}': {}",
            path, e
        ))
    })
}
```

### Using `anyhow` for Application Errors

```rust
use anyhow::{Context, Result};
use libmagic_rs::MagicDatabase;

fn main() -> Result<()> {
    let db = MagicDatabase::load_from_file("magic.db").context("Failed to load magic database")?;

    let result = db
        .evaluate_file("example.bin")
        .context("Failed to analyze file")?;

    println!("File type: {}", result.description);
    Ok(())
}
```

## Error Recovery

### Graceful Degradation

```rust
fn analyze_with_fallback(path: &str) -> String {
    match MagicDatabase::load_from_file("magic.db") {
        Ok(db) => match db.evaluate_file(path) {
            Ok(result) => result.description,
            Err(_) => "unknown file type".to_string(),
        },
        Err(_) => "magic database unavailable".to_string(),
    }
}
```

### Retry Logic

```rust
use std::thread;
use std::time::Duration;

fn load_with_retry(path: &str, max_attempts: u32) -> Result<MagicDatabase> {
    let mut attempts = 0;

    loop {
        match MagicDatabase::load_from_file(path) {
            Ok(db) => return Ok(db),
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                eprintln!("Attempt {} failed: {}", attempts, e);
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Best Practices

### 1. Use Specific Error Types

```rust
// Good: Specific error information
Err(LibmagicError::ParseError {
    line: 42,
    message: "Invalid offset specification".to_string()
})

// Avoid: Generic error messages
Err(LibmagicError::EvaluationError("something went wrong".to_string()))
```

### 2. Provide Context

```rust
// Good: Contextual error information
fn parse_magic_file(path: &Path) -> Result<Vec<MagicRule>> {
    std::fs::read_to_string(path)
        .map_err(|e| LibmagicError::IoError(e))
        .and_then(|content| parse_magic_string(&content))
}

// Better: Even more context
fn parse_magic_file(path: &Path) -> Result<Vec<MagicRule>> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        LibmagicError::InvalidFormat(format!(
            "Cannot read magic file '{}': {}",
            path.display(),
            e
        ))
    })?;

    parse_magic_string(&content).map_err(|e| {
        LibmagicError::InvalidFormat(format!("Invalid magic file '{}': {}", path.display(), e))
    })
}
```

### 3. Handle Errors at the Right Level

```rust
// Library level: Return detailed errors
pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult> {
    // Detailed error handling
}

// Application level: Handle user-facing concerns
fn main() {
    match analyze_file("example.bin") {
        Ok(description) => println!("{}", description),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
```

### 4. Document Error Conditions

````rust
/// Evaluate magic rules against a file
///
/// # Errors
///
/// This function will return an error if:
/// - The file cannot be read (`IoError`)
/// - The file is too large for processing (`EvaluationError`)
/// - Rule evaluation encounters invalid data (`EvaluationError`)
///
/// # Examples
///
/// ```rust,no_run
/// use libmagic_rs::MagicDatabase;
///
/// let db = MagicDatabase::load_from_file("magic.db")?;
/// match db.evaluate_file("example.bin") {
///     Ok(result) => println!("Type: {}", result.description),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult> {
    // Implementation
}
````

## Testing Error Conditions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_file_error() {
        let result = MagicDatabase::load_from_file("nonexistent.magic");
        assert!(result.is_err());

        match result {
            Err(LibmagicError::IoError(_)) => (), // Expected
            _ => panic!("Expected IoError for missing file"),
        }
    }

    #[test]
    fn test_invalid_magic_file() {
        let result = parse_magic_string("invalid syntax here");
        assert!(result.is_err());

        if let Err(LibmagicError::ParseError { line, message }) = result {
            assert_eq!(line, 1);
            assert!(message.contains("syntax"));
        } else {
            panic!("Expected ParseError for invalid syntax");
        }
    }
}
```

This comprehensive error handling approach ensures libmagic-rs provides clear, actionable error information while maintaining type safety and enabling robust error recovery strategies.
