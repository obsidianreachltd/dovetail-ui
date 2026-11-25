# M10 plan — Docking

| Field | Value |
|---|---|
| ID | M10 |
| Status | In progress |
| Overview | [M10 overview](../overviews/M10-docking.md) |
| Last updated | 2026-11-22 |

## Goal

Build dockable panels, tabs and split views in `dovetail-docking`, with layouts that are saved between runs.

## Starting point

- **Elements** have layout, clipping, focus, keys, pointer capture, layers and roles (M04–M09).
- **Growing children** (`Style::grow`) share the free space of a row or column in proportion. A split can use the panels' shares as their `grow`.
- **State** lives in one runtime for the whole UI thread, so components in two windows can read and change the same signals. But each window's `Tree::update` takes every marked component and keeps only its own. A second UI window therefore loses the first one's rebuilds ([B0005](../../bugs/B0005-second-window-loses-rebuilds.md)).
- **What is missing:**
  - a way for components to open and close windows;
  - drag and drop, inside a window and between windows;
  - a way to move focus to a part of the window from code;
  - the size of an element, which a split needs to turn a drag in pixels into new shares.
- **Decided on 2026-11-22** ([D0016](../../decisions/D0016-docking-foundations.md)):
  - the layout is saved in a small text format of our own, with no new dependency;
  - drag and drop is general and lives in `dovetail-core`, and the desktop carries it between Dovetail windows;
  - a floating panel goes back by dragging its tab onto the main window, or with a Dock button.
- **The new package** `dovetail-docking` is named in the [crate map](../../architecture/crates.md). It depends on `dovetail-core`, `dovetail-controls` and `dovetail-macros`, like `dovetail-commands`.

## Steps

1. [x] Settle the open questions (above), on 2026-11-22.
2. [x] **Fix B0005**, done on 2026-11-22: each tree takes only its own marked components, and leaves the rest for the other trees (`dovetail-core/tests/two_trees.rs`).
3. [x] **Core**, done on 2026-11-22:
   - drag and drop: `start_drag` with a `DragData` payload and a `DragEnd` callback, and `Element::on_drop` with `DropEvent` (over, leave, drop; the point and the target's size). A target that declines passes the drag to the nearest target around it;
   - `Element::on_size`, told the element's size after layout when it changes;
   - `FocusTarget` and `Element::focus_target`: `focus()` moves focus to the first focusable element inside the target, or to the target, in whichever window shows it;
   - a window host (`window_host`, `WindowRequest`, `WindowPlace`), installed by the desktop crate, like the menu host;
   - `PointerEvent::window_x` and `window_y`, so a splitter can measure a drag that moves it;
   - `Role::Tab`, `Role::TabList`, `Role::TabPanel` and `Role::Splitter`.
4. [x] **Desktop**, done on 2026-11-22 (`dovetail-desktop/src/app/multi.rs`):
   - a drag goes to the drop target under the pointer, in this window or another Dovetail window, found from the windows' places on screen. Floating windows are looked at first;
   - Escape gives up a drag. A drop outside every window tells the source where on screen it ended;
   - windows for the host open and close, tell their component when they move, ask it when the user closes them, and close when the first window closes;
   - size handlers and focus requests are carried out after layout. A window that takes focus this way comes to the front;
   - `settings_dir` gives the folder for an application's settings on each system;
   - AccessKit roles for tabs and splitters, with the chosen tab selected.
5. [x] **The model**, done on 2026-11-22: `DockLayout` in `dovetail-docking`, with `move_panel` to a side, into tabs, along an edge or out to float, `dock_floating`, `set_shares`, `keep_known`, and tidying (empty stacks go, a split with one part gives way to it, and splits the same way merge). `to_text` and `from_text` read and write the format in D0016. 15 tests.
6. [x] **The views**, done on 2026-11-22: `Dock` (made with `use_state`, kept in a file with `keep_in_file`), `DockArea` and `FloatingStack`. Tabs drag; stacks, tab bars, tabs and the dock's edges show where a drop will go; splitters resize; floating windows open, move and close with the layout. Each stack has Float, or Dock in a floating window. F6, Shift+F6, Ctrl+Page Down, Ctrl+Page Up, Left and Right work as planned.
7. [x] **`examples/docking`**, done on 2026-11-22: files, layers, board, console and inspector panels, with their state in the application's signals. The layout is kept in the user's settings folder, or in the file named by `DOVETAIL_DOCK_FILE`.
8. [x] **Tests**, done on 2026-11-22:
   - the model and the text format (15);
   - two trees sharing state (B0005), drop targets and focus targets without a window (`dovetail-layout/tests/drop.rs`);
   - `examples/docking --self-test` (18 checks): Inspector docks left of Board, Console along the right edge, Files below Board and Layers above Console, all by dragging tabs; Inspector joins Board's stack by a drop on its tab; Escape gives up a drag; a splitter drag resizes the split; Console dragged out of the window floats in a window called Console, and dragged back from that window beside Board docks again and closes it; five F6 presses visit the four stacks and go round; Ctrl+Page Down shows the next tab and focuses it; the saved file reads back as the layout shown.
## Prototypes and open decisions

- **Save format:** decided, our own text format (D0016).
- **Drag and drop:** decided, general, in core (D0016).
- **Re-docking:** decided, drag back, plus a Dock button (D0016).
- **Panel state:** a panel's content is built again when it moves to another stack or window, so state kept inside it is lost. Applications keep panel state outside the panel, in their own signals. This is written in the docs.
- **Closing a floating window** puts its panels back in the main window, rather than hiding them.

## Risks

- **Pointer outside the window:** dragging a tab out needs the pointer's movements outside the window while the button is held. Windows, macOS and X11 send them. If a system does not, a panel can still be floated with its button.
- **Window places:** Wayland does not tell applications where their windows are. There, drags between windows do not work, and floating windows open where the system puts them. The Dock button still works.
- **Floating windows behind the main window:** on Windows they can go behind the main window when it is clicked. Making them owned windows is a follow-up.
- **Writing the layout file often:** the layout is written when it changes, which includes moving a floating window. The file is small, and it is written only when its text changes.

## Testing

- **Unit tests** for the model: moves, tidying, shares, and the text format both ways, including bad input.
- **Tests without a window** for a tree that leaves other trees' rebuilds (B0005), for finding drop targets, and for focus targets.
- **`examples/docking --self-test`** for drag and drop, splitters, floating windows, keys and saving.
- **By hand, on each system:** dragging tabs between windows with a real mouse, and that the layout comes back after closing and opening the example. Not done yet.
- **Results on 2026-11-22**, Windows 11:
  - 210 workspace tests pass, and every example's self-test passes, `examples/docking` included;
  - the real example, run with its layout in a file of its own: the Float button, pressed through UI Automation, opened a window called Files and the file was saved. A second run opened both windows again, with Files floating. Screenshots of both windows look right. No keys or mouse input were sent.

## Done when

Copied from the overview.

1. Panels can be docked left, right, top or bottom, as tabs, or in splits, by drag and drop.
2. Split sizes can be changed by dragging.
3. Panels can float in their own windows.
4. The layout is saved and restored between runs.
5. The keyboard can move focus between panels.
