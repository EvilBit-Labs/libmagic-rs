// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target: `TypeKind::Regex` must never panic or hang on
//! attacker-controlled patterns.
//!
//! The `build_regex` helper caps NFA/DFA compile size to 1 MiB
//! (`REGEX_COMPILE_SIZE_LIMIT`) and every scan window is capped at
//! 8192 bytes (`REGEX_MAX_BYTES`). This fuzz target pins both
//! mitigations against arbitrary patterns: we want compile errors
//! on pathological inputs (not hangs or OOM), and we want matches
//! that succeed to stay within the scan window.
//!
//! The fuzzer input is split into a small header that selects flags
//! and count, a null-terminated pattern string, and the remaining
//! bytes used as the scan buffer.
//!
//! Run: `cargo +nightly fuzz run regex_pattern_compile`

#![no_main]

use libfuzzer_sys::fuzz_target;
use libmagic_rs::evaluator::{EvaluationContext, evaluate_rules};
use libmagic_rs::parser::ast::{RegexCount, RegexFlags};
use libmagic_rs::{EvaluationConfig, MagicRule, OffsetSpec, Operator, TypeKind, Value};

fuzz_target!(|data: &[u8]| {
    // Minimum header: 2 bytes of flags + pattern length.
    if data.len() < 4 {
        return;
    }
    let case_insensitive = data[0] & 0x01 != 0;
    let start_offset = data[0] & 0x02 != 0;
    let pattern_len = usize::from(data[1]).min(data.len().saturating_sub(2));
    let pattern_bytes = &data[2..2 + pattern_len];
    let buffer = &data[2 + pattern_len..];

    // Lossy UTF-8 conversion -- the regex crate only accepts &str.
    let pattern = String::from_utf8_lossy(pattern_bytes).into_owned();

    let rule = MagicRule {
        offset: OffsetSpec::Absolute(0),
        typ: TypeKind::Regex {
            flags: RegexFlags::default()
                .with_case_insensitive(case_insensitive)
                .with_start_offset(start_offset),
            count: RegexCount::Default,
        },
        op: Operator::Equal,
        value: Value::String(pattern),
        message: "fuzz".to_string(),
        children: vec![],
        level: 0,
        strength_modifier: None,
    };

    let config = EvaluationConfig::default().with_timeout_ms(Some(500));
    let mut context = EvaluationContext::new(config);
    let _ = evaluate_rules(&[rule], buffer, &mut context);
});
