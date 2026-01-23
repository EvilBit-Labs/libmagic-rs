# Contributing to libmagic-rs

Thank you for your interest in contributing to libmagic-rs! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Architecture](#project-architecture)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Documentation](#documentation)
- [Submitting Changes](#submitting-changes)
- [Style Guidelines](#style-guidelines)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and constructive in all interactions.

## Getting Started

### Prerequisites

- **Rust 1.85+** (edition 2024)
- **Cargo** (comes with Rust)
- **Git** for version control

### Quick Start

```bash
# Clone the repository
git clone https://github.com/EvilBit-Labs/libmagic-rs.git
cd libmagic-rs

# Build the project
cargo build

# Run tests
cargo test

# Run the CLI
cargo run -- --help
```

## Development Setup

### Recommended Tools

- **rust-analyzer**: IDE support for Rust
- **cargo-watch**: Auto-rebuild on file changes (`cargo install cargo-watch`)
- **mdbook**: Documentation building (`cargo install mdbook`)

### Building Documentation

```bash
# Build and serve the mdbook documentation
cd docs
mdbook serve --open

# Generate rustdoc
cargo doc --open
```

## Project Architecture

libmagic-rs follows a **parser-evaluator architecture**:

```text
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Magic File  │───▶│   Parser    │───▶│     AST     │───▶│  Evaluator  │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

### Module Overview

| Module | Purpose | Status |
|--------|---------|--------|
| `parser/` | Magic file parsing with nom | In Development |
| `parser/ast.rs` | AST data structures | Complete |
| `parser/grammar.rs` | Parsing combinators | Partial |
| `evaluator/` | Rule evaluation engine | Planned |
| `io/` | Memory-mapped file I/O | Complete |
| `output/` | Result formatting | Planned |

See [Architecture Documentation](docs/src/architecture.md) for detailed information.

## Making Changes

### Branching Strategy

1. Create a feature branch from `main`:

   ```bash
   git checkout -b feat/your-feature-name
   ```

2. Use conventional commit prefixes:
   - `feat:` - New features
   - `fix:` - Bug fixes
   - `docs:` - Documentation changes
   - `refactor:` - Code refactoring
   - `test:` - Test additions/changes
   - `chore:` - Maintenance tasks

### Code Quality Requirements

Before submitting changes, ensure:

1. **All tests pass**: `cargo test`
2. **No clippy warnings**: `cargo clippy -- -D warnings`
3. **Code is formatted**: `cargo fmt`
4. **Documentation builds**: `cargo doc --no-deps`

### Safety Requirements

This project **forbids unsafe code**. The following lint is enforced:

```rust
#![forbid(unsafe_code)]
```

If you believe unsafe code is necessary, open an issue for discussion first.

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run tests for a specific module
cargo test parser::
```

### Writing Tests

- Place unit tests in the same file as the code being tested
- Use `#[cfg(test)]` modules for test code
- Include doc tests for public API examples
- Test both success and error cases

Example test structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_success() {
        // Arrange
        let input = "test input";

        // Act
        let result = function_under_test(input);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_feature_error() {
        let result = function_under_test("");
        assert!(result.is_err());
    }
}
```

## Documentation

### Types of Documentation

1. **Rustdoc**: API documentation in source code
2. **mdbook**: User guide and architecture docs in `docs/`
3. **README.md**: Project overview and quick start

### Rustdoc Guidelines

- Document all public items
- Include examples in doc comments
- Use `# Examples` sections with runnable code
- Add `# Errors` sections for fallible functions
- Use `# Panics` sections if applicable

Example:

```rust
/// Parses a magic rule from the input string.
///
/// # Arguments
///
/// * `input` - The magic rule text to parse
///
/// # Returns
///
/// Returns the parsed `MagicRule` and remaining input on success.
///
/// # Errors
///
/// Returns `ParseError` if the input is malformed.
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::parse_rule;
///
/// let input = "0 string hello Hello file";
/// let result = parse_rule(input);
/// assert!(result.is_ok());
/// ```
pub fn parse_rule(input: &str) -> Result<MagicRule, ParseError> {
    // implementation
}
```

### mdbook Guidelines

- Keep explanations clear and beginner-friendly
- Include diagrams where helpful (Mermaid is supported)
- Update `SUMMARY.md` when adding new pages
- Test all code examples

## Submitting Changes

### Pull Request Process

1. **Update documentation** for any API changes
2. **Add tests** for new functionality
3. **Run the full test suite** locally
4. **Create a pull request** with a clear description
5. **Address review feedback** promptly

### PR Description Template

```markdown
## Summary
Brief description of changes

## Changes
- Change 1
- Change 2

## Testing
How were these changes tested?

## Checklist
- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Documentation updated
```

## Style Guidelines

### Rust Style

This project uses `rustfmt` with the following configuration:

```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Max"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

Run `cargo fmt` before committing.

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types | PascalCase | `MagicRule`, `ParseError` |
| Functions | snake_case | `parse_rule`, `read_bytes` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_BUFFER_SIZE` |
| Modules | snake_case | `parser`, `evaluator` |

### Error Handling

- Use `Result<T, E>` for fallible operations
- Create specific error types with `thiserror`
- Provide context in error messages
- Avoid `unwrap()` in library code

```rust
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid offset at position {position}: {message}")]
    InvalidOffset { position: usize, message: String },

    #[error("unexpected end of input")]
    UnexpectedEof,
}
```

## Getting Help

- **Issues**: For bug reports and feature requests
- **Discussions**: For questions and ideas
- **Documentation**: Check [docs/](docs/) for detailed guides

Thank you for contributing to libmagic-rs!
