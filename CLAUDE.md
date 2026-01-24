# libmagic-rs - Claude Code Context

Pure-Rust implementation of libmagic for file type identification. See @AGENTS.md for detailed guidelines.

## Quick Reference

### Build & Test

- `cargo build` / `cargo build --release` - Build project
- `cargo test` or `cargo nextest run` - Run tests (650+ tests)
- `cargo clippy -- -D warnings` - Lint (zero warnings policy enforced)
- `cargo fmt` - Format code
- `cargo llvm-cov --html` - Coverage report (target >85%)

### Project Structure

- `src/parser/` - Magic file DSL parsing (nom-based)
- `src/evaluator/` - Rule evaluation engine
- `src/output/` - Text and JSON formatters
- `src/io/` - Memory-mapped file I/O
- Binary: `rmagic` (src/main.rs)

### Code Standards

- **No unsafe code** - `unsafe_code = "forbid"` in Cargo.toml
- **No unwrap/panic** - Use proper error handling with `thiserror`
- **No emojis** in code, comments, or documentation
- Keep files under 500-600 lines
- Rust 2024 edition with rustfmt 2024 style

### Tooling (via mise)

- `mise install` - Install all dev tools
- `cargo nextest run` - Faster test runner
- `cargo insta` - Snapshot testing
- `cargo audit` / `cargo deny` - Security checks

### Current Branch Focus

CLI enhancements: multiple file inputs, stdin processing, magic file discovery
