# Migration from libmagic

This guide helps you migrate from the C-based libmagic library to libmagic-rs, covering API differences, compatibility considerations, and best practices.

## API Comparison

### C libmagic API

```c
#include <magic.h>

magic_t magic = magic_open(MAGIC_MIME_TYPE);
magic_load(magic, NULL);
const char* result = magic_file(magic, "example.bin");
printf("MIME type: %s\n", result);
magic_close(magic);
```

### libmagic-rs API

```rust
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::load_from_file("magic.db")?;
let result = db.evaluate_file("example.bin")?;
println!("File type: {}", result.description);
```

## Key Differences

### Memory Safety

- **C libmagic**: Manual memory management, potential for leaks/corruption
- **libmagic-rs**: Automatic memory management, compile-time safety guarantees

### Error Handling

- **C libmagic**: Error codes and global error state
- **libmagic-rs**: Result types with detailed error information

### Thread Safety

- **C libmagic**: Requires careful synchronization
- **libmagic-rs**: Thread-safe by design (when complete)

## Migration Strategies

### Direct Replacement

For simple use cases, libmagic-rs can be a drop-in replacement:

```rust
// Before (C)
// const char* type = magic_file(magic, path);

// After (Rust)
let result = db.evaluate_file(path)?;
let type_str = &result.description;
```

### Gradual Migration

For complex applications:

1. **Start with new code**: Use libmagic-rs for new features
2. **Wrap existing code**: Create Rust wrappers around C libmagic calls
3. **Replace incrementally**: Migrate modules one at a time
4. **Remove C dependency**: Complete the migration

## Compatibility Notes

### Magic File Format

- **Supported**: Standard magic file syntax
- **Extensions**: Additional features planned (regex, etc.)
- **Compatibility**: Existing magic files should work

### Output Format

- **Text mode**: Compatible with GNU `file` command
- **JSON mode**: New structured format for modern applications
- **MIME types**: Similar to `file --mime-type`

### Performance

- **Memory usage**: Comparable to C libmagic
- **Speed**: Target within 10% of C performance
- **Startup**: Faster with compiled rule caching

## Common Migration Issues

### Error Handling Patterns

**C libmagic:**

```c
if (magic_load(magic, NULL) != 0) {
    fprintf(stderr, "Error: %s\n", magic_error(magic));
    return -1;
}
```

**libmagic-rs:**

```rust
let db = match MagicDatabase::load_from_file("magic.db") {
    Ok(db) => db,
    Err(e) => {
        eprintln!("Error: {}", e);
        return Err(e);
    }
};
```

### Resource Management

**C libmagic:**

```c
magic_t magic = magic_open(flags);
// ... use magic ...
magic_close(magic);  // Manual cleanup required
```

**libmagic-rs:**

```rust
{
    let db = MagicDatabase::load_from_file("magic.db")?;
    // ... use db ...
}  // Automatic cleanup when db goes out of scope
```

## Best Practices

### Error Handling

- Use `?` operator for error propagation
- Match on specific error types when needed
- Provide context with error messages

### Performance

- Reuse `MagicDatabase` instances when possible
- Consider caching for frequently accessed files
- Use appropriate configuration for your use case

### Testing

- Test with your existing magic files
- Verify output compatibility with your applications
- Benchmark performance for your workload

## Future Compatibility

libmagic-rs aims to maintain compatibility with:

- **Standard magic file format**: Core syntax will remain supported
- **GNU file output**: Text output format compatibility
- **Common use cases**: Drop-in replacement for most applications

## Getting Help

If you encounter migration issues:

- Check the [troubleshooting guide](./troubleshooting.md)
- Search [existing issues](https://github.com/EvilBit-Labs/libmagic-rs/issues)
- Ask questions in [discussions](https://github.com/EvilBit-Labs/libmagic-rs/discussions)
- Report bugs with minimal reproduction cases
