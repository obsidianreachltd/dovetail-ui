# D0022 — Data views: rows made from the scroll position, data from functions

| Field | Value |
|---|---|
| ID | D0022 |
| Status | Accepted |
| Date | 2026-11-24 |

## Context

M16 adds lists, trees and tables that show 100,000 rows quickly, and a property grid. To do that, each view makes only the rows on screen. That raises four questions:

- how a view learns which rows are on screen;
- how an application gives it data;
- who sorts a table;
- how property edits are undone.

Each answer changes core, layout or the desktop, so they are decided together here.

## Decision

- **Rows come from the scroll position.**
  - Layout tells an element its scroll offset and the size of what can be seen, through `Element::on_scroll`, when either changes.
  - The view then makes only the rows in sight, with a few more. Two spacers stand in for the rows above and below, so the scrollbar shows the whole length.
  - When that changes the tree, the window builds and lays out again before it paints, up to a few times. No frame shows blank rows.
  - A view moves its own scroll position with `Element::scroll_to`. Layout carries out each new request once.
- **Data comes from functions.**
  - A list or table is given a row count, a key for each index, and a function that builds the row for an index.
  - A tree is given a function for the children of a node, which it calls only for open nodes.
  - Rows and selections are found by key, so they survive changes to the data.
- **The application sorts.**
  - A table shows which column it is sorted by, and tells the application when a header is pressed.
  - The application sorts its own data, as it filters it.
- **Property edits go through `History`.**
  - Each property gives its value and a setter.
  - The grid records each change in the application's M08 `History`, with the old and new values. Undo and Redo then work as they do everywhere else.

## Consequences

- The view keeps rows in a normal scrolling column, so scrollbars, the wheel, clipping and hit testing work as they already do.
- A frame can lay out more than once. Only frames that scroll new rows into sight do so.
- Applications can show data that is not in a `Vec`, such as a database or a file read on demand.
- The row the keyboard is on is drawn and named by the view, not by focus. Focus stays on the view, so a row that scrolls away and is removed takes no focus with it. Assistive tools are told which row is current.
- `dovetail-data` depends on `dovetail-commands`, for `History`.

## Alternatives considered

- **Rows made by layout:** there would be no second pass, but layout would have to build components. That breaks the line between core and layout.
- **Data in a `Signal<Vec<T>>`:** simpler for small applications. But data must be copied into a vector, and lazy tree children would need another way in.
- **The table sorts, with a compare function for each column:** less work for the application. But the table would have to keep and re-sort 100,000 indexes.
- **Property edits through a callback only:** more flexible, but every application would repeat the same undo code.
