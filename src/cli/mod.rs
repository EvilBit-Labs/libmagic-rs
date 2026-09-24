// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! CLI-only support modules for the `rmagic` binary.
//!
//! These are part of the binary crate, not the library: they encode
//! command-line presentation decisions that library consumers make for
//! themselves.

pub mod symlink;

use libmagic_rs::LibmagicError;
use std::io::Write;
use std::path::Path;

/// How a single input path was resolved
///
/// A broken symlink must reach stdout *and* count toward `--strict` *and* stay
/// off stderr. `Result<(), LibmagicError>` cannot express that, because its
/// `Err` arm is what drives the stderr report.
pub enum FileOutcome {
    /// Classified normally; nothing for `--strict` to flag
    Classified,
    /// Classified and written to stdout, but the path was unreadable.
    ///
    /// `--strict` surfaces this; a default run must not print to stderr.
    ClassifiedUnreadable(LibmagicError),
}

/// Whether stdout is an interactive terminal, resolved once per run
pub fn stdout_is_terminal() -> bool {
    static IS_TERMINAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_TERMINAL.get_or_init(|| std::io::IsTerminal::is_terminal(&std::io::stdout()))
}

/// Build a synthetic `EvaluationResult` carrying a CLI-produced description
///
/// Both `description` and one `matches` entry are populated. The text output
/// arm reads `description` and never touches `matches`, while the JSON arm
/// builds from `matches` -- populating only one leaves the other empty.
pub fn synthetic_result(description: &str) -> libmagic_rs::EvaluationResult {
    let rule_match = libmagic_rs::RuleMatch::new(
        description.to_string(),
        0,
        0,
        // Empty, not the description: `value` reports the file bytes a rule
        // matched, and a synthetic classification matched none. Filling it
        // with the description hex-encodes prose into a field consumers read
        // as file content.
        libmagic_rs::Value::Bytes(Vec::new()),
        libmagic_rs::TypeKind::String {
            max_length: None,
            flags: libmagic_rs::parser::ast::StringFlags::default(),
        },
        1.0,
    );

    libmagic_rs::EvaluationResult::new(
        description.to_string(),
        None,
        1.0,
        vec![rule_match],
        libmagic_rs::EvaluationMetadata::new(0, 0.0, 0, None, false),
    )
}

/// Write a CLI-produced description whose bytes may not be valid UTF-8
///
/// The text arm writes the description bytes verbatim, which is what keeps a
/// non-UTF-8 symlink target byte-for-byte identical to GNU `file`. Routing it
/// through `output_result` would require a `String` and substitute U+FFFD for
/// every invalid byte.
///
/// `escape_control_bytes` gates the same terminal-inertness contract as
/// [`symlink::render_symlink_target`] (issue #498, U5): the text arm escapes
/// terminal-actionable bytes in the already-assembled description when set,
/// and passes them through unchanged when not. Callers resolve the flag once
/// per run via [`stdout_is_terminal`].
///
/// The JSON arm still goes through `output_result`, decoding lossily: JSON
/// strings must be valid UTF-8, so there is no byte-exact form to preserve,
/// and `file` has no JSON output to match against. JSON carries no
/// file-derived text (R11), so the flag has no effect there; it is threaded
/// through only because `output_result` takes it.
pub fn output_description_bytes(
    writer: &mut impl Write,
    file_path: &Path,
    description: &[u8],
    args: &crate::Args,
    is_multiple_files: bool,
    escape_control_bytes: bool,
) -> Result<(), LibmagicError> {
    match args.output_format() {
        crate::OutputFormat::Text => {
            write!(writer, "{}: ", file_path.display()).map_err(LibmagicError::IoError)?;
            let rendered =
                symlink::escape_terminal_control_bytes(description, escape_control_bytes);
            writer
                .write_all(&rendered)
                .map_err(LibmagicError::IoError)?;
            writeln!(writer).map_err(LibmagicError::IoError)?;
            Ok(())
        }
        crate::OutputFormat::Json => {
            let result = synthetic_result(&String::from_utf8_lossy(description));
            crate::output_result(
                writer,
                file_path,
                &result,
                args,
                is_multiple_files,
                escape_control_bytes,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    // Test code is exempt from the panic-safety restriction lints (see
    // clippy.toml), which have no allow-in-tests config option.
    #![allow(clippy::unwrap_used)]

    use super::*;
    use clap::Parser as _;

    // =========================================================================
    // `output_description_bytes` wiring (issue #498, U5)
    //
    // These call the real production entry point `process_file` uses for the
    // CLI-produced description path, with an explicit escape flag -- the same
    // testability pattern `render_symlink_target` already uses -- so the
    // terminal branch is provable without a live terminal. `assert_cmd`
    // always captures stdout, so an integration test can only ever reach the
    // pass-through branch; see tests/description_escaping_tests.rs for that
    // half and for why a pseudo-terminal proof was not practical here.
    // =========================================================================

    fn args_text() -> crate::Args {
        crate::Args::try_parse_from(["rmagic", "unused.bin"]).unwrap()
    }

    fn args_json() -> crate::Args {
        crate::Args::try_parse_from(["rmagic", "--json", "unused.bin"]).unwrap()
    }

    #[test]
    fn test_output_description_bytes_text_escapes_when_flag_is_set() {
        let mut buf = Vec::new();
        let description = b"before\x1b]0;pwn\x07after";
        let args = args_text();

        output_description_bytes(
            &mut buf,
            Path::new("f.bin"),
            description,
            &args,
            false,
            true,
        )
        .unwrap();

        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "f.bin: before\\x1b]0;pwn\\x07after\n");
    }

    #[test]
    fn test_output_description_bytes_text_is_verbatim_when_flag_is_unset() {
        let mut buf = Vec::new();
        let description = b"before\x1b]0;pwn\x07after";
        let args = args_text();

        output_description_bytes(
            &mut buf,
            Path::new("f.bin"),
            description,
            &args,
            false,
            false,
        )
        .unwrap();

        assert_eq!(buf, b"f.bin: before\x1b]0;pwn\x07after\n");
    }

    #[test]
    fn test_output_description_bytes_json_is_unaffected_by_the_escape_flag() {
        let description = b"before\x1b]0;pwn\x07after";
        let args = args_json();

        let mut escaped_run = Vec::new();
        output_description_bytes(
            &mut escaped_run,
            Path::new("f.bin"),
            description,
            &args,
            false,
            true,
        )
        .unwrap();

        let mut plain_run = Vec::new();
        output_description_bytes(
            &mut plain_run,
            Path::new("f.bin"),
            description,
            &args,
            false,
            false,
        )
        .unwrap();

        assert_eq!(
            escaped_run, plain_run,
            "R11: JSON carries no file-derived text, so the escape flag must not change it"
        );
        let json = String::from_utf8(escaped_run).unwrap();
        assert!(
            json.contains("pwn"),
            "the raw control bytes must still reach JSON unescaped: {json}"
        );
    }
}
