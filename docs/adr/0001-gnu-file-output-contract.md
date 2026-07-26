# ADR-0001: GNU `file` compatibility is an output contract, not an ergonomics contract

**Date**: 2026-07-26\
**Status**: accepted\
**Deciders**: @UncleSp1d3r

## Context

AGENTS.md sets a v1.0.0 goal of "95%+ compatibility with GNU `file`", but never says what "compatibility" covers. That ambiguity surfaced concretely in issue #383: `file` spells no-dereference `-h`, while `-h` is already rmagic's `--help` short flag. Reading the goal as total parity forces a breaking change to rmagic's own documented CLI; reading it as loose inspiration would undermine the reason to prefer rmagic over `file` at all. The same ambiguity governs exit codes, error-reporting channels, and rmagic-only flags such as `--strict` and `--json`, which have no upstream analogue.

## Decision

Compatibility with GNU `file` is a contract on **observable classification output for identical input**. Given the same source file, rmagic produces the same detection result. Tool ergonomics — flag spellings and short letters, exit codes, error-reporting channels, help text — are rmagic's own design surface.

## Alternatives Considered

### Alternative 1: Full magic(1) parity, ergonomics included

- **Pros**: one rule, no judgment calls; `file`-targeted scripts and muscle memory port unchanged.
- **Cons**: imports `file`'s historical quirks wholesale, including its lack of a short help flag; forces breaking changes to rmagic's existing published CLI; blocks rmagic-only features that have no upstream spelling.
- **Why not**: it would require rebinding `-h` away from `--help`, a silent behavior change to a documented flag, purchased only with cosmetic parity that changes no classification output.

### Alternative 2: Loose "inspired by" compatibility, no binding output contract

- **Pros**: maximum design freedom; no differential-testing burden.
- **Cons**: destroys drop-in replaceability; makes "95%+ compatibility" unmeasurable; every output difference becomes arguable rather than a defect.
- **Why not**: drop-in detection compatibility is the project's core value proposition. Without a binding output contract there is no acceptance criterion for the v1.0.0 goal.

## Consequences

### Positive

- Differential testing against the real `file` binary is the acceptance test for the v1.0.0 goal — an output difference is a defect, full stop.
- rmagic can adopt modern CLI conventions (`-h` stays `--help`) and ship flags `file` lacks (`--strict`, `--json`, `--timeout-ms`) without relitigating parity each time.
- Classification-string details that are easy to dismiss as cosmetic are unambiguously in scope: verbatim uncanonicalized symlink targets, the `` `name' `` quoting style, distinct wording for empty-target links.

### Negative

- "Compatible with `file`" requires qualification in user-facing docs; a `file -h x` invocation does not port verbatim.
- Contributors must classify each proposed divergence as output vs ergonomics before evaluating it, rather than applying one blanket rule.

### Risks

- **Divergence creep** — "ergonomics" could be stretched to excuse output differences.

  *Mitigation (binding):* every accepted output divergence is a **tracked contract gap**. It gets a GitHub issue and stays open until closed. An output divergence is never recorded as a settled design choice, and "pre-existing", "cosmetic", and "out of scope for this issue" are reasons to defer the fix, never reasons to skip filing. Divergences known at the time of writing, all from issue #383's spec, requiring issues:

  1. Filename column padding in multi-file output (`file` pads, rmagic uses a single space).
  2. Nonexistent non-symlink paths (`file` prints to stdout and exits 0; rmagic prints to stderr).
  3. `directory` classification (`file` prints `directory`; rmagic errors).
  4. Unsanitized `read_link` text reaching stdout (control characters and terminal escapes pass through).

- **Under-specified boundary** — MIME output (`--mime`, `!:mime`) is output, so it inherits the binding contract when issue #51 lands.

## Scope note

This ADR governs the *`file` compatibility* question only. It does not constrain rmagic's library API surface, which has its own stability commitments under the v1.0.0 "Stable API" milestone.
