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

- ✅ **Core AST Structures**: Complete data model for magic rules with full serialization
- ✅ **Parser Components**: Numbers, offsets, operators, and values parsing with nom
- ✅ **Memory-Mapped I/O**: FileBuffer implementation with memmap2 and comprehensive safety
- ✅ **CLI Framework**: Command-line interface with clap and basic file handling
- ✅ **Project Infrastructure**: Build system, strict linting, and comprehensive testing
- ✅ **Extensive Test Coverage**: 98 comprehensive unit tests covering parser, AST, and I/O
- ✅ **Memory Safety**: Zero unsafe code with comprehensive bounds checking
- ✅ **Error Handling**: Structured error types with proper propagation and context
- ✅ **Code Quality**: Strict clippy linting with zero-warnings policy

### What's In Progress

- 🔄 **Complete Magic File Parser**: Full rule parsing with hierarchical structure support
- 🔄 **Rule Evaluation Engine**: Offset resolution, type interpretation, and operators
- 🔄 **Output Formatters**: Text and JSON result formatting with metadata

### Next Milestones

- 📋 **Parser Integration**: Combine parsing components into complete magic file parser
- 📋 **Basic Evaluator**: Simple rule evaluation against file buffers
- 📋 **Result Formatting**: Human-readable and structured output generation
- 📋 **Integration Testing**: End-to-end workflow validation

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

## How to Use This Guide

This documentation is organized into five main parts:

- **Part I: User Guide** - Getting started, CLI usage, and basic library integration
- **Part II: Architecture & Implementation** - Deep dive into the codebase structure and components
- **Part III: Advanced Topics** - Magic file formats, testing, and performance optimization
- **Part IV: Integration & Migration** - Moving from libmagic and troubleshooting
- **Part V: Development & Contributing** - Contributing guidelines and development setup

The appendices provide quick reference materials for commands, examples, and compatibility information.

## Getting Help

- **Documentation**: This comprehensive guide covers all aspects of the library
- **API Reference**: Generated rustdoc for detailed API information (Appendix A)
- **Command Reference**: Complete CLI documentation (Appendix B)
- **Examples**: Magic file examples and patterns (Appendix C)
- **Issues**: [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues) for bugs and feature requests
- **Discussions**: [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions) for questions and ideas

## Contributing

We welcome contributions! See the [Development Setup](./development.md) and [Contributing Guidelines](./testing-guidelines.md) for information on how to get started.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](https://github.com/EvilBit-Labs/libmagic-rs/blob/main/LICENSE) file for details.

## Acknowledgments

This project is inspired by and respects the original [libmagic](https://www.darwinsys.com/file/) implementation by Ian Darwin and the current maintainers led by Christos Zoulas. We aim to provide a modern, safe alternative while maintaining compatibility with the established magic file format.
