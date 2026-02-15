# Roadmap

See [GitHub Milestones](https://github.com/EvilBit-Labs/libmagic-rs/milestones) for detailed issue tracking.

## v0.1.0 - MVP (Current)

- [x] Core AST data structures with comprehensive serialization
- [x] Magic file parser for text format with hierarchical rules
- [x] Rule evaluation engine with confidence scoring
- [x] Memory-mapped file I/O with FileBuffer
- [x] Text and JSON output formatters
- [x] CLI with multiple file support, stdin, `--json`, and `--magic-file`
- [x] Built-in fallback rules with build-time compilation
- [x] Strength calculation and `!:strength` parsing
- [x] Comprehensive error handling, rustdoc, and mdbook documentation
- [x] 94%+ test coverage across 1,058+ tests

## v0.2.0 - Core Primitives

- [ ] Split evaluator module into focused submodules ([#59](https://github.com/EvilBit-Labs/libmagic-rs/issues/59))
- [ ] Disambiguate `MatchResult` types ([#60](https://github.com/EvilBit-Labs/libmagic-rs/issues/60))
- [ ] Extract CLI tests into integration tests ([#61](https://github.com/EvilBit-Labs/libmagic-rs/issues/61))
- [ ] Pre-create evaluator submodules for new features ([#62](https://github.com/EvilBit-Labs/libmagic-rs/issues/62))
- [ ] Comparison operators: `>`, `<`, `>=`, `<=` ([#34](https://github.com/EvilBit-Labs/libmagic-rs/issues/34))
- [ ] Bitwise XOR, NOT, and any-value operators ([#35](https://github.com/EvilBit-Labs/libmagic-rs/issues/35))
- [ ] Indirect offset resolution ([#37](https://github.com/EvilBit-Labs/libmagic-rs/issues/37))
- [ ] Relative offset resolution ([#38](https://github.com/EvilBit-Labs/libmagic-rs/issues/38))
- [ ] Quad (64-bit integer) type ([#36](https://github.com/EvilBit-Labs/libmagic-rs/issues/36))

## v0.3.0 - Advanced Features

- [ ] Convert `evaluator/types.rs` to directory module ([#63](https://github.com/EvilBit-Labs/libmagic-rs/issues/63))
- [ ] Regex and search types ([#39](https://github.com/EvilBit-Labs/libmagic-rs/issues/39))
- [ ] Float and double types ([#40](https://github.com/EvilBit-Labs/libmagic-rs/issues/40))
- [ ] Date and timestamp types ([#41](https://github.com/EvilBit-Labs/libmagic-rs/issues/41))
- [ ] Pascal string type ([#43](https://github.com/EvilBit-Labs/libmagic-rs/issues/43))
- [ ] Meta-types: default, clear, name, use, indirect ([#42](https://github.com/EvilBit-Labs/libmagic-rs/issues/42))

## v0.4.0 - API and UX Polish

- [ ] Builder pattern for `MagicDatabase` ([#45](https://github.com/EvilBit-Labs/libmagic-rs/issues/45))
- [ ] JSON output metadata ([#46](https://github.com/EvilBit-Labs/libmagic-rs/issues/46))
- [ ] Parse warnings for skipped rules ([#47](https://github.com/EvilBit-Labs/libmagic-rs/issues/47))
- [ ] Improved error messages ([#49](https://github.com/EvilBit-Labs/libmagic-rs/issues/49))
- [ ] Partial results on timeout ([#44](https://github.com/EvilBit-Labs/libmagic-rs/issues/44))

## v1.0.0 - Production Ready

- [ ] 95%+ compatibility with GNU `file` ([#48](https://github.com/EvilBit-Labs/libmagic-rs/issues/48), [#57](https://github.com/EvilBit-Labs/libmagic-rs/issues/57))
- [ ] Stable API with semver guarantees
- [ ] Migration guide from C libmagic
- [ ] Performance parity validation
- [ ] crates.io publication

## Non-Goals

The following are explicitly out of scope for this project:

- **Binary `.mgc` compilation**: We follow the OpenBSD approach of using text magic files directly, not the compiled binary format
- **Drop-in C ABI replacement**: This is a Rust-native library, not a C-compatible shared library with `libmagic.so` ABI
- **MIME database management**: We detect file types via magic rules, not by maintaining a MIME type registry
- **File modification or conversion**: This is a read-only detection tool
- **Network protocol detection**: We identify file contents, not network traffic
