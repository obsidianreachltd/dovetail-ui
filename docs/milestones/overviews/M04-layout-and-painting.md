# M04 — Layout and painting

| Field | Value |
|---|---|
| ID | M04 |
| Status | In progress |
| Depends on | M01, M03 |
| Plan | [M04 plan](../plans/M04-layout-and-painting.md) |
| Report | — |

## Summary

Build our own layout engine and 2D painting, plus a first set of basic controls, so the Counter example runs in a real window.

## Why it matters

This is the first time components appear on screen. It also proves that only changed parts are redrawn.

## What it includes

- Layout in `dovetail-layout`: rows, columns, stacks, padding, alignment, and minimum and maximum sizes.
- Painting in `dovetail-render`: rectangles, borders, rounded corners and clipping.
- Simple single-line text drawing for labels, with no editing.
- Redoing layout and painting only for parts that changed.
- Button, Toggle and Text in `dovetail-controls`. The text control is named `Text`, as M03 built it and the syntax example uses it (decided 2026-11-20).
- The `examples/counter` example.

## What it leaves out

- Text editing, complex scripts and IME ([M06](M06-text-and-input.md)).
- Themes ([M14](M14-theming.md)).

## Done when

1. Rows, columns and stacks with padding, alignment, and minimum and maximum sizes lay out as shown in layout tests.
2. Rectangles, borders, rounded corners and clipping are drawn correctly, as checked by image tests.
3. A measurement shows that only changed regions are laid out and painted again.
4. Button, Toggle and Text work in `dovetail-controls`.
5. `examples/counter` runs on Windows, Linux and macOS, and clicking the button updates the count.
6. The frame time for a 1,000-element test scene is measured and recorded.
