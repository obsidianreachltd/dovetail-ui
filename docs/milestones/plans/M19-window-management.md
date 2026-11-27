# M19 plan — Window management

| Field | Value |
|---|---|
| ID | M19 |
| Status | In progress |
| Overview | [M19 overview](../overviews/M19-window-management.md) |
| Last updated | 2026-11-26 |

## Goal

Give applications full control of their top-level windows: several of them, each remembering its size, position and state between runs, and staying on screen when monitors change.

## Starting point

- **Opening windows (M01, M10):**
  - `Context::open_window` and `Context::open_ui_window` in `crates/dovetail-desktop/src/app.rs` open a window from `WindowOptions`. These have only a title, a logical size (800 × 600 by default) and an optional logical position. They have no minimum or maximum size, no resizing setting, no maximised or full-screen state, and no owner.
  - The first window opened is the primary one. Closing it closes every hosted window (`Shell::close_hosted`). When no windows are left the program ends, unless the event loop is pumped, as in tests. Nothing is different on macOS.
  - `Context` can also list windows, give a window's place, bring one to the front, set its size, minimise it, and close it.
- **Windows from components (M10):**
  - `dovetail-core`'s `WindowHost` (`open`, `close`, `set_title`) takes a `WindowRequest` with a title, a `WindowPlace`, a root view, `on_close` and `on_place`. `WindowPlace` holds only a position (none on Wayland) and a size.
  - The desktop's `DesktopHost` in `app/multi.rs` queues the requests and carries them out after each update.
  - Only docking uses it: `Dock::sync_windows` opens a window for each floating stack. No general component opens a window while it is shown.
  - `TestUi` installs no host, so a headless test cannot open a window. `TestWindows` can open several real windows.
- **Keeping a place:** `dovetail dev` keeps the primary window's place across restarts, in physical pixels, in a one-line file named by `DOVETAIL_DEV_STATE` (`crates/dovetail-desktop/src/dev.rs`). Nothing keeps a window's place between normal runs.
- **Saved text (D0016):** the docking layout is saved as plain text of Dovetail's own. Its first line names the format and its version (`dovetail-dock 1`). An unreadable file gives the default layout. `Dock::keep_in_file` reads it and writes it again when it changes. Floating stacks keep a logical position and size.
- **Monitors:** only the frame pace reads the current monitor's refresh rate. Nothing lists monitors, reads their work area or scale, or sees one added or removed. A scale change is handled as a resize.
- **Modal windows:** only the in-window `Dialog` in `dovetail-controls`, with a scrim that swallows presses. Native file dialogs are owned by the front window on Windows, and run modal on macOS.
- **Closing with a question (M11):** `DocumentFrame` guards its window with a `CloseGuard` and asks about unsaved changes. When the user agrees, guarded windows close, the primary one last.
- **Platform code:** `platform/` has clipboard, menus, file dialogs, appearance and settings code for Windows, macOS and X11. None of it touches windows, monitors or window state.
- **winit 0.30.13** can set the minimum and maximum size, resizing, maximised and minimised, and borderless full screen. It can list monitors with their position, size and scale. It cannot give a monitor's work area, does not say when monitors change, and has no modal windows. On Wayland it cannot read or set a window's position.

## Steps

1. [ ] **Decide the shape**, in a decision record, D0029:
   - how a window is declared in the component tree: a `Window` component that opens its window while it is shown and closes it when it goes, which crate it lives in, and how its content and its own state stay with the component that declares it;
   - what is saved for a window, and where: logical or physical sizes, the monitor it was on, and the state (normal, maximised, full screen). Saved as text in the same way as the docking layout, with a header such as `dovetail-windows 1`;
   - how a restored window is fitted onto the monitors there are;
   - how modal windows block their owner: by the system where it can (an owned window on Windows, a child window on macOS), or by Dovetail refusing input to the owner on every system;
   - how a monitor change is found, since winit does not report one;
   - whether the application ends with its last window, and the default on each system.
   - Written on 2026-11-26 as Proposed: [D0029](../../decisions/D0029-window-management.md). It is accepted after the prototype below checks modal blocking and monitor changes on Windows and Linux.
2. [x] **Window options in `dovetail-core`:**
   - a minimum and maximum size, whether it can be resized, and the state it opens in, in `WindowRequest` and in the desktop's `WindowOptions`;
   - `WindowState`: normal, maximised, minimised or full screen;
   - the desktop applies them with winit, and tells the window's component when the user changes them.
   - Done on 2026-11-26:
     - `WindowState` and `WindowLimits` (minimum and maximum size, resizable, `clamp`) in `crates/dovetail-core/src/window.rs`. `WindowRequest` has `limits`, `state`, `on_state` and `modal`, and `WindowRequest::new` with callbacks that do nothing. The host can `set_state`, `set_limits` and `focus`.
     - `WindowOptions::with_limits` and `with_state`. The desktop applies them with winit when a window opens, and `Context::window_state`, `set_window_state` and `set_window_limits` after. `set_window_size` keeps to a window's limits itself: Windows does not hold a size asked for by the program to the minimum.
     - The desktop reads a window's state after each resize, move and visibility change, and tells the component that opened it, then `Application::state_changed`. It tells the state before the new place, so a window being maximised is not saved at its maximised size.
     - Four real-window tests in `crates/dovetail-testing/tests/windows.rs`: size limits hold, a window maximised and restored comes back to its own size, a window opens maximised, and full screen and back.
3. [x] **Windows in the component tree:**
   - the `Window` component from step 1: opened while shown, closed when it goes, its title, sizes and state following its properties;
   - closing it from its title bar calls its `on_close`, and the component decides whether it goes;
   - a headless window host in `dovetail-testing`, so `TestUi` can find the windows a tree has open, look inside them, and close them (criterion 3).
   - Done on 2026-11-26:
     - `Window` in `crates/dovetail-core/src/toplevel.rs`, written by hand with the builder `ui!` uses, since core cannot use the macros. It keeps what it opened in its state, and closes the window when that state is dropped, as the component goes. Its children reach the window through a signal it writes but does not read, so it is not built again when they change. Its window sees the values provided around it.
     - `Tree::contains` and `Session::contains`, so a host finds the window that shows a component.
     - `TestUi` installs a window host (`crates/dovetail-testing/src/host.rs`). Each window has its own session. `windows`, `window`, `switch_to`, `switch_to_main`, `close_window`, `set_window_state` and `move_window` act as the user would. The window the test switches to keeps its session where the main one was, so every other function works in it unchanged.
     - `crates/dovetail-testing/tests/toplevel.rs`: a window opens when shown and closes when it goes (criterion 3), shares state with the component that declares it, follows its title, asks its component before closing, closes itself with no `on_close`, follows its state property and tells the user's changes, keeps its limits when moved, sees provided values, and opens nothing in a plain tree.
4. [x] **The application's windows:**
   - a list of the open windows, with their titles and states, that a component can read;
   - bringing a window to the front;
   - the choice of ending the application after its last window, or staying, with staying the default on macOS. On macOS a click on the Dock icon opens the main window again (criterion 5).
   - Done on 2026-11-26:
     - `app_windows()` gives each window opened through the host, with its title, state and whether it has the keyboard, and builds its readers again when the list changes. `bring_to_front` asks the host. The desktop and `TestUi` keep the list up to date.
     - `run_windows` and `run_windows_with` in `crates/dovetail-desktop/src/app/root.rs`: the root is a session with no window, built before each wait. `Context::set_root` sets it, and `View::Empty` takes it away.
     - `ExitPolicy` in `crates/dovetail-desktop/src/app/manage.rs`, set with `Context::set_exit_policy`. Its default keeps running on macOS. Programs run with `run` keep ending after their last window, so no example changed (D0029 was updated for this before it was accepted).
     - On macOS a Dock click with no window open builds the root again, through a method added to the application's delegate in `platform/macos_windows.rs`. It has only been type-checked.
     - Bringing a window to the front ignores the Alt press winit's `focus_window` sends on Windows, for 250 ms, as the prototype found.
5. [x] **Monitors:**
   - the list of monitors, with their position, size, work area and scale. The work area needs platform code: `GetMonitorInfoW` on Windows, `NSScreen`'s `visibleFrame` on macOS, and `_NET_WORKAREA` on X11. Where it cannot be read, it is the whole monitor;
   - readers for the list, so a component is built again when a monitor is added, removed or changed;
   - the list is read again as step 1 decides, for example on a scale change and when a window comes to the front.
   - Done on 2026-11-26:
     - `Monitor`, `ScreenRect`, `monitors()`, `set_monitors()` and `monitor_at()` in `crates/dovetail-core/src/screen.rs`. Places on the desktop are in physical pixels.
     - The work area: `work_area` in `platform/windows.rs`, `platform/macos_windows.rs` and `platform/x11_windows.rs`, each with `// SAFETY:` comments. X11 cuts the window manager's one work area down to each monitor. Only the Windows code has run.
     - The desktop reads the list at start, when a window opens, and on focus, moves and scale changes.
     - A window's logical position is turned into physical pixels with the scale of the monitor it lands on, as windows report their places.
6. [x] **Saving and restoring:**
   - each window's place and state as text, with a name for each window, kept in a file with `keep_in_file` as the docking layout is;
   - on the next run, a window with a saved place opens there, in its saved state;
   - fitting it onto a monitor: a plain function, tested on every system, that moves a window from a monitor that is gone onto one that is there, shrinks it to the work area if it is too big, and keeps its size in logical pixels across a scale change (criterion 2);
   - on Wayland, the size and state are restored and the system places the window.
   - Done on 2026-11-26:
     - `SavedWindows` in `crates/dovetail-core/src/saved.rs`, in the `dovetail-windows 1` format, with `keep_in_file`. A window saved minimised keeps the state it had before.
     - `fit` in `screen.rs`, with `FRAME_ROOM` for the frame each system adds. Eleven unit tests: a window comes back where it was, a monitor that is gone (criterion 2), too big for the monitor left, its minimum size kept, half off screen, a change of scale, monitors moved around, states, no position, no monitors, and the monitor at a point.
     - `Window` takes `name` and `saved`. It opens fitted onto the monitors there are, saves its normal place while it is normal, and saves only its state otherwise. A saved state wins over its `state` property when it opens, and its component is told.
     - Four unit tests of the text: read back the same, a minimised window, unreadable text naming its line, and a file kept up to date and read on the next run.
7. [x] **Full screen:** entering and leaving it, read with the window's state. A standard `Commands` command, with F11 on Windows and Linux and Ctrl+Cmd+F on macOS, and a menu item on macOS.
   - Done on 2026-11-26: `add_full_screen` and `FULL_SCREEN` (`view.full_screen`) in `crates/dovetail-commands/src/window.rs`, which goes back to the state before. `Shortcut` and `MenuKey` gained the other modifier, written `MacCtrl`, so Control+Command+F can be written and the macOS menu binds Control too. Added to a menu by its id, it is in the macOS menu bar. Three unit tests.
8. [x] **Modal windows:** a `Window` with an owner that blocks the owner's input until it closes, as step 1 decides. It is kept in front of its owner, and centred on it when it opens. Pointer presses and keys to the owner are refused, and the owner cannot be closed while it is blocked (criterion 4).
   - Done on 2026-11-26:
     - `Window { modal: true }`. Its owner is the window that shows its component. The desktop refuses the owner's presses, wheel, keys, text and close, through the system and through `Context`'s input for tests, and a press brings the modal window to the front.
     - The system's ownership: an owned window on Windows, the dialog type and `WM_TRANSIENT_FOR` on X11, and a child window on macOS (`platform::make_owned`). It opens centred on its owner.
     - `TestUi` blocks the same way. A headless test checks that clicks and keys to the owner are refused until the modal window closes (criterion 4), and the example's self-test checks it in real windows.
     - Not done: marking the owner's accessibility nodes as disabled (see the follow-ups under Prototypes and open decisions).
9. [x] **`dovetail dev`** keeps every window's place across restarts with the saved state from step 6, in place of its own one-line file.
   - Done on 2026-11-26: `DevState` in `crates/dovetail-desktop/src/dev.rs` keeps a `SavedWindows` in `DOVETAIL_DEV_STATE`, with each window named by the order it opened in, and restores each window fitted onto the monitors there are. A file from before is written anew. Not yet run under `dovetail dev` by hand.
10. [x] **Docking's floating windows** use the `Window` component and its saved state, so that they are fitted onto the monitors there are, as other windows are.
    - Done on 2026-11-26: `DockArea` shows a `Window` for each floating stack, opened at the layout's place fitted onto the monitors there are. The dock layout still keeps the places, so docking needs no `SavedWindows`. They close when the dock area goes. The docking example's self-test passes, with a panel dragged out into its own window and back.
11. [x] **An example**, `examples/multi-window`:
    - a main window, a window for each "document" it opens, a settings window that is modal, and a full-screen command;
    - its `--self-test`, with `TestWindows`:
      - opens several windows, moves, resizes and maximises them, closes them and opens them again, and checks each comes back where it was and in its state;
      - checks a modal window blocks clicks and keys to its owner;
      - enters and leaves full screen;
      - checks the application stays running or ends after its last window, as it chose (criteria 1, 4, 5 and 6);
    - CI runs the self-test on all three systems.
    - Done on 2026-11-26:
      - `examples/multi-window`, run with `run_windows`. Its windows are kept in `windows.txt` in its settings folder.
      - Its self-test, with `set_root`, `window_titled`, `move_to` and `request_close`, passes here: three windows moved, resized and maximised come back as they were after every window closed and the application started again, the settings window blocks clicks, keys and closing in its owner, the shortcut enters and leaves full screen, and with no windows left the application ends (it would keep running on macOS).
      - CI runs it on all three systems. On Linux the window tests and this self-test run with Openbox, a small window manager, since without one X11 windows cannot be maximised. Not yet seen passing in CI.
12. [x] **Documents:** the architecture overview and crates page, the testing guide for the headless window host, the glossary, AGENTS.md if a rule holds across milestones, and this plan's record of each step.
    - Done on 2026-11-26: the architecture overview, the crates page, the testing guide (windows in `TestUi`, and whole applications in `TestWindows`), the glossary, the README's examples, and AGENTS.md, for `Window` components, `SavedWindows` and bringing windows to the front. D0029 was brought in line with what was built, and is still Proposed.

## Prototypes and open decisions

- **The shape of windows, saved state, monitors and modal windows:** to be decided in D0029 (step 1).
- **Modal windows by the system or by Dovetail.** The system gives the right behaviour, such as the owner flashing when it is clicked on Windows and the sheet-like stacking on macOS, but each system does it differently, and Linux has only a hint to the window manager. Blocking input in Dovetail works everywhere the same, but the owner can still be brought to the front. A short prototype on Windows and Linux shows which is good enough before D0029 is accepted.
- **Finding a monitor change.** winit has no event for it. The candidates are reading the list again when a window comes to the front or changes scale, and reading it on a timer while the application runs. The prototype checks what each system shows when a monitor is unplugged.
- **The prototype:** `prototypes/window-modal`, with its [report](../../architecture/window-modal-prototype.md). Its automatic check passed on Windows on 2026-11-26. It found that winit's `focus_window` sends an Alt press on Windows, which the owner gets. The checks by hand on Windows and Linux, and with a second monitor, are still to do.

- **Follow-ups, after M19's steps:**
  - D0029 is accepted once the prototype's checks by hand are done (its report has the table).
  - A modal window's owner marks its accessibility nodes as disabled while it is blocked, once assistive tools are checked by hand with modal windows.
  - Each system's own notice of a monitor change, such as `WM_DISPLAYCHANGE`, if reading the monitors on window events misses one that matters.
  - The macOS code (work area, child windows, the Dock click, the real full-screen state) and the X11 code (work area, `WM_TRANSIENT_FOR`) have only been type-checked. The checks by hand below cover them.
  - CI found that a state asked for can be missed: under Openbox when a window opens maximised, and on macOS when a window leaves full screen just after entering it. The desktop now asks again, every 300 ms up to eight times, until the window shows the state. On macOS it reads the real full-screen state from the `NSWindow`, and leaves full screen itself when AppKit refused to.
  - [B0010](../../bugs/B0010-docking-float-window-stays-open.md), fixed: the docking self-test checked a step only at a frame it had asked for, and a floating window now closes just after the frame that docks its panel. It now looks again until the check passes.

## Risks

- **Wayland cannot place windows.** A window's position cannot be read or restored there, so criterion 1 on Linux is checked on X11 for the position, and on Wayland for the size and maximised state only. The plan's manual checks say which.
- **Platform code that has never run.** The work area and modal windows need new code for each system in `platform/`, with `// SAFETY:` comments. The macOS code can only be type-checked here, so the manual checks below need a Mac.
- **Saved places in physical pixels go wrong across scale changes.** A window saved at 200 % and opened at 100 % would be twice the size. Sizes are saved in logical pixels, with the scale they were saved at, and the fitting function is tested with different scales.
- **A window saved off screen.** If the fitting function is wrong, a window could open where it cannot be seen. It always keeps at least the title bar's height and a part of the width on a work area, and a test checks this for monitors that are gone, smaller, or arranged differently.
- **Changing how the application ends breaks existing examples.** Every example ends after its last window today. The default stays the same on Windows and Linux, and each example's self-test is run again after step 4.
- **Real-window tests that change window state are slow or flaky** on CI, where the window manager on Linux is Xvfb with none at all. The self-test waits for each change with a time limit, and checks what the window says about itself, not the pixels.

## Testing

- **Headless, with `TestUi` and its window host:**
  - a `Window` in the tree opens when shown and closes when it goes, and its title and sizes follow its properties (criterion 3);
  - the list of windows, and the choice of ending after the last one.
- **Plain unit tests, on every system:**
  - the saved text: written and read back, older versions read, and an unreadable file gives no saved places;
  - the fitting function, with monitors removed, made smaller, moved and at other scales (criterion 2).
- **Real windows:** the example's self-test in CI on all three systems (criteria 1, 4, 5 and 6), and each example's self-test after a change to the desktop.
- **By hand, on Windows, Linux (GNOME on X11 and Wayland) and macOS:**
  - move a window, resize it and maximise it, close the application and run it again: it comes back the same (criterion 1);
  - save a window on a second monitor, unplug it, and run again: the window opens fully on the monitor that is left (criterion 2);
  - open the modal window: the owner cannot be clicked, typed into or closed until it goes (criterion 4);
  - on macOS, close the last window: the application stays in the Dock, and a click on its icon opens the main window again (criterion 5);
  - full screen with the command and with the system's own button.

## Done when

Copied from the overview.

1. A window closed and opened again comes back at the same size, position and maximised state, on Windows, Linux and macOS.
2. A window saved on a monitor that is no longer there opens fully on one that is.
3. A window in the component tree opens when shown and closes when it goes, checked by a test.
4. A modal window blocks input to its owner until it closes.
5. The application closes after its last window, or stays running on macOS, as it chooses.
6. The example's self-test checks several windows, their saved state, and full screen.
