# M08 — Commands and undo

| Field | Value |
|---|---|
| ID | M08 |
| Status | In progress |
| Depends on | M06 |
| Plan | [M08 plan](../plans/M08-commands-and-undo.md) |
| Report | — |

## Summary

Build the shared command system in `dovetail-commands`. It covers menus, toolbar buttons, shortcuts and command search, plus undo and redo.

## Why it matters

A command is defined once and used everywhere. Document editing and undo depend on it.

## What it includes

- Commands with a name, a shortcut, and an enabled or disabled state.
- Menu items, toolbar buttons, keyboard shortcuts and command search that all call the same command.
- Native menus where the platform has them, such as the macOS menu bar, and in-window menus elsewhere.
- An undo and redo stack for edits made through commands.
- Configurable shortcuts, with conflict detection.

## What it leaves out

- Saving documents ([M11](M11-documents.md)).

## Done when

1. A command that is defined once can be run from a menu item, a toolbar button, a shortcut and command search.
2. Enabling or disabling a command changes every place it appears.
3. Native menus work on macOS, and in-window menus work on Windows and Linux.
4. Tests show that edits made through commands can be undone and redone.
5. Shortcuts can be changed, and conflicts are reported.
