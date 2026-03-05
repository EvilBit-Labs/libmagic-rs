---
name: warn-emoji-in-prs
enabled: true
event: bash
pattern: gh\s+pr\s+create
action: warn
---

**WARNING: Emoji detected in PR creation command**

AGENTS.md prohibits emojis and non-ASCII characters in code, comments, or documentation.

Before creating this PR, verify:

- PR title contains NO emoji characters
- PR body contains NO emoji characters (no checkmarks, memo, sparkles, gear, shield, etc.)
- Use plain text markers instead: `[x]`, `*`, `-`, `>` for formatting

Strip all emoji from the --title and --body arguments before proceeding.
