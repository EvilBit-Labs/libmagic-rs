# Test Infrastructure: Compatibility Tests & Coverage

## Overview

Implement comprehensive test infrastructure including compatibility tests against GNU file command, unit tests for all components, and achieve >85% test coverage. This validates Phase 1 MVP quality and ensures GNU file compatibility targets are met.

## Scope

**In Scope:**

- Compatibility tests using `file:third_party/tests/` corpus
- Comparison with GNU file output
- Unit tests for all new components
- Integration tests for end-to-end flows
- Test coverage measurement and reporting
- CI/CD integration

**Out of Scope:**

- Performance benchmarks (deferred to Phase 3)
- Fuzzing infrastructure (deferred to Phase 2)
- Property-based testing (deferred to Phase 2)

## Technical Approach

### 1. Compatibility Test Framework

Create `file:tests/compatibility_tests.rs`:

```rust
use libmagic_rs::MagicDatabase;
use std::process::Command;
use std::path::PathBuf;

#[test]
fn test_compatibility_with_gnu_file() -> Result<(), Box<dyn std::error::Error>> {
    let test_files = discover_test_files("third_party/tests/");
    let db = MagicDatabase::load_from_file("/usr/share/file/magic")?;

    let mut passed = 0;
    let mut failed = 0;
    let mut common_type_results = HashMap::new();

    for file in test_files {
        // Get GNU file output
        let gnu_output = Command::new("file")
            .arg("-b") // Brief mode
            .arg(&file)
            .output()?;

        // Get libmagic-rs output
        let result = db.evaluate_file(&file)?;

        // Compare outputs
        if outputs_match(&gnu_output, &result.description) {
            passed += 1;

            // Track common file types separately
            if is_common_type(&file) {
                common_type_results.insert(file.clone(), true);
            }
        } else {
            failed += 1;
            eprintln!("Mismatch for {}: GNU='{}', ours='{}'",
                     file.display(),
                     String::from_utf8_lossy(&gnu_output.stdout),
                     result.description);

            if is_common_type(&file) {
                common_type_results.insert(file.clone(), false);
            }
        }
    }

    let compatibility = (passed as f64 / (passed + failed) as f64) * 100.0;
    let common_compatibility = calculate_common_type_compatibility(&common_type_results);

    println!("Overall compatibility: {:.1}%", compatibility);
    println!("Common types compatibility: {:.1}%", common_compatibility);

    assert!(common_compatibility >= 100.0,
            "Common types compatibility: {:.1}% (target: 100%)",
            common_compatibility);
    assert!(compatibility >= 95.0,
            "Overall compatibility: {:.1}% (target: 95%)",
            compatibility);

    Ok(())
}

fn is_common_type(path: &Path) -> bool {
    // Check if file is ELF, PE, ZIP, JPEG, PNG, PDF
    let filename = path.file_name().unwrap().to_str().unwrap();
    filename.contains("elf") ||
    filename.contains("pe") ||
    filename.contains("zip") ||
    filename.contains("jpeg") ||
    filename.contains("png") ||
    filename.contains("pdf")
}
```

### 2. Unit Test Coverage

Add comprehensive unit tests to each module:

**Parser Tests** (`file:tests/parser_tests.rs`):

- Format detection
- Directory loading
- Strength parsing
- Error handling

**Evaluator Tests** (`file:tests/evaluator_tests.rs`):

- Confidence calculation
- Rule ordering
- Timeout handling

**MIME Tests** (`file:tests/mime_tests.rs`):

- Hardcoded mappings
- File loading
- Prefix matching

**Tag Tests** (`file:tests/tags_tests.rs`):

- Keyword extraction
- Case insensitivity

### 3. Integration Tests

Create `file:tests/integration_tests.rs`:

```rust
#[test]
fn test_end_to_end_elf_detection() {
    let db = MagicDatabase::load_from_file("third_party/magic.mgc")?;
    let result = db.evaluate_file("third_party/tests/elf-sample.bin")?;
    assert!(result.description.contains("ELF"));
    assert!(result.confidence > 0.8);
}

#[test]
fn test_builtin_rules() {
    let db = MagicDatabase::with_builtin_rules();
    let result = db.evaluate_file("third_party/tests/elf-sample.bin")?;
    assert!(result.description.contains("ELF"));
}

#[test]
fn test_multiple_files_cli() {
    let output = Command::new("cargo")
        .args(&["run", "--", "file1.bin", "file2.bin"])
        .output()?;
    assert!(output.status.success());
    assert_eq!(output.stdout.lines().count(), 2);
}
```

### 4. Coverage Measurement

Add to `file:.github/workflows/ci.yml`:

```yaml
- name: Test Coverage
  run: |
    cargo install cargo-llvm-cov
    cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

- name: Upload Coverage
  uses: codecov/codecov-action@v3
  with:
    files: lcov.info
    fail_ci_if_error: true
```

### 5. Test Utilities

Create `file:tests/common/mod.rs`:

```rust
pub fn discover_test_files(dir: &str) -> Vec<PathBuf> {
    // Recursively find all test files
}

pub fn outputs_match(gnu_output: &Output, our_output: &str) -> bool {
    // Normalize and compare outputs
    // Handle minor formatting differences
}

pub fn calculate_common_type_compatibility(results: &HashMap<PathBuf, bool>) -> f64 {
    // Calculate percentage for common types
}
```

## Acceptance Criteria

- [ ] Compatibility tests run against full test corpus
- [ ] 100% compatibility for common file types (ELF, PE, ZIP, JPEG, PNG, PDF)
- [ ] 95%+ compatibility for full test corpus
- [ ] >85% test coverage across all modules
- [ ] Unit tests for all new components
- [ ] Integration tests for all core flows
- [ ] CI/CD runs all tests automatically
- [ ] Coverage report uploaded to codecov
- [ ] Test failures show clear diagnostics
- [ ] Test suite runs in <5 minutes

## Dependencies

- Depends on: All implementation tickets
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/c554e409-ae60-407f-9596-64c5b03a9b92 (Parser Integration)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/bb48e54f-995b-448e-add8-fe1546bfa9d9 (CLI Enhancements)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/5fa9fb36-a0bf-4d1b-924a-41a0a39f126e (Built-in Rules)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/33ad363d-27da-40ad-94bb-e1e24bd5cb39 (Evaluation Enhancements)
  - ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/fae78198-06dc-4d77-ab4a-1cc1f7e117a4 (Strength Calculation)

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/3ce0475b-153d-487f-bc0d-47d0a8f6708a (Epic Brief - Success Criteria)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - Test Infrastructure)

## Files to Create

- `file:tests/compatibility_tests.rs` - Compatibility test suite
- `file:tests/parser_tests.rs` - Parser unit tests
- `file:tests/evaluator_tests.rs` - Evaluator unit tests
- `file:tests/mime_tests.rs` - MIME mapper tests
- `file:tests/tags_tests.rs` - Tag extractor tests
- `file:tests/integration_tests.rs` - End-to-end integration tests
- `file:tests/common/mod.rs` - Test utilities

## Files to Modify

- `file:.github/workflows/ci.yml` - Add coverage measurement
- `file:Cargo.toml` - Add test dependencies
