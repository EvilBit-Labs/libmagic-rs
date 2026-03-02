# libmagic-rs

![GitHub License](https://img.shields.io/github/license/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github)
![GitHub Sponsors](https://img.shields.io/github/sponsors/EvilBit-Labs?style=flat-square&logo=github)

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/EvilBit-Labs/libmagic-rs/ci.yml?style=flat-square)
![docs.rs](https://img.shields.io/docsrs/libmagic-rs?style=flat-square)
![Deps.rs Repository Dependencies](https://img.shields.io/deps-rs/repo/github/EvilBit-Labs/libmagic-rs?style=flat-square)

![Codecov](https://img.shields.io/codecov/c/github/EvilBit-Labs/libmagic-rs?style=flat-square&logoColor=white&logo=codecov)
[![GitHub issues](https://img.shields.io/github/issues/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github)](https://github.com/EvilBit-Labs/libmagic-rs/issues)
![GitHub last commit](https://img.shields.io/github/last-commit/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github)

[![Crates.io](https://img.shields.io/crates/v/libmagic-rs?style=flat-square&logo=rust)](https://crates.io/crates/libmagic-rs)
![GitHub Release Date](https://img.shields.io/github/release-date/EvilBit-Labs/libmagic-rs?display_date=published_at&style=flat-square&logo=github&link=https%3A%2F%2Fgithub.com%2FEvilBit-Labs%2Flibmagic-rs%2Freleases)
![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/libmagic-rs?style=flat-square&logo=rust)
![Crates.io MSRV](https://img.shields.io/crates/msrv/libmagic-rs?style=flat-square&logo=rust)

---
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/EvilBit-Labs/libmagic-rs/badge?style=flat-square)](https://scorecard.dev/viewer/?uri=github.com/EvilBit-Labs/libmagic-rs)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/11947/badge?style=flat-square)](https://www.bestpractices.dev/projects/11947)

---

A pure-Rust implementation of libmagic, the library that powers the `file` command for identifying file types. This project provides a memory-safe, efficient alternative to the C-based libmagic library.

> [!NOTE]
> This is a clean-room implementation inspired by the original [libmagic](https://www.darwinsys.com/file/) project. We respect and acknowledge the original work by Ian Darwin and the current maintainers led by Christos Zoulas.

## Project Status

**v0.1.1** -- The core file identification pipeline is functional. Common file types can be identified using text magic files today.

- 1,068 tests with >94% line coverage
- Zero unsafe code (`unsafe_code = "forbid"` enforced project-wide)
- Zero warnings with strict clippy linting
- Published on [crates.io](https://crates.io/crates/libmagic-rs)

## Features

- Parse and evaluate text magic files (the stable, documented format)
- Identify files via CLI (`rmagic`) or as a library dependency
- Text and JSON output formats
- Built-in fallback rules for 10 common formats (ELF, PE, ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF)
- Custom magic files via `--magic-file`
- Memory-mapped I/O with bounds checking
- Hierarchical rule evaluation with confidence scoring
- Stdin support (`rmagic -`)

### Supported Magic File Syntax

| Category | Supported |
|----------|-----------|
| **Types** | `byte`, `short`, `long`, `string` (with endianness and sign options) |
| **Operators** | `=`, `!=`, `&` (bitwise AND with optional mask) |
| **Offsets** | Absolute, from-end (indirect and relative are parsed but not yet evaluated) |
| **Directives** | `!:strength` (parsed; `!:mime`, `!:ext`, `!:apple` planned) |

## Quick Start

### Installation

```bash
cargo install libmagic-rs
```

### CLI Usage

```bash
# Basic file identification
rmagic file.bin

# JSON output
rmagic file.bin --json

# Use built-in rules (no external magic file needed)
rmagic --use-builtin file.bin

# Custom magic file
rmagic --magic-file custom.magic file.bin

# Multiple files
rmagic file1.bin file2.bin file3.bin

# Read from stdin
cat file.bin | rmagic -
```

### Library Usage

```rust
use libmagic_rs::MagicDatabase;

// Load magic rules from a text magic file
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;

// Identify file type
let result = db.evaluate_file("example.bin")?;
println!("File type: {}", result.description);
println!("Confidence: {:.0}%", result.confidence * 100.0);

// Or evaluate an in-memory buffer
let buffer = std::fs::read("example.bin")?;
let result = db.evaluate_buffer(&buffer)?;
if let Some(mime) = result.mime_type {
    println!("MIME type: {}", mime);
}

// Or use built-in rules (no external files needed)
let db = MagicDatabase::with_builtin_rules();
let result = db.evaluate_file("example.bin")?;
```

## Architecture

```text
Magic File --> Parser --> AST --> Evaluator --> Match Results --> Output Formatter
     |
Target File --> Memory Mapper --> File Buffer
```

| Module | Purpose |
|--------|---------|
| `parser/` | Magic file DSL parsing into AST (nom-based) |
| `evaluator/` | Rule evaluation with offset resolution, type interpretation, operator matching |
| `output/` | Text (GNU `file` compatible) and JSON formatting |
| `io/` | Memory-mapped file buffers with safe bounds checking |

### Key Types

```rust
pub struct MagicRule {
    pub offset: OffsetSpec,     // Where to look in the file
    pub typ: TypeKind,          // How to interpret the bytes
    pub op: Operator,           // How to compare
    pub value: Value,           // What to compare against
    pub message: String,        // Output on match
    pub children: Vec<MagicRule>, // Nested sub-rules
    pub level: u32,             // Nesting depth
    pub strength_modifier: Option<StrengthModifier>,
}

pub enum TypeKind {
    Byte,
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
}

pub enum OffsetSpec {
    Absolute(i64),
    FromEnd(i64),
    Indirect { base_offset, pointer_type, adjustment, endian },
    Relative(i64),
}
```

## Compatibility

libmagic-rs follows the **OpenBSD approach**: parse text magic files directly, prioritizing simplicity and correctness. Text magic files are stable across libmagic versions and work unchanged from system installations (`/usr/share/misc/magic`).

Compatibility is validated against the [original file project](https://github.com/file/file) test suite.

## Security

- **Memory Safety**: `unsafe_code = "forbid"` enforced project-wide
- **Bounds Checking**: All buffer access protected
- **Resource Limits**: Configurable recursion depth, string length, and per-file timeout
- **Fuzzing**: Robustness testing with malformed inputs

### Verifying Releases

All release artifacts are signed via [Sigstore](https://www.sigstore.dev/) using GitHub Attestations:

```bash
gh attestation verify <artifact> --repo EvilBit-Labs/libmagic-rs
```

See the [release verification guide](https://evilbitlabs.io/libmagic-rs/release-verification.html) for details.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full roadmap, or [GitHub Milestones](https://github.com/EvilBit-Labs/libmagic-rs/milestones) for issue tracking.

| Milestone | Focus |
|-----------|-------|
| **v0.1.x** (current) | MVP: parser, evaluator, CLI, built-in rules, 94%+ test coverage |
| **v0.2.0** | Comparison operators, bitwise XOR/NOT, indirect/relative offsets, 64-bit integers |
| **v0.3.0** | Regex, float/double, date/timestamp, pascal strings, meta-types |
| **v0.4.0** | Builder API, JSON metadata, parse warnings, improved errors |
| **v1.0.0** | 95%+ GNU `file` compatibility, stable API |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding guidelines, and submission process.

## License

Licensed under the Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Support

- [Documentation](https://evilbitlabs.io/libmagic-rs/)
- [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)

## Acknowledgments

- [Ian Darwin](https://www.darwinsys.com/file/) for the original file command and libmagic
- [Christos Zoulas](https://www.darwinsys.com/file/) and the current libmagic maintainers
- The Rust community for excellent tooling and ecosystem
