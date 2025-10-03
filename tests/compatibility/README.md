# Compatibility Testing

This directory contains compatibility tests to ensure that libmagic-rs produces identical results to the original libmagic implementation.

## Overview

The compatibility test suite uses test files from the original [file/file](https://github.com/file/file) repository as a git submodule and runs our `rmagic` binary against each `.testfile` to verify that the output matches the corresponding `.result` file.

## Quick Start

### Initialize Test Files

```bash
# Initialize git submodule for test files from file/file repository
just download-compatibility-tests
```

### Run Compatibility Tests

```bash
# Run compatibility tests (requires test files to be downloaded)
just test-compatibility

# Run full compatibility test suite (initializes submodule and runs tests)
just test-compatibility-full
```

## Manual Usage

### Initialize Test Files

```bash
git submodule update --init --recursive tests/compatibility/file-tests
```

### Run Tests

```bash
# Build the project first
cargo build --release

# Run compatibility tests
cargo test test_compatibility_with_original_libmagic -- --ignored
```

## Test Structure

- `compatibility_tests.rs` - Rust test suite that runs compatibility tests
- `file-tests/` - Git submodule containing test files from file/file repository

## Test Files

The test files are downloaded to `tests/compatibility/file-tests/tests/` and include:

- `.testfile` - Test files to analyze
- `.result` - Expected output from original libmagic

## Output

The Rust test suite provides:

- Console output with test results and summary
- Detailed failure information for debugging
- Test status: PASS, FAIL, or ERROR

## CI/CD Integration

### GitHub Actions

The compatibility tests are automatically run on:

- Push to main/develop branches
- Pull requests
- Daily at 2 AM UTC

### Local Development

```bash
# Full compatibility test suite
just test-compatibility-full

# Just run tests (if files already downloaded)
just test-compatibility
```

## Troubleshooting

### Test Files Not Found

If you get "Test directory not found", run:

```bash
just download-compatibility-tests
```

### Binary Not Found

Ensure the project is built:

```bash
cargo build --release
```

### Magic File Not Found

Ensure the magic file exists at `test_files/magic`:

```bash
ls test_files/magic
```

## Test Results

The compatibility test runner provides:

- **PASS** - Output matches expected result exactly
- **FAIL** - Output differs from expected result
- **ERROR** - Test failed to run (binary error, file not found, etc.)

Failed tests show the expected vs actual output for debugging.

## Performance

The test suite typically runs in 30-60 seconds depending on the number of test files and system performance.

## Contributing

When adding new features to libmagic-rs:

1. Run the compatibility tests to ensure no regressions
2. If tests fail, investigate the differences
3. Update the implementation to match expected behavior
4. Re-run tests to verify fixes

## Test Coverage

The compatibility test suite covers:

- Basic file type detection
- Complex magic rules
- Edge cases and error conditions
- Various file formats and structures
- Performance characteristics

This ensures that libmagic-rs maintains full compatibility with the original libmagic implementation.
