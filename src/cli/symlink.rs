// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Symlink classification, run ahead of magic-rule evaluation.
//!
//! GNU `file` reports symlinks itself rather than handing them to the magic
//! engine, and rmagic does the same here in the CLI layer so `src/lib.rs` and
//! `src/io/` stay untouched. That placement is forced, not stylistic:
//! `fs::metadata` follows symlinks, so a dangling link fails inside
//! `evaluate_file` before a `FileBuffer` is ever constructed, and no fix
//! confined to the library layer can reach it.
//!
//! See `docs/adr/0001-gnu-file-output-contract.md` for which strings here are
//! binding detection results and which are diagnostics we are free to word.

use std::path::Path;

/// A CLI-produced classification for a symlink path
pub struct SymlinkClassification {
    /// The description to print, e.g. `broken symbolic link to missing.txt`
    pub description: String,
    /// Whether the path turned out to be unreadable.
    ///
    /// `--strict` surfaces these; a default run still prints the description
    /// to stdout and exits 0.
    pub unreadable: bool,
}

/// Render a symlink target for display
///
/// Symlink targets carry no character restrictions, so a planted link can hold
/// raw ESC or OSC bytes. When `escape_control_bytes` is set, bytes below 0x20
/// and 0x7F render as `\xHH`; otherwise the target passes through unchanged.
///
/// Callers set the flag from [`stdout_is_terminal`]. The pass-through branch is
/// what keeps redirected and piped output byte-for-byte identical to GNU
/// `file` -- do not collapse the two branches into unconditional escaping.
pub fn render_symlink_target(target: &Path, escape_control_bytes: bool) -> String {
    let rendered = target.to_string_lossy();
    if !escape_control_bytes {
        return rendered.into_owned();
    }

    let mut escaped = String::with_capacity(rendered.len());
    for character in rendered.chars() {
        let code = character as u32;
        if code < 0x20 || code == 0x7F {
            escaped.push('\\');
            escaped.push('x');
            // Both nibbles of a byte are always valid radix-16 digits, so
            // the fallbacks below are unreachable; they exist only to keep
            // this panic-free.
            escaped.push(char::from_digit(code >> 4, 16).unwrap_or('0'));
            escaped.push(char::from_digit(code & 0xF, 16).unwrap_or('0'));
        } else {
            escaped.push(character);
        }
    }
    escaped
}

/// Classify `path` when it is a symlink
///
/// Returns `None` when `path` is not a symlink, or when the link itself cannot
/// be inspected -- in both cases the caller continues with ordinary file
/// classification.
///
/// Must run before any `is_dir()` check: `Path::is_dir()` follows symlinks, so
/// a symlink-to-directory reports `is_dir() == true` and would be consumed by
/// a directory branch before this could run.
pub fn classify_symlink(
    path: &Path,
    follows_symlinks: bool,
    escape_control_bytes: bool,
) -> Option<SymlinkClassification> {
    // lstat -- does not follow the link.
    if !std::fs::symlink_metadata(path)
        .ok()?
        .file_type()
        .is_symlink()
    {
        return None;
    }

    // The stored target, verbatim: no canonicalization, no parent-joining.
    let target = std::fs::read_link(path).ok()?;

    // `ln -s "" x` is creatable and `read_link` succeeds on it, returning an
    // empty path, so it arrives here rather than at the fall-through above.
    // Without this branch it would render as `broken symbolic link to ` with a
    // dangling trailing space -- a wrong detection result.
    if target.as_os_str().is_empty() {
        return Some(SymlinkClassification {
            description: format!(
                "unreadable symlink `{}' (No such file or directory)",
                path.display()
            ),
            unreadable: true,
        });
    }

    // One reachability probe covers ENOENT (missing target), ELOOP (cycle),
    // and EACCES (unreadable parent directory) alike, which is what gives all
    // three `file`-identical output with no per-errno branch.
    if std::fs::metadata(path).is_err() {
        return Some(SymlinkClassification {
            description: format!(
                "broken symbolic link to {}",
                render_symlink_target(&target, escape_control_bytes)
            ),
            unreadable: true,
        });
    }

    if follows_symlinks {
        // Reachable, and we were asked to follow: fall through so the target
        // itself gets classified.
        return None;
    }

    // Reachable but not followed. Not `unreadable`: the target is readable,
    // rmagic simply chose not to read it, so `--strict` has nothing to flag.
    Some(SymlinkClassification {
        description: format!(
            "symbolic link to {}",
            render_symlink_target(&target, escape_control_bytes)
        ),
        unreadable: false,
    })
}

#[cfg(test)]
mod tests {
    // Test code is exempt from the panic-safety restriction lints (see
    // clippy.toml), which have no allow-in-tests config option.
    #![allow(clippy::unwrap_used)]

    use super::*;

    // =========================================================================
    // Symlink target rendering (issue #383)
    //
    // These are unit tests rather than integration tests because this module
    // belongs to the binary crate and is unreachable from tests/. `assert_cmd`
    // also always captures stdout, so the TTY branch can only be exercised by
    // calling the helper with an explicit flag.
    // =========================================================================

    #[test]
    fn test_render_symlink_target_passes_control_bytes_through_when_not_a_terminal() {
        let cases: &[(&str, &str)] = &[
            ("plain.txt", "plain.txt"),
            ("../../up/two.txt", "../../up/two.txt"),
            ("/absolute/target", "/absolute/target"),
            ("esc\u{1b}[2Jclear", "esc\u{1b}[2Jclear"),
            ("bell\u{7}", "bell\u{7}"),
            ("del\u{7f}", "del\u{7f}"),
        ];

        for (input, expected) in cases {
            let rendered = render_symlink_target(Path::new(input), false);
            assert_eq!(
                rendered, *expected,
                "captured output must pass bytes through verbatim for {input:?} \
                 -- this is the branch that preserves GNU `file` parity"
            );
        }
    }

    #[test]
    fn test_render_symlink_target_escapes_control_bytes_on_a_terminal() {
        let cases: &[(&str, &str)] = &[
            // No control bytes -- escaping must not alter ordinary targets.
            ("plain.txt", "plain.txt"),
            ("../../up/two.txt", "../../up/two.txt"),
            // ESC opens the OSC/CSI sequences a planted link could abuse.
            ("esc\u{1b}[2Jclear", "esc\\x1b[2Jclear"),
            ("bell\u{7}", "bell\\x07"),
            ("del\u{7f}", "del\\x7f"),
            ("tab\there", "tab\\x09here"),
            ("nl\nhere", "nl\\x0ahere"),
            // Non-ASCII is not a control byte and must survive intact.
            ("caf\u{e9}", "caf\u{e9}"),
        ];

        for (input, expected) in cases {
            let rendered = render_symlink_target(Path::new(input), true);
            assert_eq!(
                rendered, *expected,
                "interactive output must escape control bytes for {input:?}"
            );
        }
    }

    #[test]
    fn test_classify_symlink_returns_none_for_a_regular_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("regular.txt");
        std::fs::write(&path, b"content").unwrap();

        assert!(
            classify_symlink(&path, true, false).is_none(),
            "a regular file must fall through to ordinary classification"
        );
    }

    #[test]
    fn test_classify_symlink_returns_none_for_a_missing_path() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("does-not-exist");

        assert!(
            classify_symlink(&path, true, false).is_none(),
            "a nonexistent non-symlink path must keep its existing error path"
        );
    }
}
