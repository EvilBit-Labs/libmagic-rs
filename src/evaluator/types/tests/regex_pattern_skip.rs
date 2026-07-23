// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! U2 skip-classification methods: `TypeReadError::is_pattern_skip` /
//! `is_regex_compile_failure` (issue #391 item 2 -- dedicated variants
//! replace the earlier string-keyed allowlist; the narrow S2.1 contract is
//! now compiler-enforced by variant identity, not string content).

use super::*;

#[test]
fn test_is_pattern_skip_recognizes_missing_operand_and_compile_error_variants() {
    let skippable = [
        TypeReadError::MissingPatternOperand {
            type_name: "regex without string pattern".to_string(),
        },
        TypeReadError::MissingPatternOperand {
            type_name: "search without string/bytes pattern".to_string(),
        },
        TypeReadError::MissingPatternOperand {
            type_name: "string with flags requires string/bytes pattern".to_string(),
        },
        TypeReadError::RegexCompileError {
            detail: "some failure".to_string(),
        },
    ];
    for err in &skippable {
        assert!(
            err.is_pattern_skip(),
            "expected {err:?} to be an allowlisted graceful-skip condition"
        );
    }
}

#[test]
fn test_is_pattern_skip_rejects_genuine_capability_gaps() {
    // R3 narrowness guard: a genuine capability gap (UnsupportedType) or an
    // unrelated read error must NOT be treated as skippable, or it would be
    // silently swallowed instead of aborting evaluation.
    let not_skippable = [
        TypeReadError::UnsupportedType {
            type_name: "meta-type Offset cannot be read as a value".to_string(),
        },
        TypeReadError::UnsupportedType {
            type_name: "operator GreaterThan is not supported for pattern-bearing type".to_string(),
        },
        TypeReadError::BufferOverrun {
            offset: 10,
            buffer_len: 4,
        },
    ];
    for err in &not_skippable {
        assert!(
            !err.is_pattern_skip(),
            "expected {err:?} to NOT be an allowlisted graceful-skip condition"
        );
    }
}

#[test]
fn test_is_regex_compile_failure_matches_only_the_compile_error_variant() {
    assert!(
        TypeReadError::RegexCompileError {
            detail: "Compiled regex exceeds size limit of 1048576 bytes.".to_string(),
        }
        .is_regex_compile_failure()
    );
    assert!(
        !TypeReadError::MissingPatternOperand {
            type_name: "regex without string pattern".to_string(),
        }
        .is_regex_compile_failure()
    );
    assert!(
        !TypeReadError::UnsupportedType {
            type_name: "some other gap".to_string(),
        }
        .is_regex_compile_failure()
    );
}
