// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! I/O benchmarks for libmagic-rs
//!
//! Benchmarks file I/O operations including:
//! - FileBuffer creation
//! - Memory-mapped file access
//! - Buffer slice operations

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use libmagic_rs::io::FileBuffer;
use std::hint::black_box;
use std::io::Write;
use tempfile::NamedTempFile;

/// Create a temporary file with random data
fn create_temp_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("create temp file");
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    file.write_all(&data).expect("write data");
    file.flush().expect("flush");
    file
}

/// Benchmark FileBuffer creation from files of various sizes
fn bench_file_buffer_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_buffer_creation");

    for size in [64, 1024, 4096, 65536, 262144, 1048576] {
        let temp_file = create_temp_file(size);
        let path = temp_file.path();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("mmap", size), &path, |b, path| {
            b.iter(|| {
                let buffer = FileBuffer::new(black_box(path)).expect("should create");
                black_box(buffer)
            })
        });
    }

    group.finish();
}

/// Benchmark slice access patterns on FileBuffer
fn bench_buffer_access(c: &mut Criterion) {
    let temp_file = create_temp_file(1_048_576); // 1MB file
    let path = temp_file.path();
    let buffer = FileBuffer::new(path).expect("should create");
    let slice = buffer.as_slice();

    let mut group = c.benchmark_group("buffer_access");

    // Sequential read
    group.bench_function("sequential_read_1mb", |b| {
        b.iter(|| {
            let mut sum: u64 = 0;
            for &byte in slice {
                sum = sum.wrapping_add(u64::from(byte));
            }
            black_box(sum)
        })
    });

    // Random access pattern (simulating rule evaluation)
    group.bench_function("random_access_pattern", |b| {
        let offsets = [0, 4, 16, 64, 256, 1024, 4096, 65536, 262144];
        b.iter(|| {
            let mut sum: u64 = 0;
            for &offset in &offsets {
                if offset < slice.len() {
                    sum = sum.wrapping_add(u64::from(slice[offset]));
                }
            }
            black_box(sum)
        })
    });

    // Slice extraction (common in rule evaluation)
    group.bench_function("slice_extraction", |b| {
        b.iter(|| {
            let slices = [
                slice.get(0..4),
                slice.get(4..8),
                slice.get(16..32),
                slice.get(64..128),
            ];
            black_box(slices)
        })
    });

    group.finish();
}

/// Benchmark small file handling (common case)
fn bench_small_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_files");

    // Common small file sizes
    for size in [8, 16, 32, 64, 128, 256, 512] {
        let temp_file = create_temp_file(size);
        let path = temp_file.path();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("create_and_access", size),
            &path,
            |b, path| {
                b.iter(|| {
                    let buffer = FileBuffer::new(black_box(path)).expect("should create");
                    let slice = buffer.as_slice();
                    // Simulate typical magic number check
                    let result = if slice.len() >= 4 {
                        (slice.get(0..4).map(<[u8]>::to_vec), slice.len())
                    } else {
                        (None, slice.len())
                    };
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_file_buffer_creation,
    bench_buffer_access,
    bench_small_files
);
criterion_main!(benches);
