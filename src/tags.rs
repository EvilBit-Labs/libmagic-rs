// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Tag extraction for file type classification
//!
//! This module extracts classification tags from file type descriptions
//! to enable categorization and filtering of detected file types.

use std::collections::HashSet;

/// Tag extractor for file type classification
///
/// Extracts classification tags from file descriptions based on
/// a configurable set of keywords.
///
/// # Examples
///
/// ```
/// use libmagic_rs::tags::TagExtractor;
///
/// let extractor = TagExtractor::new();
/// let tags = extractor.extract_tags("ELF 64-bit executable");
/// assert!(tags.contains(&"executable".to_string()));
///
/// let tags = extractor.extract_tags("Zip archive data, compressed");
/// assert!(tags.contains(&"archive".to_string()));
/// assert!(tags.contains(&"compressed".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct TagExtractor {
    /// Set of keywords to look for in descriptions
    keywords: HashSet<String>,
}

impl Default for TagExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TagExtractor {
    /// Create a new tag extractor with default keywords
    ///
    /// Default keywords include common file type classifications:
    /// - executable, archive, image, video, audio
    /// - document, compressed, encrypted, text, binary
    /// - data, script, font, database, spreadsheet
    #[must_use]
    pub fn new() -> Self {
        let keywords: HashSet<String> = [
            "executable",
            "archive",
            "image",
            "video",
            "audio",
            "document",
            "compressed",
            "encrypted",
            "text",
            "binary",
            "data",
            "script",
            "font",
            "database",
            "spreadsheet",
            "presentation",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();

        Self { keywords }
    }

    /// Create a tag extractor with custom keywords
    ///
    /// # Arguments
    ///
    /// * `keywords` - Iterator of keyword strings to use for tag extraction
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::tags::TagExtractor;
    ///
    /// let extractor = TagExtractor::with_keywords(vec!["custom", "tags"]);
    /// let tags = extractor.extract_tags("This has custom content");
    /// assert!(tags.contains(&"custom".to_string()));
    /// ```
    pub fn with_keywords<I, S>(keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let keywords = keywords
            .into_iter()
            .map(|s| s.into().to_lowercase())
            .collect();
        Self { keywords }
    }

    /// Extract tags from a file description
    ///
    /// Performs case-insensitive matching against the keyword set.
    /// Returns a sorted vector of unique matching tags.
    ///
    /// # Arguments
    ///
    /// * `description` - The file type description to extract tags from
    ///
    /// # Returns
    ///
    /// A vector of matching tag strings, sorted alphabetically.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::tags::TagExtractor;
    ///
    /// let extractor = TagExtractor::new();
    ///
    /// // Single tag extraction
    /// let tags = extractor.extract_tags("PNG image, 800x600");
    /// assert_eq!(tags, vec!["image".to_string()]);
    ///
    /// // Multiple tags
    /// let tags = extractor.extract_tags("Zip archive, encrypted and compressed");
    /// assert!(tags.contains(&"archive".to_string()));
    /// assert!(tags.contains(&"encrypted".to_string()));
    /// assert!(tags.contains(&"compressed".to_string()));
    /// ```
    #[must_use]
    pub fn extract_tags(&self, description: &str) -> Vec<String> {
        let lower = description.to_lowercase();

        let mut tags: Vec<String> = self
            .keywords
            .iter()
            .filter(|keyword| lower.contains(keyword.as_str()))
            .cloned()
            .collect();

        tags.sort();
        tags
    }

    /// Extract rule path tags from match messages
    ///
    /// Normalizes match messages into tag-like identifiers by:
    /// - Converting to lowercase
    /// - Replacing spaces with hyphens
    /// - Removing special characters
    ///
    /// # Arguments
    ///
    /// * `messages` - Iterator of match messages to convert
    ///
    /// # Returns
    ///
    /// A vector of normalized tag strings.
    ///
    /// # Examples
    ///
    /// ```
    /// use libmagic_rs::tags::TagExtractor;
    ///
    /// let extractor = TagExtractor::new();
    /// let messages = vec!["ELF magic", "64-bit LSB", "executable"];
    /// let tags = extractor.extract_rule_path(messages.iter().map(|s| *s));
    /// assert_eq!(tags, vec!["elf-magic", "64-bit-lsb", "executable"]);
    /// ```
    pub fn extract_rule_path<'a, I>(&self, messages: I) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        messages
            .into_iter()
            .map(|msg| {
                msg.to_lowercase()
                    .replace(' ', "-")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-')
                    .collect()
            })
            .collect()
    }

    /// Get the number of configured keywords
    #[must_use]
    pub fn keyword_count(&self) -> usize {
        self.keywords.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_extractor_has_keywords() {
        let extractor = TagExtractor::new();
        assert!(extractor.keyword_count() > 10);
    }

    #[test]
    fn test_extract_executable_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("ELF 64-bit executable");
        assert!(tags.contains(&"executable".to_string()));
    }

    #[test]
    fn test_extract_image_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("PNG image data, 800x600");
        assert!(tags.contains(&"image".to_string()));
    }

    #[test]
    fn test_extract_archive_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("Zip archive data");
        assert!(tags.contains(&"archive".to_string()));
    }

    #[test]
    fn test_extract_multiple_tags() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("Zip archive, encrypted and compressed");
        assert!(tags.contains(&"archive".to_string()));
        assert!(tags.contains(&"encrypted".to_string()));
        assert!(tags.contains(&"compressed".to_string()));
    }

    #[test]
    fn test_case_insensitive() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("EXECUTABLE file");
        assert!(tags.contains(&"executable".to_string()));
    }

    #[test]
    fn test_no_tags_found() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("unknown format");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_tags_are_sorted() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("compressed archive with encrypted data");
        assert_eq!(
            tags,
            vec![
                "archive".to_string(),
                "compressed".to_string(),
                "data".to_string(),
                "encrypted".to_string()
            ]
        );
    }

    #[test]
    fn test_custom_keywords() {
        let extractor = TagExtractor::with_keywords(vec!["custom", "special"]);
        let tags = extractor.extract_tags("This is a custom file with special content");
        assert!(tags.contains(&"custom".to_string()));
        assert!(tags.contains(&"special".to_string()));
        assert!(!tags.contains(&"executable".to_string())); // Not in custom set
    }

    #[test]
    fn test_with_keywords_lowercases_input() {
        // Keywords should be lowercased for case-insensitive matching
        let extractor = TagExtractor::with_keywords(vec!["Executable", "ARCHIVE"]);
        // Should match lowercase version in description
        let tags = extractor.extract_tags("executable file in archive");
        assert!(tags.contains(&"executable".to_string()));
        assert!(tags.contains(&"archive".to_string()));
    }

    #[test]
    fn test_extract_rule_path() {
        let extractor = TagExtractor::new();
        let messages = ["ELF magic", "64-bit LSB", "executable"];
        let tags = extractor.extract_rule_path(messages.iter().copied());
        assert_eq!(tags, vec!["elf-magic", "64-bit-lsb", "executable"]);
    }

    #[test]
    fn test_extract_rule_path_removes_special_chars() {
        let extractor = TagExtractor::new();
        let messages = ["File (version 1.0)", "Data: test!"];
        let tags = extractor.extract_rule_path(messages.iter().copied());
        assert_eq!(tags, vec!["file-version-10", "data-test"]);
    }

    #[test]
    fn test_default_trait() {
        let extractor = TagExtractor::default();
        assert!(extractor.keyword_count() > 0);
    }

    #[test]
    fn test_video_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("MPEG video stream");
        assert!(tags.contains(&"video".to_string()));
    }

    #[test]
    fn test_audio_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("FLAC audio bitstream data");
        assert!(tags.contains(&"audio".to_string()));
    }

    #[test]
    fn test_document_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("PDF document, version 1.4");
        assert!(tags.contains(&"document".to_string()));
    }

    #[test]
    fn test_script_tag() {
        let extractor = TagExtractor::new();
        let tags = extractor.extract_tags("Python script, ASCII text executable");
        assert!(tags.contains(&"script".to_string()));
        assert!(tags.contains(&"text".to_string()));
        assert!(tags.contains(&"executable".to_string()));
    }
}
