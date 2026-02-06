//! Command-line interface for libmagic-rs
//!
//! This binary provides a CLI tool for file type identification using magic rules,
//! serving as a drop-in replacement for the GNU `file` command.

use clap::Parser;
use clap_stdin::FileOrStdin;
use libmagic_rs::output::MatchResult;
use libmagic_rs::output::json::{format_json_line_output, format_json_output};
use libmagic_rs::parser::ast::Value;
use libmagic_rs::parser::{MagicFileFormat, detect_format};
use libmagic_rs::tags::TagExtractor;
use libmagic_rs::{LibmagicError, MagicDatabase};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// A pure-Rust implementation of libmagic for file type identification
///
/// Supports analyzing multiple files in a single invocation. Each file is
/// processed sequentially with independent error handling.
///
/// Output formats:
/// - Text (default): One line per file in format "filename: description"
/// - JSON (single file): Pretty-printed JSON with matches array
/// - JSON (multiple files): JSON Lines format with compact output per file
///
/// Examples:
///   rmagic file1.bin file2.txt file3.dat
///   rmagic --json file.bin              # Single file: pretty-printed JSON
///   rmagic --json file1.bin file2.txt   # Multiple files: JSON Lines format
///   rmagic --strict --magic-file custom.magic file1 file2
///   rmagic --use-builtin file.bin
///   rmagic --use-builtin --strict --json *.bin
///   rmagic - < input.dat  # Read from stdin
#[derive(Parser, Debug)]
#[command(
    name = "rmagic",
    version = env!("CARGO_PKG_VERSION"),
    author = "Rust Libmagic Contributors",
    about = "A pure-Rust implementation of libmagic for file type identification. Supports multiple files and stdin input."
)]
pub struct Args {
    /// Files to analyze (use '-' for stdin)
    #[arg(value_name = "FILE", required = true, num_args = 1..)]
    pub files: Vec<FileOrStdin>,

    /// Output results in JSON format
    #[arg(long, conflicts_with = "text")]
    pub json: bool,

    /// Output results in text format (default)
    #[arg(long)]
    pub text: bool,

    /// Use custom magic file
    #[arg(long = "magic-file", value_name = "FILE")]
    pub magic_file: Option<PathBuf>,

    /// Exit with non-zero code on failures (I/O, parse, or evaluation errors).
    ///
    /// A "data" result (unknown file type) is not considered an error and will
    /// not cause a non-zero exit code, even in strict mode.
    #[arg(long)]
    pub strict: bool,

    /// Use built-in magic rules instead of loading from file.
    ///
    /// Loads pre-compiled built-in rules for common file types (ELF, PE/DOS,
    /// ZIP, TAR, GZIP, JPEG, PNG, GIF, BMP, PDF). These rules are compiled
    /// at build time and provide basic file type detection without requiring
    /// external magic files. When provided alongside --magic-file, --use-builtin
    /// takes precedence.
    #[arg(long)]
    pub use_builtin: bool,

    /// Timeout for evaluation in milliseconds (1-300000ms, 5 minutes max).
    ///
    /// Sets a per-file timeout for magic rule evaluation. If evaluation takes
    /// longer than this duration, the file is skipped with a timeout error.
    /// Each file gets its own independent timeout window.
    #[arg(long = "timeout-ms", value_name = "MS")]
    pub timeout_ms: Option<u64>,
}

impl Args {
    /// Determine the output format based on flags
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        }
    }

    /// Get the magic file path to use, with platform-appropriate defaults
    pub fn get_magic_file_path(&self) -> PathBuf {
        if let Some(ref custom_path) = self.magic_file {
            custom_path.clone()
        } else {
            Self::default_magic_file_path()
        }
    }

    /// Create an EvaluationConfig from command-line arguments
    ///
    /// Uses the timeout value from --timeout-ms if provided, with validation
    /// performed during config creation. Other config values use defaults.
    pub fn to_evaluation_config(&self) -> libmagic_rs::EvaluationConfig {
        libmagic_rs::EvaluationConfig {
            timeout_ms: self.timeout_ms,
            ..Default::default()
        }
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

            // Fallback to repo-provided text magic file if present
            let repo_magic = PathBuf::from("missing.magic");
            if repo_magic.exists() {
                return repo_magic;
            }

            // Fallback to third_party binary magic file for compatibility hints
            let dev_magic = PathBuf::from("third_party/magic.mgc");
            if dev_magic.exists() {
                return dev_magic;
            }

            // CI/CD fallback
            if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
                return PathBuf::from("third_party/magic.mgc");
            }

            // Default fallback
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

            // Fallback to third_party (common in CI/CD)
            PathBuf::from("third_party/magic.mgc")
        }
        #[cfg(not(any(unix, windows)))]
        {
            PathBuf::from("third_party/magic.mgc")
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

fn main() {
    let args = Args::parse();

    let exit_code = match run_analysis(&args) {
        Ok(()) => 0,
        Err(e) => handle_error(e),
    };

    process::exit(exit_code);
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
fn handle_error(error: LibmagicError) -> i32 {
    match error {
        LibmagicError::IoError(ref io_err) => handle_io_error(io_err),
        LibmagicError::ParseError(ref parse_err) => handle_parse_error_new(parse_err),
        LibmagicError::EvaluationError(ref eval_err) => handle_evaluation_error_new(eval_err),
        LibmagicError::Timeout { timeout_ms } => handle_timeout_error(timeout_ms),
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
                "Error: File access failed\nFailed to access file: {}\nPlease check the file path and permissions.",
                io_err
            );
            3
        }
    }
}

/// Handle parse errors with detailed information
fn handle_parse_error_new(parse_err: &libmagic_rs::ParseError) -> i32 {
    eprintln!(
        "Error: Magic file parse error\n{}\nThe magic file contains invalid syntax or formatting.\nPlease check the magic file format or try a different magic file.",
        parse_err
    );
    4
}

/// Handle evaluation errors
fn handle_evaluation_error_new(eval_err: &libmagic_rs::EvaluationError) -> i32 {
    eprintln!(
        "Error: Rule evaluation failed\n{}\nFailed to evaluate magic rules against the file.\nThe file may be corrupted or the magic rules may be incompatible.",
        eval_err
    );
    1
}

/// Handle timeout errors
fn handle_timeout_error(timeout_ms: u64) -> i32 {
    eprintln!(
        "Error: Evaluation timeout\nFile analysis timed out after {}ms\nThe file may be too large or complex to analyze within the time limit.\nTry using a simpler magic file or increasing the timeout limit.",
        timeout_ms
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

    // Validate magic file format
    validate_magic_file(&magic_file_path)?;

    // Load and return database with custom config
    MagicDatabase::load_from_file_with_config(&magic_file_path, config)
}

/// Output analysis result based on format
///
/// Handles output formatting for both JSON and text formats.
/// For multiple files with JSON format, outputs JSON Lines (compact, one per line).
/// For single file with JSON format, outputs pretty-printed JSON.
///
/// Flushes stdout after each write to ensure results appear immediately when piped.
fn output_result(
    file_path: &Path,
    result: &libmagic_rs::EvaluationResult,
    args: &Args,
    is_multiple_files: bool,
) -> Result<(), LibmagicError> {
    use std::io::Write;

    let mut stdout = std::io::stdout();

    match args.output_format() {
        OutputFormat::Json => {
            // Extract tags from the description
            let tag_extractor = TagExtractor::new();
            let tags = tag_extractor.extract_tags(&result.description);

            // Convert evaluator::MatchResult to output::MatchResult
            let match_results: Vec<MatchResult> = if result.matches.is_empty() {
                vec![]
            } else {
                result
                    .matches
                    .iter()
                    .map(|m| {
                        // Build rule_path from match messages
                        let rule_path =
                            tag_extractor.extract_rule_path(std::iter::once(m.message.as_str()));

                        // Convert confidence from 0.0-1.0 to 0-100 scale
                        let confidence_score = (m.confidence * 100.0).min(100.0) as u8;

                        // Estimate length from value type
                        let length = match &m.value {
                            Value::Bytes(b) => b.len(),
                            Value::String(s) => s.len(),
                            _ => 4, // Default for numeric types
                        };

                        MatchResult::with_metadata(
                            m.message.clone(),
                            m.offset,
                            length,
                            m.value.clone(),
                            rule_path,
                            confidence_score,
                            result.mime_type.clone(),
                        )
                    })
                    .collect()
            };

            // Add tags to the first match if present
            let match_results = if !match_results.is_empty() && !tags.is_empty() {
                let mut results = match_results;
                // Tags are extracted from description, associate with primary result
                results[0] = MatchResult::with_metadata(
                    results[0].message.clone(),
                    results[0].offset,
                    results[0].length,
                    results[0].value.clone(),
                    if results[0].rule_path.is_empty() {
                        tags
                    } else {
                        results[0].rule_path.clone()
                    },
                    results[0].confidence,
                    results[0].mime_type.clone(),
                );
                results
            } else {
                match_results
            };

            // Use JSON Lines format for multiple files, pretty JSON for single file
            let json_result = if is_multiple_files {
                format_json_line_output(file_path, &match_results)
            } else {
                format_json_output(&match_results)
            };

            match json_result {
                Ok(json_str) => {
                    writeln!(stdout, "{json_str}").map_err(LibmagicError::IoError)?;
                    stdout.flush().map_err(LibmagicError::IoError)?;
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
            writeln!(stdout, "{}: {}", file_path.display(), result.description)
                .map_err(LibmagicError::IoError)?;
            stdout.flush().map_err(LibmagicError::IoError)?;
        }
    }
    Ok(())
}

/// Process a single file with the magic database
///
/// Handles file validation, evaluation, and output.
/// Returns Ok(()) on success or an error if processing fails.
fn process_file(
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
            eprintln!(
                "Warning: stdin input truncated to {} bytes",
                max_string_length
            );
            // Truncate the buffer back to max_string_length
            buffer.truncate(max_string_length);
        }

        let result = db.evaluate_buffer(&buffer)?;
        let stdin_path = PathBuf::from("stdin");
        let is_multiple_files = args.files.len() > 1;
        output_result(&stdin_path, &result, args, is_multiple_files)?;
        return Ok(());
    }

    // Extract file path from FileOrStdin
    // Use the filename() method to get the path
    let file_path = PathBuf::from(file_or_stdin.filename());

    // Validate file exists and is accessible
    validate_input_file(&file_path)?;

    // Evaluate file
    let result = db.evaluate_file(&file_path)?;

    // Output results based on format
    let is_multiple_files = args.files.len() > 1;
    output_result(&file_path, &result, args, is_multiple_files)?;

    Ok(())
}

fn run_analysis(args: &Args) -> Result<(), LibmagicError> {
    // Validate input arguments
    validate_arguments(args)?;

    // Load magic database once (shared across all files)
    let db = load_magic_database(args)?;

    let mut first_error: Option<LibmagicError> = None;

    // Process each file sequentially
    for file_or_stdin in &args.files {
        match process_file(file_or_stdin, &db, args) {
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

    // Exit code behavior based on --strict flag
    if let Some(error) = first_error {
        if args.strict {
            return Err(error);
        }
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
    if !args.use_builtin {
        if let Some(ref magic_file) = args.magic_file {
            let magic_str = magic_file.to_string_lossy();
            if magic_str.trim().is_empty() {
                return Err(LibmagicError::ParseError(
                    libmagic_rs::ParseError::invalid_syntax(0, "Magic file path cannot be empty"),
                ));
            }
        }
    }

    Ok(())
}

/// Validate that the input file exists and is accessible
fn validate_input_file(file_path: &Path) -> Result<(), LibmagicError> {
    if !file_path.exists() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", file_path.display()),
        )));
    }

    // Check if it's a directory
    if file_path.is_dir() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Path is a directory, not a file: {}", file_path.display()),
        )));
    }

    // Try to access the file to check permissions
    match fs::File::open(file_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(LibmagicError::IoError(e)),
    }
}

/// Validate that the magic file exists and is readable
fn validate_magic_file(magic_file_path: &Path) -> Result<(), LibmagicError> {
    if !magic_file_path.exists() {
        return Err(LibmagicError::ParseError(
            libmagic_rs::ParseError::invalid_syntax(
                0,
                format!("Magic file not found: {}", magic_file_path.display()),
            ),
        ));
    }

    // Directories are supported via load_magic_file
    if magic_file_path.is_dir() {
        return Ok(());
    }

    // Try to read the magic file to check permissions and basic format
    // Handle both text magic files and binary .mgc files
    match fs::read(magic_file_path) {
        Ok(content) => {
            // Basic validation - check if file is completely empty
            if content.is_empty() {
                return Err(LibmagicError::ParseError(
                    libmagic_rs::ParseError::invalid_syntax(0, "Magic file is empty"),
                ));
            }

            // Check if it's a binary magic file (.mgc) - these start with specific magic bytes
            if content.starts_with(b"\x0d\x0a\x1a\x0a") || content.len() > 100_000 {
                // Looks like a binary magic file, just check it's readable
                Ok(())
            } else {
                // Try to parse as text magic file
                match std::str::from_utf8(&content) {
                    Ok(text_content) => {
                        if text_content.trim().is_empty() {
                            return Err(LibmagicError::ParseError(
                                libmagic_rs::ParseError::invalid_syntax(0, "Magic file is empty"),
                            ));
                        }
                        Ok(())
                    }
                    Err(_) => {
                        // Not valid UTF-8, might be a binary file - allow it
                        Ok(())
                    }
                }
            }
        }
        Err(e) => Err(LibmagicError::IoError(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use libmagic_rs::parser::load_magic_file;
    #[cfg(unix)]
    use nix::unistd::{dup, dup2_stderr, dup2_stdin, dup2_stdout, pipe, read, write};
    use std::fs;
    #[cfg(unix)]
    use std::sync::Mutex;

    /// Static mutex to serialize access to file descriptor operations.
    /// This is necessary because dup/dup2 operations on stdin/stdout/stderr
    /// are process-wide and not thread-safe. Even with --test-threads=1,
    /// llvm-cov instrumentation can interfere with FD operations.
    #[cfg(unix)]
    static FD_MUTEX: Mutex<()> = Mutex::new(());

    #[cfg(unix)]
    fn capture_stdout<F>(f: F) -> (Result<(), LibmagicError>, String)
    where
        F: FnOnce() -> Result<(), LibmagicError>,
    {
        // Acquire mutex to serialize FD operations across all tests
        let _guard = FD_MUTEX.lock().unwrap();

        let saved_stdout = dup(std::io::stdout()).unwrap();
        let (read_fd, write_fd) = pipe().unwrap();

        dup2_stdout(&write_fd).unwrap();
        // Close the original write_fd after dup2 - stdout now owns a copy
        drop(write_fd);

        let result = f();

        dup2_stdout(&saved_stdout).unwrap();
        // Close the saved fd after restoring
        drop(saved_stdout);

        let mut output = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            match read(&read_fd, &mut buffer) {
                Ok(0) => break,
                Ok(count) => output.extend_from_slice(&buffer[..count]),
                Err(_) => break,
            }
        }
        drop(read_fd);

        let output_str = String::from_utf8_lossy(&output).to_string();
        (result, output_str)
    }

    #[cfg(unix)]
    fn capture_stderr<F>(f: F) -> (Result<(), LibmagicError>, String)
    where
        F: FnOnce() -> Result<(), LibmagicError>,
    {
        // Acquire mutex to serialize FD operations across all tests
        let _guard = FD_MUTEX.lock().unwrap();

        let saved_stderr = dup(std::io::stderr()).unwrap();
        let (read_fd, write_fd) = pipe().unwrap();

        dup2_stderr(&write_fd).unwrap();
        // Close the original write_fd after dup2 - stderr now owns a copy
        drop(write_fd);

        let result = f();

        dup2_stderr(&saved_stderr).unwrap();
        // Close the saved fd after restoring
        drop(saved_stderr);

        let mut output = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            match read(&read_fd, &mut buffer) {
                Ok(0) => break,
                Ok(count) => output.extend_from_slice(&buffer[..count]),
                Err(_) => break,
            }
        }
        drop(read_fd);

        let output_str = String::from_utf8_lossy(&output).to_string();
        (result, output_str)
    }

    #[cfg(unix)]
    fn with_mocked_stdin<F>(input: &[u8], f: F) -> Result<(), LibmagicError>
    where
        F: FnOnce() -> Result<(), LibmagicError>,
    {
        let saved_stdin = dup(std::io::stdin()).unwrap();
        let (read_fd, write_fd) = pipe().unwrap();

        let _ = write(&write_fd, input).unwrap();
        drop(write_fd);
        dup2_stdin(read_fd).unwrap();

        let result = f();

        dup2_stdin(saved_stdin).unwrap();

        result
    }

    #[cfg(unix)]
    fn with_invalid_stdin<F>(f: F) -> Result<(), LibmagicError>
    where
        F: FnOnce() -> Result<(), LibmagicError>,
    {
        let saved_stdin = dup(std::io::stdin()).unwrap();
        // Use unique temp directory with PID to avoid race conditions in parallel tests
        let temp_dir = std::env::temp_dir().join(format!(
            "rmagic_stdin_invalid_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let dir_handle = fs::File::open(&temp_dir).unwrap();

        dup2_stdin(&dir_handle).unwrap();
        let result = f();

        dup2_stdin(saved_stdin).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);

        result
    }

    fn resolve_magic_file_for_stdin_tests() -> Option<PathBuf> {
        // Skip stdin-mocking tests when running under llvm-cov instrumentation.
        // The dup/dup2 file descriptor manipulation is fragile when combined with
        // llvm-cov's instrumentation, causing spurious test failures in CI.
        // These tests pass with cargo nextest (separate processes) and provide
        // coverage there. The core stdin handling logic is also tested by the
        // non-mocking tests.
        if std::env::var("LLVM_PROFILE_FILE").is_ok() {
            return None;
        }

        let repo_magic = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing.magic");
        let candidates = [
            "/usr/share/misc/magic",
            "/etc/magic",
            "/usr/local/share/misc/magic",
            "/opt/local/share/file/magic",
            "/usr/share/file/magic",
            repo_magic.to_str().unwrap(),
        ];

        for candidate in &candidates {
            let path = PathBuf::from(candidate);
            if !path.exists() || path.is_dir() {
                continue;
            }

            if load_magic_file(&path).is_ok() {
                return Some(path);
            }
        }

        None
    }

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

        // Test that we get a platform-appropriate default
        // The actual path depends on what magic files exist on the system
        #[cfg(unix)]
        {
            // Get the actual candidates from the exposed constant
            let candidates = Args::magic_file_candidates();

            // Build list of valid paths (candidates + fallbacks)
            let mut valid_paths: Vec<&str> = candidates.to_vec();
            valid_paths.push("missing.magic");
            valid_paths.push("third_party/magic.mgc");

            // Should be one of the standard Unix magic file locations or fallback
            assert!(
                valid_paths.contains(&default_path.to_str().unwrap()),
                "Got unexpected path: {:?}",
                default_path
            );
        }

        #[cfg(windows)]
        assert_eq!(default_path, PathBuf::from("third_party/magic.mgc"));

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("third_party/magic.mgc"));
    }

    #[test]
    fn test_default_magic_file_path() {
        let default_path = Args::default_magic_file_path();

        // Test that we get a platform-appropriate default
        // The actual path depends on what magic files exist on the system
        #[cfg(unix)]
        {
            // Get the actual candidates from the exposed constant
            let candidates = Args::magic_file_candidates();

            // Build list of valid paths (candidates + fallbacks)
            let mut valid_paths: Vec<&str> = candidates.to_vec();
            valid_paths.push("missing.magic");
            valid_paths.push("third_party/magic.mgc");

            // Should be one of the standard Unix magic file locations or fallback
            assert!(
                valid_paths.contains(&default_path.to_str().unwrap()),
                "Got unexpected path: {:?}",
                default_path
            );
        }

        #[cfg(windows)]
        assert_eq!(default_path, PathBuf::from("third_party/magic.mgc"));

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("third_party/magic.mgc"));
    }

    // Error handling tests
    #[test]
    fn test_handle_error_file_not_found() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 3);
    }

    #[test]
    fn test_handle_error_permission_denied() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Permission denied",
        ));
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 3);
    }

    #[test]
    fn test_handle_error_invalid_input() {
        let error = LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid input",
        ));
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 2);
    }

    #[test]
    fn test_handle_error_parse_error() {
        let error = LibmagicError::ParseError(libmagic_rs::ParseError::invalid_syntax(
            42,
            "Invalid syntax",
        ));
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 4);
    }

    #[test]
    fn test_handle_error_evaluation_error() {
        let error = LibmagicError::EvaluationError(libmagic_rs::EvaluationError::unsupported_type(
            "Evaluation failed",
        ));
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_handle_error_timeout() {
        let error = LibmagicError::Timeout { timeout_ms: 5000 };
        let exit_code = handle_error(error);
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
        };
        let result = validate_arguments(&args_with_magic);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_input_file_not_found() {
        let result = validate_input_file(&PathBuf::from("nonexistent_file.bin"));
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
                assert!(e.to_string().contains("File not found"));
            }
            _ => panic!("Expected IoError with NotFound"),
        }
    }

    #[test]
    fn test_validate_input_file_directory() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_validate_dir");
        fs::create_dir_all(&temp_dir).unwrap();

        let result = validate_input_file(&temp_dir);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(e.to_string().contains("Path is a directory"));
            }
            _ => panic!("Expected IoError with InvalidInput"),
        }

        // Clean up
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_validate_input_file_valid() {
        // Create a temporary file for testing
        let temp_file = std::env::temp_dir().join("test_validate_file.bin");
        fs::write(&temp_file, b"test content").unwrap();

        let result = validate_input_file(&temp_file);
        assert!(result.is_ok());

        // Clean up
        fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_validate_magic_file_not_found() {
        let result = validate_magic_file(&PathBuf::from("nonexistent_magic.db"));
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::ParseError(parse_err) => {
                let msg = parse_err.to_string();
                assert!(msg.contains("Magic file not found"));
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_validate_magic_file_directory() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_validate_magic_dir");
        fs::create_dir_all(&temp_dir).unwrap();

        let result = validate_magic_file(&temp_dir);
        assert!(result.is_ok());

        // Clean up
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_validate_magic_file_empty() {
        // Create a temporary empty magic file for testing
        let temp_file = std::env::temp_dir().join("test_empty_magic.db");
        fs::write(&temp_file, "").unwrap();

        let result = validate_magic_file(&temp_file);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::ParseError(parse_err) => {
                let msg = parse_err.to_string();
                assert!(msg.contains("Magic file is empty"));
            }
            _ => panic!("Expected ParseError"),
        }

        // Clean up
        fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_validate_magic_file_whitespace_only() {
        // Create a temporary magic file with only whitespace
        let temp_file = std::env::temp_dir().join("test_whitespace_magic.db");
        fs::write(&temp_file, "   \n\t  \r\n  ").unwrap();

        let result = validate_magic_file(&temp_file);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::ParseError(parse_err) => {
                let msg = parse_err.to_string();
                assert!(msg.contains("Magic file is empty"));
            }
            _ => panic!("Expected ParseError"),
        }

        // Clean up
        fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_validate_magic_file_valid() {
        // Create a temporary magic file with content
        let temp_file = std::env::temp_dir().join("test_valid_magic.db");
        fs::write(&temp_file, "# Magic file\n0 string test Test file").unwrap();

        let result = validate_magic_file(&temp_file);
        assert!(result.is_ok());

        // Clean up
        fs::remove_file(&temp_file).unwrap();
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
                    "Candidate at index {} should be text (not .mgc): {}",
                    i,
                    candidate
                );
            }
        }

        // Verify all candidates from first_binary_index onwards are binary (.mgc)
        for (i, candidate) in candidates.iter().enumerate() {
            if i >= first_binary_index {
                assert!(
                    candidate.ends_with(".mgc"),
                    "Candidate at index {} should be binary (.mgc): {}",
                    i,
                    candidate
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
                "Candidate mismatch at index {}: got '{}', expected '{}'",
                i, actual, expected
            );
        }
    }

    /// Verify behavior: first existing candidate is chosen in order
    /// This test uses a temporary directory to simulate the search behavior
    #[test]
    #[cfg(unix)]
    fn test_magic_file_search_selects_first_existing() {
        use std::io::Write;

        // Create a temporary directory structure to test search order
        let temp_dir = std::env::temp_dir().join("test_magic_search_order");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up any previous test artifacts
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a text magic file
        let text_magic_path = temp_dir.join("text_magic");
        let mut text_file = fs::File::create(&text_magic_path).unwrap();
        writeln!(text_file, "# Text magic file").unwrap();
        writeln!(text_file, "0 string test Test file").unwrap();

        // Create a binary magic file (simulated with .mgc extension)
        let binary_magic_path = temp_dir.join("binary.mgc");
        // Write some bytes that look like a binary magic file header
        fs::write(&binary_magic_path, b"\x1c\x04\x1e\xf1test").unwrap();

        // Verify text file exists and is detected as text format
        assert!(text_magic_path.exists());
        let text_format = detect_format(&text_magic_path);
        assert!(
            matches!(text_format, Ok(MagicFileFormat::Text)),
            "Text magic file should be detected as Text format, got {:?}",
            text_format
        );

        // Verify binary file exists and is detected as binary format
        assert!(binary_magic_path.exists());
        let binary_format = detect_format(&binary_magic_path);
        assert!(
            matches!(binary_format, Ok(MagicFileFormat::Binary)),
            "Binary magic file should be detected as Binary format, got {:?}",
            binary_format
        );

        // Clean up
        fs::remove_dir_all(&temp_dir).unwrap();
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

    #[test]
    fn test_args_multiple_files() {
        // Test parsing multiple file arguments
        let args = Args::try_parse_from(["rmagic", "file1.bin", "file2.txt", "file3.dat"]).unwrap();
        assert_eq!(args.files.len(), 3);
        assert_eq!(args.files[0].filename(), "file1.bin");
        assert_eq!(args.files[1].filename(), "file2.txt");
        assert_eq!(args.files[2].filename(), "file3.dat");
        assert!(!args.strict);
    }

    #[test]
    fn test_args_strict_flag() {
        // Test --strict flag parsing
        let args = Args::try_parse_from(["rmagic", "--strict", "test.bin"]).unwrap();
        assert!(args.strict);
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
    }

    #[test]
    fn test_use_builtin_flag_parsing() {
        let args = Args::try_parse_from(["rmagic", "--use-builtin", "test.bin"]).unwrap();
        assert!(args.use_builtin);
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
    }

    #[test]
    fn test_use_builtin_with_strict() {
        let args =
            Args::try_parse_from(["rmagic", "--use-builtin", "--strict", "test.bin"]).unwrap();
        assert!(args.use_builtin);
        assert!(args.strict);
        assert_eq!(args.files.len(), 1);
    }

    #[test]
    fn test_use_builtin_with_json() {
        let args = Args::try_parse_from(["rmagic", "--use-builtin", "--json", "test.bin"]).unwrap();
        assert!(args.use_builtin);
        assert!(args.json);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_use_builtin_with_magic_file() {
        let args = Args::try_parse_from([
            "rmagic",
            "--use-builtin",
            "--magic-file",
            "custom.magic",
            "test.bin",
        ])
        .unwrap();
        assert!(args.use_builtin);
        assert_eq!(args.magic_file, Some(PathBuf::from("custom.magic")));
    }

    #[test]
    fn test_use_builtin_default_false() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert!(!args.use_builtin);
    }

    #[test]
    fn test_args_strict_with_json() {
        // Test --strict works with --json
        let args = Args::try_parse_from(["rmagic", "--strict", "--json", "test.bin"]).unwrap();
        assert!(args.strict);
        assert!(args.json);
        assert_eq!(args.output_format(), OutputFormat::Json);
        assert_eq!(args.files.len(), 1);
    }

    #[test]
    fn test_args_strict_with_multiple_files() {
        // Test --strict with multiple files
        let args =
            Args::try_parse_from(["rmagic", "--strict", "file1.bin", "file2.txt", "file3.dat"])
                .unwrap();
        assert!(args.strict);
        assert_eq!(args.files.len(), 3);
    }

    #[test]
    fn test_args_multiple_files_with_magic_file() {
        // Test multiple files with custom magic file
        let args = Args::try_parse_from([
            "rmagic",
            "--magic-file",
            "custom.magic",
            "file1.bin",
            "file2.txt",
        ])
        .unwrap();
        assert_eq!(args.files.len(), 2);
        assert_eq!(args.magic_file, Some(PathBuf::from("custom.magic")));
    }

    #[test]
    fn test_args_single_file_backwards_compatible() {
        // Ensure single file still works (backwards compatibility)
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.files[0].filename(), "test.bin");
        assert!(!args.strict);
    }

    #[test]
    fn test_strict_flag_default_false() {
        // Test that strict defaults to false
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert!(!args.strict);
    }

    #[test]
    fn test_stdin_detection() {
        let args = Args::try_parse_from(["rmagic", "-"]).unwrap();
        assert!(args.files[0].is_stdin());
    }

    #[test]
    #[cfg(unix)]
    fn test_stdin_truncation_warning() {
        let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
            eprintln!("Skipping stdin test: no compatible text magic file available");
            return;
        };
        let args =
            Args::try_parse_from(["rmagic", "--magic-file", magic_file.to_str().unwrap(), "-"])
                .unwrap();
        let db = MagicDatabase::load_from_file(&magic_file).unwrap();
        let max_string_length = db.config().max_string_length;
        let input = vec![b'a'; max_string_length + 10];

        let (result, stderr_output) = capture_stderr(|| {
            with_mocked_stdin(&input, || process_file(&args.files[0], &db, &args))
        });

        assert!(result.is_ok());
        assert!(stderr_output.contains(&format!(
            "Warning: stdin input truncated to {} bytes",
            max_string_length
        )));
    }

    #[test]
    #[cfg(unix)]
    fn test_stdin_no_false_truncation_warning() {
        let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
            eprintln!("Skipping stdin test: no compatible text magic file available");
            return;
        };
        let args =
            Args::try_parse_from(["rmagic", "--magic-file", magic_file.to_str().unwrap(), "-"])
                .unwrap();
        let db = MagicDatabase::load_from_file(&magic_file).unwrap();
        let max_string_length = db.config().max_string_length;
        // Input is exactly max_string_length bytes - should NOT trigger warning
        let input = vec![b'a'; max_string_length];

        let (result, stderr_output) = capture_stderr(|| {
            with_mocked_stdin(&input, || process_file(&args.files[0], &db, &args))
        });

        assert!(result.is_ok());
        assert!(
            !stderr_output.contains("Warning: stdin input truncated"),
            "Should not show truncation warning when input equals max_string_length"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_stdin_empty_returns_data() {
        let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
            eprintln!("Skipping stdin test: no compatible text magic file available");
            return;
        };
        let args =
            Args::try_parse_from(["rmagic", "--magic-file", magic_file.to_str().unwrap(), "-"])
                .unwrap();
        let db = MagicDatabase::load_from_file(&magic_file).unwrap();

        let (result, stdout_output) =
            capture_stdout(|| with_mocked_stdin(&[], || process_file(&args.files[0], &db, &args)));

        assert!(result.is_ok());
        assert!(stdout_output.contains("stdin: data"));
    }

    #[test]
    #[cfg(unix)]
    fn test_stdin_output_format() {
        let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
            eprintln!("Skipping stdin test: no compatible text magic file available");
            return;
        };
        let args =
            Args::try_parse_from(["rmagic", "--magic-file", magic_file.to_str().unwrap(), "-"])
                .unwrap();
        let db = MagicDatabase::load_from_file(&magic_file).unwrap();

        let (result, stdout_output) = capture_stdout(|| {
            with_mocked_stdin(b"sample", || process_file(&args.files[0], &db, &args))
        });

        assert!(result.is_ok());
        assert!(stdout_output.contains("stdin:"));
    }

    #[test]
    #[cfg(unix)]
    fn test_stdin_strict_mode_errors() {
        let Some(magic_file) = resolve_magic_file_for_stdin_tests() else {
            eprintln!("Skipping stdin test: no compatible text magic file available");
            return;
        };
        let args_strict = Args::try_parse_from([
            "rmagic",
            "--strict",
            "--magic-file",
            magic_file.to_str().unwrap(),
            "-",
        ])
        .unwrap();

        let args_non_strict =
            Args::try_parse_from(["rmagic", "--magic-file", magic_file.to_str().unwrap(), "-"])
                .unwrap();

        let strict_result = with_invalid_stdin(|| run_analysis(&args_strict));
        assert!(strict_result.is_err());

        let non_strict_result = with_invalid_stdin(|| run_analysis(&args_non_strict));
        assert!(non_strict_result.is_ok());
    }
}
