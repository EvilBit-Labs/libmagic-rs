# Development Setup

This guide covers setting up a development environment for contributing to libmagic-rs, including tools, workflows, and best practices.

## Prerequisites

### Required Tools

- **Rust 1.85+** with the 2021 edition
- **Git** for version control
- **Cargo** (included with Rust)

### Recommended Tools

```bash
# Enhanced test runner
cargo install cargo-nextest

# Auto-rebuild on file changes
cargo install cargo-watch

# Code coverage
cargo install cargo-llvm-cov

# Security auditing
cargo install cargo-audit

# Dependency analysis
cargo install cargo-tree

# Documentation tools
cargo install mdbook  # For this documentation
```

## Environment Setup

### 1. Clone the Repository

```bash
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
cd libmagic-rs
```

### 2. Verify Setup

```bash
# Check Rust version
rustc --version  # Should be 1.85+

# Verify project builds
cargo check

# Run tests
cargo test

# Check linting passes
cargo clippy -- -D warnings
```

### 3. IDE Configuration

#### VS Code

Recommended extensions:

- `rust-analyzer`: Rust language server
- `CodeLLDB`: Debugging support
- `Better TOML`: TOML syntax highlighting
- `Error Lens`: Inline error display

Settings (`.vscode/settings.json`):

```json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": [
    "--",
    "-D",
    "warnings"
  ],
  "rust-analyzer.cargo.features": "all"
}
```

#### Other IDEs

- **IntelliJ IDEA**: Use the Rust plugin
- **Vim/Neovim**: Configure with rust-analyzer LSP
- **Emacs**: Use rustic-mode with lsp-mode

## Development Workflow

### Daily Development

```bash
# Start development session
cargo watch -x check -x test

# In another terminal, make changes and see results automatically
```

### Code Quality Checks

```bash
# Format code (required before commits)
cargo fmt

# Check for issues (must pass)
cargo clippy -- -D warnings

# Run all tests
cargo nextest run  # or cargo test

# Check documentation
cargo doc --document-private-items
```

### Testing Strategy

```bash
# Run specific test modules
cargo test ast_structures
cargo test parser
cargo test evaluator

# Run tests with output
cargo test -- --nocapture

# Run ignored tests (if any)
cargo test -- --ignored

# Test documentation examples
cargo test --doc
```

## Project Standards

### Code Style

The project enforces strict code quality standards:

#### Linting Configuration

See `Cargo.toml` for the complete linting setup. Key rules:

- **No unsafe code**: `unsafe_code = "forbid"`
- **Zero warnings**: `warnings = "deny"`
- **Comprehensive clippy**: Pedantic, nursery, and security lints enabled
- **No unwrap/panic**: `unwrap_used = "deny"`, `panic = "deny"`

#### Formatting

```bash
# Format all code (required)
cargo fmt

# Check formatting without changing files
cargo fmt -- --check
```

### Documentation Standards

#### Code Documentation

All public APIs must have rustdoc comments:

````rust
/// Parses a magic file into an AST
///
/// This function reads a magic file from the given path and parses it into
/// a vector of `MagicRule` structures that can be used for file type detection.
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
/// - The file cannot be read
/// - The magic file syntax is invalid
/// - Memory allocation fails
///
/// # Examples
///
/// ```rust,no_run
/// use libmagic_rs::parser::parse_magic_file;
///
/// let rules = parse_magic_file("magic.db")?;
/// println!("Loaded {} rules", rules.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_magic_file<P: AsRef<Path>>(path: P) -> Result<Vec<MagicRule>> {
    // Implementation
}
````

#### Module Documentation

Each module should have comprehensive documentation:

````rust
//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.
//!
//! # Magic File Format
//!
//! Magic files use a simple DSL to describe file type detection rules:
//!
//! ```text
//! # ELF files
//! 0    string    \x7fELF    ELF
//! >4   byte      1          32-bit
//! >4   byte      2          64-bit
//! ```
//!
//! # Examples
//!
//! ```rust,no_run
//! use libmagic_rs::parser::parse_magic_file;
//!
//! let rules = parse_magic_file("magic.db")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
````

### Testing Standards

#### Unit Tests

Every module should have comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Test basic case
        let result = function_under_test();
        assert_eq!(result, expected_value);
    }

    #[test]
    fn test_error_conditions() {
        // Test error handling
        let result = function_that_should_fail();
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary conditions
        // Empty inputs, maximum values, etc.
    }
}
```

#### Integration Tests

Place integration tests in the `tests/` directory:

```rust
// tests/integration_test.rs
use libmagic_rs::*;

#[test]
fn test_end_to_end_workflow() {
    // Test complete workflows
    let db = MagicDatabase::load_from_file("test_files/magic/simple.magic").unwrap();
    let result = db.evaluate_file("test_files/samples/elf64").unwrap();
    assert_eq!(result.description, "ELF 64-bit LSB executable");
}
```

### Error Handling

Use the project's error types consistently:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Processing failed: {reason}")]
    ProcessingFailed { reason: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ModuleError>;
```

## Contribution Workflow

### 1. Issue Creation

Before starting work:

- Check existing issues and discussions
- Create an issue describing the problem or feature
- Wait for maintainer feedback on approach

### 2. Branch Creation

```bash
# Create feature branch
git checkout -b feature/descriptive-name

# Or for bug fixes
git checkout -b fix/issue-description
```

### 3. Development Process

```bash
# Make changes following the standards above
# Run checks frequently
cargo watch -x check -x test

# Before committing
cargo fmt
cargo clippy -- -D warnings
cargo test
```

### 4. Commit Guidelines

Use conventional commit format:

```bash
# Feature commits
git commit -m "feat(parser): add support for indirect offsets"

# Bug fixes
git commit -m "fix(evaluator): handle buffer overflow in string reading"

# Documentation
git commit -m "docs(api): add examples for MagicRule creation"

# Tests
git commit -m "test(ast): add comprehensive serialization tests"
```

### 5. Pull Request Process

1. **Push branch**: `git push origin feature/descriptive-name`
2. **Create PR** with:
   - Clear description of changes
   - Reference to related issues
   - Test coverage information
   - Breaking change notes (if any)
3. **Address feedback** from code review
4. **Ensure CI passes** all checks

## Debugging

### Logging

Use the `log` crate for debugging:

```rust
use log::{debug, error, info, warn};

pub fn parse_rule(input: &str) -> Result<MagicRule> {
    debug!("Parsing rule: {}", input);

    let result = do_parsing(input)?;

    info!("Successfully parsed rule: {}", result.message);
    Ok(result)
}
```

Run with logging:

```bash
RUST_LOG=debug cargo test
RUST_LOG=libmagic_rs=trace cargo run
```

### Debugging Tests

```bash
# Run single test with output
cargo test test_name -- --nocapture

# Debug with lldb/gdb
cargo test --no-run
lldb target/debug/deps/libmagic_rs-<hash>
```

### Performance Profiling

```bash
# Install profiling tools
cargo install cargo-flamegraph

# Profile specific benchmarks
cargo flamegraph --bench evaluation_bench

# Memory profiling with valgrind
cargo build
valgrind --tool=massif target/debug/rmagic large_file.bin
```

## Continuous Integration

The project uses GitHub Actions for CI. Local checks should match CI:

```bash
# Run the same checks as CI
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
cargo doc --document-private-items
```

## Release Process

For maintainers:

### Version Bumping

```bash
# Update version in Cargo.toml
# Update CHANGELOG.md
# Commit changes
git commit -m "chore: bump version to 0.2.0"
git tag v0.2.0
git push origin main --tags
```

### Documentation Updates

```bash
# Update documentation
mdbook build docs/
# Deploy to GitHub Pages (automated)
```

This development setup ensures high code quality, comprehensive testing, and smooth collaboration across the project.
