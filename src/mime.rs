// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! MIME type mapping for file type detection
//!
//! This module provides MIME type mapping from file type descriptions
//! to standard MIME types. It includes hardcoded mappings for common
//! file types.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Static, process-wide mapping from lowercased keywords to MIME types.
///
/// Built once via [`LazyLock`] rather than per [`MimeMapper`] instance so that
/// constructing a [`MagicDatabase`](crate::MagicDatabase) does not pay the cost
/// of 40+ string allocations and [`HashMap`] inserts on every construction.
static MIME_MAPPINGS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let entries: &[(&str, &str)] = &[
        // Executables
        ("elf", "application/x-executable"),
        ("pe32", "application/vnd.microsoft.portable-executable"),
        ("pe32+", "application/vnd.microsoft.portable-executable"),
        ("mach-o", "application/x-mach-binary"),
        ("msdos", "application/x-dosexec"),
        // Archives
        ("zip", "application/zip"),
        ("gzip", "application/gzip"),
        ("tar", "application/x-tar"),
        ("rar", "application/vnd.rar"),
        ("7-zip", "application/x-7z-compressed"),
        ("bzip2", "application/x-bzip2"),
        ("xz", "application/x-xz"),
        // Images
        ("jpeg", "image/jpeg"),
        ("png", "image/png"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
        ("webp", "image/webp"),
        ("tiff", "image/tiff"),
        ("ico", "image/x-icon"),
        ("svg", "image/svg+xml"),
        // Documents
        ("pdf", "application/pdf"),
        ("postscript", "application/postscript"),
        // Audio/Video
        ("mp3", "audio/mpeg"),
        ("mpeg adts", "audio/mpeg"),
        ("mpeg audio", "audio/mpeg"),
        ("mp4", "video/mp4"),
        ("avi", "video/x-msvideo"),
        ("wav", "audio/wav"),
        ("ogg", "audio/ogg"),
        ("flac", "audio/flac"),
        ("webm", "video/webm"),
        // Web formats
        ("html", "text/html"),
        ("xml", "application/xml"),
        ("json", "application/json"),
        ("javascript", "text/javascript"),
        ("css", "text/css"),
        // Text
        ("ascii", "text/plain"),
        ("utf-8", "text/plain"),
        ("text", "text/plain"),
        // Office documents
        ("microsoft word", "application/msword"),
        ("microsoft excel", "application/vnd.ms-excel"),
        ("microsoft powerpoint", "application/vnd.ms-powerpoint"),
    ];

    let mut map = HashMap::with_capacity(entries.len());
    for &(k, v) in entries {
        map.insert(k, v);
    }
    map
});

/// MIME type mapper for converting file descriptions to MIME types
///
/// Provides case-insensitive matching of file type descriptions
/// to their corresponding MIME types.
///
/// Internally backed by a process-wide [`LazyLock`] table, so construction is
/// effectively free and the underlying mapping is shared across all instances.
///
/// # Examples
///
/// ```
/// use libmagic_rs::mime::MimeMapper;
///
/// let mapper = MimeMapper::new();
/// assert_eq!(mapper.get_mime_type("ELF 64-bit executable"), Some("application/x-executable".to_string()));
/// assert_eq!(mapper.get_mime_type("PNG image data"), Some("image/png".to_string()));
/// assert_eq!(mapper.get_mime_type("unknown format"), None);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct MimeMapper {
    _private: (),
}

impl MimeMapper {
    /// Create a new MIME mapper.
    ///
    /// This is a zero-cost constructor: all mappings are shared through a
    /// process-wide [`LazyLock`] table initialized on first use. Repeated calls
    /// to `new` do not rebuild the mapping.
    ///
    /// Includes mappings for common file types:
    /// - Executables (ELF, PE32, Mach-O)
    /// - Archives (ZIP, GZIP, TAR, RAR, 7Z)
    /// - Images (JPEG, PNG, GIF, BMP, WEBP, TIFF, ICO)
    /// - Documents (PDF, PostScript)
    /// - Audio/Video (MP3, MP4, AVI, WAV)
    /// - Web (HTML, XML, JSON, JavaScript, CSS)
    /// - Text formats
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Get MIME type for a file description
    ///
    /// Performs case-insensitive matching against known file type keywords.
    /// Returns the best matching MIME type found in the description,
    /// preferring longer (more specific) keyword matches.
    ///
    /// # Arguments
    ///
    /// * `description` - The file type description to match
    ///
    /// # Returns
    ///
    /// `Some(String)` with the MIME type if a match is found, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::mime::MimeMapper;
    ///
    /// let mapper = MimeMapper::new();
    ///
    /// // Case-insensitive matching
    /// assert_eq!(mapper.get_mime_type("ELF executable"), Some("application/x-executable".to_string()));
    /// assert_eq!(mapper.get_mime_type("elf executable"), Some("application/x-executable".to_string()));
    ///
    /// // Matches within longer descriptions
    /// assert_eq!(mapper.get_mime_type("PNG image data, 800x600"), Some("image/png".to_string()));
    /// ```
    #[must_use]
    pub fn get_mime_type(&self, description: &str) -> Option<String> {
        let lower = description.to_lowercase();

        // Find the longest matching keyword for best specificity
        // e.g., "gzip" should match before "zip" for "gzip compressed"
        let mut best_match: Option<(&'static str, &'static str)> = None;

        for (keyword, mime_type) in MIME_MAPPINGS.iter() {
            if lower.contains(*keyword) {
                match best_match {
                    Some((best_keyword, _)) if keyword.len() > best_keyword.len() => {
                        best_match = Some((*keyword, *mime_type));
                    }
                    None => {
                        best_match = Some((*keyword, *mime_type));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(_, mime_type)| mime_type.to_string())
    }

    /// Get the number of registered MIME mappings
    #[must_use]
    pub fn len(&self) -> usize {
        MIME_MAPPINGS.len()
    }

    /// Check if the mapper has no mappings
    #[must_use]
    pub fn is_empty(&self) -> bool {
        MIME_MAPPINGS.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_mapper_has_mappings() {
        let mapper = MimeMapper::new();
        assert!(!mapper.is_empty());
        assert!(mapper.len() > 20); // Should have many mappings
    }

    #[test]
    fn test_elf_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("ELF 64-bit LSB executable"),
            Some("application/x-executable".to_string())
        );
    }

    #[test]
    fn test_pe32_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("PE32 executable"),
            Some("application/vnd.microsoft.portable-executable".to_string())
        );
    }

    #[test]
    fn test_pe32_plus_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("PE32+ executable (DLL)"),
            Some("application/vnd.microsoft.portable-executable".to_string())
        );
    }

    #[test]
    fn test_zip_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("Zip archive data"),
            Some("application/zip".to_string())
        );
    }

    #[test]
    fn test_jpeg_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("JPEG image data, JFIF standard"),
            Some("image/jpeg".to_string())
        );
    }

    #[test]
    fn test_png_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("PNG image data, 800 x 600"),
            Some("image/png".to_string())
        );
    }

    #[test]
    fn test_gif_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("GIF image data, version 89a"),
            Some("image/gif".to_string())
        );
    }

    #[test]
    fn test_pdf_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("PDF document, version 1.4"),
            Some("application/pdf".to_string())
        );
    }

    #[test]
    fn test_case_insensitive() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("elf executable"),
            Some("application/x-executable".to_string())
        );
        assert_eq!(
            mapper.get_mime_type("ELF EXECUTABLE"),
            Some("application/x-executable".to_string())
        );
    }

    #[test]
    fn test_unknown_type_returns_none() {
        let mapper = MimeMapper::new();
        assert_eq!(mapper.get_mime_type("unknown binary format"), None);
        assert_eq!(mapper.get_mime_type("data"), None);
    }

    #[test]
    fn test_gzip_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("gzip compressed data"),
            Some("application/gzip".to_string())
        );
    }

    #[test]
    fn test_tar_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("POSIX tar archive"),
            Some("application/x-tar".to_string())
        );
    }

    #[test]
    fn test_html_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("HTML document"),
            Some("text/html".to_string())
        );
    }

    #[test]
    fn test_json_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("JSON data"),
            Some("application/json".to_string())
        );
    }

    #[test]
    fn test_mp3_mime_type() {
        let mapper = MimeMapper::new();
        assert_eq!(
            mapper.get_mime_type("Audio file with ID3 version 2.4.0, contains: MPEG ADTS, layer III, v1, 128 kbps, 44.1 kHz, JntStereo"),
            Some("audio/mpeg".to_string())
        );
    }

    #[test]
    fn test_default_trait() {
        let mapper = MimeMapper::default();
        assert!(!mapper.is_empty());
    }
}
