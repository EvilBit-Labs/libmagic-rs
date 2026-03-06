---
name: block-unsigned-commits
enabled: true
event: bash
conditions:
  - field: command
    operator: regex_match
    pattern: git\s+commit\s+(?!.*-s)
action: block
---

**BLOCKED: Missing DCO sign-off**

All commits must include the DCO sign-off flag. Use `git commit -s` on every commit.

This is enforced by the GitHub DCO App and is required by project policy (AGENTS.md, CONTRIBUTING.md).
