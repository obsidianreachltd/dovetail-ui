# B0010 — The docking self-test's drag-back step sometimes times out

| Field | Value |
|---|---|
| ID | B0010 |
| Status | Fixed |
| Severity | Low |
| Milestone | M19 |
| Found on | 2026-11-27 |
| Platform | All |

## Summary

The step of `examples/docking --self-test` that drags Console back from its floating window into the main window sometimes timed out, though Console was docked and its window had closed. It happened in 4 of about 50 runs on Windows.

## Steps to reproduce

1. Run `cargo run -p example-docking -- --self-test` many times.

## Expected

Every run prints "PASS  Console dragged back beside Board docks again, and its window closes".

## Actual

Some runs printed:

```text
FAIL  timed out waiting for: Console dragged back beside Board docks again, and its window closes
      the layout is:
dovetail-dock 1
root split row
  0.5713 split column
    0.5 split row
      0.5 tabs 0 console
      0.5 tabs 1 board inspector
    0.5 tabs 0 files
  0.4287 tabs 0 layers

      window "Docking self-test": 11 redraw requests, 11 frames shown, 0 skipped, hidden None, focused None, changes waiting false
```

Everything the step waits for was true by then: no floating windows, Console beside Inspector's stack, and one window open.

## Notes

- **The cause is in the self-test.** It checks a step when a frame is shown, and asked for another frame only after it acted. Since M19 step 10, a floating window is a `Window` component. It closes when the dock area is built again without it: its state is dropped, it asks the host to close it, and the desktop closes it in `serve_host`, just after the frame that showed Console docked. So the check at that frame saw two windows. Closing a window draws nothing, so no frame followed, and the check never ran again.
- Whether the close came before or after that frame depended on timing, which is why only some runs failed.
- Closing a window from the host one turn of the event loop later is how D0029's `Window` works, and is not a bug.

## Fix

The self-test asks for another frame whenever a step's check fails, so it looks again until the check passes or the time runs out. It also prints every open window when a step times out, with what the window has received and whether its tree has changes waiting. After the change, 15 runs in a row passed.
