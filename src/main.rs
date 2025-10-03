//! Command-line interface for libmagic-rs
//!
//! This binary provides a CLI tool for file type identification using magic rules,
//! serving as a drop-in replacement for the GNU `file` command.

use clap::Parser;
use libmagic_rs::{LibmagicError, MagicDatabase};
use std::path::PathBuf;
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
            PathBuf::from("/etc/magic")
        }
        #[cfg(windows)]
        {
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

    if let Err(e) = run_analysis(&args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run_analysis(args: &Args) -> Result<(), LibmagicError> {
    // Verify file exists
    if !args.file.exists() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", args.file.display()),
        )));
    }

    // Load magic database with platform-appropriate defaults
    let magic_file_path = args.get_magic_file_path();
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
            println!("{}", serde_json::to_string_pretty(&json_result).unwrap());
        }
        OutputFormat::Text => {
            println!("{}: {}", args.file.display(), result.description);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

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
}
