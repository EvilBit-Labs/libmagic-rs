# Product Overview

## libmagic-rs

A pure-Rust implementation of libmagic, the library that powers the `file` command for identifying file types.

### Core Purpose

- Replace libmagic with a safe, efficient Rust alternative
- Parse magic files (DSL for byte-level file type detection)
- Evaluate magic rules against file buffers to identify file types
- Provide both human-readable and structured JSON output

### Key Features

- **Memory Safety**: Pure Rust with no unsafe code (except vetted crates)
- **Performance**: Uses mmap for efficient file reading
- **Compatibility**: Supports common magic file syntax (offsets, types, operators, nesting)
- **Extensibility**: Designed for modern use cases (PE resources, Mach-O, Go build info)
- **Multiple Output Formats**: Classic text output and structured JSON

### Target Use Cases

- File type identification in security tools
- Content analysis pipelines
- Modern applications requiring safe file type detection
- Drop-in replacement for existing libmagic usage

### Success Metrics

- Performance parity with libmagic
- Compatibility with existing magic file formats
- Zero memory safety vulnerabilities
- Extensible architecture for future file formats
