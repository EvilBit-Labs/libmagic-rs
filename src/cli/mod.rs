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
        libmagic_rs::Value::String(description.to_string()),
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
/// The JSON arm still goes through `output_result`, decoding lossily: JSON
/// strings must be valid UTF-8, so there is no byte-exact form to preserve, and
/// `file` has no JSON output to match against.
pub fn output_description_bytes(
    writer: &mut impl Write,
    file_path: &Path,
    description: &[u8],
    args: &crate::Args,
    is_multiple_files: bool,
) -> Result<(), LibmagicError> {
    match args.output_format() {
        crate::OutputFormat::Text => {
            write!(writer, "{}: ", file_path.display()).map_err(LibmagicError::IoError)?;
            writer
                .write_all(description)
                .map_err(LibmagicError::IoError)?;
            writeln!(writer).map_err(LibmagicError::IoError)?;
            Ok(())
        }
        crate::OutputFormat::Json => {
            let result = synthetic_result(&String::from_utf8_lossy(description));
            crate::output_result(writer, file_path, &result, args, is_multiple_files)
        }
    }
}
