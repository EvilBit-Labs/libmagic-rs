# Project Structure

## Recommended Organization

```text
libmagic-rs/
├── Cargo.toml              # Project manifest and dependencies
├── Cargo.lock              # Dependency lock file
├── README.md               # Project documentation
├── LICENSE                 # License file
├── src/
│   ├── lib.rs              # Library root and public API
│   ├── main.rs             # CLI binary entry point
│   ├── parser/
│   │   ├── mod.rs          # Magic file parser module
│   │   ├── ast.rs          # Abstract Syntax Tree definitions
│   │   └── grammar.rs      # Grammar rules (nom/pest)
│   ├── evaluator/
│   │   ├── mod.rs          # Rule evaluation engine
│   │   ├── offset.rs       # Offset resolution logic
│   │   ├── types.rs        # Type interpretation (byte, short, etc.)
│   │   └── operators.rs    # Comparison operators
│   ├── output/
│   │   ├── mod.rs          # Output formatting
│   │   ├── text.rs         # Human-readable output
│   │   └── json.rs         # JSON structured output
│   └── error.rs            # Error types and handling
├── tests/
│   ├── integration/        # Integration tests
│   ├── fixtures/           # Test files and magic databases
│   └── corpus/             # Sample files for testing
├── benches/                # Performance benchmarks
├── magic/                  # Magic file databases
│   ├── standard.magic      # Standard magic rules
│   └── custom/             # Custom rule sets
└── docs/                   # Additional documentation
```

## Module Responsibilities

### Core Data Types (`src/lib.rs`)

- Public API exports
- Core data structures: `MagicRule`, `Value`, `TypeKind`, `Operator`, `OffsetSpec`
- Main library interface

### Parser Module (`src/parser/`)

- **ast.rs**: AST node definitions matching the spec
- **grammar.rs**: Magic file DSL parsing logic
- **mod.rs**: Parser interface and coordination

### Evaluator Module (`src/evaluator/`)

- **offset.rs**: Resolve absolute, indirect, and relative offsets
- **types.rs**: Interpret bytes according to TypeKind (endianness handling)
- **operators.rs**: Apply comparison and bitwise operations
- **mod.rs**: Main evaluation engine and matching algorithm

### Output Module (`src/output/`)

- **text.rs**: Format results like GNU `file` command
- **json.rs**: Structured JSON output with metadata
- **mod.rs**: Output format selection and coordination

## Naming Conventions

- **Files**: snake_case (e.g., `magic_rule.rs`)
- **Types**: PascalCase (e.g., `MagicRule`, `TypeKind`)
- **Functions**: snake_case (e.g., `resolve_offset`, `evaluate_rule`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_BUFFER_SIZE`)
- **Modules**: snake_case (e.g., `evaluator`, `output`)

## Testing Strategy

- **Unit tests**: Alongside source files (`#[cfg(test)]`)
- **Integration tests**: In `tests/` directory
- **Fixtures**: Sample files and expected outputs in `tests/fixtures/`
- **Benchmarks**: Performance tests in `benches/`

## Configuration Files

- **Cargo.toml**: Dependencies, metadata, and feature flags
- **magic/**: Magic rule databases (text format)
- **.gitignore**: Exclude target/, compiled artifacts
- **rustfmt.toml**: Code formatting rules (if customized)

## Development Phases

- **MVP**: Focus on `src/parser/` and basic `src/evaluator/`
- **v0.2**: Complete offset resolution and type handling
- **v0.3**: Performance optimizations and extended output formats
- **v1.0**: Full feature parity and stable API
