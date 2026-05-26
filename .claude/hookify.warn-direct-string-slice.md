---
name: warn-direct-string-slice
enabled: true
event: file
conditions:
  - field: new_text
    operator: regex_match
    pattern: "&\\w+\\[\\d+\\.\\.\\]|&\\w+\\[\\.\\.\\d+\\]|&\\w+\\[\\d+\\.\\.]"
---

**Direct string slicing detected.**

Use `strip_prefix()` / `strip_suffix()` instead of `&str[n..]` to avoid UTF-8 boundary panics.

```rust
// Wrong: can panic on non-ASCII
let rest = &input[2..];

// Correct: safe
let rest = input.strip_prefix("0x").unwrap_or(input);
```
