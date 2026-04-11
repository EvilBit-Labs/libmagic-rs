// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target: `MagicDatabase::evaluate_buffer` with built-in rules
//! must never panic on any input buffer.
//!
//! This is the primary evaluation path users hit when scanning
//! arbitrary file content. The built-in rule set exercises every
//! `TypeKind` variant, offset kind, and operator, so fuzzing the
//! buffer input catches bounds errors, integer overflow, and
//! panics in any code reachable from a successful or failing match.
//!
//! The evaluation runs with a 1-second timeout so the fuzzer cannot
//! get stuck on pathological regex/search patterns loaded from the
//! built-in rules (the timeout itself is a library feature being
//! exercised here).
//!
//! Run: `cargo +nightly fuzz run evaluate_rules_buffer`

#![no_main]

use libfuzzer_sys::fuzz_target;
use libmagic_rs::{EvaluationConfig, MagicDatabase};
use std::sync::OnceLock;

static DB: OnceLock<MagicDatabase> = OnceLock::new();

fn get_db() -> &'static MagicDatabase {
    DB.get_or_init(|| {
        MagicDatabase::with_builtin_rules_and_config(
            EvaluationConfig::default().with_timeout_ms(Some(1_000)),
        )
        .expect("built-in rules must load")
    })
}

fuzz_target!(|data: &[u8]| {
    let _ = get_db().evaluate_buffer(data);
});
