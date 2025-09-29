# Code Style

libmagic-rs follows strict code style guidelines to ensure consistency, readability, and maintainability across the codebase.

## Formatting

### Rustfmt Configuration

The project uses `rustfmt` with default settings. All code must be formatted before committing:

```bash
# Format all code
cargo fmt

# Check formatting without changing files
cargo fmt -- --check
```

### Key Formatting Rules

- **Line length**: 100 characters (rustfmt default)
- **Indentation**: 4 spaces (no tabs)
- **Trailing commas**: Required in multi-line constructs
- **Import organization**: Automatic grouping and sorting

```rust
// Good: Proper formatting
use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::parser::ast::MagicRule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub description: String,
    pub mime_type: Option<String>,
    pub confidence: f64,
}
```

## Naming Conventions

### Types and Structs

Use `PascalCase` for types, structs, enums, and traits:

```rust
// Good
pub struct MagicDatabase {}
pub enum OffsetSpec {}
pub trait BinaryRegex {}

// Bad
pub struct magic_database {}
pub enum offset_spec {}
```

### Functions and Variables

Use `snake_case` for functions, methods, and variables:

```rust
// Good
pub fn parse_magic_file(path: &Path) -> Result<Vec<MagicRule>> { }
let magic_rules = vec![];
let file_buffer = FileBuffer::new(path)?;

// Bad
pub fn ParseMagicFile(path: &Path) -> Result<Vec<MagicRule>> { }
let magicRules = vec![];
```

### Constants

Use `SCREAMING_SNAKE_CASE` for constants:

```rust
// Good
const DEFAULT_BUFFER_SIZE: usize = 8192;
const MAX_RECURSION_DEPTH: u32 = 50;

// Bad
const default_buffer_size: usize = 8192;
const maxRecursionDepth: u32 = 50;
```

### Modules

Use `snake_case` for module names:

```rust
// Good
mod file_evaluator;
mod magic_parser;
mod output_formatter;

// Bad
mod MagicParser;
mod fileEvaluator;
```

## Documentation Standards

### Public API Documentation

All public items must have rustdoc comments with examples:

````rust
/// Parses a magic file into a vector of magic rules
///
/// This function reads a magic file from the specified path and parses it into
/// a collection of `MagicRule` structures that can be used for file type detection.
///
/// # Arguments
///
/// * `path` - Path to the magic file to parse
///
/// # Returns
///
/// Returns `Ok(Vec<MagicRule>)` on success, or `Err(LibmagicError)` if parsing fails.
///
/// # Errors
///
/// This function will return an error if:
/// - The file cannot be read due to permissions or missing file
/// - The magic file contains invalid syntax
/// - Memory allocation fails during parsing
///
/// # Examples
///
/// ```rust,no_run
/// use libmagic_rs::parser::parse_magic_file;
///
/// let rules = parse_magic_file("magic.db")?;
/// println!("Loaded {} magic rules", rules.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_magic_file<P: AsRef<Path>>(path: P) -> Result<Vec<MagicRule>> {
    // Implementation
}
````

### Module Documentation

Each module should have comprehensive documentation:

````rust
//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.
//!
//! The parser uses nom combinators for robust, efficient parsing with good
//! error reporting. It supports the standard magic file format with extensions
//! for modern file types.
//!
//! # Examples
//!
//! ```rust,no_run
//! use libmagic_rs::parser::parse_magic_file;
//!
//! let rules = parse_magic_file("magic.db")?;
//! for rule in &rules {
//!     println!("Rule: {}", rule.message);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
````

### Inline Comments

Use inline comments sparingly, focusing on *why* rather than *what*:

```rust
// Good: Explains reasoning
// Use indirect offset to handle relocatable executables
let actual_offset = resolve_indirect_offset(base_offset, buffer)?;

// Bad: States the obvious
// Set the offset to the resolved value
let actual_offset = resolved_offset;
```

## Error Handling Style

### Use Result Types

Always use `Result` for fallible operations:

```rust
// Good
pub fn parse_offset(input: &str) -> Result<OffsetSpec> {
    // Implementation that can fail
}

// Bad: Using Option for errors
pub fn parse_offset(input: &str) -> Option<OffsetSpec> {
    // Loses error information
}

// Bad: Using panics
pub fn parse_offset(input: &str) -> OffsetSpec {
    // Implementation that panics on error
    input.parse().unwrap()
}
```

### Descriptive Error Messages

Provide context in error messages:

```rust
// Good: Specific, actionable error
return Err(LibmagicError::ParseError {
    line: line_number,
    message: format!("Invalid offset '{}': expected number or hex value", input),
});

// Bad: Generic error
return Err(LibmagicError::ParseError {
    line: line_number,
    message: "parse error".to_string(),
});
```

### Error Propagation

Use the `?` operator for error propagation:

```rust
// Good
pub fn load_and_parse(path: &Path) -> Result<Vec<MagicRule>> {
    let content = std::fs::read_to_string(path)?;
    let rules = parse_magic_string(&content)?;
    Ok(rules)
}

// Avoid: Manual error handling when ? works
pub fn load_and_parse(path: &Path) -> Result<Vec<MagicRule>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => return Err(LibmagicError::IoError(e)),
    };
    // ...
}
```

## Code Organization

### Import Organization

Group imports in this order:

1. Standard library
2. External crates
3. Internal crates/modules

```rust
// Standard library
use std::collections::HashMap;
use std::path::Path;

// External crates
use nom::{bytes::complete::tag, IResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Internal modules
use crate::evaluator::EvaluationContext;
use crate::parser::ast::{MagicRule, OffsetSpec};
```

### Function Organization

Organize functions logically within modules:

```rust
impl MagicRule {
    // Constructors first
    pub fn new(/* ... */) -> Self {}

    // Public methods
    pub fn evaluate(&self, buffer: &[u8]) -> Result<bool> {}
    pub fn message(&self) -> &str {}

    // Private helpers last
    fn validate_offset(&self) -> bool {}
}
```

### File Organization

Keep files focused and reasonably sized (< 500-600 lines):

```rust
// Good: Focused module
// src/parser/offset.rs - Only offset parsing logic

// Bad: Everything in one file
// src/parser/mod.rs - All parsing logic (thousands of lines)
```

## Testing Style

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Group related tests
    mod offset_parsing {
        use super::*;

        #[test]
        fn test_absolute_offset() {
            // Test implementation
        }

        #[test]
        fn test_indirect_offset() {
            // Test implementation
        }
    }

    mod error_handling {
        use super::*;

        #[test]
        fn test_invalid_syntax_error() {
            // Test implementation
        }
    }
}
```

### Test Naming

Use descriptive test names that explain the scenario:

```rust
// Good: Descriptive names
#[test]
fn test_parse_absolute_offset_with_hex_value() {}

#[test]
fn test_parse_offset_returns_error_for_invalid_syntax() {}

// Bad: Generic names
#[test]
fn test_parse_offset() {}

#[test]
fn test_error() {}
```

### Assertion Style

Use specific assertions with helpful messages:

```rust
// Good: Specific assertion with context
assert_eq!(
    result.unwrap().message,
    "ELF executable",
    "Magic rule should identify ELF files correctly"
);

// Good: Pattern matching for complex types
match result {
    Ok(OffsetSpec::Absolute(offset)) => assert_eq!(offset, 42),
    _ => panic!("Expected absolute offset with value 42"),
}

// Avoid: Generic assertions
assert!(result.is_ok());
```

## Performance Considerations

### Prefer Borrowing

Use references instead of owned values when possible:

```rust
// Good: Borrowing
pub fn evaluate_rule(rule: &MagicRule, buffer: &[u8]) -> Result<bool> {}

// Avoid: Unnecessary ownership
pub fn evaluate_rule(rule: MagicRule, buffer: Vec<u8>) -> Result<bool> {}
```

### Avoid Unnecessary Allocations

```rust
// Good: String slice
pub fn parse_message(input: &str) -> &str {
    input.trim()
}

// Avoid: Unnecessary allocation
pub fn parse_message(input: &str) -> String {
    input.trim().to_string()
}
```

### Use Appropriate Data Structures

```rust
// Good: Vec for ordered data
let rules: Vec<MagicRule> = parse_rules(input)?;

// Good: HashMap for key-value lookups
let mime_types: HashMap<String, String> = load_mime_mappings()?;

// Consider: BTreeMap for sorted keys
let sorted_rules: BTreeMap<u32, MagicRule> = rules_by_priority();
```

This style guide ensures consistent, readable, and maintainable code across the libmagic-rs project. All contributors should follow these guidelines, and automated tools enforce many of these rules during CI.
