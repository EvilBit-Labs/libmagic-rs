// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! MIME type mapping for file type detection
//!
//! This module provides MIME type mapping from file type descriptions
//! to standard MIME types. It includes hardcoded mappings for common
//! file types.

use std::collections::HashMap;

/// MIME type mapper for converting file descriptions to MIME types
///
/// Provides case-insensitive matching of file type descriptions
/// to their corresponding MIME types.
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
#[derive(Debug, Clone)]
pub struct MimeMapper {
    /// Mapping from description keywords to MIME types
    mappings: HashMap<String, String>,
}

impl Default for MimeMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl MimeMapper {
    /// Create a new MIME mapper with hardcoded mappings
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
        let mut mappings = HashMap::new();

        // Executables
        mappings.insert("elf".to_string(), "application/x-executable".to_string());
        mappings.insert(
            "pe32".to_string(),
            "application/vnd.microsoft.portable-executable".to_string(),
        );
        mappings.insert(
            "pe32+".to_string(),
            "application/vnd.microsoft.portable-executable".to_string(),
        );
        mappings.insert(
            "mach-o".to_string(),
            "application/x-mach-binary".to_string(),
        );
        mappings.insert("msdos".to_string(), "application/x-dosexec".to_string());

        // Archives
        mappings.insert("zip".to_string(), "application/zip".to_string());
        mappings.insert("gzip".to_string(), "application/gzip".to_string());
        mappings.insert("tar".to_string(), "application/x-tar".to_string());
        mappings.insert("rar".to_string(), "application/vnd.rar".to_string());
        mappings.insert(
            "7-zip".to_string(),
            "application/x-7z-compressed".to_string(),
        );
        mappings.insert("bzip2".to_string(), "application/x-bzip2".to_string());
        mappings.insert("xz".to_string(), "application/x-xz".to_string());

        // Images
        mappings.insert("jpeg".to_string(), "image/jpeg".to_string());
        mappings.insert("png".to_string(), "image/png".to_string());
        mappings.insert("gif".to_string(), "image/gif".to_string());
        mappings.insert("bmp".to_string(), "image/bmp".to_string());
        mappings.insert("webp".to_string(), "image/webp".to_string());
        mappings.insert("tiff".to_string(), "image/tiff".to_string());
        mappings.insert("ico".to_string(), "image/x-icon".to_string());
        mappings.insert("svg".to_string(), "image/svg+xml".to_string());

        // Documents
        mappings.insert("pdf".to_string(), "application/pdf".to_string());
        mappings.insert(
            "postscript".to_string(),
            "application/postscript".to_string(),
        );

        // Audio/Video
        mappings.insert("mp3".to_string(), "audio/mpeg".to_string());
        mappings.insert("mpeg adts".to_string(), "audio/mpeg".to_string());
        mappings.insert("mpeg audio".to_string(), "audio/mpeg".to_string());
        mappings.insert("mp4".to_string(), "video/mp4".to_string());
        mappings.insert("avi".to_string(), "video/x-msvideo".to_string());
        mappings.insert("wav".to_string(), "audio/wav".to_string());
        mappings.insert("ogg".to_string(), "audio/ogg".to_string());
        mappings.insert("flac".to_string(), "audio/flac".to_string());
        mappings.insert("webm".to_string(), "video/webm".to_string());

        // Web formats
        mappings.insert("html".to_string(), "text/html".to_string());
        mappings.insert("xml".to_string(), "application/xml".to_string());
        mappings.insert("json".to_string(), "application/json".to_string());
        mappings.insert("javascript".to_string(), "text/javascript".to_string());
        mappings.insert("css".to_string(), "text/css".to_string());

        // Text
        mappings.insert("ascii".to_string(), "text/plain".to_string());
        mappings.insert("utf-8".to_string(), "text/plain".to_string());
        mappings.insert("text".to_string(), "text/plain".to_string());

        // Office documents
        mappings.insert(
            "microsoft word".to_string(),
            "application/msword".to_string(),
        );
        mappings.insert(
            "microsoft excel".to_string(),
            "application/vnd.ms-excel".to_string(),
        );
        mappings.insert(
            "microsoft powerpoint".to_string(),
            "application/vnd.ms-powerpoint".to_string(),
        );

        Self { mappings }
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
        let mut best_match: Option<(&String, &String)> = None;

        for (keyword, mime_type) in &self.mappings {
            if lower.contains(keyword.as_str()) {
                match best_match {
                    Some((best_keyword, _)) if keyword.len() > best_keyword.len() => {
                        best_match = Some((keyword, mime_type));
                    }
                    None => {
                        best_match = Some((keyword, mime_type));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(_, mime_type)| mime_type.clone())
    }

    /// Get the number of registered MIME mappings
    #[must_use]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Check if the mapper has no mappings
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
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
