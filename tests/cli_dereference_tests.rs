// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Directory classification and dereference-flag tests (issue #383).
//!
//! Separate from `cli_symlink_tests.rs`: those cover what a symlink is
//! reported as, these cover directory classification and how the
//! `-L` / `--no-dereference` pair selects between link and target.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate instead.
#![allow(clippy::expect_used)]

mod common;

use common::{ELF_HEADER, create_data_file, path_str, rmagic_cmd, symlink_or_skip};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Directory Classification Tests (ADR-0001 detection gap, issue #383)
// =============================================================================

#[test]
fn test_directory_is_classified_not_reported_as_an_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    // `file <dir>` prints `<dir>: directory` and exits 0. rmagic previously
    // returned an I/O error with no stdout line.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_directory_with_strict_exits_zero() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    // A directory is a successful detection, not an I/O failure, so
    // `--strict` has nothing to flag.
    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"));
}

#[test]
fn test_symlink_to_directory_classifies_as_directory_by_default() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");
    let link = temp_dir.path().join("dir.link");

    if !symlink_or_skip(
        &dir,
        &link,
        "test_symlink_to_directory_classifies_as_directory_by_default",
    ) {
        return;
    }

    // Default flags follow the link, so the precheck falls through to the
    // directory branch -- matching `file dir.link` -> `directory`.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stdout(predicate::str::contains("symbolic link").not());
}

#[test]
fn test_directory_and_regular_file_in_one_invocation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", path_str(&dir), path_str(&regular)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"))
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("realdir:"));
}

#[test]
fn test_directory_json_output_is_coherent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("directory"));
}

// =============================================================================
// Dereference Flag Tests (issue #383)
// =============================================================================

/// Paths for an ELF file plus a symlink to it via a **relative** target.
///
/// The link target is deliberately relative rather than the absolute path
/// `create_data_file` returns, so tests asserting the verbatim rendering read
/// naturally (`symbolic link to real.elf`). Returns `(target, link)`; the
/// caller still creates the link via `symlink_or_skip`.
fn relative_elf_link_paths(temp_dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    create_data_file(temp_dir, "real.elf", ELF_HEADER);
    (
        std::path::PathBuf::from("real.elf"),
        temp_dir.path().join("valid.link"),
    )
}

#[test]
fn test_no_dereference_reports_the_link_itself() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (target, link) = relative_elf_link_paths(&temp_dir);

    if !symlink_or_skip(
        &target,
        &link,
        "test_no_dereference_reports_the_link_itself",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("symbolic link to real.elf"))
        .stdout(predicate::str::contains("ELF").not());
}

#[test]
fn test_no_dereference_keeps_the_broken_prefix() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_no_dereference_keeps_the_broken_prefix",
    ) {
        return;
    }

    // Brokenness is flag-independent: both states share one reachability
    // probe, so a dangling link never reports as merely `symbolic link to`.
    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ));
}

#[test]
fn test_no_dereference_on_symlink_to_directory_reports_the_link() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");
    let link = temp_dir.path().join("dir.link");

    if !symlink_or_skip(
        &dir,
        &link,
        "test_no_dereference_on_symlink_to_directory_reports_the_link",
    ) {
        return;
    }

    // Proves the precheck runs before the `is_dir()` branch -- `is_dir()`
    // follows symlinks and would otherwise claim this path first.
    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("symbolic link to"))
        .stdout(predicate::str::contains("realdir"));
}

#[test]
fn test_dereference_flag_is_a_no_op_matching_the_default() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("valid.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_dereference_flag_is_a_no_op_matching_the_default",
    ) {
        return;
    }

    for args in [
        vec!["--use-builtin", path_str(&link)],
        vec!["--use-builtin", "-L", path_str(&link)],
    ] {
        rmagic_cmd()
            .args(&args)
            .assert()
            .success()
            .stdout(predicate::str::contains("ELF"));
    }
}

#[test]
fn test_dereference_flags_are_last_one_wins() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (target, link) = relative_elf_link_paths(&temp_dir);

    if !symlink_or_skip(&target, &link, "test_dereference_flags_are_last_one_wins") {
        return;
    }

    // GNU `file` accepts both orders rather than rejecting the pair, so
    // these use `overrides_with`, not `conflicts_with`.
    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", "-L", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));

    rmagic_cmd()
        .args(["--use-builtin", "-L", "--no-dereference", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("symbolic link to real.elf"));
}

#[test]
fn test_no_dereference_leaves_non_symlink_paths_alone() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let dir = temp_dir.path().join("realdir");
    fs::create_dir_all(&dir).expect("Failed to create dir");

    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&regular)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));

    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("directory"));
}

#[test]
fn test_no_dereference_renders_an_absolute_target_verbatim() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("abs.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_no_dereference_renders_an_absolute_target_verbatim",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", "--no-dereference", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "symbolic link to {}",
            path_str(&target)
        )));
}

#[test]
fn test_no_dereference_on_a_valid_link_with_strict_exits_zero() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (target, link) = relative_elf_link_paths(&temp_dir);

    if !symlink_or_skip(
        &target,
        &link,
        "test_no_dereference_on_a_valid_link_with_strict_exits_zero",
    ) {
        return;
    }

    // Declining to read a readable target is not an I/O failure.
    rmagic_cmd()
        .args([
            "--use-builtin",
            "--no-dereference",
            "--strict",
            path_str(&link),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("symbolic link to real.elf"));
}

#[test]
fn test_help_flags_are_unchanged() {
    // Regression guard: GNU `file` spells no-dereference `-h`, but rmagic
    // keeps `-h` bound to `--help`.
    for flag in ["-h", "--help"] {
        rmagic_cmd()
            .arg(flag)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage"))
            .stdout(predicate::str::contains("--no-dereference"));
    }
}
