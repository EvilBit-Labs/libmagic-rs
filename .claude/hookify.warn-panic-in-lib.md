---
name: warn-panic-in-lib
enabled: true
event: file
conditions:
  - field: file_path
    operator: regex_match
    pattern: src/.*\.rs$
  - field: new_text
    operator: regex_match
    pattern: \.unwrap\(\)|\.expect\(|panic!\(
---

**Potential panic in library code detected.**

This project uses `unwrap_used = "deny"` and `panic = "deny"` in clippy config. Use `Result<T, E>` patterns instead.

```rust
// Wrong: panics at runtime
let val = something.unwrap();

// Correct: propagate error
let val = something.ok_or(MagicError::InvalidValue)?;
```

Note: `.unwrap()` is acceptable inside `#[cfg(test)]` modules.
