# B0013 — On macOS a window opened maximised does not go back to its place

| Field | Value |
|---|---|
| ID | B0013 |
| Status | In progress |
| Severity | Medium |
| Milestone | M19 |
| Found on | 2026-11-27 |
| Platform | macOS |

## Summary

The multi-window self-test opens its main window maximised, as it was saved, then puts it back in its normal state. On macOS in CI the window did not go back to its saved place and size.

## Steps to reproduce

1. On macOS, run `cargo run -p example-multi-window -- --self-test`.

## Expected

The main window goes back to the place and size it had before it was maximised.

## Actual

```text
PASS  three windows open, moved, resized and the main one maximised

thread 'main' panicked at examples/multi-window/src/self_test.rs:136:5:
timed out after 10s waiting for the main window's own place.
The windows have received:
  "Multi-window": 7 redraw requests, 4 frames shown, 3 skipped, hidden Some(false), focused Some(true)
```

## Notes

- winit 0.30.13 maximises a resizable window on macOS with AppKit's `zoom:`, and leaves it the same way. AppKit takes the window back to the frame it remembers, which need not be the one it was opened at when it opened zoomed. If its frame moves even slightly, AppKit no longer counts it as zoomed, and winit's `set_maximized(false)` does nothing, because it sees a window that is already normal.
- Either way, winit reports the window as normal at its maximised size, so asking again for the normal state does not help.
- This has not been seen by hand on a Mac. The cause is read from winit's code, and not proven.

## Fix

The desktop remembers each window's normal place: the place it was opened at, when it opens in another state, and its place after each move or resize while it is normal. On macOS, if a window asked for its normal state is normal 300 ms later but not at that place, the desktop puts it there. It needs a CI run on macOS to confirm, and then a check by hand.
