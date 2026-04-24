// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared test helpers for the engine test suite.
//!
//! The sibling `meta_*_tests` submodules pull everything they need from
//! `super::*` in `tests/mod.rs`; this directory splits those helpers out
//! of the oversized parent module so each concern lives in a focused
//! sub-file (per AGENTS.md `**/*.rs`: "Keep source files under 500-600
//! lines; split larger files into focused modules.").

pub mod meta;
