// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Caller-level coverage for terminal-gated description escaping
//! (issue #498, U5).
//!
//! U5 extends the terminal-inertness contract already applied to symlink
//! targets (GOTCHAS S17.3) to both magic-description text-output paths: the
//! library-produced description (`output_result`'s `Text` arm, for ordinary
//! files) and the CLI-produced description bytes (`output_description_bytes`,
//! currently reached by the symlink path). Both must be inert on an
//! interactive terminal (R9) and byte-identical to GNU `file` when captured
//! (R10); JSON carries no file-derived text and is unaffected either way
//! (R11).
//!
//! ## Why no pseudo-terminal here
//!
//! `assert_cmd::Command` always pipes stdout, so it can only ever exercise
//! the pass-through (non-terminal) branch of both paths -- confirmed by the
//! existing `src/cli/symlink.rs` test-module doc comment, which notes the
//! same limitation for `render_symlink_target`. Proving the *terminal*
//! branch through the real compiled binary needs a pseudo-terminal, and that
//! was evaluated and found impractical within this unit's constraints:
//!
//!   - This unit may touch only `src/cli/symlink.rs`, `src/cli/mod.rs`,
//!     `src/main.rs`, and this file -- `Cargo.toml` is out of scope, and no
//!     PTY crate (`portable-pty`, `expectrl`, `rexpect`, ...) is already a
//!     dependency.
//!   - A hand-rolled PTY via `libc::openpty`/`posix_openpt` would need
//!     `unsafe`, which this crate denies project-wide except one vetted
//!     memmap2 exception (GOTCHAS S8.2); adding a second exception for test
//!     code is not a call this unit can make unilaterally.
//!
//! The closest achievable caller-level proof for the terminal branch is
//! therefore a *direct call* to the real production entry points
//! (`output_result` in `src/main.rs`, `output_description_bytes` in
//! `src/cli/mod.rs`) with an explicit escape flag, rather than relying on
//! `stdout_is_terminal()` to observe a real terminal. This is the same
//! testability pattern `render_symlink_target` already uses (it takes the
//! terminal flag as a parameter specifically so both branches are
//! unit-testable) applied one layer up. Those tests live in each file's own
//! `#[cfg(test)]` module, since both functions are private to the binary
//! crate and unreachable from here; see:
//!   - `src/main.rs::tests::test_output_result_text_escapes_when_flag_is_set`
//!     (and its pass-through and JSON-unaffected siblings)
//!   - `src/cli/mod.rs::tests::test_output_description_bytes_text_escapes_when_flag_is_set`
//!     (and its siblings)
//!
//! What THIS file proves instead, end-to-end through the real compiled
//! binary: that the pass-through (R10) contract holds for both paths, and
//! that JSON is unaffected, which is everything `assert_cmd` can observe.

#![allow(clippy::expect_used)]

mod common;

use common::{create_data_file, path_str, rmagic_cmd, symlink_or_skip};
use predicates::prelude::*;
use tempfile::TempDir;

/// A raw OSC-title-style control sequence: `ESC ] 0 ; pwn BEL`.
///
/// The scenario named in the U5 requirements -- exactly what a planted
/// description (or symlink target) could use to rewrite a terminal's title
/// or worse via OSC 52.
const OSC_SEQUENCE: &str = "\u{1b}]0;pwn\u{7}";

/// Write a magic file whose single rule matches literal file content and
/// carries the OSC sequence directly in its message text.
///
/// The bytes are written as-is (not as a `\x1b`-style magic-file escape), so
/// `parse_message` captures them verbatim (GOTCHAS S14.1) with no escape
/// resolution involved -- this is testing the CLI output path, not the
/// parser.
fn control_byte_magic_file(dir: &TempDir) -> std::path::PathBuf {
    let content = format!("0\tstring\tMARK\tbefore{OSC_SEQUENCE}after\n");
    create_data_file(dir, "control.magic", content.as_bytes())
}

// =============================================================================
// Library-produced description path (`output_result`'s Text arm)
// =============================================================================

#[test]
fn test_library_description_with_control_bytes_is_verbatim_when_captured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = control_byte_magic_file(&temp_dir);
    let data_file = create_data_file(&temp_dir, "sample.bin", b"MARK");

    // `assert_cmd` pipes stdout, so this is the pass-through branch (R10):
    // the raw ESC and BEL bytes must reach stdout unchanged, matching GNU
    // `file`'s own behavior for captured output.
    let output = rmagic_cmd()
        .args(["--magic-file", path_str(&magic_file), path_str(&data_file)])
        .output()
        .expect("Failed to run rmagic");

    assert!(output.status.success());
    let expected = format!("before{OSC_SEQUENCE}after");
    assert!(
        output
            .stdout
            .windows(expected.len())
            .any(|w| w == expected.as_bytes()),
        "captured output must carry the raw control bytes verbatim, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_library_description_json_is_unescaped_regardless_of_terminal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let magic_file = control_byte_magic_file(&temp_dir);
    let data_file = create_data_file(&temp_dir, "sample.bin", b"MARK");

    // R11: JSON carries no file-derived text needing escaping. This proves
    // the JSON arm is untouched end-to-end, through the real binary.
    rmagic_cmd()
        .args([
            "--magic-file",
            path_str(&magic_file),
            "--json",
            path_str(&data_file),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("pwn"));
}

// =============================================================================
// CLI-produced description-bytes path (`output_description_bytes`, reached
// today via the symlink classifier)
// =============================================================================

#[test]
fn test_symlink_description_with_control_bytes_is_verbatim_when_captured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("ctrl.link");
    let target = format!("before{OSC_SEQUENCE}after");

    if !symlink_or_skip(
        std::path::Path::new(&target),
        &link,
        "test_symlink_description_with_control_bytes_is_verbatim_when_captured",
    ) {
        return;
    }

    // Same pass-through contract (R10), through the CLI-produced
    // description-bytes path rather than the library-produced one.
    let output = rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .output()
        .expect("Failed to run rmagic");

    assert!(output.status.success());
    let expected = format!("broken symbolic link to before{OSC_SEQUENCE}after");
    assert!(
        output
            .stdout
            .windows(expected.len())
            .any(|w| w == expected.as_bytes()),
        "captured output must carry the raw control bytes verbatim, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_symlink_description_json_is_unescaped_regardless_of_terminal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("ctrl.link");
    let target = format!("before{OSC_SEQUENCE}after");

    if !symlink_or_skip(
        std::path::Path::new(&target),
        &link,
        "test_symlink_description_json_is_unescaped_regardless_of_terminal",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("pwn"));
}
