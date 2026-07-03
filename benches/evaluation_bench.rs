// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Evaluation benchmarks for libmagic-rs
//!
//! Benchmarks rule evaluation performance against various file types:
//! - ELF binary detection
//! - ZIP archive detection
//! - PDF document detection
//! - Unknown data handling

// Bench code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::unreadable_literal,
    clippy::cast_possible_truncation,
    clippy::semicolon_outside_block,
    clippy::semicolon_inside_block
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use libmagic_rs::{EvaluationConfig, MagicDatabase};
use std::hint::black_box;
use std::io::Write;

/// Create a minimal ELF 64-bit header for testing
fn create_elf64_header() -> Vec<u8> {
    let mut header = vec![0u8; 64];

    // ELF magic number
    header[0] = 0x7f;
    header[1] = b'E';
    header[2] = b'L';
    header[3] = b'F';

    // 64-bit
    header[4] = 2;

    // Little endian
    header[5] = 1;

    // ELF version
    header[6] = 1;

    // System V ABI
    header[7] = 0;

    // Type: executable
    header[16] = 2;

    // Machine: x86-64
    header[18] = 0x3e;

    // ELF version
    header[20] = 1;

    header
}

/// Create a minimal ZIP header for testing
fn create_zip_header() -> Vec<u8> {
    vec![
        0x50, 0x4b, 0x03, 0x04, // PK signature
        0x14, 0x00, // Version needed
        0x00, 0x00, // General purpose flags
        0x08, 0x00, // Compression method (deflate)
        0x00, 0x00, // Last mod time
        0x00, 0x00, // Last mod date
        0x00, 0x00, 0x00, 0x00, // CRC-32
        0x00, 0x00, 0x00, 0x00, // Compressed size
        0x00, 0x00, 0x00, 0x00, // Uncompressed size
        0x04, 0x00, // Filename length
        0x00, 0x00, // Extra field length
        b't', b'e', b's', b't', // Filename
    ]
}

/// Create a minimal PDF header for testing
fn create_pdf_header() -> Vec<u8> {
    b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec()
}

/// Create random unknown data for testing fallback behavior
fn create_unknown_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Benchmark evaluation of different file types
fn bench_file_type_detection(c: &mut Criterion) {
    let db = MagicDatabase::with_builtin_rules().expect("should load");

    let mut group = c.benchmark_group("file_type_detection");

    // ELF detection
    let elf_data = create_elf64_header();
    group.throughput(Throughput::Bytes(elf_data.len() as u64));
    group.bench_function("detect_elf", |b| {
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&elf_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    // ZIP detection
    let zip_data = create_zip_header();
    group.throughput(Throughput::Bytes(zip_data.len() as u64));
    group.bench_function("detect_zip", |b| {
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&zip_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    // PDF detection
    let pdf_data = create_pdf_header();
    group.throughput(Throughput::Bytes(pdf_data.len() as u64));
    group.bench_function("detect_pdf", |b| {
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&pdf_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    // Unknown data (fallback to "data")
    let unknown_data = create_unknown_data(64);
    group.throughput(Throughput::Bytes(unknown_data.len() as u64));
    group.bench_function("detect_unknown", |b| {
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&unknown_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark evaluation with different buffer sizes
fn bench_buffer_sizes(c: &mut Criterion) {
    let db = MagicDatabase::with_builtin_rules().expect("should load");

    let mut group = c.benchmark_group("buffer_sizes");

    // Test with various buffer sizes
    for size in [64, 256, 1024, 4096, 16384, 65536] {
        let data = create_unknown_data(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("unknown", size), &data, |b, data| {
            b.iter(|| {
                let result = db
                    .evaluate_buffer(black_box(data))
                    .expect("should evaluate");
                black_box(result)
            });
        });
    }

    group.finish();
}

/// Benchmark evaluation with different configurations
fn bench_evaluation_configs(c: &mut Criterion) {
    let elf_data = create_elf64_header();

    let mut group = c.benchmark_group("evaluation_configs");
    group.throughput(Throughput::Bytes(elf_data.len() as u64));

    // Default configuration
    group.bench_function("config_default", |b| {
        let db = MagicDatabase::with_builtin_rules_and_config(EvaluationConfig::default())
            .expect("should load");
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&elf_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    // Performance configuration
    group.bench_function("config_performance", |b| {
        let db = MagicDatabase::with_builtin_rules_and_config(EvaluationConfig::performance())
            .expect("should load");
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&elf_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    // Comprehensive configuration
    group.bench_function("config_comprehensive", |b| {
        let db = MagicDatabase::with_builtin_rules_and_config(EvaluationConfig::comprehensive())
            .expect("should load");
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&elf_data))
                .expect("should evaluate");
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark `name`/`use` subroutine dispatch overhead.
///
/// Establishes a baseline for the meta-type machinery before future
/// Aho-Corasick or compiled-regex caching optimizations. The magic source
/// declares a `part2` subroutine that matches a byte and its top-level rule
/// invokes that subroutine at two offsets via `use part2`.
fn bench_name_use_subroutines(c: &mut Criterion) {
    let magic_source = "0 string TEST Testfmt\n\
         >0 use part2\n\
         >4 use part2\n\
         0 name part2\n\
         >0 byte 0x42 inner_match\n";

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let magic_path = tmp_dir.path().join("subroutines.magic");
    {
        let mut f = std::fs::File::create(&magic_path).expect("create magic file");
        f.write_all(magic_source.as_bytes())
            .expect("write magic source");
    }

    let db = MagicDatabase::load_from_file(&magic_path).expect("should load subroutines magic");

    let mut buf = b"TEST".to_vec();
    buf.push(0x42);
    buf.extend_from_slice(&[0u8; 16]);

    let mut group = c.benchmark_group("name_use_subroutines");
    group.throughput(Throughput::Bytes(buf.len() as u64));
    group.bench_function("use_dispatch", |b| {
        b.iter(|| {
            let result = db
                .evaluate_buffer(black_box(&buf))
                .expect("should evaluate");
            black_box(result)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_file_type_detection,
    bench_buffer_sizes,
    bench_evaluation_configs,
    bench_name_use_subroutines
);
criterion_main!(benches);
