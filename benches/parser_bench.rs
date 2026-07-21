// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser benchmarks for libmagic-rs
//!
//! Benchmarks magic file parsing performance including:
//! - Single line parsing
//! - Multi-line rule parsing
//! - Built-in rules loading
//! - Directory loading

// Test/bench code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::unreadable_literal
)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use libmagic_rs::MagicDatabase;
use std::hint::black_box;

/// Benchmark loading built-in magic rules
fn bench_builtin_rules_loading(c: &mut Criterion) {
    c.bench_function("load_builtin_rules", |b| {
        b.iter(|| {
            let db = MagicDatabase::with_builtin_rules().expect("should load");
            black_box(db)
        });
    });
}

/// Benchmark loading magic file from disk
fn bench_magic_file_loading(c: &mut Criterion) {
    let magic_path = "src/builtin_rules.magic";

    // Skip if file doesn't exist
    if !std::path::Path::new(magic_path).exists() {
        return;
    }

    let mut group = c.benchmark_group("magic_file_loading");

    // Measure throughput based on file size
    let file_size = std::fs::metadata(magic_path).map_or(0, |m| m.len());
    group.throughput(Throughput::Bytes(file_size));

    group.bench_function("load_builtin_magic_file", |b| {
        b.iter(|| {
            let db = MagicDatabase::load_from_file(black_box(magic_path)).expect("should load");
            black_box(db)
        });
    });

    group.finish();
}

/// Benchmark parsing individual magic rule patterns
fn bench_rule_parsing(c: &mut Criterion) {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let mut group = c.benchmark_group("rule_parsing");

    // Simple string match rule
    let simple_rule = "0 string \"test\" Test file\n";
    group.bench_function("parse_simple_string_rule", |b| {
        let mut temp = NamedTempFile::new().expect("temp file");
        temp.write_all(simple_rule.as_bytes()).expect("write temp");
        let path = temp.path().to_owned();

        b.iter(|| {
            let db = MagicDatabase::load_from_file(black_box(&path)).expect("should load");
            black_box(db)
        });
    });

    // Numeric rule with endianness
    let numeric_rule = "0 belong 0x7f454c46 ELF executable\n";
    group.bench_function("parse_numeric_rule", |b| {
        let mut temp = NamedTempFile::new().expect("temp file");
        temp.write_all(numeric_rule.as_bytes()).expect("write temp");
        let path = temp.path().to_owned();

        b.iter(|| {
            let db = MagicDatabase::load_from_file(black_box(&path)).expect("should load");
            black_box(db)
        });
    });

    // Complex nested rule
    let nested_rule = r"0 string \x7fELF ELF
>4 byte 1 32-bit
>4 byte 2 64-bit
>5 byte 1 LSB
>5 byte 2 MSB
";
    group.bench_function("parse_nested_rules", |b| {
        let mut temp = NamedTempFile::new().expect("temp file");
        temp.write_all(nested_rule.as_bytes()).expect("write temp");
        let path = temp.path().to_owned();

        b.iter(|| {
            let db = MagicDatabase::load_from_file(black_box(&path)).expect("should load");
            black_box(db)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_builtin_rules_loading,
    bench_magic_file_loading,
    bench_rule_parsing
);
criterion_main!(benches);
