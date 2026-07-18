# AI Assistant Guidelines for libmagic-rs

This document provides comprehensive guidelines for AI assistants working on the libmagic-rs project, ensuring consistent, high-quality development practices and project understanding.

@GOTCHAS.md

## Project Overview

**libmagic-rs** is a pure-Rust implementation of libmagic, designed to replace the C-based library with a memory-safe, efficient alternative for file type detection.

### Core Mission

- **Memory Safety**: Pure Rust implementation with no unsafe code (except vetted dependencies)
- **Performance**: Memory-mapped I/O with zero-copy operations where possible
- **Compatibility**: Support for common libmagic syntax patterns
- **Extensibility**: AST-based design for easy addition of new rule types

## Development Principles

### 1. Memory Safety First

- **No unsafe code** except in vetted dependencies (memmap2, byteorder, etc.)
- **Bounds checking** for all buffer access using `.get()` methods
- **Safe resource management** with RAII patterns
- **Graceful error handling** for malformed inputs
- **Safe string operations**: Use `strip_prefix()`/`strip_suffix()` instead of direct slicing (`&str[n..]`) to avoid UTF-8 panics

### 2. Zero-Warnings Policy

- All code must pass `cargo clippy -- -D warnings` with no exceptions
- Preserve all `deny` attributes and `-D warnings` flags
- Fix clippy suggestions unless they conflict with project requirements
- Use `cargo fmt` for consistent code formatting

### 3. Performance Critical

- Use memory-mapped I/O (`memmap2`) for efficient file access
- Implement zero-copy operations where possible
- Use Aho-Corasick indexing for multi-pattern string searches (planned)
- Cache compiled magic rules for performance (planned)
- Profile with `cargo bench` for performance regressions

### 4. Testing Required

- Target >85% test coverage with `cargo llvm-cov`
- All code changes must include comprehensive tests
- Use `cargo nextest` for faster, more reliable test execution
- Include property tests with `proptest` for fuzzing
- Benchmark critical path components with `criterion`
- Verify doc examples with `cargo test --doc` - ensure example strings don't accidentally match multiple patterns

## Architecture Patterns

### Parser-Evaluator Design

The project follows a clear separation of concerns:

```text
Magic File → Parser → AST → Evaluator → Match Results → Output Formatter
     ↓
Target File → Memory Mapper → File Buffer
```

### Module Organization

```rust
// Core data structures in lib.rs
pub struct MagicRule { /* ... */ }
pub enum TypeKind {
    Byte { signed: bool },
    Short { endian: Endianness, signed: bool },
    Long { endian: Endianness, signed: bool },
    Quad { endian: Endianness, signed: bool },
    String { max_length: Option<usize> },
}
pub enum Operator {
    Equal, NotEqual, LessThan, GreaterThan, LessEqual, GreaterEqual,
    BitwiseAnd, BitwiseAndMask(u64), BitwiseXor, BitwiseNot, AnyValue,
}

// Parser module structure
parser/
├── mod.rs      // Public parser interface
├── ast.rs      // AST node definitions
├── grammar/    // Magic file DSL parsing (nom) -- split into focused submodules
│   ├── mod.rs        // Top-level parse_magic_rule_line, dispatch
│   ├── numbers.rs    // parse_number, parse_unsigned_number
│   ├── value.rs      // parse_value (quoted strings, numeric literals)
│   ├── type_suffix.rs // pstring /B/H/L, regex /c/s, search /N suffixes
│   └── tests/        // Grammar test modules
├── types.rs    // Type keyword parsing and TypeKind conversion
└── codegen.rs  // Serialization for code generation (shared with build.rs)

// Evaluator module structure
evaluator/
├── mod.rs          // Public interface: EvaluationContext, RuleMatch, re-exports
├── tests.rs        // Unit tests for EvaluationContext and RuleMatch
├── engine/         // Core evaluation engine submodule
│   ├── mod.rs      // evaluate_single_rule, evaluate_rules, evaluate_rules_with_config
│   └── tests.rs    // Engine unit tests
├── types/          // Type interpretation with endianness (directory module, issue #63)
│   ├── mod.rs      // read_typed_value, read_pattern_match, bytes_consumed_with_pattern
│   ├── numeric.rs  // byte/short/long/quad readers
│   ├── string.rs   // string/pstring readers
│   ├── float.rs    // float/double readers
│   ├── date.rs     // date/qdate readers and timestamp formatting
│   └── regex.rs    // regex/search readers, REGEX_MAX_BYTES cap, thread-local cache
├── strength.rs     // Strength modifier application
├── offset/         // Offset resolution submodule
│   ├── mod.rs      // Dispatcher (resolve_offset) and re-exports
│   ├── absolute.rs // OffsetError, resolve_absolute_offset
│   ├── indirect.rs // resolve_indirect_offset (fully implemented, issue #37)
│   └── relative.rs // resolve_relative_offset (GNU `file` anchor semantics)
└── operators/      // Operator application submodule
    ├── mod.rs      // Dispatcher (apply_operator, apply_any_value) and re-exports
    ├── equality.rs // apply_equal, apply_not_equal
    ├── comparison.rs // compare_values, apply_less_than/greater_than/less_equal/greater_equal
    └── bitwise.rs  // apply_bitwise_and, apply_bitwise_and_mask, apply_bitwise_xor, apply_bitwise_not
```

## Code Quality Standards

### File Size Limits

- Keep source files under 500-600 lines
- Split larger files into focused modules
- Use clear, descriptive module names

### Emoji Usage

- Avoid using emojis and other non-ASCII characters in code, comments, or documentation, except when the code is handling non-plaintext characters (for example: em dash, en dash, or other non-ASCII symbols).

### Case-Insensitive Matching Pattern

When implementing case-insensitive string matching:

- Lowercase inputs at ALL entry points (constructors, setters)
- Store normalized values internally
- Document the case-insensitivity in public API docs

### Error Handling Patterns

```rust
// Library errors should be descriptive and actionable
#[derive(Debug, thiserror::Error)]
pub enum MagicError {
    #[error("Failed to parse magic file at line {line}: {reason}")]
    ParseError { line: usize, reason: String },

    #[error("IO error reading file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid offset specification: {offset}")]
    InvalidOffset { offset: String },
}

// Use Result types consistently
pub fn evaluate_magic_rules(
    rules: &[MagicRule],
    data: &[u8],
) -> Result<Option<String>, MagicError> {
    // Implementation
}
```

### Architecture Constraints

- Clippy pedantic lints are active (e.g., prefer `trailing_zeros()` over bitwise masks)
- Comparison operators share a `compare_values() -> Option<Ordering>` helper in `operators/comparison.rs` -- new comparison logic goes there, not in individual `apply_*` functions

> See **[GOTCHAS.md](GOTCHAS.md)** for build script boundaries (S1), enum variant update checklists (S2), parser architecture (S3), numeric type pitfalls (S5), string/pstring encoding (S6), and other non-obvious behaviors.

### Naming Conventions

- **Files**: snake_case (e.g., `magic_rule.rs`)
- **Types**: PascalCase (e.g., `MagicRule`, `TypeKind`)
- **Functions**: snake_case (e.g., `resolve_offset`, `evaluate_rule`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_BUFFER_SIZE`)
- **Modules**: snake_case (e.g., `evaluator`, `output`)

## Development Workflow

### Standard Commands

All commands should be run via `mise exec --` to use the project's pinned Rust toolchain.

```bash
# Development cycle
cargo check        # Fast syntax/type checking
cargo build        # Build project
cargo test         # Run all tests
cargo clippy       # Linting with strict warnings
cargo fmt          # Format code
just ci-check      # Run complete CI suite locally (pre-commit validation)

# Performance and quality
cargo bench        # Run benchmarks
cargo doc          # Generate documentation
cargo test --doc   # Test documentation examples
```

### Testing Strategy

- **Unit Tests**: Alongside source files with `#[cfg(test)]`
- **Integration Tests**: In `tests/` directory with real magic files
- **Compatibility Tests**: Complete test suite from [original file project](https://raw.githubusercontent.com/file/file/refs/heads/master/tests/README)
- **Property Tests**: Use `proptest` for fuzzing magic rule evaluation
- **Benchmarks**: Critical path performance tests with `criterion`
- **Coverage**: Target >85% with `cargo llvm-cov`
- **Test style**: Prefer table-driven tests over one-assertion-per-function tests; consolidate related cases into a single test with descriptive failure messages

## Magic File Compatibility

### Currently Implemented (v0.5.x, unreleased)

- **Offsets**: Absolute, from-end, indirect, and relative specifications (relative offsets `&+N`/`&-N` are evaluated using GNU `file` semantics -- the previous-match anchor)
- **Types**: `byte`, `short`, `long`, `quad`, `float`, `double`, `string`, `pstring`, `lestring16`/`bestring16` with endianness support; unsigned variants `ubyte`, `ushort`/`ubeshort`/`uleshort`, `ulong`/`ubelong`/`ulelong`, `uquad`/`ubequad`/`ulequad`; float/double endian variants `befloat`/`lefloat`, `bedouble`/`ledouble`; 32-bit date/timestamp types `date`/`ldate`/`bedate`/`beldate`/`ledate`/`leldate`; 64-bit date/timestamp types `qdate`/`qldate`/`beqdate`/`beqldate`/`leqdate`/`leqldate`; `pstring` is a Pascal string (length-prefixed) with support for 1/2/4-byte length prefixes via `/B`, `/H` (2-byte BE), `/h` (2-byte LE), `/L` (4-byte BE), `/l` (4-byte LE) suffixes, and the `/J` flag (stored length includes prefix width, JPEG convention) which is combinable with width suffixes (e.g., `pstring/HJ`); `lestring16`/`bestring16` are UCS-2 strings (2 bytes per character, little/big-endian) that read until a U+0000 terminator (`0x00 0x00`), end-of-buffer, or 8192-unit cap, with surrogate halves replaced by U+FFFD; date values formatted as "Www Mmm DD HH:MM:SS YYYY" matching GNU `file` output; types are signed by default (libmagic-compatible)
- **Operators**: `=` (equal), `!=` and bare `!` (not equal), `<` (less than), `>` (greater than), `<=` (less equal), `>=` (greater equal), `&` (bitwise AND with optional mask), `^` (bitwise XOR), `~` (bitwise NOT), `x` (any value). magic(5) bare `!` prefix (e.g., `!0xb8c0078e`) is treated as `NotEqual`.
- **Nested Rules**: Hierarchical rule evaluation with proper indentation
- **String Matching**: Exact string matching with null-termination and Pascal string (length-prefixed) support
- **Regex type**: Binary-safe regex matching via `regex::bytes::Regex`. Full flag support: `/c` (case-insensitive), `/s` (anchor advances to match-start instead of match-end), `/l` (scan window is measured in lines instead of bytes). Flags combine in any order (`regex/cs`, `regex/csl`, `regex/lc`). Numeric counts are honored: `regex/100` scans at most 100 bytes; `regex/1l` scans at most 1 line. Multi-line regex matching is always on (matching libmagic's unconditional `REG_NEWLINE`), so `^` and `$` match at line boundaries regardless of `/l`. Every scan window is capped at 8192 bytes (`FILE_REGEX_MAX`) regardless of the user's count. Unquoted (bareword) `regex` patterns are captured as a GNU `file` `getstr`-resolved `Value::String`: magic(5) escapes (`\t`, octal `\NNN`, hex `\xNN`, named control escapes `\a`/`\b`/`\f`/`\n`/`\r`/`\v`) resolve to their byte; an unrecognized escape drops the backslash (`\^` -> `^`), so `\d`/`\s`/`\w` demote to literal `d`/`s`/`w` (genuine upstream libmagic behavior, not a bug); resolved bytes `>= 0x80` are re-encoded as regex-native `\xHH` in the pattern text. Quoted regex values are unaffected. The evaluator additionally accepts a `Value::Bytes` pattern operand for `Regex` (mirroring `Search`, see GOTCHAS S2.4/S2.12), decoding it via lossy UTF-8 with a `warn!` on real substitution, as a floor-tier safety net for any pattern that still reaches evaluation as `Bytes`.
- **Search type**: Bounded literal pattern scan via `memchr::memmem::find`; `search/N` caps the scan window to `N` bytes from the offset. The range is **mandatory** and stored as `NonZeroUsize`, so bare `search` and `search/0` are parse errors (matching GNU `file` magic(5)). Anchor advance follows GNU `file` semantics (match-end, not window-end) so relative-offset children resolve to the byte immediately after the matched pattern. Full flag support (issue #235): `/s` (start-anchor — anchor lands at match-START instead of match-END, required for TGA footer and sfnt name table rules), `/c`/`/C` (asymmetric case-insensitive matching, pattern-controlled fold direction — see GOTCHAS S6.5), `/w` (whitespace-optional parallel-walk), `/W` (whitespace-required compact), `/T` (`STRING_TRIM` — leading/trailing whitespace stripped from pattern at evaluation time), `/f` (`STRING_FULL_WORD` — post-match word boundary check), `/t` and `/b` (parsed and captured as MIME-output hints; comparison-time effect deferred to `!:mime` evaluation in #51). `/B` on search is an alias for `/b` (binary hint) — distinct from `/B` on pstring which is the 1-byte length-width letter. Flag dispatcher splits on category: anchor-only flags (`/s`/`/t`/`/b`) keep the SIMD-accelerated `memchr::memmem::find` fast path; comparison-altering flags (`/c`/`/C`/`/w`/`/W`/`/T`/`/f`) trigger byte-by-byte walk via `compare_string_with_flags` through `SearchFlags::to_string_flags()`. See GOTCHAS S2.6 for the `/s` anchor-advance contract.
- **Meta-type directives**: `default`, `clear`, `name <id>`, `use <id>`, `indirect`, and `offset` are fully implemented. `name` blocks are hoisted into a `NameTable` at load time (`parser::name_table::extract_name_table`). `use` invokes subroutines at the resolved offset via `RuleEnvironment` threaded through `EvaluationContext::rule_env`; subroutine-local absolute offsets resolve relative to the use-site base (tracked via `EvaluationContext::base_offset`). `default` fires only when no sibling at the same level has matched; `clear` resets the per-level sibling-matched flag so a later `default` can fire. `indirect` re-applies the root rule set at the resolved offset, bounded by `EvaluationConfig::max_recursion_depth`. `offset` reports the resolved file offset as `Value::Uint(pos)` for format-string rendering. Continuation siblings (`recursion_depth > 0`) see the parent-level anchor on each iteration rather than chaining -- matching libmagic's `ms->c.li[cont_level]` model. Top-level siblings still chain (documented in GOTCHAS S3.8).
- **Printf-style format substitution**: Rule messages support `%d`, `%i`, `%u`, `%x`, `%X`, `%o`, `%s`, `%c`, and `%%`, along with width/padding modifiers (`%05d`, `%-5d`) and length modifiers (`l`, `ll`, `h`, etc. -- parsed and ignored). Hex specifiers respect the rule's `TypeKind::bit_width()` to mask sign-extended signed reads (so a signed byte carrying `-1` renders as `ff`, not `ffffffffffffffff`). Implemented in `src/output/format.rs::format_magic_message` and wired into `MagicDatabase::build_result`. Unrecognized specifiers pass through literally with a `debug!` log.

See **Development Phases** below for the planned roadmap of features not yet implemented (Aho-Corasick multi-pattern optimization and `!:mime`/`!:ext`/`!:apple` directive evaluation).

## Current Limitations (v0.5.x, unreleased)

### Type System

- 64-bit integer types: `quad`/`uquad`, `bequad`/`ubequad`, `lequad`/`ulequad` are implemented; `qquad` (128-bit) is not yet supported
- `string` evaluation has two read modes selected by whether a comparison pattern is supplied: pattern-comparison uses `read_string_exact` (reads exactly `pattern.len()` bytes byte-for-byte, with NO NUL truncation -- required for patterns containing embedded NULs like `0 string PNCIHISK\0 ...`), while `string x` (any-value) uses `read_string` which reads until first NUL or end-of-buffer for printable-prefix output. The `x` path also fires when `read_string` is called directly without a pattern. Bareword string values interpret magic(5) escape sequences (`\0`, `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, octal `\NNN`, hex `\xNN`); `max_length: Some(_)` is supported programmatically (via the AST) and is now used by the dispatcher for explicit-length reads. See `docs/solutions/logic-errors/magic-string-rule-matching-3-bug-fix-2026-04-25.md` for the load-bearing details.
- `string` type modifier flags `/c /C /w /W /b /t /T /f` are fully implemented (issue #234, PR #288). `/c` and `/C` perform ASCII case-insensitive matching with the libmagic asymmetric contract (the pattern character controls fold direction — see GOTCHAS S6.5); `/w` and `/W` give whitespace-optional and whitespace-required-compact parallel-walk matching; `/T` trims leading/trailing ASCII whitespace from the pattern at evaluation time (`STRING_TRIM`); `/f` enforces a post-match word boundary (`STRING_FULL_WORD`). `/t` (`STRING_TEXTTEST`) and `/b` (`STRING_BINTEST`) are MIME-output hints — parsed and stored, but with no comparison effect today (output-side wiring deferred to the `!:mime` evaluation work in #51). Default-flag `string` rules continue to take the byte-exact fast path. `/B` is **not** a `string` flag in libmagic — it is exclusively the `pstring` 1-byte length-width letter — so `string/B` is rejected at parse time. See GOTCHAS S6.5 (asymmetric `/c`/`/C`) and S6.6 (`/B` exclusion).
- `pstring` supports 1-byte (`/B`), 2-byte big-endian (`/H`), 2-byte little-endian (`/h`), 4-byte big-endian (`/L`), and 4-byte little-endian (`/l`) length prefixes, plus the `/J` flag (stored length includes prefix width). All flags are combinable (e.g., `pstring/HJ`) and fully implemented.

### Operators

- Parser handles `&`, `&<decimal>`, and `&0x<hex>` masks across the full `u64` range; compound forms like arithmetic expressions in mask position (`&(N+M)`) or post-mask modifiers are not parsed

### Offset Specifications

- Indirect offsets are fully implemented (parsing + evaluation) with specifiers: `.b/.B` (byte), `.s/.S` (short), `.l/.L` (long), `.q/.Q` (quad); lowercase = little-endian, uppercase = big-endian (GNU `file` semantics); pointer types signed by default. Adjustment is accepted both **inside** the parens (canonical magic(5): `(base.type+N)`, `(base.type-N)`, `(base.type*N)`, `(base.type/N)`, `(base.type%N)`, `(base.type&N)`, `(base.type|N)`, `(base.type^N)`) and **after** the closing paren as a legacy form (`(base.type)+N`, `(base.type)-N` only). Only one form per rule. Anchor-relative variants are supported via `base_relative` (the `(&N.X)` form -- pointer-read base is `anchor + N`) and `result_relative` (the `&(N.X)` wrapper -- final offset is added to the anchor) on `OffsetSpec::Indirect`.
- **Pre-comparison value transforms**: `type+N`, `type-N`, `type*N`, `type/N`, `type%N`, `type|N`, `type^N` between the type keyword and the operator. The transform runs on the read value before the comparison and before printf-style format substitution. Stored on `MagicRule::value_transform` as `ValueTransform { op, operand }` (default: `None`). magic(5) usage: `lelong+1 x volume %d`, `ulequad/1073741824 x size %lluGB`. Bitwise AND (`&MASK`) is *not* part of `ValueTransformOp` -- it predates this enum and is encoded at the operator layer via `Operator::BitwiseAndMask`.
- Relative offsets are fully evaluated against the GNU `file` previous-match anchor: the engine tracks `EvaluationContext::last_match_end()`, advancing it after each successful match by the bytes consumed (variable-width types include c-string NUL terminators and pstring length prefixes). Top-level relative offsets resolve from anchor 0. Magic-file `&N`, `&+N`, and `&-N` parsing is implemented (decimal and `0x` hex forms supported; bare `&` is rejected). See `parse_offset_relative` in `src/parser/grammar/tests/mod.rs` for the parse-side coverage.

### Magic File Syntax

- Only `!:strength` is parsed and evaluated. Other `!:` directives (`!:mime`, `!:ext`, `!:apple`, ...) are silently skipped at preprocessing time so files that include them load successfully, but their metadata is not surfaced anywhere.
- Meta-type directives (`default`, `clear`, `name`, `use`, `indirect`, `offset`) are all fully implemented with evaluator dispatch, including printf-style format substitution in message rendering (see "Currently Implemented" above for details).

See issue #52 for the planned enhancement roadmap.

## Performance Requirements

### Critical Optimizations

- **Memory Mapping**: Use `mmap` to avoid loading entire files into memory
- **Zero-Copy**: Minimize allocations during rule evaluation
- **Aho-Corasick**: Use for multi-pattern string searches when beneficial
- **Rule Caching**: Cache compiled magic rules for repeated use
- **Early Exit**: Stop evaluation as soon as a definitive match is found

### Benchmarking

```rust
// Example benchmark structure
#[bench]
fn bench_magic_evaluation(b: &mut Bencher) {
    let rules = load_magic_rules("tests/fixtures/standard.magic");
    let file_data = include_bytes!("../tests/fixtures/sample.bin");

    b.iter(|| evaluate_rules(&rules, file_data));
}
```

## Output Formats

### Text Output (Default)

```text
sample.bin: ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

### JSON Output (Structured)

```json
{
  "filename": "sample.bin",
  "matches": [
    {
      "text": "ELF 64-bit LSB executable",
      "offset": 0,
      "value": "7f454c46",
      "tags": [
        "executable",
        "elf"
      ],
      "score": 90,
      "mime_type": "application/x-executable"
    }
  ],
  "metadata": {
    "file_size": 8192,
    "evaluation_time_ms": 2.3,
    "rules_evaluated": 45
  }
}
```

## Common Tasks and Patterns

### Adding New Type Support

> **Note:** Currently implemented types are `Byte`, `Short`, `Long`, `Quad`, `Float`, `Double`, `Date`, `QDate`, `String`, `String16`, `PString`, `Regex`, and `Search`. See "Current Limitations" for the remaining gaps in regex/search flag coverage.

1. Extend `TypeKind` enum in `src/parser/ast.rs`
2. Add keyword parsing in `src/parser/types.rs` (`parse_type_keyword` and `type_keyword_to_kind`)
3. Add value/operator parsing in `src/parser/grammar/mod.rs` if needed
4. Implement reading logic in `src/evaluator/types.rs`
5. Update `serialize_type_kind()` in `src/parser/codegen.rs`
6. Add tests for the new type
7. Update documentation

### Adding a new meta-type

Meta-types sit inside `TypeKind::Meta(MetaType)` and do not read bytes. Adding a new variant requires:

1. Add the variant to `MetaType` in `src/parser/ast.rs`. Update the three test fixtures that iterate `MetaType` variants: `test_meta_type_variants_debug_clone_eq`, `test_meta_type_serde_roundtrip`, `test_type_kind_meta_bit_width_is_none` (see GOTCHAS S2.11).
2. Add the keyword tag in `parse_type_keyword` and the arm in `type_keyword_to_kind` in `src/parser/types.rs`, plus the `test_roundtrip_all_keywords` array.
3. Update `serialize_type_kind` (the inner `TypeKind::Meta(meta)` arm) in `src/parser/codegen.rs`.
4. Update `arb_type_kind` in `tests/property_tests.rs` (`prop_oneof` branch for `MetaType`).
5. Decide semantics: does the new variant need inline loop-level dispatch in `evaluate_rules` (like `Use`, `Default`, `Clear`, `Indirect` — each of which mutates the match vector or `sibling_matched` flag) or is it a silent no-op via the `Meta(_)` wildcard arm in `evaluate_single_rule_with_anchor`? Add the arm accordingly in `src/evaluator/engine/mod.rs`.
6. Add unit tests covering parse round-trip, the evaluator arm, and any new `RuleEnvironment` lookups.

### Adding New Operators

> **Note:** Currently implemented operators are `Equal`, `NotEqual`, `LessThan`, `GreaterThan`, `LessEqual`, `GreaterEqual`, `BitwiseAnd` (with `BitwiseAndMask`), `BitwiseXor`, `BitwiseNot`, and `AnyValue`.

1. Extend `Operator` enum in `src/parser/ast.rs`
2. Add parsing logic in `src/parser/grammar/mod.rs`
3. Implement operator logic in `src/evaluator/operators/` submodule
4. Update `serialize_operator()` in `src/parser/codegen.rs`
5. Update strength calculation match in `src/evaluator/strength.rs`
6. Update `arb_operator()` in `tests/property_tests.rs`
7. Add tests for the new operator
8. Update documentation

### Performance Optimization

1. Profile with `cargo bench` to identify bottlenecks
2. Use memory-mapped I/O for file access
3. Implement caching for compiled rules
4. Use Aho-Corasick for multi-pattern searches
5. Minimize allocations in hot paths

### Testing Build Scripts

Build scripts (`build.rs`) cannot import the crate being built, which makes them difficult to test. To enable comprehensive testing of build script logic:

1. Extract build logic into a library module with `#[cfg(any(test, doc))]`
2. Keep build.rs minimal, calling functions from the testable module
3. Write unit tests in the library module to verify all code paths
4. Example: `src/build_helpers.rs` provides testable parsing and code generation

This pattern ensures build-time failures (e.g., invalid magic files) are properly tested and produce clear error messages.

## Error Recovery Strategy

### Parse Errors

- Continue parsing after syntax errors
- Collect all errors for batch reporting
- Provide clear error messages with line numbers

### Evaluation Errors

- Graceful degradation
- Skip problematic rules and continue with others
- Maintain evaluation context for nested rules
- A pattern-bearing rule (`regex`, `search`, or a flagged `string`) evaluated without a usable `String`/`Bytes` pattern operand, or a regex rule that fails to compile (including the `REGEX_COMPILE_SIZE_LIMIT` DoS guard), is skipped as a non-match rather than aborting the rest of the file's evaluation -- logged at `debug!` for the ordinary case and `warn!` for a compile failure. This is a narrow, allowlisted exception; other evaluator capability gaps (an unwired `TypeKind`, an unsupported operator on a pattern-bearing type) still propagate as errors. See GOTCHAS S2.1 for the exact predicate.

### IO Errors

- Proper resource cleanup
- Clear error messages for file access issues
- Handle truncated and corrupted files safely

## Security Considerations

### Memory Safety

- No unsafe code except in vetted dependencies
- Bounds checking for all buffer access
- Safe handling of malformed input
- Fuzzing integration for robustness testing

### Input Validation

- Validate magic file syntax before parsing
- Check file size limits and resource usage
- Handle malicious or malformed input gracefully
- Implement timeouts for long-running evaluations

## Documentation Requirements

### API Documentation

- All public APIs require rustdoc with examples
- Include error conditions and recovery strategies
- Provide usage examples for common patterns
- Document performance characteristics

### Code Comments

- Explain complex algorithms and optimizations
- Document magic file syntax support
- Include references to libmagic compatibility
- Explain design decisions and trade-offs

## CI/CD Integration

### Automated Checks

The project uses GitHub Actions CI with Mergify merge protections:

1. **Formatting**: `cargo fmt` for consistent code style
2. **Linting**: `cargo clippy -- -D warnings` for best practices
3. **Compilation**: `cargo check` and `cargo build` for error detection
4. **Testing**: `cargo test` and `cargo nextest run` for validation
5. **Security**: `cargo audit` for vulnerability detection
6. **License Compliance**: Verify dependency licenses

### Quality Gates

- All code must pass clippy with `-D warnings`
- Test coverage must be >85%
- No compilation warnings or errors
- All tests must pass
- Security audit must pass
- Performance benchmarks must not regress

### Code Review Requirements

All pull requests require review before merging. Reviews are performed by maintainers and automated tools (CodeRabbit). Reviewers check for:

- **Correctness**: Does the code do what it claims? Are edge cases handled?
- **Memory safety**: No unsafe code blocks (except vetted dependencies). All buffer access must use bounds checking with `.get()` methods. No raw pointer arithmetic or transmute operations.
- **Error handling**: Proper use of `Result` types, no panics in library code, no `unwrap()` or `expect()` in library code. Use `thiserror` for structured error types.
- **Tests**: New functionality has tests, existing tests still pass, edge cases and error conditions are covered. Property tests with `proptest` for complex data structures.
- **Performance**: No unnecessary allocations in hot paths, no regressions in benchmarks. Memory-mapped I/O used for file access.
- **libmagic compatibility**: Changes maintain compatibility with libmagic behavior and magic file format. Output format matches GNU `file` command expectations.
- **Style**: Follows project conventions, passes `cargo fmt` and `cargo clippy -- -D warnings`
- **Documentation**: Public APIs have rustdoc with examples, AGENTS.md updated if architecture changes

CI must pass before merge. Mergify merge protections enforce these checks. Bot PRs from dependabot and dosubot are auto-merged by Mergify when all required CI checks pass. Bot PRs from release-plz are auto-merged by Mergify when their required DCO check passes (they are exempt from full CI in `.mergify.yml`). Human PRs are merged manually by maintainers.

## Project Context

### Current Status

- **Phase**: Early development (MVP)
- **Focus**: Core parser and evaluator implementation
- **Priority**: Memory safety and basic functionality
- **Next Steps**: Enhanced features and performance optimization

### Key Dependencies

- `memmap2`: Memory-mapped file I/O
- `byteorder`: Endianness handling
- `nom`: Parser combinators
- `serde`: Serialization
- `clap`: CLI argument parsing
- `regex`: Binary-safe pattern matching via `regex::bytes::Regex` for `TypeKind::Regex` evaluation
- `memchr`: SIMD-accelerated literal pattern search, used for `TypeKind::Search`
- `aho-corasick`: Multi-pattern search (planned, not yet added)

### Development Phases

1. **MVP (v0.1.0)** - shipped: Basic parsing and evaluation with byte/short/long/quad/string types, equality and bitwise AND operators, built-in rules for 10 common formats
2. **Enhanced Features (v0.2.0)** - shipped: Comparison operators (`>`, `<`, `<=`, `>=`), bitwise XOR/NOT, indirect and relative offset evaluation, strength-based rule ordering
3. **Advanced Types (v0.3.0)** - shipped: float/double/date/qdate/pstring types, regex, search, evaluator submodule split
4. **v0.4.0** - shipped: parse warnings, JSON metadata, improved errors, cargo-dist release pipeline
5. **v0.5.x (current)** - in flight: TOCTOU/search-path hardening, regex compile cache, `EvaluationConfig` non_exhaustive, `MagicRule::new` validator, thread-local regex cache
6. **v0.6.0**: `Value` pattern refactor (eliminate pattern-as-literal overloading), `MagicDatabase::builder()`, `Directive` extension point for `!:mime`/`!:ext`/`!:apple`
7. **v1.0.0**: Stable API, complete documentation, 95%+ compatibility with GNU file, Aho-Corasick multi-pattern optimization, cargo-fuzz harness, complete `#[non_exhaustive]` coverage

## Best Practices

### Code Organization

- Keep modules focused and cohesive
- Use clear, descriptive names
- Minimize coupling between modules
- Maximize cohesion within modules

### Error Handling

- Use `Result<T, E>` patterns consistently
- Avoid panics in library code
- Provide actionable error messages
- Implement graceful degradation

### Testing

- Write tests alongside implementation
- Include edge cases and error conditions
- Use property-based testing for complex logic
- Benchmark performance-critical code

### Documentation

- Document public APIs thoroughly
- Include usage examples
- Explain design decisions
- Keep documentation up-to-date

## Troubleshooting

### Common Issues

- **Compilation errors**: Check for missing dependencies and syntax issues
- **Test failures**: Verify test logic and expected behavior
- **Performance issues**: Profile with `cargo bench` and optimize hot paths
- **Memory issues**: Check for bounds violations and resource leaks

### Debugging Tips

- Use `cargo test -- --nocapture` for test output
- Enable debug logging with `RUST_LOG=debug`
- Use `cargo clippy` to catch potential issues
- Profile with `cargo bench` for performance analysis

This guide ensures consistent, high-quality development practices for the libmagic-rs project while maintaining focus on memory safety, performance, and compatibility.

## Quick Reference

- Mergify auto-merges dependabot/dosubot PRs when full CI passes; release-plz PRs when DCO passes (exempt from full CI)
- Human PRs are merged manually -- Mergify only provides merge protections for those
- `.mergify.yml` configures auto-merge rules and merge protections
- `cargo deny check` uses `deny.toml` (default) -- do not specify a custom config path
- `.github/workflows/release.yml` is auto-generated by cargo-dist -- do not modify manually
- All `.rs` files must have copyright and SPDX headers (see any source file for format)
- `Cargo.lock` and `mise.lock` are committed for reproducible builds -- do not gitignore
- In justfile recipes, never wrap `just` in `{{ mise_exec }}` -- it's redundant
- Changelog: `just changelog`, `just changelog-version <tag>`, `just changelog-unreleased`
- Security contact: <support@evilbitlabs.io> (matches PGP key in SECURITY.md)
- `docs/solutions/` — documented solutions to past problems and reusable design patterns, organized into per-category subdirectories (see `ls docs/solutions/` for the current set; bug-track categories like `logic-errors/`, knowledge-track categories like `design-patterns/` and `developer-experience/`). Each doc has YAML frontmatter keyed on `module`, `problem_type`, `component`, `severity`, and `tags`. Relevant when implementing or debugging in documented areas.

## Open Source Quality Standards (OSSF Best Practices)

This project has the OSSF Best Practices passing badge. Maintain these standards:

### Every PR must

- Sign off commits with `git commit -s` (DCO enforced by GitHub App)
- Pass CI (clippy, fmt, tests, CodeQL, cargo audit) before merge
- Include tests for new functionality -- this is policy, not optional
- Be reviewed (human or CodeRabbit) for correctness, safety, and style
- Not introduce `unsafe` code, `unwrap()`/`expect()` in library code, or panics

### Every release must

- Have human-readable release notes via git-cliff (not raw git log)
- Use unique SemVer identifiers (`vX.Y.Z` tags)
- Be built reproducibly (pinned toolchain, committed lock files, cargo-dist)

### Security

- Vulnerabilities go through private reporting (GitHub advisories or <support@evilbitlabs.io>), never public issues
- `cargo audit` and `cargo deny` run daily in CI -- fix findings promptly
- Medium+ severity vulnerabilities: we aim to release a fix within 90 days of confirmation (see SECURITY.md for canonical policy)
- `unsafe_code = "deny"` is enforced project-wide via workspace lints in `Cargo.toml` (inherited through `[lints] workspace = true`) -- this is a hardening mechanism, not a suggestion. Exactly one vetted, SAFETY-commented `#[allow(unsafe_code)]` exception exists: the memmap2 `map()` call in `src/io/mod.rs` (memory mapping is inherently unsafe). Any new exception requires the same treatment and review (see GOTCHAS S8.2)
- `docs/src/security-assurance.md` must be updated when new attack surface is introduced

### Documentation

- Public APIs require rustdoc with examples
- CONTRIBUTING.md documents code review criteria, test policy, DCO, and governance
- SECURITY.md documents vulnerability reporting with scope, safe harbor, and PGP key
- AGENTS.md must accurately reflect implemented features (not aspirational)
- `docs/src/release-verification.md` documents artifact signing for users

## Agent Rules <!-- tessl-managed -->

@.tessl/RULES.md follow the [instructions](.tessl/RULES.md)

<!-- dosu:mcp:start v1 -->

## Dosu

Shared team knowledge lives in [Dosu](https://dosu.dev), via the Dosu MCP server.

- Before a task, and for any codebase or docs questions: pull context with `read_knowledge` before digging through source.
- After a task: save durable learnings with `write_knowledge`.

Missing these tools? Run `dosu setup --help` — it covers agent-assisted setup.

<!-- dosu:mcp:end -->
