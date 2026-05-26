---
name: warn-unsafe-code
enabled: true
event: file
conditions:
  - field: new_text
    operator: regex_match
    pattern: unsafe\s*\{|unsafe\s+fn|unsafe\s+impl
---

**Unsafe code detected.**

This project enforces `#![forbid(unsafe_code)]` project-wide. No unsafe blocks, functions, or impls are permitted in project source code.

If you believe unsafe is absolutely necessary, stop and discuss with the user before proceeding.
