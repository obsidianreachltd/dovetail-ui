# D0016 — Docking: our own layout format, drag and drop in core

| Field | Value |
|---|---|
| ID | D0016 |
| Status | Accepted |
| Date | 2026-11-22 |

## Context

M10 needs three things that Dovetail does not have yet:

- a way to save a docking layout between runs;
- drag and drop, to move tabs inside a window, out of it, and between windows;
- a way to put a floating panel back into the main window.

## Decision

- **The layout is saved as text in a format of our own,** written and read by `dovetail-docking`. The first line names the format and its version. A file that cannot be read gives the application's default layout. No new dependency is added.
- **Drag and drop is general and lives in `dovetail-core`.** A pointer handler starts a drag with `start_drag`, giving a payload of any type and a callback for how the drag ended. Elements take drops with `Element::on_drop`. `dovetail-desktop` sends the events, finds the window under the pointer from the windows' places on screen, and tells the source when a drag ends outside every window.
- **A floating panel goes back** by dragging its tab onto the main window, or with a Dock button. Where the system does not give window places (Wayland), only the button works.

## Consequences

- Other applications can use drag and drop, for example to reorder lists or drop shapes on a canvas.
- The layout file is easy to read and to fix by hand. We must keep reading older versions of the format once it is released.
- `dovetail-core` gains a host for windows, like the menu host, so that components can open floating windows without depending on the desktop crate.

## Alternatives considered

- **serde with JSON or RON:** well known, but new dependencies need their own decision, and a docking layout is small and simple.
- **Drag and drop only inside docking:** smaller, but nothing else could use it, and drags between windows would need the same desktop work anyway.
- **Only a Dock button for floating panels:** simpler, and the same on every system, but dragging a panel back is what users of other docking tools expect.
