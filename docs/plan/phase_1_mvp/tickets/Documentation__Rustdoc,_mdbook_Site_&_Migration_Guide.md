# Documentation: Rustdoc, mdbook Site & Migration Guide

## Overview

Create comprehensive documentation for Phase 1 MVP including rustdoc for all public APIs, mdbook documentation site with guides and examples, and migration information for users coming from C libmagic. This ensures the MVP is "ready for early adopters" per Epic Brief success criteria.

## Scope

**In Scope:**

- Rustdoc for all public APIs with examples
- mdbook documentation site structure
- Getting started guide
- Library API guide with usage patterns
- CLI reference documentation
- Migration guide from C libmagic
- Architecture overview
- Examples for common use cases

**Out of Scope:**

- Advanced tutorials (deferred to Phase 2)
- Video tutorials
- API reference beyond rustdoc
- Performance tuning guide (deferred to Phase 3)

## Technical Approach

### 1. Rustdoc Coverage

Ensure all public APIs have comprehensive rustdoc:

**file:src/lib.rs:**

- `MagicDatabase` struct and all methods
- `EvaluationConfig` struct and presets
- `EvaluationResult` and `EvaluationMetadata` structs
- `MatchResult` struct
- All error types

**file:src/parser/mod.rs:**

- `load_magic_file()` function
- Public parsing functions

**file:src/evaluator/mod.rs:**

- Public evaluation functions

**Example Rustdoc Pattern:**

```rust
/// Load magic rules from a file or directory
///
/// This function automatically detects the format (text file, directory, or binary .mgc)
/// and loads the magic rules accordingly. Binary .mgc files are not supported in Phase 1
/// and will return an error with suggestions for alternatives.
///
/// # Arguments
///
/// * `path` - Path to magic file or Magdir directory
///
/// # Errors
///
/// Returns `LibmagicError::IoError` if the file/directory cannot be read.
/// Returns `LibmagicError::ParseError` if the magic file format is invalid.
/// Returns `LibmagicError::UnsupportedFormat` if a binary .mgc file is encountered.
///
/// # Examples
///
/// ```rust
/// use libmagic_rs::MagicDatabase;
///
/// // Load from text file
/// let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;
///
/// // Load from Magdir directory
/// let db = MagicDatabase::load_from_file("/usr/share/file/magic/Magdir")?;
///
/// // Use built-in rules
/// let db = MagicDatabase::with_builtin_rules();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self>
```

### 2. mdbook Site Structure

Create `file:docs/book/` with mdbook configuration:

**file:docs/book/book.toml:**

```toml
[book]
title = "libmagic-rs Documentation"
authors = ["libmagic-rs Contributors"]
language = "en"
multilingual = false
src = "src"

[build]
build-dir = "book"

[output.html]
default-theme = "light"
git-repository-url = "https://github.com/yourusername/libmagic-rs"
```

**Documentation Structure:**

```
docs/book/src/
├── SUMMARY.md
├── introduction.md
├── getting-started.md
├── library-api.md
├── cli-reference.md
├── migration-guide.md
├── architecture.md
└── examples/
    ├── basic-usage.md
    ├── advanced-config.md
    └── custom-rules.md
```

### 3. Getting Started Guide

**file:docs/book/src/getting-started.md:**

Content should cover:

- Installation instructions
- Quick start with CLI
- Quick start with library API
- Common use cases
- Troubleshooting

### 4. Library API Guide

**file:docs/book/src/library-api.md:**

Content should cover:

- Simple API usage (`load_from_file()`)
- Builder pattern for advanced configuration
- Evaluating files vs buffers
- Error handling patterns
- Configuration options explained
- Best practices

### 5. CLI Reference

**file:docs/book/src/cli-reference.md:**

Content should cover:

- All CLI flags and arguments
- Output formats (text vs JSON)
- Magic file discovery
- Built-in rules usage
- Exit codes
- Examples for common scenarios

### 6. Migration Guide

**file:docs/book/src/migration-guide.md:**

Content should cover:

- Differences from C libmagic
- API mapping (C functions → Rust methods)
- Configuration migration
- Error handling differences
- Performance considerations
- Known limitations in Phase 1

### 7. Architecture Overview

**file:docs/book/src/architecture.md:**

Content should cover:

- OpenBSD-inspired approach
- Parser-evaluator pipeline
- Module organization
- Data structures
- Design decisions and rationale

### 8. Examples

**file:docs/book/src/examples/basic-usage.md:**

- Simple file type detection
- Batch processing
- Stdin processing

**file:docs/book/src/examples/advanced-config.md:**

- Custom configuration
- Timeout handling
- MIME type mapping

**file:docs/book/src/examples/custom-rules.md:**

- Writing magic rules
- Testing custom rules
- Debugging rules

## Acceptance Criteria

### Rustdoc

- [ ] All public APIs have rustdoc comments
- [ ] All rustdoc includes examples
- [ ] All rustdoc examples compile and run
- [ ] `cargo doc` builds without warnings
- [ ] `cargo test --doc` passes all doc tests

### mdbook Site

- [ ] mdbook site builds successfully
- [ ] All pages render correctly
- [ ] Navigation structure is clear
- [ ] Code examples are syntax-highlighted
- [ ] Links between pages work
- [ ] External links are valid

### Content Quality

- [ ] Getting started guide covers installation and first use
- [ ] Library API guide covers all major use cases
- [ ] CLI reference documents all flags
- [ ] Migration guide addresses C libmagic users
- [ ] Architecture overview explains design decisions
- [ ] Examples are practical and tested

### Integration

- [ ] Documentation builds in CI/CD
- [ ] Documentation published to GitHub Pages or similar
- [ ] README links to documentation
- [ ] Cargo.toml includes documentation link

## Dependencies

- Depends on: All implementation tickets (documents the complete implementation)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/c554e409-ae60-407f-9596-64c5b03a9b92 (Parser Integration)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/bb48e54f-995b-448e-add8-fe1546bfa9d9 (CLI Enhancements)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/5fa9fb36-a0bf-4d1b-924a-41a0a39f126e (Built-in Rules)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/33ad363d-27da-40ad-94bb-e1e24bd5cb39 (Evaluation Enhancements)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/fae78198-06dc-4d77-ab4a-1cc1f7e117a4 (Strength Calculation)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/b0addca4-6d49-407e-89c8-a8262b4219d8 (Test Infrastructure)

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/3ce0475b-153d-487f-bc0d-47d0a8f6708a (Epic Brief - Documentation Requirements)

## Files to Create

- `file:docs/book/book.toml` - mdbook configuration
- `file:docs/book/src/SUMMARY.md` - Table of contents
- `file:docs/book/src/introduction.md` - Introduction
- `file:docs/book/src/getting-started.md` - Getting started guide
- `file:docs/book/src/library-api.md` - Library API guide
- `file:docs/book/src/cli-reference.md` - CLI reference
- `file:docs/book/src/migration-guide.md` - Migration guide
- `file:docs/book/src/architecture.md` - Architecture overview
- `file:docs/book/src/examples/*.md` - Example guides

## Files to Modify

- All `file:src/**/*.rs` files - Add/enhance rustdoc comments
- `file:README.md` - Add documentation links
- `file:Cargo.toml` - Add documentation metadata
- `file:.github/workflows/docs.yml` - Add mdbook build and deployment
