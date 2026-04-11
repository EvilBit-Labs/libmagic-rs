// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target: `parse_text_magic_file` must never panic on any input.
//!
//! The parser is the primary entry point for untrusted magic-file
//! content (user-supplied `--magic-file <path>`, system magic files,
//! or web-uploaded configurations). A panic in the parser crashes the
//! rmagic binary and exposes a DoS surface in any library consumer
//! that evaluates user-supplied magic.
//!
//! Run: `cargo +nightly fuzz run parse_text_magic_file`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Convert the arbitrary byte input to a UTF-8 string via lossy
    // decoding so the fuzzer can reach every parser code path,
    // including the non-UTF-8 replacement handling, without being
    // limited to valid-UTF-8 shapes.
    let text = String::from_utf8_lossy(data);
    let _ = libmagic_rs::parser::parse_text_magic_file(&text);
});
