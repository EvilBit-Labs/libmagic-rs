# Introduction

Welcome to the **libmagic-rs** developer guide! This documentation provides comprehensive information about the pure-Rust implementation of libmagic, the library that powers the `file` command for identifying file types.

## What is libmagic-rs?

libmagic-rs is a clean-room implementation of the libmagic library, written entirely in Rust. It provides:

- **Memory Safety**: Pure Rust with no unsafe code (except vetted dependencies)
- **Performance**: Memory-mapped I/O for efficient file processing
- **Compatibility**: Support for standard magic file syntax and formats
- **Modern Design**: Extensible architecture for contemporary file formats
- **Multiple Outputs**: Both human-readable text and structured JSON formats

## Project Status

🚧 **Early Development Phase** - The project is currently in active development with core components being implemented.

### What's Complete

- ✅ **Core AST Structures**: Complete data model for magic rules
- ✅ **Serialization Support**: Full serde integration for all data types
- ✅ **CLI Framework**: Basic command-line interface structure
- ✅ **Project Infrastructure**: Build system, linting, and testing setup
- ✅ **Comprehensive Tests**: Extensive unit tests for AST components

### What's In Progress

- 🔄 **Magic File Parser**: nom-based parser for magic file DSL
- 🔄 **Rule Evaluator**: Engine for executing magic rules against files
- 🔄 **Memory-Mapped I/O**: Efficient file access implementation
- 🔄 **Output Formatters**: Text and JSON result formatting

## Why Rust?

The choice of Rust for this implementation provides several key advantages:

1. **Memory Safety**: Eliminates entire classes of security vulnerabilities
2. **Performance**: Zero-cost abstractions and efficient compiled code
3. **Concurrency**: Safe parallelism for processing multiple files
4. **Ecosystem**: Rich crate ecosystem for parsing, I/O, and serialization
5. **Maintainability**: Strong type system and excellent tooling

## Architecture Overview

The library follows a clean parser-evaluator architecture:

```text
Magic File → Parser → AST → Evaluator → Results → Formatter
                              ↓
                        Target File Buffer
```

This separation allows for:

- Independent testing of each component
- Flexible output formatting
- Efficient rule caching and optimization
- Clear error handling and debugging

## Getting Help

- **Documentation**: This guide covers all aspects of the library
- **API Reference**: Generated rustdoc for detailed API information
- **Issues**: [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues) for bugs and feature requests
- **Discussions**: [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions) for questions and ideas

## Contributing

We welcome contributions! See the [Development Setup](./development.md) and [Contributing Guidelines](./testing-guidelines.md) for information on how to get started.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](https://github.com/EvilBit-Labs/libmagic-rs/blob/main/LICENSE) file for details.

## Acknowledgments

This project is inspired by and respects the original [libmagic](https://www.darwinsys.com/file/) implementation by Ian Darwin and the current maintainers led by Christos Zoulas. We aim to provide a modern, safe alternative while maintaining compatibility with the established magic file format.
