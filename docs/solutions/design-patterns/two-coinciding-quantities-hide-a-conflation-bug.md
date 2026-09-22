---
title: Two quantities that coincide in the common case hide a conflation bug until they diverge
date: 2026-09-21
category: design-patterns
module: evaluator (anchor advance and string bounds)
problem_type: design_pattern
component: evaluator
severity: high
applies_when:
  - Writing or changing a helper whose return value feeds both a slice index and a relative-offset anchor advance
  - Measuring a contract against the GNU `file` oracle and about to write the result into GOTCHAS.md as a general rule
  - Choosing fixtures for a string, encoding, or offset test
  - Reading an upstream libmagic field name (`m->vallen`, `rm_len`) and inferring what it counts
  - Reviewing a claim that sounds plausible, cites a real upstream line, and has a passing test behind it
resolution_type: design_pattern
related_components:
  - src/evaluator/types/search.rs
  - src/evaluator/types/string.rs
  - src/evaluator/types/mod.rs
  - GOTCHAS.md
tags:
  - anchor-advance
  - relative-offsets
  - string16
  - measurement
  - oracle-testing
  - test-fixtures
  - encoding
---

# Two quantities that coincide in the common case hide a conflation bug until they diverge

## Context

Issue #498 set out to bound the file-derived text a description renders and to correct the relative-offset anchor for flag-walked `string` and `search` matches. Along the way, four separate claims in this repository turned out to be wrong in the same way, and two of them had been written into GOTCHAS.md as settled contracts with passing tests behind them.

The shape is always the same. Two different measurements are *equal* for ordinary input, so code, tests, and prose conflate them freely. Nothing fails. The divergence only shows up for an input nobody chose as a fixture, and by then the wrong rule is documented as verified.

What makes this specifically hard to catch here: the common case is also the case you reach for when measuring against the real `file` binary. An ASCII payload and an unflagged pattern are the natural things to type into a hand-written magic file. So the oracle confirms the measurement, the measurement gets generalized, and the generalization is wrong.

## Guidance

**When two quantities coincide in your fixture, assume you have conflated them until you prove otherwise.** Before writing a measured result down as a general rule, name the two candidate quantities explicitly and construct the input where they differ. If you cannot think of such an input, say so in the note rather than implying you ruled it out.

**When one helper's value feeds two consumers that need different units, return both.** Do not return a single number and let each caller reinterpret it. Make the type carry the distinction so a future caller cannot pick the wrong one silently:

```rust
// Before: one number, two meanings. Callers guessed, and one guessed wrong.
pub(crate) fn any_value_string16_bound(decoded: &str, max: usize) -> usize

// After: the units are in the type.
pub(crate) struct String16Bound {
    /// Decoded code units kept -- what the relative-offset anchor advances by.
    pub(crate) units: usize,
    /// Byte index into the decoded UTF-8 string, for slicing it.
    pub(crate) byte_cut: usize,
}
```

**Choose fixtures that separate the quantities.** For a string bound, that means at least one non-ASCII character, so a byte count and a character count cannot be the same number. For an anchor advance, it means a flagged pattern (`/w`, `/W`, `/T`) where the walked byte count and the pattern's declared length differ.

**Do not infer what an upstream field counts from its name.** `m->vallen` in libmagic's `moffset()` is the magic rule's *declared* pattern length, not however many file bytes the comparator walked to confirm the match. GOTCHAS S2.6 cited that exact line correctly and still described the wrong quantity, because "vallen" reads like "the length of what matched."

## Why This Matters

A conflation bug of this shape is invisible to every normal signal. It compiles, the tests pass, the oracle agrees, the reviewer reads a plausible sentence citing a real source line, and the documentation makes the next contributor confident in the wrong rule. In this repo it survived long enough to be written into GOTCHAS.md twice.

The consequence is not cosmetic. The anchor advance is what a `>>&N` child rule resolves against, so an anchor that is wrong by a few bytes silently reclassifies files: the child either matches at the wrong offset or fails to match at all, and the description quietly loses a fragment. That failure looks like a magic-rule problem, not an arithmetic one, which is why it costs so much to track down.

There is also a documentation cost specific to this project. GOTCHAS.md exists to stop rediscovery, so a wrong entry is worse than a missing one: it actively spends the next contributor's trust. Both corrected entries now open by naming what the earlier revision claimed and why measurement disproved it, which is the convention to follow when you overturn one.

## When to Apply

Reach for this whenever a change touches the read/anchor pair, a string bound, or an encoding boundary. Concretely:

- A new `TypeKind` variant that reads a variable-width value, which needs both a read length and an anchor advance that must agree (see the read/anchor agreement invariant in `docs/solutions/security-issues/pstring-anchor-poisoning.md`).
- Any helper named `*_bound`, `*_consumed`, `*_len`, or `*_cut` whose result crosses between a decoded representation and the raw file buffer.
- Any claim about GNU `file` behavior you are about to record as general. The bar this project holds is a measurement against the real binary, and the fixture has to be the discriminating one for the measurement to mean anything.

## Examples

**Three instances found in one change, all the same shape.**

*Anchor advance for a `search` match.* "Match position plus walked byte count" and "match position plus the pattern's declared length" are the same number for an unflagged match, because `memchr::memmem` walks exactly `pattern.len()` bytes. They diverge only under `/w` and `/W`, where the file may hold zero, one, or many whitespace bytes where the pattern spells one. Measured against `file`-5.41: pattern `A\ B` (declared length 3) over `A   Bqz` resolves a relative-offset child to index 3, the declared length, not to the walked match-end at 5. Now `src/evaluator/types/search.rs` derives the advance from the pattern and never from `ScanHit.matched_len`.

*The same conflation on the flagged `string` path,* plus a paired claim that the anchor steps one byte past a trailing NUL. Both halves were wrong; the anchor lands **on** the NUL. Now GOTCHAS S6.8.

*Byte length versus code-unit count in `any_value_string16_bound`.* The helper measured both its 127-character cap and its anchor advance in UTF-8 bytes of the decoded string, where the contract is decoded UCS-2 code units. For ASCII these are identical, and every fixture in the suite was ASCII (`'Q'` repeated, `"AB\ncd"`), so CI could not see it. For 200 CJK characters the byte-measured cap yields 42 rendered characters instead of 127, and hands the anchor a number in the wrong unit entirely. Caught in code review, not by a test.

**The fixture that would have caught the third one:**

```rust
// ASCII cannot distinguish the two quantities -- this passes either way.
let decoded = "Q".repeat(300);

// Non-ASCII separates them: 200 code units, 600 UTF-8 bytes.
let decoded = "\u{4e2d}".repeat(200);
let bound = any_value_string16_bound(&decoded, usize::MAX);
assert_eq!(bound.units, 127, "anchor advance is in code units");
assert_eq!(bound.byte_cut, 127 * 3, "slice index is in UTF-8 bytes");
```

## Related

- `docs/solutions/security-issues/pstring-anchor-poisoning.md` — the read/anchor agreement invariant these bugs all live inside. That doc covers an attacker-controlled length poisoning the anchor; this one covers the anchor and the read disagreeing because two units were conflated. Same invariant, different cause.
- GOTCHAS S2.6 (search anchor), S6.8 (flagged-string anchor), S3.8 (the general buffer-derived rule and its exception), S14.8 (the description bound).
- Issue #498.
