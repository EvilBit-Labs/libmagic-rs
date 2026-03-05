# Configuration

libmagic-rs provides a single configuration struct, `EvaluationConfig`, that controls how magic rules are evaluated. All fields have safe defaults, and the `validate()` method enforces security bounds before any evaluation begins.

## EvaluationConfig

```rust
use libmagic_rs::EvaluationConfig;

pub struct EvaluationConfig {
    pub max_recursion_depth: u32,       // Default: 20, max: 1000
    pub max_string_length: usize,       // Default: 8192, max: 1MB (1_048_576)
    pub stop_at_first_match: bool,      // Default: true
    pub enable_mime_types: bool,         // Default: false
    pub timeout_ms: Option<u64>,        // Default: None, max: 300_000 (5 min)
}
```

### Field Reference

| Field                 | Type          | Default | Bounds         | Purpose                                                       |
| --------------------- | ------------- | ------- | -------------- | ------------------------------------------------------------- |
| `max_recursion_depth` | `u32`         | 20      | 1 -- 1000      | Limits nested rule traversal depth to prevent stack overflow  |
| `max_string_length`   | `usize`       | 8192    | 1 -- 1_048_576 | Caps bytes read for string types to prevent memory exhaustion |
| `stop_at_first_match` | `bool`        | `true`  | --             | When true, evaluation stops after the first matching rule     |
| `enable_mime_types`   | `bool`        | `false` | --             | When true, maps file type descriptions to standard MIME types |
| `timeout_ms`          | `Option<u64>` | `None`  | 1 -- 300_000   | Per-file evaluation timeout in milliseconds; `None` disables  |

## Constructor Presets

### `EvaluationConfig::new()` / `EvaluationConfig::default()`

Returns balanced defaults suitable for most workloads:

```rust
let config = EvaluationConfig::new();

assert_eq!(config.max_recursion_depth, 20);
assert_eq!(config.max_string_length, 8192);
assert!(config.stop_at_first_match);
assert!(!config.enable_mime_types);
assert_eq!(config.timeout_ms, None);
```

### `EvaluationConfig::performance()`

Optimized for high-throughput scenarios where speed matters more than completeness:

```rust
let config = EvaluationConfig::performance();

assert_eq!(config.max_recursion_depth, 10);
assert_eq!(config.max_string_length, 1024);
assert!(config.stop_at_first_match);
assert!(!config.enable_mime_types);
assert_eq!(config.timeout_ms, Some(1000)); // 1 second
```

### `EvaluationConfig::comprehensive()`

Finds all matches with deep analysis, MIME type mapping, and a generous timeout:

```rust
let config = EvaluationConfig::comprehensive();

assert_eq!(config.max_recursion_depth, 50);
assert_eq!(config.max_string_length, 32768);
assert!(!config.stop_at_first_match);
assert!(config.enable_mime_types);
assert_eq!(config.timeout_ms, Some(30000)); // 30 seconds
```

## Custom Configuration

Use struct update syntax to override individual fields from any preset:

```rust
use libmagic_rs::EvaluationConfig;

let config = EvaluationConfig {
    max_recursion_depth: 30,
    enable_mime_types: true,
    timeout_ms: Some(5000),
    ..EvaluationConfig::default()
};
```

## Validation

Call `validate()` to check that all values fall within safe bounds. The `MagicDatabase` constructors call `validate()` automatically, so you only need to call it explicitly when creating a config that will be stored for later use.

```rust
use libmagic_rs::EvaluationConfig;

let config = EvaluationConfig::default();
assert!(config.validate().is_ok());

let bad = EvaluationConfig {
    max_recursion_depth: 0,
    ..EvaluationConfig::default()
};
assert!(bad.validate().is_err());
```

### Validation Rules

The `validate()` method enforces four categories of security constraints:

**Recursion depth** -- must be between 1 and 1000. A value of 0 is rejected because evaluation cannot proceed without at least one level. Values above 1000 risk stack overflow.

**String length** -- must be between 1 and 1_048_576 (1 MB). A value of 0 is rejected because no string matching could occur. Values above 1 MB risk memory exhaustion.

**Timeout** -- if `Some`, must be between 1 and 300_000 (5 minutes). A value of 0 is rejected as meaningless. Values above 5 minutes risk denial-of-service. `None` (no timeout) is always accepted.

**Resource combination** -- a recursion depth above 100 combined with a string length above 65_536 is rejected. Deep recursion with large string reads at every level can compound into excessive resource consumption even when each value individually falls within safe bounds.

## Using Configuration with MagicDatabase

### Built-in Rules with Default Config

```rust
use libmagic_rs::MagicDatabase;

let db = MagicDatabase::with_builtin_rules()?;
let result = db.evaluate_buffer(b"\x7fELF")?;
```

### Built-in Rules with Custom Config

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig {
    timeout_ms: Some(5000),
    ..EvaluationConfig::default()
};
let db = MagicDatabase::with_builtin_rules_and_config(config)?;
```

### Magic File with Custom Config

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig::performance();
let db = MagicDatabase::load_from_file_with_config("custom.magic", config)?;
```

All three constructors call `config.validate()` internally and return an error if the configuration is invalid. There is no way to create a `MagicDatabase` with an invalid configuration.

## CLI Usage

The command-line interface exposes the `timeout_ms` field via the `--timeout-ms` flag. All other configuration values use their defaults when running from the CLI.

```bash
# No timeout (default)
libmagic-rs sample.bin

# 5-second timeout per file
libmagic-rs --timeout-ms 5000 sample.bin
```

If evaluation exceeds the timeout, the file is skipped and an error message is printed to stderr with exit code 5.

## Choosing a Preset

| Scenario                    | Preset               | Why                                      |
| --------------------------- | -------------------- | ---------------------------------------- |
| General file identification | `default()`          | Balanced depth and limits                |
| Batch processing many files | `performance()`      | Low limits, 1s timeout, early exit       |
| Forensic analysis           | `comprehensive()`    | Deep traversal, all matches, MIME types  |
| Untrusted input             | `performance()`      | Tight bounds reduce attack surface       |
| Custom requirements         | Struct update syntax | Override specific fields from any preset |
