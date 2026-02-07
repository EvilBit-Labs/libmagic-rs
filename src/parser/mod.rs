//! Magic file parser module
//!
//! This module handles parsing of magic files into an Abstract Syntax Tree (AST)
//! that can be evaluated against file buffers for type identification.
//!
//! # Overview
//!
//! The parser implements a complete pipeline for transforming magic file text into
//! a hierarchical rule structure suitable for evaluation. The pipeline consists of:
//!
//! 1. **Preprocessing**: Line handling, comment removal, continuation processing
//! 2. **Parsing**: Individual magic rule parsing using nom combinators
//! 3. **Hierarchy Building**: Constructing parent-child relationships based on indentation
//! 4. **Validation**: Type checking and offset resolution
//!
//! # Format Detection and Loading
//!
//! The module automatically detects and handles three types of magic file formats:
//! - **Text files**: Human-readable magic rule definitions
//! - **Directories**: Collections of magic files (Magdir pattern)
//! - **Binary files**: Compiled .mgc files (currently unsupported)
//!
//! ## Unified Loading API
//!
//! The recommended entry point for loading magic files is [`load_magic_file()`], which
//! automatically detects the format and dispatches to the appropriate handler:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_file;
//! use std::path::Path;
//!
//! // Works with text files
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic"))?;
//!
//! // Also works with directories
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic.d"))?;
//!
//! // Binary .mgc files return an error with guidance
//! match load_magic_file(Path::new("/usr/share/misc/magic.mgc")) {
//!     Ok(rules) => { /* ... */ },
//!     Err(e) => eprintln!("Use --use-builtin for binary files: {}", e),
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Three-Tier Loading Strategy
//!
//! The loading process works as follows:
//!
//! 1. **Format Detection**: [`detect_format()`] examines the path to determine the file type
//! 2. **Dispatch to Handler**:
//!    - Text files → [`parse_text_magic_file()`] after reading contents
//!    - Directories → [`load_magic_directory()`] to load and merge all files
//!    - Binary files → Returns error suggesting `--use-builtin` option
//! 3. **Return Merged Rules**: All rules are returned in a single `Vec<MagicRule>`
//!
//! # Examples
//!
//! ## Loading Magic Files (Recommended)
//!
//! Use the unified [`load_magic_file()`] API for automatic format detection:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_file;
//! use std::path::Path;
//!
//! let rules = load_magic_file(Path::new("/usr/share/misc/magic"))?;
//! println!("Loaded {} magic rules", rules.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Parsing Text Content Directly
//!
//! For parsing magic rule text that's already in memory:
//!
//! ```ignore
//! use libmagic_rs::parser::parse_text_magic_file;
//!
//! let magic_content = r#"
//! 0 string \x7fELF ELF executable
//! >4 byte 1 32-bit
//! >4 byte 2 64-bit
//! "#;
//!
//! let rules = parse_text_magic_file(magic_content)?;
//! assert_eq!(rules.len(), 1);
//! assert_eq!(rules[0].children.len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Loading a Directory Explicitly
//!
//! For Magdir-style directories containing multiple magic files:
//!
//! ```ignore
//! use libmagic_rs::parser::load_magic_directory;
//! use std::path::Path;
//!
//! // Directory structure:
//! // /usr/share/file/magic.d/
//! //   ├── elf
//! //   ├── archive
//! //   └── text
//!
//! let rules = load_magic_directory(Path::new("/usr/share/file/magic.d"))?;
//! // Rules from all files are merged in alphabetical order by filename
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Migration Note
//!
//! **For users upgrading from direct function calls:**
//!
//! - **Old approach**: Call `detect_format()` then dispatch manually
//! - **New approach**: Use `load_magic_file()` for automatic dispatching
//!
//! The individual functions (`parse_text_magic_file()`, `load_magic_directory()`)
//! remain available for advanced use cases where you need direct control.
//!
//! **Key differences:**
//! - `load_magic_file()`: Unified API with automatic format detection (recommended)
//! - `parse_text_magic_file()`: Parses a single text string containing magic rules
//! - `load_magic_directory()`: Loads and merges all magic files from a directory
//! - `detect_format()`: Low-level format detection (now called internally by `load_magic_file()`)
//!
//! **Error handling in `load_magic_directory()`:**
//! - Critical errors (I/O failures, invalid UTF-8): Returns `ParseError` immediately
//! - Non-critical errors (parse failures in individual files): Logs warning to stderr and continues

pub mod ast;
pub mod grammar;

// Re-export AST types for convenience
pub use ast::{Endianness, MagicRule, OffsetSpec, Operator, StrengthModifier, TypeKind, Value};

// Re-export parser functions for convenience
pub use grammar::{parse_number, parse_offset};

use crate::{
    error::ParseError,
    parser::grammar::{
        has_continuation, is_comment_line, is_empty_line, is_strength_directive, parse_comment,
        parse_magic_rule, parse_strength_directive,
    },
};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Represents the format of a magic file or directory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicFileFormat {
    /// Text-based magic file (human-readable)
    Text,
    /// Directory containing multiple magic files (Magdir pattern)
    Directory,
    /// Binary compiled magic file (.mgc format)
    Binary,
}

/// Detect the format of a magic file or directory
///
/// This function examines the filesystem metadata and file contents to determine
/// whether the path points to a text magic file, a directory, or a binary .mgc file.
///
/// # Detection Logic
///
/// 1. Check if path is a directory → `MagicFileFormat::Directory`
/// 2. Read first 4 bytes and check for binary magic number `0xF11E041C` → `MagicFileFormat::Binary`
/// 3. Otherwise → `MagicFileFormat::Text`
///
/// # Arguments
///
/// * `path` - Path to the magic file or directory to detect
///
/// # Errors
///
/// Returns `ParseError::IoError` if the path doesn't exist or cannot be read.
///
/// # Notes
///
/// This function only detects the format and returns it. It does not validate whether
/// the format is supported by the parser. Higher-level code should check the returned
/// format and decide how to handle unsupported formats (e.g., binary .mgc files).
///
/// # Examples
///
/// ```rust,no_run
/// use libmagic_rs::parser::detect_format;
/// use std::path::Path;
///
/// let format = detect_format(Path::new("/usr/share/file/magic"))?;
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
pub fn detect_format(path: &Path) -> Result<MagicFileFormat, ParseError> {
    // Check if path exists and is accessible
    let metadata = std::fs::metadata(path)?;

    // Check if it's a directory
    if metadata.is_dir() {
        return Ok(MagicFileFormat::Directory);
    }

    // Read first 4 bytes to check for binary magic number
    let mut file = std::fs::File::open(path)?;

    let mut magic_bytes = [0u8; 4];

    match file.read_exact(&mut magic_bytes) {
        Ok(()) => {
            // Check for binary magic number 0xF11E041C in little-endian format
            let magic_number = u32::from_le_bytes(magic_bytes);
            if magic_number == 0xF11E_041C {
                return Ok(MagicFileFormat::Binary);
            }
            // Not a binary magic file, assume text
            Ok(MagicFileFormat::Text)
        }
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            // File is too small to be a binary magic file, assume text
            Ok(MagicFileFormat::Text)
        }
        Err(e) => Err(ParseError::invalid_syntax(
            0,
            format!("Failed to read magic file: {e}"),
        )),
    }
}

/// Internal structure to track line metadata during preprocessing.
///
/// Stores the processed content, original line number, and flags for comment
/// and strength directive lines in the input magic file.
#[derive(Debug)]
struct LineInfo {
    content: String,
    line_number: usize,
    is_comment: bool,
    /// Optional strength modifier parsed from `!:strength` directive
    strength_modifier: Option<StrengthModifier>,
}

impl LineInfo {
    fn new(content: String, line_number: usize, is_comment: bool) -> Self {
        Self {
            content,
            line_number,
            is_comment,
            strength_modifier: None,
        }
    }

    fn with_strength(
        content: String,
        line_number: usize,
        strength_modifier: StrengthModifier,
    ) -> Self {
        Self {
            content,
            line_number,
            is_comment: false,
            strength_modifier: Some(strength_modifier),
        }
    }
}

/// Preprocesses raw magic file input by handling comments, empty lines, and continuations.
///
/// This function performs the following transformations:
/// - Removes empty lines from the input
/// - Handles comment lines (lines starting with '#')
/// - Processes line continuations (lines ending with '\')
/// - Concatenates continued lines into single entries
/// - Preserves original line numbers for error reporting (continued lines
///   are assigned the line number of the first line in the continuation sequence)
///
/// # Arguments
///
/// * `input` - The raw magic file content as a string
///
/// # Returns
///
/// `Result<Vec<LineInfo>, ParseError>` - A vector of processed lines or a parse error
///
/// # Errors
///
/// Returns an error if:
/// - Comment lines cannot be parsed
/// - Input ends with an unterminated line continuation
/// - The input is malformed
///
/// # Examples
///
/// ```ignore
/// let input = r#"0 string 0 Test
/// >4 byte 1 Child"#;
/// let lines = preprocess_lines(input)?;
/// assert_eq!(lines.len(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn preprocess_lines(input: &str) -> Result<Vec<LineInfo>, ParseError> {
    let mut lines_info: Vec<LineInfo> = Vec::new();
    let mut line_buf = String::new();
    let mut start_line_number: Option<usize> = None;
    for (i, mut line) in input.lines().enumerate() {
        if is_empty_line(line) {
            continue;
        }
        if is_comment_line(line) {
            // Bug 1 fix: If we have an ongoing continuation, discard it before processing comment
            if !line_buf.is_empty() {
                line_buf.clear();
                start_line_number = None;
            }
            let parsed_comment = parse_comment(line)
                .map_err(|_| ParseError::invalid_syntax(i + 1, "Unable to parse comment"))?;
            line = parsed_comment.1.as_str();
            lines_info.push(LineInfo::new(line.trim().to_string(), i + 1, true));
            continue;
        }
        // Handle strength directives (!:strength ...)
        if is_strength_directive(line) {
            // If we have an ongoing continuation, discard it before processing directive
            if !line_buf.is_empty() {
                line_buf.clear();
                start_line_number = None;
            }
            let strength_modifier = parse_strength_directive(line)
                .map_err(|e| {
                    ParseError::invalid_syntax(
                        i + 1,
                        format!("Failed to parse strength directive: {e}"),
                    )
                })?
                .1;
            lines_info.push(LineInfo::with_strength(
                line.trim().to_string(),
                i + 1,
                strength_modifier,
            ));
            continue;
        }
        // Track the starting line number when we begin accumulating a rule
        if start_line_number.is_none() {
            start_line_number = Some(i + 1);
        }
        line_buf.push_str(line.trim());
        if has_continuation(line) {
            if let Some(stripped) = line_buf.strip_suffix('\\') {
                line_buf = stripped.to_string();
            }
            continue;
        }
        // Bug 2 fix: Use the stored starting line number instead of calculating from cont_ctr
        let rule_line_number = start_line_number.unwrap_or(i + 1);
        lines_info.push(LineInfo::new(
            std::mem::take(&mut line_buf),
            rule_line_number,
            false,
        ));
        start_line_number = None;
    }

    // Handle unterminated continuation at end of input
    if !line_buf.is_empty() {
        let last_line = input.lines().count();
        return Err(ParseError::invalid_syntax(
            last_line,
            "Unterminated line continuation",
        ));
    }

    Ok(lines_info)
}

/// Parses a single magic rule line into a `MagicRule` AST node.
///
/// This function takes a preprocessed `LineInfo` and converts it into a `MagicRule`
/// by delegating to the grammar parser. It handles error mapping to include
/// context about which line failed.
///
/// # Arguments
///
/// * `line` - The `LineInfo` struct containing the rule text and metadata
///
/// # Returns
///
/// `Result<MagicRule, ParseError>` - The parsed rule or a parse error
///
/// # Errors
///
/// Returns an error if:
/// - The line is marked as a comment
/// - The rule syntax is invalid
/// - Required fields are missing
/// - Value parsing fails
///
/// # Examples
///
/// ```ignore
/// let line = LineInfo::new("0 string 0 Test".to_string(), 1, false);
/// let rule = parse_magic_rule_line(&line)?;
/// assert_eq!(rule.level, 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn parse_magic_rule_line(line: &LineInfo) -> Result<MagicRule, ParseError> {
    if line.is_comment {
        return Err(ParseError::invalid_syntax(
            line.line_number,
            "Comment lines cannot be parsed as rules",
        ));
    }
    parse_magic_rule(&line.content)
        .map_err(|e| {
            ParseError::invalid_syntax(line.line_number, format!("Failed to parse rule: {e}"))
        })
        .map(|(_, rule)| rule)
}

/// Builds a hierarchical structure from a flat list of parsed magic rules.
///
/// This function establishes parent-child relationships based on indentation levels.
/// Rules at deeper indentation levels become children of the most recent rule at a
/// shallower level. This implements a stack-based algorithm for hierarchy construction.
///
/// # Arguments
///
/// * `lines` - A vector of preprocessed `LineInfo` structs
///
/// # Returns
///
/// `Result<Vec<MagicRule>, ParseError>` - Root-level rules with children attached
///
/// # Behavior
///
/// - Rules with `level=0` are root rules
/// - Rules with `level=1` become children of the most recent `level=0` rule
/// - Rules with `level=2` become children of the most recent `level=1` rule
/// - When indentation decreases, the stack is unwound and completed rules are attached
/// - Orphaned child rules (starting with '>' but with no preceding parent) are
///   added to the root list with their hierarchy level preserved
///
/// # Errors
///
/// Returns an error if:
/// - Any line contains invalid magic rule syntax
/// - Rule parsing fails (propagated from `parse_magic_rule_line`)
///
/// # Examples
///
/// ```ignore
/// let lines = vec![
///     LineInfo::new("0 string 0 ELF".to_string(), 1, false),
///     LineInfo::new(">4 byte 1 32-bit".to_string(), 2, false),
/// ];
/// let rules = build_rule_hierarchy(lines)?;
/// assert_eq!(rules[0].children.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn build_rule_hierarchy(lines: Vec<LineInfo>) -> Result<Vec<MagicRule>, ParseError> {
    /// Helper to pop a rule from the stack and attach it to its parent or roots
    fn pop_and_attach(stack: &mut Vec<MagicRule>, roots: &mut Vec<MagicRule>) {
        if let Some(completed) = stack.pop() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(completed);
            } else {
                roots.push(completed);
            }
        }
    }

    let mut stack: Vec<MagicRule> = Vec::new();
    let mut roots: Vec<MagicRule> = Vec::new();
    let mut pending_strength: Option<StrengthModifier> = None;

    for line in lines {
        if line.is_comment {
            continue;
        }

        // Handle strength directive: store modifier for next rule
        if line.strength_modifier.is_some() {
            pending_strength = line.strength_modifier;
            continue;
        }

        let mut rule = parse_magic_rule_line(&line)?;

        // Apply pending strength modifier to this rule
        if pending_strength.is_some() {
            rule.strength_modifier = pending_strength.take();
        }

        // Unwind stack until we find a parent with lower level
        while stack.last().is_some_and(|top| top.level >= rule.level) {
            pop_and_attach(&mut stack, &mut roots);
        }

        stack.push(rule);
    }

    // Unwind remaining stack
    while !stack.is_empty() {
        pop_and_attach(&mut stack, &mut roots);
    }

    Ok(roots)
}

/// Parses a complete magic file from raw text input.
///
/// This is the main public-facing parser function that orchestrates the complete
/// parsing pipeline: preprocessing, parsing individual rules, and building the
/// hierarchical structure.
///
/// # Arguments
///
/// * `input` - The raw magic file content as a string
///
/// # Returns
///
/// `Result<Vec<MagicRule>, ParseError>` - A vector of root rules with nested children
///
/// # Errors
///
/// Returns an error if any stage of parsing fails:
/// - Preprocessing errors
/// - Rule parsing errors
/// - Hierarchy building errors
///
/// # Example
///
/// ```ignore
/// use libmagic_rs::parser::parse_text_magic_file;
///
/// let magic = r#"0 string \x7fELF ELF file
/// >4 byte 1 32-bit
/// >4 byte 2 64-bit"#;
///
/// let rules = parse_text_magic_file(magic)?;
/// assert_eq!(rules.len(), 1);
/// assert_eq!(rules[0].message, "ELF file");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_text_magic_file(input: &str) -> Result<Vec<MagicRule>, ParseError> {
    let lines = preprocess_lines(input)?;
    build_rule_hierarchy(lines)
}

/// Loads and parses all magic files from a directory, merging them into a single rule set.
///
/// This function reads all regular files in the specified directory, parses each as a magic file,
/// and combines the resulting rules into a single `Vec<MagicRule>`. Files are processed in
/// alphabetical order by filename to ensure deterministic results.
///
/// # Error Handling Strategy
///
/// This function distinguishes between critical and non-critical errors:
///
/// - **Critical errors** (I/O failures, directory access issues, encoding problems):
///   These cause immediate failure and return a `ParseError`. The function stops processing
///   and propagates the error to the caller.
///
/// - **Non-critical errors** (individual file parse failures):
///   These are logged to stderr with a warning message and the file is skipped. Processing
///   continues with remaining files.
///
/// # Behavior
///
/// - Subdirectories are skipped (not recursively processed)
/// - Symbolic links are skipped
/// - Empty directories return an empty rules vector
/// - Files are processed in alphabetical order by filename
/// - All successfully parsed rules are merged in order
///
/// # Examples
///
/// Loading a directory of magic files:
///
/// ```rust,no_run
/// use libmagic_rs::parser::load_magic_directory;
/// use std::path::Path;
///
/// let rules = load_magic_directory(Path::new("/usr/share/file/magic.d"))?;
/// println!("Loaded {} rules from directory", rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// Creating a Magdir-style directory structure:
///
/// ```rust,no_run
/// use libmagic_rs::parser::load_magic_directory;
/// use std::path::Path;
///
/// // Directory structure:
/// // magic.d/
/// //   ├── 01-elf
/// //   ├── 02-archive
/// //   └── 03-text
///
/// let rules = load_magic_directory(Path::new("./magic.d"))?;
/// // Rules from all three files are merged in alphabetical order
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The directory does not exist or cannot be accessed
/// - Directory entries cannot be read
/// - A file cannot be read due to I/O errors
/// - A file contains invalid UTF-8 encoding
///
/// # Panics
///
/// This function does not panic under normal operation.
#[allow(clippy::print_stderr)]
pub fn load_magic_directory(dir_path: &Path) -> Result<Vec<MagicRule>, ParseError> {
    use std::fs;

    // Read directory entries
    let entries = fs::read_dir(dir_path).map_err(|e| {
        ParseError::invalid_syntax(
            0,
            format!("Failed to read directory '{}': {}", dir_path.display(), e),
        )
    })?;

    // Collect and sort entries by filename for deterministic ordering
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            ParseError::invalid_syntax(
                0,
                format!(
                    "Failed to read directory entry in '{}': {}",
                    dir_path.display(),
                    e
                ),
            )
        })?;

        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            ParseError::invalid_syntax(
                0,
                format!("Failed to read file type for '{}': {}", path.display(), e),
            )
        })?;

        // Only process regular files, skip directories and symlinks
        if file_type.is_file() && !file_type.is_symlink() {
            file_paths.push(path);
        }
    }

    // Sort by filename for deterministic ordering
    file_paths.sort_by_key(|path| path.file_name().map(std::ffi::OsStr::to_os_string));

    // Accumulate rules from all files
    let mut all_rules = Vec::new();
    let mut parse_failures: Vec<(PathBuf, ParseError)> = Vec::new();
    let file_count = file_paths.len();

    for path in file_paths {
        // Read file contents
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(e) => {
                // I/O errors are critical
                return Err(ParseError::invalid_syntax(
                    0,
                    format!("Failed to read file '{}': {}", path.display(), e),
                ));
            }
        };

        // Parse the file
        match parse_text_magic_file(&contents) {
            Ok(rules) => {
                // Successfully parsed - merge rules
                all_rules.extend(rules);
            }
            Err(e) => {
                // Track parse failures for reporting
                parse_failures.push((path, e));
            }
        }
    }

    // If all files failed to parse, return an error
    if all_rules.is_empty() && !parse_failures.is_empty() {
        use std::fmt::Write;

        let failure_details: Vec<String> = parse_failures
            .iter()
            .take(3) // Limit to first 3 failures for brevity
            .map(|(path, e)| format!("  - {}: {}", path.display(), e))
            .collect();

        let mut message = format!("All {file_count} magic file(s) in directory failed to parse");
        if !failure_details.is_empty() {
            message.push_str(":\n");
            message.push_str(&failure_details.join("\n"));
            if parse_failures.len() > 3 {
                let _ = write!(message, "\n  ... and {} more", parse_failures.len() - 3);
            }
        }

        return Err(ParseError::invalid_syntax(0, message));
    }

    // Log warnings for partial failures (some files parsed, some failed)
    // Note: Using eprintln for now; consider a logging framework in the future
    #[allow(clippy::print_stderr)]
    for (path, e) in &parse_failures {
        eprintln!("Warning: Failed to parse '{}': {}", path.display(), e);
    }

    Ok(all_rules)
}

/// Loads magic rules from a file or directory, automatically detecting the format.
///
/// This is the unified entry point for loading magic rules from the filesystem. It
/// automatically detects whether the path points to a text magic file, a directory
/// containing magic files, or a binary compiled magic file, and dispatches to the
/// appropriate handler.
///
/// # Format Detection and Handling
///
/// The function uses [`detect_format()`] to determine the file type and handles each
/// format as follows:
///
/// - **Text format**: Reads the file contents and parses using [`parse_text_magic_file()`]
/// - **Directory format**: Loads all magic files from the directory using [`load_magic_directory()`]
/// - **Binary format**: Returns an error with guidance to use the `--use-builtin` option
///
/// # Arguments
///
/// * `path` - Path to a magic file or directory. Can be absolute or relative.
///
/// # Returns
///
/// Returns `Ok(Vec<MagicRule>)` containing all successfully parsed magic rules. For
/// directories, rules from all files are merged in alphabetical order by filename.
///
/// # Errors
///
/// This function returns a [`ParseError`] in the following cases:
///
/// - **File not found**: The specified path does not exist
/// - **Unsupported format**: The file is a binary compiled magic file (`.mgc`)
/// - **Parse errors**: The magic file contains syntax errors or invalid rules
/// - **I/O errors**: File system errors during reading (permissions, disk errors, etc.)
///
/// # Examples
///
/// ## Loading a text magic file
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// let rules = load_magic_file(Path::new("/usr/share/misc/magic"))?;
/// println!("Loaded {} magic rules", rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// ## Loading a directory of magic files
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// let rules = load_magic_file(Path::new("/usr/share/misc/magic.d"))?;
/// println!("Loaded {} rules from directory", rules.len());
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// ## Handling binary format errors
///
/// ```no_run
/// use libmagic_rs::parser::load_magic_file;
/// use std::path::Path;
///
/// match load_magic_file(Path::new("/usr/share/misc/magic.mgc")) {
///     Ok(rules) => println!("Loaded {} rules", rules.len()),
///     Err(e) => {
///         eprintln!("Error loading magic file: {}", e);
///         eprintln!("Hint: Use --use-builtin for binary files");
///     }
/// }
/// # Ok::<(), libmagic_rs::ParseError>(())
/// ```
///
/// # Security
///
/// This function delegates to [`parse_text_magic_file()`] or [`load_magic_directory()`]
/// based on format detection. Security considerations are handled by those functions:
///
/// - Rule hierarchy depth is bounded during parsing
/// - Invalid syntax is rejected with descriptive errors
/// - Binary `.mgc` files are rejected (not parsed)
///
/// Note: File size limits and memory exhaustion protection are not currently implemented.
/// Large magic files will be loaded entirely into memory.
///
/// # See Also
///
/// - [`detect_format()`] - Format detection logic
/// - [`parse_text_magic_file()`] - Text file parser
/// - [`load_magic_directory()`] - Directory loader
pub fn load_magic_file(path: &Path) -> Result<Vec<MagicRule>, ParseError> {
    // Detect the magic file format
    let format = detect_format(path)?;

    // Dispatch to appropriate handler based on format
    match format {
        MagicFileFormat::Text => {
            // Read file contents and parse as text magic file
            let content = std::fs::read_to_string(path)?;
            parse_text_magic_file(&content)
        }
        MagicFileFormat::Directory => {
            // Load all magic files from directory
            load_magic_directory(path)
        }
        MagicFileFormat::Binary => {
            // Binary compiled magic files are not supported
            Err(ParseError::unsupported_format(
                0,
                "binary .mgc file",
                "Binary compiled magic files (.mgc) are not supported for parsing.\n\
                 Use the --use-builtin option to use the built-in magic rules instead,\n\
                 or provide a text-based magic file or directory.",
            ))
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn li(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: false,
            strength_modifier: None,
        }
    }

    fn li_comment(line_number: usize, content: &str) -> LineInfo {
        LineInfo {
            content: content.to_string(),
            line_number,
            is_comment: true,
            strength_modifier: None,
        }
    }

    // ============================================================
    // Tests for parse_magic_rule_line (10+ test cases)
    // ============================================================

    #[test]
    fn test_parse_magic_rule_line_simple_string() {
        let line = li(1, "0 string \\x7fELF ELF executable");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 0);
        assert_eq!(rule.message, "ELF executable");
    }

    #[test]
    fn test_parse_magic_rule_line_byte_type() {
        let line = li(1, "0 byte 1 ELF");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 0);
        assert!(matches!(rule.typ, TypeKind::Byte));
    }

    #[test]
    fn test_parse_magic_rule_line_with_child_indentation() {
        let line = li(2, ">4 byte 1 32-bit");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 1);
    }

    #[test]
    fn test_parse_magic_rule_line_deep_indentation() {
        let line = li(3, ">>>8 long = 0x12345678 Complex match");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.level, 3);
    }

    #[test]
    fn test_parse_magic_rule_line_not_equal_operator() {
        let line = li(1, "0 byte != 0 Non-zero");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::NotEqual);
    }

    #[test]
    fn test_parse_magic_rule_line_greater_operator() {
        let line = li(1, "0 long = 1000 Number");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::Equal);
    }

    #[test]
    fn test_parse_magic_rule_line_less_operator() {
        let line = li(1, "0 long != 256 Not equal");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::NotEqual);
    }

    #[test]
    fn test_parse_magic_rule_line_bitwise_and_operator() {
        let line = li(1, "0 byte & 0xFF Bitmask");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.op, Operator::BitwiseAnd);
    }

    #[test]
    fn test_parse_magic_rule_line_comment_line_error() {
        let line = li_comment(1, "This is a comment");
        let result = parse_magic_rule_line(&line);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_magic_rule_line_hex_offset() {
        let line = li(1, "0x100 byte 1 PDF document");
        let rule = parse_magic_rule_line(&line).unwrap();
        match rule.offset {
            OffsetSpec::Absolute(offset) => assert_eq!(offset, 0x100),
            _ => panic!("Expected absolute offset"),
        }
    }

    #[test]
    fn test_parse_magic_rule_line_string_with_spaces() {
        let line = li(1, "0 byte 1 Long message with multiple words");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert_eq!(rule.message, "Long message with multiple words");
    }

    #[test]
    fn test_parse_magic_rule_line_short_type() {
        let line = li(1, "0 short 0x4d5a MS-DOS executable");
        let rule = parse_magic_rule_line(&line).unwrap();
        assert!(matches!(rule.typ, TypeKind::Short { .. }));
    }

    // ============================================================
    // Tests for preprocess_lines (10+ test cases)
    // ============================================================

    #[test]
    fn test_preprocess_lines_single_rule() {
        let input = "0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test");
        assert!(!lines[0].is_comment);
    }

    #[test]
    fn test_preprocess_lines_multiple_rules() {
        let input = "0 string 0 Test\n0 byte 1 Byte";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].content, "0 string 0 Test");
        assert_eq!(lines[1].content, "0 byte 1 Byte");
    }

    #[test]
    fn test_preprocess_lines_with_comments() {
        let input = "# Comment\n0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].is_comment);
        assert!(!lines[1].is_comment);
    }

    #[test]
    fn test_preprocess_lines_empty_lines() {
        let input = "0 string 0 Test\n\n0 byte 1 Byte";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_preprocess_lines_leading_empty_lines() {
        let input = "\n\n0 string 0 Test";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test");
    }

    #[test]
    fn test_preprocess_lines_trailing_empty_lines() {
        let input = "0 string 0 Test\n\n";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_preprocess_lines_line_continuation() {
        let input = "0 string 0 Long message \\\ncontinued here";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Long message continued here");
    }

    #[test]
    fn test_preprocess_lines_multiple_continuations() {
        let input = "0 string 0 Multi \\\nline \\\ncontinuation";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Multi line continuation");
    }

    #[test]
    fn test_preprocess_lines_mixed_comments_and_rules() {
        let input = "# Header\n0 string 0 Test\n# Another comment\n>4 byte 1 Child";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].is_comment);
        assert!(!lines[1].is_comment);
        assert!(lines[2].is_comment);
        assert!(!lines[3].is_comment);
    }

    #[test]
    fn test_preprocess_lines_preserves_line_numbers() {
        let input = "0 string 0 Test\n>4 byte 1 Child";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[1].line_number, 2);
    }

    #[test]
    fn test_preprocess_lines_empty_input() {
        let input = "";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_preprocess_lines_only_comments() {
        let input = "# Comment 1\n# Comment 2\n# Comment 3";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.is_comment));
    }

    // ============================================================
    // Tests for build_rule_hierarchy (10+ test cases)
    // ============================================================

    #[test]
    fn test_build_rule_hierarchy_single_root() {
        let lines = vec![li(1, "0 string \\x7fELF ELF executable")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 0);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_one_child() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 1);
    }

    #[test]
    fn test_build_rule_hierarchy_root_with_multiple_children() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 32-bit"),
            li(3, ">4 byte 2 64-bit"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_nested_three_levels() {
        let lines = vec![
            li(1, "0 string \\x7fELF ELF executable"),
            li(2, ">4 byte 1 class"),
            li(3, ">>5 byte 1 subtype"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children.len(), 1);
        assert_eq!(roots[0].children[0].children[0].level, 2);
    }

    #[test]
    fn test_build_rule_hierarchy_multiple_roots() {
        let lines = vec![
            li(1, r#"0 string "ELF" "ELF executable""#),
            li(2, r#"0 string "%PDF" "PDF document""#),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_sibling_rules() {
        let lines = vec![
            li(1, "0 byte 1 Root"),
            li(2, ">4 byte 1 Child1"),
            li(3, ">4 byte 2 Child2"),
            li(4, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_rule_hierarchy_deep_nesting() {
        let lines = vec![
            li(1, "0 byte 1 L0"),
            li(2, ">4 byte 1 L1"),
            li(3, ">>5 byte 2 L2"),
            li(4, ">>>6 byte 3 L3"),
            li(5, ">>>>7 byte 4 L4"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(
            roots[0].children[0].children[0].children[0].children.len(),
            1
        );
    }

    #[test]
    fn test_build_rule_hierarchy_return_to_root_level() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 Child"),
            li(3, "0 byte 2 Root2"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[1].children.len(), 0);
    }

    #[test]
    fn test_build_rule_hierarchy_orphaned_child() {
        let lines = vec![li(1, ">4 byte 1 Orphaned child")];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].level, 1);
    }

    #[test]
    fn test_build_rule_hierarchy_complex_structure() {
        let lines = vec![
            li(1, "0 byte 1 Root1"),
            li(2, ">4 byte 1 C1"),
            li(3, ">4 byte 2 C2"),
            li(4, ">>6 byte 3 GC1"),
            li(5, "0 byte 2 Root2"),
            li(6, ">4 byte 4 C3"),
        ];
        let roots = build_rule_hierarchy(lines).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[0].children[1].children.len(), 1);
        assert_eq!(roots[1].children.len(), 1);
    }

    // ============================================================
    // Tests for parse_text_magic_file (10+ test cases)
    // ============================================================

    #[test]
    fn test_parse_text_magic_file_single_rule() {
        let input = "0 string 0 ZIP archive";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "ZIP archive");
    }

    #[test]
    fn test_parse_text_magic_file_hierarchical_rules() {
        let input = r"
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 2);
    }

    #[test]
    fn test_parse_text_magic_file_with_comments() {
        let input = r"
# ELF file format
0 string 0 ELF
>4 byte 1 32-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_multiple_roots() {
        let input = r"
0 byte 1 ELF
>4 byte 1 32-bit

0 byte 2 PDF
>5 byte 1 v1
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_parse_text_magic_file_empty_input() {
        let input = "";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_text_magic_file_only_comments() {
        let input = r"
# Comment 1
# Comment 2
# Comment 3
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_parse_text_magic_file_empty_lines_only() {
        let input = r"


0 string 0 Test file


";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_with_message_spaces() {
        let input = "0 string 0 Long message continued here";
        let rules = parse_text_magic_file(input).unwrap();
        assert!(rules[0].message.contains("continued"));
    }

    #[test]
    fn test_parse_text_magic_file_mixed_indentation() {
        let input = r"
0 byte 1 Root1
>4 byte 1 Child1
>4 byte 2 Child2
>>6 byte 3 Grandchild

0 byte 2 Root2
>4 byte 4 Child3
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].children.len(), 2);
        assert_eq!(rules[0].children[1].children.len(), 1);
        assert_eq!(rules[1].children.len(), 1);
    }

    #[test]
    fn test_parse_text_magic_file_complex_real_world() {
        let input = r"
# Magic file for common formats

# ELF binaries
0 byte 0x7f ELF executable
>4 byte 1 Intel 80386
>4 byte 2 x86-64
>>5 byte 1 LSB
>>5 byte 2 MSB

# PDF files
0 byte 0x25 PDF document
>5 byte 0x31 version 1.0
>5 byte 0x34 version 1.4
>5 byte 0x32 version 2.0
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "ELF executable");
        assert!(rules[0].children.len() > 1);
    }

    // ============================================================
    // Strength directive integration tests
    // ============================================================

    #[test]
    fn test_parse_text_magic_file_with_strength_directive() {
        let input = r"
!:strength +10
0 string \\x7fELF ELF executable
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Add(10)));
    }

    #[test]
    fn test_parse_text_magic_file_strength_applies_to_next_rule() {
        let input = r"
!:strength *2
0 string \\x7fELF ELF executable
0 string \\x50\\x4b ZIP archive
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        // Strength should only apply to the immediately following rule
        assert_eq!(
            rules[0].strength_modifier,
            Some(StrengthModifier::Multiply(2))
        );
        assert_eq!(rules[1].strength_modifier, None);
    }

    #[test]
    fn test_parse_text_magic_file_strength_with_child_rules() {
        let input = r"
!:strength =50
0 string \\x7fELF ELF executable
>4 byte 1 32-bit
>4 byte 2 64-bit
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
        // Strength applies to root rule
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Set(50)));
        // Children should not have strength modifier
        assert_eq!(rules[0].children[0].strength_modifier, None);
        assert_eq!(rules[0].children[1].strength_modifier, None);
    }

    #[test]
    fn test_parse_text_magic_file_multiple_strength_directives() {
        let input = r"
!:strength +10
0 string \\x7fELF ELF executable
!:strength -5
0 string \\x50\\x4b ZIP archive
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].strength_modifier, Some(StrengthModifier::Add(10)));
        assert_eq!(
            rules[1].strength_modifier,
            Some(StrengthModifier::Subtract(5))
        );
    }

    #[test]
    fn test_parse_text_magic_file_strength_all_operators() {
        let inputs = [
            ("!:strength +20\n0 byte 1 Test", StrengthModifier::Add(20)),
            (
                "!:strength -15\n0 byte 1 Test",
                StrengthModifier::Subtract(15),
            ),
            (
                "!:strength *3\n0 byte 1 Test",
                StrengthModifier::Multiply(3),
            ),
            ("!:strength /2\n0 byte 1 Test", StrengthModifier::Divide(2)),
            ("!:strength =100\n0 byte 1 Test", StrengthModifier::Set(100)),
            ("!:strength 50\n0 byte 1 Test", StrengthModifier::Set(50)),
        ];

        for (input, expected_modifier) in inputs {
            let rules = parse_text_magic_file(input).unwrap();
            assert_eq!(
                rules[0].strength_modifier,
                Some(expected_modifier),
                "Failed for input: {input}"
            );
        }
    }

    // ============================================================
    // Integration and edge case tests
    // ============================================================

    #[test]
    fn test_continuation_with_indentation() {
        let input = r">4 byte 1 Message \
continued";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_multiple_hex_offsets() {
        let input = r"
0x100 string 0 At 256
0x200 string 0 At 512
";
        let rules = parse_text_magic_file(input).unwrap();
        assert_eq!(rules.len(), 2);
    }

    // ============================================================
    // Overflow protection tests (from pr-test-analyzer)
    // ============================================================

    #[test]
    fn test_overflow_decimal_too_many_digits() {
        use crate::parser::grammar::parse_number;
        // Test exactly 20 digits (should fail - over i64 max)
        let result = parse_number("12345678901234567890");
        assert!(result.is_err(), "Should reject 20+ decimal digits");
    }

    #[test]
    fn test_overflow_hex_too_many_digits() {
        use crate::parser::grammar::parse_number;
        // Test 17 hex digits (should fail)
        let result = parse_number("0x10000000000000000");
        assert!(result.is_err(), "Should reject 17+ hex digits");
    }

    #[test]
    fn test_overflow_i64_max() {
        use crate::parser::grammar::parse_number;
        // i64::MAX = 9223372036854775807
        let result = parse_number("9223372036854775807");
        assert!(result.is_ok(), "Should accept i64::MAX");
    }

    #[test]
    fn test_overflow_i64_max_plus_one() {
        use crate::parser::grammar::parse_number;
        // i64::MAX + 1 should fail
        let result = parse_number("9223372036854775808");
        assert!(result.is_err(), "Should reject i64::MAX + 1");
    }

    // ============================================================
    // Continuation edge case tests (from pr-test-analyzer)
    // ============================================================

    #[test]
    fn test_continuation_at_eof() {
        // Continuation on last line with no following line - should error
        let input = "0 string 0 Test \\";
        let result = preprocess_lines(input);
        assert!(
            result.is_err(),
            "Should error on unterminated continuation at EOF"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{err:?}").contains("Unterminated"),
            "Error should mention unterminated continuation"
        );
    }

    #[test]
    fn test_continuation_with_empty_next() {
        // Empty line after continuation causes unterminated continuation
        // (empty lines are skipped but continuation state persists)
        let input = "0 string 0 Test \\\n\n0 byte 1 Next";
        let lines = preprocess_lines(input).unwrap();
        // The continuation carries through the empty line, so "Next" gets appended
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 Test 0 byte 1 Next");
    }

    #[test]
    fn test_continuation_into_empty_then_rule() {
        let input = "0 string 0 First \\\n\ncontinued";
        let lines = preprocess_lines(input).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "0 string 0 First continued");
    }

    // ============================================================
    // Line number accuracy tests (from pr-test-analyzer)
    // ============================================================

    #[test]
    fn test_line_numbers_with_continuations() {
        let input = "0 string 0 test1\n0 string 0 multi \\\nline \\\ntest\n0 string 0 test2";
        let lines = preprocess_lines(input).unwrap();

        // Line 1: "0 string 0 test1" should report line 1
        assert_eq!(lines[0].line_number, 1);

        // Line 2-4 continuation should report line 2 (first line of continuation)
        assert_eq!(lines[1].line_number, 2);

        // Line 5: "0 string 0 test2" should report line 5
        assert_eq!(lines[2].line_number, 5);
    }

    #[test]
    fn test_error_reports_correct_line_for_continuation() {
        // When a continued rule fails to parse, error should show the starting line
        let input = "0 string 0 valid\n0 invalid \\\nsyntax here\n0 string 0 valid2";
        let result = parse_text_magic_file(input);

        match result {
            Err(ref e) => {
                // Error should mention line 2 (start of the bad rule), not line 3
                let error_str = format!("{e:?}");
                assert!(
                    error_str.contains("line 2") || error_str.contains("line: 2"),
                    "Error should reference line 2, got: {error_str}"
                );
            }
            Ok(_) => panic!("Expected InvalidSyntax error"),
        }
    }

    #[test]
    fn test_line_numbers_with_mixed_content() {
        let input = "# Comment line 1\n0 string 0 rule1\n\n# Another comment\n0 string 0 rule2 \\\ncontinued";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].line_number, 1); // Comment
        assert_eq!(lines[1].line_number, 2); // rule1
        assert_eq!(lines[2].line_number, 4); // Another comment
        assert_eq!(lines[3].line_number, 5); // rule2 (continued on line 6)
    }

    // ============================================================
    // Bug reproduction tests
    // ============================================================

    #[test]
    fn test_bug1_comment_during_continuation() {
        // Bug 1: Comment during continuation should not corrupt line_buf
        // The partial rule should be discarded, leaving only the comment and new rule
        let input = "0 string 0 Partial rule \\\n# This is a comment\n0 byte 1 New rule";
        let lines = preprocess_lines(input).unwrap();

        // The partial rule is discarded, so we should have 2 lines: comment and new rule
        assert_eq!(lines.len(), 2);
        // The comment should be separate and not contain rule content
        let comment_line = lines.iter().find(|l| l.is_comment).unwrap();
        assert!(!comment_line.content.contains("Partial rule"));
        assert_eq!(comment_line.content, "This is a comment");
        // The new rule should be intact
        let rule_line = lines
            .iter()
            .find(|l| !l.is_comment && l.content.contains("New rule"))
            .unwrap();
        assert_eq!(rule_line.content, "0 byte 1 New rule");
    }

    #[test]
    fn test_bug2_empty_line_in_continuation() {
        // Bug 2: Empty line in continuation should not break line number calculation
        let input = "0 string 0 Test \\\n\ncontinued here";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 1);
        // Line number should point to line 1 (where the rule started), not line 3
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].content, "0 string 0 Test continued here");
    }

    #[test]
    fn test_bug2_multiple_empty_lines_in_continuation() {
        // Multiple empty lines in continuation
        let input = "0 string 0 Test \\\n\n\ncontinued here";
        let lines = preprocess_lines(input).unwrap();

        assert_eq!(lines.len(), 1);
        // Line number should still point to line 1
        assert_eq!(lines[0].line_number, 1);
    }
}

#[cfg(test)]
mod output_test {
    use crate::parser::{build_rule_hierarchy, parse_text_magic_file, preprocess_lines};

    #[test]
    fn demo_show_all_parser_outputs() {
        let input = r"
# ELF file
0 string 0 ELF
>4 byte 1 32-bit
>4 byte 2 64-bit

0 string 0 ZIP
>0 byte 3 zipped
";

        println!("\n================ RAW INPUT ================\n");
        println!("{input}");

        // --------------------------------------------------
        // 1. preprocess_lines
        // --------------------------------------------------
        println!("\n================ PREPROCESS LINES ================\n");

        let lines = preprocess_lines(input).expect("preprocess_lines failed");

        for (idx, line) in lines.iter().enumerate() {
            println!(
                "[{}] line_no={} is_comment={} content='{}'",
                idx, line.line_number, line.is_comment, line.content
            );
        }

        // --------------------------------------------------
        // 2. parse_text_magic_file (full pipeline)
        // --------------------------------------------------
        println!("\n================ PARSED MAGIC RULES ================\n");

        let rules = parse_text_magic_file(input).expect("parse_text_magic_file failed");

        for (i, rule) in rules.iter().enumerate() {
            println!("ROOT RULE [{i}]:");
            print_rule(rule, 1);
        }

        // --------------------------------------------------
        // 3. build_rule_hierarchy (explicit)
        // --------------------------------------------------
        println!("\n================ EXPLICIT HIERARCHY BUILD ================\n");

        let rebuilt = build_rule_hierarchy(lines).expect("build_rule_hierarchy failed");

        for (i, rule) in rebuilt.iter().enumerate() {
            println!("ROOT [{i}]:");
            print_rule(rule, 1);
        }
    }

    // Helper to pretty-print rule trees
    fn print_rule(rule: &crate::parser::MagicRule, indent: usize) {
        let pad = "  ".repeat(indent);

        println!(
            "{}- level={} offset={:?} type={:?} op={:?} value={:?} message='{}'",
            pad, rule.level, rule.offset, rule.typ, rule.op, rule.value, rule.message
        );

        for child in &rule.children {
            print_rule(child, indent + 1);
        }
    }
}

#[cfg(test)]
mod format_detection_tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_detect_format_text_file() {
        let temp_dir = std::env::temp_dir();
        let text_file = temp_dir.join("test_text_magic.txt");
        fs::write(&text_file, "# Magic file\n0 string test Test").unwrap();

        let format = detect_format(&text_file).unwrap();
        assert_eq!(format, MagicFileFormat::Text);

        fs::remove_file(&text_file).unwrap();
    }

    #[test]
    fn test_detect_format_directory() {
        let temp_dir = std::env::temp_dir().join("test_magic_dir");
        fs::create_dir_all(&temp_dir).unwrap();

        let format = detect_format(&temp_dir).unwrap();
        assert_eq!(format, MagicFileFormat::Directory);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_detect_format_binary_mgc() {
        let temp_dir = std::env::temp_dir();
        let binary_file = temp_dir.join("test_binary.mgc");

        // Write binary magic number 0xF11E041C in little-endian
        let mut file = fs::File::create(&binary_file).unwrap();
        file.write_all(&[0x1C, 0x04, 0x1E, 0xF1]).unwrap();
        file.write_all(b"additional binary data").unwrap();

        let result = detect_format(&binary_file);
        assert!(result.is_ok());

        match result.unwrap() {
            MagicFileFormat::Binary => {
                // Expected result
            }
            other => panic!("Expected Binary format, got {other:?}"),
        }

        fs::remove_file(&binary_file).unwrap();
    }

    #[test]
    fn test_detect_format_nonexistent_path() {
        let nonexistent = std::env::temp_dir().join("nonexistent_magic_file.txt");

        let result = detect_format(&nonexistent);
        assert!(result.is_err());

        match result.unwrap_err() {
            ParseError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("Expected IoError, got: {other:?}"),
        }
    }

    #[test]
    fn test_detect_format_empty_file() {
        let temp_dir = std::env::temp_dir();
        let empty_file = temp_dir.join("test_empty_magic.txt");
        fs::write(&empty_file, "").unwrap();

        // Empty files should be detected as text (too small for binary magic)
        let format = detect_format(&empty_file).unwrap();
        assert_eq!(format, MagicFileFormat::Text);

        fs::remove_file(&empty_file).unwrap();
    }

    #[test]
    fn test_detect_format_small_file() {
        let temp_dir = std::env::temp_dir();
        let small_file = temp_dir.join("test_small_magic.txt");
        fs::write(&small_file, "ab").unwrap(); // Only 2 bytes

        // Small files should be detected as text
        let format = detect_format(&small_file).unwrap();
        assert_eq!(format, MagicFileFormat::Text);

        fs::remove_file(&small_file).unwrap();
    }

    #[test]
    fn test_detect_format_text_with_binary_content() {
        let temp_dir = std::env::temp_dir();
        let binary_text_file = temp_dir.join("test_binary_text.txt");

        // Write binary data that's NOT the magic number
        let mut file = fs::File::create(&binary_text_file).unwrap();
        file.write_all(&[0xFF, 0xFE, 0xFD, 0xFC]).unwrap();
        file.write_all(b"some text").unwrap();

        // Should be detected as text (wrong magic number)
        let format = detect_format(&binary_text_file).unwrap();
        assert_eq!(format, MagicFileFormat::Text);

        fs::remove_file(&binary_text_file).unwrap();
    }

    #[test]
    fn test_magic_file_format_enum_equality() {
        assert_eq!(MagicFileFormat::Text, MagicFileFormat::Text);
        assert_eq!(MagicFileFormat::Directory, MagicFileFormat::Directory);
        assert_eq!(MagicFileFormat::Binary, MagicFileFormat::Binary);

        assert_ne!(MagicFileFormat::Text, MagicFileFormat::Directory);
        assert_ne!(MagicFileFormat::Text, MagicFileFormat::Binary);
        assert_ne!(MagicFileFormat::Directory, MagicFileFormat::Binary);
    }

    #[test]
    fn test_magic_file_format_debug() {
        let text_format = MagicFileFormat::Text;
        let debug_str = format!("{text_format:?}");
        assert!(debug_str.contains("Text"));
    }

    #[test]
    fn test_magic_file_format_clone() {
        let original = MagicFileFormat::Directory;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_magic_file_format_copy() {
        let original = MagicFileFormat::Binary;
        let copied = original; // Copy trait allows this
        assert_eq!(original, copied);
    }

    // ============================================================
    // Tests for load_magic_directory (6+ test cases)
    // ============================================================

    #[test]
    fn test_load_directory_critical_error_io() {
        use std::path::Path;

        let non_existent = Path::new("/this/should/not/exist/anywhere/at/all");
        let result = load_magic_directory(non_existent);

        assert!(
            result.is_err(),
            "Should return error for non-existent directory"
        );
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to read directory"));
    }

    #[test]
    fn test_load_directory_non_critical_error_parse() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a valid file
        let valid_path = temp_dir.path().join("valid.magic");
        fs::write(&valid_path, "0 string \\x01\\x02 valid\n").expect("Failed to write valid file");

        // Create an invalid file
        let invalid_path = temp_dir.path().join("invalid.magic");
        fs::write(&invalid_path, "this is invalid syntax\n").expect("Failed to write invalid file");

        // Should succeed, loading only the valid file
        let rules = load_magic_directory(temp_dir.path()).expect("Should load valid files");

        assert_eq!(rules.len(), 1, "Should load only valid file");
        assert_eq!(rules[0].message, "valid");
    }

    #[test]
    fn test_load_directory_empty_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create an empty file
        let empty_path = temp_dir.path().join("empty.magic");
        fs::write(&empty_path, "").expect("Failed to write empty file");

        // Create a file with only comments
        let comments_path = temp_dir.path().join("comments.magic");
        fs::write(&comments_path, "# Just comments\n# Nothing else\n")
            .expect("Failed to write comments file");

        // Should succeed with no rules
        let rules = load_magic_directory(temp_dir.path()).expect("Should handle empty files");

        assert_eq!(rules.len(), 0, "Empty files should contribute no rules");
    }

    #[test]
    fn test_load_directory_binary_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a binary file (invalid UTF-8)
        let binary_path = temp_dir.path().join("binary.dat");
        fs::write(&binary_path, [0xFF, 0xFE, 0xFF, 0xFE]).expect("Failed to write binary file");

        // Create a valid text file
        let valid_path = temp_dir.path().join("valid.magic");
        fs::write(&valid_path, "0 string \\x01\\x02 valid\n").expect("Failed to write valid file");

        // Binary file should cause a critical error (invalid UTF-8)
        let result = load_magic_directory(temp_dir.path());

        // The function should fail when encountering binary files (critical I/O error)
        assert!(
            result.is_err(),
            "Binary files should cause critical error due to invalid UTF-8"
        );
    }

    #[test]
    fn test_load_directory_mixed_extensions() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create files with different extensions
        fs::write(
            temp_dir.path().join("file.magic"),
            "0 string \\x01\\x02 magic\n",
        )
        .expect("Failed to write .magic file");
        fs::write(
            temp_dir.path().join("file.txt"),
            "0 string \\x03\\x04 txt\n",
        )
        .expect("Failed to write .txt file");
        fs::write(temp_dir.path().join("noext"), "0 string \\x05\\x06 noext\n")
            .expect("Failed to write no-ext file");

        let rules = load_magic_directory(temp_dir.path())
            .expect("Should load all files regardless of extension");

        assert_eq!(
            rules.len(),
            3,
            "Should process all files regardless of extension"
        );

        let messages: Vec<&str> = rules.iter().map(|r| r.message.as_str()).collect();
        assert!(messages.contains(&"magic"));
        assert!(messages.contains(&"txt"));
        assert!(messages.contains(&"noext"));
    }

    #[test]
    fn test_load_directory_alphabetical_ordering() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create files in non-alphabetical order - using valid magic syntax with hex escapes
        fs::write(
            temp_dir.path().join("03-third"),
            "0 string \\x07\\x08\\x09 third\n",
        )
        .expect("Failed to write third file");
        fs::write(
            temp_dir.path().join("01-first"),
            "0 string \\x01\\x02\\x03 first\n",
        )
        .expect("Failed to write first file");
        fs::write(
            temp_dir.path().join("02-second"),
            "0 string \\x04\\x05\\x06 second\n",
        )
        .expect("Failed to write second file");

        let rules = load_magic_directory(temp_dir.path()).expect("Should load directory in order");

        assert_eq!(rules.len(), 3);
        // Should be sorted alphabetically by filename
        assert_eq!(rules[0].message, "first");
        assert_eq!(rules[1].message, "second");
        assert_eq!(rules[2].message, "third");
    }

    // ============================================================
    // Tests for load_magic_file (5+ test cases)
    // ============================================================

    #[test]
    fn test_load_magic_file_text_format() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let magic_file = temp_dir.path().join("magic.txt");

        // Create text magic file with valid content
        fs::write(&magic_file, "0 string \\x7fELF ELF executable\n")
            .expect("Failed to write magic file");

        // Load using load_magic_file
        let rules = load_magic_file(&magic_file).expect("Failed to load text magic file");

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].message, "ELF executable");
    }

    #[test]
    fn test_load_magic_file_directory_format() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let magic_dir = temp_dir.path().join("magic.d");
        fs::create_dir(&magic_dir).expect("Failed to create magic directory");

        // Create multiple files in directory
        fs::write(
            magic_dir.join("00_elf"),
            "0 string \\x7fELF ELF executable\n",
        )
        .expect("Failed to write elf file");
        fs::write(
            magic_dir.join("01_zip"),
            "0 string \\x50\\x4b\\x03\\x04 ZIP archive\n",
        )
        .expect("Failed to write zip file");

        // Load using load_magic_file
        let rules = load_magic_file(&magic_dir).expect("Failed to load directory");

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].message, "ELF executable");
        assert_eq!(rules[1].message, "ZIP archive");
    }

    #[test]
    fn test_load_magic_file_binary_format_error() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let binary_file = temp_dir.path().join("magic.mgc");

        // Create binary file with .mgc magic number
        let mut file = File::create(&binary_file).expect("Failed to create binary file");
        let magic_number: [u8; 4] = [0x1C, 0x04, 0x1E, 0xF1]; // Little-endian 0xF11E041C
        file.write_all(&magic_number)
            .expect("Failed to write magic number");

        // Attempt to load binary file
        let result = load_magic_file(&binary_file);

        assert!(result.is_err(), "Should fail to load binary .mgc file");

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Verify error mentions unsupported format and --use-builtin
        assert!(
            error_msg.contains("Binary") || error_msg.contains("binary"),
            "Error should mention binary format: {error_msg}",
        );
        assert!(
            error_msg.contains("--use-builtin") || error_msg.contains("built-in"),
            "Error should mention --use-builtin option: {error_msg}",
        );
    }

    #[test]
    fn test_load_magic_file_io_error() {
        use std::path::Path;

        // Try to load non-existent file
        let non_existent = Path::new("/this/path/should/not/exist/magic.txt");
        let result = load_magic_file(non_existent);

        assert!(result.is_err(), "Should fail for non-existent file");
    }

    #[test]
    fn test_load_magic_file_parse_error_propagation() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let invalid_file = temp_dir.path().join("invalid.magic");

        // Create file with invalid syntax (missing offset)
        fs::write(&invalid_file, "string test invalid\n").expect("Failed to write invalid file");

        // Attempt to load file with parse errors
        let result = load_magic_file(&invalid_file);

        assert!(result.is_err(), "Should fail for file with parse errors");

        // Error should be a parse error (not I/O error)
        let error = result.unwrap_err();
        let error_msg = format!("{error:?}");
        assert!(
            error_msg.contains("InvalidSyntax") || error_msg.contains("syntax"),
            "Error should be parse error: {error_msg}",
        );
    }
}
