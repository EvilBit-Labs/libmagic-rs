# ADR-0002: A divergence where rmagic is demonstrably more accurate than GNU `file` is acceptable

**Date**: 2026-09-01\
**Status**: accepted\
**Deciders**: @UncleSp1d3r

**Refines** [ADR-0001](0001-gnu-file-output-contract.md). That ADR stands; this narrows one binding rule inside it.

## Context

ADR-0001 makes detection results a binding contract and, under "Divergence creep", states that every detection-result divergence "is a contract gap that must be closed, never recorded as a settled design choice."

That rule assumes divergence means rmagic is wrong. It does not cover the case where rmagic is **right and `file` is wrong**, which is reachable because the two tools do not read the same bytes. GNU `file` reads a bounded prefix (`HOWMANY`, ~1 MB); rmagic memory-maps the whole file.

The concrete instance: `/usr/lib/dyld` declares `arch[1]` at offset 1,064,960, past `file`'s read buffer. `file` cannot reach that Mach-O header and prints a bare `[arm64e]`; rmagic reaches it and prints `[arm64e:Mach-O 64-bit dynamic linker arm64e]`, which is what the file actually contains. Read literally, ADR-0001 requires closing that gap — that is, making rmagic discard a correct result to reproduce a truncated one.

## Decision

A detection-result divergence is **acceptable, not a contract gap**, when rmagic's output is demonstrably more accurate than `file`'s.

Three conditions, all required:

1. **Verified against the file's actual contents**, not inferred from the two output strings. The structure rmagic reports is really there.
2. **The cause is a known `file` limitation**, named — a read-buffer bound, a recursion cap, an unimplemented construct — and not an rmagic guess, heuristic, or invented detail.
3. **Documented where a maintainer will hit it**, with the cause, so a later differential run does not read it as a regression and "fix" it backwards.

A divergence that is merely *different* remains a contract gap under ADR-0001. Being more detailed is not the same as being more accurate: extra output that the file does not support is a defect, and a worse one than truncation.

## Consequences

### Positive

- The differential acceptance criterion is **"no row moved away from parity"**, not "every row matches". A row where rmagic is correct and `file` is truncated counts as passing.
- rmagic keeps the benefit of memory-mapping the whole file rather than capping its own reads to match a 1990s buffer size.
- Removes a perverse incentive: without this, closing the "gap" means deleting correct output.

### Negative

- Differential results need classification rather than a single equality count. A reviewer must separate *more accurate* from *wrong*, and condition 1 is what makes that decidable.

### Risks

- **"More accurate" as an excuse.** The failure mode is claiming superiority for output that is simply wrong.

  *Mitigation (binding):* condition 1 is positive evidence from the file's bytes. A divergence justified only by reasoning about the two strings does not qualify and stays a contract gap.

## Known acceptable divergences

| Divergence                                                                                              | Cause                                                                   | Evidence                                                                                 |
| ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `/usr/lib/dyld`: rmagic prints `[arm64e:Mach-O 64-bit dynamic linker arm64e]`, `file` prints `[arm64e]` | `arch[1].offset` = 1,064,960, past `file`'s ~1 MB `HOWMANY` read buffer | Fat header decoded directly; the Mach-O header is present at that offset (GOTCHAS S14.5) |
