# Cross-platform justfile using OS annotations
# Windows uses PowerShell, Unix uses bash

set windows-shell := ["powershell.exe", "-c"]
set shell := ["bash", "-c"]
set dotenv-load := true
set ignore-comments := true

# Use mise to manage all dev tools (go, pre-commit, uv, etc.)
# See mise.toml for tool versions
mise_exec := "mise exec --"

root := justfile_dir()

# =============================================================================
# GENERAL COMMANDS
# =============================================================================

default:
    @just --list

# =============================================================================
# CROSS-PLATFORM HELPERS (private)
# =============================================================================

[private, windows]
ensure-dir dir:
    New-Item -ItemType Directory -Force -Path "{{ dir }}" | Out-Null

[private, unix]
ensure-dir dir:
    /bin/mkdir -p "{{ dir }}"

[private, windows]
rmrf path:
    if (Test-Path "{{ path }}") { Remove-Item "{{ path }}" -Recurse -Force }

[private, unix]
rmrf path:
    /bin/rm -rf "{{ path }}"

# =============================================================================
# SETUP AND INITIALIZATION
# =============================================================================

# Development setup - mise handles all tool installation via mise.toml
setup:
    mise install

# =============================================================================
# FORMATTING AND LINTING
# =============================================================================

alias format-rust := fmt
alias format-md := format-docs
alias format-just := fmt-justfile

# Main format recipe - calls all formatters
format: fmt format-json-yaml format-docs fmt-justfile

# Individual format recipes

format-json-yaml:
    @{{ mise_exec }} prettier --write "**/*.{json,yaml,yml}"

format-docs:
    @{{ mise_exec }} mdformat --exclude "target/*" --exclude "node_modules/*" .

fmt:
    @{{ mise_exec }} cargo fmt --all

fmt-check:
    @{{ mise_exec }} cargo fmt --all --check

lint-rust: fmt-check
    @{{ mise_exec }} cargo clippy --workspace --all-targets --all-features -- -D warnings

lint-rust-min:
    @{{ mise_exec }} cargo clippy --workspace --all-targets --no-default-features -- -D warnings

# Format justfile
fmt-justfile:
    @{{ mise_exec }} just --fmt --unstable

# Lint justfile formatting
lint-justfile:
    @{{ mise_exec }} just --fmt --check --unstable
# Main lint recipe - calls all sub-linters
lint: lint-rust lint-actions lint-docs lint-justfile

# Individual lint recipes
lint-actions:
    @{{ mise_exec }} actionlint .github/workflows/audit.yml .github/workflows/benchmarks.yml .github/workflows/ci.yml .github/workflows/codeql.yml .github/workflows/compatibility.yml .github/workflows/copilot-setup-steps.yml .github/workflows/docs.yml .github/workflows/release.yml .github/workflows/security.yml

lint-docs:
    @{{ mise_exec }} markdownlint-cli2 docs/**/*.md README.md
    @{{ mise_exec }} lychee docs/**/*.md README.md

alias lint-just := lint-justfile

# Run clippy with fixes
fix:
    @{{ mise_exec }} cargo clippy --fix --allow-dirty --allow-staged

# Quick development check
check: pre-commit-run lint

[private]
pre-commit-run:
    @{{ mise_exec }} pre-commit run -a

# Format a single file (for pre-commit hooks)
format-files +FILES:
    @{{ mise_exec }} prettier --write --config .prettierrc.json {{ FILES }}


# =============================================================================
# BUILDING AND TESTING
# =============================================================================

build:
    @{{ mise_exec }} cargo build --workspace

build-release:
    @{{ mise_exec }} cargo build --workspace --release

test:
    @{{ mise_exec }} cargo nextest run --workspace --no-capture

# Verify compatibility test files are available
[windows]
verify-compatibility-tests:
    @echo "Verifying compatibility test files are available..."
    if (-not (Test-Path "third_party/tests")) { Write-Error "third_party/tests directory not found" }
    if (-not (Test-Path "third_party/magic.mgc")) { Write-Error "third_party/magic.mgc not found" }

[unix]
verify-compatibility-tests:
    @echo "Verifying compatibility test files are available..."
    @if [ ! -d "third_party/tests" ]; then echo "third_party/tests directory not found" && exit 1; fi
    @if [ ! -f "third_party/magic.mgc" ]; then echo "third_party/magic.mgc not found" && exit 1; fi

# Run compatibility tests against original libmagic test suite
test-compatibility:
    @{{ mise_exec }} cargo test test_compatibility_with_original_libmagic -- --ignored

# Run all compatibility tests (including setup)
test-compatibility-full: verify-compatibility-tests
    @{{ mise_exec }} cargo build --release
    @{{ mise_exec }} cargo test test_compatibility_with_original_libmagic -- --ignored

test-ci:
    @{{ mise_exec }} cargo nextest run --workspace --no-capture

# Run all tests including ignored/slow tests across workspace
test-all:
    @{{ mise_exec }} cargo nextest run --workspace --no-capture -- --ignored

# =============================================================================
# BENCHMARKING
# =============================================================================

# Run all benchmarks
bench:
    @{{ mise_exec }} cargo bench --workspace

# =============================================================================
# SECURITY AND AUDITING
# =============================================================================

audit:
    @{{ mise_exec }} cargo audit

deny:
    @{{ mise_exec }} cargo deny check

# =============================================================================
# CI AND QUALITY ASSURANCE
# =============================================================================

# Private helper: run cargo llvm-cov with proper setup
[private, unix]
_coverage +args:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf target/llvm-cov-target
    RUSTFLAGS="--cfg coverage" {{ mise_exec }} cargo llvm-cov --workspace --lcov --output-path lcov.info {{ args }}

[private, windows]
_coverage +args:
    Remove-Item -Recurse -Force target/llvm-cov-target -ErrorAction SilentlyContinue
    $env:RUSTFLAGS = "--cfg coverage"; {{ mise_exec }} cargo llvm-cov --workspace --lcov --output-path lcov.info {{ args }}

coverage:
    @{{ mise_exec }} just _coverage

coverage-check:
    @{{ mise_exec }} just _coverage --fail-under-lines 9.7

# Generate HTML coverage report for local viewing
[unix]
coverage-report:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf target/llvm-cov-target
    RUSTFLAGS="--cfg coverage" {{ mise_exec }} cargo llvm-cov --workspace --html --open

[windows]
coverage-report:
    $env:RUSTFLAGS = "--cfg coverage"; {{ mise_exec }} cargo llvm-cov --workspace --html --open

# Show coverage summary by file
[unix]
coverage-summary:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf target/llvm-cov-target
    RUSTFLAGS="--cfg coverage" {{ mise_exec }} cargo llvm-cov --workspace

[windows]
coverage-summary:
    $env:RUSTFLAGS = "--cfg coverage"; {{ mise_exec }} cargo llvm-cov --workspace

# Full local CI parity check
ci-check: pre-commit-run fmt-check lint-rust lint-rust-min test-ci build-release audit coverage-check dist-plan

# Run compatibility tests as part of CI
ci-check-compatibility: pre-commit-run fmt-check lint-rust lint-rust-min test-ci build-release audit coverage-check test-compatibility dist-plan

# =============================================================================
# DISTRIBUTION AND PACKAGING
# =============================================================================

dist:
    @{{ mise_exec }} dist build

dist-check:
    @{{ mise_exec }} dist check

dist-plan:
    @{{ mise_exec }} dist plan

# Regenerate cargo-dist CI workflow safely
dist-generate-ci:
    {{ mise_exec }} dist generate --ci github
    @echo "Generated CI workflow. Remember to fix any expression errors if they exist."
    @echo "Run 'just lint:actions' to validate the generated workflow."

install:
    @{{ mise_exec }} cargo install --path .

# =============================================================================
# DOCUMENTATION
# =============================================================================

# Build complete documentation (mdBook + rustdoc)
[unix]
docs-build:
    #!/usr/bin/env bash
    set -euo pipefail
    # Build rustdoc
    {{ mise_exec }} cargo doc --no-deps --document-private-items --target-dir docs/book/api-temp
    # Move rustdoc output to final location
    mkdir -p docs/book/api
    cp -r docs/book/api-temp/doc/* docs/book/api/
    rm -rf docs/book/api-temp
    # Build mdBook
    cd docs && {{ mise_exec }} mdbook build

# Serve documentation locally with live reload
[unix]
docs-serve:
    cd docs && {{ mise_exec }} mdbook serve --open

# Clean documentation artifacts
[unix]
docs-clean:
    rm -rf docs/book target/doc

# Check documentation (build + link validation + formatting)
[unix]
docs-check:
    cd docs && {{ mise_exec }} mdbook build
    @{{ mise_exec }} just fmt-check

# Generate and serve documentation
[unix]
docs: docs-build docs-serve

[windows]
docs:
    @echo "mdbook requires a Unix-like environment to serve"

# =============================================================================
# GORELEASER TESTING
# =============================================================================

# Private helper: run goreleaser with macOS SDK env configured (no-op on non-mac)
[private, unix]
_goreleaser +args:
    #!/bin/bash
    set -euo pipefail
    if command -v xcrun >/dev/null 2>&1; then
        SDKROOT_PATH=$(xcrun --sdk macosx --show-sdk-path)
        export SDKROOT="${SDKROOT_PATH}"
        export MACOSX_DEPLOYMENT_TARGET="11.0"
        export CARGO_ZIGBUILD_SYSROOT="${SDKROOT_PATH}"
        export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-syslibroot,${SDKROOT_PATH} -C link-arg=-F${SDKROOT_PATH}/System/Library/Frameworks"
    fi
    goreleaser {{ args }}

[private, windows]
_goreleaser +args:
    @{{ mise_exec }} goreleaser {{ args }}

goreleaser-check:
    @{{ mise_exec }} goreleaser check

goreleaser-build:
    @just _goreleaser build --clean

goreleaser-snapshot:
    @just _goreleaser release --snapshot --clean

goreleaser-build-target target:
    @just _goreleaser build --clean --single-target {{ target }}

# Clean GoReleaser artifacts
goreleaser-clean:
    @just rmrf dist

# =============================================================================
# RELEASE MANAGEMENT
# =============================================================================

release:
    @{{ mise_exec }} cargo release

release-dry-run:
    @{{ mise_exec }} cargo release --dry-run

release-patch:
    @{{ mise_exec }} cargo release patch

release-minor:
    @{{ mise_exec }} cargo release minor

release-major:
    @{{ mise_exec }} cargo release major
