# R01 — First release

| Field | Value |
|---|---|
| ID | R01 |
| Status | Not started |
| Depends on | M05, M07, M09, M10, M11, M13, M14, M15, M16, M17, M18, M19, M20, M21, M22 |
| Plan | — |
| Report | — |

## Summary

Finish the validation editor in `examples/document-editor`, package it for all three operating systems, and publish the first version of the crates.

## Why it matters

This proves the framework works as a whole before the widget catalogue grows ([D0001](../../decisions/D0001-framework-is-the-product.md)).

## What it includes

The validation editor shows all ten acceptance points:

1. Creating the project through the CLI.
2. A window with a declarative component tree.
3. Reactive state and typed events.
4. A custom GPU canvas.
5. A dockable inspector.
6. One undoable document edit.
7. Saving and reopening.
8. Keyboard shortcuts and focus behaviour.
9. A usable development rebuild loop.
10. Builds on Windows, Linux and macOS.

The PCB-like scene supports selecting shapes, zooming, and editing the properties of a selected shape in the inspector.

Beyond the editor:

- Packaging for each operating system.
- API documentation for every published crate.
- The order of publishing and the compatibility policy.
- Removing the guard that stops the crates being published by mistake: `publish = false` in the root `Cargo.toml`'s `[workspace.package]`, which every framework crate takes with `publish.workspace = true`.

## What it leaves out

- Everything listed as "later" in [goals](../../vision/goals.md#first-release-and-later).

## Done when

1. The validation editor meets all ten acceptance points above.
2. CI builds and packages the editor for Windows, Linux and macOS.
3. The packages install and run on a clean machine with each operating system.
4. Every published crate has API documentation.
5. The crates are published in the documented order, with one shared version.
6. Performance results for the editor are recorded.
