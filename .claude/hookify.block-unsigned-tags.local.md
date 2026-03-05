---
name: block-unsigned-tags
enabled: true
event: bash
conditions:
  - field: command
    operator: regex_match
    pattern: git\s+tag\s+(?!.*-s)
action: block
---

**BLOCKED: Unsigned tag detected**

All tags must be GPG-signed. Use `git tag -s` instead.

If signing fails, STOP and troubleshoot. Never push unsigned tags.
