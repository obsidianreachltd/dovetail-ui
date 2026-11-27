# B0014 — On macOS an owned window panics AccessKit when it opens

| Field | Value |
|---|---|
| ID | B0014 |
| Status | Fixed |
| Severity | Critical |
| Milestone | M19 |
| Found on | 2026-11-27 |
| Platform | macOS |

## Summary

On macOS, opening a UI window that has an owner, such as the multi-window example's modal Settings window, panicked in AccessKit's winit adapter. The window was shown before the adapter was made.

## Steps to reproduce

1. On macOS, run `cargo run -p example-multi-window -- --self-test`.

## Expected

The Settings window opens above the main window, and the self-test goes on.

## Actual

```text
PASS  closed and opened again, each window came back at its size, position and state

thread 'main' panicked at accesskit_winit-0.34.2/src/lib.rs:198:13:
The AccessKit winit adapter must be created before the window is shown (made visible) for the first time.
```

## Notes

- A UI window opens hidden, its AccessKit adapter is made, and then it is shown, because the adapter needs that order.
- On macOS the desktop makes an owned window a child of its owner with AppKit's `addChildWindow:ordered:`. That call also shows the child window. It was made while the window was still being opened, before the adapter.
- Windows and Linux were not affected: there the owner is set without showing the window.

## Fix

A hidden window is given to its owner only after its adapter is made and it is shown. A window opened visible is still given to its owner as it opens. A CI run on macOS then opened the Settings window without the panic. The multi-window self-test covers it.
