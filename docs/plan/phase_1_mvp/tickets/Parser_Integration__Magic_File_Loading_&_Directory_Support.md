# Parser Integration: Magic File Loading & Directory Support

## Overview

Implement magic file loading in `MagicDatabase::load_from_file()` by integrating the existing text parser with format detection, directory loading (Magdir pattern), and text-first search priority. This ticket connects the complete parser to the database layer.

## Scope

**In Scope:**

- Format detection (text file, directory, binary .mgc)
- Text magic file parsing integration (parser already complete)
- Directory loading (Magdir pattern) - read all files, merge rules
- Text-first search paths (Magdir directories before .mgc files)
- Error handling for binary .mgc with helpful message directing to --use-builtin
- Per-file error handling:
  - Critical errors (I/O, encoding): fail immediately
  - Non-critical errors (individual rule syntax): warn and continue

**Out of Scope:**

- Binary .mgc parsing (deferred to Phase 2)
- Strength calculation (separate ticket)
- Built-in rules compilation (separate ticket)
- MIME mapping (separate ticket)

## Technical Approach

### 1. Format Detection

Add to `file:src/parser/mod.rs`:

```rust
pub enum MagicFileFormat {
    Text,
    Directory,
    Binary,
}

pub fn detect_format<P: AsRef<Path>>(path: P) -> Result<MagicFileFormat>
```

**Logic:**

- Check if path is directory → `Directory`
- Read first 4 bytes, check for binary magic number `0xF11E041C` → `Binary`
- Otherwise → `Text`

### 2. Directory Loading

Add to `file:src/parser/mod.rs`:

```rust
pub fn load_magic_directory<P: AsRef<Path>>(dir: P) -> Result<Vec<MagicRule>>
```

**Logic:**

- Iterate through directory entries
- Parse each file with existing `parse_text_magic_file()`
- Collect critical errors (return immediately)
- Collect non-critical errors (warn, continue)
- Merge all rules maintaining order

### 3. Public API

Add to `file:src/parser/mod.rs`:

```rust
pub fn load_magic_file<P: AsRef<Path>>(path: P) -> Result<Vec<MagicRule>> {
    match detect_format(&path)? {
        MagicFileFormat::Text => parse_text_magic_file(path),
        MagicFileFormat::Directory => load_magic_directory(path),
        MagicFileFormat::Binary => Err(Error::UnsupportedFormat {
            path: path.as_ref().to_path_buf(),
            message: BINARY_MGC_ERROR_MESSAGE,
        }),
    }
}
```

### 4. MagicDatabase Integration

Update `file:src/lib.rs`:

```rust
impl MagicDatabase {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let rules = parser::load_magic_file(path)?;
        Ok(Self {
            rules,
            config: EvaluationConfig::default(),
        })
    }
}
```

### 5. Binary .mgc Error Message

```
Error: Binary magic file format not supported in Phase 1 MVP

Found: /usr/share/file/magic.mgc (binary format)

This version of libmagic-rs supports text-format magic files only.

Options:
  --use-builtin       Use built-in rules for common file types
  --create-magic      Create a basic text magic file
  --magic-file PATH   Specify a text magic file location

Text magic files are typically located at:
  - /usr/share/file/magic/Magdir/* (directory of files)
  - /usr/share/misc/magic (single file)
  - Download from: https://github.com/file/file/tree/master/magic/Magdir

Example: rmagic --use-builtin sample.bin
```

## Acceptance Criteria

- [ ] `MagicDatabase::load_from_file()` successfully loads text magic files
- [ ] Directory loading works with `/usr/share/file/magic/Magdir/` pattern
- [ ] Binary .mgc files show helpful error message (not crash)
- [ ] Critical parse errors (I/O, encoding) fail immediately with context
- [ ] Non-critical parse errors (rule syntax) warn and continue
- [ ] MagicDatabase stores magic file path for metadata
- [ ] Rustdoc added for all new functions and types
- [ ] All rules from directory are merged in correct order
- [ ] Unit tests for format detection
- [ ] Unit tests for directory loading
- [ ] Integration test with real Magdir directory
- [ ] Error message test for binary .mgc

## Dependencies

None - this is the foundation ticket

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - Parser Module)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/36539700-862d-4fdf-9c79-3c36390f6aa8 (Core Flows - Flow 3)

## Files to Modify

- `file:src/parser/mod.rs` - Add format detection, directory loading, public API
- `file:src/lib.rs` - Update `MagicDatabase::load_from_file()`
- `file:src/error.rs` - Add `UnsupportedFormat` error variant if needed
- `file:tests/` - Add integration tests
