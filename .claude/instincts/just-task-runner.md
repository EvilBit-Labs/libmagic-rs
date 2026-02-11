---
id: libmagic-rs-just-tasks
trigger: "when running project commands or CI checks"
confidence: 0.9
domain: tooling
source: local-repo-analysis
---

# Use just for Task Running

## Action

Use `just` (not raw cargo) for project tasks. Key commands:

| Command | Purpose |
|---------|---------|
| `just ci-check` | Full CI parity check (run before committing) |
| `just test` | Run tests with nextest |
| `just lint-rust` | Clippy with `-D warnings` |
| `just fmt` | Format Rust code |
| `just coverage` | Generate LCOV coverage report |
| `just coverage-report` | HTML coverage with browser open |
| `just bench` | Run all benchmarks |
| `just audit` | Security audit |
| `just docs` | Build and serve documentation |
| `just test-compatibility` | Test against original libmagic test suite |

All commands use `mise exec --` prefix for tool version management.

## Evidence

- `justfile` has 50+ recipes organized into sections
- All CI workflows use `just` commands
- `mise.toml` manages tool versions (Rust, pre-commit, mdbook, etc.)
- Cross-platform support with `[unix]`/`[windows]` annotations
