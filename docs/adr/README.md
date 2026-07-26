# Architecture Decision Records

Structured records of architectural decisions for libmagic-rs — what was decided, what was rejected, and why. Use the [template](template.md) for new entries.

| ADR                                      | Title                                                                      | Status   | Date       |
| ---------------------------------------- | -------------------------------------------------------------------------- | -------- | ---------- |
| [0001](0001-gnu-file-output-contract.md) | GNU `file` compatibility is an output contract, not an ergonomics contract | accepted | 2026-07-26 |

## Lifecycle

`proposed` -> `accepted` -> (`deprecated` | `superseded by ADR-NNNN`)

A superseded ADR always links its replacement. Do not delete or rewrite an accepted ADR to reflect a changed decision — supersede it, so the reasoning trail survives.

## When to record one

Technology choices, architecture patterns, API design, data modeling, infrastructure, security posture, testing strategy, and process decisions. Skip trivia — naming and formatting choices do not need an ADR.

The bar is whether a future contributor would otherwise ask "why is it like this?" and find no answer in the code.
