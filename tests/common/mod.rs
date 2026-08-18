// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Helpers shared across the CLI integration-test binaries.
//!
//! Each file under `tests/` compiles as its own binary, so a helper used by
//! more than one of them lives here rather than being duplicated. A `tests/`
//! subdirectory is not itself compiled as a test binary, which is what makes
//! this the idiomatic home for shared code.

// Each test binary pulls in the whole module but uses only part of it, so
// unused-item warnings here are expected rather than a signal.
#![allow(dead_code)]
// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option.
#![allow(clippy::expect_used)]

use assert_cmd::Command;
use tempfile::TempDir;

/// ELF magic bytes, the workhorse fixture for "this file is detectable"
pub const ELF_HEADER: &[u8] = b"\x7fELF\x02\x01\x01\x00";

/// Build a Command for the rmagic binary
pub fn rmagic_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("rmagic"))
}

/// Write a temporary data file and return its path
pub fn create_data_file(dir: &TempDir, filename: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(filename);
    std::fs::write(&path, content).expect("Failed to create data file");
    path
}

/// Convert a path to a string, panicking with context on failure
pub fn path_str(path: &std::path::Path) -> &str {
    path.to_str().expect("Invalid path")
}

/// Create a symlink at `link` pointing at `target`.
///
/// `FileBuffer::create_symlink` is `pub(crate)` and so is not reachable from an
/// integration test; this mirrors its three-arm platform dispatch. The `#[cfg]`
/// blocks sit inside the body so the function itself compiles everywhere.
pub fn try_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlink creation is not supported on this platform",
        ))
    }
}

/// Create a symlink, or report that the caller should skip.
///
/// Returns `false` after printing a skip message when symlink creation is not
/// permitted (Windows without developer mode, restricted CI sandboxes). Tests
/// skip at runtime rather than compiling out under `#[cfg(unix)]`, so the
/// symlink suite still runs wherever symlinks happen to be available. Mirrors
/// `src/io/mod.rs`'s `test_file_buffer_symlink_to_directory_rejection`.
#[must_use]
pub fn symlink_or_skip(target: &std::path::Path, link: &std::path::Path, test_name: &str) -> bool {
    match try_symlink(target, link) {
        Ok(()) => true,
        Err(e) => {
            // A silent skip is the failure mode that matters here: if symlink
            // creation is denied for the whole run, every symlink test returns
            // early having asserted nothing and the suite still reports green.
            // Setting RMAGIC_REQUIRE_SYMLINKS=1 turns that into a hard failure,
            // so CI on a platform where symlinks must work cannot pass
            // vacuously. Unset, the runtime skip is preserved.
            assert!(
                std::env::var_os("RMAGIC_REQUIRE_SYMLINKS").is_none(),
                "{test_name}: symlink creation failed ({e}) but \
                 RMAGIC_REQUIRE_SYMLINKS is set, so skipping is not permitted"
            );
            eprintln!("Skipping {test_name}: cannot create symlink ({e})");
            false
        }
    }
}
