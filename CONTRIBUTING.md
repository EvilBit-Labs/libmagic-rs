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
- [Project Governance](#project-governance)

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

| Module             | Purpose                                           | Status      |
| ------------------ | ------------------------------------------------- | ----------- |
| `parser/`          | Magic file parsing with nom                       | Implemented |
| `parser/ast.rs`    | AST data structures                               | Implemented |
| `parser/grammar/`  | Parsing combinators                               | Implemented |
| `evaluator/`       | Rule evaluation engine                            | Implemented |
| `io/`              | Memory-mapped file I/O                            | Implemented |
| `output/`          | Result formatting (text and JSON)                 | Implemented |
| `mime.rs`          | MIME type lookup table                            | Implemented |
| `tags.rs`          | Tag inference from rule descriptions              | Implemented |
| `builtin_rules.rs` | Built-in magic rules generated from `.magic` file | Implemented |

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

````rust
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
````

### mdbook Guidelines

- Keep explanations clear and beginner-friendly
- Include diagrams where helpful (Mermaid is supported)
- Update `SUMMARY.md` when adding new pages
- Test all code examples

## Submitting Changes

### Pull Request Process

1. **Update documentation** for any API changes
2. **Add tests** for new functionality
3. **Run the full test suite** locally: `just ci-check`
4. **Create a pull request** with a clear description
5. **Keep your PR up to date**: PRs must be within 3 commits of `main` to merge. Rebase onto `main` if your branch has fallen behind.
6. **Address review feedback** promptly

### Code Review Requirements

All pull requests require review before merging. Reviewers check for:

- **Correctness**: Does the code do what it claims? Are edge cases handled?
- **Safety**: No unsafe code, proper bounds checking, no panics in library code
- **Tests**: New functionality has tests, existing tests still pass
- **Style**: Follows project conventions, passes `cargo fmt` and `cargo clippy -- -D warnings`
- **Documentation**: Public APIs have rustdoc with examples, AGENTS.md updated if architecture changes
- **Performance**: No unnecessary allocations in hot paths, no regressions in benchmarks

All CI checks must pass before merge, including quality checks, tests, coverage, and cross-platform tests (ubuntu-latest, ubuntu-22.04, macos-latest, windows-latest). Mergify enforces merge protections that require the quality check and cross-platform tests for ubuntu-latest, macos-latest, and windows-latest to pass. Bot PRs are auto-merged when conditions are met; human PRs are merged manually by maintainers once CI is green and review is complete.

### Developer Certificate of Origin (DCO)

This project requires all contributors to sign off on their commits, certifying that they have the right to submit the code under the project's license. This is enforced by the [DCO GitHub App](https://github.com/apps/dco).

To sign off, add `-s` to your commit command:

```bash
git commit -s -m "feat: add new feature"
```

This adds a `Signed-off-by` line to your commit message:

```text
Signed-off-by: Your Name <your.email@example.com>
```

By signing off, you agree to the [Developer Certificate of Origin](https://developercertificate.org/).

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
- [ ] Commits signed off (`git commit -s`)
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

| Item      | Convention           | Example                    |
| --------- | -------------------- | -------------------------- |
| Types     | PascalCase           | `MagicRule`, `ParseError`  |
| Functions | snake_case           | `parse_rule`, `read_bytes` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_BUFFER_SIZE`          |
| Modules   | snake_case           | `parser`, `evaluator`      |

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

## Project Governance

### Decision-Making

libmagic-rs uses a **maintainer-driven** governance model. Decisions are made by the project maintainers through consensus on GitHub issues and pull requests.

### Roles

| Role            | Responsibilities                                                           | Current                                                                                        |
| --------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| **Maintainer**  | Merge PRs, manage releases, set project direction, review security reports | [@unclesp1d3r](https://github.com/unclesp1d3r), [@KryptoKat08](https://github.com/KryptoKat08) |
| **Contributor** | Submit issues, PRs, and participate in discussions                         | Anyone following this guide                                                                    |

### How Decisions Are Made

- **Bug fixes and minor changes**: Any maintainer can review and merge
- **New features**: Discussed in a GitHub issue before implementation; maintainer approval required
- **Architecture changes**: Require agreement from both maintainers
- **Breaking API changes**: Discussed in a GitHub issue with community input; require agreement from both maintainers
- **Releases**: Prepared by any maintainer, following the [release process](docs/src/release-process.md)

### Becoming a Maintainer

As the project grows, active contributors who demonstrate sustained, high-quality contributions and alignment with project goals may be invited to become maintainers.

## Known Gotchas

Before diving into the codebase, read **[GOTCHAS.md](GOTCHAS.md)** -- it documents non-obvious behaviors, common pitfalls, and architectural quirks that will save you debugging time. Key topics include:

- Build script boundary constraints and shared code between `build.rs` and the library (S1)
- Enum variant update checklists for `TypeKind`, `Operator`, and `Value` (S2)
- Parser architecture split between `types.rs` and `grammar/mod.rs` (S3)
- Numeric type conversion and checked arithmetic requirements (S5)
- PString multi-byte length prefix endianness (S6)
- Clippy lint surprises and `unsafe_code = "forbid"` enforcement (S8)

## Getting Help

- **Issues**: For bug reports and feature requests
- **Discussions**: For questions and ideas
- **Documentation**: Check [docs/](docs/) for detailed guides
- **Gotchas**: Check [GOTCHAS.md](GOTCHAS.md) for known pitfalls

Thank you for contributing to libmagic-rs!
