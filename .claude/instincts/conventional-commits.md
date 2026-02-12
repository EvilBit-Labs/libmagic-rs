---
id: libmagic-rs-commits
trigger: "when writing a commit message"
confidence: 0.8
domain: git
source: local-repo-analysis
---

# Conventional Commits Format

## Action

Prefix commit messages with type:

- `feat:` - New features (most common)
- `fix:` - Bug fixes
- `chore:` - Maintenance (deps, config)
- `docs:` - Documentation changes
- `test:` - Test additions/changes
- `refactor:` - Code restructuring
- `perf:` - Performance improvements
- `ci:` - CI/CD changes

Optional scope: `chore(deps):`, `chore(ci):`

PR references: Include `(#N)` suffix when applicable.

Examples from this repo:
- `feat: implement comprehensive test infrastructure`
- `feat: evaluation enhancements with confidence, MIME, tags, metadata (#29)`
- `chore(deps): update dependencies in mise.toml for improved tooling`
- `feat: built-in rules build time compilation fallback (#28)`

## Evidence

- Analyzed 34 commits
- 77% follow conventional commit format
- `feat:` is the most common type (30% of commits)
- PR numbers consistently included as `(#N)` suffix
