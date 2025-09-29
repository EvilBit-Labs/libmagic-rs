//! Command-line interface for libmagic-rs
//!
//! This binary provides a CLI tool for file type identification using magic rules,
//! serving as a drop-in replacement for the GNU `file` command.

use clap::{Arg, Command};
use libmagic_rs::{LibmagicError, MagicDatabase};
use std::path::Path;
use std::process;

fn main() {
    let matches = Command::new("rmagic")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Rust Libmagic Contributors")
        .about("A pure-Rust implementation of libmagic for file type identification")
        .arg(
            Arg::new("file")
                .help("File to analyze")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Output results in JSON format")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("text")
                .long("text")
                .help("Output results in text format (default)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("magic-file")
                .long("magic-file")
                .help("Use custom magic file")
                .value_name("FILE"),
        )
        .get_matches();

    let file_path = matches.get_one::<String>("file").unwrap();
    let json_output = matches.get_flag("json");
    let _magic_file = matches.get_one::<String>("magic-file");

    if let Err(e) = run_analysis(file_path, json_output) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run_analysis(file_path: &str, json_output: bool) -> Result<(), LibmagicError> {
    // Verify file exists
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(LibmagicError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", file_path),
        )));
    }

    // Load magic database (placeholder implementation)
    let db = MagicDatabase::load_from_file("magic.db")?;

    // Evaluate file
    let result = db.evaluate_file(path)?;

    // Output results
    if json_output {
        let json_result = serde_json::json!({
            "filename": file_path,
            "description": result.description,
            "mime_type": result.mime_type,
            "confidence": result.confidence
        });
        println!("{}", serde_json::to_string_pretty(&json_result).unwrap());
    } else {
        println!("{}: {}", file_path, result.description);
    }

    Ok(())
}
