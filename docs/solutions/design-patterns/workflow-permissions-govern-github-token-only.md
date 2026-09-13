---
title: A workflow permissions block governs only GITHUB_TOKEN -- validating a narrowing when the affected step is conditionally gated
date: 2026-09-12
category: design-patterns
module: CI workflow permissions (.github/workflows/)
problem_type: design_pattern
component: tooling
severity: medium
applies_when:
  - Narrowing or adding a workflow-level `permissions:` block to a GitHub Actions workflow
  - 'A step that could be affected by the change is gated (`if: failure()`, `if: cancelled()`, an event or fork guard) so a green validation run never executes it'
  - An issue or PR justifies a permissions change by asserting the workflow "writes nothing back"
  - Resolving an OSSF Scorecard `actions/missing-workflow-permissions` or `TokenPermissions` finding
  - Reviewing a workflow step that authenticates with something other than the ambient GITHUB_TOKEN
resolution_type: config_change
related_components:
  - .github/workflows/compatibility.yml
  - .github/workflows/ci.yml
  - .github/workflows/release-plz.yml
tags:
  - github-actions
  - ossf-scorecard
  - least-privilege
  - workflow-permissions
  - upload-artifact
  - ci-cd
---

# A workflow permissions block governs only GITHUB_TOKEN -- validating a narrowing when the affected step is conditionally gated

## Context

Issue #456 flagged that `.github/workflows/compatibility.yml` declared no `permissions:` block at any level, so its `GITHUB_TOKEN` inherited the repository default instead of an explicit least-privilege floor (OSSF Scorecard alert #165, MEDIUM, `actions/missing-workflow-permissions`). The fix, merged in **PR #479**, was three lines at `compatibility.yml:13-14`:

```yaml
# .github/workflows/compatibility.yml -- top level, after `on:`
permissions:
  contents: read
```

The three-line diff is not the durable part. What is worth keeping is why the change was safe, because the issue's own stated reason was wrong and the obvious validation path -- a green CI run on the PR -- is structurally incapable of proving it.

This repo has now hit the underlying principle **twice**, independently. The first time is already recorded as an inline comment at `release-plz.yml:13`:

```yaml
# .github/workflows/release-plz.yml -- release-plz-release JOB level (not top level)
permissions:
  contents: read # writes use the App token, not GITHUB_TOKEN -- see #449
```

## Guidance

### 1. A `permissions:` block constrains the ambient GITHUB_TOKEN and nothing else

This is the generative principle behind both occurrences. A step that authenticates with a *different* credential is unaffected by the workflow's `permissions:` block:

| Step's credential                                               | Governed by `permissions:`? | Seen in this repo                                                                          |
| --------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------ |
| Ambient `GITHUB_TOKEN` (incl. `${{ secrets.GITHUB_TOKEN }}`)    | Yes                         | `release-plz-pr` job, which legitimately keeps `contents: write` (`release-plz.yml:51-52`) |
| A minted GitHub App token                                       | No                          | `release-plz-release` job -- issue #449                                                    |
| `ACTIONS_RUNTIME_TOKEN` (used by `actions/upload-artifact` v4+) | No                          | `compatibility.yml` and `ci.yml` artifact uploads -- issue #456                            |

The first two rows are verifiable in this tree. The third is **upstream action behavior and cannot be confirmed from this repository** -- nothing here vendors `actions/upload-artifact`'s source. Treat the mechanism as the *explanation* and the in-repo precedent in rule 4 as the *evidence*: the precedent holds whether or not the stated mechanism is the reason.

So "this workflow uploads an artifact" is **not** on its own a reason to grant more than `contents: read`.

### 2. Re-derive the risk argument from the workflow's actual steps

Issue #456 argued the change was low-risk because the workflow "writes nothing back to the repository: **no artifact upload**, no SARIF upload, no comment posting, no release step." That is false on this tree -- `compatibility.yml:66-74` is an artifact upload:

```yaml
# .github/workflows/compatibility.yml -- compatibility-tests job, steps:
  - name: Upload test results on failure
    if: failure()
    uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1
```

The conclusion survived, but only via rule 1: per upstream documentation, `upload-artifact` v4+ uses `ACTIONS_RUNTIME_TOKEN`, so `contents: read` does not constrain it. That mechanism is not verifiable from this tree -- which is exactly why rule 4's precedent, not the mechanism, is what makes the change safe to merge. PR #479 recorded this as an explicit "Correction to the issue's rationale" rather than quietly restating the incorrect premise.

### 3. A green run never validates a conditionally-gated step

`compatibility.yml`'s upload is `if: failure()`. A PR whose point is "this workflow still works" produces, when successful, a run in which that step **never executes**. The PR's own CI is blind to exactly the step in question.

> A green run never validates a conditionally-gated step. If a change could affect one, you need either a way to force the gate to fire, or independent evidence that does not depend on your run passing.

### 4. Substitute in-repo precedent, matched on the pinned SHA

The `coverage` job in `ci.yml` (lines 126-174) declares **no job-level `permissions`**, inherits the top-level `contents: read` (`ci.yml:10-11`), and uploads at `ci.yml:168-173` through the *identical pinned action SHA* -- `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a`, which is an upstream `actions/upload-artifact` commit (v7.0.1), not a commit in this repository. Its gate (`ci.yml:169`) is true for same-repo PRs, so it ran and passed on PR #479 itself -- live proof that this action, at this version, succeeds under this permission floor in this repo.

Match on that upstream pinned SHA, not the action name: a different major version can change the auth mechanism, which is the whole basis of the argument.

Do not cite the sibling `upload-coverage-rust` job (`ci.yml:175-181`) as the precedent. It declares job-level `contents: read` + `code-quality: write` because it uses a *different* action with a real scope requirement, and it is precedent for a different claim.

### 5. Workflow linting here is local-only

`actionlint` runs only as a pre-commit hook (`.pre-commit-config.yaml:37-44`, which also excludes `release.yml`); no CI job invokes it. For any workflow edit, local `just ci-check` is the only lint gate before the PR opens -- CI will not catch a workflow schema mistake the way it catches Rust errors. (auto memory [claude], confirmed against the tree this session.)

## Why This Matters

The failure mode is quiet and delayed: permissions get narrowed, the PR goes green, it merges, and weeks later a genuinely failing run needs to upload its diagnostic artifact and cannot -- discovered at the worst possible moment, because the gated path is the one nobody exercises until something has already broken.

The opposite failure is just as real and more common: a contributor who cannot prove the narrowing is safe defensively grants a scope that is not needed, and the least-privilege finding is "resolved" by a block that does not actually reduce privilege.

Rule 1 resolves both, and the blast radius argument sets the evidence bar honestly. Worst case here was a lost diagnostic artifact on an already-failing run, not a broken pipeline -- which is why precedent was an acceptable substitute for a directly exercised test.

## When to Apply

- Any workflow-level `permissions:` narrowing where a step further down might need a scope being removed.
- Any change to a step gated by `if: failure()`, `if: cancelled()`, or an event/fork guard that a normal validation run will not satisfy.
- Any change justified by "this workflow doesn't do X" -- re-derive that claim from the step list first.

The check each time:

1. Enumerate every step in the changed file that could be affected, not just the ones the issue names.
2. For each, identify which credential it actually authenticates with (rule 1).
3. If a step is gated such that this run will not trigger it, find a job elsewhere in the repo exercising the same mechanism under the same constraint on ordinary runs, and cite it precisely -- file, line range, and matching pinned SHA.
4. State in the PR body which parts were verified by direct run and which by precedent, so "CI is green" is not read as covering more than it does.

## Examples

**Validation basis, by job:**

| Job                                | Declares `permissions`?                            | Gating of upload step                       | Validated by                              |
| ---------------------------------- | -------------------------------------------------- | ------------------------------------------- | ----------------------------------------- |
| `compatibility.yml` (PR #479)      | Top-level `contents: read` (new)                   | `if: failure()` -- never runs on a green PR | Precedent, not this PR's run              |
| `ci.yml` -> `coverage`             | None; inherits top-level `contents: read`          | Runs on non-fork PRs and pushes             | Direct -- passed on PR #479               |
| `ci.yml` -> `upload-coverage-rust` | Job-level `contents: read` + `code-quality: write` | Runs when `coverage` succeeds               | Direct; different action, different claim |

**Out of scope, recorded so it is not re-investigated:**

- `.github/workflows/release.yml` is cargo-dist-generated. Any permissions or pinning finding against it goes through `dist generate`, never a hand-edit (GOTCHAS.md S12.3).
- Scorecard's `BinaryArtifacts` alerts on `third_party/tests/rpm-*.testfile` are vendored upstream GNU `file` fixtures -- expected, not actionable.
- Scorecard alerts clear on the next scheduled `scorecard.yml` run, not at merge. A still-open alert shortly after a fix is not evidence the fix failed.

## Related

- `docs/solutions/design-patterns/agent-facing-infrastructure-verify-against-canonical-contract.md` -- same family of failure: verifying against the canonical contract rather than an assumed one. That doc covers agent-facing infrastructure; this one covers CI permissions.
- `docs/solutions/integration-issues/release-plz-cargo-dist-tag-trigger-github-app-token.md` -- the other half of the `release-plz.yml` App-token story, where GITHUB_TOKEN-authored tag pushes fail to trigger downstream workflows.
- Issues #456 (this fix, closed) and #449 (closed) -- the two occurrences of the rule-1 principle.
