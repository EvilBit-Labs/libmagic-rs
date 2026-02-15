#!/usr/bin/env bash
# Claude Code PreToolUse hook: ensure all git commits include DCO sign-off (-s)

CMD="${CLAUDE_TOOL_INPUT_command:-}"

# Only check commands that contain "git commit"
if ! echo "$CMD" | grep -q "git commit"; then
  exit 0
fi

# Allow if -s or --signoff is present anywhere in the command
if echo "$CMD" | grep -qE -- '(-s\b|--signoff)'; then
  exit 0
fi

echo "BLOCK: All commits must include DCO sign-off. Add the -s flag to your git commit command."
exit 2
