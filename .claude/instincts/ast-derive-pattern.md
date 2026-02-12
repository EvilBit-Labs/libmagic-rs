---
id: libmagic-rs-ast-derives
trigger: "when creating new AST types or data structures"
confidence: 0.9
domain: rust-types
source: local-repo-analysis
---

# AST Type Derive Pattern

## Action

When creating new data structures (especially AST nodes):

1. Derive: `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`
2. Add rustdoc with `# Examples` section
3. Use `#[non_exhaustive]` on public enums
4. Document each field/variant with `///` comments
5. Import `serde::{Serialize, Deserialize}` (not `serde_derive`)

```rust
use serde::{Deserialize, Serialize};

/// Description of the type
///
/// # Examples
///
/// ```
/// use libmagic_rs::parser::ast::MyType;
/// let val = MyType::Variant(42);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum MyType {
    /// Description of variant
    Variant(i64),
}
```

## Evidence

- All types in `src/parser/ast.rs` follow this exact pattern
- 6 enum types and 1 struct type all derive the same 6 traits
- Every public type has rustdoc examples
- Every variant has `///` doc comments
