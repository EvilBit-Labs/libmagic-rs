# Troubleshooting

Common issues and solutions when using libmagic-rs.

## Installation Issues

### Rust Version Compatibility

**Problem**: Build fails with older Rust versions

```
error: package `libmagic-rs v0.1.0` cannot be built because it requires rustc 1.85 or newer
```

**Solution**: Update Rust to version 1.85 or newer

```bash
rustup update stable
rustc --version  # Should show 1.85+
```

### Dependency Conflicts

**Problem**: Cargo fails to resolve dependencies

```
error: failed to select a version for the requirement `serde = "^1.0"`
```

**Solution**: Clean and rebuild

```bash
cargo clean
rm Cargo.lock
cargo build
```

## Runtime Issues

### Magic File Loading Errors

**Problem**: Cannot load magic file

```
Error: Parse error at line 42: Invalid offset specification
```

**Solutions**:

1. **Check file path**: Ensure the magic file exists and is readable
2. **Validate syntax**: Check the magic file format at the specified line
3. **Use absolute paths**: Relative paths may not resolve correctly

```rust
// Use absolute path
let db = MagicDatabase::load_from_file("/usr/share/misc/magic")?;

// Or check if file exists first
use std::path::Path;
let magic_path = "magic.db";
if !Path::new(magic_path).exists() {
    eprintln!("Magic file not found: {}", magic_path);
    return;
}
```

### File Evaluation Errors

**Problem**: File analysis fails

```
Error: IO error: Permission denied (os error 13)
```

**Solutions**:

1. **Check permissions**: Ensure the file is readable
2. **Handle missing files**: Check if file exists before analysis
3. **Use proper error handling**: Match specific error types

```rust
use libmagic_rs::LibmagicError;

match db.evaluate_file("example.bin") {
    Ok(result) => println!("Type: {}", result.description),
    Err(LibmagicError::IoError(e)) => {
        eprintln!("Cannot access file: {}", e);
    }
    Err(e) => eprintln!("Analysis failed: {}", e),
}
```

## Performance Issues

### Slow File Analysis

**Problem**: File analysis takes too long

**Solutions**:

1. **Optimize configuration**: Reduce recursion depth and string length limits
2. **Use early termination**: Stop at first match for faster results
3. **Check file size**: Large files may need special handling

```rust
let fast_config = EvaluationConfig {
    max_recursion_depth: 5,
    max_string_length: 512,
    stop_at_first_match: true,
};

let result = db.evaluate_file_with_config("large_file.bin", &fast_config)?;
```

### Memory Usage Issues

**Problem**: High memory consumption

**Solutions**:

1. **Use memory mapping**: Avoid loading entire files into memory
2. **Limit string lengths**: Reduce max_string_length in configuration
3. **Process files individually**: Don't keep multiple databases in memory

```rust
// Process files one at a time
for file_path in file_list {
    let result = db.evaluate_file(&file_path)?;
    println!("{}: {}", file_path, result.description);
    // Result is dropped here, freeing memory
}
```

## Development Issues

### Compilation Errors

**Problem**: Clippy warnings treated as errors

```
error: this expression creates a reference which is immediately dereferenced
```

**Solution**: Fix clippy warnings or temporarily allow them for development

```rust
#[allow(clippy::needless_borrow)]
fn development_function() {
    // Temporary code
}
```

**Better solution**: Fix the underlying issue

```rust
// Instead of
let result = function(&value);

// Use
let result = function(value);
```

### Test Failures

**Problem**: Tests fail on different platforms

**Solutions**:

1. **Check file paths**: Use platform-independent path handling
2. **Handle endianness**: Test both little and big-endian scenarios
3. **Use conditional compilation**: Platform-specific test cases

```rust
#[cfg(target_endian = "little")]
#[test]
fn test_little_endian_parsing() {
    // Little-endian specific test
}

#[cfg(target_endian = "big")]
#[test]
fn test_big_endian_parsing() {
    // Big-endian specific test
}
```

## Magic File Issues

### Syntax Errors

**Problem**: Magic file parsing fails

```
Parse error at line 15: Expected operator, found 'invalid'
```

**Solutions**:

1. **Check syntax**: Verify magic file format
2. **Use comments**: Add comments to document complex rules
3. **Test incrementally**: Add rules one at a time

```text
# Good magic file syntax
0    string    \x7fELF    ELF executable
>4   byte      1          32-bit
>4   byte      2          64-bit

# Bad syntax (missing operator)
0    string    \x7fELF    # Missing value
```

### Encoding Issues

**Problem**: String matching fails with non-ASCII content

**Solutions**:

1. **Use byte sequences**: For binary data, use hex escapes
2. **Specify encoding**: Use appropriate string types
3. **Test with sample files**: Verify rules work with real data

```text
# Use hex escapes for binary data
0    string    \x7f\x45\x4c\x46    ELF

# Use quotes for text with spaces
0    string    "#!/bin/bash"        Bash script
```

## Debugging Tips

### Enable Logging

```bash
# Set log level for debugging
RUST_LOG=debug cargo run -- example.bin
RUST_LOG=libmagic_rs=trace cargo test
```

### Use Debug Output

```rust
// Print debug information
println!("Evaluating rule: {:?}", rule);
println!("Buffer slice: {:?}", &buffer[offset..offset + length]);
```

### Minimal Reproduction

When reporting issues:

1. **Create minimal example**: Simplest code that reproduces the problem
2. **Include sample files**: Provide test files that trigger the issue
3. **Specify environment**: OS, Rust version, dependency versions

```rust
// Minimal reproduction example
use libmagic_rs::MagicDatabase;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = MagicDatabase::load_from_file("simple.magic")?;
    let result = db.evaluate_file("test.bin")?;
    println!("Result: {}", result.description);
    Ok(())
}
```

## Getting Help

### Check Documentation

- [API Reference](./api-reference.md)
- [Architecture Overview](./architecture.md)
- [Development Guide](./development.md)

### Search Existing Issues

- [GitHub Issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- [GitHub Discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)

### Report New Issues

When creating an issue, include:

- **Rust version**: `rustc --version`
- **Library version**: From `Cargo.toml`
- **Operating system**: OS and version
- **Minimal reproduction**: Smallest example that shows the problem
- **Expected behavior**: What should happen
- **Actual behavior**: What actually happens
- **Error messages**: Complete error output

### Community Support

- **Discussions**: Ask questions and share ideas
- **Discord/IRC**: Real-time community chat (if available)
- **Stack Overflow**: Tag questions with `libmagic-rs`

This troubleshooting guide covers the most common issues. For specific problems not covered here, please check the existing issues or create a new one with detailed information.
