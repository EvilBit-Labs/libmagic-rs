// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Staging a magic database that both rmagic and real `file` provably read.
//!
//! The oracle cannot simply be `file --magic-file <system dir>`. Measured with
//! `file-5.41`: `--magic-file <dir>` prefers a sibling compiled `<dir>.mgc`
//! over the directory itself, and `/usr/share/file/magic.mgc` exists next to
//! `/usr/share/file/magic`. A directory holding only a sentinel rule still
//! yields the built-in classification, so the naive form compares rmagic on
//! source files against `file` on the compiled database. Staging a copy of the
//! source directory and pointing `MAGIC=` at it avoids that.

use std::path::Path;
use std::process::Command;

pub const SYSTEM_MAGIC_DIR: &str = "/usr/share/file/magic";

/// Whether a like-for-like comparison against `file` is possible here.
pub enum OracleReadiness {
    /// A staged copy of the system magic sources, at `<dir>/magic`.
    Ready(tempfile::TempDir),
    /// The environment cannot support a like-for-like comparison.
    Skip(String),
}

pub fn has_file_binary() -> bool {
    Command::new("file")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Count the plain files in `dir`.
///
/// Debian and Ubuntu ship only the compiled `magic.mgc` and leave the source
/// directory empty; that is the case a zero count detects. A directory that
/// cannot be read is an error, not an empty directory.
pub fn magic_source_file_count(dir: &Path) -> std::io::Result<usize> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .count())
}

/// Ask `file` to classify `target` using only `magic_dir`.
///
/// `MAGIC=` rather than `--magic-file`: measured on this host, the flag did not
/// restrict the database while the environment variable did. Panics with
/// `file`'s stderr when it exits non-zero, so a rejected database is not
/// mistaken for an empty classification.
pub fn file_says(magic_dir: &Path, target: &str) -> String {
    let output = Command::new("file")
        .env("MAGIC", magic_dir)
        .arg("-b")
        .arg(target)
        .output()
        .expect("invoking `file` must not fail once it is known present");
    assert!(
        output.status.success(),
        "`file` failed on {target} with MAGIC={}: {}",
        magic_dir.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Stage a magic directory `file` demonstrably reads, or explain why not.
///
/// The canary is the load-bearing step: point `file` at a database knowing
/// only a sentinel type and require `canary_target` NOT to classify as
/// `real_class`. If it still does, `file` is answering from some other
/// database and no comparison would mean anything, so this panics.
pub fn stage_system_magic(canary_target: &str, real_class: &str) -> OracleReadiness {
    let system_dir = Path::new(SYSTEM_MAGIC_DIR);
    if !system_dir.is_dir() {
        return OracleReadiness::Skip(format!("{SYSTEM_MAGIC_DIR} is not present"));
    }
    match magic_source_file_count(system_dir) {
        Ok(0) => {
            return OracleReadiness::Skip(format!(
                "{SYSTEM_MAGIC_DIR} holds no source magic files (compiled-only install)"
            ));
        }
        Ok(_) => {}
        Err(e) => {
            return OracleReadiness::Skip(format!("{SYSTEM_MAGIC_DIR} is unreadable: {e}"));
        }
    }
    if !has_file_binary() {
        return OracleReadiness::Skip("`file` is not on PATH".to_string());
    }

    let staged = tempfile::TempDir::new().expect("temp dir for the staged magic copy");

    let canary_dir = staged.path().join("canary");
    std::fs::create_dir_all(&canary_dir).expect("create canary dir");
    std::fs::write(
        canary_dir.join("sentinel"),
        "0\tstring\tZZ-SENTINEL-NEVER-MATCHES\tsentinel\n",
    )
    .expect("write sentinel rule");
    let canary = file_says(&canary_dir, canary_target);
    assert!(
        !canary.contains(real_class),
        "`file` classified {canary_target} as {real_class:?} from a database \
         holding only a sentinel rule, so it is answering from some other \
         database and this comparison would be meaningless. Got: {canary:?}"
    );

    let magic_copy = staged.path().join("magic");
    std::fs::create_dir_all(&magic_copy).expect("create staged magic dir");
    for entry in std::fs::read_dir(system_dir).expect("read system magic dir") {
        let entry = entry.expect("read system magic entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), magic_copy.join(entry.file_name()))
                .expect("copy magic source file");
        }
    }
    OracleReadiness::Ready(staged)
}
