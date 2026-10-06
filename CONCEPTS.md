# Concepts

Shared domain vocabulary for this project — entities, named processes, and status concepts with project-specific meaning. Seeded with core domain vocabulary, then accretes as ce-compound and ce-compound-refresh process learnings; direct edits are fine. Glossary only, not a spec or catch-all.

## Classification passes

### Binary pass

The first top-level evaluation of a buffer, over the entries whose Entry test type is binary. If it produces a Message-bearing match, classification is finished and the description carries no Text class.

### Text pass

The second top-level evaluation, run only when the Binary pass printed nothing and the buffer classifies as text. It covers the entries whose Entry test type is text, sees only the Text window, and always ends the description with the Text class.

### Entry test type

Whether a top-level magic entry belongs to the Binary pass, the Text pass, or both, decided from the entry's first line alone: numeric types are binary, string types are text only when flagged for the text test, and pattern types follow their flags or whether the pattern itself looks like text. Entries that only steer control flow (default, use, indirect, clear) belong to neither and never run at top level.

### Admission

The per-entry check that decides whether an entry runs in the current pass: its Entry test type must match the pass, and a string-family entry flagged for one buffer kind is skipped on the other kind. Child rules and subroutine bodies are never subject to it.

## Text classification

### Text class

The encoding-level label GNU `file` and rmagic give a text buffer (ASCII, UTF-8 Unicode, ISO-8859, non-ISO extended ASCII). It is the whole description when no rule matched a text buffer, and the tail appended after a Text pass match, followed by line-terminator, long-line, escape, and overstrike qualifiers. *Avoid:* text-class tail, ascmagic suffix

### Text window

The prefix of a buffer that text classification, the qualifier scan, and the Text pass operate on: the read is first trimmed of trailing NULs, then capped at the encoding-inspection limit. A separate untrimmed view of the same prefix supplies the text-or-binary hint used by Admission. Rules in the Binary pass see the whole buffer.

### Message-bearing match

A rule match that contributes visible description text, directly or through a descendant. Only a message-bearing match ends a pass under stop-at-first-match or suppresses the Text pass; a match with no text still counts as a sibling match for default and clear directives.
