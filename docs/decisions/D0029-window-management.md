# D0029 — Windows in the component tree, saved as text, fitted onto the monitors there are

| Field | Value |
|---|---|
| ID | D0029 |
| Status | Proposed |
| Date | 2026-11-26 |

## Context

M19 gives applications full control of their top-level windows ([M19 plan](../milestones/plans/M19-window-management.md), step 1). Six questions must be settled first:

- **Declaring a window:** today only docking opens windows from components, through `dovetail-core`'s `WindowHost` ([D0016](D0016-docking-foundations.md)). The application opens the others itself, with `Context::open_window`.
- **What is saved:** only `dovetail dev` keeps a place, for the first window, in physical pixels, in a one-line file.
- **Fitting:** a saved place may be on a monitor that is gone, or at another scale.
- **Modal windows:** winit 0.30.13 has none. Each system has its own way, and Linux has only a hint to the window manager.
- **Monitor changes:** winit does not report them.
- **Ending:** the program ends when no windows are left, on every system. macOS applications usually keep running.

## Decision

Windows are declared in the component tree with a `Window` component in `dovetail-core`, saved as Dovetail's own text in logical pixels, and fitted onto the monitors there are by a plain function. Dovetail blocks input to a modal window's owner itself, on every system, and uses the system's ownership where it has one.

### The `Window` component: in `dovetail-core`, built on the window host

- **`Window`** opens its window when it is first shown, and closes it when it goes from the tree. Its properties are a title, a size, a minimum and maximum size, whether it can be resized, a state, an optional `name` for saving, an optional `owner` and `modal`, `on_close`, and its content as children.
- **It lives in `dovetail-core`,** beside `WindowHost`. It draws nothing, so the rule that concrete widgets live outside core still holds. Docking and documents use it without depending on the controls or the desktop.
- **Its content stays with the component that declares it.** The content is built in the new window's own tree, with the declaring component as its `context`, as floating panels are now. So it sees the same provided values, such as the theme, and state made with `use_state` in the declaring component lives on while the window moves, closes and opens again.
- **Properties follow while it is shown.** A new title, size limit or state is passed to the host, which applies it with winit. When the user changes the state or place, the host tells the component, which the application reads with `on_place` and `on_state`.
- **Closing from the title bar calls `on_close`.** The window stays until the component stops showing it, so the application decides, as `DocumentFrame` does now. With no `on_close`, the window closes, and opens again only when the component is shown anew.
- **`WindowHost` grows** to take the new options, and to set a window's state and size limits after it opens. `dovetail-testing` gains a headless host, so `TestUi` can find the windows a tree has open, look inside them, and close them.
- **The application's root:** `run_windows(root)` runs an application whose windows are all `Window`s. The root is a component tree with no window of its own, so the application can run with none open. `dovetail::run`, `Context::open_window` and `Context::open_ui_window` stay for programs with one window, and for those that do not use components, such as the GPU examples.

### Window options and state

- **`WindowState`:** `Normal`, `Maximised`, `Minimised` or `FullScreen`.
- **Full screen** is borderless, on the monitor the window is on: winit's `Fullscreen::Borderless`. On macOS that is the system's own full-screen space. Changing the display mode, as games do, is left out.
- **The full-screen command** is `view.full_screen` in `dovetail-commands`, with F11 on Windows and Linux and Control+Command+F on macOS. A shortcut can now hold the other modifier (Control on macOS, the Windows key elsewhere), written `MacCtrl`, and so can a menu's key.
- **The application's windows:** `windows()` lists the open ones, with their names, titles and states, and remembers the component as a reader, as `system_appearance()` does. `bring_to_front` brings one to the front.

### Saved state: text of our own, in logical pixels

- **`SavedWindows`,** made with `use_state(SavedWindows::new)`, holds the saved state of every window with a `name`. A `Window` given it, with a name, opens where its name was saved, and writes its place back when it changes.
- **`SavedWindows::keep_in_file`** reads a file and writes it again when anything changes, as `Dock::keep_in_file` does. An unreadable file gives no saved places, and the error is returned.
- **The format** follows D0016. The first line is `dovetail-windows 1`. Each window has one line with its name, its position and size in logical pixels, the scale it was saved at, its state, and the monitor it was on, by name, position and size.
- **The normal place is saved,** not the maximised one. While a window is maximised or full screen, the last place it had in its normal state is kept, so it goes back there when it is restored.
- **A window saved minimised opens in its state from before**, normal or maximised. A window saved full screen opens full screen.
- **A saved state wins over the `state` property when the window opens.** The component is told with `on_state`, so the property follows it from then on.
- **On Wayland** no position is read or saved. The size and state are restored, and the system places the window.

### Fitting: a plain function, tested everywhere

- **`fit(saved, monitors)`** returns the place to open at. It has no platform code, so unit tests check it on every system.
- **The monitor:** the saved one if a monitor with the same name, position and size is there. If not, the one the saved rectangle overlaps most. If none, the primary monitor, with the window centred on it.
- **The size** is kept in logical pixels, so a window saved at 200 % opens the same size at 100 %. It shrinks to fit the monitor's work area if it is too big, but not below its minimum size.
- **The position** is moved so the whole window, frame included, is inside the work area. That is what criterion 2 asks.
- **Monitors** come from winit, with their name, position, size and scale. The work area is read by platform code: `GetMonitorInfoW` on Windows, `NSScreen`'s `visibleFrame` on macOS, and `_NET_WORKAREA` on X11. Where it cannot be read, as on Wayland, it is the whole monitor.

### Modal windows: Dovetail blocks, the system helps

- **Dovetail blocks the owner itself, on every system.** While a modal window is open, the desktop refuses the owner's pointer presses, wheel, keys and text, and refuses its title bar's close. Hover and painting go on, so the owner still draws. A press on the owner brings the modal window to the front.
- **The system's ownership keeps the modal window in front of its owner:**
  - on Windows, an owned window, with winit's `with_owner_window`;
  - on macOS, a child window of the owner, with `addChildWindow` in platform code;
  - on X11, the dialog window type, which winit sets, and `WM_TRANSIENT_FOR`, in platform code;
  - on Wayland, nothing yet: the modal window is a normal window, and Dovetail's blocking still holds.
- **A modal window opens centred on its owner,** and fitted onto the owner's monitor.
- **Accessibility:** the owner's tree is still there. Marking its nodes as disabled while it is blocked is left until assistive tools are checked by hand with modal windows.
- **Bringing a window to the front** goes through the desktop, which ignores the Alt press that winit's `focus_window` sends on Windows to be allowed to change the window in front. The prototype found it.
- **The prototype checks** that this is good enough on Windows and Linux before the decision is accepted. If it is not, the Windows owner is also disabled with `EnableWindow`, which gives the system's own flash and sound.

### Monitor changes: read again on window events

- **The desktop reads the monitor list again** when the application starts, and when a window changes scale, moves, comes to the front, or opens. When the list is different, the readers of `monitors()` are built again.
- **No timer.** Reading on a timer would wake the program while nothing happens, against [D0008](D0008-draw-on-demand.md).
- **Fitting at start is what criterion 2 needs,** and it reads the list as it is then. A monitor unplugged while the application runs is handled by the system, which moves its windows.
- **The prototype checks** what each system shows when a monitor is unplugged. Each system's own notice, such as `WM_DISPLAYCHANGE` on Windows, is added in platform code only if reading on window events misses a case that matters.

### Ending the application

- **`ExitPolicy`:** `AfterLastWindow` or `KeepRunning`. The application sets it when it starts, with `Context::set_exit_policy`, or passes it to `run_windows_with`.
- **The default for `run_windows`** is `AfterLastWindow` on Windows and Linux, as now, and `KeepRunning` on macOS, as macOS users expect. Programs run with `run` or `dovetail::run` end after their last window on every system, as they always have, so no example changes.
- **On macOS, a click on the Dock icon** with no window open builds the root anew, which opens its windows again. winit does not report the click, so platform code adds the reopen method to the application's delegate.
- **Under a root, the primary window** no longer closes every hosted window. A window declared inside another window's tree goes with it, because the tree goes. Docking's floating windows are declared in the dock area's tree, so they close with it. Programs with no root keep the old rule.

## Consequences

- **Windows are components,** so an application opens, closes and changes them by changing state, and a headless test checks them with `TestUi`.
- **Saved places survive scale changes,** because they are logical. A window never opens off screen, and the function that makes sure of it is tested on every system.
- **Docking and `dovetail dev`** move onto `Window` and `SavedWindows`, so all windows are saved and fitted the same way. `dovetail dev`'s one-line file goes.
- **`dovetail-core` still has no dependencies.** `Window`, `WindowState`, `SavedWindows`, the format and `fit` use only the standard library.
- **New platform code** for the work area, the modal ownership and the Dock click needs `// SAFETY:` comments ([D0013](D0013-scoped-unsafe-for-platform-code.md)). The macOS parts can only be type-checked here, and need a Mac for the checks by hand.
- **On Wayland,** positions are not saved, and modal windows are not kept in front by the system.
- **On macOS, applications run with `run_windows` keep running** after their last window closes, unless they choose to end. Other programs, including the examples that use `run`, end as before.
- **Docking's floating windows** are fitted onto the monitors there are when they open, as other windows are.
- **The window host's contract changes** for its existing user, docking, which is updated in step 10.

## Alternatives considered

- **`Window` in `dovetail-controls`.** Next to the other components, but docking and documents would then depend on the controls for their windows.
- **Opening windows only from `Context`.** No new component, but state would live outside the tree, and headless tests could not check windows.
- **Saving physical pixels, as `dovetail dev` does.** Simpler, but a window saved at one scale opens the wrong size at another.
- **serde for the saved text.** A new dependency needs its own decision, and the format is a few lines, as the docking layout is.
- **Modal windows by the system only.** The right look on each system, but each does it differently, Linux has only a hint, and Wayland nothing. Blocking in Dovetail behaves the same everywhere and is tested by the self-test.
- **Disabling the owner on Windows from the start.** The system's own behaviour, but it also stops the owner drawing hover changes, and is only on one system. Kept as the fallback if the prototype shows it is needed.
- **Reading the monitors on a timer.** It sees every change, but wakes the program every few seconds while nothing happens.
- **Keeping the program running after its last window on every system.** One rule, but not what Windows and Linux users expect, and it would leave examples running after they are closed.
