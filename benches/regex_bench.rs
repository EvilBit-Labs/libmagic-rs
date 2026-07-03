// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Regex evaluation benchmarks.
//!
//! Covers review finding T-M3 (missing benchmarks) and establishes a
//! baseline for measuring the P-H1 regex compile-cache win. Workloads:
//!
//! * **hot cache hit**: one regex, compiled once, matched many times.
//!   With the thread-local cache (P-H1), subsequent matches pay only
//!   `HashMap::get + Regex::clone` rather than a full `RegexBuilder::build`.
//! * **cache miss**: each iteration uses a unique pattern string so the
//!   cache never helps -- this measures raw compile cost + first match.
//! * **many rules**: 50 distinct patterns against a 1 KiB buffer,
//!   approximating a realistic magic-file-driven scan. This is the
//!   workload the P-H1 win targets.
//!
//! Run with:
//!
//! ```sh
//! cargo bench --bench regex_bench
//! cargo bench --bench regex_bench -- --save-baseline pre-p-h1
//! cargo bench --bench regex_bench -- --baseline pre-p-h1
//! ```

// Bench code is exempt from the panic-safety restriction lints (see
// clippy.toml); these lack an allow-*-in-tests config option, so the
// exemption is applied per crate instead.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::unreadable_literal
)]

use criterion::{Criterion, criterion_group, criterion_main};
use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
use libmagic_rs::parser::ast::{RegexCount, RegexFlags};
use libmagic_rs::{EvaluationConfig, MagicRule, OffsetSpec, Operator, TypeKind, Value};
use std::hint::black_box;

fn regex_rule(pattern: &str) -> MagicRule {
    MagicRule::new(
        OffsetSpec::Absolute(0),
        TypeKind::Regex {
            flags: RegexFlags::default(),
            count: RegexCount::Default,
        },
        Operator::Equal,
        Value::String(pattern.to_string()),
        "bench-match".to_string(),
    )
}

fn make_context() -> EvaluationContext {
    EvaluationContext::new(EvaluationConfig::default().with_timeout_ms(Some(5_000)))
}

fn bench_hot_cache_hit(c: &mut Criterion) {
    let rule = regex_rule(r"^Hello, World!$");
    let buffer = b"Hello, World!\n".to_vec();
    c.bench_function("regex/hot_cache_hit", |b| {
        b.iter(|| {
            let mut ctx = make_context();
            let _ = evaluate_rules(
                black_box(std::slice::from_ref(&rule)),
                black_box(&buffer),
                &mut ctx,
            );
        });
    });
}

fn bench_cold_compile(c: &mut Criterion) {
    // Each call uses a fresh evaluation context, which resets the cache
    // and forces a compile. Same pattern every iteration to remove
    // per-pattern complexity noise.
    let buffer = b"abc 1234 def".to_vec();
    c.bench_function("regex/cold_compile", |b| {
        b.iter(|| {
            let rule = regex_rule(r"[a-z]+ [0-9]+ [a-z]+");
            let mut ctx = make_context();
            let _ = evaluate_rules(black_box(&[rule]), black_box(&buffer), &mut ctx);
        });
    });
}

fn bench_many_rules_over_buffer(c: &mut Criterion) {
    // 50 distinct patterns, 1 KiB buffer containing a needle that
    // matches a handful of them. Approximates a realistic magic-file
    // scan. With P-H1 the cache is warmed by the first successful
    // match and subsequent `regex_bytes_consumed` calls are free.
    let rules: Vec<MagicRule> = (0..50)
        .map(|i| regex_rule(&format!(r"prefix{i}\s[A-Z]+")))
        .collect();
    let mut buffer = vec![b' '; 1024];
    buffer[0..32].copy_from_slice(b"prefix3 NEEDLE prefix7 NEEDLE  \n");
    c.bench_function("regex/many_rules_over_1kib_buffer", |b| {
        b.iter(|| {
            let mut ctx = make_context();
            let _ = evaluate_rules(black_box(&rules), black_box(&buffer), &mut ctx);
        });
    });
}

criterion_group!(
    benches,
    bench_hot_cache_hit,
    bench_cold_compile,
    bench_many_rules_over_buffer,
);
criterion_main!(benches);
