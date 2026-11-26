# B0009 — A timer that ends after its component has gone panics

| Field | Value |
|---|---|
| ID | B0009 |
| Status | Fixed |
| Severity | High |
| Milestone | M18 |
| Found on | 2026-11-26 |
| Platform | All |

## Summary

An animated `Image` and a `Tooltip` each wait with `Task::after`. When the timer ended after the control had been removed, its callback changed the control's state, and the program panicked with "this state belonged to a component that has been removed".

## Steps to reproduce

1. Show an animated `Image`, or rest the pointer on a `Tooltip`'s content.
2. Remove the control, or close its window, before the next frame or the tooltip's wait.
3. Let the timer end.

## Expected

Nothing happens. The control is gone, so there is nothing to change.

## Actual

`examples/motion --self-test` failed on all three systems in CI, as its window closed while the spinner played:

```text
thread 'main' panicked at crates\dovetail-core\src\reactive.rs:431:27:
this state belonged to a component that has been removed
```

It passed on Windows by hand, where the window closed before the spinner's next frame.

## Notes

- Nothing stops a timer when its component is removed. `Task` is a copyable handle, so dropping it does not cancel it. The `Image`'s timer also holds the control's `Loader`, so the `Loader` is never dropped while the timer waits.
- A document's recovery timer is not affected: dropping its `Snapshots` cancels the timer, and a test checks this.
- A way for a component to clean up when it is removed would stop all such timers. That is not part of M18.

## Fix

The two callbacks change their state with `try_update`, and stop if the state has gone. Two tests remove each control while its timer waits, then move the clock past its end: `an_animation_removed_while_it_plays_stops_quietly` in `crates/dovetail-controls/tests/media.rs`, and `a_tooltip_removed_while_it_waits_stops_quietly` in `crates/dovetail-testing/tests/testing.rs`.
