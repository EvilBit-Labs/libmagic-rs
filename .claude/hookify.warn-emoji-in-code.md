---
name: warn-emoji-in-code
enabled: true
event: file
conditions:
  - field: new_text
    operator: regex_match
    pattern: "[\U0001F300-\U0001F9FF\u2600-\u26FF\u2700-\u27BF\U0001FA00-\U0001FA6F\U0001FA70-\U0001FAFF]"
---

**Emoji detected in code or documentation.**

Project guidelines (AGENTS.md "Emoji Usage") prohibit emojis and non-ASCII characters in code, comments, and documentation -- **except** for typographic symbols like em-dash (`--`, U+2014), en-dash (U+2013), and similar punctuation, which are allowed. The regex above intentionally targets the pictographic-emoji ranges and does not match em-dash / en-dash.

**Exception:** If this code is specifically handling or processing emoji/non-ASCII characters (e.g., test cases for Unicode handling, regex byte-match fixtures), this warning can be disregarded.

If this is not emoji-processing code, remove the emoji and use plain text instead.
