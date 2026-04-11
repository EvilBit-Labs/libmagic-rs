# libmagic-rs fuzzing harness

Continuous fuzzing targets for libmagic-rs, built with [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) / [`libfuzzer-sys`](https://github.com/rust-fuzz/libfuzzer-sys).

This crate is **excluded from the parent workspace** because `libfuzzer-sys` requires a nightly toolchain and links against LLVM's libFuzzer runtime. `cargo check --workspace` and `cargo nextest run` do not build this crate; use the commands below from the `fuzz/` directory.

## Targets

| Target                  | Focus                                                                                                                      |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `parse_text_magic_file` | Fuzzes the text magic-file parser; verifies `parse_text_magic_file` never panics on arbitrary input.                       |
| `evaluate_rules_buffer` | Fuzzes `MagicDatabase::evaluate_buffer` with the built-in rule set; verifies no panic and a 1-second timeout holds.        |
| `regex_pattern_compile` | Fuzzes `TypeKind::Regex` with attacker-controlled pattern strings; pins the `REGEX_COMPILE_SIZE_LIMIT` compile-time bound. |

## Running

```sh
# Install cargo-fuzz once per machine
cargo install --locked cargo-fuzz

# From the fuzz/ directory, run a target for 10 minutes
cd fuzz
cargo +nightly fuzz run parse_text_magic_file -- -max_total_time=600
cargo +nightly fuzz run evaluate_rules_buffer -- -max_total_time=600
cargo +nightly fuzz run regex_pattern_compile -- -max_total_time=600
```

Crashes reproduce under `fuzz/artifacts/<target>/`. Minimize them with:

```sh
cargo +nightly fuzz tmin <target> <crash-file>
```

## CI integration

These targets are run on a nightly cron in `.github/workflows/` (tracked as a follow-up to review finding D-H1 on the operational side). To run locally in short sessions (60 seconds each), use the `just fuzz` recipe — see `justfile` for the wiring.

## Adding a new target

1. Create `fuzz_targets/<new_target>.rs` following the existing shape.
2. Add a `[[bin]]` entry in `fuzz/Cargo.toml`.
3. Verify it builds: `cargo +nightly fuzz build <new_target>`.

## Why a separate crate?

Cargo-fuzz targets are `#![no_main]` libFuzzer shims that link against the libFuzzer runtime. They require nightly Rust and cannot coexist with the main workspace's stable toolchain policy. Putting them in a separate crate under `fuzz/` (a cargo-fuzz convention) keeps the stable developer workflow unaffected while still giving the project a permanent fuzzing harness tied to the library code.
