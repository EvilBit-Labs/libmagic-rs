# libmagic-rs Documentation

Welcome to the libmagic-rs documentation. This is a pure-Rust implementation of libmagic for safe, efficient file type identification.

## Primary Documentation

The main documentation is available as an **mdbook** in `docs/src/`. To build and view:

```bash
# Install mdbook if needed
cargo install mdbook

# Build and serve the documentation
cd docs
mdbook serve --open
```

Or build static HTML:

```bash
cd docs
mdbook build
# Output in docs/book/
```

## Quick Reference Documents

For quick access without building the mdbook:

| Document | Description |
|----------|-------------|
| [Getting Started](GETTING_STARTED.md) | Quick start guide and tutorials |
| [API Reference](API_REFERENCE.md) | Complete library API documentation |
| [CLI Reference](CLI_REFERENCE.md) | Command-line tool documentation |
| [Architecture Guide](ARCHITECTURE.md) | System design and internals |
| [Magic File Format](MAGIC_FORMAT.md) | Guide to writing magic rules |

## Architecture Diagrams

Mermaid diagrams are available in `docs/diagrams/`:

| Diagram | Description |
|---------|-------------|
| [architecture.mmd](diagrams/architecture.mmd) | System architecture |
| [evaluation-flow.mmd](diagrams/evaluation-flow.mmd) | Rule evaluation flowchart |
| [error-handling.mmd](diagrams/error-handling.mmd) | Error hierarchy |
| [module-structure.mmd](diagrams/module-structure.mmd) | Module dependencies |

Render with: `mmdc -i diagram.mmd -o diagram.svg`

---

## Quick Links

### Installation

```bash
# Add to Cargo.toml
[dependencies]
libmagic-rs = "0.1"

# Install CLI
cargo install libmagic-rs
```

### Basic Library Usage

```rust
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;
let result = db.evaluate_file("sample.bin")?;
println!("Type: {}", result.description);
```

### Basic CLI Usage

```bash
# Identify a file
rmagic --use-builtin document.pdf

# JSON output
rmagic --json --use-builtin image.png

# Multiple files
rmagic --use-builtin *.bin
```

---

## Feature Overview

### Core Features

- **Pure Rust**: Memory-safe implementation with no unsafe code
- **Built-in Rules**: Pre-compiled rules for common file types
- **Custom Rules**: Support for standard magic file format
- **Multiple Formats**: Text and JSON output
- **Stdin Support**: Read from pipes and redirects

### Supported File Types (Built-in)

| Category | Formats |
|----------|---------|
| Executables | ELF, PE/DOS (MZ) |
| Archives | ZIP, TAR, GZIP |
| Images | JPEG, PNG, GIF, BMP |
| Documents | PDF |

### Security Features

- Configurable timeouts
- Recursion depth limits
- String length limits
- Bounds-checked buffer access

---

## Documentation Versions

This documentation is for **libmagic-rs v0.1.0**.

For the latest documentation, visit:
- [docs.rs/libmagic-rs](https://docs.rs/libmagic-rs) - API documentation
- [GitHub](https://github.com/EvilBit-Labs/libmagic-rs) - Source and issues

---

## Contributing

Found an issue with the documentation? Please report it on [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues).

---

## License

libmagic-rs is licensed under the Apache-2.0 license.
