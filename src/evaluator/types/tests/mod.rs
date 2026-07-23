// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `evaluator::types`, split into themed submodules (issue
//! #391 item 1, unit U6) to keep each file under the project's line-count
//! guideline:
//!
//! - `numeric_dispatch` -- `TypeReadError` basics and `read_typed_value`
//!   numeric/string/date dispatch across byte/short/long/quad/float/
//!   double/date/qdate.
//! - `coerce` -- `coerce_value_to_type` signed/unsigned reinterpretation,
//!   float/double precision, and numeric-to-timestamp-string formatting.
//! - `bytes_consumed_basic` -- `bytes_consumed_with_pattern` for
//!   fixed-width types, `String`, and `PString`, called with no pattern.
//! - `bytes_consumed_pattern` -- `bytes_consumed_with_pattern` for the
//!   pattern-bearing types (`Regex`, `Search`) and `String` compared
//!   against a `Value::Bytes` operand.
//! - `regex_pattern_skip` -- U2 `TypeReadError::is_pattern_skip` /
//!   `is_regex_compile_failure` classification.
//! - `regex_decode` -- U1 `Value::Bytes` backstop for `TypeKind::Regex`
//!   pattern acceptance, and the `decode_regex_bytes_pattern`
//!   warn!-on-real-substitution contract (KTD6).
//! - `endian_flip` -- `flip_type_endian` (`use \^name`, issue #236).

mod bytes_consumed_basic;
mod bytes_consumed_pattern;
mod coerce;
mod endian_flip;
mod numeric_dispatch;
mod regex_decode;
mod regex_pattern_skip;

use super::*;
use crate::parser::ast::{Endianness, SearchFlags, StringFlags};
