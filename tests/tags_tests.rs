//! Tag extraction integration tests
//!
//! Tests for keyword extraction, case insensitivity, rule path tags,
//! and custom keyword support.

use libmagic_rs::tags::TagExtractor;

// ============================================================
// Keyword Extraction
// ============================================================

#[test]
fn test_extract_single_keyword() {
    let extractor = TagExtractor::new();
    let tags = extractor.extract_tags("ELF 64-bit executable");
    assert!(tags.contains(&"executable".to_string()));
}

#[test]
fn test_extract_multiple_keywords() {
    let extractor = TagExtractor::new();
    let tags = extractor.extract_tags("compressed archive with encrypted data");
    assert!(tags.contains(&"archive".to_string()));
    assert!(tags.contains(&"compressed".to_string()));
    assert!(tags.contains(&"encrypted".to_string()));
    assert!(tags.contains(&"data".to_string()));
}

#[test]
fn test_extract_all_default_keywords() {
    let extractor = TagExtractor::new();

    let keyword_descriptions = [
        ("ELF executable", "executable"),
        ("Zip archive", "archive"),
        ("PNG image data", "image"),
        ("MPEG video stream", "video"),
        ("FLAC audio bitstream", "audio"),
        ("PDF document", "document"),
        ("gzip compressed", "compressed"),
        ("AES encrypted", "encrypted"),
        ("ASCII text", "text"),
        ("binary data", "binary"),
        ("raw data", "data"),
        ("Python script", "script"),
        ("TrueType font", "font"),
        ("SQLite database", "database"),
        ("Excel spreadsheet", "spreadsheet"),
    ];

    for (description, expected_tag) in &keyword_descriptions {
        let tags = extractor.extract_tags(description);
        assert!(
            tags.contains(&expected_tag.to_string()),
            "Expected tag '{}' from description '{}'",
            expected_tag,
            description
        );
    }
}

#[test]
fn test_no_match_returns_empty() {
    let extractor = TagExtractor::new();
    let tags = extractor.extract_tags("unknown format xyz");
    assert!(tags.is_empty());
}

#[test]
fn test_tags_are_sorted() {
    let extractor = TagExtractor::new();
    let tags = extractor.extract_tags("compressed archive with encrypted data");
    let mut sorted = tags.clone();
    sorted.sort();
    assert_eq!(tags, sorted);
}

// ============================================================
// Case Insensitivity
// ============================================================

#[test]
fn test_case_insensitive_matching() {
    let extractor = TagExtractor::new();
    assert_eq!(
        extractor.extract_tags("EXECUTABLE file"),
        extractor.extract_tags("executable file")
    );
}

#[test]
fn test_mixed_case_matching() {
    let extractor = TagExtractor::new();
    let tags = extractor.extract_tags("Executable Archive Compressed");
    assert!(tags.contains(&"executable".to_string()));
    assert!(tags.contains(&"archive".to_string()));
    assert!(tags.contains(&"compressed".to_string()));
}

// ============================================================
// Custom Keywords
// ============================================================

#[test]
fn test_custom_keywords() {
    let extractor = TagExtractor::with_keywords(vec!["custom", "special"]);
    let tags = extractor.extract_tags("This has custom and special content");
    assert!(tags.contains(&"custom".to_string()));
    assert!(tags.contains(&"special".to_string()));
    assert!(!tags.contains(&"executable".to_string()));
}

#[test]
fn test_custom_keywords_case_normalized() {
    let extractor = TagExtractor::with_keywords(vec!["UPPER", "MiXeD"]);
    let tags = extractor.extract_tags("upper and mixed content");
    assert!(tags.contains(&"upper".to_string()));
    assert!(tags.contains(&"mixed".to_string()));
}

#[test]
fn test_keyword_count() {
    let extractor = TagExtractor::new();
    assert!(
        extractor.keyword_count() >= 15,
        "Default extractor should have at least 15 keywords"
    );

    let custom = TagExtractor::with_keywords(vec!["a", "b", "c"]);
    assert_eq!(custom.keyword_count(), 3);
}

// ============================================================
// Rule Path Tags
// ============================================================

#[test]
fn test_extract_rule_path_basic() {
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
fn test_extract_rule_path_empty() {
    let extractor = TagExtractor::new();
    let messages: Vec<&str> = vec![];
    let tags = extractor.extract_rule_path(messages.iter().copied());
    assert!(tags.is_empty());
}
