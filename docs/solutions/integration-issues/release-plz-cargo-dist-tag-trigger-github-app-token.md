---
title: release-plz tags pushed with GITHUB_TOKEN never trigger cargo-dist (no GitHub Releases)
date: 2026-07-26
category: integration-issues
module: release pipeline (release-plz + cargo-dist)
problem_type: integration_issue
component: tooling
symptoms:
  - GitHub Releases stopped being produced after v0.1.1 while crates.io/tags continued through v0.11.1
  - git tags v0.6.0 through v0.11.1 exist but have no corresponding GitHub Release or rmagic artifacts
  - every historical run of the Release workflow (release.yml) is pull_request-triggered; zero tag-triggered runs
root_cause: config_error
resolution_type: config_change
severity: high
tags: [release-plz, cargo-dist, github-actions, github-token, github-app-token, ci-cd]
related_components: [.github/workflows/release-plz.yml, .github/workflows/release.yml, release-plz.toml]
---

# release-plz tags pushed with GITHUB_TOKEN never trigger cargo-dist (no GitHub Releases)

## Problem

The two-tool release pipeline is wired correctly in intent — release-plz creates and pushes the version tag, and cargo-dist's `release.yml` (`on: push: tags`) is supposed to catch that tag and cut the GitHub Release with signed `rmagic` artifacts. The handoff silently broke: no GitHub Release has been produced since **v0.1.1**, leaving nine tagged versions (**v0.6.0–v0.11.1**) with no Release and no artifacts — the exact artifacts `docs/src/release-verification.md` tells users to verify.

## Symptoms

- GitHub Releases list ends at `v0.1.1` (2026-03-01) while `git tag` and crates.io go through `v0.11.1`.
- `gh run list --workflow=release.yml` shows **only `pull_request`-triggered runs** — the `on: push: tags` path has never fired.
- cargo-dist jobs (`announce`, `host`, `build-*-artifacts`) show as `skipping` on the PR/push runs because they only do real work on a tag event that never arrives.

## What Didn't Work

- **Assuming the trigger pattern was wrong.** `release.yml` has a correct `on: push: tags: ['**[0-9]+.[0-9]+.[0-9]+*']` — the tag names match it. The trigger definition was never the problem.
- **Assuming release-plz was misconfigured.** `release-plz.toml` is correct: `git_release_enable = false` + `git_tag_enable = true` deliberately delegates the GitHub Release to cargo-dist. The config expresses the right intent.

## Solution

Root cause: the `release-plz-release` job pushed the tag using the default `GITHUB_TOKEN` (`GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`). **GitHub does not emit `push`/`create` workflow-trigger events for refs pushed with `GITHUB_TOKEN`** (a deliberate recursion guard), so the tag push never triggers `release.yml`.

The fix (opened in **PR #411**, unmerged as of this writing) mints a GitHub App token in the `release-plz-release` job and uses it for both the checkout credential (the token the tag push authenticates with) and release-plz's `GITHUB_TOKEN` env. A tag pushed with an App token *does* trigger downstream workflows.

```yaml
# .github/workflows/release-plz.yml — release-plz-release job
steps:
  - name: Generate GitHub App token
    id: app-token
    uses: 
      actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1     # v3.2.0
    with:
      app-id: ${{ secrets.RELEASE_PLZ_APP_ID }}
      private-key: ${{ secrets.RELEASE_PLZ_APP_PRIVATE_KEY }}
  - name: Checkout repository
    uses: actions/checkout@... # v7.0.1
    with:
      fetch-depth: 0
      persist-credentials: true
      token: ${{ steps.app-token.outputs.token }}   # tag push uses this
  # ... mise ...
  - name: Run release-plz
    uses: release-plz/action@... # v0.5.131
    with:
      command: release
    env:
      GITHUB_TOKEN: ${{ steps.app-token.outputs.token }}
```

Out-of-band setup (maintainer): register a GitHub App with **Contents: write** + **Pull requests: write**, install it on the repo, and add secrets `RELEASE_PLZ_APP_ID` (the App ID) and `RELEASE_PLZ_APP_PRIVATE_KEY` (the full `.pem`). Auth uses App ID + private key (JWT flow); the App's Client ID / Client secret are **not** used (those are for OAuth user-authorization, which this does not do).

The `release-plz-pr` job was intentionally **left on `GITHUB_TOKEN`** so its bot author is unchanged and the `.mergify.yml` author-matching auto-merge exemption for release PRs keeps working.

## Why This Works

The break was never in the pipeline's structure — only in the credential used for the one cross-workflow trigger. GitHub's recursion guard suppresses workflow events for `GITHUB_TOKEN`-authored ref pushes specifically to stop workflows from endlessly triggering each other. A GitHub App installation token is a distinct identity that is *not* subject to that guard, so the tag push it authors emits the `push` event `release.yml` waits for, reconnecting release-plz → cargo-dist.

This is the same root cause already noted in project memory ("GITHUB_TOKEN-triggered pushes suppress workflow events") — here it manifested as a missing *downstream workflow trigger* rather than a missing CI run on a bot PR.

## Prevention

- **When one workflow must trigger another via a git push (tag or branch), never use `GITHUB_TOKEN` for that push.** Use a GitHub App token (preferred — not user-tied, no expiry, better Scorecard/OSSF posture) or a PAT. This applies to any release-plz + cargo-dist (or tag-triggered release) setup.
- **SHA-pin the token-minting action** (`create-github-app-token@<sha> # vX.Y.Z`) to satisfy Scorecard's pinned-dependencies check, matching every other pinned action in the repo.
- **Verification signal:** after the fix, `gh run list --workflow=release.yml` should show a run with `event: push` (tag-triggered). Today every run there is `pull_request` — that asymmetry is itself the smoke test that the trigger is broken.
- **Backfill is separate:** v0.6.0–v0.11.1 will not retroactively get Releases; either re-push each tag with the new token or do a local `dist build` + `gh release create`, or simply start fresh from the next release.

## Related Issues

- PR #411 — the workflow change (open/unmerged as of 2026-07-26).
- Project memory: `ci_dependabot_cargo_dist_release_yml.md` (release.yml is cargo-dist-generated — do not hand-edit it; this fix touches `release-plz.yml`, not `release.yml`).
