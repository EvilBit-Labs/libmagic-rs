---
id: libmagic-rs-co-change
trigger: "when modifying core source files"
confidence: 0.85
domain: architecture
source: local-repo-analysis
---

# Co-Change Awareness

## Action

When modifying these files, check if related files also need updates:

| If you change... | Also check... |
|-------------------|--------------|
| `src/lib.rs` | `src/main.rs`, `src/parser/ast.rs`, tests |
| `src/evaluator/mod.rs` | `src/lib.rs`, `src/main.rs`, tests |
| `src/parser/ast.rs` | `src/lib.rs`, `src/parser/grammar.rs`, `src/evaluator/types.rs` |
| `src/main.rs` | `tests/cli_integration_tests.rs` |
| `Cargo.toml` | `src/lib.rs`, `src/main.rs` |
| `src/parser/grammar.rs` | `src/parser/mod.rs`, `tests/parser_integration_tests.rs` |

## Evidence

- Analyzed 34 commits for file co-change frequency
- `src/lib.rs` + `src/main.rs` changed together 8 times (100% of main.rs changes)
- `src/evaluator/mod.rs` + `src/lib.rs` changed together 8 times
- API types are re-exported through `src/lib.rs`, making it a hub file
