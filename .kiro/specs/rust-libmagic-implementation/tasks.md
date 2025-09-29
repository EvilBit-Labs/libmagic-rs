# Implementation Plan

- [x] 1. Create basic project structure

  - Create Cargo.toml with project metadata and basic dependencies (serde, thiserror)
  - Create src/lib.rs with empty public API structure
  - Create src/main.rs with basic CLI entry point
  - _Requirements: 6.1, 6.2_

- [x] 1.1 Set up directory structure

  - Create src/parser/ directory with mod.rs file
  - Create src/evaluator/ directory with mod.rs file
  - Create src/output/ directory with mod.rs file
  - Create src/io/ directory with mod.rs file
  - _Requirements: 6.1_

- [x] 1.2 Add core dependencies to Cargo.toml

  - Add memmap2 for memory-mapped file I/O
  - Add byteorder for endianness handling
  - Add nom for parser combinators
  - Add clap for CLI argument parsing
  - _Requirements: 3.3, 2.2, 1.1, 5.1_

- [x] 2. Create basic AST value types

  - Create src/parser/ast.rs with Value enum (Uint, Int, Bytes, String)
  - Implement Debug, Clone, PartialEq, Serialize, Deserialize for Value
  - Write unit tests for Value enum serialization and comparison
  - _Requirements: 1.1, 1.2_

- [x] 2.1 Create offset specification types

  - Add OffsetSpec enum to ast.rs (Absolute, Indirect, Relative, FromEnd)
  - Implement Debug, Clone, Serialize, Deserialize for OffsetSpec
  - Write unit tests for OffsetSpec variants
  - _Requirements: 1.2, 2.1_

- [x] 2.2 Create type kind definitions

  - Add TypeKind enum to ast.rs (Byte, Short, Long, String with basic options)
  - Include endianness and signedness fields for numeric types
  - Write unit tests for TypeKind variants and serialization
  - _Requirements: 1.3, 2.2_

- [x] 2.3 Create operator definitions

  - Add Operator enum to ast.rs (Equal, NotEqual, BitwiseAnd)
  - Implement Debug, Clone, Serialize, Deserialize for Operator
  - Write unit tests for Operator enum functionality
  - _Requirements: 1.4, 2.3_

- [x] 2.4 Create magic rule structure

  - Add MagicRule struct to ast.rs with offset, typ, op, value, message, children fields
  - Implement Debug, Clone, Serialize, Deserialize for MagicRule
  - Write unit tests for MagicRule creation and serialization
  - _Requirements: 1.1, 1.5_

- [ ] 3. Create basic nom parser setup

  - Create src/parser/grammar.rs with nom imports and basic parser structure
  - Implement parse_number function for parsing decimal and hex numbers
  - Write unit tests for number parsing with various formats
  - _Requirements: 1.1, 1.6_

- [x] 3.1 Implement offset parsing

  - Add parse_offset function to grammar.rs for absolute offset parsing
  - Support decimal and hexadecimal offset formats
  - Write unit tests for offset parsing with positive and negative values
  - _Requirements: 1.2, 1.6_

- [ ] 3.2 Implement type parsing

  - Add parse_type function to grammar.rs for basic type parsing (byte, short, long)
  - Support endianness specifiers (le, be) for multi-byte types
  - Write unit tests for type parsing with various endianness options
  - _Requirements: 1.3, 1.6_

- [ ] 3.3 Implement operator parsing

  - Add parse_operator function to grammar.rs for comparison operators (=, !=, &)
  - Support both symbolic and text representations of operators
  - Write unit tests for operator parsing with different formats
  - _Requirements: 1.4, 1.6_

- [ ] 3.4 Implement value parsing

  - Add parse_value function to grammar.rs for string and numeric literals
  - Support quoted strings with escape sequences and hex byte sequences
  - Write unit tests for value parsing with various literal formats
  - _Requirements: 1.1, 1.6_

- [ ] 4. Create basic file buffer structure

  - Create src/io/mod.rs with FileBuffer struct using memmap2
  - Implement new() method for creating memory-mapped file buffers
  - Add as_slice() method for safe buffer access
  - _Requirements: 3.3, 3.4_

- [ ] 4.1 Add file buffer error handling

  - Create IoError type for file access errors in io/mod.rs
  - Implement proper error handling in FileBuffer::new() with descriptive messages
  - Add resource cleanup using RAII patterns for file handles
  - Write unit tests for file buffer creation with invalid files
  - _Requirements: 3.5, 6.5_

- [ ] 4.2 Add buffer bounds checking helpers

  - Create safe_read_bytes function in io/mod.rs for bounds-checked buffer access
  - Implement buffer length validation and overflow prevention
  - Write unit tests for bounds checking with various buffer sizes and offsets
  - _Requirements: 3.2, 3.5_

- [ ] 5. Create basic offset resolution

  - Create src/evaluator/offset.rs with resolve_absolute_offset function
  - Implement simple absolute offset calculation with bounds checking
  - Write unit tests for absolute offset resolution with valid offsets
  - _Requirements: 2.1, 3.2_

- [ ] 5.1 Add negative offset support

  - Extend resolve_absolute_offset to handle negative offsets from file end
  - Implement safe arithmetic to prevent integer overflow in offset calculations
  - Write unit tests for negative offset resolution with various file sizes
  - _Requirements: 2.1, 3.2_

- [ ] 5.2 Create offset resolution interface

  - Add resolve_offset function in offset.rs that handles OffsetSpec enum
  - Implement basic absolute offset resolution using existing functions
  - Write unit tests for offset resolution interface with OffsetSpec::Absolute
  - _Requirements: 2.1_

- [ ] 6. Create basic type reading for byte values

  - Create src/evaluator/types.rs with read_byte function
  - Implement safe byte reading from buffer with bounds checking
  - Write unit tests for byte reading at various buffer positions
  - _Requirements: 2.2, 3.2_

- [ ] 6.1 Add multi-byte type reading with endianness

  - Add read_short and read_long functions to types.rs using byteorder crate
  - Implement little-endian and big-endian reading for 16-bit and 32-bit values
  - Write unit tests for multi-byte reading with different endianness
  - _Requirements: 2.2, 3.2_

- [ ] 6.2 Create type interpretation interface

  - Add read_typed_value function in types.rs that handles TypeKind enum
  - Implement type-specific reading using existing read functions
  - Write unit tests for typed value reading with various TypeKind variants
  - _Requirements: 2.2_

- [ ] 7. Create basic equality operator

  - Create src/evaluator/operators.rs with apply_equal function for value equality comparison
  - Implement Value-to-Value comparison with proper type matching
  - Write unit tests for equality comparison with same and different value types
  - _Requirements: 2.3, 1.4_

- [ ] 7.1 Add inequality operator

  - Add apply_not_equal function to operators.rs for inequality comparison
  - Implement negation of equality comparison logic
  - Write unit tests for inequality comparison with various value combinations
  - _Requirements: 2.3, 1.4_

- [ ] 7.2 Add bitwise AND operator

  - Add apply_bitwise_and function to operators.rs for pattern matching
  - Implement bitwise AND operation for integer values with proper type handling
  - Write unit tests for bitwise AND with various integer values and masks
  - _Requirements: 2.3, 1.4_

- [ ] 7.3 Create operator application interface

  - Add apply_operator function in operators.rs that handles Operator enum
  - Implement operator dispatch using existing apply functions
  - Write unit tests for operator application interface with all supported operators
  - _Requirements: 2.3_

- [ ] 8. Create basic rule evaluation

  - Create src/evaluator/mod.rs with evaluate_single_rule function
  - Implement single rule evaluation using offset resolution, type reading, and operator application
  - Write unit tests for single rule evaluation with simple magic rules
  - _Requirements: 2.1, 2.2, 2.3, 2.5_

- [ ] 8.1 Add evaluation context structure

  - Create EvaluationContext struct in evaluator/mod.rs for maintaining evaluation state
  - Add fields for current offset, recursion depth, and configuration
  - Write unit tests for context creation and state management
  - _Requirements: 2.4_

- [ ] 8.2 Add evaluation configuration

  - Create EvaluationConfig struct in evaluator/mod.rs with evaluation options
  - Add fields for recursion limits, string length limits, and match behavior
  - Write unit tests for configuration creation and validation
  - _Requirements: 2.4, 6.3_

- [ ] 8.3 Implement hierarchical rule evaluation

  - Add evaluate_rules function to evaluator/mod.rs for processing rule lists
  - Implement parent-child rule relationship handling with proper hierarchy traversal
  - Add early termination on first match and context preservation for nested rules
  - Write unit tests for hierarchical evaluation with nested magic rules
  - _Requirements: 2.4, 2.5_

- [ ] 9. Create basic match result structure

  - Create src/output/mod.rs with MatchResult struct for storing evaluation results
  - Add fields for message, offset, value, and rule metadata
  - Write unit tests for match result creation and serialization
  - _Requirements: 4.1, 4.2_

- [ ] 9.1 Implement text output formatting

  - Create src/output/text.rs with format_text_result function
  - Implement message formatting for single match results
  - Write unit tests for text formatting with various match results
  - _Requirements: 4.1_

- [ ] 9.2 Add text output concatenation

  - Add format_text_output function to text.rs for multiple match results
  - Implement message concatenation and fallback handling for no matches
  - Write unit tests comparing output with expected GNU file command format
  - _Requirements: 4.1, 4.4_

- [ ] 10. Create basic CLI argument structure

  - Create CLI argument struct in src/main.rs using clap derive macros
  - Add fields for input file, output format flags (--text, --json)
  - Write unit tests for argument parsing with various command line inputs
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 10.1 Implement CLI file processing

  - Add main function logic in main.rs for processing input files
  - Implement file loading, rule evaluation, and output formatting
  - Write integration tests for CLI functionality with sample files
  - _Requirements: 5.1, 5.5_

- [ ] 10.2 Add CLI error handling

  - Implement error handling in main.rs with proper exit codes
  - Add user-friendly error messages for common failure scenarios
  - Add usage information display when no arguments are provided
  - Write unit tests for CLI error handling and exit code behavior
  - _Requirements: 5.5, 6.5_

- [ ] 11. Create JSON match result structure

  - Create src/output/json.rs with JsonMatchResult struct following original spec
  - Add fields for text, offset, value, tags, and score
  - Implement Serialize trait for JSON output compatibility
  - Write unit tests for JSON match result serialization
  - _Requirements: 4.2_

- [ ] 11.1 Implement JSON output formatting

  - Add format_json_output function to json.rs for converting match results to JSON
  - Implement matches array structure with proper field mapping
  - Write unit tests for JSON output format validation and structure
  - _Requirements: 4.2, 1.1_

- [ ] 11.2 Add JSON output integration

  - Integrate JSON formatter into CLI output routing in main.rs
  - Add --json flag handling with appropriate output selection
  - Write integration tests for JSON output through CLI interface
  - _Requirements: 5.2, 4.2_

- [ ] 12. Add basic string type to AST

  - Extend TypeKind enum in ast.rs to include String variant with max_length field
  - Update serialization and unit tests for new String type variant
  - _Requirements: 1.3_

- [ ] 12.1 Implement string reading in evaluator

  - Add read_string function to evaluator/types.rs for null-terminated string reading
  - Implement safe string extraction with length limits and bounds checking
  - Write unit tests for string reading with various string lengths and termination
  - _Requirements: 2.2, 3.2_

- [ ] 12.2 Add string matching support

  - Extend read_typed_value function in types.rs to handle String type
  - Implement UTF-8 validation and ASCII fallback for string values
  - Write unit tests for string type interpretation with various encodings
  - _Requirements: 1.3, 2.2_

- [ ] 13. Create basic error types

  - Create src/error.rs with LibmagicError enum using thiserror
  - Add variants for ParseError, EvaluationError, and IoError
  - Write unit tests for error type creation and Display formatting
  - _Requirements: 1.6, 2.6, 6.5_

- [ ] 13.1 Add evaluation error types

  - Create EvaluationError enum in error.rs for runtime evaluation errors
  - Add variants for BufferOverrun, InvalidOffset, and UnsupportedType
  - Write unit tests for evaluation error scenarios and error messages
  - _Requirements: 2.6, 3.5_

- [ ] 13.2 Integrate error handling in evaluator

  - Update evaluator functions to return Result types with proper error handling
  - Implement graceful degradation to skip problematic rules and continue evaluation
  - Write unit tests for error recovery behavior in rule evaluation
  - _Requirements: 2.6, 3.5_

- [ ] 14. Create basic library API structure

  - Create public API functions in lib.rs for loading and parsing magic files
  - Add load_magic_file function that returns parsed rules
  - Write unit tests for magic file loading with valid and invalid files
  - _Requirements: 6.1, 6.2_

- [ ] 14.1 Add file evaluation API

  - Create evaluate_file function in lib.rs for processing files with magic rules
  - Implement file loading, rule evaluation, and result collection
  - Write unit tests for file evaluation API with sample files and rules
  - _Requirements: 6.2, 6.3_

- [ ] 14.2 Create magic database structure

  - Implement MagicDatabase struct in lib.rs for rule management
  - Add methods for loading rules, caching, and evaluation configuration
  - Write unit tests for database creation and rule management
  - _Requirements: 6.1, 6.3_

- [ ] 15. Add indirect offset parsing

  - Extend parse_offset function in parser/grammar.rs to support indirect syntax
  - Implement parsing for parentheses-based indirect offset notation
  - Write unit tests for indirect offset parsing with various formats
  - _Requirements: 1.2, 1.6_

- [ ] 15.1 Implement indirect offset resolution

  - Add resolve_indirect_offset function to evaluator/offset.rs
  - Implement pointer dereferencing with proper endianness handling using byteorder
  - Write unit tests for indirect offset resolution with different pointer types
  - _Requirements: 2.1, 1.2_

- [ ] 15.2 Integrate indirect offsets in evaluation

  - Update resolve_offset function to handle OffsetSpec::Indirect variant
  - Add recursion limits to prevent infinite indirect offset chains
  - Write unit tests for indirect offset integration in rule evaluation
  - _Requirements: 2.1_

- [ ] 16. Add regex type to AST

  - Extend TypeKind enum in ast.rs to include Regex variant
  - Add regex pattern field and compilation flags
  - Write unit tests for regex type serialization and deserialization
  - _Requirements: 1.3_

- [ ] 16.1 Create binary regex trait

  - Create BinaryRegex trait in evaluator/types.rs for abstracting regex engines
  - Implement trait methods for binary-safe pattern matching
  - Write unit tests for binary regex trait interface
  - _Requirements: 1.3, 2.2_

- [ ] 16.2 Implement regex matching

  - Add regex crate dependency and implement BinaryRegex for regex::bytes::Regex
  - Create read_regex function in types.rs for pattern matching on binary data
  - Write unit tests for regex matching with various binary patterns
  - _Requirements: 1.3, 2.2_

- [ ] 17. Set up basic test infrastructure

  - Create tests/ directory with fixtures/ subdirectory for test files
  - Add sample binary files (simple ELF, basic ZIP archive)
  - Create basic magic rule files for testing common patterns
  - _Requirements: 9.1_

- [ ] 17.1 Create compatibility test framework

  - Implement test harness for comparing results with GNU file command
  - Add test cases for basic file type detection accuracy
  - Write unit tests for compatibility test framework functionality
  - _Requirements: 9.1_

- [ ] 17.2 Add performance benchmark setup

  - Create benchmark framework using criterion crate for performance testing
  - Implement basic benchmarks for file loading and rule evaluation
  - Write benchmark tests measuring detection speed on sample files
  - _Requirements: 9.4_

- [ ] 18. Create basic cache structure

  - Create src/cache/mod.rs with CachedRules struct for rule serialization
  - Add fields for version, source hash, timestamp, and rules
  - Implement Serialize/Deserialize traits using serde and bincode
  - _Requirements: 3.4, 7.4_

- [ ] 18.1 Implement cache validation

  - Add cache validation functions for checking timestamps and checksums
  - Implement source file hash calculation for cache invalidation
  - Write unit tests for cache validation with modified and unmodified files
  - _Requirements: 7.4_

- [ ] 18.2 Add cache location management

  - Implement cache directory creation using XDG cache directories
  - Add cache file path generation and management functions
  - Write unit tests for cache location handling across different platforms
  - _Requirements: 7.4_

- [ ] 19. Add magic file CLI argument

  - Extend CLI argument struct in main.rs to include --magic-file option
  - Update argument parsing to handle custom magic file paths
  - Write unit tests for CLI argument parsing with magic file options
  - _Requirements: 5.4_

- [ ] 19.1 Implement custom magic file loading

  - Add custom magic file loading logic to main.rs
  - Implement file validation and error reporting for invalid magic files
  - Write integration tests for custom magic file usage scenarios
  - _Requirements: 5.4, 7.1_

- [ ] 19.2 Add magic file precedence handling

  - Implement support for multiple magic file sources with priority ordering
  - Add logic for combining rules from different magic file sources
  - Write unit tests for magic file precedence and rule merging
  - _Requirements: 7.1_

- [ ] 20. Add basic rustdoc documentation

  - Add rustdoc comments to all public functions in lib.rs with usage examples
  - Document MagicDatabase struct and its methods with code examples
  - Write documentation for error types and their usage patterns
  - _Requirements: 8.1_

- [ ] 20.1 Create library usage examples

  - Create examples/ directory with basic library usage example
  - Add example for loading magic files and evaluating single files
  - Write example demonstrating error handling and result processing
  - _Requirements: 8.5_

- [ ] 20.2 Document API patterns

  - Add rustdoc documentation for evaluation configuration options
  - Document output format selection and result interpretation
  - Write documentation covering synchronous API usage patterns
  - _Requirements: 8.1, 8.5_

- [ ] 21. Set up basic fuzzing infrastructure

  - Add cargo-fuzz dependency and create fuzz/ directory
  - Create basic fuzz target for magic file parser
  - Write fuzz harness for testing parser with malformed input
  - _Requirements: 9.3_

- [ ] 21.1 Add evaluator fuzzing

  - Create fuzz target for rule evaluator with corrupted file inputs
  - Implement fuzz harness for testing evaluation engine robustness
  - Write unit tests verifying no crashes with malformed binary data
  - _Requirements: 9.3, 3.5_

- [ ] 21.2 Integrate continuous fuzzing

  - Set up fuzzing configuration for automated testing
  - Add fuzzing to CI pipeline for continuous robustness testing
  - Write documentation for running and interpreting fuzz tests
  - _Requirements: 9.3_

- [ ] 22. Create basic MIME type mapping

  - Create src/mime/mod.rs with basic MIME type database structure
  - Add common file type to MIME type mappings (text, image, executable)
  - Write unit tests for MIME type lookup functionality
  - _Requirements: 4.5, 7.3_

- [ ] 22.1 Integrate MIME types in output

  - Add MIME type resolution to output formatters (text and JSON)
  - Implement optional MIME type inclusion in match results
  - Write unit tests for MIME type integration in output formatting
  - _Requirements: 4.5_

- [ ] 22.2 Add MIME type CLI support

  - Add --mime CLI flag for MIME type output mode
  - Implement MIME-only output format for compatibility with file --mime
  - Write integration tests for MIME type CLI functionality
  - _Requirements: 4.5_

- [ ] 23. Set up mdbook project structure

  - Create docs/ directory with mdbook configuration and basic structure
  - Add introduction chapter with project overview and goals
  - Create table of contents for architecture, usage, and migration sections
  - _Requirements: 8.2_

- [ ] 23.1 Create architecture documentation

  - Write architecture chapter explaining parser-evaluator design pattern
  - Document module organization and component responsibilities
  - Add diagrams showing data flow and component interactions
  - _Requirements: 8.2, 8.4_

- [ ] 23.2 Write migration guide

  - Create migration chapter comparing libmagic C API to Rust API
  - Add code examples showing equivalent operations in both libraries
  - Document compatibility differences and recommended workarounds
  - _Requirements: 8.3, 8.6_

- [ ] 23.3 Add usage tutorials

  - Write tutorial chapter with common usage patterns and examples
  - Add best practices guide for magic rule creation and optimization
  - Create troubleshooting section for common issues and solutions
  - _Requirements: 8.4, 8.2_

- [ ] 24. Add Aho-Corasick string optimization

  - Add aho-corasick dependency for multi-pattern string search
  - Implement string pattern indexing for improved search performance
  - Write unit tests for Aho-Corasick integration with string matching
  - _Requirements: 9.4, 3.4_

- [ ] 24.1 Implement lazy rule evaluation

  - Modify evaluation engine to only process child rules when parent matches
  - Add evaluation statistics tracking for performance monitoring
  - Write unit tests for lazy evaluation behavior and performance impact
  - _Requirements: 3.4_

- [ ] 24.2 Create performance validation

  - Implement comprehensive performance benchmarks comparing with libmagic
  - Add performance regression testing to CI pipeline
  - Write performance analysis documentation and optimization guidelines
  - _Requirements: 9.4_

- [ ] 25. Add basic PE format detection

  - Create src/formats/pe.rs with basic PE header detection
  - Implement PE signature and header validation
  - Write unit tests for PE format detection with sample executables
  - _Requirements: 7.3_

- [ ] 25.1 Add Mach-O format detection

  - Create src/formats/macho.rs with Mach-O header detection
  - Implement magic number and architecture detection for Mach-O files
  - Write unit tests for Mach-O format detection with sample binaries
  - _Requirements: 7.3_

- [ ] 25.2 Add Go build info extraction

  - Create src/formats/go.rs with Go build info detection
  - Implement Go version and build information extraction from binaries
  - Write unit tests for Go build info detection with compiled Go programs
  - _Requirements: 7.3, 7.4_
