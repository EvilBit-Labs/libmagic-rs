# Built-in Rules: Build-Time Compilation & Fallback

## Overview

Implement build-time compilation of built-in magic rules using `build.rs`, providing fallback rules for common file types (ELF, PE, ZIP, JPEG, PNG, PDF, GIF) that work without external magic files. This enables `--use-builtin` and `--create-magic` functionality.

## Scope

**In Scope:**
- Create `file:src/builtin_rules.magic` with common file type patterns
- Build script (`build.rs`) to parse and compile rules at build time
- Build-time validation with clear error messages
- `MagicDatabase::with_builtin_rules()` API

**Out of Scope:**
- Comprehensive magic rules (only common types)
- Runtime rule compilation
- Dynamic rule loading

## Technical Approach

### 1. Magic File Source

Create `file:src/builtin_rules.magic`:

```magic
# Built-in fallback rules for common file types

# ELF executables
0       string  \x7fELF         ELF
>4      byte    1               32-bit
>4      byte    2               64-bit
>5      byte    1               LSB
>5      byte    2               MSB

# PE executables
0       string  MZ              DOS/Windows executable
>0x3c   lelong  <0x40000000
>>0x3c  lelong  >0
>>>0x3c lelong  x               PE

# ZIP archives
0       string  PK\003\004      ZIP archive

# JPEG images
0       string  \xff\xd8\xff    JPEG image

# PNG images
0       string  \x89PNG\r\n\x1a\n   PNG image

# PDF documents
0       string  %PDF-           PDF document

# GIF images
0       string  GIF8            GIF image
```

### 2. Build Script

Create `file:build.rs`:

```rust
use std::fs;
use std::path::Path;

fn main() {
    let magic_text = include_str!("src/builtin_rules.magic");
    
    // Parse at build time
    match parse_magic_text(magic_text) {
        Ok(rules) => {
            // Generate Rust code with AST structures
            let code = generate_builtin_rules_code(&rules);
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let dest_path = Path::new(&out_dir).join("builtin_rules.rs");
            fs::write(&dest_path, code).unwrap();
        }
        Err(e) => {
            eprintln!("ERROR: Failed to parse built-in magic rules");
            eprintln!("File: src/builtin_rules.magic");
            eprintln!("Error: {}", e);
            eprintln!("\nBuilt-in rules must be valid magic file syntax.");
            eprintln!("Please fix the syntax errors and rebuild.");
            std::process::exit(1);
        }
    }
}
```

### 3. Built-in Rules Module

Create `file:src/builtin_rules.rs`:

```rust
include!(concat!(env!("OUT_DIR"), "/builtin_rules.rs"));

pub fn get_builtin_rules() -> Vec<MagicRule> {
    BUILTIN_RULES.clone()
}
```

### 4. MagicDatabase API

Add to `file:src/lib.rs`:

```rust
impl MagicDatabase {
    pub fn with_builtin_rules() -> Self {
        Self {
            rules: builtin_rules::get_builtin_rules(),
            config: EvaluationConfig::default(),
        }
    }
}
```


## Acceptance Criteria

- [ ] `builtin_rules.magic` contains rules for common file types
- [ ] `build.rs` parses magic file at build time
- [ ] Build fails with clear error if magic file is invalid
- [ ] Generated code compiles without warnings
- [ ] `MagicDatabase::with_builtin_rules()` returns working database
- [ ] Built-in rules correctly identify ELF, PE, ZIP, JPEG, PNG, PDF, GIF
- [ ] `--use-builtin` flag works end-to-end
- [ ] Rustdoc added for all public APIs
- [ ] Unit tests for built-in rules
- [ ] Integration test with `--use-builtin`
- [ ] Build script test (verify error handling)

## Dependencies

None - can be implemented independently

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - Built-in Rules Module)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/36539700-862d-4fdf-9c79-3c36390f6aa8 (Core Flows - Flow 3)

## Files to Create

- `file:src/builtin_rules.magic` - Magic file source
- `file:build.rs` - Build script
- `file:src/builtin_rules.rs` - Module for built-in rules

## Files to Modify

- `file:src/lib.rs` - Add `with_builtin_rules()` method
- `file:Cargo.toml` - Add build script configuration if needed
