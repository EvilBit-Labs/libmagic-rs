# Release Process

This document outlines the release process for libmagic-rs, including version management, testing procedures, and deployment steps.

## Release Types

### Semantic Versioning

libmagic-rs follows [Semantic Versioning](https://semver.org/) (SemVer):

- **Major version** (X.0.0): Breaking API changes
- **Minor version** (0.X.0): New features, backward compatible
- **Patch version** (0.0.X): Bug fixes, backward compatible

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
version = "0.2.0"  # Update version
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

## [0.2.0] - 2024-03-15

### Added
- Magic file parser implementation
- Basic rule evaluation engine
- Memory-mapped file I/O support

### Changed
- Improved AST structure for better performance
- Enhanced error messages with more context

### Fixed
- Buffer overflow protection in string reading
- Proper handling of indirect offsets

### Breaking Changes
- `EvaluationConfig` structure modified
- `MagicRule::new()` signature changed
```

### Release Creation

#### 1. Create Release Branch

```bash
# Create release branch
git checkout -b release/v0.2.0

# Commit version updates
git add Cargo.toml CHANGELOG.md README.md
git commit -m "chore: bump version to 0.2.0"

# Push release branch
git push origin release/v0.2.0
```

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

# Create and push tag
git tag -a v0.2.0 -m "Release version 0.2.0"
git push origin v0.2.0
```

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
git commit -m "chore: bump version to 0.3.0-dev"
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
git commit -m "fix: critical security vulnerability in offset parsing"
```

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
git commit -m "chore: bump version to 0.2.1"
git tag -a v0.2.1 -m "Hotfix release 0.2.1"

# Push hotfix
git push origin hotfix/v0.2.1
git push origin v0.2.1
```

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

### GitHub Actions Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run tests
        run: cargo test --all-features

      - name: Build release
        run: cargo build --release

      - name: Create release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Release ${{ github.ref }}
          draft: false
          prerelease: false
```

### Automated Checks

```bash
#!/bin/bash
# scripts/pre_release_check.sh

set -e

echo "Running pre-release checks..."

# Code quality
cargo fmt -- --check
cargo clippy -- -D warnings

# Tests
cargo test --all-features
cargo test --doc

# Security
cargo audit

# Performance
cargo bench --bench evaluation_bench

# Documentation
cargo doc --document-private-items

echo "All pre-release checks passed!"
```

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
