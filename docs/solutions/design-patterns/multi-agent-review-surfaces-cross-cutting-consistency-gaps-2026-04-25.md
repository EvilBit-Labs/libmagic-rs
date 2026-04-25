---
title: Multi-agent review surfaces cross-cutting consistency gaps that local tests miss
date: 2026-04-25
category: design-patterns
module: development-workflow/multi-agent-pr-review
problem_type: design_pattern
component: development_workflow
severity: high
applies_when:
  - extending shared AST types with new variants or fields
  - adding cross-type coercion policies in equality or comparison operators
  - introducing new error variants that participate in graceful-skip lists
  - modifying dual-purpose helpers (read/consume, parse/serialize) where partners must stay in lockstep
  - reviewing PRs that touch parser-evaluator boundaries
related_components:
  - evaluator-operators
  - evaluator-types
  - evaluator-engine
  - parser-grammar
  - error-handling
tags:
  - cross-cutting-consistency
  - multi-agent-review
  - dual-purpose-helper-sync
  - consistency-partners
  - silent-failure
---

# Multi-agent review surfaces cross-cutting consistency gaps that local tests miss

## Context

When a PR extends a feature that has **cross-cutting consistency partners** -- paired functions, doc-vs-code claims, struct-vs-rustdoc-example sets, or parse-vs-diagnostic paths -- local unit tests on the changed side can pass while the partner side silently drifts out of sync. The drift is invisible because each side's local contract remains internally consistent; only joint observation reveals the asymmetry.

PR #233 on libmagic-rs (`fix/loader-non-utf8-magic-files`) shipped 5 commits implementing magic-file syntax extensions and 3 load-bearing semantic bug fixes. All 1148 lib tests passed, all 10 sampled magic files loaded, `just ci-check` was green. After the PR opened, a multi-agent code review (`/pr-review-toolkit:review-pr` running 6 specialized agents in parallel) surfaced **5 critical cross-cutting consistency gaps** the original tests missed. All 5 fits a different pattern of partner drift; all were fixed in 5 follow-on commits. This doc captures the meta-pattern and the discovery mechanism so the next major AST extension catches its own gaps before merge.

This is **the third confirmed incident** of CI-invisible issues caught by 6-agent parallel review in libmagic-rs (session history). The earlier incidents:

- **Branch 39 (regex/search)** -- multi-agent review found `search_bytes_consumed` returning the full window size rather than match-end position (now GOTCHAS S2.6). The same dual-purpose-helper-sync class as gap 2 below. (session history)
- **Branch 38 / `todo_cleanup`** -- the first whole-codebase multi-agent sweep produced 81 findings, including documentation staleness, duplicate `EvaluationResult` types, and config enforcement gaps -- structurally similar to gaps 3 and 4 below. (session history)
- **Branch 42 (meta-types)** -- iterated through multiple review rounds, each catching distinct gaps (`MetaType::Indirect` not advancing the anchor; `use` rules not descending; the `String { max_length: None }` read-length / bytes_consumed mismatch). The pattern of "review surfaces a gap → fix → re-review surfaces another gap" was already documented as part of the meta-types delivery. (session history)

The pattern is now repeating reliably enough to deserve a design-pattern doc rather than another bug-track entry.

## Guidance

### The five consistency-partner classes

When extending a feature, identify which of these partner classes the change touches and update both sides together:

| Class               | Partner A (where change usually starts) | Partner B (where drift hides)                                  | How drift hides locally                                             |
| ------------------- | --------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------- |
| Cross-type policy   | `apply_equal` cross-type arm            | `compare_values` cross-type arm (used by `<`, `>`, `<=`, `>=`) | Equality test passes; ordering test wasn't written                  |
| Dual-purpose helper | `read_typed_value_with_pattern` arm     | `bytes_consumed_with_pattern` arm                              | Read returns correct value; anchor advance corrupts silently        |
| Doc-vs-code         | docstring promises variant `X`          | `EvaluationError` enum lacks `X` (helper uses fallback)        | Helper compiles via wrong variant; no test asserts variant identity |
| Struct-vs-rustdoc   | new field added to public struct        | rustdoc examples not updated                                   | `cargo test` passes; only `cargo test --doc` catches it             |
| Parse-vs-diagnostic | tolerance branch added                  | log emission missing or at wrong level                         | Parse succeeds; user has no signal at default log levels            |

The shared property: each pair has **one side that is exercised by the new tests and one side that is structurally adjacent but logically distinct**. Test discipline that targets the changed side will not exercise the partner.

### Why local unit tests miss the gap

Tests verify *contracts*, not *cross-contract agreement*. The PR's `apply_equal(Bytes, String)` test passed -- nothing in that test exercised `compare_values`. The PR's `read_typed_value_with_pattern` test for `Value::Bytes` passed -- nothing in that test exercised the matching `bytes_consumed_with_pattern` path. The PR's `apply_value_transform` overflow test passed -- nothing in that test asserted the error variant identity required by the engine's graceful-skip allowlist.

Each side's contract was internally consistent; the gap lived in the *relationship* between sides, not on either side alone.

### Multi-agent review as discovery mechanism

Running `/pr-review-toolkit:review-pr` dispatches 6 specialized agents in parallel, each looking at a different cross-section of the diff. The mechanism works because each agent is keyed to a specific anti-pattern that single-lens review misses:

| Agent                   | Cross-section                                                           | Caught in PR #233                                                                                              |
| ----------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `type-design-analyzer`  | View enum/struct as a unit; flag asymmetric variant handling            | Gap 1 (`apply_equal` cross-type but `compare_values` strict)                                                   |
| `comment-analyzer`      | Cross-reference docstrings against actual code paths                    | Gap 3 (`InvalidValueTransform` promised but not defined) and gap 2 (dual-purpose helper sync)                  |
| `silent-failure-hunter` | Audit `_ =>`, `unwrap_or`, `debug!`-vs-`warn!` levels, silent fallbacks | Gap 5 (5 silent parse-and-drop paths)                                                                          |
| `code-reviewer`         | Run `cargo test --doc`; check AGENTS.md/CLAUDE.md compliance            | Gap 4 (8 doctest failures)                                                                                     |
| `pr-test-analyzer`      | Behavioral coverage; property-test gaps                                 | Independent gap-4 detection; flagged property-test `arb_magic_rule.value_transform` always-`None` (still open) |
| `code-simplifier`       | Polish-pass; DRY violations; long files                                 | Surfaced 45-site `OffsetSpec::Indirect` builder pattern opportunity (still open)                               |

No single reviewer following one mental model would have caught all five. The cost of running the review is one command and ~2 minutes of wall-clock time; the cost of any single gap reaching main is much larger.

### Pre-PR self-review checklist

Before opening a PR that touches cross-cutting partners, audit each class:

- [ ] **Cross-type policy:** for every new `Value` arm in equality, did I add the matching arm in `compare_values`?
- [ ] **Dual-purpose helper:** for every new arm in `read_typed_value_with_pattern`, did I add it to `bytes_consumed_with_pattern`? Same applies to `calculate_default_strength`.
- [ ] **Error variant:** for every new error variant, did I decide whether it belongs in the engine's graceful-skip allowlist (`src/evaluator/engine/mod.rs`)? The default of `InternalError` is fail-the-evaluation; this is rarely correct for per-rule recoverable conditions (overflow, divide-by-zero, invalid offset).
- [ ] **Struct field:** for every new `MagicRule` field or `OffsetSpec` variant, did I update all rustdoc examples? `grep -rn 'MagicRule {' src/` is canonical. `cargo test --doc` is the local gate (NOT in `just ci-check` as of this writing).
- [ ] **Parse tolerance:** for every parser path that accepts new syntax, does it emit `warn!` or `info!` if the semantic side is unimplemented? Reference the tracking issue number in the log message.

When in doubt, run `/pr-review-toolkit:review-pr` -- it costs nothing and has a high hit rate.

### "If you change X, also check Y" cheatsheet

| If you change                               | Also check                                                                                                                                        |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apply_equal` cross-type arm                | `compare_values` cross-type arm, `apply_not_equal`                                                                                                |
| `read_typed_value_with_pattern` pattern arm | `bytes_consumed_with_pattern` pattern arm, `calculate_default_strength`                                                                           |
| `EvaluationError` variants                  | engine graceful-skip match arm in `src/evaluator/engine/mod.rs`                                                                                   |
| Required field on `MagicRule`/`OffsetSpec`  | `grep -rn "MagicRule {" src/` AND `cargo test --doc`                                                                                              |
| Parser tolerance path that drops syntax     | Add `warn!` (typo / unimplemented-with-correctness-impact) or `info!` (known unimplemented) with tracking-issue reference at point of consumption |
| New `Value` variant                         | GOTCHAS S2.3 update sites                                                                                                                         |
| New `TypeKind` variant                      | GOTCHAS S2.1 update sites + `is_string_family_type` if it's a string-family variant                                                               |
| New `MetaType` variant                      | GOTCHAS S2.11 update sites + evaluator dispatch decision                                                                                          |

## Why This Matters

### The 5 specific gaps as evidence

Each gap below would have produced a different class of silent user-visible failure if it had reached main. Each was caught by a different review cross-section.

#### Gap 1: trichotomy invariant broken between `apply_equal` and `compare_values`

PR #233 added cross-type byte-equality in `apply_equal`:

```rust
// src/evaluator/operators/equality.rs (PR #233)
match (left, right) {
    (Value::String(s), Value::Bytes(b)) | (Value::Bytes(b), Value::String(s)) => {
        return s.as_bytes() == b.as_slice();
    }
    _ => {}
}
compare_values(left, right) == Some(Ordering::Equal)
```

But `compare_values` (used by `<`, `>`, `<=`, `>=`) was still type-strict:

```rust
// src/evaluator/operators/comparison.rs (BEFORE)
match (left, right) {
    (Value::Uint(a), Value::Uint(b)) => Some(a.cmp(b)),
    // ... same-type arms ...
    _ => None,  // <-- cross-type Bytes/String falls here
}
```

Trichotomy broken: two byte-equal cross-type values compared as `==` but BOTH `<` and `>` returned `false`. magic(5) rules using `>` or `<` against `Value::Bytes` literals (e.g., `\177ELF`-style patterns) silently never fired even when equality with the same literal succeeded.

The fix (commit `6e00f72`) added matching cross-type arms to `compare_values`:

```rust
(Value::String(s), Value::Bytes(b)) => Some(s.as_bytes().cmp(b.as_slice())),
(Value::Bytes(b), Value::String(s)) => Some(b.as_slice().cmp(s.as_bytes())),
```

Regression test (`cross_type_string_bytes_ordering_is_byte_sequence`) asserts: for any cross-type pair, exactly one of `<`, `==`, `>` is true via the `apply_*` helpers.

Caught by `type-design-analyzer` viewing `Value` as a unit.

#### Gap 2: `bytes_consumed_with_pattern` divergence for `Value::Bytes` patterns

PR #233 extended `read_typed_value_with_pattern` for `TypeKind::String` to call `read_string_exact` when the comparison value is `Value::Bytes`:

```rust
// src/evaluator/types/mod.rs (read path, PR #233)
TypeKind::String { max_length } => match (max_length, pattern) {
    (Some(n), _) => read_string_exact(buffer, offset, *n),
    (None, Some(Value::String(p))) => read_string_exact(buffer, offset, p.len()),
    (None, Some(Value::Bytes(b))) => read_string_exact(buffer, offset, b.len()),  // NEW
    (None, _) => read_string(buffer, offset, None),
}
```

But the consume side `bytes_consumed_with_pattern` had no matching arm -- it fell through to `string_bytes_consumed` with `None`, which does a NUL-scan. On a NUL-free ELF header, that returned the full remaining buffer length. Any `&+N` child rule of an ELF parent resolved its relative offset hundreds of bytes past where the pattern actually ended.

This is the **second incident** of the dual-purpose-helper-sync bug class -- the first was branch 39's `search_bytes_consumed` returning window size instead of match-end (GOTCHAS S2.6) (session history). The original PR's learning doc (`magic-string-rule-matching-3-bug-fix-2026-04-25.md`) explicitly named "keep dual-purpose helpers in sync" as a prevention rule, but the same PR re-introduced the gap on a different helper triad.

The fix (commit `1590f55`):

```rust
// src/evaluator/types/mod.rs (consume path, NEW arm)
(None, Some(Value::Bytes(b))) => {
    let blen = b.len();
    offset
        .checked_add(blen)
        .map_or(0, |end| if end > buffer.len() { 0 } else { blen })
}
```

Regression test (`test_bytes_consumed_string_with_bytes_pattern_is_exact_length`): on a NUL-free 16-byte buffer with a 4-byte ELF-style pattern, consumed must be exactly 4, not 16.

Caught by `comment-analyzer` cross-referencing the dual-purpose-sync rule from the prequel learning doc against the actual code.

#### Gap 3: `EvaluationError::InvalidValueTransform` doc/code drift + broken graceful-skip

`apply_value_transform`'s docstring promised `EvaluationError::InvalidValueTransform`, but no such variant existed. The helper actually used `EvaluationError::internal_error`, which is NOT in the engine's graceful-skip allowlist. A single rule with `lequad*N` triggering overflow killed the whole evaluation instead of dropping the rule and continuing -- exact opposite of documented graceful-degradation behavior.

Two problems in one: (a) doc/code drift (compile error for anyone pattern-matching the documented variant), (b) actual runtime behavior wrong.

The fix (commit `27afcc9`) added the variant, updated the helper, and threaded it into all 3 graceful-skip arms:

```rust
// src/error.rs (NEW variant)
#[error("invalid value transform: {reason}")]
InvalidValueTransform { reason: String },
```

```rust
// src/evaluator/operators/mod.rs (helper now returns the correct variant)
fn invalid_transform(op: &str, value: &Value, operand: i64) -> EvaluationError {
    EvaluationError::InvalidValueTransform {
        reason: format!("{op}({operand}) failed on {value:?} (overflow or div-by-zero)"),
    }
}
```

```rust
// src/evaluator/engine/mod.rs (graceful-skip arm now includes it)
e @ LibmagicError::EvaluationError(
    EvaluationError::BufferOverrun { .. }
    | EvaluationError::InvalidOffset { .. }
    | EvaluationError::InvalidValueTransform { .. }    // NEW
    | EvaluationError::TypeReadError(...)
) => { /* skip rule, continue */ }
```

Regression test (`test_apply_value_transform_errors_use_invalid_value_transform_variant`) covers div-by-zero, mul overflow, signed underflow.

Caught by `comment-analyzer` cross-referencing docstrings against actual error classification.

#### Gap 4: 8 rustdoc doctest failures (CI-blocker)

PR #233 added `MagicRule::value_transform: Option<ValueTransform>` and `OffsetSpec::Indirect { base_relative, result_relative }`. All `src/` and `tests/` struct-literal call sites were updated, but **8 rustdoc examples** were missed. `cargo test --doc` failed with `E0063: missing field`.

Affected sites: `src/parser/ast.rs:227`, `src/evaluator/engine/mod.rs:159, 569, 1152`, `src/evaluator/strength.rs:49, 273, 315, 404`.

This was a CI-blocker. `just ci-check` does NOT include `cargo test --doc` (it runs `cargo test` for unit tests but the doctest step is separate). The PR's iterative-fix cycle had passed `just ci-check` repeatedly without exercising the doctest path; the gap only surfaced when upstream CI ran the doctest step.

The same class of recurrence happened on branch 39 (session history) -- session `eb76585a` added `cargo test --doc` to a verification checklist after discovering a stale doc example. The lesson did not propagate into `just ci-check`.

The fix (commit `a419aa7`) updated all 8 examples with default values for the new fields. Doctest count: 184 → 193 passing.

Caught by `code-reviewer` actually running `cargo test --doc`.

#### Gap 5: silent-failure pattern across 5 parse-and-drop paths

PR #233 added several "tolerance" paths that consumed user syntax silently:

- Unknown `!:` directives (typos like `!:mim`, plus known-unimplemented `!:mime`/`!:ext`/`!:apple`) used `debug!` -- invisible at default log levels.
- `,` accepted as `.` synonym in indirect-offset separator (msdos:638 typo) -- silent.
- `\^use` endian-flip prefix consumed but semantic flip not implemented (issue #236) -- silent.
- string flag suffixes `/c`/`/w`/etc. parsed-and-dropped (issue #234) -- silent.
- search flag suffixes `/s`/`/c`/etc. parsed-and-dropped (issue #235) -- silent.

Users debugging "why doesn't `string/c FOO` match `foo`?" had no breadcrumb. The PR's commit messages documented these as intentional limitations, but at runtime there was no signal.

The fix (commit `a33756c`):

| Path                              | Old level | New level           | Rationale                                                   |
| --------------------------------- | --------- | ------------------- | ----------------------------------------------------------- |
| `!:mime`, `!:ext`, `!:apple`      | `debug!`  | `info!`             | Known unimplemented, user opted in by writing the directive |
| Unknown `!:` directive            | `debug!`  | `warn!`             | Probable typo                                               |
| `,` as `.` separator              | none      | `warn!`             | Tolerated typo, matches GNU `file`'s diagnostic             |
| `\^use` endian-flip prefix        | none      | `warn!` (refs #236) | Parsed but flip not implemented                             |
| string `/c`/`/W`/etc. flag suffix | none      | `warn!` (refs #234) | Parsed but semantic dropped                                 |
| search `/s`/`/c`/etc. flag suffix | none      | `warn!` (refs #235) | Parsed but semantic dropped                                 |

Each warning includes the relevant tracking issue number so users hitting the limitation have a direct link to implementation status, not just a silent miss.

Bonus fix folded in (I-3 from `code-reviewer`): `TypeKind::String16` was missing from `is_string_family_type` in `parse_magic_rule`, so a programmatically-constructed `lestring16` rule with a bareword value would fail strict `parse_value` instead of falling through to `parse_bare_string_value`.

Caught by `silent-failure-hunter` auditing `debug!`-vs-`warn!`-vs-`info!` levels.

## When to Apply

Run `/pr-review-toolkit:review-pr` (or invoke the equivalent multi-agent review for your harness) before merging when the diff includes any of:

- New `Value` enum variant or new arm in `apply_equal`/`compare_values`
- New `TypeKind` variant or new arm in `read_typed_value_with_pattern`/`bytes_consumed_with_pattern`/`calculate_default_strength`
- New `EvaluationError` or `TypeReadError` variant
- New required field on `MagicRule`, `OffsetSpec`, or any other public AST type with rustdoc examples
- New parser branch that consumes user syntax without implementing its full semantics
- Cross-cutting refactor that touches both the parser and evaluator layers

Single-agent review (or solo human review) misses the gaps because no single mental model covers all 5 cross-sections.

## Examples

The 5 fixes documented in this learning are each before/after worked examples:

1. **Cross-type policy (Fix 1)**: `apply_equal` byte-equality → `compare_values` byte-ordering (commit `6e00f72`).
2. **Dual-purpose helper sync (Fix 2)**: `read_typed_value_with_pattern` `Value::Bytes` arm → `bytes_consumed_with_pattern` `Value::Bytes` arm (commit `1590f55`).
3. **Error variant + graceful-skip (Fix 3)**: docstring promise → variant exists → graceful-skip allowlist updated (commit `27afcc9`).
4. **Struct field + rustdoc (Fix 4)**: new field → grep all examples + run `cargo test --doc` (commit `a419aa7`).
5. **Parse tolerance + diagnostic (Fix 5)**: parse-and-drop branch → `warn!` with tracking-issue reference (commit `a33756c`).

## Related

- [`docs/solutions/logic-errors/magic-string-rule-matching-3-bug-fix-2026-04-25.md`](../logic-errors/magic-string-rule-matching-3-bug-fix-2026-04-25.md) -- **prequel.** PR #233's original 3-bug fix introduced the cross-type equality policy, NUL-safe reads, and `read_string_exact`/`read_typed_value_with_pattern` dispatch. This doc is the follow-on for the 5 cross-cutting consistency gaps the same PR introduced or missed. The prequel's "Prevention" section anticipated this doc's existence ("any new `Value` variant carrying byte data should extend this cross-equality" -- now extended in `compare_values` per Fix 1).
- [`docs/solutions/developer-experience/multi-agent-pr-review-fixes.md`](../developer-experience/multi-agent-pr-review-fixes.md) -- **methodology twin.** Same 6-agent parallel review pattern on PR #212. This is the third confirmed incident of CI-invisible issues caught by multi-agent review; that doc is the second.
- [`docs/solutions/security-issues/pstring-anchor-poisoning.md`](../security-issues/pstring-anchor-poisoning.md) -- **partner-drift sibling.** Codifies the "keep dual-purpose helpers in sync" rule for `read_pstring` \<-> `pstring_bytes_consumed`. Gap 2 is the same class applied to the `String` + `Value::Bytes` triad.
- [`docs/solutions/integration-issues/implementing-variable-width-typekind-variant.md`](../integration-issues/implementing-variable-width-typekind-variant.md) -- documents `bytes_consumed_with_pattern` exhaustive-match invariant. Gap 2 manifests this rule.
- [`GOTCHAS.md`](../../../GOTCHAS.md) S2.1 (`TypeKind` exhaustive matches) -- list of update sites; consider adding `is_string_family_type` per the bonus fix in Gap 5.
- [`GOTCHAS.md`](../../../GOTCHAS.md) S2.3 (`Value` exhaustive matches) -- documents the cross-type equality policy from the prequel; this doc extends to the trichotomy parallel for `compare_values`.
- [`GOTCHAS.md`](../../../GOTCHAS.md) S2.6 (search anchor advance is match-end, not window-end) -- the original dual-purpose-helper-sync incident.
- GitHub Epic [#54](https://github.com/EvilBit-Labs/libmagic-rs/issues/54) -- type-system expansion umbrella.
- GitHub [#47](https://github.com/EvilBit-Labs/libmagic-rs/issues/47) (parser warnings for skipped invalid magic rules) -- Gap 5's `warn!`/`info!` logging is concrete progress on this issue.
- GitHub [#234](https://github.com/EvilBit-Labs/libmagic-rs/issues/234), [#235](https://github.com/EvilBit-Labs/libmagic-rs/issues/235), [#236](https://github.com/EvilBit-Labs/libmagic-rs/issues/236) -- the open implementation-side issues that Gap 5's diagnostics now reference.
