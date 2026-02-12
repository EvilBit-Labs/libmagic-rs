---
id: libmagic-rs-error-handling
trigger: "when adding new error types or handling errors"
confidence: 0.9
domain: rust-errors
source: local-repo-analysis
---

# Error Handling with thiserror + Constructor Methods

## Action

When creating or extending error types:

1. Use `thiserror::Error` derive macro
2. Include contextual fields (line numbers, offsets, names)
3. Add named constructor methods for each variant
4. Mark constructors with `#[must_use]`
5. Use `impl Into<String>` for string parameters
6. Write unit tests that verify Display output

```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("Something failed at {location}: {reason}")]
    SomethingFailed { location: usize, reason: String },
}

impl MyError {
    #[must_use]
    pub fn something_failed(location: usize, reason: impl Into<String>) -> Self {
        Self::SomethingFailed { location, reason: reason.into() }
    }
}
```

## Evidence

- `src/error.rs` defines 3 error enums with 20+ variants, all following this pattern
- Every variant has a corresponding constructor method
- All constructors use `impl Into<String>` and `#[must_use]`
- Comprehensive tests verify Display formatting for each variant
