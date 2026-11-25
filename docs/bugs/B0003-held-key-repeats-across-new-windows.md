# B0003 — A held key acts again in every new window

| Field | Value |
|---|---|
| ID | B0003 |
| Status | Fixed |
| Severity | Medium |
| Milestone | M01 |
| Found | 2026-11-20 |
| Platform | Windows (also X11 by winit's documentation) |

## Summary

In `examples/window`, pressing `N` once opened many windows instead of one.

## Steps to reproduce

1. `cargo run -p example-window`, and click the window so it has focus.
2. Press and hold `N` for about a second.

## Expected

One new window opens.

## Actual

A chain of windows opens: 12 in a one-second hold, measured by holding the key through the Windows input API.

## Notes

- When `N` opens a window, the new window takes focus while `N` is still down. winit then reports the held key to that window as a press with `is_synthetic: true`.
- `dovetail-desktop` skipped key repeats (`event.repeat`), but not synthetic presses. Each new window opened another.
- A quick tap could also do it, if the new window took focus before the key was released.
- It was reported by the user on 2026-11-20 as "pressing N once creates multiple windows".

## Fix

`Runner::window_event` in `crates/dovetail-desktop/src/app.rs` now ignores keyboard events with `is_synthetic: true`, as well as repeats.

Checked on 2026-11-20 on Windows. The test held `N` for one second through `keybd_event`, then pressed `L` to confirm the window had keyboard focus. Exactly one new window opened in 3 of 3 held runs and 2 of 2 tapped runs. Before the fix, one held run opened 12.

There is no automatic test: it needs real focus changes and real keyboard input, which the self-tests do not drive. M06 (keyboard input) should add one.
