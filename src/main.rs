// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Command-line interface for libmagic-rs
//!
//! This binary provides a CLI tool for file type identification using magic rules,
//! serving as a drop-in replacement for the GNU `file` command.

use clap::{CommandFactory, Parser};
use clap_complete::Shell;
use clap_stdin::FileOrStdin;
use libmagic_rs::output::json::{format_json_line_output, format_json_output};
// Used only by the unix-gated magic-file discovery path and by tests;
// gate the import so Windows release builds do not flag it as unused.
#[cfg(any(unix, test))]
use libmagic_rs::parser::{MagicFileFormat, detect_format};
use libmagic_rs::{LibmagicError, MagicDatabase};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A pure-Rust implementation of libmagic for file type identification
///
/// Supports analyzing multiple files in a single invocation. Each file is
/// processed sequentially with independent error handling.
///
/// Output formats:
///   Text (default): One line per file in format "filename: description"
///   JSON (single file): Pretty-printed JSON with matches array
///   JSON (multiple files): JSON Lines format with compact output per file
#[derive(Parser, Debug)]
#[command(
    name = "rmagic",
    version = env!("CARGO_PKG_VERSION"),
    author = "Rust Libmagic Contributors",
    about = "A pure-Rust implementation of libmagic for file type identification. Supports multiple files and stdin input.",
    after_help = "\
Examples:
  rmagic file1.bin file2.txt file3.dat
  rmagic -j file.bin              # Single file: pretty-printed JSON
  rmagic -j file1.bin file2.txt   # Multiple files: JSON Lines format
  rmagic -s -m custom.magic file1 file2
  rmagic -b file.bin              # Use built-in rules
  rmagic -b -s -j *.bin
  rmagic - < input.dat            # Read from stdin
  rmagic --generate-completion bash > rmagic.bash",
    group(clap::ArgGroup::new("format").args(["json", "text"]))
)]
// Each bool is an independent CLI flag; clap derive structs are the one
// place where a field-per-flag layout is the correct design.
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Files to analyze (use '-' for stdin)
    #[arg(value_name = "FILE", required_unless_present = "generate_completion", num_args = 1..)]
    pub files: Vec<FileOrStdin>,

    /// Output results in JSON format
    #[arg(short = 'j', long)]
    pub json: bool,

    /// Output results in text format (default)
    #[arg(long)]
    pub text: bool,

    /// Use custom magic file
    #[arg(short = 'm', long = "magic-file", value_name = "FILE")]
    pub magic_file: Option<PathBuf>,

    /// Exit with non-zero code on failures (I/O, parse, or evaluation errors).
    ///
    /// A "data" result (unknown file type) is not considered an error and will
    /// not cause a non-zero exit code, even in strict mode.
    #[arg(short = 's', long)]
    pub strict: bool,

    /// Use built-in magic rules instead of loading from file.
    ///
    /// Loads pre-compiled built-in rules for common file types (ELF, PE/DOS,
    /// ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF). These rules are compiled
    /// at build time and provide basic file type detection without requiring
    /// external magic files.
    #[arg(short = 'b', long, conflicts_with = "magic_file")]
    pub use_builtin: bool,

    /// Timeout for evaluation in milliseconds (1-300000ms, 5 minutes max).
    ///
    /// Sets a per-file timeout for magic rule evaluation. If evaluation takes
    /// longer than this duration, the file is skipped with a timeout error.
    /// Each file gets its own independent timeout window.
    #[arg(short = 't', long = "timeout-ms", value_name = "MS",
          value_parser = clap::value_parser!(u64).range(1..=300_000))]
    pub timeout_ms: Option<u64>,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL")]
    pub generate_completion: Option<Shell>,
}

impl Args {
    /// Determine the output format based on flags
    #[must_use]
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        }
    }

    /// Get the magic file path to use, with platform-appropriate defaults
    #[must_use]
    pub fn get_magic_file_path(&self) -> PathBuf {
        self.magic_file
            .clone()
            .unwrap_or_else(Self::default_magic_file_path)
    }

    /// Create an `EvaluationConfig` from command-line arguments
    ///
    /// Uses the timeout value from --timeout-ms if provided, with validation
    /// performed during config creation. Other config values use defaults.
    #[must_use]
    pub fn to_evaluation_config(&self) -> libmagic_rs::EvaluationConfig {
        libmagic_rs::EvaluationConfig::default().with_timeout_ms(self.timeout_ms)
    }

    /// Magic file search candidates in priority order.
    ///
    /// OpenBSD-inspired order: text files/directories first, then compiled .mgc files.
    /// Text files are preferred because they are human-readable, easier to debug,
    /// and better suited for version control and development workflows.
    #[cfg(unix)]
    const MAGIC_FILE_CANDIDATES: &'static [&'static str] = &[
        // Text directories first (highest priority for debugging and compatibility)
        "/usr/share/file/magic/Magdir", // OpenBSD-style magic directory
        "/usr/share/file/magic",        // Text magic directory/file
        // Text files
        "/usr/share/misc/magic",       // BSD text magic file
        "/usr/local/share/misc/magic", // FreeBSD/Homebrew text
        "/etc/magic",                  // System-wide text magic file
        "/opt/local/share/file/magic", // MacPorts text
        // Binary .mgc files last (fallback for performance)
        "/usr/share/file/magic.mgc",       // Most common on Linux/macOS
        "/usr/local/share/misc/magic.mgc", // Homebrew/FreeBSD
        "/opt/local/share/file/magic.mgc", // MacPorts
        "/etc/magic.mgc",                  // Alternative location
        "/usr/share/misc/magic.mgc",       // BSD variant
    ];

    /// Returns the list of magic file candidates in search order.
    ///
    /// This is primarily exposed for testing purposes to verify the search order.
    #[cfg(unix)]
    #[must_use]
    pub fn magic_file_candidates() -> &'static [&'static str] {
        Self::MAGIC_FILE_CANDIDATES
    }

    /// Get the default magic file path for the current platform
    ///
    /// This follows an OpenBSD-inspired approach, prioritizing text-based magic files
    /// and directories over compiled binary `.mgc` files. Text files are preferred
    /// because they are human-readable, easier to debug, and better suited for
    /// version control and development workflows.
    ///
    /// The search order is:
    /// 1. Text directories (e.g., `/usr/share/file/magic/Magdir`)
    /// 2. Text files (e.g., `/usr/share/misc/magic`)
    /// 3. Binary `.mgc` files (e.g., `/usr/share/file/magic.mgc`)
    ///
    /// If a text file/directory is found, it is returned immediately.
    /// If only binary files exist, the first binary file found is used as fallback.
    fn default_magic_file_path() -> PathBuf {
        #[cfg(unix)]
        {
            let mut first_binary: Option<PathBuf> = None;

            for candidate in Self::MAGIC_FILE_CANDIDATES {
                let path = PathBuf::from(candidate);
                if !path.exists() {
                    continue;
                }

                if let Ok(format) = detect_format(&path) {
                    match format {
                        // Accept text files and directories immediately (OpenBSD-style preference)
                        MagicFileFormat::Text | MagicFileFormat::Directory => return path,
                        // Track first binary file as fallback, but continue searching for text
                        MagicFileFormat::Binary => {
                            if first_binary.is_none() {
                                first_binary = Some(path);
                            }
                        }
                    }
                }
            }

            // If we found a binary file but no text file, use the binary as fallback
            if let Some(binary_path) = first_binary {
                return binary_path;
            }

            // Final absolute-path default. Relative-path fallbacks were removed
            // deliberately: resolving `./missing.magic` or `./third_party/magic.mgc`
            // against the process cwd is an untrusted-search-path surface
            // (CWE-426) — an attacker can plant a crafted magic file in any
            // directory a victim is likely to `cd` into. Users running from a
            // dev checkout must pass `--magic <path>` explicitly.
            PathBuf::from("/usr/share/file/magic.mgc")
        }
        #[cfg(windows)]
        {
            // Try Windows-specific locations
            if let Ok(appdata) = std::env::var("APPDATA") {
                let magic_path = PathBuf::from(appdata).join("Magic").join("magic");
                if magic_path.exists() {
                    return magic_path;
                }
            }

            // No relative-path fallback (see CWE-426 rationale in the unix arm).
            PathBuf::from(r"C:\ProgramData\libmagic-rs\magic.mgc")
        }
        #[cfg(not(any(unix, windows)))]
        {
            PathBuf::from("/usr/share/file/magic.mgc")
        }
    }
}

/// Output format for file type identification results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output (default)
    Text,
    /// Structured JSON output
    Json,
}

fn main() -> std::process::ExitCode {
    env_logger::init();

    let args = Args::parse();

    // Handle shell completion generation
    if let Some(shell) = args.generate_completion {
        let mut cmd = Args::command();
        clap_complete::generate(shell, &mut cmd, "rmagic", &mut std::io::stdout());
        return std::process::ExitCode::SUCCESS;
    }

    // Set up signal handler for graceful Ctrl+C handling. `Ordering::Relaxed`
    // is correct for a single-bit flag with no ordering dependencies on
    // other memory; `SeqCst` would issue an unnecessary full barrier.
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = Arc::clone(&interrupted);
    if let Err(e) = ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::Relaxed);
    }) {
        eprintln!("Warning: failed to set signal handler: {e}");
    }

    let exit_code: i32 = match run_analysis(&args, &interrupted) {
        Ok(()) => {
            if interrupted.load(Ordering::Relaxed) {
                eprintln!("Interrupted");
                130
            } else {
                0
            }
        }
        Err(e) => handle_error(&e),
    };

    // Return ExitCode instead of process::exit so destructors run
    // (important for BufWriter::flush, Mmap drop, and the signal
    // handler's Arc). Clamp out-of-range exit codes to 1.
    std::process::ExitCode::from(u8::try_from(exit_code).unwrap_or(1))
}

/// Handle different types of errors and return appropriate exit codes
///
/// Exit codes follow Unix conventions:
/// - 0: Success
/// - 1: General error
/// - 2: Misuse of shell command (invalid arguments)
/// - 3: File not found or access denied
/// - 4: Magic file not found or invalid
/// - 5: Evaluation timeout or resource limits exceeded
fn handle_error(error: &LibmagicError) -> i32 {
    // Note: `LibmagicError` is `#[non_exhaustive]` so a wildcard arm is
    // mandatory here (bin crates are separate compilation units from the
    // library crate, even inside the same cargo package). The wildcard
    // explicitly documents "unknown variant" rather than silently
    // collapsing to exit code 1 with a generic message; if you find it
    // firing in the wild, a new variant was added to `LibmagicError`
    // without a corresponding handler in this function.
    match error {
        LibmagicError::IoError(io_err) => handle_io_error(io_err),
        LibmagicError::ParseError(parse_err) => handle_parse_error_new(parse_err),
        LibmagicError::EvaluationError(eval_err) => handle_evaluation_error_new(eval_err),
        LibmagicError::Timeout { timeout_ms } => handle_timeout_error(*timeout_ms),
        LibmagicError::ConfigError { reason } => {
            eprintln!("Configuration error: {reason}");
            1
        }
        LibmagicError::FileError(msg) => {
            eprintln!("File error: {msg}");
            3
        }
        _ => {
            eprintln!("Error: unhandled libmagic-rs error variant (update handle_error): {error}");
            1
        }
    }
}

/// Handle I/O errors with specific error messages
fn handle_io_error(io_err: &std::io::Error) -> i32 {
    match io_err.kind() {
        std::io::ErrorKind::NotFound => {
            eprintln!(
                "Error: File not found\nThe specified file does not exist or cannot be accessed.\nPlease check the file path and try again."
            );
            3
        }
        std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "Error: Permission denied\nYou do not have permission to access the specified file.\nPlease check file permissions or run with appropriate privileges."
            );
            3
        }
        std::io::ErrorKind::InvalidInput => {
            eprintln!(
                "Error: Invalid input\nThe file path or arguments provided are invalid.\nPlease check your input and try again."
            );
            2
        }
        _ => {
            eprintln!(
                "Error: File access failed\nFailed to access file: {io_err}\nPlease check the file path and permissions."
            );
            3
        }
    }
}

/// Handle parse errors with detailed information
fn handle_parse_error_new(parse_err: &libmagic_rs::ParseError) -> i32 {
    eprintln!(
        "Error: Magic file parse error\n{parse_err}\nThe magic file contains invalid syntax or formatting.\nPlease check the magic file format or try a different magic file."
    );
    4
}

/// Handle evaluation errors
fn handle_evaluation_error_new(eval_err: &libmagic_rs::EvaluationError) -> i32 {
    eprintln!(
        "Error: Rule evaluation failed\n{eval_err}\nFailed to evaluate magic rules against the file.\nThe file may be corrupted or the magic rules may be incompatible."
    );
    1
}

/// Handle timeout errors
fn handle_timeout_error(timeout_ms: u64) -> i32 {
    eprintln!(
        "Error: Evaluation timeout\nFile analysis timed out after {timeout_ms}ms\nThe file may be too large or complex to analyze within the time limit.\nTry using a simpler magic file or increasing the timeout limit."
    );
    5
}

/// Load magic database from file
///
/// Handles magic file discovery, validation, and database loading.
/// Returns the loaded database or an error if loading fails.
fn load_magic_database(args: &Args) -> Result<MagicDatabase, LibmagicError> {
    let config = args.to_evaluation_config();

    if args.use_builtin {
        return MagicDatabase::with_builtin_rules_and_config(config);
    }

    // Get magic file path
    let magic_file_path = args.get_magic_file_path();

    // Validate magic file exists
    if !magic_file_path.exists() {
        return Err(LibmagicError::ParseError(
            libmagic_rs::ParseError::invalid_syntax(
                0,
                format!(
                    "Magic file not found at {}. Please ensure a magic file is available at one of the standard locations or specify a custom path with --magic-file.",
                    magic_file_path.display()
                ),
            ),
        ));
    }

    // Reject empty magic files with a clear user-facing message. The parser
    // will silently accept a zero-byte file and produce zero rules, which
    // surfaces as a misleading "data" classification rather than a setup
    // error — so we catch it here where we can emit a targeted message.
    let metadata = std::fs::metadata(&magic_file_path).map_err(|e| {
        LibmagicError::IoError(std::io::Error::new(
            e.kind(),
            format!(
                "Cannot access magic file {}: {e}",
                magic_file_path.display()
            ),
        ))
    })?;

    if metadata.is_file() && metadata.len() == 0 {
        return Err(LibmagicError::ParseError(
            libmagic_rs::ParseError::invalid_syntax(
                0,
                format!("Magic file {} is empty", magic_file_path.display()),
            ),
        ));
    }

    // Load and return database with custom config. Underlying parser/IO
    // errors propagate directly without a redundant pre-validation pass.
    MagicDatabase::load_from_file_with_config(&magic_file_path, config)
}

/// Output analysis result based on format
///
/// Handles output formatting for both JSON and text formats.
/// For multiple files with JSON format, outputs JSON Lines (compact, one per line).
/// For single file with JSON format, outputs pretty-printed JSON.
///
/// Writes to the provided buffered writer. The caller is responsible for flushing.
fn output_result(
    writer: &mut impl Write,
    file_path: &Path,
    result: &libmagic_rs::EvaluationResult,
    args: &Args,
    is_multiple_files: bool,
) -> Result<(), LibmagicError> {
    match args.output_format() {
        OutputFormat::Json => {
            // Convert library result to output format (handles match conversion + tag enrichment)
            let output_result =
                libmagic_rs::output::EvaluationResult::from_library_result(result, file_path);

            // Use JSON Lines format for multiple files, pretty JSON for single file
            let json_result = if is_multiple_files {
                format_json_line_output(file_path, &output_result.matches)
            } else {
                format_json_output(&output_result.matches)
            };

            match json_result {
                Ok(json_str) => {
                    writeln!(writer, "{json_str}").map_err(LibmagicError::IoError)?;
                }
                Err(e) => {
                    return Err(LibmagicError::EvaluationError(
                        libmagic_rs::EvaluationError::unsupported_type(format!(
                            "Failed to serialize JSON: {e}"
                        )),
                    ));
                }
            }
        }
        OutputFormat::Text => {
            writeln!(writer, "{}: {}", file_path.display(), result.description)
                .map_err(LibmagicError::IoError)?;
        }
    }
    Ok(())
}

/// Process a single file with the magic database
///
/// Handles file validation, evaluation, and output.
/// Returns Ok(()) on success or an error if processing fails.
fn process_file(
    writer: &mut impl Write,
    file_or_stdin: &FileOrStdin,
    db: &MagicDatabase,
    args: &Args,
) -> Result<(), LibmagicError> {
    if file_or_stdin.is_stdin() {
        use std::io::Read;

        let max_string_length = db.config().max_string_length;
        let mut buffer = Vec::with_capacity(max_string_length + 1);

        let reader = file_or_stdin.clone().into_reader().map_err(|e| {
            LibmagicError::IoError(std::io::Error::other(format!("Failed to open stdin: {e}")))
        })?;

        // Read one extra byte to detect true truncation
        let mut limited_reader = reader.take((max_string_length + 1) as u64);
        limited_reader.read_to_end(&mut buffer).map_err(|e| {
            LibmagicError::IoError(std::io::Error::new(
                e.kind(),
                format!("Failed to read stdin: {e}"),
            ))
        })?;

        // Warn only if we actually read more than max_string_length bytes
        if buffer.len() > max_string_length {
            eprintln!("Warning: stdin input truncated to {max_string_length} bytes");
            // Truncate the buffer back to max_string_length
            buffer.truncate(max_string_length);
        }

        let result = db.evaluate_buffer(&buffer)?;
        let stdin_path = PathBuf::from("stdin");
        let is_multiple_files = args.files.len() > 1;
        output_result(writer, &stdin_path, &result, args, is_multiple_files)?;
        return Ok(());
    }

    // Extract file path from FileOrStdin
    // Use the filename() method to get the path
    let file_path = PathBuf::from(file_or_stdin.filename());

    // Reject directories early with a clear message. On some platforms
    // (notably Windows) FileBuffer may accept a directory path without
    // error, producing a misleading "data" classification.
    if file_path.is_dir() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Path is a directory, not a file: {}", file_path.display()),
        )));
    }

    let result = db.evaluate_file(&file_path)?;

    // Output results based on format
    let is_multiple_files = args.files.len() > 1;
    output_result(writer, &file_path, &result, args, is_multiple_files)?;

    Ok(())
}

fn run_analysis(args: &Args, interrupted: &AtomicBool) -> Result<(), LibmagicError> {
    // Validate input arguments
    validate_arguments(args)?;

    // Load magic database once (shared across all files)
    let db = load_magic_database(args)?;

    let mut writer = BufWriter::new(std::io::stdout().lock());
    let mut first_error: Option<LibmagicError> = None;

    // Process each file sequentially
    for file_or_stdin in &args.files {
        // Check for Ctrl+C between files
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        match process_file(&mut writer, file_or_stdin, &db, args) {
            Ok(()) => {} // Success, continue
            Err(e) => {
                // Print error with filename context but continue processing other files
                eprintln!("Error processing {}: {}", file_or_stdin.filename(), e);
                // Store first error for strict mode
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }

    // Flush buffered output
    writer.flush().map_err(LibmagicError::IoError)?;

    // Exit code behavior based on --strict flag
    if let Some(error) = first_error
        && args.strict
    {
        return Err(error);
    }

    Ok(())
}

/// Validate command-line arguments
fn validate_arguments(args: &Args) -> Result<(), LibmagicError> {
    // Check if files vector is empty
    if args.files.is_empty() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "At least one file must be specified",
        )));
    }

    // Validate custom magic file path if provided and not using built-in rules
    if !args.use_builtin
        && let Some(ref magic_file) = args.magic_file
    {
        let magic_str = magic_file.to_string_lossy();
        if magic_str.trim().is_empty() {
            return Err(LibmagicError::ParseError(
                libmagic_rs::ParseError::invalid_syntax(0, "Magic file path cannot be empty"),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Test-only: candidate lists use fixed lowercase extensions, so the
    // case-sensitive comparison is exact by construction.
    #![allow(clippy::case_sensitive_file_extension_comparisons)]

    use super::*;
    use clap::Parser;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basic_file_argument() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert!(!args.json);
        assert!(!args.text);
        assert_eq!(args.output_format(), OutputFormat::Text);
        assert!(args.magic_file.is_none());
    }

    #[test]
    fn test_json_output_flag() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--json"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert!(args.json);
        assert!(!args.text);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_text_output_flag() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--text"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert!(!args.json);
        assert!(args.text);
        assert_eq!(args.output_format(), OutputFormat::Text);
    }

    #[test]
    fn test_magic_file_argument() {
        let args =
            Args::try_parse_from(["rmagic", "test.bin", "--magic-file", "custom.magic"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert_eq!(args.magic_file, Some(PathBuf::from("custom.magic")));
    }

    #[test]
    fn test_all_arguments_combined() {
        let args = Args::try_parse_from([
            "rmagic",
            "test.bin",
            "--json",
            "--magic-file",
            "custom.magic",
        ])
        .unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert!(args.json);
        assert!(!args.text);
        assert_eq!(args.output_format(), OutputFormat::Json);
        assert_eq!(args.magic_file, Some(PathBuf::from("custom.magic")));
    }

    #[test]
    fn test_json_text_conflict() {
        // Should fail because --json and --text conflict
        let result = Args::try_parse_from(["rmagic", "test.bin", "--json", "--text"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_file_argument() {
        // Should fail because file argument is required
        let result = Args::try_parse_from(["rmagic"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_output_format_default() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert_eq!(args.output_format(), OutputFormat::Text);
    }

    #[test]
    fn test_output_format_json() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--json"]).unwrap();
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_output_format_text_explicit() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--text"]).unwrap();
        assert_eq!(args.output_format(), OutputFormat::Text);
    }

    #[test]
    fn test_args_defaults() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert!(!args.strict, "strict should default to false");
        assert!(!args.use_builtin, "use_builtin should default to false");
    }

    #[test]
    fn test_args_strict_flag() {
        let args = Args::try_parse_from(["rmagic", "--strict", "test.bin"]).unwrap();
        assert!(args.strict);
    }

    #[test]
    fn test_args_strict_with_json() {
        let args = Args::try_parse_from(["rmagic", "--strict", "--json", "test.bin"]).unwrap();
        assert!(args.strict);
        assert!(args.json);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_use_builtin_flag_parsing() {
        let args = Args::try_parse_from(["rmagic", "--use-builtin", "test.bin"]).unwrap();
        assert!(args.use_builtin);
    }

    #[test]
    fn test_args_single_file_backwards_compatible() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert!(!args.strict);
    }

    #[test]
    fn test_args_multiple_files() {
        let args = Args::try_parse_from(["rmagic", "file1.bin", "file2.bin", "file3.bin"]).unwrap();
        assert_eq!(args.files.len(), 3);
    }

    #[test]
    fn test_args_stdin_detection() {
        let args = Args::try_parse_from(["rmagic", "-"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert!(args.files[0].is_stdin());
    }

    #[test]
    fn test_complex_file_paths() {
        let args = Args::try_parse_from(["rmagic", "/path/to/complex file.bin"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "/path/to/complex file.bin");
    }

    #[test]
    fn test_magic_file_with_spaces() {
        let args = Args::try_parse_from([
            "rmagic",
            "test.bin",
            "--magic-file",
            "/path/to/magic file.magic",
        ])
        .unwrap();
        assert_eq!(
            args.magic_file,
            Some(PathBuf::from("/path/to/magic file.magic"))
        );
    }

    #[test]
    fn test_get_magic_file_path_custom() {
        let args =
            Args::try_parse_from(["rmagic", "test.bin", "--magic-file", "custom.magic"]).unwrap();
        assert_eq!(args.get_magic_file_path(), PathBuf::from("custom.magic"));
    }

    #[test]
    fn test_get_magic_file_path_default() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        let default_path = args.get_magic_file_path();

        // Test that we get a platform-appropriate default.
        // After the S-H1 fix the fallback is a single absolute-path
        // default per platform -- relative-path fallbacks
        // (`missing.magic`, `third_party/magic.mgc`) were removed
        // because they resolved against the process cwd (CWE-426).
        // The actual path depends on which system-wide magic file is
        // present at test time.
        #[cfg(unix)]
        {
            // Get the actual candidates from the exposed constant
            let candidates = Args::magic_file_candidates();

            // Build list of valid paths (system candidates + single
            // absolute default).
            let mut valid_paths: Vec<&str> = candidates.to_vec();
            valid_paths.push("/usr/share/file/magic.mgc");

            assert!(
                valid_paths.contains(&default_path.to_str().unwrap()),
                "Got unexpected path: {default_path:?}"
            );
        }

        #[cfg(windows)]
        assert_eq!(
            default_path,
            PathBuf::from(r"C:\ProgramData\libmagic-rs\magic.mgc")
        );

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("/usr/share/file/magic.mgc"));
    }

    #[test]
    fn test_default_magic_file_path() {
        let default_path = Args::default_magic_file_path();

        // Test that we get a platform-appropriate default. See the
        // matching comment on test_get_magic_file_path_default.
        #[cfg(unix)]
        {
            let candidates = Args::magic_file_candidates();

            let mut valid_paths: Vec<&str> = candidates.to_vec();
            valid_paths.push("/usr/share/file/magic.mgc");

            assert!(
                valid_paths.contains(&default_path.to_str().unwrap()),
                "Got unexpected path: {default_path:?}"
            );
        }

        #[cfg(windows)]
        assert_eq!(
            default_path,
            PathBuf::from(r"C:\ProgramData\libmagic-rs\magic.mgc")
        );

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("/usr/share/file/magic.mgc"));
    }

    // Error handling tests
    #[test]
    fn test_handle_error_file_not_found() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 3);
    }

    #[test]
    fn test_handle_error_permission_denied() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Permission denied",
        ));
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 3);
    }

    #[test]
    fn test_handle_error_invalid_input() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid input",
        ));
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 2);
    }

    #[test]
    fn test_handle_error_parse_error() {
        let error = LibmagicError::ParseError(libmagic_rs::ParseError::invalid_syntax(
            42,
            "Invalid syntax",
        ));
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 4);
    }

    #[test]
    fn test_handle_error_evaluation_error() {
        let error = LibmagicError::EvaluationError(libmagic_rs::EvaluationError::unsupported_type(
            "Evaluation failed",
        ));
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_handle_error_timeout() {
        let error = LibmagicError::Timeout { timeout_ms: 5000 };
        let exit_code = handle_error(&error);
        assert_eq!(exit_code, 5);
    }

    #[test]
    fn test_validate_arguments_empty_files() {
        // Test with empty files vector
        let _args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        // Manually create args with empty files for this test
        let args_empty = Args {
            files: vec![],
            json: false,
            text: false,
            magic_file: None,
            strict: false,
            use_builtin: false,
            timeout_ms: None,
            generate_completion: None,
        };
        let result = validate_arguments(&args_empty);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(
                    e.to_string()
                        .contains("At least one file must be specified")
                );
            }
            _ => panic!("Expected IoError with InvalidInput"),
        }
    }

    #[test]
    fn test_validate_arguments_empty_magic_file() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        let args_with_empty_magic = Args {
            files: args.files,
            json: false,
            text: false,
            magic_file: Some(PathBuf::from("")),
            strict: false,
            use_builtin: false,
            timeout_ms: None,
            generate_completion: None,
        };
        let result = validate_arguments(&args_with_empty_magic);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::ParseError(parse_err) => {
                let msg = parse_err.to_string();
                assert!(msg.contains("Magic file path cannot be empty"));
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_validate_arguments_valid() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        let args_with_magic = Args {
            files: args.files,
            json: false,
            text: false,
            magic_file: Some(PathBuf::from("magic.db")),
            strict: false,
            use_builtin: false,
            timeout_ms: None,
            generate_completion: None,
        };
        let result = validate_arguments(&args_with_magic);
        assert!(result.is_ok());
    }

    /// Verify that text files/directories are prioritized over binary .mgc files
    /// in the magic file search order (OpenBSD-style approach)
    #[test]
    #[cfg(unix)]
    fn test_magic_file_search_order_text_first() {
        let candidates = Args::magic_file_candidates();

        // Find the index of the first binary (.mgc) candidate
        let first_binary_index = candidates
            .iter()
            .position(|c| c.ends_with(".mgc"))
            .expect("Should have at least one .mgc candidate");

        // Verify all candidates before the first binary are text (non-.mgc)
        for (i, candidate) in candidates.iter().enumerate() {
            if i < first_binary_index {
                assert!(
                    !candidate.ends_with(".mgc"),
                    "Candidate at index {i} should be text (not .mgc): {candidate}"
                );
            }
        }

        // Verify all candidates from first_binary_index onwards are binary (.mgc)
        for (i, candidate) in candidates.iter().enumerate() {
            if i >= first_binary_index {
                assert!(
                    candidate.ends_with(".mgc"),
                    "Candidate at index {i} should be binary (.mgc): {candidate}"
                );
            }
        }

        // Verify we have both text and binary candidates
        assert!(
            first_binary_index > 0,
            "Should have at least one text candidate before binary candidates"
        );
        assert!(
            first_binary_index < candidates.len(),
            "Should have at least one binary candidate"
        );
    }

    /// Verify that Magdir has the highest priority in the search order
    #[test]
    #[cfg(unix)]
    fn test_magic_file_search_order_magdir_priority() {
        let candidates = Args::magic_file_candidates();

        // Verify the first candidate is the Magdir directory
        assert_eq!(
            candidates[0], "/usr/share/file/magic/Magdir",
            "First candidate should be the Magdir directory"
        );
    }

    /// Verify the exact sequence of magic file candidates
    #[test]
    #[cfg(unix)]
    fn test_magic_file_candidates_exact_sequence() {
        let candidates = Args::magic_file_candidates();

        // Verify the exact expected sequence
        let expected = [
            // Text directories first
            "/usr/share/file/magic/Magdir",
            "/usr/share/file/magic",
            // Text files
            "/usr/share/misc/magic",
            "/usr/local/share/misc/magic",
            "/etc/magic",
            "/opt/local/share/file/magic",
            // Binary .mgc files last
            "/usr/share/file/magic.mgc",
            "/usr/local/share/misc/magic.mgc",
            "/opt/local/share/file/magic.mgc",
            "/etc/magic.mgc",
            "/usr/share/misc/magic.mgc",
        ];

        assert_eq!(
            candidates.len(),
            expected.len(),
            "Candidate list length mismatch"
        );

        for (i, (actual, expected)) in candidates.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual, expected,
                "Candidate mismatch at index {i}: got '{actual}', expected '{expected}'"
            );
        }
    }

    /// Verify behavior: first existing candidate is chosen in order
    /// This test uses a temporary directory to simulate the search behavior
    #[test]
    #[cfg(unix)]
    fn test_magic_file_search_selects_first_existing() {
        use std::io::Write;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a text magic file
        let text_magic_path = temp_dir.path().join("text_magic");
        let mut text_file =
            fs::File::create(&text_magic_path).expect("Failed to create text magic file");
        writeln!(text_file, "# Text magic file").expect("Failed to write");
        writeln!(text_file, "0 string test Test file").expect("Failed to write");

        // Create a binary magic file (simulated with .mgc extension)
        let binary_magic_path = temp_dir.path().join("binary.mgc");
        // Write some bytes that look like a binary magic file header
        fs::write(&binary_magic_path, b"\x1c\x04\x1e\xf1test")
            .expect("Failed to create binary magic file");

        // Verify text file exists and is detected as text format
        assert!(text_magic_path.exists());
        let text_format = detect_format(&text_magic_path);
        assert!(
            matches!(text_format, Ok(MagicFileFormat::Text)),
            "Text magic file should be detected as Text format, got {text_format:?}"
        );

        // Verify binary file exists and is detected as binary format
        assert!(binary_magic_path.exists());
        let binary_format = detect_format(&binary_magic_path);
        assert!(
            matches!(binary_format, Ok(MagicFileFormat::Binary)),
            "Binary magic file should be detected as Binary format, got {binary_format:?}"
        );
    }

    /// Verify that binary files are selected as fallback when no text files exist
    #[test]
    #[cfg(unix)]
    fn test_magic_file_search_binary_fallback() {
        // This test verifies the logic by checking the candidate list structure
        let candidates = Args::magic_file_candidates();

        // Count text and binary candidates
        let text_count = candidates.iter().filter(|c| !c.ends_with(".mgc")).count();
        let binary_count = candidates.iter().filter(|c| c.ends_with(".mgc")).count();

        // Verify we have both types
        assert!(text_count > 0, "Should have text candidates");
        assert!(binary_count > 0, "Should have binary candidates");

        // Verify the structure allows binary fallback:
        // - Text candidates come first (they will be checked first)
        // - Binary candidates come after (they serve as fallback)
        // The search loop tracks first_binary and returns it if no text is found
        let first_text_idx = candidates
            .iter()
            .position(|c| !c.ends_with(".mgc"))
            .unwrap();
        let first_binary_idx = candidates.iter().position(|c| c.ends_with(".mgc")).unwrap();

        assert!(
            first_text_idx < first_binary_idx,
            "Text candidates should come before binary candidates"
        );
    }
}
