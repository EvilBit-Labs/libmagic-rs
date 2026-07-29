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
    ///
    /// Bytes, not `String`: a symlink target is an arbitrary byte string on
    /// Unix and need not be valid UTF-8, and GNU `file` reproduces those bytes
    /// verbatim. Routing the description through a `String` would replace the
    /// invalid ones with U+FFFD and break the detection-result contract.
    pub description: Vec<u8>,
    /// Whether the path turned out to be unreadable.
    ///
    /// `--strict` surfaces these; a default run still prints the description
    /// to stdout and exits 0.
    pub unreadable: bool,
}

/// Raw bytes of a path, without lossy UTF-8 conversion where the OS allows it
fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    // Windows paths are UTF-16 with no raw-byte equivalent, so a lossy
    // conversion is the only option there -- and Windows symlink targets are
    // already constrained to valid Unicode, so nothing is lost in practice.
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned().into_bytes()
    }
}

/// Render a symlink target for display
///
/// Returns raw bytes rather than a `String`. On Unix a symlink target is an
/// arbitrary byte string; GNU `file` prints it verbatim, so passing it through
/// `String` (which must be valid UTF-8) would substitute U+FFFD for any invalid
/// byte and change the output. Verified against `file-5.41`: for a target
/// containing `0xFF 0xFE`, `file` emits those two bytes unchanged.
///
/// When `escape_control_bytes` is set, characters a terminal would act on are
/// rendered inert: C0 controls and DEL, the C1 range (whose UTF-8 forms a
/// terminal decodes to 8-bit CSI/OSC), and the Unicode bidi/format overrides
/// that let a target display as something other than its real bytes.
///
/// Callers set the flag from [`stdout_is_terminal`]. The pass-through branch is
/// what keeps redirected and piped output byte-for-byte identical to GNU
/// `file` -- do not collapse the two branches into unconditional escaping.
pub fn render_symlink_target(target: &Path, escape_control_bytes: bool) -> Vec<u8> {
    use std::fmt::Write;

    let raw = path_bytes(target);
    if !escape_control_bytes {
        // The parity branch: verbatim, including invalid UTF-8.
        return raw;
    }

    // The presentation branch. This one only ever reaches an interactive
    // terminal, so a lossy decode is acceptable here -- unlike above, no
    // byte-for-byte contract applies, and a terminal cannot render an invalid
    // sequence meaningfully anyway.
    let decoded = String::from_utf8_lossy(&raw);
    let mut escaped = String::with_capacity(decoded.len());
    for character in decoded.chars() {
        let code = character as u32;
        if is_terminal_control(code) {
            // fmt::Write to a String is infallible; discard the Result
            // rather than unwrap so the no-panic policy holds regardless.
            #[allow(clippy::let_underscore_must_use)]
            let _ = if code <= 0xFF {
                write!(escaped, "\\x{code:02x}")
            } else {
                write!(escaped, "\\u{{{code:04x}}}")
            };
        } else {
            escaped.push(character);
        }
    }
    escaped.into_bytes()
}

/// Whether a terminal would act on this character rather than print it
///
/// Covers three families, all of which a planted symlink target can carry:
/// C0 controls and DEL; the C1 range, whose UTF-8 encoding a terminal in UTF-8
/// mode decodes back to 8-bit CSI/OSC/ST (so `U+009D` opens an OSC sequence
/// exactly as `ESC ]` does); and the bidi/format overrides behind
/// Trojan-Source-style spoofing, which can make a target display as a different
/// path than its bytes describe.
fn is_terminal_control(code: u32) -> bool {
    const BIDI_OVERRIDES: [u32; 9] = [
        0x200E, 0x200F, // LRM, RLM
        0x061C, // ARABIC LETTER MARK
        0x202A, 0x202B, 0x202C, 0x202D, 0x202E, // embedding / override
        0x2066, // isolates start; 0x2066..=0x2069 handled by the range below
    ];

    code < 0x20
        || code == 0x7F
        || (0x80..=0x9F).contains(&code)
        || (0x2066..=0x2069).contains(&code)
        || BIDI_OVERRIDES.contains(&code)
}

/// Concatenate a literal description prefix with a rendered target
///
/// Kept byte-level rather than using `format!` so a non-UTF-8 target survives
/// into the output unchanged.
fn prefixed(prefix: &[u8], target: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(prefix.len() + target.len());
    out.extend_from_slice(prefix);
    out.extend_from_slice(target);
    out
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
            )
            .into_bytes(),
            unreadable: true,
        });
    }

    // One reachability probe covers ENOENT (missing target), ELOOP (cycle),
    // and EACCES (unreadable parent directory) alike, which is what gives all
    // three `file`-identical output with no per-errno branch.
    if std::fs::metadata(path).is_err() {
        return Some(SymlinkClassification {
            description: prefixed(
                b"broken symbolic link to ",
                &render_symlink_target(&target, escape_control_bytes),
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
        description: prefixed(
            b"symbolic link to ",
            &render_symlink_target(&target, escape_control_bytes),
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
                rendered,
                expected.as_bytes(),
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
                rendered,
                expected.as_bytes(),
                "interactive output must escape control bytes for {input:?}"
            );
        }
    }

    #[test]
    fn test_render_symlink_target_escapes_c1_controls_and_bidi_overrides_on_a_terminal() {
        // A terminal in UTF-8 mode decodes the UTF-8 form of the C1 range back
        // to 8-bit CSI/OSC, so U+009D opens an OSC sequence exactly as `ESC ]`
        // does. Bidi overrides let a target display as a different path than
        // its bytes describe.
        let cases: &[(&str, &str)] = &[
            ("osc\u{9d}0;title", "osc\\x9d0;title"),
            ("csi\u{9b}2J", "csi\\x9b2J"),
            ("low\u{80}", "low\\x80"),
            ("high\u{9f}", "high\\x9f"),
            ("rtl\u{202e}txt.exe", "rtl\\u{202e}txt.exe"),
            ("iso\u{2066}x", "iso\\u{2066}x"),
            ("lrm\u{200e}x", "lrm\\u{200e}x"),
            // Just outside the escaped ranges -- must survive untouched.
            ("ok\u{a0}x", "ok\u{a0}x"),
            ("caf\u{e9}", "caf\u{e9}"),
        ];

        for (input, expected) in cases {
            let rendered = render_symlink_target(Path::new(input), true);
            assert_eq!(
                rendered,
                expected.as_bytes(),
                "interactive output must neutralize {input:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_render_symlink_target_preserves_non_utf8_bytes_when_not_a_terminal() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // A Unix symlink target is an arbitrary byte string. GNU `file`
        // reproduces it verbatim; a String round-trip would substitute U+FFFD
        // and change the bytes, breaking the detection-result contract.
        let raw = b"bad\xff\xfename.txt";
        let target = Path::new(OsStr::from_bytes(raw));

        assert_eq!(
            render_symlink_target(target, false),
            raw,
            "the captured-output branch must pass invalid UTF-8 through unchanged"
        );

        // The interactive branch may lossily decode -- it only ever reaches a
        // terminal, which cannot render an invalid sequence anyway -- but it
        // must not panic and must leave the valid bytes intact.
        let escaped = render_symlink_target(target, true);
        assert!(escaped.starts_with(b"bad"), "valid prefix must survive");
        assert!(escaped.ends_with(b"name.txt"), "valid suffix must survive");
    }

    #[cfg(unix)]
    #[test]
    fn test_classify_symlink_distinguishes_followed_from_not_a_symlink() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let target = temp_dir.path().join("real.txt");
        std::fs::write(&target, b"content").unwrap();
        let link = temp_dir.path().join("valid.link");
        std::os::unix::fs::symlink("real.txt", &link).unwrap();

        // Both of these return None, but for different reasons -- the trap
        // GOTCHAS S17.5a documents. A refactor that conflates them silently
        // reclassifies every symlink-to-directory.
        assert!(
            classify_symlink(&link, true, false).is_none(),
            "a reachable symlink under follow must fall through to the target"
        );

        let classified = classify_symlink(&link, false, false)
            .expect("a reachable symlink under no-follow must be classified");
        assert_eq!(classified.description, b"symbolic link to real.txt");
        assert!(
            !classified.unreadable,
            "a readable target rmagic declined to read is not an I/O failure"
        );
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
