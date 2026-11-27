# B0011 — The motion self-test sometimes sees one frame while the spinner should be still

| Field | Value |
|---|---|
| ID | B0011 |
| Status | Open |
| Severity | Low |
| Milestone | M18 |
| Found on | 2026-11-27 |
| Platform | Windows |

## Summary

`examples/motion --self-test` turns on less motion and checks that no frame is shown in the next 400 ms. Twice in about 25 runs, one frame was shown.

## Steps to reproduce

1. Run `cargo run -p example-motion -- --self-test` many times, straight after other self-tests.

## Expected

Every run prints "PASS  with less motion the box jumps and no frames are drawn".

## Actual

```text
thread 'main' panicked at examples\motion\src\self_test.rs:103:3:
assertion `left == right` failed: no frames: the spinner is still too
  left: 22
 right: 21
```

## Notes

- Both failures came in runs straight after other self-tests had run, while M19's fixes for CI were checked. In 18 runs on their own, before and after those fixes, it passed every time.
- Those fixes do not change what the desktop does on Windows for a window that is not asked for a state, so they are not thought to be the cause.
- **Suspected cause, not proven:** the system asks the window to draw, as when another window moves over it or it comes to the front, and that frame is counted. The check cannot tell a frame the spinner asked for from one the system asked for.
- A fix could count only frames that Dovetail asked for, or check that the spinner's component was not built again.

## Fix
