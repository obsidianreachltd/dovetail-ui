# M13 — Complex scripts

| Field | Value |
|---|---|
| ID | M13 |
| Status | In progress |
| Depends on | M06 |
| Plan | [M13 plan](../plans/M13-complex-scripts.md) |
| Report | — |

## Summary

Shape and lay out text in scripts whose letters change form or whose direction runs right to left, starting with Arabic. It was split from [M06](M06-text-and-input.md) on 2026-11-20 to keep M06 a size that can be built and tested.

## Why it matters

Applications built with Dovetail must show their users' languages correctly. Arabic tests the two hardest parts of text layout at once: letters that join in context, and right-to-left order mixed with left-to-right on one line.

## What it includes

- OpenType `GSUB` lookups for Arabic joining forms (`isol`, `init`, `medi`, `fina`) and required ligatures (`rlig`), and `GPOS` mark positioning.
- Arabic joining types from the Unicode tables ([D0014](../../decisions/D0014-unicode-data.md)).
- Bidirectional text: resolving levels and ordering runs on a line, from a subset of the Unicode bidirectional algorithm.
- Caret movement and selection in mixed-direction text in `TextField`.

## What it leaves out

- Scripts that need a full shaping engine, such as Indic scripts: after the first release.
- Vertical text: after the first release.

## Done when

1. Arabic words join correctly (isolated, initial, medial and final forms, and the lam-alef ligature), checked against known glyph sequences for a system font.
2. A line that mixes Arabic and English shows each run in the right order, checked against the bidirectional algorithm's test data for the cases supported.
3. The caret moves and selection works in mixed-direction text, in visual order.
4. Arabic text is sharp at 100%, 150% and 200% display scale.
