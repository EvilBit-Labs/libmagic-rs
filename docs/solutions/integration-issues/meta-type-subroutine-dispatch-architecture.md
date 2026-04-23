---
title: Parse-time name table extraction and context-threaded RuleEnvironment for meta-type subroutines
date: 2026-04-22
last_refreshed: 2026-04-22
status: resolved
severity: medium
category: integration-issues
components:
  - parser/name_table
  - parser/mod
  - parser/loader
  - evaluator/mod
  - evaluator/engine
  - evaluator/offset
  - output/format
  - MagicDatabase
tags:
  - rust
  - parser
  - evaluator
  - meta-types
  - name-use
  - subroutine
  - rule-environment
  - recursion-guard
  - control-flow
  - architecture-pattern
issue: '#42'
pr: '#230'
branch: 42-parser-implement-default-clear-name-use-and-indirect-meta-types
applies_when:
  - Implementing a new magic(5) control-flow directive (all six -- default, clear, name, use, indirect, offset -- are now wired through; reference this pattern when adding a seventh or when refactoring existing dispatch)
  - Adding any whole-database state that evaluation needs to consult outside the current rule
  - Considering breaking changes to evaluate_rules / evaluate_rules_with_config
root_cause: Control-flow directives do not fit the evaluator's "resolve offset -> read typed value -> apply operator" pipeline; whole-database state (name tables, root rule re-entry) must live somewhere
solution_files:
  - src/parser/name_table.rs
  - src/parser/mod.rs
  - src/parser/loader.rs
  - src/evaluator/mod.rs
  - src/evaluator/engine/mod.rs
  - src/evaluator/offset/mod.rs
  - src/output/format.rs
  - src/error.rs
  - src/lib.rs
  - tests/meta_types_integration.rs
related_gotchas:
  - S2.1 TypeKind exhaustive-match discipline still applies; the Meta(Use) / Meta(Indirect) / Meta(Offset) arms are dispatched from evaluate_rules, not evaluate_single_rule_with_anchor
  - S3 parser architecture now produces ParsedMagic { rules, name_table }, not Vec<MagicRule>
  - S3.8 top-level sibling anchor chaining; S3.10 subroutine base_offset semantics
  - S14.2 printf-style format substitution (wired into concatenate_messages via src/output/format.rs)
  - Property tests synthesize arbitrary TypeKind values; evaluator arms for Meta must debug!-log rather than debug_assert!-panic
---

# Parse-time name table extraction and context-threaded RuleEnvironment for meta-type subroutines

## Context

The magic(5) grammar includes directives that are **control-flow, not value-reading**: `name`/`use` (callable subroutines), `indirect` (re-enter the full rule set against an offset computed from the current buffer), and `default`/`clear` (sibling-chain predicates that depend on whether any prior rule at the same level matched). These don't fit the evaluator's core pipeline -- `resolve offset -> read typed value -> coerce expected value -> apply operator -> produce RuleMatch`. There is nothing to read, no operator to apply, and the output is either "splice in another rule list's matches" (`use`, `indirect`) or "alter dispatch of the next sibling" (`default`, `clear`).

Phase 1 (issue #42, earlier on the same branch) absorbed the grammar and AST by modeling these as `TypeKind::Meta(MetaType::...)` and treating them as silent no-ops in `evaluate_single_rule_with_anchor`. That was the right way to ship the parser without blocking the evaluator, but silent no-ops are not a viable endpoint: real third-party magic corpora (the GNU `file` distribution's `Magdir/` tree, fed into libmagic-rs via `third_party/`) make heavy use of `use` for shared subroutines (MS Office variants, archive headers, JPEG/EXIF chains), and silently dropping them produces visibly inferior classification output versus GNU `file`.

The question this phase answered was: where in the **parse -> database -> evaluate** pipeline should whole-database concerns like "look up a subroutine by name" live?

## Guidance: the three-layer pattern

### Layer 1 -- parse-time extraction, not runtime lookup

`Meta(Name(id))` rules are hoisted *out* of the flat rule list at load time by `src/parser/name_table.rs::extract_name_table`, which returns a `(Vec<MagicRule>, NameTable)` pair. The evaluator's hot loop never sees a `Name` rule at all; duplicate-name detection is a one-shot `warn!` at parse time rather than a per-evaluation cost, and nested `Name` rules (not well-defined in magic(5)) are scrubbed with a warning.

```rust
pub(crate) fn extract_name_table(
    rules: Vec<MagicRule>,
) -> (Vec<MagicRule>, NameTable) {
    // For each top-level rule: if Meta(Name(id)), move children into
    // the table keyed by id; otherwise keep the rule and scrub any
    // stray nested Name rules out of its children.
}
```

### Layer 2 -- `ParsedMagic` as the parser return type

`parse_text_magic_file`, `load_magic_file`, and `load_magic_directory` now return `Result<ParsedMagic { rules: Vec<MagicRule>, name_table: NameTable }, ParseError>` instead of `Result<Vec<MagicRule>, ParseError>`. Directory loads merge per-file name tables with a first-wins policy (matching GNU `file` behavior: earlier `Magdir/` files shadow later ones, logged at `warn!`).

All callers destructure at the boundary:

```rust
let ParsedMagic { rules, name_table } = parse_text_magic_file(&source)?;
// codegen uses `rules`; runtime attaches `name_table` to the database
```

### Layer 3 -- optional `RuleEnvironment` threaded through `EvaluationContext`

Whole-database state lives in:

```rust
pub(crate) struct RuleEnvironment {
    name_table: Arc<NameTable>,
    root_rules: Arc<[MagicRule]>,
}
```

`EvaluationContext` gained a `rule_env: Option<Arc<RuleEnvironment>>` field. `MagicDatabase::evaluate_file` attaches the environment before calling `evaluate_rules`; programmatic consumers (`evaluate_rules_with_config`, property tests, fuzz harnesses) default to `None`, and `Use` / `Indirect` rules then become silent no-ops.

`Arc` (not `&`) because the context already outlives individual rule borrows, and property tests construct contexts without a lifetime parameter on `EvaluationContext`. `root_rules` was initially staged speculatively for `indirect` and is now live — `MetaType::Indirect` dispatch in `evaluate_rules` reads `root_rules` and re-enters the full ruleset at the resolved offset, bounded by the existing `max_recursion_depth` via `RecursionGuard`.

`EvaluationContext` also grew a companion field — `base_offset: usize` — that is not on `RuleEnvironment` because it is per-evaluation-frame state rather than per-database state. `base_offset` biases positive `OffsetSpec::Absolute(n)` resolution inside a `MetaType::Use` subroutine body so that `>N` rules resolve relative to the use-site (magic(5) semantics). See GOTCHAS S3.10 and the companion learning in `logic-errors/raii-scope-guards-for-evaluator-context-save-restore.md` for why `base_offset` is save/restored via a `SubroutineScope` RAII guard rather than manually.

## Why this matters

Four alternatives were considered and rejected. Each rejection is load-bearing for future meta-type work; revisit the rationale before reverting any of them.

**Rejected: runtime lookup in the hot loop.** Walking the rule list every evaluation to resolve a `use name` target would turn a flat O(N) dispatch into an amortized quadratic one, and would make duplicate-name detection a *per-buffer* cost rather than *per-load*. The parse-time hoist pays the cost exactly once per magic file.

**Rejected: non-optional `RuleEnvironment` / new required arg to `evaluate_rules`.** A cleaner API would have `RuleEnvironment` as a required field -- it is required for correct `use` evaluation. The concrete reason to make it optional is not API stability in the abstract; it is that every property test, fuzz harness, and in-tree integration test that calls `evaluate_rules` with a hand-built rule tree would have to synthesize and pass an empty environment to keep compiling. Under the "every meta-type will eventually need environment state" worldview that is cheap. Under the actual Phase 2 scope -- one directive needs it -- the churn buys nothing. Make it optional on the context now; tighten if we ever need to enforce "`use` must have an environment" as a contract. (session history)

**Rejected: `debug_assert!` that `Name` rules never reach the evaluator.** `prop_arbitrary_rule_evaluation_never_panics` synthesizes arbitrary `TypeKind` instances, including `Meta(Name(_))`, and feeds them directly to `evaluate_single_rule`. A `debug_assert!` there would break the never-panics invariant the entire property test exists to enforce. The implementation uses `debug!` logging instead -- correct in production, non-fatal in property-test space.

**Rejected: dispatching `Use` through `evaluate_single_rule_with_anchor`.** The single-rule helper returns `Result<Option<(usize, Value)>, _>` -- one match, one value. `Use` produces a *vector* of child matches that must be spliced into the caller's match buffer in document order. Pushing that semantic through the helper would have reshaped its return type to `Vec<RuleMatch>` and cascaded through every other `TypeKind` branch. Keeping `Use` at the `evaluate_rules` level is a cleaner seam. (session history)

## When to apply

The three-layer pattern is the template every shipped magic(5) control-flow directive follows, and the template for future ones:

- **`indirect`** (shipped in PR #230): resolves an offset, re-enters `env.root_rules` against a sub-slice of the buffer via `AnchorScope`. Layer 1 is trivial (no hoist — `indirect` is a value-position directive, not a top-level declaration); Layer 3 provides `root_rules` as the re-entry point. The anchor semantics differ from `use`: `indirect` starts fresh at the resolved offset and does **not** save/restore the caller's `last_match_end` across sibling evaluation, whereas `use` is a scoped subroutine that saves and restores via `SubroutineScope` (which also covers `base_offset`).
- **`default`/`clear`** (shipped in PR #230): sibling-chain predicates, implemented via a **frame-local `sibling_matched: bool`** inside `evaluate_rules` — explicitly NOT a new field on `EvaluationContext`, because the state's lifetime is the single recursion frame rather than the whole evaluation. `clear` resets the flag, `default` fires only when the flag is still false. The earlier speculation in this doc about a `MatchStateTracker` context field was rejected in favor of the simpler frame-local approach.
- **`offset`** (shipped in PR #230): a value-position directive that reports the resolved file offset as `Value::Uint(pos)` so printf-style format specifiers (`%lld`, `%d`) can substitute it in the rule message. Layer 3 is not involved; the dispatch reads nothing from `RuleEnvironment`. What it does need is the companion printf substitution path in `src/output/format.rs::format_magic_message`, wired into `MagicDatabase::concatenate_messages`.
- **Continuation-sibling anchor reset** (shipped in PR #230): at `recursion_depth > 0`, each sibling's `&N` offset resolves against the parent-level entry anchor rather than the previous sibling's advance. Top-level siblings (depth 0) keep chaining per GOTCHAS S3.8. This is the mechanism that makes `searchbug.magic`-style continuation chains match GNU `file` byte-for-byte.
- **Future `!:mime` / `!:ext` / `!:apple` directive evaluation** (tracked under v0.6.0's `Directive` extension point): same shape — extracted at parse time into a per-rule directive table, threaded via `RuleEnvironment`, consulted only by the match-accumulation path, not the hot read loop.

The general rule: **if a directive's meaning depends on state outside the single rule being evaluated, hoist it at parse time into an environment that rides alongside the context. Never reach for the whole rule tree from inside the evaluation loop.**

## Examples

### The `Use` dispatch in `evaluate_rules` (`src/evaluator/engine/mod.rs`)

```rust
if let TypeKind::Meta(MetaType::Use(name)) = &rule.typ {
    match evaluate_use_rule(rule, name, buffer, context) {
        Ok((Some(absolute_offset), subroutine_matches)) => {
            matches.extend(subroutine_matches);
            // Re-advance the anchor to the use-site offset so sibling
            // rules resolve relative offsets from the use-site end.
            context.set_last_match_end(absolute_offset);
        }
        Ok((None, _)) => { /* no env or name not found -- no-op */ }
        // Error handling: demote buffer/offset errors to a debug log,
        // propagate everything else.
        Err(e) => return Err(e), // (simplified; see source for skip arms)
    }
    continue;
}
```

The anchor save/restore inside `evaluate_use_rule` is implemented via `SubroutineScope<'a>`, a Drop-based RAII guard that saves both `last_match_end` and `base_offset` on entry, seeds them with the use-site offset, and restores both on every exit path — including panic unwind and `?` short-circuits from inner `RecursionGuard::enter(context)?` or inner `evaluate_rules(...)?`. After `evaluate_use_rule` returns, the outer loop re-advances the anchor to the use-site offset so sibling rules see the `use` as having "consumed" the use-site position. Mutual recursion (`a use b; b use a`) is caught by `RecursionGuard::enter(context)?` and surfaced as `EvaluationError::RecursionLimitExceeded`; the `SubroutineScope` guarantees the caller's anchor and base_offset are restored even when that error propagates. See `logic-errors/raii-scope-guards-for-evaluator-context-save-restore.md` for the full rationale and the anti-pattern it replaced.

One subtlety the first Phase 3 attempt got wrong: the `Use` rule's own *children* (continuation rules at deeper indentation following the `use` directive) must still be evaluated after the subroutine returns. The initial implementation skipped them, silently breaking valid libmagic chains. The fix evaluates the `use` rule's children after the named rule body completes. (session history)

### The `ParsedMagic` destructure pattern at call sites

```rust
// build.rs / src/build_helpers.rs -- codegen does not need the name table
let parsed = parse_text_magic_file(&source)?;
generate_rules_module(&parsed.rules, out_path)?;

// src/lib.rs::MagicDatabase::load_from_file_with_config
let ParsedMagic { rules, name_table } =
    parser::load_magic_file(path.as_ref())?;
// ... strength-sort `rules` and each subroutine in `name_table.values_mut()`,
// then construct the database with Arc-wrapped state.
```

Each subroutine body is strength-sorted recursively the same way top-level rules are, so evaluation of a `use` site is deterministic regardless of source order inside the `name` block.

### Property-test-safe leaked-`Name` handling (`evaluate_single_rule_with_anchor`)

```rust
TypeKind::Meta(MetaType::Name(name)) => {
    // Normally hoisted at parse time; reaching here means a
    // programmatic consumer (property test, fuzz harness) built
    // the rule directly. Log and no-op -- a debug_assert would
    // break prop_arbitrary_rule_evaluation_never_panics.
    debug!(
        "Name rule '{name}' reached evaluator (likely bypassed \
         name-table extraction); treating as no-op"
    );
    return Ok(None);
}
TypeKind::Meta(MetaType::Use(_)) => {
    // `Use` is dispatched inline by `evaluate_rules`. Reaching
    // this arm means the rule went through the single-rule path
    // (e.g. evaluate_single_rule) which lacks that wiring.
    return Ok(None);
}
TypeKind::Meta(_) => return Ok(None),
```

The asymmetry between `debug!` (production-safe and test-safe) and `debug_assert!` (production-safe but test-hostile) is the load-bearing detail future maintainers will want to preserve when adding `indirect`, `default`, and `clear` arms here.

## Prevention

- When adding a new `MetaType` variant, add an explicit arm to the match in `evaluate_single_rule_with_anchor`. The catch-all `TypeKind::Meta(_) => return Ok(None)` is the default, but anything needing inline dispatch (like `Use`) should be handled at the `evaluate_rules` loop level, not in the single-rule helper. This is catalogued under GOTCHAS S2.1.
- The smoke test in `tests/meta_types_integration.rs` evaluates `third_party/tests/searchbug.magic` (the GNU `file` fixture exercising `name`/`use` + `search/N` + relative offsets). The assertion `result.description.starts_with("Testfmt")` guards the primary regression target for subroutine dispatch plus continuation rules -- a weaker non-empty check alone passes even when `use`-site children are silently skipped.
- The unit-test helper `build_name_table` in `src/evaluator/engine/tests.rs` goes through the real `extract_name_table` path rather than inserting directly into the `HashMap`. New subroutine tests should follow the same convention so they exercise the production extraction code.
- `RecursionGuard::enter(context)?` (not manual increment/decrement) inside any new meta-type dispatch. Mutual recursion between subroutines is a real failure mode; the guard is the only correct way to surface it as `EvaluationError::RecursionLimitExceeded` instead of a stack overflow.

## Related

- [`logic-errors/raii-scope-guards-for-evaluator-context-save-restore.md`](../logic-errors/raii-scope-guards-for-evaluator-context-save-restore.md) -- the companion learning from PR #230's post-commit review pass. Documents the `SubroutineScope` RAII guard pattern that replaced the manual save/restore originally shipped in this doc's Use dispatch, plus the secondary UTF-8 byte-preservation fix in `format_magic_message` and a false-positive postmortem on `AtomicBool::swap` semantics.
- [`integration-issues/indirect-offset-parser-evaluator-sync.md`](indirect-offset-parser-evaluator-sync.md) -- closest sibling pattern: AST variant existed but was unreachable from `MagicDatabase::load_from_file()` until parser and evaluator were wired together. Different surface (offset syntax vs. directive dispatch) but same "parser-evaluator sync" shape. The earlier consolidation-review note has been resolved now that `indirect` has shipped: the two docs remain distinct (this doc covers dispatch architecture; that doc covers offset-resolution semantics).
- [`integration-issues/implementing-variable-width-typekind-variant.md`](implementing-variable-width-typekind-variant.md) -- same discipline around "adding a TypeKind variant that does not fit the fixed-shape `read_typed_value` pipeline"; relevant precedent for dispatch threading.
- [`logic-errors/indirect-offset-gnu-file-semantics.md`](../logic-errors/indirect-offset-gnu-file-semantics.md) -- precedent for honoring GNU `file` semantics in a meta-directive.
- [`developer-experience/rust-test-visibility-boundary.md`](../developer-experience/rust-test-visibility-boundary.md) -- the `pub(crate)` accessor pattern used for `RuleEnvironment` and `NameTable`.
- GOTCHAS.md S2.1 (TypeKind exhaustive matches), S3 (parser architecture -- now yields `ParsedMagic { rules, name_table }`), S3.8 (top-level sibling anchor chaining), S3.10 (subroutine base_offset semantics), S13 (evaluation configuration -- `use` recursion bounded by the existing recursion-depth guard), S14.2 (printf-style format substitution via `format_magic_message`).
- GitHub issues: #42 (driving), #54 (parent epic: Type System Expansion), #48 (third_party/tests compatibility baseline). PR: #230 (the landing PR; all six MetaType variants shipped).
