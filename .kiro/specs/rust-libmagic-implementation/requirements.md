# Requirements Document

## Introduction

This document outlines the requirements for implementing a pure-Rust replacement of libmagic, the library that powers the `file` command for identifying file types. The implementation will provide a safe, efficient, and extensible engine capable of parsing magic files (DSL describing byte-level tests) and evaluating them against file buffers to identify file types with both human-readable and structured JSON output formats.

## Requirements

### Requirement 1: Magic File Parsing

**User Story:** As a developer, I want to parse magic files containing file type detection rules, so that I can use existing magic databases to identify file types.

#### Acceptance Criteria

1. WHEN a magic file is provided THEN the system SHALL parse it into an Abstract Syntax Tree (AST) of rules
2. WHEN parsing magic file syntax THEN the system SHALL support offset specifications (absolute, relative, indirect)
3. WHEN parsing magic file syntax THEN the system SHALL support type specifications (byte, short, long, quad, string, regex)
4. WHEN parsing magic file syntax THEN the system SHALL support comparison operators (=, !=, >, <, &, XOR)
5. WHEN parsing magic file syntax THEN the system SHALL support hierarchical rule nesting with indentation or leading markers
6. IF a magic file contains syntax errors THEN the system SHALL provide clear error messages with line numbers

### Requirement 2: File Type Evaluation

**User Story:** As a developer, I want to evaluate magic rules against file buffers, so that I can identify file types based on byte-level patterns.

#### Acceptance Criteria

1. WHEN evaluating rules against a file buffer THEN the system SHALL resolve offsets (absolute, indirect, relative) correctly
2. WHEN reading bytes at resolved offsets THEN the system SHALL interpret them according to the specified type with proper endianness handling
3. WHEN comparing interpreted values THEN the system SHALL apply the specified operator correctly
4. WHEN a parent rule matches THEN the system SHALL evaluate child rules for refinement
5. WHEN a rule matches THEN the system SHALL record the associated message
6. IF file buffer is truncated or corrupted THEN the system SHALL handle it safely without crashes

### Requirement 3: Memory Safety and Performance

**User Story:** As a security-conscious developer, I want a memory-safe file type detection library, so that I can avoid vulnerabilities present in C-based libmagic.

#### Acceptance Criteria

1. WHEN implementing the library THEN the system SHALL use only safe Rust code (no unsafe blocks except in vetted dependencies)
2. WHEN accessing file buffers THEN the system SHALL perform bounds checking for all buffer access
3. WHEN reading files THEN the system SHALL use memory-mapped I/O for efficient access
4. WHEN processing large files THEN the system SHALL avoid loading entire files into memory unnecessarily
5. WHEN handling malformed input THEN the system SHALL prevent buffer overflows and out-of-bounds reads

### Requirement 4: Output Formats

**User Story:** As a user, I want multiple output formats for file type identification results, so that I can integrate the tool into different workflows.

#### Acceptance Criteria

1. WHEN outputting results in text mode THEN the system SHALL format them like GNU `file` command
2. WHEN outputting results in JSON mode THEN the system SHALL provide structured data with matches, offsets, values, and metadata
3. WHEN multiple rules match THEN the system SHALL support both first-match and all-matches modes
4. WHEN no rules match THEN the system SHALL provide appropriate fallback messages
5. IF MIME type mapping is requested THEN the system SHALL optionally provide MIME type information

### Requirement 5: CLI Interface

**User Story:** As a command-line user, I want a CLI tool for file type identification, so that I can use it as a drop-in replacement for the `file` command.

#### Acceptance Criteria

1. WHEN running the CLI with a file argument THEN the system SHALL identify the file type and output results
2. WHEN using --json flag THEN the system SHALL output results in JSON format
3. WHEN using --text flag THEN the system SHALL output results in human-readable format
4. WHEN using --magic-file flag THEN the system SHALL use the specified custom magic file
5. IF no arguments are provided THEN the system SHALL display usage information

### Requirement 6: Library API

**User Story:** As a library consumer, I want a clean Rust API for file type detection, so that I can integrate it into my applications.

#### Acceptance Criteria

1. WHEN using the library API THEN the system SHALL provide functions to load magic rules from files
2. WHEN using the library API THEN the system SHALL provide functions to evaluate rules against byte buffers
3. WHEN using the library API THEN the system SHALL provide configurable output formats
4. WHEN using the library API THEN the system SHALL support both synchronous and asynchronous operations
5. IF invalid parameters are provided THEN the system SHALL return appropriate error types

### Requirement 7: Compatibility and Extensibility

**User Story:** As a developer migrating from libmagic, I want compatibility with existing magic files, so that I can use established file type databases.

#### Acceptance Criteria

1. WHEN processing standard magic files THEN the system SHALL produce results compatible with GNU `file` command
2. WHEN encountering common magic file syntax THEN the system SHALL support the most frequently used features
3. WHEN extending functionality THEN the system SHALL support modern file formats (PE resources, Mach-O, Go build info)
4. WHEN adding new rule types THEN the system SHALL maintain backward compatibility with existing magic files
5. IF unsupported syntax is encountered THEN the system SHALL provide clear warnings and graceful degradation

### Requirement 8: Documentation and Migration Support

**User Story:** As a developer migrating from libmagic, I want comprehensive documentation and migration guides, so that I can easily transition to the Rust implementation.

#### Acceptance Criteria

1. WHEN reading the API documentation THEN the system SHALL provide complete rustdoc documentation for all public APIs with examples
2. WHEN accessing the developer guide THEN the system SHALL provide an mdbook-based guide covering architecture, usage patterns, and best practices
3. WHEN migrating from libmagic THEN the system SHALL provide a migration guide comparing C API to Rust API with code examples
4. WHEN learning the library THEN the system SHALL provide tutorials for common use cases in the mdbook guide
5. WHEN integrating the library THEN the system SHALL provide examples for both CLI and library usage in documentation
6. IF compatibility differences exist THEN the system SHALL document them clearly with workarounds in the migration guide

### Requirement 9: Testing and Validation

**User Story:** As a quality-conscious developer, I want comprehensive testing of the file type detection system, so that I can ensure reliability and correctness.

#### Acceptance Criteria

1. WHEN running tests THEN the system SHALL compare results with GNU `file` on a sample corpus
2. WHEN testing with malformed files THEN the system SHALL handle truncated and corrupted inputs safely
3. WHEN performing fuzzing tests THEN the system SHALL not crash or produce memory safety violations
4. WHEN measuring performance THEN the system SHALL achieve parity with libmagic for common use cases
5. IF test coverage falls below 85% THEN the system SHALL require additional tests before release
