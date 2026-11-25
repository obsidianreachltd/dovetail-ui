# M07 — Accessibility

| Field | Value |
|---|---|
| ID | M07 |
| Status | In progress |
| Depends on | M06 |
| Plan | [M07 plan](../plans/M07-accessibility.md) |
| Report | — |

## Summary

Build an AccessKit accessibility tree from the component tree, keep it in step with changes and focus, and test it with screen readers on each OS ([D0003](../../decisions/D0003-accesskit-for-accessibility.md)).

## Why it matters

Applications built with Dovetail must be usable with assistive tools. The tree must be correct from the first release, because adding it later is much harder.

## What it includes

- A way for each component to describe its role, name, state and actions.
- Descriptions for the built-in controls and text fields.
- Updates to the tree that change only what changed.
- Keeping the tree's focus the same as keyboard focus.
- Handling actions that come from assistive tools.

## What it leaves out

- Accessibility of the custom canvas contents ([M09](M09-custom-gpu-canvas.md)).

## Done when

1. Every built-in control declares a role, name, state and actions.
2. The AccessKit tree is built from the component tree and updated only where it changed.
3. Focus in the accessibility tree always matches keyboard focus.
4. Narrator (Windows), VoiceOver (macOS) and Orca (Linux) read the Counter example and a text field correctly.
5. Actions from assistive tools, such as pressing a button, work.
