# M11 — Documents

| Field | Value |
|---|---|
| ID | M11 |
| Status | In progress |
| Depends on | M08 |
| Plan | [M11 plan](../plans/M11-documents.md) |
| Report | — |

## Summary

Build the document lifecycle in `dovetail-documents`: saving, reopening, tracking unsaved changes, recovery after a crash, and long-running work.

## Why it matters

Many desktop applications open and save documents, and the hardest ones Dovetail aims at are document-based editors ([goals](../../vision/goals.md)). Applications without documents leave this package out.

## What it includes

- Document state kept outside widget state, changed only through commands.
- Save, Save As and Open.
- Tracking unsaved changes, and a warning before closing without saving.
- Automatic saving and recovery after a crash.
- Long-running work with progress and cancellation.

## What it leaves out

- File formats for real PCB designs (not part of the framework).

## Done when

1. Document state lives outside widget state, and every edit goes through a command.
2. A round-trip test shows that saving and reopening gives the same document.
3. Unsaved changes are shown in the window title, and closing asks first.
4. A test that stops the process during editing shows the work is recovered on the next start.
5. A long operation shows progress and can be cancelled.
