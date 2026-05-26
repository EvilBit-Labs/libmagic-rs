---
name: security-review
description: Security review for Rust systems code. Covers memory safety, buffer handling, unsafe code, input validation, resource exhaustion, and supply chain security.
---

# Security Review (Rust Systems Code)

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
let byte = buffer.get(offset).ok_or(MagicError::OutOfBounds)?;

// CORRECT: Slice with bounds check
let slice = buffer.get(start..end).ok_or(MagicError::OutOfBounds)?;
```

#### Safe String Operations

```rust
// WRONG: Direct slicing can panic on non-UTF-8 boundaries
let rest = &input[2..];

// CORRECT: Use strip_prefix/strip_suffix
let rest = input.strip_prefix("0x").unwrap_or(input);
```

#### Verification Steps

- [ ] All buffer access uses `.get()` with bounds checking
- [ ] No direct indexing (`buffer[i]`) on untrusted data
- [ ] String operations use `strip_prefix`/`strip_suffix` instead of slicing
- [ ] No panicking operations (`.unwrap()`, `.expect()`, `panic!()`) in library code

### 2. Unsafe Code Policy

#### Zero Tolerance

```rust
// This is enforced project-wide
#![forbid(unsafe_code)]
```

#### Verification Steps

- [ ] `#![forbid(unsafe_code)]` present in `lib.rs`
- [ ] No `unsafe` blocks anywhere in project source
- [ ] Dependencies with `unsafe` are vetted (memmap2, byteorder, nom)
- [ ] `cargo audit` passes with no vulnerabilities

### 3. Integer Safety

#### Overflow Protection

```rust
// WRONG: Can overflow silently in release builds
let offset = base + adjustment;

// CORRECT: Checked arithmetic
let offset = base.checked_add(adjustment)
    .ok_or(MagicError::InvalidOffset { offset: format!("{} + {}", base, adjustment) })?;

// CORRECT: Saturating for non-critical paths
let score = base_score.saturating_add(bonus);
```

#### Verification Steps

- [ ] Offset calculations use checked arithmetic
- [ ] No implicit integer truncation (e.g., `u64 as u32`)
- [ ] Cast operations use `TryFrom`/`try_into()` where overflow is possible
- [ ] Clippy pedantic lints catch suspicious casts

### 4. Input Validation (Magic Files)

#### Parser Robustness

```rust
// Magic files are untrusted input -- strict validation required
fn parse_magic_line(line: &str) -> Result<MagicRule, ParseError> {
    // Validate line structure before parsing
    // Return ParseError on invalid syntax, never panic
    // Skip unrecognized directives gracefully
}
```

#### Verification Steps

- [ ] Parser returns `Err` on invalid syntax, never panics
- [ ] Deeply nested rules have depth limits
- [ ] Unrecognized directives are skipped with warnings
- [ ] Malformed offset/type/operator specifications produce clear errors
- [ ] Property tests fuzz the parser with arbitrary input

### 5. Input Validation (Target Files)

#### File Buffer Safety

```rust
// All file access through FileBuffer with bounds checking
let fb = FileBuffer::open(path)?;

// Size limits prevent resource exhaustion
if fb.len() > MAX_FILE_SIZE {
    return Err(MagicError::FileTooLarge);
}

// Memory-mapped I/O avoids loading entire file
// Bounds checking on every access
let data = fb.get(offset, length)?;
```

#### Verification Steps

- [ ] File size limits enforced before processing
- [ ] Memory-mapped I/O used (not reading entire file into memory)
- [ ] All `FileBuffer` access is bounds-checked
- [ ] Truncated/corrupted files handled gracefully
- [ ] Zero-length files handled without errors

### 6. Resource Exhaustion Prevention

#### CPU Limits

```rust
// Evaluation timeout prevents infinite loops
let config = EvaluationConfig {
    timeout: Duration::from_secs(5),
    max_rules: 10_000,
    ..Default::default()
};
```

#### Memory Limits

```rust
// Limit collected results to prevent unbounded growth
const MAX_MATCHES: usize = 100;
if matches.len() >= MAX_MATCHES {
    break;
}
```

#### Verification Steps

- [ ] Evaluation has configurable timeout
- [ ] Maximum rule evaluation count enforced
- [ ] Match results bounded
- [ ] No unbounded recursion in rule evaluation
- [ ] Stack depth limited for nested rules

### 7. Supply Chain Security

#### Dependency Audit

```bash
# Check for known vulnerabilities
cargo audit

# Check license compliance
cargo deny check

# Review dependency tree
cargo tree --depth 2
```

#### Verification Steps

- [ ] `cargo audit` clean (no known vulnerabilities)
- [ ] `cargo deny` passes (license compliance)
- [ ] Minimal dependency surface
- [ ] `Cargo.lock` committed for reproducible builds
- [ ] All GitHub Actions pinned to SHA hashes
- [ ] Dependabot enabled for automated updates

### 8. Error Information Leakage

#### Safe Error Messages

```rust
// WRONG: Exposes internal paths or system info
Err(format!("Failed to read {}: {}", full_path, system_error))

// CORRECT: Actionable but not leaking
Err(MagicError::IoError(std::io::Error::new(
    std::io::ErrorKind::NotFound,
    "magic file not found"
)))
```

#### Verification Steps

- [ ] Error messages don't expose absolute file paths to end users
- [ ] System-level errors wrapped before surfacing to CLI
- [ ] Debug output gated behind `RUST_LOG` / verbose flags

### 9. CLI Argument Safety

#### Path Handling

```rust
// clap validates arguments before they reach application code
// No shell expansion or command injection possible
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

- [ ] `#![forbid(unsafe_code)]` enforced
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo audit` clean
- [ ] `cargo deny check` passes
- [ ] All buffer access bounds-checked
- [ ] Integer arithmetic overflow-safe
- [ ] Property tests cover parser and evaluator
- [ ] Resource limits configured
- [ ] Error messages reviewed for information leakage
- [ ] Dependencies minimized and pinned
- [ ] Sigstore attestations configured for release artifacts

## References

- [Rust Secure Coding Guidelines](https://anssi-fr.github.io/rust-guide/)
- [RustSec Advisory Database](https://rustsec.org/)
- Project [Security Assurance Case](../../docs/src/security-assurance.md)
- Project [SECURITY.md](../../SECURITY.md)
