# Evaluation Enhancements: Confidence, MIME, Tags, Metadata

## Overview

Enhance evaluation results with confidence scoring, MIME type mapping, tag extraction, and comprehensive metadata. This provides rich output for JSON format and improves programmatic usage of the library.

## Validation Findings (Implementation Validation)

**Edge Cases in Existing Code to Address:**

1. **EDGE CASE: No builder pattern API** (`file:src/lib.rs`)

   - Current: Only `load_from_file()` exists
   - Required: `MagicDatabase::new().with_config(config)?.load(path)`
   - Impact: Advanced users cannot customize configuration before loading
   - Fix: Implement builder pattern with `new()`, `with_config()`, and `load()` methods

2. **EDGE CASE: No evaluate_buffer() method** (`file:src/lib.rs`)

   - Current: Only `evaluate_file()` exists
   - Required: Both `evaluate_file(path)` and `evaluate_buffer(buffer)` per Core Flow 6
   - Impact: Library users cannot evaluate in-memory buffers
   - Fix: Add `evaluate_buffer(&[u8])` method

3. **EDGE CASE: Confidence always 1.0 or 0.0** (`file:src/lib.rs` lines 440-441)

   - Current: Hardcoded `confidence: 1.0` with TODO comment
   - Required: Depth-based calculation `min(1.0, 0.3 + (depth * 0.2))`
   - Impact: JSON output shows meaningless confidence scores
   - Fix: Calculate confidence based on match depth in hierarchy

4. **EDGE CASE: MIME type always None** (`file:src/lib.rs` line 439)

   - Current: Hardcoded `mime_type: None` with TODO comment
   - Required: Hardcoded mappings + optional system MIME database loading
   - Impact: JSON output missing MIME type information
   - Fix: Implement MIME mapper with hardcoded fallbacks

5. **EDGE CASE: Missing EvaluationResult fields** (`file:src/lib.rs` lines 446-455)

   - Current: Only `description`, `mime_type`, `confidence`
   - Required: Add `matches: Vec<MatchResult>` and `metadata: EvaluationMetadata`
   - Impact: JSON output cannot show match details or metadata
   - Fix: Add missing fields to structure

## Scope

**In Scope:**

- Confidence scoring based on match depth
- MIME type mapping (hardcoded + optional file loading)
- Tag extraction from descriptions
- Evaluation metadata (timing, rules evaluated, file size)
- Enhanced `EvaluationResult` structure
- Enhanced `MatchResult` structure
- Builder pattern API for `MagicDatabase`

**Out of Scope:**

- Strength calculation (separate ticket)
- Advanced MIME database parsing
- Machine learning-based confidence
- Performance optimization

## Technical Approach

### 1. Enhanced Data Structures

Update `file:src/lib.rs`:

```rust
pub struct EvaluationResult {
    pub description: String,        // Concatenated hierarchical message (libmagic behavior)
    pub mime_type: Option<String>,
    pub confidence: f64,
    pub matches: Vec<MatchResult>,  // Individual match entries for each level
    pub metadata: EvaluationMetadata,
}

pub struct EvaluationMetadata {
    pub file_size: u64,
    pub evaluation_time_ms: f64,
    pub rules_evaluated: usize,
    pub magic_file: Option<PathBuf>,  // Path to magic file, None for built-in rules
    pub timed_out: bool,
}
```

Update `file:src/evaluator/mod.rs`:

```rust
pub struct MatchResult {
    pub offset: usize,
    pub value: Vec<u8>,
    pub message: String,
    pub level: usize,
    pub confidence: f64,  // NEW
}

impl MatchResult {
    fn calculate_confidence(depth: usize) -> f64 {
        (0.3 + (depth as f64 * 0.2)).min(1.0)
    }
}
```

### 2. MIME Mapper

Create `file:src/mime.rs`:

```rust
pub struct MimeMapper {
    mappings: HashMap<String, String>,
}

impl MimeMapper {
    pub fn new() -> Self {
        let mut mapper = Self::with_hardcoded_mappings();

        // Try to load system MIME database (optional)
        for path in ["/usr/share/file/magic.mime", "/usr/local/share/misc/magic.mime"] {
            if let Ok(mime_db) = Self::load_mime_database(path) {
                mapper.merge(mime_db);
                break;
            }
        }

        mapper
    }

    pub fn get_mime_type(&self, description: &str) -> Option<String> {
        // Try exact match, then prefix matching
    }

    fn with_hardcoded_mappings() -> Self {
        // ELF, PE, ZIP, JPEG, PNG, PDF, GIF mappings
    }
}
```

### 3. Tag Extractor

Create `file:src/tags.rs`:

```rust
pub struct TagExtractor {
    keywords: HashSet<String>,
}

impl TagExtractor {
    pub fn new() -> Self {
        let keywords = vec![
            "executable", "archive", "image", "video", "audio",
            "document", "compressed", "encrypted", "text", "binary",
        ].into_iter().map(String::from).collect();

        Self { keywords }
    }

    pub fn extract_tags(&self, description: &str) -> Vec<String> {
        let lower = description.to_lowercase();
        self.keywords.iter()
            .filter(|k| lower.contains(k.as_str()))
            .cloned()
            .collect()
    }

    pub fn extract_rule_path(&self, matches: &[MatchResult]) -> Vec<String> {
        // Normalize messages to lowercase identifiers
        matches.iter()
            .map(|m| m.message.to_lowercase().replace(' ', "-"))
            .collect()
    }
}
```

### 4. Builder Pattern API

Update `file:src/lib.rs`:

```rust
impl MagicDatabase {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            config: EvaluationConfig::default(),
        }
    }

    pub fn with_config(mut self, config: EvaluationConfig) -> Result<Self> {
        config.validate()?;
        self.config = config;
        Ok(self)
    }

    pub fn load<P: AsRef<Path>>(mut self, path: P) -> Result<Self> {
        self.rules = parser::load_magic_file(path)?;
        Ok(self)
    }

    // Convenience method
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::new().load(path)
    }
}
```

### 5. Enhanced Evaluation

Update `file:src/lib.rs`:

```rust
pub fn evaluate_file<P: AsRef<Path>>(&self, path: P) -> Result<EvaluationResult> {
    let start_time = Instant::now();
    let file_buffer = FileBuffer::new(path.as_ref())?;
    let buffer = file_buffer.as_slice();

    let matches = evaluate_rules_with_config(&self.rules, buffer, self.config.clone())?;

    // Concatenate hierarchical messages (libmagic behavior)
    let description = if matches.is_empty() {
        "data".to_string()
    } else {
        concatenate_messages(&matches)
    };

    fn concatenate_messages(matches: &[MatchResult]) -> String {
        let mut result = String::new();
        for m in matches {
            if !result.is_empty() && !m.message.starts_with('\u{0008}') {
                result.push(' ');
            }
            if m.message.starts_with('\u{0008}') {
                result.push_str(&m.message[1..]);
            } else {
                result.push_str(&m.message);
            }
        }
        result
    }

    let confidence = matches.first()
        .map(|m| m.confidence)
        .unwrap_or(0.0);

    let mime_type = if self.config.enable_mime_types {
        MimeMapper::new().get_mime_type(&description)
    } else {
        None
    };

    Ok(EvaluationResult {
        description,
        mime_type,
        confidence,
        matches,
        metadata: EvaluationMetadata {
            file_size: buffer.len() as u64,
            evaluation_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
            rules_evaluated: self.rules.len(),
            timed_out: false,
        },
    })
}
```

### 6. JSON Output Enhancement

Update `file:src/main.rs`:

```rust
fn output_json(filename: &str, result: &EvaluationResult) -> Result<()> {
    let tags = TagExtractor::new().extract_tags(&result.description);

    let json = serde_json::json!({
        "filename": filename,
        "matches": result.matches.iter().map(|m| {
            serde_json::json!({
                "text": m.message,
                "offset": m.offset,
                "value": hex::encode(&m.value),
                "score": (m.confidence * 100.0) as u32,
                "mime_type": result.mime_type,
            })
        }).collect::<Vec<_>>(),
        "metadata": {
            "file_size": result.metadata.file_size,
            "evaluation_time_ms": result.metadata.evaluation_time_ms,
            "rules_evaluated": result.metadata.rules_evaluated,
        }
    });

    println!("{}", serde_json::to_string(&json)?);
    Ok(())
}
```

## Acceptance Criteria

- [ ] Confidence calculated based on match depth
- [ ] MIME types mapped for common file types
- [ ] MIME database loaded if available (optional)
- [ ] Tags extracted from descriptions
- [ ] Evaluation metadata includes timing and counts
- [ ] Builder pattern API works: `MagicDatabase::new().with_config(config)?.load(path)`
- [ ] `load_from_file()` convenience method works
- [ ] JSON output includes all metadata fields
- [ ] metadata.magic_file populated correctly (Some for loaded files, None for built-in)
- [ ] description field concatenates hierarchical messages (libmagic behavior)
- [ ] Backspace (\\b) in messages suppresses space
- [ ] rule_path derived from messages (normalized to lowercase)
- [ ] Rustdoc added for all new structures and methods
- [ ] Unit tests for confidence calculation
- [ ] Unit tests for MIME mapper
- [ ] Unit tests for tag extractor
- [ ] Unit tests for builder pattern
- [ ] Integration test with full JSON output

## Dependencies

- Depends on: ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/c554e409-ae60-407f-9596-64c5b03a9b92 (Parser Integration)

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - MIME Mapper, Confidence Scoring, Tag Extraction)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/36539700-862d-4fdf-9c79-3c36390f6aa8 (Core Flows - Flow 5, 8)

## Files to Create

- `file:src/mime.rs` - MIME mapper module
- `file:src/tags.rs` - Tag extractor module

## Files to Modify

- `file:src/lib.rs` - Enhanced structures, builder pattern, evaluation logic
- `file:src/evaluator/mod.rs` - Add confidence to MatchResult
- `file:src/main.rs` - Enhanced JSON output
