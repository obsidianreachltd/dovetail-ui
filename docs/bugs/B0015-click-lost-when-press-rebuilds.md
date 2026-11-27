# B0015 — A click is lost when the press builds the element again

| Field | Value |
|---|---|
| ID | B0015 |
| Status | Fixed |
| Severity | High |
| Milestone | M25 |
| Found on | 2026-11-27 |
| Platform | All |

## Summary

A click did nothing when pressing the button changed state that built the pressed element again before the button was let go. This happens when the press moves focus and the control's look follows focus.

## Steps to reproduce

1. Make an element with an `on_press` handler whose component is built again when focus moves. For example, a time field whose border follows focus, with a stepper button inside it.
2. Click the stepper button while the field has focus. The press moves focus to the button, and the field is built again with a new handler for the button.
3. Let go of the button.

## Expected

The button is pressed once.

## Actual

Nothing happens. The session kept the handler it found when the button went down, and pressed only if the same handler was under the pointer when it came up. Building the element again made a new handler, so the two did not match.

## Notes

Found while writing M25's button groups, time field and tool palette. Each records where the keyboard is when focus moves. That state change built the buttons again between the press and the release.

## Fix

Fixed on 2026-11-27, on the M25 branch:

- `Layout::press_target_at` gives the key of the element whose press handler is under a point. `hit_test` is built on it.
- The session keeps the pressed element's key, not its handler. On release, if the same element is under the pointer, it calls that element's handler as it is now.

The test `a_click_that_builds_the_element_again_still_presses_it` in `crates/dovetail-session/tests/session.rs` covers it. The M25 button groups and palette also set their keyboard position when a button is pressed, not when it takes focus.
