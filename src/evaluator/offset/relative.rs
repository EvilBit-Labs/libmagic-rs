// Copyright (c) 2025-2026 the libmagic-rs contributors
// SPDX-License-Identifier: Apache-2.0

//! Relative offset resolution.
//!
//! Relative offsets (`&+N` / `&-N` in magic-file syntax) resolve against the
//! end position of the most recent successful match -- the GNU `file`
//! "previous match" anchor. The anchor is supplied by the evaluation engine
//! via `EvaluationContext::last_match_end()` and starts at 0 for a fresh
//! evaluation, which makes top-level relative offsets resolve from the file
//! start (matching libmagic semantics).

use crate::LibmagicError;
use crate::parser::ast::OffsetSpec;

/// Resolve a relative offset specification against the previous-match anchor.
///
/// The result is `last_match_end + delta` (where `delta` may be negative),
/// bounds-checked against the buffer. Both arithmetic underflow/overflow and
/// out-of-buffer targets are reported as `LibmagicError::EvaluationError`.
///
/// # Arguments
///
/// * `spec` - Must be `OffsetSpec::Relative(delta)` (debug-asserted).
/// * `buffer` - The file buffer used for the bounds check.
/// * `last_match_end` - End offset of the most recent successful match, or
///   `0` if no prior match exists in this evaluation pass.
///
/// # Errors
///
/// * `EvaluationError::InvalidOffset` -- arithmetic over/underflow, or the
///   delta cannot be represented as `isize` on the current target. Caught
///   by the engine's graceful-skip arm, so the rule is silently dropped.
/// * `EvaluationError::BufferOverrun` -- the resolved target is at or past
///   the end of the buffer. Same graceful-skip treatment.
/// * `EvaluationError::InternalError` -- only if called with a non-`Relative`
///   spec (programming error; debug-asserts in test/dev builds). This
///   variant is intentionally NOT in the engine's graceful-skip list and
///   will terminate evaluation if it ever fires in release builds.
pub fn resolve_relative_offset(
    spec: &OffsetSpec,
    buffer: &[u8],
    last_match_end: usize,
) -> Result<usize, LibmagicError> {
    debug_assert!(
        matches!(spec, OffsetSpec::Relative(_)),
        "resolve_relative_offset called with non-relative spec"
    );
    let &OffsetSpec::Relative(delta) = spec else {
        // Defensive: outside of debug builds, fall through with a clear error
        // rather than relying on the assertion.
        return Err(LibmagicError::EvaluationError(
            crate::error::EvaluationError::internal_error(
                "resolve_relative_offset called with non-relative spec",
            ),
        ));
    };

    let delta_isize = isize::try_from(delta).map_err(|_| {
        LibmagicError::EvaluationError(crate::error::EvaluationError::InvalidOffset {
            offset: delta,
        })
    })?;

    let target =
        last_match_end
            .checked_add_signed(delta_isize)
            .ok_or(LibmagicError::EvaluationError(
                crate::error::EvaluationError::InvalidOffset { offset: delta },
            ))?;

    if target >= buffer.len() {
        return Err(LibmagicError::EvaluationError(
            crate::error::EvaluationError::BufferOverrun { offset: target },
        ));
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EvaluationError;

    fn unwrap_eval_err(err: LibmagicError) -> EvaluationError {
        match err {
            LibmagicError::EvaluationError(e) => e,
            other => panic!("expected EvaluationError, got {other:?}"),
        }
    }

    #[test]
    fn positive_delta_resolves() {
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(3);
        assert_eq!(resolve_relative_offset(&spec, buf, 4).unwrap(), 7);
    }

    #[test]
    fn negative_delta_resolves() {
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(-3);
        assert_eq!(resolve_relative_offset(&spec, buf, 8).unwrap(), 5);
    }

    #[test]
    fn zero_delta_resolves_to_anchor() {
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(0);
        assert_eq!(resolve_relative_offset(&spec, buf, 4).unwrap(), 4);
    }

    #[test]
    fn top_level_anchor_zero_resolves_from_start() {
        // last_match_end = 0 means top-level: Relative(5) -> absolute 5.
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(5);
        assert_eq!(resolve_relative_offset(&spec, buf, 0).unwrap(), 5);
    }

    #[test]
    fn top_level_anchor_zero_with_zero_delta_is_zero() {
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(0);
        assert_eq!(resolve_relative_offset(&spec, buf, 0).unwrap(), 0);
    }

    #[test]
    fn last_valid_index_resolves() {
        // 16-byte buffer, last valid index is 15.
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(0);
        assert_eq!(resolve_relative_offset(&spec, buf, 15).unwrap(), 15);
    }

    #[test]
    fn negative_delta_underflows_to_invalid_offset() {
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(-5);
        let err = unwrap_eval_err(resolve_relative_offset(&spec, buf, 2).unwrap_err());
        assert!(
            matches!(err, EvaluationError::InvalidOffset { offset: -5 }),
            "got {err:?}"
        );
    }

    #[test]
    fn positive_delta_overflows_to_invalid_offset() {
        // anchor=usize::MAX, delta=+1 -> checked_add_signed returns None.
        let buf = b"0123456789ABCDEF";
        let spec = OffsetSpec::Relative(1);
        let err = unwrap_eval_err(resolve_relative_offset(&spec, buf, usize::MAX).unwrap_err());
        assert!(
            matches!(err, EvaluationError::InvalidOffset { offset: 1 }),
            "got {err:?}"
        );
    }

    #[test]
    fn target_past_buffer_end_returns_buffer_overrun() {
        let buf = b"0123456789ABCDEF"; // len 16
        let spec = OffsetSpec::Relative(50);
        let err = unwrap_eval_err(resolve_relative_offset(&spec, buf, 10).unwrap_err());
        assert!(
            matches!(err, EvaluationError::BufferOverrun { offset: 60 }),
            "got {err:?}"
        );
    }

    #[test]
    fn target_equal_to_buffer_len_is_overrun() {
        // Resolved offset == buffer.len() is out of bounds.
        let buf = b"0123456789ABCDEF"; // len 16
        let spec = OffsetSpec::Relative(1);
        let err = unwrap_eval_err(resolve_relative_offset(&spec, buf, 15).unwrap_err());
        assert!(matches!(err, EvaluationError::BufferOverrun { offset: 16 }));
    }
}
