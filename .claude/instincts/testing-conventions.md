---
id: libmagic-rs-testing
trigger: "when writing tests or adding new functionality"
confidence: 0.9
domain: testing
source: local-repo-analysis
---

# Testing Conventions

## Action

Follow these testing patterns:

1. **Unit tests**: Place in `#[cfg(test)] mod tests` within each source file
2. **Integration tests**: Add to `tests/` directory with `_tests.rs` suffix
3. **CLI tests**: Use `insta` snapshots in `tests/cli_integration_tests.rs`
4. **Property tests**: Add to `tests/property_tests.rs` using `proptest`
5. **Benchmarks**: Add to `benches/` using `criterion` with `harness = false`

Run tests with:

```bash
cargo nextest run --workspace --no-capture   # Standard
just ci-check                                 # Full CI parity
just coverage                                 # With coverage
```

Test naming: `test_<module>_<behavior>` (e.g., `test_parse_error_display`)

Coverage target: >85% with `cargo llvm-cov`

## Evidence

- 8 test files in `tests/` directory
- 3 benchmark files in `benches/`
- Every source file has inline `#[cfg(test)]` module
- `insta` used for snapshot testing CLI output
- `proptest` used for property-based testing
- `criterion` used for benchmarks (not built-in bench)
