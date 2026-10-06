---
title: Porting a libmagic subsystem means measuring its limits and pre-processing, not only its output strings
date: 2026-10-05
category: design-patterns
module: output/ascmagic and evaluator (two-pass text classification)
problem_type: design_pattern
component: evaluator
severity: high
applies_when:
  - Porting or extending a GNU file / libmagic subsystem (ascmagic, encoding, softmagic passes, structural detectors)
  - A plan was built from measured output strings plus a read of the upstream C and looks complete
  - A review finding asserts an upstream behavior and must be confirmed or refuted before it becomes a fix
tags: [libmagic-parity, oracle-measurement, upstream-c, size-limits, preprocessing, cross-model-review, ascmagic]
---

# Porting a libmagic subsystem means measuring its limits and pre-processing, not only its output strings

## Context

Issue #382 ported GNU `file`'s two-pass text classification (the `, ASCII text` tail). The plan was unusually well grounded: every description string was measured on two `file` versions (5.41 on macOS, 5.45 on Linux) with inline magic, and the relevant C (`funcs.c`, `softmagic.c`, `apprentice.c`, `ascmagic.c`, `encoding.c`) was read end to end. It passed a six-persona document review. The implementation matched every measured string, the full-DB differential showed 280 rows improving and none regressing, and the suite was green.

It was still wrong in two ways that no fixture had exercised, because every fixture was a short, unpadded buffer:

- `file_ascmagic` trims trailing NULs (`trim_nuls`, keeping one byte) before `file_encoding` classifies the buffer, so `QQTEXT6\n\0\0\0` is text to `file` and was `data` to rmagic.
- `file_encoding` inspects at most `FILE_ENCODING_MAX` (64 KiB) of the `FILE_BYTES_MAX` (1 MiB) read, and the text pass evaluates only that window. A 70000-byte line is `very long lines (65536)` to `file`; rmagic said `(70000)`. A text-typed rule hit at offset 70000 is invisible to `file`'s text pass.

The first gap was raised by the cross-model code review as a P2 with a one-line fix. Measuring it with padded and oversized buffers is what exposed the second, which no reviewer had named. Two other peer findings from the same review (`string/bt` should run in both passes; Ctrl-Z is a text byte) were refuted in minutes by reading the upstream tables (`set_test_type` gives strings TEXTTEST iff `/t`; `text_chars` marks only ESC in the 0x1X row) and would have introduced real divergences had they been applied on trust.

## Guidance

When porting a libmagic code path, treat the output strings as the last thing to verify, not the first. Before the plan is called complete:

1. **Enumerate every size constant and pre-processing step on the path**, from the C, as a checklist: read caps (`FILE_BYTES_MAX`), inspection caps (`FILE_ENCODING_MAX`, `FILE_REGEX_MAX`), trims (`trim_nuls`), transcoding (`encode_utf8`, the UTF-16 odd-byte adjustment), NUL-termination tricks (`copy[--slen] = '\0'` in the regex check), and the early returns (`nbytes <= 1`). Each one is a behavior a reader of the output strings cannot see.
2. **Measure each item with a fixture built to cross it**: a buffer longer than the cap with the discriminating content on the far side, a buffer padded with the trimmed byte, a buffer ending exactly at the boundary. Keep the inline magic trivial so the limit, not the rule, is what the fixture tests. Record the `file` output next to rmagic's in the test row.
3. **Separate the windows the C separates.** `file_buffer`'s `looks_text` hint (untrimmed) and `file_ascmagic`'s classification (trimmed) are different views of the same buffer; conflating them reproduces neither. In rmagic this is `text_window()` in `src/output/ascmagic.rs`, which returns the trimmed 64 KiB `scan` window, the untrimmed 64 KiB `hint` window, and the trimmed read length for the trailing-CR rule.
4. **Verify every review finding about upstream behavior against the C table or the host binary before acting**, in either direction. A confident finding with a concrete fix is still a claim; the cost of checking it is minutes, the cost of a wrong "fix" is a measured regression.
5. **Record the measured model in GOTCHAS, with the commands**, so the next port of the same subsystem (#524's encodings, #377's structural detectors) starts from the limits, not from the strings.

## Why This Matters

Output strings are the visible 10 percent of a libmagic subsystem. The limits and pre-processing decide which bytes those strings are computed over, and they only matter for inputs the typical fixture never has: padded files, files over 64 KiB, files whose signature sits past a cap. Those inputs exist in every real corpus, and a divergence there is a detection result, which ADR-0001 makes binding. A plan that measures strings on two versions can look exhaustive while missing them entirely, because nothing in the measurement forces a fixture across a boundary.

Both halves of the discipline are needed. Measuring surfaced a real gap the review had found and a second one it had not; checking the C refuted two findings that measurement would have had to disprove the slow way. Without the C, the Ctrl-Z finding would have become a one-line table change that is wrong for every file with a 0x1A byte.

## When to Apply

- Any port of a `file` subsystem or a change to an existing one: `ascmagic`, `encoding`, the softmagic pass structure, `file_is_json` and friends, compression handling.
- Any plan whose Test Plan rows are all short inline buffers. Add at least one row per constant on the path that crosses it.
- Any code-review finding, from a person or a model, that states what upstream does. Confirm or refute it against the source table or a measurement before it reaches the apply step.
- Any time a differential over real files comes back clean: real corpora rarely contain padded or boundary-straddling cases, so a clean differential does not retire the boundary fixtures.

## Examples

Fixtures that crossed the limits (host `file` 5.41, inline magic `0 regex QQTEXT6 TOPMSG6` and `0 search/100000 QQTEXTS SMSG`):

```text
printf 'QQTEXT6\n\0\0\0'          file: TOPMSG6, ASCII text          before fix: data
printf 'ab\0'                     file: ASCII text, with no line terminators
a*70000 + LF                      file: ASCII text, with very long lines (65536), with no line terminators
                                  before fix: ..., with very long lines (70000)
x*70000 + QQTEXTS + LF            file: ASCII text, with very long lines (65536), with no line terminators
                                  before fix: SMSG, ASCII text, with very long lines (70007)
x*60000 + QQTEXTS + LF            file: SMSG, ASCII text, with very long lines (60007)
```

The `x*70000` row is the one that proves the text pass itself, not just the qualifier scan, stops at 64 KiB (the LF past the window is invisible, so `file` reports no line terminators). These rows live in `tests/text_class_tail_tests.rs` as `trailing_nuls_and_the_64k_encoding_window_match_gnu_file`.

Refuting a finding from the table instead of the binary: the review claimed `string/bt` must run in both passes. `apprentice.c::set_test_type` reads, for `FILE_STRING`, `if (mstart->str_flags & STRING_TEXTTEST) mstart->flag |= TEXTTEST; else mstart->flag |= BINTEST;`. There is no `/b` branch for strings, so the existing `(!flags.text_test, flags.text_test)` mapping in `src/evaluator/test_type.rs` was already right and the finding was dropped.

The measured model, including which window feeds which decision, is GOTCHAS S13.7.

## Related

- GOTCHAS.md S13.7 (two-pass text classification, the trimmed 64 KiB window, the regex last-byte quirk)
- docs/solutions/design-patterns/two-coinciding-quantities-hide-a-conflation-bug.md (the same discipline for two quantities that coincide on small fixtures)
- docs/solutions/design-patterns/multi-agent-review-surfaces-cross-cutting-consistency-gaps-2026-04-25.md (review as the discovery mechanism; this doc adds the verify-before-apply step)
- docs/solutions/integration-issues/loading-real-system-magic-db-getstr-rawbyte-graceful-skip-2026-07-17.md (an earlier port done by reading the C and measuring)
- Issues #382 (the port), #524 (encodings and BOM, next port of this subsystem), #525 (regex last-byte quirk found by measuring), #377 (structural detectors, next ports)
