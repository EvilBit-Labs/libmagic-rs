# Residual Review Findings -- issue #383 symlink follow policy

Source run: `ce-code-review mode:agent` over `25695d4..HEAD`, five reviewers (correctness, security, adversarial, testing, project-standards), plus a preceding `ce-simplify-code` pass (reuse, quality, efficiency).

Findings that were applied are in the branch history and are not repeated here. This file records what was **not** applied, and why, so the reasoning survives the PR.

## Deferred

- **P2 -- `tests/cli_integration.rs` is 1567 lines** (project-standards), against AGENTS.md's 500-600 line guidance. Not split: Rust integration tests are separate binaries, and this repo has no `tests/common/` module, so a thematic split requires introducing one and refactoring the pre-existing 645 lines onto it. That is a wider change than this PR should carry, and it would touch code this PR otherwise does not.

- **P2 -- filename prefixes are still lossily decoded** (security, residual risk). `output_result` and the `ClassifiedUnreadable` diagnostic both render the path via `Path::display()`, which substitutes U+FFFD for invalid UTF-8. The symlink *target* was fixed in this PR because ADR-0001 binds it; the filename prefix is pre-existing, applies to every file type rather than just symlinks, and is out of scope here. Worth a follow-up in its own right.

## Rejected, with evidence

- **P2 -- "trailing slash reintroduces issue #383"** (adversarial, confidence 100). The observation is real -- `rmagic brokenlink/` does not classify -- but the framing is wrong. Measured: `file brokenlink/` also declines, printing `` cannot open `brokenlink/' (No such file or directory) ``. Both tools refuse; only the diagnostic wording differs, which ADR-0001 explicitly frees. Stripping trailing separators would make rmagic classify a path GNU `file` will not -- a divergence in the other direction. Recorded as GOTCHAS S17.2b.

- **P2 -- "GOTCHAS S17.3 documents the old escape set"** (project-standards, confidence 100). Stale: read from a snapshot predating commit `f3d0d4c`, which had already rewritten that section.

- **Reuse of the precheck's discarded `lstat` for the `is_dir()` check** (efficiency). A real redundant syscall, but `is_dir()` follows symlinks and `lstat` does not, and `classify_symlink` returns `None` for both "not a symlink" and "symlink, followed" -- only the first may reuse it. Measured: `stat` reports `is_dir() == true` for a directory symlink where `lstat` reports `false`. Documented as GOTCHAS S17.5a so it is not re-attempted.

## Settled-decision conflicts (proceeded and flagged)

- **KTD6 (`user-approved`) -- runtime skip, not `#[cfg(unix)]`.** The testing reviewer proposed gating the symlink test module behind `#[cfg(unix)]` so a permission failure is fatal. That directly contradicts KTD6, which chose runtime skip precisely so the tests still run wherever symlinks happen to work. The underlying risk was real, so it was addressed compatibly instead: `RMAGIC_REQUIRE_SYMLINKS=1` turns a skip into a hard failure for CI, leaving the runtime-skip default intact.

- **R5 -- "`--strict` ... stderr silent".** The shipped behavior prints one accurate line (`File error: unreadable symlink: <path>`) at exit 3. The per-file loop is silent as the plan's dispatch table specifies; the remaining line is `main()`'s exit-code explanation. Total silence would leave a bare exit 3 with no stated reason, and the previous `IoError(NotFound)` mapping printed actively wrong advice ("check the file path") for a path that was classified successfully. Confirmed by the maintainer during the run.

## Coverage not run

Disclosed rather than silently skipped:

- **Cross-model adversarial pass** -- not started. It ships repository source to a third-party provider; that egress was not separately authorized, so the in-process adversarial reviewer ran instead. It shares the orchestrator's model family and therefore some of its blind spots.
- **maintainability persona** -- its surface was present (1276 executable changed lines, a new module, file moves), but `ce-simplify-code` had just run three dedicated reuse/quality/efficiency reviewers over the identical diff and those findings were applied.
- **reliability persona** -- error handling is central to this change, but the `FileOutcome` dispatch and the three-arm `--strict` path were given to correctness and adversarial as explicit focus areas.
