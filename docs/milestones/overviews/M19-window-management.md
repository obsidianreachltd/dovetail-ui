# M19 — Window management

| Field | Value |
|---|---|
| ID | M19 |
| Status | In progress |
| Depends on | M10, M11 |
| Plan | [M19 plan](../plans/M19-window-management.md) |
| Report | — |

## Summary

Give applications full control of their top-level windows: several of them, each remembering its size, position and state between runs, and staying on screen when monitors change.

## Why it matters

- **Remembered windows:** a desktop application is expected to open where it was last closed, at the same size, maximised if it was. Dovetail only keeps a window's place across `dovetail dev` restarts.
- **Several windows:** document applications open one window for each document. [M10](M10-docking.md) has floating panels and [M11](M11-documents.md) has documents, but there is no general way to open, find and close an application's own top-level windows.
- **Monitors change:** a window saved on a second monitor that is no longer there must come back on a screen that is.
- **The first release:** the validation editor in [R01](R01-first-release.md) should open where it was left.

## What it includes

- **Opening windows from components:** a window as part of the component tree, opened while it is shown and closed when it goes, with its title, size, minimum and maximum sizes, and whether it can be resized.
- **The application's windows:** a list of the open windows, bringing one to the front, and closing the application when the last one closes, or not, as the application chooses (macOS keeps running by default).
- **Window state:**
  - size, position, maximised, minimised and full screen, read and set;
  - saved as plain text with a name, and restored on the next run, in the same way as the docking layout (D0016);
  - a restored window is moved onto a monitor that is there, at a scale that fits it.
- **Monitors:** the list of monitors, with their work area and scale, and a change event when they are added or removed.
- **Modal windows:** a window that blocks its owner until it closes, for settings and other dialogs that are larger than an in-window `Dialog`.
- **Full screen:** entering and leaving it, with a command.
- **An example** with several windows and a `--self-test` that closes and reopens them and checks their state.

## What it leaves out

- Drawing the window's own title bar and frame; the system draws them.
- Windows that are not rectangles, and see-through windows.
- Tabs of windows joined together by the system, such as macOS's window tabs.

## Done when

1. A window closed and opened again comes back at the same size, position and maximised state, on Windows, Linux and macOS.
2. A window saved on a monitor that is no longer there opens fully on one that is.
3. A window in the component tree opens when shown and closes when it goes, checked by a test.
4. A modal window blocks input to its owner until it closes.
5. The application closes after its last window, or stays running on macOS, as it chooses.
6. The example's self-test checks several windows, their saved state, and full screen.
