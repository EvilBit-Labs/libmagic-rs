# Evaluator Engine

> [!NOTE]
> The evaluator is currently in development. This documentation describes the planned implementation.

The evaluator engine executes magic rules against file buffers to identify file types. It's designed for safety, performance, and accuracy.

## Overview

The evaluator processes magic rules hierarchically:

1. **Load file** into memory-mapped buffer
2. **Resolve offsets** (absolute, indirect, relative)
3. **Read typed values** from buffer with bounds checking
4. **Apply operators** for comparison
5. **Collect results** and format output

## Architecture

```text
File Buffer → Offset Resolution → Type Reading → Operator Application → Results
     ↑              ↑                  ↑              ↑
Memory Map    Context State      Endian Handling   Match Logic
```

## Core Components (Planned)

### Offset Resolution (`evaluator/offset.rs`)

Handles all offset types safely:

- **Absolute offsets**: Direct file positions
- **Indirect offsets**: Pointer dereferencing with bounds checking
- **Relative offsets**: Based on previous match positions
- **From-end offsets**: Calculated from file size

### Type Reading (`evaluator/types.rs`)

Interprets bytes according to type specifications:

- **Numeric types**: Byte, short, long with endianness
- **String types**: Null-terminated with length limits
- **Binary data**: Raw byte sequences
- **Bounds checking**: Prevents buffer overruns

### Operator Application (`evaluator/operators.rs`)

Applies comparison operations:

- **Equality**: Exact value matching
- **Inequality**: Non-matching values
- **Bitwise AND**: Pattern matching for flags

### Evaluation Context

Maintains state during rule processing:

- **Current position**: For relative offsets
- **Recursion depth**: Prevents infinite loops
- **Match history**: For debugging and optimization

## Safety Features

### Memory Safety

- **Bounds checking**: All buffer access is validated
- **Integer overflow protection**: Safe arithmetic operations
- **Resource limits**: Prevent runaway evaluations

### Error Handling

- **Graceful degradation**: Skip problematic rules
- **Detailed errors**: Specific failure reasons
- **Recovery**: Continue evaluation after errors

## Performance Optimizations

### Lazy Evaluation

- **Parent-first**: Only evaluate children if parent matches
- **Early termination**: Stop on definitive matches
- **Rule ordering**: Most likely matches first

### Memory Efficiency

- **Memory mapping**: Avoid loading entire files
- **Zero-copy**: Minimize data copying
- **Efficient algorithms**: Optimized for common patterns

## Implementation Status

- [ ] Basic evaluation engine structure
- [ ] Offset resolution (absolute, indirect, relative)
- [ ] Type reading with endianness support
- [ ] Operator application
- [ ] Hierarchical rule processing
- [ ] Error handling and recovery
- [ ] Performance optimizations

## Planned API

```rust
pub fn evaluate_rules(rules: &[MagicRule], buffer: &[u8]) -> Result<Vec<Match>>;
pub fn evaluate_file<P: AsRef<Path>>(rules: &[MagicRule], path: P) -> Result<Vec<Match>>;
```
