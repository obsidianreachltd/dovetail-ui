# D0014 — Unicode tables generated from the Unicode Character Database

| Field | Value |
|---|---|
| ID | D0014 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

Correct text handling needs facts about every character:

- where a caret may stop (grapheme clusters);
- where a line may break;
- which script a character belongs to, to choose a fallback font;
- in [M13](../milestones/overviews/M13-complex-scripts.md), bidirectional classes and Arabic joining types.

[D0002](D0002-build-own-stack.md) rules out the usual libraries for these, so Dovetail needs its own tables.

## Decision

A generator in `tools/` reads Unicode Character Database (UCD) files and writes compact Rust tables into `dovetail-text`.

- The UCD files are downloaded from unicode.org when the generator first needs them. The user agreed to this on 2026-11-20.
- Only the files needed are fetched. For M06 they are `GraphemeBreakProperty.txt`, `emoji-data.txt`, `LineBreak.txt` and `Scripts.txt`. M13 adds `DerivedBidiClass.txt` and `ArabicShaping.txt`.
- The generated tables are committed with the Unicode version they came from, and the UCD's licence notice. A build never downloads anything.
- The downloaded files themselves are not committed. The generator can fetch them again.

## Consequences

- Text behaviour follows the Unicode standard, and upgrading means running the generator again with a new version.
- The Unicode licence is permissive and needs its notice kept. That is recorded next to the generated tables and in `NOTICE`.
- The tables add to the size of `dovetail-text`. Compact range tables keep this to tens of kilobytes.

## Alternatives considered

- **Unicode crates** (`unicode-segmentation`, `unicode-bidi` and others): proven, but against D0002.
- **Hand-written tables:** small, but they would be wrong for many characters and hard to keep up to date.
