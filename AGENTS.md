# AI Assistant Guidelines for libmagic-rs

This document provides comprehensive guidelines for AI assistants working on the libmagic-rs project, ensuring consistent, high-quality development practices and project understanding.

## Project Overview

**libmagic-rs** is a pure-Rust implementation of libmagic, designed to replace the C-based library with a memory-safe, efficient alternative for file type detection.

### Core Mission

- **Memory Safety**: Pure Rust implementation with no unsafe code (except vetted dependencies)
- **Performance**: Memory-mapped I/O with zero-copy operations where possible
- **Compatibility**: Support for common libmagic syntax patterns
- **Extensibility**: AST-based design for easy addition of new rule types

## Development Principles

### 1. Memory Safety First

- **No unsafe code** except in vetted dependencies (memmap2, byteorder, etc.)
- **Bounds checking** for all buffer access using `.get()` methods
- **Safe resource management** with RAII patterns
- **Graceful error handling** for malformed inputs
- **Safe string operations**: Use `strip_prefix()`/`strip_suffix()` instead of direct slicing (`&str[n..]`) to avoid UTF-8 panics

### 2. Zero-Warnings Policy

- All code must pass `cargo clippy -- -D warnings` with no exceptions
- Preserve all `deny` attributes and `-D warnings` flags
- Fix clippy suggestions unless they conflict with project requirements
- Use `cargo fmt` for consistent code formatting

### 3. Performance Critical

- Use memory-mapped I/O (`memmap2`) for efficient file access
- Implement zero-copy operations where possible
- Use Aho-Corasick indexing for multi-pattern string searches (planned)
- Cache compiled magic rules for performance (planned)
- Profile with `cargo bench` for performance regressions

### 4. Testing Required

- Target >85% test coverage with `cargo llvm-cov`
- All code changes must include comprehensive tests
- Use `cargo nextest` for faster, more reliable test execution
- Include property tests with `proptest` for fuzzing
- Benchmark critical path components with `criterion`
- Verify doc examples with `cargo test --doc` - ensure example strings don't accidentally match multiple patterns

## Architecture Patterns

### Parser-Evaluator Design

The project follows a clear separation of concerns:

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Module Organization

```rust
// Core data structures in lib.rs
pub struct MagicRule { /* ... */ }
pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
}
pub enum Operator {
    Equal, NotEqual, BitwiseAnd, BitwiseAndMask(u64),
}
// Additional types and operators are planned -- see Current Limitations below

// Parser module structure
parser/
├── mod.rs      // Public parser interface
├── ast.rs      // AST node definitions
└── grammar.rs  // Magic file DSL parsing (nom/pest)

// Evaluator module structure
evaluator/
├── mod.rs       // Main evaluation engine
├── offset.rs    // Offset resolution (absolute, indirect, relative)
├── types.rs     // Type interpretation with endianness
└── operators.rs // Equality and bitwise operations
```

## Code Quality Standards

### File Size Limits

- Keep source files under 500-600 lines
- Split larger files into focused modules
- Use clear, descriptive module names

### Emoji Usage

- Avoid using emojis and other non-ASCII characters in code, comments, or documentation, except when the code is handling non-plaintext characters (for example: em dash, en dash, or other non-ASCII symbols).

### Case-Insensitive Matching Pattern

When implementing case-insensitive string matching:

- Lowercase inputs at ALL entry points (constructors, setters)
- Store normalized values internally
- Document the case-insensitivity in public API docs

### Error Handling Patterns

```rust
// Library errors should be descriptive and actionable
#[derive(Debug, thiserror::Error)]
pub enum MagicError {
    #[error("Failed to parse magic file at line {line}: {reason}")]
    ParseError { line: usize, reason: String },

    #[error("IO error reading file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid offset specification: {offset}")]
    InvalidOffset { offset: String },
}

// Use Result types consistently
pub fn evaluate_magic_rules(
    rules: &[MagicRule],
    data: &[u8],
) -> Result<Option<String>, MagicError> {
    // Implementation
}
```

### Architecture Constraints

- `src/error.rs` is shared with `build.rs` -- cannot reference lib-only types like `crate::io::IoError`
- `FileError(String)` wraps structured I/O errors as strings to work around the build.rs constraint
- Use `ParseError::IoError` for I/O errors in parser code, not `ParseError::invalid_syntax`
- Use `LibmagicError::ConfigError` for config validation, not `ParseError::invalid_syntax`
- Clippy pedantic lints are active (e.g., prefer `trailing_zeros()` over bitwise masks)
- All public enum variants need `# Examples` rustdoc sections

### Naming Conventions

- **Files**: snake_case (e.g., `magic_rule.rs`)
- **Types**: PascalCase (e.g., `MagicRule`, `TypeKind`)
- **Functions**: snake_case (e.g., `resolve_offset`, `evaluate_rule`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_BUFFER_SIZE`)
- **Modules**: snake_case (e.g., `evaluator`, `output`)

## Development Workflow

### Standard Commands

All commands should be run via `mise exec --` to use the project's pinned Rust toolchain.

```bash
# Development cycle
cargo check        # Fast syntax/type checking
cargo build        # Build project
cargo test         # Run all tests
cargo clippy       # Linting with strict warnings
cargo fmt          # Format code
just ci-check      # Run complete CI suite locally (pre-commit validation)

# Performance and quality
cargo bench        # Run benchmarks
cargo doc          # Generate documentation
cargo test --doc   # Test documentation examples
```

### Testing Strategy

- **Unit Tests**: Alongside source files with `#[cfg(test)]`
- **Integration Tests**: In `tests/` directory with real magic files
- **Compatibility Tests**: Complete test suite from [original file project](https://raw.githubusercontent.com/file/file/refs/heads/master/tests/README)
- **Property Tests**: Use `proptest` for fuzzing magic rule evaluation
- **Benchmarks**: Critical path performance tests with `criterion`
- **Coverage**: Target >85% with `cargo llvm-cov`

## Magic File Compatibility

### Currently Implemented (v0.1.0)

- **Offsets**: Absolute and from-end specifications (indirect and relative are parsed but not yet evaluated)
- **Types**: `byte`, `short`, `long`, `string` with endianness support
- **Operators**: `=` (equal), `!=` (not equal), `&` (bitwise AND with optional mask)
- **Nested Rules**: Hierarchical rule evaluation with proper indentation
- **String Matching**: Exact string matching with null-termination

### Planned Features (v1.0+)

- Comparison operators: `>`, `<`, `>=`, `<=`
- Bitwise XOR operator: `^`
- Regex type: Pattern matching with binary-safe regex support
- Additional types: 64-bit integers, floats, doubles, dates
- Search type: Multi-pattern string searching

### Future Enhancement: Binary-Safe Regex Handling

> **Note:** The following is planned for future releases and is not yet implemented.

```rust
// Use regex crate with bytes feature for binary-safe matching
pub trait BinaryRegex {
    fn find_at(&self, haystack: &[u8], start: usize) -> Option<Match>;
}

impl BinaryRegex for regex::bytes::Regex {
    /* ... */
}
```

## Current Limitations (v0.1.0)

### Type System

- No regex/search pattern matching
- No 64-bit integer types (quad, qquad)
- No floating-point types (float, double, befloat, lefloat)
- No date/time types (date, qdate, ldate, qldate)
- String evaluation reads until first NUL or end-of-buffer by default; `max_length: Some(_)` is supported internally but no dedicated fixed-length string parser syntax exists yet

### Operators

- No comparison operators (`>`, `<`, `>=`, `<=`)
- No XOR operator (`^`)
- No negation operator (`~`)
- BitwiseAnd supports mask values but not all libmagic mask syntax

### Offset Specifications

- Indirect offsets are parsed into the AST but evaluation is not yet implemented (#37)
- Relative offsets are parsed into the AST but evaluation is not yet implemented (#38)
- Only absolute and from-end offsets are fully functional

### Magic File Syntax

- Limited support for special directives (only `!:strength` is parsed)
- No support for `!:mime`, `!:ext`, `!:apple` directives in evaluation
- No support for named tests or use/name directives

See issue #52 for the planned enhancement roadmap.

## Performance Requirements

### Critical Optimizations

- **Memory Mapping**: Use `mmap` to avoid loading entire files into memory
- **Zero-Copy**: Minimize allocations during rule evaluation
- **Aho-Corasick**: Use for multi-pattern string searches when beneficial
- **Rule Caching**: Cache compiled magic rules for repeated use
- **Early Exit**: Stop evaluation as soon as a definitive match is found

### Benchmarking

```rust
// Example benchmark structure
#[bench]
fn bench_magic_evaluation(b: &mut Bencher) {
    let rules = load_magic_rules("tests/fixtures/standard.magic");
    let file_data = include_bytes!("../tests/fixtures/sample.bin");

    b.iter(|| evaluate_rules(&rules, file_data));
}
```

## Output Formats

### Text Output (Default)

```text
sample.bin: ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

### JSON Output (Structured)

```json
{
  "filename": "sample.bin",
  "matches": [
    {
      "text": "ELF 64-bit LSB executable",
      "offset": 0,
      "value": "7f454c46",
      "tags": [
        "executable",
        "elf"
      ],
      "score": 90,
      "mime_type": "application/x-executable"
    }
  ],
  "metadata": {
    "file_size": 8192,
    "evaluation_time_ms": 2.3,
    "rules_evaluated": 45
  }
}
```

## Common Tasks and Patterns

### Adding New Type Support

> **Note:** Currently implemented types are `Byte`, `Short`, `Long`, and `String`. Regex and other advanced types are planned for future releases.

1. Extend `TypeKind` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar.rs`
3. Implement reading logic in `src/evaluator/types.rs`
4. Add tests for the new type
5. Update documentation

### Adding New Operators

> **Note:** Currently implemented operators are `Equal`, `NotEqual`, and `BitwiseAnd` (with `BitwiseAndMask`). Comparison operators (`>`, `<`) and XOR (`^`) are planned for future releases.

1. Extend `Operator` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar.rs`
3. Implement operator logic in `src/evaluator/operators.rs`
4. Add tests for the new operator
5. Update documentation

### Performance Optimization

1. Profile with `cargo bench` to identify bottlenecks
2. Use memory-mapped I/O for file access
3. Implement caching for compiled rules
4. Use Aho-Corasick for multi-pattern searches
5. Minimize allocations in hot paths

### Testing Build Scripts

Build scripts (`build.rs`) cannot import the crate being built, which makes them difficult to test. To enable comprehensive testing of build script logic:

1. Extract build logic into a library module with `#[cfg(any(test, doc))]`
2. Keep build.rs minimal, calling functions from the testable module
3. Write unit tests in the library module to verify all code paths
4. Example: `src/build_helpers.rs` provides testable parsing and code generation

This pattern ensures build-time failures (e.g., invalid magic files) are properly tested and produce clear error messages.

## Error Recovery Strategy

### Parse Errors

- Continue parsing after syntax errors
- Collect all errors for batch reporting
- Provide clear error messages with line numbers

### Evaluation Errors

- Graceful degradation
- Skip problematic rules and continue with others
- Maintain evaluation context for nested rules

### IO Errors

- Proper resource cleanup
- Clear error messages for file access issues
- Handle truncated and corrupted files safely

## Security Considerations

### Memory Safety

- No unsafe code except in vetted dependencies
- Bounds checking for all buffer access
- Safe handling of malformed input
- Fuzzing integration for robustness testing

### Input Validation

- Validate magic file syntax before parsing
- Check file size limits and resource usage
- Handle malicious or malformed input gracefully
- Implement timeouts for long-running evaluations

## Documentation Requirements

### API Documentation

- All public APIs require rustdoc with examples
- Include error conditions and recovery strategies
- Provide usage examples for common patterns
- Document performance characteristics

### Code Comments

- Explain complex algorithms and optimizations
- Document magic file syntax support
- Include references to libmagic compatibility
- Explain design decisions and trade-offs

## CI/CD Integration

### Automated Checks

The project uses GitHub Actions CI with Mergify merge queue:

1. **Formatting**: `cargo fmt` for consistent code style
2. **Linting**: `cargo clippy -- -D warnings` for best practices
3. **Compilation**: `cargo check` and `cargo build` for error detection
4. **Testing**: `cargo test` and `cargo nextest run` for validation
5. **Security**: `cargo audit` for vulnerability detection
6. **License Compliance**: Verify dependency licenses

### Quality Gates

- All code must pass clippy with `-D warnings`
- Test coverage must be >85%
- No compilation warnings or errors
- All tests must pass
- Security audit must pass
- Performance benchmarks must not regress

### Code Review Requirements

All pull requests require review before merging. Reviews are performed by maintainers and automated tools (CodeRabbit). Reviewers check for:

- **Correctness**: Does the code do what it claims? Are edge cases handled?
- **Memory safety**: No unsafe code blocks (except vetted dependencies). All buffer access must use bounds checking with `.get()` methods. No raw pointer arithmetic or transmute operations.
- **Error handling**: Proper use of `Result` types, no panics in library code, no `unwrap()` or `expect()` in library code. Use `thiserror` for structured error types.
- **Tests**: New functionality has tests, existing tests still pass, edge cases and error conditions are covered. Property tests with `proptest` for complex data structures.
- **Performance**: No unnecessary allocations in hot paths, no regressions in benchmarks. Memory-mapped I/O used for file access.
- **libmagic compatibility**: Changes maintain compatibility with libmagic behavior and magic file format. Output format matches GNU `file` command expectations.
- **Style**: Follows project conventions, passes `cargo fmt` and `cargo clippy -- -D warnings`
- **Documentation**: Public APIs have rustdoc with examples, AGENTS.md updated if architecture changes

CI must pass before merge. Mergify merge queue and merge protections enforce these checks.
PRs enter the merge queue when approved (or automatically for release-plz/dependabot).
Mergify rebases against main, runs CI, and squash-merges on success.

## Project Context

### Current Status

- **Phase**: Early development (MVP)
- **Focus**: Core parser and evaluator implementation
- **Priority**: Memory safety and basic functionality
- **Next Steps**: Enhanced features and performance optimization

### Key Dependencies

- `memmap2`: Memory-mapped file I/O
- `byteorder`: Endianness handling
- `nom`: Parser combinators
- `serde`: Serialization
- `clap`: CLI argument parsing
- `regex`: Pattern matching (used in tests; regex *type* for magic rules is planned)
- `aho-corasick`: Multi-pattern search (planned, not yet added)

### Development Phases

1. **MVP (v0.1.0)** - CURRENT: Basic parsing and evaluation with byte/short/long/string types, equality and bitwise AND operators, built-in rules for 10 common formats
2. **Enhanced Features (v0.2)**: Comparison operators (`>`, `<`), indirect offset improvements, strength-based rule ordering
3. **Advanced Types (v0.3)**: Regex type, 64-bit integers, floating-point types, search patterns
4. **Full Compatibility (v0.4)**: Complete libmagic syntax support, all special directives, named tests
5. **Production Ready (v1.0)**: Stable API, complete documentation, 95%+ compatibility with GNU file

## Best Practices

### Code Organization

- Keep modules focused and cohesive
- Use clear, descriptive names
- Minimize coupling between modules
- Maximize cohesion within modules

### Error Handling

- Use `Result<T, E>` patterns consistently
- Avoid panics in library code
- Provide actionable error messages
- Implement graceful degradation

### Testing

- Write tests alongside implementation
- Include edge cases and error conditions
- Use property-based testing for complex logic
- Benchmark performance-critical code

### Documentation

- Document public APIs thoroughly
- Include usage examples
- Explain design decisions
- Keep documentation up-to-date

## Troubleshooting

### Common Issues

- **Compilation errors**: Check for missing dependencies and syntax issues
- **Test failures**: Verify test logic and expected behavior
- **Performance issues**: Profile with `cargo bench` and optimize hot paths
- **Memory issues**: Check for bounds violations and resource leaks

### Debugging Tips

- Use `cargo test -- --nocapture` for test output
- Enable debug logging with `RUST_LOG=debug`
- Use `cargo clippy` to catch potential issues
- Profile with `cargo bench` for performance analysis

This guide ensures consistent, high-quality development practices for the libmagic-rs project while maintaining focus on memory safety, performance, and compatibility.

## Quick Reference

- Merging is managed by Mergify merge queue -- PRs are squash-merged after CI passes
- `.mergify.yml` configures merge queue rules, auto-queue, and merge protections
- `cargo deny check` uses `deny.toml` (default) -- do not specify a custom config path
- `.github/workflows/release.yml` is auto-generated by cargo-dist -- do not modify manually
- All `.rs` files must have copyright and SPDX headers (see any source file for format)
- `Cargo.lock` and `mise.lock` are committed for reproducible builds -- do not gitignore
- In justfile recipes, never wrap `just` in `{{ mise_exec }}` -- it's redundant
- Changelog: `just changelog`, `just changelog-version <tag>`, `just changelog-unreleased`
- Security contact: <support@evilbitlabs.io> (matches PGP key in SECURITY.md)

## Open Source Quality Standards (OSSF Best Practices)

This project has the OSSF Best Practices passing badge. Maintain these standards:

### Every PR must

- Sign off commits with `git commit -s` (DCO enforced by GitHub App)
- Pass CI (clippy, fmt, tests, CodeQL, cargo audit) before merge
- Include tests for new functionality -- this is policy, not optional
- Be reviewed (human or CodeRabbit) for correctness, safety, and style
- Not introduce `unsafe` code, `unwrap()`/`expect()` in library code, or panics

### Every release must

- Have human-readable release notes via git-cliff (not raw git log)
- Use unique SemVer identifiers (`vX.Y.Z` tags)
- Be built reproducibly (pinned toolchain, committed lock files, cargo-dist)

### Security

- Vulnerabilities go through private reporting (GitHub advisories or <support@evilbitlabs.io>), never public issues
- `cargo audit` and `cargo deny` run daily in CI -- fix findings promptly
- Medium+ severity vulnerabilities: we aim to release a fix within 90 days of confirmation (see SECURITY.md for canonical policy)
- `unsafe_code = "forbid"` is enforced project-wide via workspace lints in `Cargo.toml` -- this is a hardening mechanism, not a suggestion
- `docs/src/security-assurance.md` must be updated when new attack surface is introduced

### Documentation

- Public APIs require rustdoc with examples
- CONTRIBUTING.md documents code review criteria, test policy, DCO, and governance
- SECURITY.md documents vulnerability reporting with scope, safe harbor, and PGP key
- AGENTS.md must accurately reflect implemented features (not aspirational)
- `docs/src/release-verification.md` documents artifact signing for users
