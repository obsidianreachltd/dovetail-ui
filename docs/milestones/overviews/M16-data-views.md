# M16 — Data views

| Field | Value |
|---|---|
| ID | M16 |
| Status | In progress |
| Depends on | M15 |
| Plan | [M16 plan](../plans/M16-data-views.md) |
| Report | — |

## Summary

Build `dovetail-data`: lists, trees and tables that show very many rows quickly by making only the rows on screen, and a property grid for inspectors.

## Why it matters

- **Size:** applications show long collections. A PCB design has thousands of parts and nets, and an editor has layer trees and long logs. Today a column of rows makes every row, which is slow at that size.
- **The first release:** the validation editor in [R01](R01-first-release.md) needs a parts list, a layer tree and an inspector of the selected shape's properties.
- **The crate:** `dovetail-data` is already named in the [crate map](../../architecture/crates.md).

## What it includes

- **Virtualised lists:** only the rows that can be seen, and a few more, are made. Rows of one height are placed by arithmetic, and rows of different heights by measuring as they appear. The scrollbar from M15 shows the whole length.
- **Choosing rows:**
  - one, or many with Ctrl and Shift, as each system does;
  - with the keyboard: arrows, Page Up, Page Down, Home, End, and typing the first letters.
- **Trees:** rows with children that open and close, with the arrow keys moving in and out, and lazy children made only when a node opens.
- **Tables:**
  - columns with headers;
  - columns resized by dragging their edges;
  - sorted by clicking a header;
  - a fixed header while the rows scroll.
- **A property grid:**
  - names and values in two columns, grouped in sections that collapse;
  - values edited with M15's controls: text, number, check box and drop-down;
  - each change made as an undoable command (M08).
- **Data from the application's signals:** keyed rows (M02) keep their state when the data changes, and only changed rows are built again.
- **Drag and drop** of rows, within a view and between views, on M10's drag and drop.
- **Accessibility:** list, tree, table, row, cell and header roles, with the position in the whole collection, not only of the rows made.
- **An example** with a list, tree and table of 100,000 rows, and a property grid, with a `--self-test` and a `--bench`.

## What it leaves out

- Editing cells in place in a table, other than through the property grid: after R01.
- Grouping and filtering rows inside the table: applications filter their own data.
- Charts: after R01.

## Done when

1. A list, a tree and a table of 100,000 rows each scroll smoothly, and the benchmark records the time to build and to scroll a frame.
2. Rows can be chosen with the pointer and with the keyboard, one or many, as each system does.
3. Table columns can be resized and sorted, and tree nodes opened and closed, with the pointer and the keyboard.
4. A property grid edits values of several kinds, and each edit can be undone.
5. Assistive tools read the position of a row in the whole collection, checked by tests on the AccessKit tree.
6. The example's self-test uses each view, and its benchmark results are recorded.
