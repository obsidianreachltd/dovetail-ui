# M16 plan — Data views

| Field | Value |
|---|---|
| ID | M16 |
| Status | In progress |
| Overview | [M16 overview](../overviews/M16-data-views.md) |
| Last updated | 2026-11-24 |

## Goal

Build `dovetail-data`: lists, trees and tables that show very many rows quickly by making only the rows on screen, and a property grid for inspectors.

## Starting point

- **Scrolling (M06, M15):** layout keeps each `Style::scroll` element's offset, the wheel moves it, and scrollbars show and drag it. A component cannot read or set the offset.
- **Layout runs once a frame.** `on_size` reports come after layout, so a change they cause shows a frame later.
- **Keyed lists (M02)** keep each item's state when the list changes.
- **Drag and drop (M10)** in core: `start_drag` and `Element::on_drop`.
- **Undo (M08):** `History::apply` with redo and undo closures.
- **Controls (M15):** `TextField`, `NumberField`, `CheckBox`, `Select` and `Collapsible`, all reading `theme()`.
- **Accessibility:** list box and option roles, and `selected` for options. There are no tree, table, row, cell or header roles, and no position in a set.
- **Decided on 2026-11-24** ([D0022](../../decisions/D0022-data-views-foundations.md)):
  - layout tells a view its scroll position, and the window lays out again in the same frame when that makes new rows;
  - data comes from a count, a key and a row function, and trees ask for the children of open nodes;
  - the application sorts;
  - property edits go through `History`.

## Steps

1. [x] Settle the open questions (above), on 2026-11-24.
2. [x] **Core**, done on 2026-11-24:
   - `Element::on_scroll` with a `Viewport` (the offset and the size of what can be seen), and `Element::scroll_to` with a `ScrollRequest`;
   - roles: tree, tree item, table, row, cell and column header;
   - `Element::item` (the position in the whole collection, and the level in a tree), `selected`, `current`, `multiselect`, `table_size`, `column` and `sorted`;
   - `Tree::needs_update`.
3. [x] **Layout**, done on 2026-11-24: `Layout::viewport` reports each scrolling element's viewport, and layout carries out each new scroll request once.
4. [x] **Desktop**, done on 2026-11-24:
   - call `on_scroll` after layout;
   - build and lay out again, up to four times, while that changes the tree;
   - map the new roles and properties to AccessKit, with the current row as the active descendant.
5. [x] **`dovetail-data`**, done on 2026-11-24:
   - `Selection`: one or many rows, by key, with Ctrl or Command and Shift as each system does;
   - row heights: fixed, by arithmetic, or measured as rows appear, with an estimate for the rest;
   - `DataList`, `DataTree` (lazy children), `DataTable` (headers, resizing, sorting, a fixed header) and `PropertyGrid`;
   - each row is a component keyed by its row's key, so what is inside keeps its state while rows come and go;
   - a tree keeps its shown rows between builds, and works them out again only when a node opens or closes, or its functions change;
   - `Key::number` and `Key::text`, so a tree can find its data from a key;
   - row colours and height in the theme (`RowTheme`, `row_look`);
   - keys: arrows, Page Up, Page Down, Home, End, Ctrl+A, Space, and typing the first letters;
   - dragging rows within a view and between views.
6. [x] **`examples/data`**, done on 2026-11-24: a tree of 100 layers with 1,000 shapes each, a table of 100,000 parts, a log of 100,000 entries of different heights, a Picked list that takes parts dragged from the table, and an inspector. It has a `--self-test` of 23 steps and a `--bench`.
7. [x] **Tests**, done on 2026-11-24:
   - unit tests: selection, with clicks, keys, typing and changed data (5); row heights (3); sorting (1);
   - view tests through the component tree and layout (9): only rows in sight are made, rows chosen with the pointer and the keyboard, dragging and dropping, measured rows, a tree opened and closed, a table sorted, resized and scrolled, and the property grid's edits undone;
   - desktop: the AccessKit tree for rows, items, cells and headers in a whole collection (1).
8. [x] Run everything on Windows, and record the benchmark, on 2026-11-24.

## Prototypes and open decisions

- **Rows:** decided, from the scroll position (D0022).
- **Data:** decided, from functions (D0022).
- **Sorting:** decided, by the application (D0022).
- **Property edits:** decided, through `History` (D0022).
- **Focus stays on the view, not on a row.** The view draws the current row, and tells assistive tools which it is. A row that scrolls away is removed, so focus on it would be lost.
- **Keyboard for table headers:** the header row is one Tab stop. Left and Right move between headers, Enter or Space sorts, and Ctrl+Left and Ctrl+Right (Command on macOS) make a column narrower or wider.
- **Found by the benchmark**, fixed on 2026-11-24:
  - A list of measured rows first took its unmeasured rows to be 0 pixels tall, so every row seemed to be in sight, and all 100,000 were made. The estimate is now used from the first build.
  - A tree worked out its shown rows in every build, as it scrolled: 8.8 ms a frame with 100,100 open rows. It now keeps them until a node opens or closes: 1.3 ms.
- **Found by the self-test**, fixed on 2026-11-24: a number field's step buttons do not take focus, so an edit made with them never ended when focus left. An edit under way now also ends when another property is focused or changed.
- **Measured rows can take four layouts in a frame**, the most a frame does, when new rows come into sight and are then measured. Anything left is finished in the next frame.
- **Follow-ups, after M16:**
  - assistive tools cannot yet open and close tree items, or select rows, through actions: they use the keys;
  - Undo while a text or number edit is under way takes back the step before it, not the edit;
  - editing cells in place in a table, after R01, as the overview says.

## Risks

- **A second layout in a frame costs time.** It runs only when new rows come into sight. The benchmark measures it.
- **Measured rows can move** when an estimated height is replaced by the real one. Rows above the view are measured as they pass, so the view moves only when it jumps a long way.
- **Very large trees** are flattened in each build. The benchmark measures a tree with 100,000 open rows.

## Testing

- **Unit tests** for selection (clicks with each modifier, keys and ranges), row heights and the visible range.
- **View tests** through the component tree and layout, with scroll reports passed on as the window does: only rows in sight are made, keys and clicks choose rows, trees open and close, columns resize and sort, and property edits undo.
- **AccessKit tree tests** in the desktop: roles, the position of a row in the whole collection, selection, the current row, sorting and the table's size.
- **`examples/data --self-test`** in a real window, using each view with the pointer and the keyboard.
- **`examples/data --bench`** records the time to build each view and to scroll a frame.
- **By hand, on each system:** that scrolling feels smooth, and that selection follows the system's habits.
- **Results on 2026-11-24**, Windows 11:
  - 297 workspace tests pass, every GPU test passes, and clippy is clean, including for macOS and Linux;
  - `examples/data --self-test` passes all 23 steps, and every other example's self-test passes, twice each;
  - the real example, driven through UI Automation only, with no keys or mouse input: pictures in light and dark look right;
  - `examples/data --bench` (release, NVIDIA TITAN RTX on Vulkan, a 900 × 700 view, the median of 31 frames). A frame builds, lays out as often as it needs, paints and waits for the GPU:

    | View | First frame | Scroll a page | Scroll 40 pixels | Jump 1,000,000 pixels | Rows made | Layouts in a page frame |
    |---|---|---|---|---|---|---|
    | List, 100,000 rows | 27.82 ms | 1.10 ms | 0.74 ms | 0.80 ms | 41 | 2 |
    | List, 100,000 measured rows | 26.25 ms | 1.86 ms | 0.81 ms | 0.84 ms | 47 | 4 |
    | Tree, 100,100 open rows | 17.12 ms | 1.32 ms | 1.05 ms | 1.01 ms | 41 | 2 |
    | Table, 100,000 rows, 4 columns | 26.52 ms | 1.84 ms | 1.55 ms | 1.67 ms | 40 | 2 |

    The first frame includes making the painter's pipelines and drawing the first glyphs.

## Done when

Copied from the overview.

1. A list, a tree and a table of 100,000 rows each scroll smoothly, and the benchmark records the time to build and to scroll a frame.
2. Rows can be chosen with the pointer and with the keyboard, one or many, as each system does.
3. Table columns can be resized and sorted, and tree nodes opened and closed, with the pointer and the keyboard.
4. A property grid edits values of several kinds, and each edit can be undone.
5. Assistive tools read the position of a row in the whole collection, checked by tests on the AccessKit tree.
6. The example's self-test uses each view, and its benchmark results are recorded.
