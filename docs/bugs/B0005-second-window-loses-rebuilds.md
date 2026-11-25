# B0005 — A second UI window loses the other window's rebuilds

| Field | Value |
|---|---|
| ID | B0005 |
| Status | Fixed |
| Severity | High |
| Milestone | M10 |
| Found on | 2026-11-22 |
| Platform | All |

## Summary

With two UI windows open, a state change read by a component in one window may never be shown, because the other window's update throws its rebuild away.

## Steps to reproduce

1. Open two UI windows whose components read the same signal.
2. Change the signal.
3. Both windows redraw. The first one to draw calls `Tree::update`.

## Expected

Both windows build their components again and show the new value.

## Actual

`Tree::update` takes every marked component from the runtime, and keeps only those in its own tree. The marks for the other window are dropped, so that window shows the old value.

## Notes

Found while reading the code for M10, before any example used two UI windows. Floating panels need two UI windows that share state.

## Fix

Fixed on 2026-11-22 for M10. `Tree::update` takes only the marked components that belong to it (`take_dirty_where`), and leaves the others marked for their own trees. `dovetail-core/tests/two_trees.rs` covers it, and `examples/docking --self-test` shows two windows sharing state.
