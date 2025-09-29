# Specification: Pure-Rust Implementation of libmagic

## Overview

This document specifies the design and implementation details for a pure-Rust replacement of **libmagic**, the library behind the `file` command. The goal is to implement a safe, efficient, and extensible engine capable of parsing **magic files** (DSL describing byte-level tests) and evaluating them against file buffers to identify file types.

## Goals

- Provide compatibility with common **magic file syntax** (offsets, types, operators, nesting).
- Offer **safe Rust memory handling** with no unsafe code (except in vetted crates).
- Support **structured output** (JSON) and classic human-readable output.
- Be extensible for modern use cases (PE resources, Mach-O load commands, Go build info).
- Maintain performance parity with libmagic using **mmap** and efficient matching.

## Core Concepts

### Magic Rule

Each rule in a magic file consists of:

- **Offset**: where in the file to look (absolute, relative, indirect).
- **Type**: how to interpret bytes (`byte`, `short`, `long`, `quad`, `string`, `regex`, etc.).
- **Operator**: comparison (`=`, `!=`, `>`, `<`, bitmasking `&`, XOR, etc.).
- **Value**: literal to compare against (e.g., `0xCAFEBABE`, `"PK\003\004"`).
- **Message**: description if matched.
- **Children**: nested continuation rules that refine identification.

### Rule Hierarchy

Rules are hierarchical. A parent rule must match before child rules are evaluated. Nested indentation or leading markers (`>`) represent dependent checks.

### Matching Process

1. **Parse magic file** → AST of rules.
2. **Evaluate rules** sequentially against target file buffer.
3. **Resolve offsets** (absolute, indirect, relative).
4. **Read value** at offset according to type.
5. **Compare** against rule value using operator.
6. On match → record message, evaluate child rules.
7. Produce result: textual description or structured JSON.

## Rust Implementation Details

### Crates & Dependencies

- `memmap2`: mmap files for efficient reads.
- `byteorder`: handle endianness conversions.
- `bstr`: byte-safe string handling.
- `regex` or `onig`: regex matching (binary-safe preferred).
- `aho-corasick`: fast multi-pattern search.
- `serde/serde_json`: serialization for JSON outputs and compiled rules.
- `nom` or `pest`: parsing the magic file DSL.

### Data Model

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum Value {
    Bytes(Vec<u8>),
    Int(i64),
    Uint(u64),
    String(String),
    Regex(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TypeKind {
    Byte,
    Short { le: bool, signed: bool },
    Long { le: bool, signed: bool },
    Quad { le: bool, signed: bool },
    String { encoding: StringKind },
    Regex,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Operator {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And(u64),
    Xor(u64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OffsetSpec {
    Absolute(i64),
    Indirect { off: i64, typ: TypeKind, add: i64 },
    FromEnd(i64),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MagicRule {
    pub offset: OffsetSpec,
    pub typ: TypeKind,
    pub op: Operator,
    pub value: Value,
    pub message: String,
    pub children: Vec<MagicRule>,
}
```

### Evaluation Algorithm

1. Resolve offset with `resolve_offset()`.
2. Read bytes using safe slice indexing.
3. Interpret according to `TypeKind`.
4. Apply operator to compare value.
5. If match → emit message and evaluate children.
6. Stop at first match (default) or collect all matches (configurable).

### Output Modes

- **Human-readable**: like `file` (`ELF 64-bit LSB executable, x86-64`).
- **JSON**:

```json
{
  "matches": [
    {
      "text": "Zip archive data",
      "offset": 0,
      "value": "PK\u0003\u0004",
      "tags": ["archive", "zip"],
      "score": 90
    }
  ]
}
```

- **MIME type mapping** (optional mapping DB).

## Roadmap

### MVP (v0.1)

- Support absolute offsets.
- Handle `byte`, `short`, `long`, `string`.
- Operators: `=`, `!=`, `&`.
- Nested rules.
- CLI: `rmagic file --json | --text`.

### v0.2

- Indirect offsets.
- Regex and masks.
- UTF-16 string support.
- Compiled binary rule cache.

### v0.3

- Performance improvements (Aho-Corasick indexing).
- PE resources + Mach-O load strings.
- Go build info detection.

### v1.0

- Full subset of libmagic syntax.
- MIME mappings.
- Stable CLI + library API.

## Testing & Validation

- Compare results with GNU `file` on sample corpus (ELF, PE, Mach-O, archives, images, PDFs).
- Ensure safe handling of truncated/corrupted inputs.
- Fuzzing to catch OOB reads.
- Unit tests for offset resolution, type parsing, regex matches.

## Pitfalls & Notes

- Regex crate UTF-8 limitation → consider `onig` or binary-safe wrapper.
- Endianness handling for indirect offsets is tricky.
- Performance must be addressed with caching/indexing.
- Packaged/obfuscated binaries will still report as generic `data` unless unpacked.

---
**Conclusion:** This spec outlines a practical, safe, and extensible pure-Rust replacement for libmagic. Initial focus should be on correctness and compatibility with common magic rules, with extensibility for modern file types in later versions.
