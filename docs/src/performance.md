# Performance Optimization

libmagic-rs includes several performance optimizations across I/O, evaluation, and compilation. This chapter describes each optimization and how to take advantage of them.

## Memory-Mapped I/O

The `FileBuffer` type in `src/io/mod.rs` uses the `memmap2` crate to memory-map files instead of reading them entirely into memory. This provides:

- **Demand paging**: The OS loads only the pages that are actually accessed during evaluation, avoiding unnecessary reads for large files.
- **Zero-copy access**: `FileBuffer::as_slice()` returns a `&[u8]` directly backed by the memory mapping with no intermediate copy.
- **OS page cache reuse**: Repeated analysis of the same file reuses cached pages without additional I/O.

Files up to 1 GB are supported. Empty files and non-regular files (devices, FIFOs, directories) are rejected at construction time with descriptive errors.

## SIMD-Accelerated Null Byte Scanning

String type reading in `src/evaluator/types.rs` uses the `memchr` crate to locate null terminators. The `memchr` crate automatically uses SIMD instructions (SSE2/AVX2 on x86-64, NEON on aarch64) when available, making null-terminated string extraction significantly faster than a byte-by-byte loop.

```rust
// From src/evaluator/types.rs - uses SIMD-accelerated memchr for null scan
let read_length = memchr::memchr(0, &remaining_buffer[..search_len])
    .unwrap_or(search_len);
```

## Evaluation Pipeline Optimizations

### Unified Offset and Value Resolution

The internal `evaluate_single_rule_with_anchor` worker resolves the offset, reads the typed value, and applies the operator in a single pass, returning `Option<(usize, Value)>`. Callers receive the resolved offset and matched value directly, avoiding redundant re-resolution when constructing match results. The public `evaluate_single_rule` now delegates to `evaluate_rules` so safety limits (timeout, recursion, max string length) are enforced consistently across all entry points.

### Pre-Allocated Collections

Several hot paths pre-allocate collections with known or estimated capacities:

- `evaluate_rules` pre-allocates the match results vector with `Vec::with_capacity(8)`.
- `concatenate_messages` computes the total capacity from match message lengths and allocates the output string once with `String::with_capacity`.
- Hex encoding in the JSON output formatter pre-allocates based on byte count.

### Early Exit on First Match

When `EvaluationConfig::stop_at_first_match` is `true` (the default), the evaluator stops iterating rules as soon as the first successful match is found. This avoids evaluating the remaining rule set when only one result is needed.

### Timeout Support

Each evaluation tracks elapsed time from a `std::time::Instant` created at the start. If `timeout_ms` is set in the configuration, the evaluator checks the elapsed time during rule iteration and returns a timeout error if the limit is exceeded. This prevents runaway evaluations on adversarial or unusually large inputs.

## Static Tag Extraction Patterns

The `DEFAULT_TAG_EXTRACTOR` in `src/output/mod.rs` is a `static LazyLock<TagExtractor>` initialized once on first use. This avoids constructing the keyword set on every call to `from_evaluator_match` or `from_library_result`.

```rust
static DEFAULT_TAG_EXTRACTOR: LazyLock<crate::tags::TagExtractor> =
    LazyLock::new(crate::tags::TagExtractor::new);
```

## Release Profile Optimization

The `Cargo.toml` release profile enables link-time optimization and single-codegen-unit compilation:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
```

- **Thin LTO** allows cross-crate inlining while keeping link times reasonable.
- **Single codegen unit** gives LLVM a complete view of the crate for better optimization at the cost of longer compile times.

## Configuration Presets

`EvaluationConfig` provides named presets that trade off between speed and completeness:

| Preset            | Recursion Depth | String Length | Stop First | MIME Types | Timeout |
| ----------------- | --------------: | ------------: | :--------: | :--------: | ------: |
| `default()`       |              20 |          8192 |    yes     |     no     |    none |
| `performance()`   |              10 |          1024 |    yes     |     no     |      1s |
| `comprehensive()` |              50 |         32768 |     no     |    yes     |     30s |

Use the performance preset when throughput matters more than detail:

```rust
use libmagic_rs::{MagicDatabase, EvaluationConfig};

let db = MagicDatabase::with_builtin_rules_and_config(
    EvaluationConfig::performance()
)?;
let result = db.identify_file("target.bin")?;
```

## Benchmarking

The project uses [Criterion](https://docs.rs/criterion) for benchmarks. The benchmark suite in `benches/evaluation_bench.rs` covers three areas:

1. **File type detection** -- ELF, ZIP, PDF, and unknown data detection throughput.
2. **Buffer sizes** -- Evaluation time across buffer sizes from 64 bytes to 64 KB.
3. **Configuration comparison** -- Default vs. performance vs. comprehensive presets.

Run the full benchmark suite:

```bash
cargo bench
```

Run a specific benchmark group:

```bash
cargo bench -- file_type_detection
cargo bench -- buffer_sizes
cargo bench -- evaluation_configs
```

Criterion generates HTML reports in `target/criterion/` with statistical analysis and comparison against previous runs.

## Profiling

Generate a CPU flamegraph to identify hot spots:

```bash
cargo install flamegraph
cargo flamegraph --bench evaluation_bench -- --bench
```

The resulting `flamegraph.svg` shows where CPU time is spent during evaluation, making it straightforward to identify optimization targets.

Use `cargo bench` regularly during development to catch performance regressions. Criterion's built-in comparison mode highlights statistically significant changes between runs.
