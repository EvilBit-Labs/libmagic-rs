# ADR-0001: GNU `file` compatibility is an output contract, not an ergonomics contract

**Date**: 2026-07-26\
**Status**: accepted\
**Deciders**: @UncleSp1d3r

## Context

AGENTS.md sets a v1.0.0 goal of "95%+ compatibility with GNU `file`", but never says what "compatibility" covers. That ambiguity surfaced concretely in issue #383: `file` spells no-dereference `-h`, while `-h` is already rmagic's `--help` short flag. Reading the goal as total parity forces a breaking change to rmagic's own documented CLI; reading it as loose inspiration would undermine the reason to prefer rmagic over `file` at all. The same ambiguity governs exit codes, error-reporting channels, and rmagic-only flags such as `--strict` and `--json`, which have no upstream analogue.

## Decision

Compatibility with GNU `file` is a contract on **detection results for identical input**. Given the same source file, rmagic produces the same type determination, spelled the same way.

Everything else is rmagic's own design surface:

- **Error messages and diagnostics.** Text describing a failure to read, open, or process a path is rmagic's to word. `file`'s phrasing carries no authority here.
- **Tool ergonomics.** Flag spellings and short letters, exit codes, which stream output goes to, help text, and output formatting around the detection result (column padding, separators).

The boundary test: *if the file were readable, would this string describe what it is?* If yes, it is a detection result and binding. If it instead describes why rmagic could not tell you, it is a diagnostic and free.

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

- Differential testing against the real `file` binary is the acceptance test for the v1.0.0 goal — a detection-result difference is a defect, full stop.
- rmagic can adopt modern CLI conventions (`-h` stays `--help`), ship flags `file` lacks (`--strict`, `--json`, `--timeout-ms`), and write clearer diagnostics than `file`'s terse C-era phrasing, without relitigating parity each time.
- Detection-string details easy to dismiss as cosmetic are unambiguously in scope: verbatim uncanonicalized symlink targets in `symbolic link to <target>`, and every magic-rule description string.

### Negative

- "Compatible with `file`" requires qualification in user-facing docs; a `file -h x` invocation does not port verbatim, and error text will not match.
- Contributors must classify each proposed divergence as detection vs diagnostic before evaluating it, rather than applying one blanket rule. The boundary test in the Decision section exists for this.

### Risks

- **Divergence creep** — "diagnostic" or "ergonomics" could be stretched to excuse a detection difference.

  *Mitigation (binding):* every accepted **detection-result** divergence is a **tracked contract gap**. It gets a GitHub issue and stays open until closed. It is never recorded as a settled design choice, and "pre-existing", "cosmetic", and "out of scope for this issue" are reasons to defer the fix, never reasons to skip filing.

  Applying the boundary test to the divergences known at the time of writing (all surfaced by issue #383):

  | Divergence                                             | Class                                                                                                                      | Tracked?                |
  | ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
  | `directory` — `file` prints `directory`, rmagic errors | **Detection.** `directory` describes what the path *is*.                                                                   | **Yes — file an issue** |
  | Filename column padding in multi-file output           | Formatting around the result, not the result                                                                               | No                      |
  | Nonexistent non-symlink paths (`cannot open ...`)      | Diagnostic — describes why detection failed                                                                                | No                      |
  | Unsanitized `read_link` text reaching stdout           | Detection, and passing bytes through is what *matches*; sanitizing would break parity. Security question, not a parity gap | No                      |

- **Under-specified boundary** — MIME output (`--mime`, `!:mime`) is a detection result, so it inherits the binding contract when issue #51 lands.

## Scope note

This ADR governs the *`file` compatibility* question only. It does not constrain rmagic's library API surface, which has its own stability commitments under the v1.0.0 "Stable API" milestone.
