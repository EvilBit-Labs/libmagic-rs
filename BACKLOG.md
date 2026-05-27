# Backlog

Living, dependency-ordered view of open work. Items are grouped by milestone and sequenced so each can begin without waiting on later items in the same group. For the higher-level roadmap, see [ROADMAP.md](ROADMAP.md). For canonical issue state, see [GitHub issues](https://github.com/EvilBit-Labs/libmagic-rs/issues).

## Legend

- **Blocks**: this issue must land before downstream work can start.
- **Blocked by**: cannot start until the listed prerequisite is closed.
- **Dep'd by**: another open issue depends on this one; closing it unblocks them.
- Priority comes from the GitHub `priority:*` label.

## v0.3.0 — Advanced Features

Type-system expansion, advanced offset resolution, and format-detection breadth. Tracked under epics [#54](https://github.com/EvilBit-Labs/libmagic-rs/issues/54) (Type System Expansion) and [#55](https://github.com/EvilBit-Labs/libmagic-rs/issues/55) (Offset Resolution).

### Critical path — flag semantics for archive parity

| Order | Issue                                                          | Title                                                            | Priority | Notes                                                                                         |
| ----- | -------------------------------------------------------------- | ---------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| 1     | [#234](https://github.com/EvilBit-Labs/libmagic-rs/issues/234) | String-type flag semantics (`/c /C /w /W /B /b /t /T`)           | normal   | Parse-and-drop landed in #233; semantics deferred. Blocks `string/b` archive rules under #51. |
| 2     | [#235](https://github.com/EvilBit-Labs/libmagic-rs/issues/235) | Search-type flag semantics (`/s` start-anchor, `/b` binary, ...) | normal   | Parse-and-drop landed in #233. Blocks EPUB-vs-generic-ZIP discrimination under #51.           |

### Parallel — independent feature work

| Order | Issue                                                          | Title                                               | Priority | Notes                                                                             |
| ----- | -------------------------------------------------------------- | --------------------------------------------------- | -------- | --------------------------------------------------------------------------------- |
| 3     | [#236](https://github.com/EvilBit-Labs/libmagic-rs/issues/236) | Endian-flip `use \^name` subroutine invocation      | —        | Independent of #51 archive work.                                                  |
| 4     | [#237](https://github.com/EvilBit-Labs/libmagic-rs/issues/237) | ID3 synchsafe `i` / `I` indirect pointer specifiers | —        | Placeholder `Long` mapping landed in #233. Needed for MP3 inner-stream detection. |
| 5     | [#106](https://github.com/EvilBit-Labs/libmagic-rs/issues/106) | Evaluate and implement fuzzing tests                | —        | Best after complex parsing paths land; size:M.                                    |

### Format detection — #51 and its dependents

| Order | Issue                                                        | Title                                                             | Priority | Notes                                                                                                                                                                                                                              |
| ----- | ------------------------------------------------------------ | ----------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 6     | [#51](https://github.com/EvilBit-Labs/libmagic-rs/issues/51) | Evaluator: ZIP content inspection for compound document detection | low      | **Blocked by #234, #235** for full parity. Interim PR can land OOXML/OASIS/HWPX core (which do not depend on `/b` flags) ahead of #234/#235. Also delivers `!:mime` / `!:ext` directive evaluation — no prior issue tracked these. |

## v0.4.0 — API and UX Polish

Tracked under epic [#56](https://github.com/EvilBit-Labs/libmagic-rs/issues/56) (Core Flow Spec Compliance).

### Spec-compliance items (epic #56)

| Order | Issue                                                        | Title                                                   | Priority | Notes                      |
| ----- | ------------------------------------------------------------ | ------------------------------------------------------- | -------- | -------------------------- |
| 1     | [#47](https://github.com/EvilBit-Labs/libmagic-rs/issues/47) | Parser: report warnings for skipped invalid magic rules | normal   | Flow 11 (Corrupted Files). |
| 2     | [#46](https://github.com/EvilBit-Labs/libmagic-rs/issues/46) | Output: include metadata object in JSON output          | normal   | Flow 8 (JSON Output).      |
| 3     | [#49](https://github.com/EvilBit-Labs/libmagic-rs/issues/49) | CLI: improve magic-file-not-found error message         | low      | Flow 3 (Magic Discovery).  |

### Doc cleanup

| Order | Issue                                                          | Title                                                                       | Priority | Notes                                         |
| ----- | -------------------------------------------------------------- | --------------------------------------------------------------------------- | -------- | --------------------------------------------- |
| 4     | [#287](https://github.com/EvilBit-Labs/libmagic-rs/issues/287) | Docs: AGENTS.md/GOTCHAS.md claim `&+N` parsing is TODO but it's implemented | low      | size:XS. Can be rolled into the first #51 PR. |

## v1.0.0 — Production Release

Tracked under epic [#57](https://github.com/EvilBit-Labs/libmagic-rs/issues/57) (Compatibility Validation & v1.0).

### API stability (must precede broad compatibility work)

| Order | Issue                                                        | Title                                                    | Priority | Notes                                                                      |
| ----- | ------------------------------------------------------------ | -------------------------------------------------------- | -------- | -------------------------------------------------------------------------- |
| 1     | [#45](https://github.com/EvilBit-Labs/libmagic-rs/issues/45) | `MagicDatabase` builder pattern                          | normal   | Foundational API surface; pinned on the v0.6.0 roadmap entry in AGENTS.md. |
| 2     | [#44](https://github.com/EvilBit-Labs/libmagic-rs/issues/44) | Return partial results on timeout instead of fatal error | high/low | Flow 12 (Timeout). Mixed priority labels — treat as high.                  |

### Compatibility validation

| Order | Issue                                                        | Title                                                      | Priority | Notes                                                          |
| ----- | ------------------------------------------------------------ | ---------------------------------------------------------- | -------- | -------------------------------------------------------------- |
| 3     | [#48](https://github.com/EvilBit-Labs/libmagic-rs/issues/48) | Establish baseline and track `third_party/tests` pass rate | normal   | Should follow #51 so the baseline reflects ZIP/OOXML coverage. |

### Format-detection breadth (post-#51)

| Order | Issue                                                          | Title                                                              | Priority | Notes                                                                                               |
| ----- | -------------------------------------------------------------- | ------------------------------------------------------------------ | -------- | --------------------------------------------------------------------------------------------------- |
| 4     | [#285](https://github.com/EvilBit-Labs/libmagic-rs/issues/285) | TAR content inspection for GLEP 78 (gpkg) and compound TAR formats | low      | Mirror of #51 against TAR containers. `gpkg-1-zst` fixture.                                         |
| 5     | [#286](https://github.com/EvilBit-Labs/libmagic-rs/issues/286) | Surface `!:apple` Mac type/creator directive metadata              | low      | After `!:mime` / `!:ext` land via #51, `!:apple` is the last common directive still parse-and-drop. |

## Dependency graph (compact)

```text
#234 (string flags) ─┐
                     ├─→ #51 (ZIP compound) ─┐
#235 (search flags) ─┘                       ├─→ #48 (compat baseline) ─→ v1.0
                                             ├─→ #285 (TAR compound)
                                             └─→ #286 (!:apple)

#287 (docs fix) ─→ folds into #51 first PR (optional)

#45 (builder) ────────────────────────────────────→ v1.0 API stability
#44 (timeout partial results) ────────────────────→ v1.0 API stability
#46/#47/#49 (Core Flow gaps) ─→ #56 epic close ───→ v0.4.0 release

#236, #237, #106 — independent v0.3.0 work
```

## How to use this file

- Pick the next item from the top of the relevant milestone's table whose "Blocked by" is clear.
- When you start an issue, move it `[~]` in your local checklist (this file stays as a snapshot of *open* state; do not check items off here — GitHub state is authoritative).
- When closing an issue, prune its row from this file in the same PR.
- When opening a new issue that creates a dependency on or from existing work, update both rows and the dependency graph at the bottom.

Last refreshed: 2026-05-26.
