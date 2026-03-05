# Implementation Plan

- [x] 1. Create basic project structure

  **Completed**: Set up complete Rust project with Cargo.toml, core dependencies (memmap2, byteorder, nom, clap, serde, thiserror), and organized module structure with src/parser/, src/evaluator/, src/output/, src/io/ directories. Created basic CLI entry point and library API foundation.

  _Requirements: 6.1, 6.2, 3.3, 2.2, 1.1, 5.1_

- [x] 2. Create comprehensive AST types

  **Completed**: Implemented complete Abstract Syntax Tree in `src/parser/ast.rs` with `Value` enum (Uint, Int, Bytes, String), `OffsetSpec` enum (Absolute, Indirect, Relative, FromEnd), `TypeKind` enum (Byte, Short, Long, String with endianness/signedness), `Operator` enum (Equal, NotEqual, BitwiseAnd), `Endianness` enum, and `MagicRule` struct with hierarchical support. All types include full serde serialization and comprehensive unit tests.

  _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3_

- [x] 3. Create parser components using nom

  **Completed**: Implemented comprehensive parser components in `src/parser/grammar.rs` using nom combinators. Created `parse_number` (decimal/hex), `parse_offset` (absolute offsets), `parse_operator` (=, !=, &), and `parse_value` (strings, numeric literals, hex bytes) functions. All parsers include proper error handling, overflow protection, and extensive unit tests covering edge cases and various input formats.

  _Requirements: 1.1, 1.2, 1.3, 1.4, 1.6_

- [x] 4. Create memory-mapped file I/O system

  **Completed**: Implemented secure file I/O system in `src/io/mod.rs` with `FileBuffer` struct using memmap2 for efficient memory-mapped file access. Created comprehensive `IoError` type for file access errors, implemented RAII resource cleanup, and added bounds-checked buffer access helpers. Includes extensive unit tests for file operations, error handling, and buffer safety.

  _Requirements: 3.3, 3.4, 3.5, 6.5, 3.2_

- [x] 5. Create offset resolution system

  **Completed**: Implemented comprehensive offset resolution in `src/evaluator/offset.rs` with `resolve_absolute_offset` function supporting positive/negative offsets, `resolve_offset` interface handling `OffsetSpec` enum variants, and safe arithmetic preventing integer overflow. Includes bounds checking, proper error handling, and extensive unit tests for various offset scenarios and edge cases.

  _Requirements: 2.1, 3.2_

- [x] 6. Create type reading and interpretation system

  **Completed**: Implemented comprehensive type reading system in `src/evaluator/types.rs` with `read_byte`, `read_short`, `read_long`, and `read_string` functions using byteorder crate for endianness handling. Created `read_typed_value` interface supporting all `TypeKind` variants, with proper bounds checking, UTF-8 validation, and extensive unit tests covering all data types and edge cases.

  _Requirements: 2.2, 3.2_

- [x] 7. Create operator evaluation system

  **Completed**: Implemented complete operator system in `src/evaluator/operators.rs` with `apply_equal`, `apply_not_equal`, and `apply_bitwise_and` functions for value comparison and pattern matching. Created `apply_operator` interface handling all `Operator` enum variants with proper type matching, integer operations, and comprehensive unit tests covering all operator combinations and edge cases.

  _Requirements: 2.3, 1.4_

- [x] 8. Create rule evaluation engine

  **Completed**: Implemented complete rule evaluation system in `src/evaluator/mod.rs` with `evaluate_single_rule` and `evaluate_rules` functions for hierarchical rule processing. Created `EvaluationContext` for state management and `EvaluationConfig` for behavior control with recursion limits, string length limits, and match behavior. Includes graceful error handling, parent-child rule relationships, and comprehensive unit tests.

  _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 6.3_

- [x] 9. Create output formatting system

  **Completed**: Implemented comprehensive output system in `src/output/mod.rs` with `MatchResult` struct for storing evaluation results, `EvaluationResult` for complete file analysis, and `EvaluationMetadata` for performance tracking. Created text formatting in `src/output/text.rs` with GNU file command compatibility, message concatenation, and proper fallback handling. Includes extensive unit tests and serialization support.

  _Requirements: 4.1, 4.2, 4.4_

- [x] 10. Create comprehensive CLI interface

  **Completed**: Implemented complete CLI interface in `src/main.rs` using clap with argument parsing for input files, output format flags (--text, --json), and custom magic file paths. Added platform-specific magic file discovery (Unix: /usr/share/file, /etc/magic; Windows: %APPDATA%\\Magic), comprehensive error handling with proper exit codes, and fallback magic file creation for CI/CD environments. Includes extensive unit tests and integration tests.

  _Requirements: 5.1, 5.2, 5.3, 5.5, 6.5_

- [x] 11. Create JSON output system

  **Completed**: Implemented comprehensive JSON output system in `src/output/json.rs` with `JsonMatchResult` struct following original libmagic specification (text, offset, value, tags, score fields). Created `format_json_output` functions for both pretty and compact JSON formatting, integrated with CLI --json flag handling, and added `JsonOutput` structure for complete results. Includes 28 comprehensive unit tests covering all JSON functionality and edge cases.

  _Requirements: 4.2, 1.1, 5.2_

- [x] 12. Add string type support

  **Completed**: Extended AST with `TypeKind::String { max_length: Option<usize> }` variant and implemented comprehensive string reading in `src/evaluator/types.rs` with `read_string` function. Added null-terminated string handling, UTF-8 validation with `String::from_utf8_lossy` fallback, length limits, bounds checking, and integration with `read_typed_value`. Includes 25 comprehensive unit tests covering string reading edge cases, encodings, and safety scenarios.

  _Requirements: 1.3, 2.2, 3.2_

- [x] 13. Create comprehensive error handling system

  **Completed**: Implemented complete error handling system in `src/error.rs` with `LibmagicError` enum using thiserror, including `ParseError`, `EvaluationError`, and `IoError` variants. Created detailed error types for buffer overruns, invalid offsets, unsupported types, and timeout scenarios. Integrated Result types throughout evaluator with graceful degradation and error recovery. Includes extensive unit tests for all error scenarios and proper error message formatting.

  _Requirements: 1.6, 2.6, 3.5, 6.5_

- [ ] 14. Implement text-based magic file parsing

**Note: Magic files come in two formats:**

- **Text format (.magic)**: Human-readable files with lines like "0 string \\x7fELF ELF executable"

- **Binary format (.mgc)**: Compiled binary files with magic signature, optimized for fast loading

- **Priority**: Implement text format first (more common in development), then binary format for compatibility

- [x] 14.1 Implement complete magic rule parsing for text format

  - Add parse_magic_rule function to parser/grammar.rs for parsing complete rule lines from text magic files
  - Support offset, type, operator, value, and message parsing in sequence for human-readable format
  - Handle hierarchical rule parsing with proper indentation levels (> prefix for child rules)
  - Parse comments (# prefix), empty lines, and continuation lines (\\ suffix)
  - Write unit tests for complete rule parsing with various text magic file formats
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [ ] 14.2 Implement text magic file parsing

  - Add parse_text_magic_file function to parser/mod.rs for parsing entire text-based magic files
  - Handle line-by-line parsing with proper error reporting and line numbers
  - Support comments, empty lines, and continuation lines in text format
  - Implement hierarchical rule nesting based on indentation and > prefixes
  - Write unit tests for text magic file parsing with sample .magic files
  - _Requirements: 1.1, 1.5, 1.6_

- [ ] 14.3 Add magic file format detection

  - Create detect_magic_file_format function to distinguish between text and binary magic files
  - Check for binary .mgc file signatures (magic bytes at start of compiled files)
  - Implement fallback logic: try binary first, then text format
  - Add proper error handling for unsupported or corrupted magic file formats
  - Write unit tests for format detection with both text and binary magic files
  - _Requirements: 6.1, 1.6_

- [ ] 15. Implement binary magic file (.mgc) support

**Note: Binary .mgc files are compiled versions of text magic files:**

- **Structure**: Header + Rule entries + String tables + Metadata

- **Advantages**: Faster loading, pre-validated rules, optimized for production use

- **Challenges**: Format is not officially documented, requires reverse engineering or libmagic source analysis

- **Detection**: Usually start with specific magic bytes (e.g., 0x0d0a1a0a) and have .mgc extension

- [ ] 15.1 Add binary magic file format detection and basic parsing

  - Research and document the binary .mgc file format structure (header, rule entries, string tables)
  - Implement detect_binary_magic_format function to identify .mgc files by magic signature
  - Create basic binary parser structure for reading .mgc file headers and metadata
  - Add proper error handling for corrupted or unsupported binary magic file versions
  - Write unit tests for binary format detection and header parsing
  - _Requirements: 6.1, 1.6_

- [ ] 15.2 Implement binary magic rule deserialization

  - Add parse_binary_magic_file function to deserialize compiled magic rules from .mgc files
  - Implement binary rule entry parsing (offset, type, operator, value, message extraction)
  - Handle string table lookups for rule messages and string values
  - Support hierarchical rule relationships as stored in binary format
  - Write unit tests for binary rule deserialization with sample .mgc files
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 15.3 Integrate unified magic file loading

  - Create unified load_magic_file function that handles both text and binary formats
  - Implement format auto-detection: try binary .mgc first, fallback to text parsing
  - Connect both text and binary parsers with MagicDatabase loading
  - Add comprehensive error handling and format-specific error messages
  - Write integration tests with both text .magic and binary .mgc files
  - _Requirements: 6.1, 6.2, 1.6_

- [ ] 16. Complete MagicDatabase integration and CLI functionality

- [ ] 16.1 Implement MagicDatabase with unified magic file loading

  - Update MagicDatabase::load_from_file to use the unified magic file parser (text and binary)
  - Replace the current placeholder that returns empty rules with actual parsing integration
  - Add proper error propagation from parsing failures to database creation errors
  - Implement rule validation and consistency checking after loading
  - Write unit tests for database loading with both text and binary magic files
  - _Requirements: 6.1, 6.2, 1.6_

- [ ] 16.2 Fix file evaluation pipeline integration

  - Connect loaded magic rules with the evaluation engine in evaluate_file function
  - Ensure proper buffer loading, rule evaluation, and result collection
  - Fix the current placeholder implementation that always returns "data"
  - Add proper error handling for file access and evaluation failures
  - Write integration tests for end-to-end file type detection with real magic files
  - _Requirements: 6.2, 6.3, 2.5_

- [ ] 16.3 Add built-in fallback magic rules

  - Create a comprehensive set of built-in magic rules for common file types (ELF, PE, ZIP, JPEG, PNG, PDF, GIF)
  - Implement fallback mechanism when no external magic file is available or loading fails
  - Ensure CLI works out-of-the-box for basic file type detection without requiring system magic files
  - Add configuration option to disable built-in rules and force external magic file usage
  - Write tests for built-in rule functionality and fallback behavior
  - _Requirements: 7.1, 5.5, 6.2_

- [ ] 17. Set up basic test infrastructure

- [ ] 17.1 Set up basic test infrastructure

  - Create tests/ directory with fixtures/ subdirectory for test files
  - Add sample binary files (simple ELF, basic ZIP archive)
  - Create basic magic rule files for testing common patterns
  - _Requirements: 9.1_

- [ ] 17.2 Create compatibility test framework

  - Implement test harness for comparing results with GNU file command
  - Add test cases for basic file type detection accuracy
  - Write unit tests for compatibility test framework functionality
  - _Requirements: 9.1_

- [ ] 17.3 Add performance benchmark setup

  - Create benchmark framework using criterion crate for performance testing
  - Implement basic benchmarks for file loading and rule evaluation
  - Write benchmark tests measuring detection speed on sample files
  - _Requirements: 9.4_

- [ ] 18. Create basic cache structure

- [ ] 18.1 Create basic cache structure

  - Create src/cache/mod.rs with CachedRules struct for rule serialization
  - Add fields for version, source hash, timestamp, and rules
  - Implement Serialize/Deserialize traits using serde and bincode
  - _Requirements: 3.4, 7.4_

- [ ] 18.2 Implement cache validation

  - Add cache validation functions for checking timestamps and checksums
  - Implement source file hash calculation for cache invalidation
  - Write unit tests for cache validation with modified and unmodified files
  - _Requirements: 7.4_

- [ ] 18.3 Add cache location management

  - Implement cache directory creation using XDG cache directories
  - Add cache file path generation and management functions
  - Write unit tests for cache location handling across different platforms
  - _Requirements: 7.4_

- [ ] 19. Add magic file CLI argument

- [ ] 19.1 Add magic file CLI argument

  - Extend CLI argument struct in main.rs to include --magic-file option
  - Update argument parsing to handle custom magic file paths
  - Write unit tests for CLI argument parsing with magic file options
  - _Requirements: 5.4_

- [ ] 19.2 Implement custom magic file loading

  - Add custom magic file loading logic to main.rs
  - Implement file validation and error reporting for invalid magic files
  - Write integration tests for custom magic file usage scenarios
  - _Requirements: 5.4, 7.1_

- [ ] 19.3 Add magic file precedence handling

  - Implement support for multiple magic file sources with priority ordering
  - Add logic for combining rules from different magic file sources
  - Write unit tests for magic file precedence and rule merging
  - _Requirements: 7.1_

- [ ] 20. Add basic rustdoc documentation

- [ ] 20.1 Add basic rustdoc documentation

  - Add rustdoc comments to all public functions in lib.rs with usage examples
  - Document MagicDatabase struct and its methods with code examples
  - Write documentation for error types and their usage patterns
  - _Requirements: 8.1_

- [ ] 20.2 Create library usage examples

  - Create examples/ directory with basic library usage example
  - Add example for loading magic files and evaluating single files
  - Write example demonstrating error handling and result processing
  - _Requirements: 8.5_

- [ ] 20.3 Document API patterns

  - Add rustdoc documentation for evaluation configuration options
  - Document output format selection and result interpretation
  - Write documentation covering synchronous API usage patterns
  - _Requirements: 8.1, 8.5_

- [ ] 21. Set up basic fuzzing infrastructure

- [ ] 21.1 Set up basic fuzzing infrastructure

  - Add cargo-fuzz dependency and create fuzz/ directory
  - Create basic fuzz target for magic file parser
  - Write fuzz harness for testing parser with malformed input
  - _Requirements: 9.3_

- [ ] 21.2 Add evaluator fuzzing

  - Create fuzz target for rule evaluator with corrupted file inputs
  - Implement fuzz harness for testing evaluation engine robustness
  - Write unit tests verifying no crashes with malformed binary data
  - _Requirements: 9.3, 3.5_

- [ ] 21.3 Integrate continuous fuzzing

  - Set up fuzzing configuration for automated testing
  - Add fuzzing to CI pipeline for continuous robustness testing
  - Write documentation for running and interpreting fuzz tests
  - _Requirements: 9.3_

- [ ] 22. Create basic MIME type mapping

- [ ] 22.1 Create basic MIME type mapping

  - Create src/mime/mod.rs with basic MIME type database structure
  - Add common file type to MIME type mappings (text, image, executable)
  - Write unit tests for MIME type lookup functionality
  - _Requirements: 4.5, 7.3_

- [ ] 22.2 Integrate MIME types in output

  - Add MIME type resolution to output formatters (text and JSON)
  - Implement optional MIME type inclusion in match results
  - Write unit tests for MIME type integration in output formatting
  - _Requirements: 4.5_

- [ ] 22.3 Add MIME type CLI support

  - Add --mime CLI flag for MIME type output mode
  - Implement MIME-only output format for compatibility with file --mime
  - Write integration tests for MIME type CLI functionality
  - _Requirements: 4.5_

- [ ] 23. Set up mdbook project structure

- [ ] 23.1 Set up mdbook project structure

  - Create docs/ directory with mdbook configuration and basic structure
  - Add introduction chapter with project overview and goals
  - Create table of contents for architecture, usage, and migration sections
  - _Requirements: 8.2_

- [ ] 23.2 Create architecture documentation

  - Write architecture chapter explaining parser-evaluator design pattern
  - Document module organization and component responsibilities
  - Add diagrams showing data flow and component interactions
  - _Requirements: 8.2, 8.4_

- [ ] 23.3 Write migration guide

  - Create migration chapter comparing libmagic C API to Rust API
  - Add code examples showing equivalent operations in both libraries
  - Document compatibility differences and recommended workarounds
  - _Requirements: 8.3, 8.6_

- [ ] 23.4 Add usage tutorials

  - Write tutorial chapter with common usage patterns and examples
  - Add best practices guide for magic rule creation and optimization
  - Create troubleshooting section for common issues and solutions
  - _Requirements: 8.4, 8.2_

- [ ] 24. Add Aho-Corasick string optimization

- [ ] 24.1 Add Aho-Corasick string optimization

  - Add aho-corasick dependency for multi-pattern string search
  - Implement string pattern indexing for improved search performance
  - Write unit tests for Aho-Corasick integration with string matching
  - _Requirements: 9.4, 3.4_

- [ ] 24.2 Implement lazy rule evaluation

  - Modify evaluation engine to only process child rules when parent matches
  - Add evaluation statistics tracking for performance monitoring
  - Write unit tests for lazy evaluation behavior and performance impact
  - _Requirements: 3.4_

- [ ] 24.3 Create performance validation

  - Implement comprehensive performance benchmarks comparing with libmagic
  - Add performance regression testing to CI pipeline
  - Write performance analysis documentation and optimization guidelines
  - _Requirements: 9.4_

- [ ] 25. Add basic PE format detection

- [ ] 25.1 Add basic PE format detection

  - Create src/formats/pe.rs with basic PE header detection
  - Implement PE signature and header validation
  - Write unit tests for PE format detection with sample executables
  - _Requirements: 7.3_

- [ ] 25.2 Add Mach-O format detection

  - Create src/formats/macho.rs with Mach-O header detection
  - Implement magic number and architecture detection for Mach-O files
  - Write unit tests for Mach-O format detection with sample binaries
  - _Requirements: 7.3_

- [ ] 25.3 Add Go build info extraction

  - Create src/formats/go.rs with Go build info detection
  - Implement Go version and build information extraction from binaries
  - Write unit tests for Go build info detection with compiled Go programs
  - _Requirements: 7.3, 7.4_
