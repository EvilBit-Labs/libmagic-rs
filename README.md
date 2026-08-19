# libmagic-rs

[![GitHub License][license-badge]][license-link] [![GitHub Sponsors][sponsors-badge]][sponsors-link]

[![GitHub Actions Workflow Status][ci-badge]][ci-link] [![docs.rs][docs-badge]][docs-link] [![Deps.rs Repository Dependencies][deps-badge]][deps-link]

[![Codecov][codecov-badge]][codecov-link] [![GitHub issues][issues-badge]][issues-link] [![GitHub last commit][last-commit-badge]][commits-link]

[![Crates.io][crates-badge]][crates-link] [![GitHub Release Date][release-date-badge]][releases-link] [![Crates.io Downloads (latest version)][downloads-badge]][crates-link] [![Crates.io MSRV][msrv-badge]][crates-link]

---

[![OpenSSF Scorecard][scorecard-badge]][scorecard-link] [![OpenSSF Best Practices][bestpractices-badge]][bestpractices-link]

---

A pure-Rust reimplementation of libmagic -- the library behind the `file` command. No `unsafe`, no C dependency.

> [!NOTE]
> Clean-room implementation. Original libmagic by Ian Darwin; current maintenance by Christos Zoulas -- see [darwinsys.com/file](https://www.darwinsys.com/file/).

## Project Status

**v0.5.0** -- usable for identifying common file types from a text magic file. Pre-1.0, expect API churn.

> [!WARNING]
> **Pre-1.0 API.** libmagic-rs is a pre-1.0 crate and the public API may change between minor versions until v1.0.0 is cut. Pin an exact version in `Cargo.toml` if you need reproducible builds, and read `CHANGELOG.md` before upgrading. See issue #52 for the v1.0 stability roadmap.

- 1,200+ tests with >94% line coverage
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

| Category       | Supported                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Types**      | `byte`, `short`, `long`, `quad`, `float`, `double`, `string`, `pstring` (with big/little-endian variants), unsigned variants (`ubyte`, `ushort`/`ubeshort`/`uleshort`, `ulong`/`ubelong`/`ulelong`, `uquad`/`ubequad`/`ulequad`), 32-bit dates (`date`/`ldate`/`bedate`/`beldate`/`ledate`/`leldate`), 64-bit dates (`qdate`/`qldate`/`beqdate`/`beqldate`/`leqdate`/`leqldate`), `regex`, and `search/N` |
| **Regex**      | Binary-safe via `regex::bytes::Regex`. Flags: `/c` (case-insensitive), `/s` (match-start anchor advance), `/l` (line-based scan window). Counts: `regex/N` (N bytes), `regex/Nl` (N lines). All variants capped at 8192 bytes (`FILE_REGEX_MAX`). Compile size is clamped to 1 MiB (`size_limit` + `dfa_size_limit`) to bound compile-time DoS exposure from adversarial patterns.                        |
| **Search**     | Bounded literal scan via `memchr::memmem::find`. `search/N` scans the first `N` bytes from the offset; the range is mandatory (`NonZeroUsize`). Match-end anchor advance for relative-offset children (matches GNU `file` semantics).                                                                                                                                                                     |
| **Operators**  | `=`, `!=`, `<`, `>`, `<=`, `>=`, `&` (bitwise AND with optional mask), `^` (bitwise XOR), `~` (bitwise NOT), `x` (any value)                                                                                                                                                                                                                                                                              |
| **Offsets**    | Absolute, from-end, indirect, and relative (all fully evaluated; magic-file `&+N`/`&-N` parsing for relative is pending)                                                                                                                                                                                                                                                                                  |
| **Directives** | `!:strength` (parsed; `!:mime`, `!:ext`, `!:apple` planned)                                                                                                                                                                                                                                                                                                                                               |

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

// Or load text magic rules from owned bytes without a reader-buffer copy
let rules = b"0 string CUSTOM Custom data\n".to_vec();
let db = MagicDatabase::load_from_bytes(rules)?;
let result = db.evaluate_buffer(b"CUSTOM payload")?;

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

| Module       | Purpose                                                                        |
| ------------ | ------------------------------------------------------------------------------ |
| `parser/`    | Magic file DSL parsing into AST (nom-based)                                    |
| `evaluator/` | Rule evaluation with offset resolution, type interpretation, operator matching |
| `output/`    | Text (GNU `file` compatible) and JSON formatting                               |
| `io/`        | Memory-mapped file buffers with safe bounds checking                           |

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
    Byte { signed: bool },
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    Quad { endian: Endianness, signed: bool },
    Float { endian: Endianness },
    Double { endian: Endianness },
    Date { endian: Endianness, utc: bool },
    QDate { endian: Endianness, utc: bool },
    String { max_length: Option<usize> },
    PString { max_length: Option<usize>, length_width: PStringLengthWidth, length_includes_itself: bool },
    Regex { flags: RegexFlags, count: RegexCount },
    Search { range: NonZeroUsize },
    // See src/parser/ast.rs for the authoritative definition.
}

pub enum OffsetSpec {
    Absolute(i64),
    FromEnd(i64),
    Indirect { base_offset, pointer_type, adjustment, endian },
    Relative(i64),
}
```

## Compatibility

libmagic-rs takes the OpenBSD route: parse text magic files directly. No compiled `.mgc` format, no binary cache. Text magic files are stable across libmagic versions, so a system `/usr/share/misc/magic` works unchanged.

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

See the [release verification guide](https://evilbit-labs.github.io/libmagic-rs/release-verification.html) for details.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full roadmap, or [GitHub Milestones](https://github.com/EvilBit-Labs/libmagic-rs/milestones) for issue tracking.

| Milestone            | Status    | Focus                                                                             |
| -------------------- | --------- | --------------------------------------------------------------------------------- |
| **v0.2.0**           | shipped   | Comparison operators, bitwise XOR/NOT, indirect/relative offsets, 64-bit integers |
| **v0.3.0**           | shipped   | Regex, float/double, date/timestamp, pascal strings, meta-types                   |
| **v0.4.0**           | shipped   | Evaluator submodule split, JSON metadata, parse warnings, improved errors         |
| **v0.5.x** (current) | in flight | TOCTOU/search-path hardening, regex compile cache, validated constructors         |
| **v0.6.0**           | planned   | `Value` pattern refactor, `MagicDatabase` builder, `Directive` extension point    |
| **v1.0.0**           | planned   | 95%+ GNU `file` compatibility, stable API, fuzzing harness, full non_exhaustive   |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding guidelines, and submission process.

## License

Licensed under the Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Support

- [Documentation](https://evilbit-labs.github.io/libmagic-rs/)
- [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)

## Acknowledgments

- [Ian Darwin](https://www.darwinsys.com/file/) for the original file command and libmagic
- [Christos Zoulas](https://www.darwinsys.com/file/) and the current libmagic maintainers
- The Rust community for excellent tooling and ecosystem

[bestpractices-badge]: https://www.bestpractices.dev/projects/11947/badge?style=flat-square
[bestpractices-link]: https://www.bestpractices.dev/projects/11947
[ci-badge]: https://img.shields.io/github/actions/workflow/status/EvilBit-Labs/libmagic-rs/ci.yml?style=flat-square
[ci-link]: https://github.com/EvilBit-Labs/libmagic-rs/actions/workflows/ci.yml
[codecov-badge]: https://img.shields.io/codecov/c/github/EvilBit-Labs/libmagic-rs?style=flat-square&logoColor=white&logo=codecov
[codecov-link]: https://app.codecov.io/gh/EvilBit-Labs/libmagic-rs
[commits-link]: https://github.com/EvilBit-Labs/libmagic-rs/commits/main
[crates-badge]: https://img.shields.io/crates/v/libmagic-rs?style=flat-square&logo=rust
[crates-link]: https://crates.io/crates/libmagic-rs
[deps-badge]: https://img.shields.io/deps-rs/repo/github/EvilBit-Labs/libmagic-rs?style=flat-square
[deps-link]: https://deps.rs/repo/github/EvilBit-Labs/libmagic-rs
[docs-badge]: https://img.shields.io/docsrs/libmagic-rs?style=flat-square
[docs-link]: https://docs.rs/libmagic-rs
[downloads-badge]: https://img.shields.io/crates/dv/libmagic-rs?style=flat-square&logo=rust
[issues-badge]: https://img.shields.io/github/issues/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github
[issues-link]: https://github.com/EvilBit-Labs/libmagic-rs/issues
[last-commit-badge]: https://img.shields.io/github/last-commit/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github
[license-badge]: https://img.shields.io/github/license/EvilBit-Labs/libmagic-rs?style=flat-square&logo=github
[license-link]: https://github.com/EvilBit-Labs/libmagic-rs/blob/main/LICENSE
[msrv-badge]: https://img.shields.io/crates/msrv/libmagic-rs?style=flat-square&logo=rust
[release-date-badge]: https://img.shields.io/github/release-date/EvilBit-Labs/libmagic-rs?display_date=published_at&style=flat-square&logo=github
[releases-link]: https://github.com/EvilBit-Labs/libmagic-rs/releases
[scorecard-badge]: https://api.scorecard.dev/projects/github.com/EvilBit-Labs/libmagic-rs/badge?style=flat-square
[scorecard-link]: https://scorecard.dev/viewer/?uri=github.com/EvilBit-Labs/libmagic-rs
[sponsors-badge]: https://img.shields.io/github/sponsors/EvilBit-Labs?style=flat-square&logo=github
[sponsors-link]: https://github.com/sponsors/EvilBit-Labs
