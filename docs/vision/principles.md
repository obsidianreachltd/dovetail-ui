# Principles

These rules guide design choices. If a change breaks one, record a decision in [decisions](../decisions/README.md) first.

## Modular packages

- Every UI element is an optional dependency. An application imports only the packages it uses.
- Start with a few practical packages. Split one only when its dependencies, its maintenance, or the way it is used makes that worth it.
- Do not make a crate for every small widget.

## Dependencies

- The core crate must not depend on any concrete widget package.
- An optional capability brings in its dependencies only when it is selected.
- Built-in components and third-party components use the same public contracts. Nothing built-in gets a private shortcut.
- Styling, focus, input and accessibility behave the same way across all components.
- Procedural macros live in their own crate, separate from the runtime.
- The default features of the `dovetail-ui` facade crate, and how Cargo features combine, must be checked before they are fixed.
- Foundation libraries are limited to winit, wgpu and AccessKit ([D0002](../decisions/D0002-build-own-stack.md)). Anything else needs a decision.

## Professional desktop behaviour

The architecture must leave room for all of the following from the start, even if they are built in later milestones:

- keyboard navigation and predictable focus
- text input, input method (IME) composition, selection and clipboard
- accessibility, through the platform's own accessibility services
- several windows, and high-DPI displays
- menus, shortcuts, dialogs, and drag-and-drop
- dockable panels, and workspace layouts that are saved between runs
- commands, and undo and redo
- long-running work with progress and cancellation
- saving documents, tracking unsaved changes, and recovery after a crash

## One command system

A menu item, a toolbar button, a keyboard shortcut and an entry in command search all call the same command. A command is defined once.

## Documents and state

- Document data lives outside the short-lived state of widgets.
- Every edit that can be undone goes through the application's command model.

## Measured performance

Performance goals must be measurable. Each milestone that affects speed records its numbers in its completion report.
