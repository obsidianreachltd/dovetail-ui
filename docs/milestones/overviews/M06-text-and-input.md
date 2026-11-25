# M06 — Text and input

| Field | Value |
|---|---|
| ID | M06 |
| Status | In progress |
| Depends on | M04 |
| Plan | [M06 plan](../plans/M06-text-and-input.md) |
| Report | [M06 report](../reports/M06-text-and-input.md) |

## Summary

Build full text handling in `dovetail-text`: shaping, editing, selection, IME and clipboard. Also build keyboard focus and keyboard input routing.

## Why it matters

Professional applications depend on text input that behaves like the platform. This is the largest risk in building our own stack ([D0002](../../decisions/D0002-build-own-stack.md)).

## What it includes

- Real fonts, a rasteriser, text shaping for Latin and CJK, and font fallback. Complex scripts and right-to-left text are in [M13](M13-complex-scripts.md).
- Single-line and multi-line text fields, with a caret, selection and editing.
- IME composition, used to type languages such as Chinese and Japanese.
- Copy, cut and paste.
- Focus order, focus that can be seen, and keyboard events sent to the focused component.

## What it leaves out

- Accessibility of text fields ([M07](M07-accessibility.md)).
- Rich text styles (after the first release).

## Done when

1. Latin text and Chinese, Japanese or Korean text are shaped correctly, with font fallback. Complex scripts such as Arabic moved to [M13](M13-complex-scripts.md) on 2026-11-20.
2. Single-line and multi-line text fields support a caret, selection with mouse and keyboard, and editing.
3. IME composition works on Windows, Linux and macOS.
4. Copy, cut and paste work with the system clipboard.
5. Tab and Shift+Tab move focus in a predictable order, and the focused component is visible.
6. Keyboard events go to the focused component, and unhandled shortcuts pass to its parents.
7. Text is sharp at 100%, 150% and 200% display scale.
