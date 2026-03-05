// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Relative offset resolution (not yet implemented)

use crate::LibmagicError;
use crate::parser::ast::OffsetSpec;

/// Resolve a relative offset specification
///
/// Relative offsets are calculated relative to the current evaluation position
/// rather than from the start of the file.
///
/// # Errors
///
/// Currently returns `LibmagicError::EvaluationError` with `UnsupportedType`
/// as relative offset resolution is not yet implemented.
// TODO: Implement relative offset resolution (issue #38)
pub fn resolve_relative_offset(spec: &OffsetSpec, _buffer: &[u8]) -> Result<usize, LibmagicError> {
    debug_assert!(
        matches!(spec, OffsetSpec::Relative(_)),
        "resolve_relative_offset called with non-relative spec"
    );
    Err(LibmagicError::EvaluationError(
        crate::error::EvaluationError::unsupported_type("Relative offsets not yet implemented"),
    ))
}
