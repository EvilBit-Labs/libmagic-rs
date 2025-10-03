//! Command-line interface for libmagic-rs
//!
//! This binary provides a CLI tool for file type identification using magic rules,
//! serving as a drop-in replacement for the GNU `file` command.

use clap::Parser;
use libmagic_rs::{LibmagicError, MagicDatabase};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// A pure-Rust implementation of libmagic for file type identification
#[derive(Parser, Debug)]
#[command(
    name = "rmagic",
    version = env!("CARGO_PKG_VERSION"),
    author = "Rust Libmagic Contributors",
    about = "A pure-Rust implementation of libmagic for file type identification"
)]
pub struct Args {
    /// File to analyze
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Output results in JSON format
    #[arg(long, conflicts_with = "text")]
    pub json: bool,

    /// Output results in text format (default)
    #[arg(long)]
    pub text: bool,

    /// Use custom magic file
    #[arg(long = "magic-file", value_name = "FILE")]
    pub magic_file: Option<PathBuf>,
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

    /// Get the default magic file path for the current platform
    fn default_magic_file_path() -> PathBuf {
        #[cfg(unix)]
        {
            // Try multiple common locations on Unix-like systems
            let candidates = [
                "/etc/magic",
                "/usr/share/misc/magic",
                "/usr/share/file/magic",
                "/opt/local/share/file/magic", // MacPorts
                "/usr/local/share/misc/magic", // FreeBSD
            ];

            for candidate in &candidates {
                let path = PathBuf::from(candidate);
                if path.exists() {
                    return path;
                }
            }

            // Fallback to test files if in CI/CD environment
            if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
                return PathBuf::from("test_files/magic");
            }

            // Default fallback
            PathBuf::from("/etc/magic")
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

            // Fallback to test files (common in CI/CD)
            PathBuf::from("test_files/magic")
        }
        #[cfg(not(any(unix, windows)))]
        {
            PathBuf::from("test_files/magic")
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
        LibmagicError::IoError(ref io_err) => match io_err.kind() {
            std::io::ErrorKind::NotFound => {
                eprintln!("Error: File not found");
                eprintln!("The specified file does not exist or cannot be accessed.");
                eprintln!("Please check the file path and try again.");
                3
            }
            std::io::ErrorKind::PermissionDenied => {
                eprintln!("Error: Permission denied");
                eprintln!("You do not have permission to access the specified file.");
                eprintln!("Please check file permissions or run with appropriate privileges.");
                3
            }
            std::io::ErrorKind::InvalidInput => {
                eprintln!("Error: Invalid input");
                eprintln!("The file path or arguments provided are invalid.");
                eprintln!("Please check your input and try again.");
                2
            }
            _ => {
                eprintln!("Error: File access failed");
                eprintln!("Failed to access file: {}", io_err);
                eprintln!("Please check the file path and permissions.");
                3
            }
        },
        LibmagicError::ParseError { line, message } => {
            eprintln!("Error: Magic file parse error");
            eprintln!("Parse error at line {}: {}", line, message);
            eprintln!("The magic file contains invalid syntax or formatting.");
            eprintln!("Please check the magic file format or try a different magic file.");
            4
        }
        LibmagicError::InvalidFormat(ref msg) => {
            eprintln!("Error: Invalid magic file format");
            eprintln!("{}", msg);
            eprintln!("The magic file format is not supported or contains errors.");
            eprintln!("Please use a valid magic file or check the file format.");
            4
        }
        LibmagicError::FileBufferError(ref msg) => {
            eprintln!("Error: File buffer error");
            eprintln!("{}", msg);
            eprintln!("Failed to create memory-mapped buffer for the file.");
            eprintln!("The file may be too large, corrupted, or in use by another process.");
            3
        }
        LibmagicError::EvaluationError(ref msg) => {
            eprintln!("Error: Rule evaluation failed");
            eprintln!("{}", msg);
            eprintln!("Failed to evaluate magic rules against the file.");
            eprintln!("The file may be corrupted or the magic rules may be incompatible.");
            1
        }
        LibmagicError::Timeout { timeout_ms } => {
            eprintln!("Error: Evaluation timeout");
            eprintln!("File analysis timed out after {}ms", timeout_ms);
            eprintln!("The file may be too large or complex to analyze within the time limit.");
            eprintln!("Try using a simpler magic file or increasing the timeout limit.");
            5
        }
    }
}

fn run_analysis(args: &Args) -> Result<(), LibmagicError> {
    // Validate input arguments
    validate_arguments(args)?;

    // Verify file exists and is accessible
    validate_input_file(&args.file)?;

    // Load magic database with platform-appropriate defaults
    let magic_file_path = args.get_magic_file_path();

    // Check if magic file exists and provide helpful error message
    if !magic_file_path.exists() {
        eprintln!(
            "Warning: Magic file not found at {}",
            magic_file_path.display()
        );
        eprintln!("Attempting to create basic magic file...");

        // Try to create basic magic files if we're in CI/CD or test environment
        if let Err(e) = download_magic_files(&magic_file_path) {
            return Err(LibmagicError::InvalidFormat(format!(
                "Magic file not found at {} and failed to create fallback: {}",
                magic_file_path.display(),
                e
            )));
        }
    }

    // Validate magic file before loading
    validate_magic_file(&magic_file_path)?;

    let db = MagicDatabase::load_from_file(&magic_file_path)?;

    // Evaluate file
    let result = db.evaluate_file(&args.file)?;

    // Output results based on format
    match args.output_format() {
        OutputFormat::Json => {
            let json_result = serde_json::json!({
                "filename": args.file.display().to_string(),
                "description": result.description,
                "mime_type": result.mime_type,
                "confidence": result.confidence
            });
            match serde_json::to_string_pretty(&json_result) {
                Ok(json_str) => println!("{}", json_str),
                Err(e) => {
                    return Err(LibmagicError::EvaluationError(format!(
                        "Failed to serialize JSON output: {}",
                        e
                    )));
                }
            }
        }
        OutputFormat::Text => {
            println!("{}: {}", args.file.display(), result.description);
        }
    }

    Ok(())
}

/// Validate command-line arguments
fn validate_arguments(args: &Args) -> Result<(), LibmagicError> {
    // Check if file path is empty or contains only whitespace
    let file_str = args.file.to_string_lossy();
    if file_str.trim().is_empty() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "File path cannot be empty",
        )));
    }

    // Validate custom magic file path if provided
    if let Some(ref magic_file) = args.magic_file {
        let magic_str = magic_file.to_string_lossy();
        if magic_str.trim().is_empty() {
            return Err(LibmagicError::InvalidFormat(
                "Magic file path cannot be empty".to_string(),
            ));
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
        return Err(LibmagicError::InvalidFormat(format!(
            "Magic file not found: {}",
            magic_file_path.display()
        )));
    }

    // Check if it's a directory
    if magic_file_path.is_dir() {
        return Err(LibmagicError::InvalidFormat(format!(
            "Magic file path is a directory, not a file: {}",
            magic_file_path.display()
        )));
    }

    // Try to read the magic file to check permissions and basic format
    match fs::read_to_string(magic_file_path) {
        Ok(content) => {
            // Basic validation - check if file is completely empty
            if content.trim().is_empty() {
                return Err(LibmagicError::InvalidFormat(
                    "Magic file is empty".to_string(),
                ));
            }
            Ok(())
        }
        Err(e) => Err(LibmagicError::IoError(e)),
    }
}

/// Download magic files for CI/CD environments
///
/// This function attempts to create a basic magic file if one doesn't exist,
/// particularly useful in CI/CD environments where system magic files may not be available.
fn download_magic_files(magic_file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = magic_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // If the file already exists, don't overwrite it
    if magic_file_path.exists() {
        return Ok(());
    }

    // Create a basic magic file with common file type signatures
    let basic_magic_content = r#"# Basic magic file for libmagic-rs
# This is a minimal magic file for testing and CI/CD environments

# ELF executables
0	string	\x7fELF	ELF
>4	byte	1	32-bit
>4	byte	2	64-bit
>5	byte	1	LSB
>5	byte	2	MSB

# PE executables
0	string	MZ	MS-DOS executable
>60	lelong	0x00004550	PE32 executable

# ZIP archives
0	string	PK\x03\x04	ZIP archive
0	string	PK\x05\x06	ZIP archive (empty)
0	string	PK\x07\x08	ZIP archive (spanned)

# JPEG images
0	string	\xff\xd8\xff	JPEG image data

# PNG images
0	string	\x89PNG\r\n\x1a\n	PNG image data

# GIF images
0	string	GIF87a	GIF image data, version 87a
0	string	GIF89a	GIF image data, version 89a

# PDF documents
0	string	%PDF-	PDF document

# Text files
0	string	#!/bin/sh	shell script
0	string	#!/bin/bash	Bash script
0	string	#!/usr/bin/env	script text

# Common text patterns
0	string	<?xml	XML document
0	string	<html	HTML document
0	string	<!DOCTYPE	HTML document

# Archive formats
0	string	\x1f\x8b	gzip compressed data
0	string	BZh	bzip2 compressed data
0	string	\xfd7zXZ\x00	XZ compressed data

# Binary formats
0	string	\x89HDF	HDF data
0	string	\xca\xfe\xba\xbe	Java class file
0	string	\xfe\xed\xfa\xce	Mach-O executable (32-bit)
0	string	\xfe\xed\xfa\xcf	Mach-O executable (64-bit)
"#;

    fs::write(magic_file_path, basic_magic_content)?;
    eprintln!("Created basic magic file at {}", magic_file_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::fs;

    #[test]
    fn test_basic_file_argument() {
        let args = Args::try_parse_from(["rmagic", "test.bin"]).unwrap();
        assert_eq!(args.file, PathBuf::from("test.bin"));
        assert!(!args.json);
        assert!(!args.text);
        assert_eq!(args.output_format(), OutputFormat::Text);
        assert!(args.magic_file.is_none());
    }

    #[test]
    fn test_json_output_flag() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--json"]).unwrap();
        assert_eq!(args.file, PathBuf::from("test.bin"));
        assert!(args.json);
        assert!(!args.text);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_text_output_flag() {
        let args = Args::try_parse_from(["rmagic", "test.bin", "--text"]).unwrap();
        assert_eq!(args.file, PathBuf::from("test.bin"));
        assert!(!args.json);
        assert!(args.text);
        assert_eq!(args.output_format(), OutputFormat::Text);
    }

    #[test]
    fn test_magic_file_argument() {
        let args =
            Args::try_parse_from(["rmagic", "test.bin", "--magic-file", "custom.magic"]).unwrap();
        assert_eq!(args.file, PathBuf::from("test.bin"));
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
        assert_eq!(args.file, PathBuf::from("test.bin"));
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
        assert_eq!(args.file, PathBuf::from("/path/to/complex file.bin"));
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
        #[cfg(unix)]
        assert_eq!(default_path, PathBuf::from("/etc/magic"));

        #[cfg(windows)]
        assert_eq!(default_path, PathBuf::from("test_files/magic"));

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("test_files/magic"));
    }

    #[test]
    fn test_default_magic_file_path() {
        let default_path = Args::default_magic_file_path();

        // Test that we get a platform-appropriate default
        #[cfg(unix)]
        assert_eq!(default_path, PathBuf::from("/etc/magic"));

        #[cfg(windows)]
        assert_eq!(default_path, PathBuf::from("test_files/magic"));

        #[cfg(not(any(unix, windows)))]
        assert_eq!(default_path, PathBuf::from("test_files/magic"));
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
        let error = LibmagicError::ParseError {
            line: 42,
            message: "Invalid syntax".to_string(),
        };
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 4);
    }

    #[test]
    fn test_handle_error_invalid_format() {
        let error = LibmagicError::InvalidFormat("Bad format".to_string());
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 4);
    }

    #[test]
    fn test_handle_error_file_buffer_error() {
        let error = LibmagicError::FileBufferError("Buffer error".to_string());
        let exit_code = handle_error(error);
        assert_eq!(exit_code, 3);
    }

    #[test]
    fn test_handle_error_evaluation_error() {
        let error = LibmagicError::EvaluationError("Evaluation failed".to_string());
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
    fn test_validate_arguments_empty_file_path() {
        let args = Args {
            file: PathBuf::from(""),
            json: false,
            text: false,
            magic_file: None,
        };
        let result = validate_arguments(&args);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(e.to_string().contains("File path cannot be empty"));
            }
            _ => panic!("Expected IoError with InvalidInput"),
        }
    }

    #[test]
    fn test_validate_arguments_whitespace_file_path() {
        let args = Args {
            file: PathBuf::from("   "),
            json: false,
            text: false,
            magic_file: None,
        };
        let result = validate_arguments(&args);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(e.to_string().contains("File path cannot be empty"));
            }
            _ => panic!("Expected IoError with InvalidInput"),
        }
    }

    #[test]
    fn test_validate_arguments_empty_magic_file() {
        let args = Args {
            file: PathBuf::from("test.bin"),
            json: false,
            text: false,
            magic_file: Some(PathBuf::from("")),
        };
        let result = validate_arguments(&args);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("Magic file path cannot be empty"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_validate_arguments_valid() {
        let args = Args {
            file: PathBuf::from("test.bin"),
            json: false,
            text: false,
            magic_file: Some(PathBuf::from("magic.db")),
        };
        let result = validate_arguments(&args);
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
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("Magic file not found"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_validate_magic_file_directory() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_validate_magic_dir");
        fs::create_dir_all(&temp_dir).unwrap();

        let result = validate_magic_file(&temp_dir);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("Magic file path is a directory"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }

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
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("Magic file is empty"));
            }
            _ => panic!("Expected InvalidFormat error"),
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
            LibmagicError::InvalidFormat(msg) => {
                assert!(msg.contains("Magic file is empty"));
            }
            _ => panic!("Expected InvalidFormat error"),
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
}
