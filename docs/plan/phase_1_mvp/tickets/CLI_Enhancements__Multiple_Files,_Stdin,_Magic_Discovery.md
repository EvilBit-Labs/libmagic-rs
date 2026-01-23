# CLI Enhancements: Multiple Files, Stdin, Magic Discovery

## Overview

Enhance the CLI to support multiple file arguments, stdin input, improved magic file discovery with text-first priority, and `--use-builtin` / `--strict` flags. This completes the end-to-end user experience for Phase 1 MVP.

## Validation Findings (Implementation Validation)

**Bugs in Existing Code to Fix:**

1. **BUG: Magic file search order wrong** (`file:src/main.rs` lines 64-76)
   - Current: Binary .mgc files searched FIRST
   - Required: Text files/directories FIRST (OpenBSD approach)
   - Impact: Users will encounter unsupported binary .mgc before finding text files
   - Fix: Reorder search candidates to prioritize Magdir directories and text files

2. **BUG: create_basic_magic_content() should not exist** (`file:src/main.rs` lines 428-484)
   - Current: 483 lines of basic magic file generation code
   - Required: Remove entirely (--create-magic flag was eliminated from scope)
   - Impact: Dead code, maintenance burden
   - Fix: Delete function and all references

3. **BUG: Missing --use-builtin and --strict flags** (`file:src/main.rs` lines 23-39)
   - Current: Flags not defined in Args struct
   - Required: Add both flags per Core Flow 3 and Technical Plan
   - Impact: Users cannot use built-in rules or control exit code behavior
   - Fix: Add flags to Args struct and implement logic

## Scope

**In Scope:**

- Multiple file argument support (sequential processing)
- Stdin input support (using clap-stdin crate with `-` as file argument)
- Text-first magic file search paths (Magdir directories before .mgc files)
- `--use-builtin` flag (use built-in rules)
- `--strict` flag (exit non-zero on any failure, default: exit 0)
- Exit code handling (default: 0 even if some fail, strict: non-zero on any fail)
- JSON Lines output for multiple files
- Error handling per file (continue on failure)

**Out of Scope:**

- Progress indicators (deferred to Phase 2)
- Batch optimization (deferred to Phase 2)
- Advanced CLI features (deferred to Phase 2)

## Technical Approach

### 1. Multiple File Support

Update `file:src/main.rs` to use clap-stdin:

```rust
use clap_stdin::FileOrStdin;

#[derive(Parser)]
struct Args {
    /// File to analyze (use '-' for stdin)
    #[arg(value_name = "FILE")]
    pub files: Vec<FileOrStdin>,

    // ... existing flags ...

    /// Exit with non-zero code if any file fails
    #[arg(long)]
    pub strict: bool,
}
```

**Processing Logic:**

```rust
let mut exit_code = 0;
for file in &args.files {
    match db.evaluate_file(file) {
        Ok(result) => output_result(file, &result, &args)?,
        Err(e) => {
            eprintln!("{}: Error: {}", file.display(), e);
            exit_code = 3;
            if args.strict {
                std::process::exit(exit_code);
            }
        }
    }
}
std::process::exit(exit_code);
```

### 2. Stdin Support

Using clap-stdin's FileOrStdin:

```rust
for file_or_stdin in &args.files {
    match file_or_stdin {
        FileOrStdin::Stdin => {
    let mut buffer = Vec::new();
    let max_size = config.max_string_length;

    io::stdin()
        .take(max_size as u64)
        .read_to_end(&mut buffer)?;

    if buffer.len() >= max_size {
        eprintln!("Warning: Stdin truncated at {} bytes", max_size);
    }

    let result = db.evaluate_buffer(&buffer)?;
    output_result("stdin", &result, &args)?;
    return Ok(());
}
```

### 3. Text-First Magic Discovery

Update `Args::default_magic_file_path()`:

```rust
let candidates = [
    // Text files/directories FIRST (OpenBSD approach)
    "/usr/share/file/magic/Magdir",
    "/usr/share/misc/magic",
    "/usr/local/share/misc/magic",

    // Binary .mgc files LAST (show helpful error)
    "/usr/share/file/magic.mgc",
    "/usr/local/share/misc/magic.mgc",
];
```

### 4. Built-in Rules Flag

```rust
#[arg(long)]
pub use_builtin: bool,

#[arg(long)]
pub strict: bool,

// In main():
let db = if args.use_builtin {
    MagicDatabase::with_builtin_rules()
} else if let Some(ref magic_file) = args.magic_file {
    MagicDatabase::load_from_file(magic_file)?
} else {
    discover_and_load_magic_database()?
};
```

### 5. Exit Code Handling

```rust
let mut exit_code = 0;
for file in &args.files {
    match db.evaluate_file(file) {
        Ok(result) => output_result(file, &result, &args)?,
        Err(e) => {
            eprintln!("{}: Error: {}", file.display(), e);
            if args.strict {
                std::process::exit(3);
            }
            exit_code = 3;
        }
    }
}
if args.strict && exit_code != 0 {
    std::process::exit(exit_code);
}
std::process::exit(0); // Default: exit 0 even if some failed
```

### 6. Removed: Create Magic Flag

```rust
#[arg(long)]
pub create_magic: bool,

// In discover_and_load_magic_database():
if args.create_magic {
    let magic_path = default_magic_file_path();
    if let Err(e) = create_basic_magic_file(&magic_path) {
        eprintln!("Failed to create magic file: {}", e);
        eprintln!("Suggestion: Use --use-builtin instead");
        std::process::exit(4);
    }
    return MagicDatabase::load_from_file(&magic_path);
}
```

### 6. JSON Lines Output

```rust
fn output_json(filename: &str, result: &EvaluationResult) -> Result<()> {
    let json = serde_json::json!({
        "filename": filename,
        "matches": [...],
        "metadata": {...}
    });
    println!("{}", serde_json::to_string(&json)?);
    Ok(())
}
```

## Acceptance Criteria

- [ ] CLI accepts multiple file arguments
- [ ] Each file processed sequentially with immediate output
- [ ] Stdin input works with `-` or `--stdin`
- [ ] Stdin respects `max_string_length` limit
- [ ] Text magic files searched before binary .mgc
- [ ] `--use-builtin` flag loads built-in rules
- [ ] `--strict` flag exits non-zero on any failure
- [ ] Default behavior: exit 0 even if some files fail (GNU file compatible)
- [ ] Per-file timeout: each file gets full timeout duration
- [ ] Timeout doesn't stop processing of remaining files
- [ ] Stdin uses clap-stdin FileOrStdin pattern
- [ ] Stdin respects max_string_length as buffer size limit
- [ ] --strict with --use-builtin: "data" result is not an error
- [ ] Rustdoc added for all new CLI flags
- [ ] JSON Lines format for multiple files
- [ ] Text format: one line per file
- [ ] Error messages show filename context
- [ ] Unit tests for all new flags
- [ ] Integration tests for multiple files
- [ ] Integration test for stdin

## Dependencies

- **Hard Dependency**: ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/c554e409-ae60-407f-9596-64c5b03a9b92 (Parser Integration) - Required for magic file loading
- **Soft Dependency**: ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/5fa9fb36-a0bf-4d1b-924a-41a0a39f126e (Built-in Rules) - Can implement with stub initially, complete when built-in rules are ready

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/36539700-862d-4fdf-9c79-3c36390f6aa8 (Core Flows - Flows 1, 2, 3, 10)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - CLI Enhancements)

## Files to Modify

- `file:src/main.rs` - Add multiple file support, stdin, flags, discovery logic
- `file:tests/cli_integration_tests.rs` - Add integration tests
