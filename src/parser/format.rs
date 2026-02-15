// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Format detection for magic files.
//!
//! Detects whether a path points to a text magic file, a directory of magic files,
//! or a binary compiled magic file (.mgc format).

use crate::error::ParseError;
use std::io::Read;
use std::path::Path;

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
/// 1. Check if path is a directory -> `MagicFileFormat::Directory`
/// 2. Read first 4 bytes and check for binary magic number `0xF11E041C` -> `MagicFileFormat::Binary`
/// 3. Otherwise -> `MagicFileFormat::Text`
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
        Err(e) => Err(ParseError::IoError(e)),
    }
}

#[cfg(test)]
mod tests {
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
}
