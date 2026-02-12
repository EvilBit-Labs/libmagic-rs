---
id: libmagic-rs-strict-clippy
trigger: "when writing or modifying Rust code in this project"
confidence: 0.95
domain: rust-linting
source: local-repo-analysis
---

# Strict Clippy Compliance

## Action

This project enforces extremely strict clippy settings. When writing code:

- Never use `.unwrap()` - use `?`, `.ok()`, or match instead (clippy `unwrap_used = "deny"`)
- Never use `panic!()` in library code (clippy `panic = "deny"`)
- Never use `unsafe` code (workspace `unsafe_code = "forbid"`)
- Never use direct indexing on slices/buffers - use `.get()` (clippy `indexing_slicing = "warn"`)
- Never use `&str[n..]` - use `strip_prefix()`/`strip_suffix()` (clippy `string_slice = "warn"`)
- Mark all constructors/getters with `#[must_use]`
- Avoid `as` casts where possible (clippy `as_conversions = "warn"`)
- No `dbg!()` macros (clippy `dbg_macro = "warn"`)
- No `todo!()` macros (clippy `todo = "warn"`)

## Evidence

- Cargo.toml contains 80+ clippy lint configurations
- `unsafe_code = "forbid"` and `unwrap_used = "deny"` are workspace-level
- All 34 analyzed commits maintain zero-warning compliance
- CI enforces `cargo clippy -- -D warnings`
