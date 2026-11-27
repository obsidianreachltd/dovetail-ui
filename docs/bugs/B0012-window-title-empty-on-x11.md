# B0012 — A window's title reads back empty on X11

| Field | Value |
|---|---|
| ID | B0012 |
| Status | Fixed |
| Severity | Medium |
| Milestone | M19 |
| Found on | 2026-11-27 |
| Platform | Linux |

## Summary

On X11, `Context::window_title` and the list of open windows gave every window an empty title. The window itself showed its title. The multi-window self-test, which waits for a window by its title, timed out in CI on Linux.

## Steps to reproduce

1. On Linux with X11, run `cargo run -p example-multi-window -- --self-test`.

## Expected

The self-test finds the window titled "Multi-window" and goes on.

## Actual

```text
thread 'main' panicked at examples/multi-window/src/self_test.rs:49:5:
timed out after 10s waiting for a window titled "Multi-window".
The windows have received:
  "": 3 redraw requests, 3 frames shown, 0 skipped, hidden Some(false), focused Some(true)
```

## Notes

- winit 0.30.13's `Window::title` on X11 always returns an empty string (`platform_impl/linux/x11/window.rs`). It reads the title back on Windows, macOS and Wayland.
- The empty titles in the timeout messages of the Linux real-window tests had the same cause.
- Windows and macOS were not affected, so tests there did not find it.

## Fix

The desktop keeps each window's title itself, set when the window opens and whenever its title changes, and reads it from there instead of from winit. `Context::window_title`, the list of open windows, and the tests that find windows by title all use it.
