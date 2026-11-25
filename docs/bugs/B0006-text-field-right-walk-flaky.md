# B0006 — The text field self-test's Right walk sometimes ends early

| Field | Value |
|---|---|
| ID | B0006 |
| Status | Open |
| Severity | Low |
| Milestone | M13 |
| Found on | 2026-11-23 |
| Platform | Windows |

## Summary

The step of `examples/text-field --self-test` that presses Right through mixed left-to-right and right-to-left text failed once in three runs. It stopped before it reached the end of the line. The other two runs passed.

## Steps to reproduce

1. Run `cargo run -p example-text-field -- --self-test` several times.

## Expected

Every run prints "PASS  Right visits all 11 caret places in mixed text once each, moving right on the screen".

## Actual

One run printed:

```text
FAIL  Right did not visit every place left to right: [(0, 33.0), (1, 40.123047), (2, 48.353516), (3, 52.188477), (11, 52.188477), (9, 60.371094)]
```

## Notes

- The walk presses Right once a frame, and takes a frame in which the caret did not move as the end of the line. The places it visited are right, in the right order, so the shaping and caret code look correct. The walk seems to have seen one frame twice and stopped early.
- Found while running every self-test for M15. Nothing in M15 changes caret movement, and the failure did not come back in two further runs.
- A likely fix: stop the walk only when Right is pressed at a place and the next laid-out frame shows the same place, and count frames so a missed frame does not end it.

## Fix

