// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Indirect offset resolution (not yet implemented)

use crate::LibmagicError;
use crate::parser::ast::OffsetSpec;

/// Resolve an indirect offset specification
///
/// Indirect offsets read a pointer value from the file at a base offset,
/// then use that value (with optional adjustment) as the final offset.
///
/// # Errors
///
/// Currently returns `LibmagicError::EvaluationError` with `UnsupportedType`
/// as indirect offset resolution is not yet implemented.
// TODO: Implement indirect offset resolution (issue #37)
pub fn resolve_indirect_offset(spec: &OffsetSpec, _buffer: &[u8]) -> Result<usize, LibmagicError> {
    debug_assert!(
        matches!(spec, OffsetSpec::Indirect { .. }),
        "resolve_indirect_offset called with non-indirect spec"
    );
    Err(LibmagicError::EvaluationError(
        crate::error::EvaluationError::unsupported_type("Indirect offsets not yet implemented"),
    ))
}
