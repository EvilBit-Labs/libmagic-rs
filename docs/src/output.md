# Output Formatters

> **Note**: Output formatters are currently in development. This documentation describes the planned implementation.

The output module handles formatting evaluation results into different output formats for various use cases.

## Supported Formats

### Text Output
Human-readable format compatible with GNU `file`:
```text
example.bin: ELF 64-bit LSB executable, x86-64, version 1 (SYSV)
```

### JSON Output
Structured format for programmatic use:
```json
{
  "filename": "example.bin",
  "description": "ELF 64-bit LSB executable, x86-64, version 1 (SYSV)",
  "mime_type": "application/x-executable",
  "confidence": 0.95
}
```

## Implementation Status

- [ ] Text formatter (`output/text.rs`)
- [ ] JSON formatter (`output/json.rs`)
- [ ] Format selection logic
- [ ] MIME type mapping

## Planned API

```rust
pub fn format_text(results: &[Match]) -> String;
pub fn format_json(results: &[Match]) -> Result<String>;
```
