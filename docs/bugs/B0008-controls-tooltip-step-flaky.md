# B0008 — The controls self-test's tooltip step sometimes times out

| Field | Value |
|---|---|
| ID | B0008 |
| Status | Open |
| Severity | Low |
| Milestone | M15 |
| Found on | 2026-11-25 |
| Platform | Windows |

## Summary

The step of `examples/controls --self-test` that rests the pointer on the Save button and waits for its tooltip failed once in seven runs. The tooltip never showed, and the self-test's 60-second limit ended it. The other six runs passed.

## Steps to reproduce

1. Run `cargo run -p example-controls -- --self-test` several times.

## Expected

Every run prints "PASS  a tooltip shows after the pointer rests".

## Actual

One run printed:

```text
FAIL  timed out waiting for: a tooltip shows after the pointer rests
```

The line after it, which gives the step's last check, was not kept.

## Notes

- It happened while M20 step 4 was checked, after the accessibility tree moved into `dovetail-session`. That change does not touch hover, timers or tasks, and the step passed five runs in a row afterwards.
- **The suspected cause:** real mouse events reach the window while the self-test runs. The step moves the window's pointer onto the button with `Context::pointer`, and the tooltip waits 500 ms. A real `CursorMoved` or `CursorLeft` in that time moves the pointer off the button, so the tooltip's timer is cancelled. Nothing moves it back, so the step waits until its limit. This is not proven.
- The test clock and headless tests in M20 step 6 can test the tooltip without real time or a real window. Running the self-test with the mouse kept still, or away from the window, would test the suspected cause.

## Fix
