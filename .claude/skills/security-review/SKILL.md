---
name: security-review
description: Security review for Rust systems code. Covers memory safety, buffer handling, unsafe code, input validation, resource exhaustion, and supply chain security.
---

# Security Review (Rust Systems Code)

> Examples use placeholder error variants (`SomeError::OutOfBounds`,
> `::InvalidOffset`, etc.) to illustrate patterns. The actual error type
> hierarchy is `LibmagicError` / `ParseError` / `EvaluationError` in
> `src/error.rs` -- check the source for the real variants. AGENTS.md and
> GOTCHAS.md are authoritative for project-specific policy.

## When to Activate

- Adding new buffer access or offset resolution code
- Handling untrusted input (magic files, target files)
- Adding or reviewing dependencies
- Modifying parser or evaluator logic
- Before releases or PRs with security-sensitive changes

## Security Checklist

### 1. Memory Safety

#### Bounds-Checked Buffer Access

```rust
// WRONG: Direct indexing can panic
let byte = buffer[offset];

// CORRECT: Bounds-checked access
let byte = *buffer.get(offset).ok_or(EvaluationError::out_of_bounds(offset))?;

// CORRECT: Slice with bounds check
let slice = buffer.get(start..end).ok_or(EvaluationError::out_of_bounds(start))?;
```

#### Safe String Operations

```rust
// WRONG: Direct slicing can panic on non-UTF-8 boundaries
let rest = &input[2..];

// CORRECT: Use strip_prefix / strip_suffix
let rest = input.strip_prefix("0x").unwrap_or(input);
```

#### Verification Steps

- [ ] All buffer access uses `.get()` with bounds checking
- [ ] No direct indexing (`buffer[i]`) on untrusted data
- [ ] String operations use `strip_prefix` / `strip_suffix` instead of
      slicing
- [ ] No panicking operations (`.unwrap()`, `.expect()`, `panic!()`) in
      library code (test code is exempt -- see
      `.claude/hookify.warn-panic-in-lib.md`)

### 2. Unsafe Code Policy

#### Deny With One Vetted Exception

`unsafe_code = "deny"` is configured as a workspace lint in
`Cargo.toml [workspace.lints]` and inherited by the package via
`[lints] workspace = true` (verify BOTH are present -- without the
inheritance line the entire lint table is inert). `lib.rs` additionally
carries `#![deny(unsafe_code)]`. Exactly one `#[allow(unsafe_code)]`
exception is sanctioned: the memmap2 `map()` call in
`src/io/mod.rs::create_memory_mapping`, which must keep its SAFETY
comment (GOTCHAS S8.2). Any other `unsafe` block is a finding.

#### Verification Steps

- [ ] `unsafe_code = "deny"` present in `Cargo.toml [workspace.lints]`
      AND `[lints] workspace = true` present in the package section
- [ ] `grep -rn 'allow(unsafe_code)' src/` returns only the vetted
      memmap2 site in `src/io/mod.rs`
- [ ] Dependencies with `unsafe` are vetted (memmap2, byteorder, nom, etc.)
- [ ] `cargo audit` passes with no vulnerabilities

### 3. Integer Safety

#### Overflow Protection

```rust
// WRONG: Can overflow silently in release builds
let offset = base + adjustment;

// CORRECT: Checked arithmetic
let offset = base.checked_add(adjustment)
    .ok_or_else(|| EvaluationError::invalid_offset(format!("{base} + {adjustment}")))?;

// CORRECT: Saturating for non-critical paths
let score = base_score.saturating_add(bonus);
```

#### Verification Steps

- [ ] Offset calculations use checked arithmetic (GOTCHAS S5.2)
- [ ] No implicit integer truncation (e.g., `u64 as u32`)
- [ ] Cast operations use `TryFrom` / `try_into()` where overflow is
      possible
- [ ] Clippy pedantic lints catch suspicious casts

### 4. Input Validation (Magic Files)

#### Parser Robustness

```rust
// Magic files are untrusted input -- strict validation required.
// Parser returns Err on invalid syntax, never panics. parse_text_magic_file
// is fail-fast: a single unparseable rule causes the whole load to fail
// (GOTCHAS S3.11).
```

#### Verification Steps

- [ ] Parser returns `Err` on invalid syntax, never panics
- [ ] Deeply nested rules have depth limits
      (`EvaluationConfig::max_recursion_depth`)
- [ ] Unrecognized directives (`!:mime`, `!:ext`, etc.) are skipped at
      preprocessing time
- [ ] Malformed offset/type/operator specifications produce clear errors
      with line numbers
- [ ] Property tests fuzz the parser with arbitrary input
      (`tests/property_tests.rs`)

### 5. Input Validation (Target Files)

#### File Buffer Safety

```rust
use libmagic_rs::io::FileBuffer;
use std::path::Path;

// Constructor is FileBuffer::new(Path::new(...)) -- see src/io/mod.rs.
let fb = FileBuffer::new(Path::new(path))?;

// Memory-mapped I/O avoids loading entire file.
// Bounds checking on every access via .get().
let data = fb.as_bytes().get(offset..offset + length);
```

#### Verification Steps

- [ ] Memory-mapped I/O used (not reading entire file into memory)
- [ ] All buffer access bounds-checked via `.get()`
- [ ] Truncated/corrupted files handled gracefully
- [ ] Zero-length files handled without errors
- [ ] Search-path / TOCTOU hardening considered for magic-file discovery
      (see `docs/src/security-assurance.md`)

### 6. Resource Exhaustion Prevention

#### CPU Limits

```rust
use libmagic_rs::EvaluationConfig;

// Use a non-default config when accepting untrusted input -- the default
// timeout is None (unbounded). See GOTCHAS S13.1.
let config = EvaluationConfig::performance()  // sets timeout_ms = Some(1000)
    .with_stop_at_first_match(true);

// Or build explicitly:
let config = EvaluationConfig::default()
    .with_timeout_ms(Some(1000))
    .with_max_recursion_depth(50);
```

Actual fields on `EvaluationConfig` (per `src/config.rs`): `timeout_ms:
Option<u64>` (milliseconds, clamped to `MAX_SAFE_TIMEOUT_MS = 5 minutes`),
`max_recursion_depth: u32`, `stop_at_first_match: bool`. The type is
`#[non_exhaustive]` -- check the source for the current full field set.

#### Verification Steps

- [ ] Library consumers handling untrusted input set a non-`None`
      `timeout_ms` (do NOT rely on `EvaluationConfig::default()` for that)
- [ ] `max_recursion_depth` set appropriately for the workload
- [ ] Regex scans are capped (`REGEX_MAX_BYTES = 8192`, GOTCHAS S2.8 --
      do not add bypass paths)
- [ ] No unbounded recursion in rule evaluation
- [ ] Stack depth limited for nested rules

### 7. Supply Chain Security

#### Dependency Audit

```bash
mise exec -- cargo audit                  # known vulnerabilities
mise exec -- cargo deny check             # license + advisory + bans
mise exec -- cargo tree --depth 2         # dependency surface
```

#### Verification Steps

- [ ] `cargo audit` clean (no known vulnerabilities)
- [ ] `cargo deny check` passes (license + bans + advisories)
- [ ] Minimal dependency surface
- [ ] `Cargo.lock` and `mise.lock` committed for reproducible builds
- [ ] All GitHub Actions pinned to SHA hashes
- [ ] Dependabot / release-plz enabled for automated updates

### 8. Error Information Leakage

#### Safe Error Messages

```rust
// WRONG: Exposes internal paths or system info verbatim
return Err(format!("Failed to read {full_path}: {system_error}").into());

// CORRECT: Wrap external errors and surface only what's actionable.
// ParseError::IoError(String) wraps I/O errors as strings because
// error.rs is shared with build.rs and cannot reference lib-only types
// (GOTCHAS S1.1).
return Err(ParseError::IoError(format!("magic file not found: {name}")));
```

#### Verification Steps

- [ ] Error messages don't expose absolute file paths to end users
- [ ] System-level errors wrapped before surfacing to CLI
- [ ] Debug output gated behind `RUST_LOG` / verbose flags

### 9. CLI Argument Safety

#### Path Handling

```rust
// clap validates arguments before they reach application code.
// No shell expansion or command injection possible.
#[derive(Parser)]
struct Args {
    /// File to identify
    file: PathBuf,

    /// Magic file to use
    #[arg(long)]
    magic_file: Option<PathBuf>,
}
```

#### Verification Steps

- [ ] CLI arguments parsed by `clap` (no manual parsing)
- [ ] File paths treated as opaque -- no string manipulation
- [ ] No shell invocation or command execution from user input
- [ ] Symlink handling considered for file access

## Pre-Release Security Checklist

- [ ] `unsafe_code = "deny"` enforced via workspace lints + `[lints] workspace = true`
- [ ] `cargo clippy -- -D warnings` passes (run via `just ci-check`)
- [ ] `cargo audit` clean
- [ ] `cargo deny check` passes
- [ ] All buffer access bounds-checked
- [ ] Integer arithmetic overflow-safe
- [ ] Property tests cover parser and evaluator
- [ ] Resource limits configured (`EvaluationConfig::timeout_ms`,
      `max_recursion_depth`)
- [ ] Error messages reviewed for information leakage
- [ ] Dependencies minimized and pinned (`Cargo.lock`, `mise.lock`)
- [ ] Sigstore attestations configured for release artifacts (see
      `docs/src/release-verification.md`)

## References

- [Rust Secure Coding Guidelines](https://anssi-fr.github.io/rust-guide/)
- [RustSec Advisory Database](https://rustsec.org/)
- Project [Security Assurance Case](../../docs/src/security-assurance.md)
- Project [SECURITY.md](../../SECURITY.md)
- Project [AGENTS.md](../../AGENTS.md) and
  [GOTCHAS.md](../../GOTCHAS.md) -- authoritative policy
