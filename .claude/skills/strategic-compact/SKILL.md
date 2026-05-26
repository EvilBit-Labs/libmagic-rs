---
name: strategic-compact
description: Suggests manual context compaction at logical intervals to preserve context through task phases rather than arbitrary auto-compaction.
---

# Strategic Compact

Suggests manual `/compact` at strategic points in your workflow rather than relying on arbitrary auto-compaction.

## When to Activate

- Running long sessions that approach context limits
- Working on multi-phase tasks (research, plan, implement, test)
- Switching between unrelated tasks within the same session
- After completing a major milestone and starting new work
- When responses slow down or become less coherent

## Compaction Decision Guide

| Phase Transition | Compact? | Why |
|-----------------|----------|-----|
| Research / Planning | Yes | Research context is bulky; plan is the distilled output |
| Planning / Implementation | Yes | Plan is in TodoWrite or a file; free up context for code |
| Implementation / Testing | Maybe | Keep if tests reference recent code; compact if switching focus |
| Debugging / Next feature | Yes | Debug traces pollute context for unrelated work |
| Mid-implementation | No | Losing variable names, file paths, and partial state is costly |
| After a failed approach | Yes | Clear the dead-end reasoning before trying a new approach |

## What Survives Compaction

| Persists | Lost |
|----------|------|
| CLAUDE.md / AGENTS.md instructions | Intermediate reasoning and analysis |
| TodoWrite task list | File contents you previously read |
| Memory files (`~/.claude/memory/`) | Multi-step conversation context |
| Git state (commits, branches) | Tool call history and counts |
| Files on disk | Nuanced user preferences stated verbally |

## Best Practices

1. **Compact after planning** -- Once plan is finalized in TodoWrite, compact to start fresh
2. **Compact after debugging** -- Clear error-resolution context before continuing
3. **Don't compact mid-implementation** -- Preserve context for related changes
4. **Write before compacting** -- Save important context to files or memory before compacting
5. **Use `/compact` with a summary** -- Add a custom message: `/compact Focus on implementing indirect offsets next`

## Rust-Specific Considerations

- After reading large source modules (the evaluator and parser submodule trees can pull hundreds of lines into context across many files), compact before implementing
- After running `cargo test` / `cargo nextest run` with verbose output, compact if moving to a different module
- After architecture review or exploration, compact before starting refactoring work
