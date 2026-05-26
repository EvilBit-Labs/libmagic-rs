---
name: libmagic-rs-patterns
description: Project-specific conventions for libmagic-rs (commit style, error patterns, testing layout, tooling)
source: AGENTS.md + GOTCHAS.md
---

# libmagic-rs Development Patterns

> Authoritative architectural state lives in [AGENTS.md](../../AGENTS.md) and
> [GOTCHAS.md](../../GOTCHAS.md). This skill captures only the stable patterns
> that don't drift -- commit conventions, error/testing style, and tooling.
> When in doubt, AGENTS.md wins.

## Commit Conventions

Conventional commits required. Types in use:

- `feat:` -- new features
- `fix:` -- bug fixes
- `refactor:` -- restructuring without behavior change
- `chore:` (incl. `chore(deps):`, `chore(ci):`) -- maintenance
- `docs:` -- documentation
- `test:` -- test additions or changes
- `perf:`, `ci:` -- as relevant

Every commit must be signed off with `git commit -s` (DCO). The project's
documented best practice is `git commit -sS -m "..."` (DCO + GPG together).
PR references in messages use `(#N)` suffix format.

## Architecture

See AGENTS.md sections "Architecture Patterns" and "Module Organization"
for the authoritative module tree. The parser splits across
`parser/{ast,grammar/,types,codegen,...}` and the evaluator splits across
`evaluator/{engine/,types/,operators/,offset/,strength,...}` -- both are
directory modules, not flat files.

### Pipeline

```
Magic File -> Parser -> AST -> Evaluator -> Match Results -> Output Formatter
                                  ^
Target File -> Memory Mapper -> File Buffer
```

### Core Types

- `MagicDatabase` -- public entry point (load rules, evaluate buffers/files)
- `MagicRule` -- a single rule (`#[non_exhaustive]`; do not literal-construct
  outside the crate)
- `OffsetSpec`, `TypeKind`, `Operator`, `Value` -- AST nodes in
  `src/parser/ast.rs`; variant lists evolve, check the source
- `LibmagicError` / `ParseError` / `EvaluationError` -- error hierarchy in
  `src/error.rs`

## Clippy Configuration

Strict workspace lints in `Cargo.toml [workspace.lints]`:

- `unsafe_code = "forbid"` -- enforced project-wide via workspace lints, not
  via a `#![forbid(unsafe_code)]` attribute
- `warnings = "deny"` -- zero-warnings policy
- `unwrap_used = "deny"`, `expect_used = "deny"` in library code
- `panic` is also denied where possible; see `Cargo.toml` for the exception
  note (it would be forbidden but breaks clap)
- `pedantic`, `nursery`, `cargo` groups enabled
- Security-focused: `indexing_slicing`, `arithmetic_side_effects`,
  `string_slice`

## Error Handling Pattern

`thiserror`-based enums with named constructor methods:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid syntax at line {line}: {message}")]
    InvalidSyntax { line: usize, message: String },
}

impl ParseError {
    #[must_use]
    pub fn invalid_syntax(line: usize, message: impl Into<String>) -> Self {
        Self::InvalidSyntax { line, message: message.into() }
    }
}
```

- Every variant has a constructor method
- Constructors use `impl Into<String>` for ergonomics
- All constructors `#[must_use]`
- Error messages include contextual info (line numbers, offsets, paths)

See GOTCHAS S1.1 for the build-script boundary: `src/error.rs` is shared
with `build.rs` and cannot reference lib-only types.

## Testing Patterns

### Organization

- **Unit tests** -- `#[cfg(test)] mod tests` alongside source files
- **Integration tests** -- `tests/*.rs`
- **Compatibility tests** -- `tests/compatibility_tests.rs` against the
  upstream `file` test suite
- **Property tests** -- `tests/property_tests.rs` using `proptest`
- **Benchmarks** -- `benches/*.rs` using `criterion`
- **Snapshots** -- `insta`

`.unwrap()` and `.expect()` are denied in library code but acceptable
inside `#[cfg(test)]` modules and `tests/*.rs` (see
`.claude/hookify.warn-panic-in-lib.md`).

### Commands (run via `mise exec --`)

```bash
mise exec -- cargo nextest run --workspace --no-fail-fast
mise exec -- just ci-check        # full pre-commit parity
mise exec -- just coverage        # coverage report
mise exec -- just test-compatibility
```

## Build Script Pattern

Build logic extracted into a testable library module so build-time failures
have proper test coverage:

- `src/build_helpers.rs` -- testable parsing and code generation
- `build.rs` -- minimal entry point that delegates to `build_helpers`

See AGENTS.md "Testing Build Scripts" for the rationale.

## Tooling

- **mise** -- toolchain manager (pinned Rust + dev tools)
- **just** -- task runner
- **cargo-nextest** -- fast test runner
- **cargo-llvm-cov** -- coverage
- **insta** -- snapshot testing
- **criterion** -- benchmarking
- **pre-commit** -- git hooks (incl. `mdformat`, `shellcheck`, `actionlint`,
  `cargo-machete`, `cargo-audit`)
- **mdbook** -- documentation site under `docs/`
- **cargo-dist** -- release pipeline (`.github/workflows/release.yml` is
  auto-generated -- never edit by hand)

## Rust Edition & Style

- **Edition**: 2024
- **MSRV**: see `Cargo.toml` `rust-version` (don't pin from memory; check
  the file)
- **rustfmt**: `style_edition = "2024"`
- Public API types derive `Debug, Clone, Serialize, Deserialize, PartialEq`
  where shape allows (`Value` does not derive `Eq` because it holds `f64`
  -- see GOTCHAS S2.3)
- Doc comments on all public items, with examples that pass `cargo test --doc`
- `#[non_exhaustive]` on public enums and many structs -- do not literal-
  construct from outside the crate

## Safety Rules

The hookify warn rules (`.claude/hookify.warn-*.md`) cover the day-to-day
discipline. Headline rules:

1. Use `.get()` for buffer access, never direct indexing
2. Use `strip_prefix()` / `strip_suffix()` instead of `&str[n..]` (avoids
   UTF-8 boundary panics)
3. No `unwrap()` / `expect()` / `panic!` in library code (test code OK)
4. Bounds-check all byte reads
5. Configurable timeouts on evaluation (`EvaluationConfig::timeout_ms`)
6. No emojis or non-ASCII in code/comments except for em-dash, en-dash,
   and similar symbols where the code is handling non-plaintext data
