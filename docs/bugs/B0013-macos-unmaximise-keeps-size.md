# B0013 — On macOS a window does not go back to, or open at, its saved place

| Field | Value |
|---|---|
| ID | B0013 |
| Status | In progress |
| Severity | Medium |
| Milestone | M19 |
| Found on | 2026-11-27 |
| Platform | macOS |

## Summary

The multi-window self-test opens its windows where they were saved. On macOS in CI, its main window, opened maximised, did not go back to its saved place and size when it was put back in its normal state. After a first fix, a document window, opened in its normal state, did not open at its saved place either.

## Steps to reproduce

1. On macOS, run `cargo run -p example-multi-window -- --self-test`.

## Expected

Each window opens at the place and size it was saved at, and the main window goes back to the place and size it had before it was maximised.

## Actual

```text
PASS  three windows open, moved, resized and the main one maximised

thread 'main' panicked at examples/multi-window/src/self_test.rs:136:5:
timed out after 10s waiting for the main window's own place.
The windows have received:
  "Multi-window": 7 redraw requests, 4 frames shown, 3 skipped, hidden Some(false), focused Some(true)
```

After the first fix, the main window went back, and the next wait failed:

```text
thread 'main' panicked at examples/multi-window/src/self_test.rs:146:5:
timed out after 10s waiting for document 1's own place.
The windows have received:
  "Document 1": 3 redraw requests, 1 frames shown, 2 skipped, hidden Some(false), focused Some(true)
  "Multi-window": 9 redraw requests, 6 frames shown, 3 skipped, hidden Some(false), focused Some(false)
```

## Notes

- winit 0.30.13 maximises a resizable window on macOS with AppKit's `zoom:`, and leaves it the same way. AppKit takes the window back to the frame it remembers, which need not be the one it was opened at when it opened zoomed. If its frame moves even slightly, AppKit no longer counts it as zoomed, and winit's `set_maximized(false)` does nothing, because it sees a window that is already normal.
- Either way, winit reports the window as normal at its maximised size, so asking again for the normal state does not help.
- Why the document window opened away from its place is not known: the message did not say where it was. The timeout messages of `TestWindows` now give each window's place, size and state, and the self-test's waits give the place they wait for.
- This has not been seen by hand on a Mac. The cause is read from winit's code, and not proven.
- While the desktop checks a new window's place, for up to 2.4 seconds, a window the user drags in its first 300 ms could be put back. Once the window has been at its place, it is not checked again.

## Fix

The desktop remembers each window's normal place: the place it was opened at, when it opens in another state, and its place after each move or resize while it is normal. On macOS, if a window asked for its normal state is normal 300 ms later but not at that place, the desktop puts it there. On macOS it also checks that a window opened at a place is there, and puts it there if not. With the macOS behaviour switched on for a run on Windows, the real-window tests and the multi-window, docking, window and documents self-tests passed. It needs a CI run on macOS to confirm, and then a check by hand.
