# Configuration

> [!NOTE]
> Configuration options are planned for future releases. This documentation describes the intended configuration system.

libmagic-rs provides flexible configuration options for customizing behavior, performance, and output formatting.

## EvaluationConfig

The main configuration structure for rule evaluation:

```rust
pub struct EvaluationConfig {
    /// Maximum recursion depth for nested rules
    pub max_recursion_depth: u32,

    /// Maximum string length to read
    pub max_string_length: usize,

    /// Stop at first match or continue for all matches
    pub stop_at_first_match: bool,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            max_recursion_depth: 20,
            max_string_length: 8192,
            stop_at_first_match: true,
        }
    }
}
```

## Usage Examples

### Basic Configuration

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let config = EvaluationConfig {
    max_recursion_depth: 10,
    max_string_length: 1024,
    stop_at_first_match: true,
};

let db = MagicDatabase::load_from_file("magic.db")?;
let result = db.evaluate_file_with_config("example.bin", &config)?;
```

### Performance-Optimized Configuration

```rust
// Fast evaluation for large-scale processing
let fast_config = EvaluationConfig {
    max_recursion_depth: 5,      // Shallow nesting
    max_string_length: 256,      // Short strings only
    stop_at_first_match: true,   // Exit early
};
```

### Comprehensive Analysis Configuration

```rust
// Thorough analysis for detailed results
let thorough_config = EvaluationConfig {
    max_recursion_depth: 50,     // Deep nesting allowed
    max_string_length: 16384,    // Long strings
    stop_at_first_match: false,  // Find all matches
};
```

## Configuration Sources (Planned)

### Environment Variables

```bash
export LIBMAGIC_RS_MAX_RECURSION=15
export LIBMAGIC_RS_MAX_STRING_LENGTH=4096
export LIBMAGIC_RS_STOP_AT_FIRST_MATCH=true
```

### Configuration Files

TOML configuration file support:

```toml
# ~/.config/libmagic-rs/config.toml
[evaluation]
max_recursion_depth = 25
max_string_length = 8192
stop_at_first_match = true

[performance]
enable_caching = true
cache_size_mb = 64

[output]
default_format = "text"
include_confidence = false
```

### Builder Pattern

```rust
let config = EvaluationConfig::builder()
    .max_recursion_depth(15)
    .max_string_length(2048)
    .stop_at_first_match(false)
    .build();
```

## Advanced Configuration (Planned)

### Cache Configuration

```rust
pub struct CacheConfig {
    pub enable_rule_caching: bool,
    pub enable_result_caching: bool,
    pub max_cache_size_mb: usize,
    pub cache_ttl_seconds: u64,
}
```

### Output Configuration

```rust
pub struct OutputConfig {
    pub format: OutputFormat,
    pub include_confidence: bool,
    pub include_mime_type: bool,
    pub include_metadata: bool,
}

pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}
```

### Security Configuration

```rust
pub struct SecurityConfig {
    pub max_file_size_mb: usize,
    pub allow_indirect_offsets: bool,
    pub max_evaluation_time_ms: u64,
}
```

## Configuration Validation

```rust
impl EvaluationConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_recursion_depth == 0 {
            return Err(ConfigError::InvalidValue(
                "max_recursion_depth must be greater than 0".to_string(),
            ));
        }

        if self.max_string_length > 1_000_000 {
            return Err(ConfigError::InvalidValue(
                "max_string_length too large (max 1MB)".to_string(),
            ));
        }

        Ok(())
    }
}
```

## Configuration Precedence (Planned)

1. **Explicit parameters**: Direct function arguments
2. **Environment variables**: Runtime environment settings
3. **Configuration files**: User and system config files
4. **Default values**: Built-in defaults

## Best Practices

### Performance Tuning

```rust
// For high-throughput scenarios
let performance_config = EvaluationConfig {
    max_recursion_depth: 5,
    max_string_length: 512,
    stop_at_first_match: true,
};

// For detailed analysis
let analysis_config = EvaluationConfig {
    max_recursion_depth: 30,
    max_string_length: 8192,
    stop_at_first_match: false,
};
```

### Security Considerations

```rust
// For untrusted files
let secure_config = EvaluationConfig {
    max_recursion_depth: 10,     // Prevent deep recursion attacks
    max_string_length: 1024,     // Limit memory usage
    stop_at_first_match: true,   // Minimize processing time
};
```

### Memory Management

```rust
// For memory-constrained environments
let minimal_config = EvaluationConfig {
    max_recursion_depth: 3,
    max_string_length: 256,
    stop_at_first_match: true,
};
```

This configuration system provides flexibility while maintaining safe defaults and preventing resource exhaustion attacks.
