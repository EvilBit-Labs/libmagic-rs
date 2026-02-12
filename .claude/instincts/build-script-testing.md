---
id: libmagic-rs-build-testing
trigger: "when modifying build.rs or build-time logic"
confidence: 0.85
domain: rust-build
source: local-repo-analysis
---

# Build Script Testing Pattern

## Action

Build scripts (`build.rs`) cannot import the crate being built. To test build logic:

1. Extract build logic into `src/build_helpers.rs` with `#[cfg(any(test, doc))]`
2. Keep `build.rs` minimal - it should only call functions from the helper module
3. Write unit tests in `build_helpers.rs` to verify all code paths
4. Test error cases (invalid magic files, malformed input)

This ensures build-time failures produce clear error messages and are properly tested.

## Evidence

- `src/build_helpers.rs` exists with testable parsing and code generation
- `build.rs` delegates to functions from `build_helpers`
- Pattern documented in AGENTS.md as a project convention
