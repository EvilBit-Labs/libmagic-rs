// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! CLI symlink and directory classification tests (issue #383).
//!
//! Split out of `cli_integration.rs`, which grew past the file-size guidance
//! in AGENTS.md once this suite landed. These cases share a theme -- what
//! `rmagic` reports for a path it classifies itself rather than handing to the
//! magic engine -- and several need the `file` binary as a differential
//! oracle, so they are kept together.

// Test code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per test crate instead.
#![allow(clippy::expect_used)]

mod common;

use common::{ELF_HEADER, create_data_file, path_str, rmagic_cmd, symlink_or_skip, try_symlink};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Symlink Test Helpers
// =============================================================================

#[test]
fn test_symlink_helper_creates_resolvable_file_link() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "target.txt", b"hello");
    let link = temp_dir.path().join("file.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_symlink_helper_creates_resolvable_file_link",
    ) {
        return;
    }

    assert!(
        fs::symlink_metadata(&link)
            .expect("lstat on link")
            .file_type()
            .is_symlink(),
        "helper must create an actual symlink, not a copy"
    );
    assert_eq!(
        fs::read(&link).expect("read through link"),
        b"hello",
        "link must resolve to the target's content"
    );
}

#[test]
fn test_symlink_helper_creates_directory_link() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = temp_dir.path().join("realdir");
    fs::create_dir_all(&target).expect("Failed to create dir");
    let link = temp_dir.path().join("dir.link");

    if !symlink_or_skip(&target, &link, "test_symlink_helper_creates_directory_link") {
        return;
    }

    // `is_dir()` follows symlinks -- this is exactly why the CLI symlink
    // precheck must run before the `is_dir()` branch in `process_file`.
    assert!(
        link.is_dir(),
        "is_dir() must report true through a dir link"
    );
}

#[test]
fn test_symlink_helper_error_arm_is_reachable_and_does_not_panic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "target.txt", b"x");
    let occupied = create_data_file(&temp_dir, "occupied.txt", b"y");

    // Creating a link where a file already exists fails with EEXIST. This
    // exercises the `Err` arm without revoking privileges, proving the
    // runtime-skip path returns cleanly instead of panicking.
    let result = try_symlink(&target, &occupied);
    assert!(
        result.is_err(),
        "creating a link over an existing file must fail"
    );
    // This test drives the skip helper's Err arm on purpose, which
    // RMAGIC_REQUIRE_SYMLINKS deliberately makes fatal -- so only exercise it
    // when that opt-in strict mode is off.
    if std::env::var_os("RMAGIC_REQUIRE_SYMLINKS").is_none() {
        assert!(
            !symlink_or_skip(&target, &occupied, "reachability probe"),
            "skip helper must report false rather than panic on the Err arm"
        );
    }
}

// =============================================================================
// Symlink Classification Tests (issue #383)
// =============================================================================

#[test]
fn test_broken_symlink_reports_to_stdout_and_exits_zero() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_reports_to_stdout_and_exits_zero",
    ) {
        return;
    }

    // GNU `file` prints this to stdout and exits 0. Before this change rmagic
    // printed nothing to stdout and an I/O error to stderr (issue #383).
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_symlink_cycle_reports_broken_not_an_eloop_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("cycle.link");

    // A link pointing at itself: `fs::metadata` fails with ELOOP rather than
    // ENOENT. One reachability probe collapses both into the same output.
    if !symlink_or_skip(
        std::path::Path::new("cycle.link"),
        &link,
        "test_symlink_cycle_reports_broken_not_an_eloop_error",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to cycle.link",
        ))
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_symlink_relative_and_absolute_targets_render_verbatim() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let cases = [
        ("relative.link", "../../elsewhere/x/y.txt"),
        ("absolute.link", "/nonexistent/absolute/target.txt"),
    ];

    for (link_name, target) in cases {
        let link = temp_dir.path().join(link_name);
        if !symlink_or_skip(
            std::path::Path::new(target),
            &link,
            "test_symlink_relative_and_absolute_targets_render_verbatim",
        ) {
            return;
        }

        // No canonicalization and no parent-joining: the stored target is
        // reproduced exactly as `readlink` would report it.
        rmagic_cmd()
            .args(["--use-builtin", path_str(&link)])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "broken symbolic link to {target}"
            )));
    }
}

#[test]
fn test_empty_target_symlink_is_not_reported_as_broken_with_a_blank_target() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("empty.link");

    // `ln -s "" x` is creatable and `read_link` succeeds on it, so an empty
    // target reaches the classifier rather than the fall-through path.
    if !symlink_or_skip(
        std::path::Path::new(""),
        &link,
        "test_empty_target_symlink_is_not_reported_as_broken_with_a_blank_target",
    ) {
        return;
    }

    // The negative is the requirement: `broken symbolic link to ` with a
    // dangling trailing space is a wrong detection result. The replacement
    // diagnostic's wording is ours to choose.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("broken symbolic link to \n").not())
        .stdout(predicate::str::contains("unreadable symlink"));
}

#[test]
fn test_valid_symlink_still_classifies_its_target() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("valid.link");

    if !symlink_or_skip(
        &target,
        &link,
        "test_valid_symlink_still_classifies_its_target",
    ) {
        return;
    }

    // Regression guard: a reachable link must keep resolving through to the
    // target, exactly as before this change.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"))
        .stdout(predicate::str::contains("symbolic link").not());
}

#[test]
fn test_broken_symlink_alongside_regular_file_reports_both() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_alongside_regular_file_reports_both",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link), path_str(&regular)])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        .stdout(predicate::str::contains("ELF"))
        // The `path: ` prefix must survive multi-file output.
        .stdout(predicate::str::contains("dangling.link:"));
}

#[test]
fn test_broken_symlink_json_output_is_coherent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_json_output_is_coherent",
    ) {
        return;
    }

    // The JSON arm builds from `matches`, not `description`, so an empty
    // `matches` would silently produce a valid-but-empty object.
    rmagic_cmd()
        .args(["--use-builtin", "--json", path_str(&link)])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        // `value` reports the file bytes a rule matched. A synthetic
        // classification matched none, so it must stay empty rather than
        // hex-encoding the description prose into a field consumers read as
        // file content.
        .stdout(predicate::str::contains("\"value\": \"\""));
}

#[test]
fn test_broken_symlink_with_strict_exits_non_zero_and_still_prints_to_stdout() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_broken_symlink_with_strict_exits_non_zero_and_still_prints_to_stdout",
    ) {
        return;
    }

    // `--strict` treats an unreadable path as a failure while the
    // classification still reaches stdout -- the distinction a plain `Err`
    // return cannot express.
    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&link)])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "broken symbolic link to missing.txt",
        ))
        // The per-file loop stays silent. The only stderr text is the
        // exit-code explanation, and it names the real cause rather than the
        // misleading "check the file path" advice NotFound would produce.
        .stderr(predicate::str::contains("Error processing").not())
        .stderr(predicate::str::contains("unreadable symlink"));
}

#[test]
fn test_regular_file_with_strict_is_unaffected() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = create_data_file(&temp_dir, "real.elf", ELF_HEADER);

    rmagic_cmd()
        .args(["--use-builtin", "--strict", path_str(&path)])
        .assert()
        .success()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_nonexistent_non_symlink_path_keeps_the_existing_error_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let missing = temp_dir.path().join("no-such-file.bin");

    // Proves the `Err` arm is untouched: a plain missing path still reports
    // to stderr and produces no stdout line.
    rmagic_cmd()
        .args(["--use-builtin", path_str(&missing)])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Error processing"));
}

#[test]
fn test_multi_file_strict_exit_depends_only_on_the_strict_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let regular = create_data_file(&temp_dir, "real.elf", ELF_HEADER);
    let link = temp_dir.path().join("dangling.link");

    if !symlink_or_skip(
        std::path::Path::new("missing.txt"),
        &link,
        "test_multi_file_strict_exit_depends_only_on_the_strict_flag",
    ) {
        return;
    }

    rmagic_cmd()
        .args(["--use-builtin", path_str(&link), path_str(&regular)])
        .assert()
        .success();

    rmagic_cmd()
        .args([
            "--use-builtin",
            "--strict",
            path_str(&link),
            path_str(&regular),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("ELF"));
}

#[test]
fn test_piped_stdin_still_classifies_with_and_without_strict() {
    // The stdin branch returns before the symlink precheck, making it the
    // easiest outcome conversion to miss.
    for extra in [&[][..], &["--strict"][..]] {
        let mut cmd = rmagic_cmd();
        cmd.arg("--use-builtin");
        for flag in extra {
            cmd.arg(flag);
        }
        cmd.arg("-")
            .write_stdin(ELF_HEADER)
            .assert()
            .success()
            .stdout(predicate::str::contains("ELF"));
    }
}

/// Run `file -b <path>`, or return `None` when `file` is unavailable.
///
/// Returns raw bytes: a symlink target need not be valid UTF-8, and the point
/// of the parity assertions is that those bytes survive unchanged.
#[cfg(unix)]
fn file_binary_description(path: &std::path::Path) -> Option<Vec<u8>> {
    let output = std::process::Command::new("file")
        .arg("-b")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut bytes = output.stdout;
    while bytes.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        bytes.pop();
    }
    Some(bytes)
}

#[cfg(unix)]
#[test]
fn test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("ctrl.link");

    // A raw ESC in the target is what a planted link would use to open an
    // OSC/CSI sequence. Captured output must still match `file` exactly --
    // escaping is gated on stdout being a terminal, and `assert_cmd`
    // captures, so this exercises the pass-through branch.
    let target = "esc\u{1b}[2Jx";
    if !symlink_or_skip(
        std::path::Path::new(target),
        &link,
        "test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured",
    ) {
        return;
    }

    let Some(expected) = file_binary_description(&link) else {
        eprintln!(
            "Skipping test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured: \
             the `file` binary is unavailable"
        );
        return;
    };

    // Guard the oracle itself. `file` translates unprintable bytes to octal
    // (`\033`) from 5.42 onward; 5.41 and earlier emit them raw. When the
    // local oracle escapes, it no longer has the property this differential
    // asserts, so there is nothing here to compare against -- skip rather
    // than fail. rmagic's own byte contract is pinned by the unit tests in
    // src/cli/symlink.rs, which need no oracle.
    if !expected.contains(&0x1b) {
        eprintln!(
            "Skipping test_control_byte_target_matches_gnu_file_byte_for_byte_when_captured: \
             this `file` build translates unprintable bytes ({expected:02x?})"
        );
        return;
    }

    let output = rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .output()
        .expect("Failed to run rmagic");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let description = stdout
        .trim_end()
        .rsplit_once(": ")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default();

    assert_eq!(
        description.as_bytes(),
        expected,
        "captured output must match GNU `file` byte-for-byte (ADR-0001)"
    );
}

#[cfg(unix)]
#[test]
fn test_non_utf8_symlink_target_matches_gnu_file_byte_for_byte() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let link = temp_dir.path().join("nonutf8.link");

    // A Unix symlink target is an arbitrary byte string; 0xFF 0xFE is not
    // valid UTF-8. GNU `file` prints those bytes verbatim, so a String
    // round-trip (which substitutes U+FFFD) is a detection-result divergence.
    let target = std::path::Path::new(OsStr::from_bytes(b"bad\xff\xfename.txt"));
    if !symlink_or_skip(
        target,
        &link,
        "test_non_utf8_symlink_target_matches_gnu_file_byte_for_byte",
    ) {
        return;
    }

    let Some(expected) = file_binary_description(&link) else {
        eprintln!(
            "Skipping test_non_utf8_symlink_target_matches_gnu_file_byte_for_byte: \
             the `file` binary is unavailable"
        );
        return;
    };

    // Same oracle guard as the control-byte differential: a `file` build that
    // translates unprintable bytes to octal cannot validate byte parity, so
    // skip instead of failing on a property the oracle no longer has.
    if !(expected.contains(&0xFF) && expected.contains(&0xFE)) {
        eprintln!(
            "Skipping test_non_utf8_symlink_target_matches_gnu_file_byte_for_byte: \
             this `file` build translates unprintable bytes ({expected:02x?})"
        );
        return;
    }

    let output = rmagic_cmd()
        .args(["--use-builtin", path_str(&link)])
        .output()
        .expect("Failed to run rmagic");
    let first_line: Vec<u8> = output
        .stdout
        .split(|b| *b == b'\n')
        .next()
        .unwrap_or_default()
        .to_vec();
    let description = first_line
        .windows(2)
        .position(|w| w == b": ")
        .map(|i| first_line[i + 2..].to_vec())
        .unwrap_or_default();

    assert_eq!(
        description, expected,
        "captured output must match GNU `file` byte-for-byte (ADR-0001)"
    );
}

#[cfg(unix)]
#[test]
fn test_symlink_into_unreadable_directory_reports_broken() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let locked = temp_dir.path().join("locked");
    fs::create_dir_all(&locked).expect("Failed to create dir");
    let target = locked.join("hidden.txt");
    fs::write(&target, ELF_HEADER).expect("Failed to write target");

    let link = temp_dir.path().join("eacces.link");
    if !symlink_or_skip(
        &target,
        &link,
        "test_symlink_into_unreadable_directory_reports_broken",
    ) {
        return;
    }

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
        .expect("Failed to lock directory");

    // Root ignores directory permissions, so the EACCES scenario cannot be
    // constructed there. Probe rather than checking the uid, which would
    // need an unsafe libc call this crate forbids.
    let is_effective = fs::read(&target).is_err();

    if is_effective {
        rmagic_cmd()
            .args(["--use-builtin", path_str(&link)])
            .assert()
            .success()
            // EACCES collapses into the same output as ENOENT and ELOOP.
            .stdout(predicate::str::contains("broken symbolic link to"));
    } else {
        eprintln!(
            "Skipping test_symlink_into_unreadable_directory_reports_broken: \
             directory permissions are not enforced for this user"
        );
    }

    // Restore before the TempDir drop, or cleanup fails.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("Failed to restore directory permissions");
}
