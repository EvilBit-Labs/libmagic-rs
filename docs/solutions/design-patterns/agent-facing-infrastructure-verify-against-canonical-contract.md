---
title: Agent-facing infrastructure must verify against the canonical contract, not training-data assumptions
date: 2026-05-26
category: design-patterns
module: agent-infrastructure
problem_type: design_pattern
component: tooling
severity: high
applies_when:
  - Authoring or modifying a Claude Code hook script under `.claude/hooks/`
  - Writing or updating a SKILL.md file under `.claude/skills/`
  - Reviewing a PR that touches `.claude/` (hooks, skills, agents, commands)
  - Onboarding agent-facing infrastructure copied from another repo or generated from training-data assumptions
  - A hook appears to never fire, or a SKILL references APIs / paths / line counts you cannot locate in the codebase
related_components:
  - documentation
  - development_workflow
tags:
  - claude-code-hooks
  - skill-files
  - dco-signoff
  - agent-infrastructure
  - documentation-drift
  - multi-agent-review
  - silent-failure
---

# Agent-facing infrastructure must verify against the canonical contract, not training-data assumptions

## Context

Agent-facing infrastructure -- Claude Code hooks, SKILL.md files, agent rules, generated project documentation -- is uniquely prone to silent failure. It looks plausible on review, runs without errors, and gets trusted by future agents who treat it as authoritative. But when it is built from training-data assumptions about how the harness works, or from a stale snapshot of the codebase that has since drifted, the failure mode is invisible: hooks no-op instead of blocking, skills cite fabricated APIs, checklists fire false positives. Nothing in `just ci-check` catches it, because there is nothing to compile or test against.

PR #278 fixed two instances of this meta-pattern in libmagic-rs:

1. A DCO sign-off enforcement hook (`.claude/hooks/enforce-dco-signoff.sh`) that read tool input from an env var Claude Code does not set, silently allowing every unsigned commit through.
2. Six SKILL.md files under `.claude/skills/` pinned to a pre-v0.5.x evaluator snapshot, citing line counts off by 6x, fabricated `EvaluationConfig` field names, wrong error type names, and non-existent constructors.

Both defects shipped because the authoring process -- an LLM generating from training data, or an older session, or another repo's template copied without adaptation -- was never reconciled against the documented harness contract (Claude Code hook spec) or the current source (AST shapes, error variants, module layout). Both were caught only because a multi-agent code review specifically asked specialized agents to verify each piece against the canonical source.

This is the same meta-pattern as [[multi-agent-review-surfaces-cross-cutting-consistency-gaps-2026-04-25]], with one new partner class: **code ↔ agent-context artifacts**. The earlier doc cataloged five partner classes that all live inside the Rust compilation graph (cross-type policy, dual-purpose helpers, doc-vs-code claims, struct-vs-rustdoc examples, parse-vs-diagnostic paths). This sixth partner class -- agent infrastructure -- sits **outside** the type system and outside CI's reach entirely, so the drift is even more invisible.

## Guidance

### Hook authoring contract

Claude Code delivers tool input as **JSON on stdin**, per <https://docs.claude.com/claude-code/hooks>. The `CLAUDE_TOOL_INPUT_*` env vars are not a documented delivery mechanism. Treat env vars as a legacy fallback at best; read stdin as the primary source.

Every hook must:

- Set `set -euo pipefail` at the top so typos, unset vars, and pipe failures are loud, not silent.
- Strip quoted segments before pattern-matching against command strings. `git commit -m "fix -s flag handling"` must not trip a `-s` check.
- Use regexes that accept the full documented surface area. For short flags, that means combined forms (`-sS`, `-sm`, `-sam`, `-Ss`), not just standalone (`-s`).
- Emit multi-line error messages with concrete remediation -- including the `--amend` path for already-made commits.
- Be verified with an explicit test matrix covering positive cases, negative cases, and edge cases (compound `&&`, quoted strings, the alternate delivery mechanism, `--flag=value` form, empty input).

Skeleton:

```bash
#!/usr/bin/env bash
set -euo pipefail

CMD="${CLAUDE_TOOL_INPUT_command:-}"

# Documented delivery: JSON on stdin. Env var is legacy fallback only.
if [ -z "$CMD" ] && [ ! -t 0 ]; then
  PAYLOAD=$(cat || true)
  if [ -n "$PAYLOAD" ] && command -v jq >/dev/null 2>&1; then
    CMD=$(printf '%s' "$PAYLOAD" | jq -r '.tool_input.command // empty' || true)
  fi
fi
[ -z "$CMD" ] && exit 0

# Strip quoted segments so flags inside message strings don't false-trigger.
STRIPPED=$(printf '%s' "$CMD" | sed "s/\"[^\"]*\"//g; s/'[^']*'//g")

# ... pattern checks against $STRIPPED ...
```

### SKILL.md authoring discipline

Anything in a SKILL.md that names a specific file path, line count, type field, enum variant, function signature, or module structure is **a liability with a half-life**. The next refactor invalidates it; nothing catches the drift.

Rules:

- **Link to authoritative docs, do not duplicate them.** AGENTS.md and GOTCHAS.md are the source of truth for libmagic-rs architecture. SKILL.md should say "see AGENTS.md 'Module Organization'" rather than re-listing the module tree.
- **Do not pin file paths or line counts.** "`evaluator/mod.rs` is 2,638 lines" was wrong six months later. Talk about the *shape* of the codebase, not its exact dimensions.
- **Do not enumerate variants.** A list of `TypeKind` variants in a SKILL.md will drift the moment someone adds `Quad` or `Regex`. Reference the source enum; do not mirror it.
- **Placeholder types need disclaimers.** If a code example uses `MyConfig { ... }`, say "illustrative -- see `src/config.rs` for actual fields." Do not fabricate field names that look real.
- **Stable architectural facts are fair game.** "`unsafe_code = \"forbid\"` is set in `Cargo.toml [workspace.lints]`" is a stable mechanism statement and belongs in security-review guidance. "`MagicError::OutOfBounds` is the variant for bounds errors" is not -- that variant does not exist, and the error type is not even named `MagicError`.
- **Cross-check before merge.** Before merging any SKILL.md, grep the source tree for every type name, function name, and file path it mentions. If the names do not resolve, the SKILL does not ship.

## Why This Matters

Silent failure in agent infrastructure compounds. The DCO hook silently no-op'd for the entire time it was in the tree -- every commit Claude made was nominally guarded but actually unprotected. Without the GitHub App enforcing DCO server-side as a second line of defense, unsigned commits would have shipped. The hook's existence was worse than no hook: it created false confidence that the local-side guard was working.

The SKILL.md drift is more insidious. A security-review skill that tells a future agent to look for `#![forbid(unsafe_code)]` in `lib.rs` will produce a false-positive finding ("the forbid attribute is missing!") because the real mechanism is in `Cargo.toml`. An api-design skill that shows `EvaluationConfig::builder()` will lead an agent to "fix" code that's "missing" a builder which does not exist yet. Agents trust the infrastructure; bad infrastructure produces confidently-wrong work that has to be unwound later.

Both defects share the root cause: the artifact was authored once, looked plausible, and was never reconciled against the actual contract (Claude Code hooks spec) or the actual code (current AST and module layout). Neither was caught by tests because both targets sit outside the test harness. The GitHub App that enforces DCO server-side caught the policy-level breach in time; nothing would have caught the SKILL drift except a reviewer reading every example.

## When to Apply

- Authoring a new Claude Code hook, agent rule, slash command, or SKILL.md
- Reviewing a PR that touches `.claude/`, `AGENTS.md`, `GOTCHAS.md`, or any agent-facing documentation
- Opening a PR with substantial LLM-generated or LLM-derived documentation
- Auditing existing `.claude/skills/` trees after a significant refactor (module splits, type renames, API surface changes)
- After a multi-agent review surfaces "this rule fired a false positive" or "the agent built on a wrong assumption" -- trace it back to the infrastructure

## Examples

### Example 1: Hook delivery mechanism

**Broken** -- read env var that Claude Code does not set:

```bash
CMD="${CLAUDE_TOOL_INPUT_command:-}"

if ! echo "$CMD" | grep -qE '(^|[;&|] *)git commit( |$)'; then
  exit 0
fi
```

With env var unset, `CMD=""`, the regex never matches, the hook silently exits 0 on every invocation. The entire intent is defeated.

**Fixed** -- read stdin per the documented contract; keep env var as legacy fallback:

```bash
set -euo pipefail
CMD="${CLAUDE_TOOL_INPUT_command:-}"

if [ -z "$CMD" ] && [ ! -t 0 ]; then
  PAYLOAD=$(cat || true)
  if [ -n "$PAYLOAD" ] && command -v jq >/dev/null 2>&1; then
    CMD=$(printf '%s' "$PAYLOAD" | jq -r '.tool_input.command // empty' || true)
  fi
fi
[ -z "$CMD" ] && exit 0
```

### Example 2: Combined-flag regex

**Broken** -- matches only standalone `-s`, blocks the documented best practice:

```bash
if echo "$CMD" | grep -qE -- '(^| )-s( |$)|(^| )--signoff( |$)'; then
  exit 0
fi
```

This rejects `git commit -sS -m "..."` (DCO + GPG together -- the canonical pattern per AGENTS.md "Git Workflow Custom"), `git commit -sm "msg"`, `git commit -sam "msg"`, and every other combined short-flag form.

**Fixed** -- character class matching any combined short flag containing `s`, applied to the quote-stripped command:

```bash
STRIPPED=$(printf '%s' "$CMD" | sed "s/\"[^\"]*\"//g; s/'[^']*'//g")

if printf '%s' "$STRIPPED" | grep -qE -- \
    '(^|[[:space:]])-[A-Za-z]*s[A-Za-z]*([[:space:]]|$)|(^|[[:space:]])--signoff([[:space:]]|=|$)'; then
  exit 0
fi
```

Quote-stripping first means `git commit -m "added -s flag"` does not false-pass via the `-s` inside the message text.

### Example 3: Fabricated config fields in a SKILL.md

**Broken** -- security-review SKILL.md inventing `EvaluationConfig` shape:

```rust
// FABRICATED -- none of these fields exist in the real type.
// Reproduced verbatim from the broken SKILL.md so the failure mode is visible.
pub struct EvaluationConfig {
    pub timeout: Duration,        // FABRICATED -- real field is timeout_ms: Option<u64>
    pub max_rules: usize,         // FABRICATED -- no such field
    pub follow_symlinks: bool,    // FABRICATED -- no such field
}
```

Zero of these field names exist. The real type, per `src/config.rs`, is:

```rust
pub struct EvaluationConfig {
    pub timeout_ms: Option<u64>,
    pub max_recursion_depth: u32,
    pub stop_at_first_match: bool,
    // #[non_exhaustive]
}
```

A future agent following this SKILL would write code referencing `config.timeout`, `config.max_rules`, `config.follow_symlinks` -- none of which compile -- and file security findings citing fields the project does not have.

**Fixed** -- link to the source of truth and use illustrative placeholders with disclaimers:

```markdown
## Configuration

The evaluator is configured via `EvaluationConfig` (see `src/config.rs`
for current fields). When reviewing for DoS hardening, verify:

- A bounded execution time is configurable (see GOTCHAS.md S13.1 for the
  `Default` no-timeout gotcha).
- Recursion depth is bounded.

Example (placeholder field names -- substitute real names from `src/config.rs`):

    let config = EvaluationConfig::default()
        .with_timeout_ms(Some(1000));
```

### Example 4: Stable architectural fact vs. snapshot

**Broken** -- security-review SKILL.md cites the wrong location for `unsafe_code`:

```markdown
- Verify `#![forbid(unsafe_code)]` is present in `src/lib.rs`.
```

`lib.rs` does not contain this attribute. A reviewer running this checklist would file a finding that the forbid is missing.

**Fixed** -- cite the real mechanism, which is itself a stable architectural fact:

```markdown
- Verify `unsafe_code = "forbid"` is set in `Cargo.toml` under
  `[workspace.lints.rust]`. This is the project-wide enforcement mechanism
  (see AGENTS.md "Memory Safety First" -- it cannot be overridden by
  `#[allow(unsafe_code)]`).
```

## Prevention

- **Hook test matrices live alongside the hook.** Every hook script should have a companion test harness (Bash, with positive + negative + edge cases) that runs in CI or as part of `just ci-check`. PR #278 added a 17-case matrix for the DCO hook; that level of coverage should be the default for new hooks, not an exception.
- **Run `shellcheck` on every hook in pre-commit.** Hooks ship as production code, not throwaway scripts.
- **SKILL.md author checklist:** before opening a PR, grep the codebase for every type name, function name, and file path the SKILL mentions. If something doesn't resolve, replace it with a link to AGENTS.md / GOTCHAS.md / the source file.
- **Multi-agent review when touching `.claude/`.** Code-only review misses agent-infrastructure drift because reviewers default to "skim the markdown, deep-read the code." Multi-agent review with a `comment-analyzer` (or equivalent) reading each SKILL example against the actual source catches drift that a code-reviewer would not flag.
- **Trace false-positive findings back to infrastructure.** When an agent files a finding that is obviously wrong, do not just dismiss it -- check whether a SKILL or rule told the agent to look in the wrong place. Treat the bad finding as a debugging signal for the infrastructure.

## Related

- [[multi-agent-review-surfaces-cross-cutting-consistency-gaps-2026-04-25]] -- the same discovery mechanism (multi-agent parallel review) applied to code-side partner classes. This doc extends that pattern to a new partner class: code ↔ agent-context artifacts.
- [[multi-agent-pr-review-fixes]] -- the first whole-codebase multi-agent sweep that surfaced documentation staleness, duplicate types, and config gaps. Catalogs in-source drift; this doc extends to `.claude/`-tree drift.
- AGENTS.md -- authoritative project guide. The fix pattern is to point at it from SKILL.md rather than mirror its content.
- GOTCHAS.md -- non-obvious behavior reference. Especially S13.1 (the `EvaluationConfig::default()` no-timeout footgun that the fabricated `EvaluationConfig` example missed).
- PR #278 (`bump_deps_followup`) -- the concrete fix that this learning was extracted from.
- PR #276 -- the dep-bump PR that introduced both defects.
