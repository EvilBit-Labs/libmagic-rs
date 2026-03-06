# Release Process

This document outlines the release process for libmagic-rs, including version management, testing procedures, and deployment steps.

## Release Types

### Semantic Versioning

libmagic-rs follows [Semantic Versioning](https://semver.org/) (SemVer):

- **Major version** (X.0.0): Breaking API changes
- **Minor version** (0.X.0): New features, backward compatible
- **Patch version** (0.0.X): Bug fixes, backward compatible

### Conventional Commits

Commit messages follow the [Conventional Commits](https://www.conventionalcommits.org/) specification for automated semantic versioning and changelog generation. The required format is:

```text
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

#### Commit Types

- `feat`: Triggers minor version bump (0.X.0)
- `fix`: Triggers patch version bump (0.0.X)
- `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`: No version bump

#### Breaking Changes

To trigger a major version bump (X.0.0), indicate breaking changes using a `BREAKING CHANGE:` footer in the commit body:

```text
feat(api): redesign evaluation interface

Replace EvaluationConfig::new() with a builder pattern
for better ergonomics.

BREAKING CHANGE: EvaluationConfig::new() removed, use
EvaluationConfig::builder() instead.
```

The `!` indicator after the type/scope (e.g., `feat!:` or `feat(scope)!:`) is not validated by the Mergify conventional commit enforcement. Use the `BREAKING CHANGE:` footer for reliable detection by both Mergify and release-plz.

### Pre-release Versions

- **Alpha** (0.1.0-alpha.1): Early development, unstable API
- **Beta** (0.1.0-beta.1): Feature complete, API stabilizing
- **Release Candidate** (0.1.0-rc.1): Final testing before release

## Release Checklist

### Pre-Release Preparation

#### 1. Code Quality Verification

```bash
# Ensure all tests pass
cargo test --all-features

# Check code formatting
cargo fmt -- --check

# Run comprehensive linting
cargo clippy -- -D warnings

# Verify documentation builds
cargo doc --document-private-items

# Run security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

#### 2. Performance Validation

```bash
# Run benchmarks and compare with baseline
cargo bench

# Profile memory usage
cargo build --release
valgrind --tool=massif target/release/rmagic large_file.bin

# Test with large files and magic databases
./performance_test.sh
```

#### 3. Compatibility Testing

```bash
# Test against GNU file compatibility suite
cargo test compatibility

# Test with various magic file formats
./test_magic_compatibility.sh

# Cross-platform testing
cargo test --target x86_64-pc-windows-gnu
cargo test --target aarch64-apple-darwin
```

#### 4. Documentation Updates

- [ ] Update `README.md` with new features and changes
- [ ] Update `CHANGELOG.md` with release notes
- [ ] Review and update API documentation
- [ ] Update migration guide if needed
- [ ] Verify all examples work with new version

### Version Bumping

#### 1. Update Version Numbers

```toml
# Cargo.toml
[package]
name = "libmagic-rs"
version = "0.2.0"    # Update version
```

#### 2. Update Documentation

```rust
// src/lib.rs - Update version in documentation
//! # libmagic-rs v0.2.0
//!
//! A pure-Rust implementation of libmagic...
```

#### 3. Update Changelog

```markdown
# Changelog

## [0.2.0] - 2026-03-01

### Features
- **parser**: Implement comparison operators

### Miscellaneous Tasks
- **Mergify**: Add outdated PR protection
- Add Mergify merge queue and simplify CI
- Mergify merge queue, dependabot integration, and CI simplification
- **release**: Add regex for version bumping based on commit types

### Breaking Changes
- `TypeKind::Byte` variant changed from unit variant to a different kind
- `Operator` enum: added `LessThan`, `GreaterThan`, `LessEqual`, `GreaterEqual` variants (exhaustive enum)
- `read_byte` function signature changed (parameter count modified)
```

### Release Creation

#### 1. Create Release Branch

```bash
# Create release branch
git checkout -b release/v0.2.0

# Commit version updates (must be signed)
git add Cargo.toml CHANGELOG.md README.md
git commit -s -m "chore: bump version to 0.2.0"

# Push release branch
git push origin release/v0.2.0
```

**Note:** Commits must be signed with DCO (`-s` flag). The local enforcement hook `.claude/hookify.block-unsigned-commits.local.md` blocks unsigned commits.

#### 2. Final Testing

```bash
# Clean build and test
cargo clean
cargo build --release
cargo test --release

# Integration testing
./integration_test.sh

# Performance regression testing
./performance_regression_test.sh
```

#### 3. Create Pull Request

- Create PR from release branch to main
- Ensure all CI checks pass
- Get approval from maintainers
- Merge to main branch

#### 4. Tag Release

```bash
# Switch to main branch
git checkout main
git pull origin main

# Create and push tag (must be signed)
git tag -s v0.2.0 -m "Release version 0.2.0"
git push origin v0.2.0
```

**Note:** Tags must be GPG-signed. The local enforcement hook `.claude/hookify.block-unsigned-tags.local.md` prevents unsigned tags from being created.

### GitHub Release

#### 1. Create GitHub Release

- Go to GitHub repository releases page
- Click "Create a new release"
- Select the version tag (v0.2.0)
- Use version number as release title
- Copy changelog content as release description

#### 2. Release Assets

Include relevant assets:

- Source code (automatically included)
- Pre-compiled binaries (if applicable)
- Documentation archive
- Checksums file

### Post-Release Tasks

#### 1. Update Development Branch

```bash
# Create new development branch
git checkout -b develop
git push origin develop

# Update version to next development version
# Cargo.toml: version = "0.3.0-dev"
git add Cargo.toml
git commit -s -m "chore: bump version to 0.3.0-dev"
git push origin develop
```

#### 2. Documentation Deployment

```bash
# Deploy documentation to GitHub Pages
mdbook build docs/
# Automated deployment via GitHub Actions
```

#### 3. Announcement

- Update project README with latest version
- Post announcement in GitHub Discussions
- Update any external documentation or websites
- Notify users through appropriate channels

## Hotfix Process

### Critical Bug Fixes

For critical bugs that need immediate release:

#### 1. Create Hotfix Branch

```bash
# Branch from latest release tag
git checkout v0.2.0
git checkout -b hotfix/v0.2.1

# Make minimal fix
# ... fix the critical bug ...

# Commit fix
git add .
git commit -s -m "fix: critical security vulnerability in offset parsing"
```

**Note:** All commits must be signed with DCO (`-s` flag).

#### 2. Test Hotfix

```bash
# Run focused tests
cargo test security
cargo test offset_parsing

# Run security audit
cargo audit

# Minimal integration testing
./critical_path_test.sh
```

#### 3. Release Hotfix

```bash
# Update version to patch release
# Cargo.toml: version = "0.2.1"

# Update changelog
# Add entry for hotfix

# Commit and tag
git add Cargo.toml CHANGELOG.md
git commit -s -m "chore: bump version to 0.2.1"
git tag -s v0.2.1 -m "Hotfix release 0.2.1"

# Push hotfix
git push origin hotfix/v0.2.1
git push origin v0.2.1
```

**Note:** Both commits and tags must be signed.

#### 4. Merge Back

```bash
# Merge hotfix to main
git checkout main
git merge hotfix/v0.2.1

# Merge hotfix to develop
git checkout develop
git merge hotfix/v0.2.1

# Clean up hotfix branch
git branch -d hotfix/v0.2.1
git push origin --delete hotfix/v0.2.1
```

## Release Automation

Releases are automated by two complementary tools:

- **release-plz**: Manages crates.io publishing, version bumping, changelog generation, and git tagging
- **cargo-dist** (v0.31.0): Builds cross-platform binaries, Homebrew formulas, SBOM, and GitHub Releases

### How It Works

1. Every push to `main` triggers release-plz, which opens (or updates) a **release PR** with:

   - Version bump in `Cargo.toml`
   - Updated `CHANGELOG.md` (generated via git-cliff)
   - Semantic versioning based on conventional commits

2. When the release PR is **merged**, release-plz:

   - Publishes the crate to **crates.io**
   - Creates a **git tag** (e.g., `v0.2.0`)

3. The git tag triggers **cargo-dist**, which:

   - Builds binaries for all target platforms
   - Generates SLSA attestations and SBOM
   - Publishes the Homebrew formula
   - Creates the **GitHub Release** with all artifacts

#### Release PR Merge Handling

Release-plz PRs (with branch names matching `release-plz-*`) are exempt from the standard "CI must pass" merge protection. This exemption exists because:

- release-plz force-pushes when updating existing PRs
- `GITHUB_TOKEN`-triggered pushes do not trigger workflow events, so CI never runs on the updated HEAD commit
- Without this exemption, the merge protection would block the PR indefinitely
- Release-plz PRs only bump versions and update changelogs -- they contain no code changes
- The code being released was already tested on `main` before the release PR was created
- CI still runs in the merge queue as a final verification step before the release is completed

The "CI must pass" merge protection requires 4 checks: `quality`, `test-cross-platform (ubuntu-latest, Linux)`, `test-cross-platform (macos-latest, macOS)`, and `test-cross-platform (windows-latest, Windows)`. The merge queue enforces additional checks including `test`, `coverage`, and `test-cross-platform (ubuntu-22.04, Linux)`.

PRs must be within 3 commits of `main` before merging. This exemption allows the release workflow to proceed smoothly while maintaining the safety of the automated release process.

### Configuration Files

| File                  | Purpose                                                     |
| --------------------- | ----------------------------------------------------------- |
| `release-plz.toml`    | release-plz configuration (crates.io, tags, changelog)      |
| `dist-workspace.toml` | cargo-dist v0.31.0 configuration (binaries, Homebrew, SBOM) |
| `cliff.toml`          | git-cliff changelog template (shared by both tools)         |

### Workflow Protection

The release workflow (`.github/workflows/release.yml`) is auto-generated by cargo-dist. Manual modifications are blocked by the local enforcement hook `.claude/hookify.block-release-workflow-edit.local.md`. To update the release workflow, modify `dist-workspace.toml` and run `cargo dist generate`.

### Authentication

- **crates.io**: Uses [trusted publishing](https://doc.rust-lang.org/cargo/reference/registry-authentication.html#oidc-token-exchange) (OIDC) -- no token secret needed. Requires configuring the trusted publisher on crates.io and `id-token: write` permission in the workflow. Note: the first publish of a new crate must be done manually with `cargo publish`.
- **Homebrew tap**: Requires a `HOMEBREW_TAP_TOKEN` secret with write access to the tap repository.
- **GitHub Releases**: Uses the automatic `GITHUB_TOKEN`.

## Release Schedule

### Regular Releases

- **Minor releases**: Every 6-8 weeks
- **Patch releases**: As needed for bug fixes
- **Major releases**: When breaking changes accumulate

### Release Windows

- **Feature freeze**: 1 week before release
- **Code freeze**: 3 days before release
- **Release day**: Tuesday (for maximum testing time)

### Communication

- **Release planning**: Discussed in GitHub Issues/Discussions
- **Release announcements**: GitHub Releases, project README
- **Breaking changes**: Documented in migration guide

This release process ensures high-quality, reliable releases while maintaining clear communication with users and contributors.
