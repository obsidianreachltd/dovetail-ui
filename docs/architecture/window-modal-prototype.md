# Modal windows and monitor changes: prototype report

This report answers the two open questions in the [M19 plan](../milestones/plans/M19-window-management.md):

- should a modal window block its owner through the system, or should Dovetail refuse the owner's input itself?
- how does the program find out that the monitors changed?

[D0029](../decisions/D0029-window-management.md) proposes an answer to both, and is accepted once this report's checks by hand are done.

The code is in `prototypes/window-modal`. It is throw-away code, but it stays in the repository so these results can be checked again. It has no `unsafe`, so the parts that need platform code are not tried here: the Windows `EnableWindow` fallback, macOS child windows, `WM_TRANSIENT_FOR` on X11, and the work area.

Run it by hand:

```sh
cargo run -p prototype-window-modal -- --block both
cargo run -p prototype-window-modal -- --block dovetail
cargo run -p prototype-window-modal -- --block system
```

Run it without a person with `--check`.

## The question

- **winit 0.30.13 has no modal windows.** On Windows it can make an owned window (`with_owner_window`). On X11 it can set the dialog window type. On macOS and Wayland it has nothing.
- **An owned window is not modal.** On Windows it stays in front of its owner, but the owner still takes clicks and keys. Real modal dialogs also disable the owner with `EnableWindow`.
- **winit does not report monitor changes.** The program can only read `available_monitors()` again.

## The design tried

- **An owner window,** blue, where `M` opens a modal window, green, centred on the owner. While the modal window is open the owner turns grey.
- **Three ways to block,** chosen with `--block`:
  - `dovetail`: the program refuses the owner's presses, wheel, keys, text and close itself. A press or close also brings the modal window to the front. The system knows nothing of the modal window.
  - `system`: the system's ownership only, as winit gives it, with nothing refused.
  - `both`: both, as D0029 proposes.
- **The blocking rule** is one function, `judge`, with unit tests. Hover, focus and painting go on, so the owner still draws.
- **Every press, key and focus change is logged,** with whether it was refused.
- **The monitor list is read again** on each focus change, move, resize and scale change, and on a timer (`--poll-ms`). Each change is logged with what saw it first. If the timer is ever first, window events alone missed that change.
- **`--check`** opens both windows, checks that the owner is blocked and the modal window is centred on it, closes the modal window, and checks that the owner is free again.

## Results

Run on 2026-11-26, on Windows 11 with one 3440 × 1440 monitor at a scale of 1:

| Check | Result |
|---|---|
| `--check --block both` | Passed. The modal window took focus at once, was blocked, and was 19 px from the owner's centre, which is the title bar's height. |
| `--check --block dovetail` | Passed, the same. |
| `--check --block system` | Passed. The owner was not blocked by Dovetail, as expected. |
| The blocking rule's unit tests | Passed. |
| Type-checked for Linux and macOS | Passed. |

### Alt presses from bringing a window to the front

When the modal window closes, the prototype brings the owner to the front with winit's `focus_window`. The owner then gets a key press and a release for `AltGraph` that nobody typed. winit does not mark them as synthetic.

winit's `focus_window` on Windows sends an Alt press with `SendInput` (`force_window_active` in its `platform_impl/windows/window.rs`), so the system allows it to change the window in front. It sends the left Alt key, which this keyboard layout reports as `AltGraph`. The window that comes to the front gets that press.

So M19's `bring_to_front` must ignore the key events that `focus_window` causes. Otherwise a menu bar, or a shortcut that uses Alt, could react to them.

### Still to check by hand

These need a person, and the monitor checks need a second monitor:

| Check | Windows | Linux (GNOME, X11) | Linux (GNOME, Wayland) |
|---|---|---|---|
| With `both`, clicks, keys and the close button on the owner do nothing, and the modal window comes to the front | | | |
| With `both`, the owner cannot be brought in front of the modal window, from the taskbar or Alt+Tab | | | |
| With `dovetail`, the owner can be brought in front of the modal window, and a click on it brings the modal window back | | | |
| With `system`, the owner still takes clicks: an owned window alone is not enough | | | |
| Whether the missing flash and sound on the owner, which `EnableWindow` would give, are missed | | | |
| Unplug a monitor: what saw the change first, a window event or the timer | | | |
| Plug it in again: the same | | | |
| Change a monitor's scale in the system settings: the same | | | |

macOS is left out here. winit gives it nothing for modal windows, so `both` there is `dovetail` alone until the platform code adds child windows.

## Measurements

- **Opening:** the modal window was focused within 1 ms of opening, in all three modes.
- **Closing:** the Alt press from `focus_window` reached the owner 11 ms after the modal window closed.
- **The timer:** its cost was not measured. It wakes the program once a second while nothing happens, which [D0008](../decisions/D0008-draw-on-demand.md) rules out for the real code.

## What decides

- **Blocking:** whether `both` is good enough without `EnableWindow` on Windows, and with only the dialog type on X11. If the owner can come in front of its modal window there, or the missing flash is missed, D0029 uses `EnableWindow` on Windows from the start.
- **Monitor changes:** whether the timer is ever first. If window events always see a change before the timer, or at once after it, D0029's plan holds. If the timer sees changes that no window event does, each system's own notice is added in platform code.

## Recommendation

Accept D0029 as it is, once the checks by hand agree:

- **Block in Dovetail, and use the system's ownership where it exists.** The automatic check shows the rule works, and refusing input in Dovetail is the only way that behaves the same on every system.
- **Read the monitors again on window events, with no timer.**
- **Add a line to D0029's consequences** about the Alt press that winit's `focus_window` sends on Windows, so `bring_to_front` ignores it.

If a check by hand disagrees, write the change into D0029 before it is accepted. It is still Proposed, so it can be edited.
