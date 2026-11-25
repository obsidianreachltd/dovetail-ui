# M10 — Docking

| Field | Value |
|---|---|
| ID | M10 |
| Status | In progress |
| Depends on | M08 |
| Plan | [M10 plan](../plans/M10-docking.md) |
| Report | — |

## Summary

Build dockable panels, tabs and split views in `dovetail-docking`, with layouts that are saved between runs.

## Why it matters

Engineering tools arrange many panels around a main view. The validation editor needs a dockable inspector.

## What it includes

- Docking panels to the left, right, top or bottom, as tabs, or in splits, by dragging.
- Resizing splits by dragging.
- Floating panels in their own windows.
- Saving and restoring the layout.
- Keyboard commands to move between panels.

## What it leaves out

- Saving document contents ([M11](M11-documents.md)).

## Done when

1. Panels can be docked left, right, top or bottom, as tabs, or in splits, by drag and drop.
2. Split sizes can be changed by dragging.
3. Panels can float in their own windows.
4. The layout is saved and restored between runs.
5. The keyboard can move focus between panels.
