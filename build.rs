// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

// Stub module to satisfy error.rs dependencies during build
#[allow(dead_code)]
mod evaluator {
    pub mod types {
        #[derive(Debug)]
        pub struct TypeReadError;
        impl std::fmt::Display for TypeReadError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "TypeReadError")
            }
        }
        impl std::error::Error for TypeReadError {}
    }
}

#[allow(dead_code, unused_imports)]
#[path = "src/error.rs"]
mod error;
#[allow(dead_code, unused_imports)]
#[path = "src/parser/mod.rs"]
mod parser;

use error::ParseError;
use parser::codegen;
use parser::parse_text_magic_file;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    println!("cargo:rerun-if-changed=src/builtin_rules.magic");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to read CARGO_MANIFEST_DIR: {err}");
            process::exit(1);
        }
    };

    let magic_path = Path::new(&manifest_dir).join("src/builtin_rules.magic");
    let magic_content = match fs::read_to_string(&magic_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", magic_path.display());
            process::exit(1);
        }
    };

    let rules = match parse_text_magic_file(&magic_content) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", format_parse_error(&err));
            process::exit(1);
        }
    };

    let out_dir = match env::var("OUT_DIR") {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to read OUT_DIR: {err}");
            process::exit(1);
        }
    };

    let output_path = Path::new(&out_dir).join("builtin_rules.rs");
    let generated = codegen::generate_builtin_rules(&rules);

    if let Err(err) = fs::write(&output_path, generated) {
        eprintln!("Failed to write {}: {err}", output_path.display());
        process::exit(1);
    }
}

fn format_parse_error(error: &ParseError) -> String {
    match error {
        ParseError::InvalidSyntax { line, message } => {
            format!("Error parsing builtin_rules.magic at line {line}: {message}")
        }
        ParseError::UnsupportedFeature { line, feature } => {
            format!("Error parsing builtin_rules.magic at line {line}: {feature}")
        }
        ParseError::InvalidOffset { line, offset } => {
            format!("Error parsing builtin_rules.magic at line {line}: {offset}")
        }
        ParseError::InvalidType { line, type_spec } => {
            format!("Error parsing builtin_rules.magic at line {line}: {type_spec}")
        }
        ParseError::InvalidOperator { line, operator } => {
            format!("Error parsing builtin_rules.magic at line {line}: {operator}")
        }
        ParseError::InvalidValue { line, value } => {
            format!("Error parsing builtin_rules.magic at line {line}: {value}")
        }
        ParseError::UnsupportedFormat {
            line,
            format_type,
            message,
        } => format!("Error parsing builtin_rules.magic at line {line}: {format_type} {message}"),
        ParseError::IoError(err) => {
            format!("Error parsing builtin_rules.magic: I/O error: {err}")
        }
    }
}
