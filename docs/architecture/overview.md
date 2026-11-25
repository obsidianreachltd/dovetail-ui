# Architecture overview

This page describes the intended shape of the framework. Parts marked **open** need a prototype or a decision before they are fixed.

## Layers

From the bottom up:

1. **Platform**: windows, the event loop and input, through winit (`dovetail-desktop`).
2. **Rendering**: GPU access through wgpu (`dovetail-render`).
3. **Layout**: sizes and positions (`dovetail-layout`, our own code).
4. **Core**: components, identity, events and reactive state (`dovetail-core`).
5. **Declarative syntax**: the `ui!` macro (`dovetail-macros`).
6. **Capabilities**: controls, text, docking, commands, documents and more.
7. **Facade**: the `dovetail-ui` package, used in code as `dovetail`, which re-exports the common parts ([D0006](../decisions/D0006-facade-crate-name.md)).

See [crates](crates.md) for the full package map.

## Component and state model

Components persist between updates, and each one has an explicit identity.

- **Properties** are a component's inputs.
- **Events** report what the user did.
- **Reactive state** causes updates when it changes.
- **Local state** belongs to one component instance.
- **Stable keys** keep identity when items in a list are added, removed or reordered.
- **Removing** a component cleans up its subscriptions and resources.

**Open:** how updates are scheduled, and how a change marks layout, painting and accessibility information as out of date. This is decided in [M02](../milestones/overviews/M02-component-and-state-core.md).

**Open:** the form of reactive values. The `ui!` example in [syntax](syntax.md) needs reactive handles that are cheap to copy into closures. Prototypes compare at least two designs in M02.

Document state lives outside widget state. Undoable edits go through the command model ([M08](../milestones/overviews/M08-commands-and-undo.md), [M11](../milestones/overviews/M11-documents.md)).

## Rendering and platform

- winit provides windows and the event loop. wgpu provides GPU rendering. Both are checked in [M01](../milestones/overviews/M01-window-and-gpu.md) before we commit to them.
- Layout, text and 2D painting are our own code ([D0002](../decisions/D0002-build-own-stack.md)).
- Accessibility goes through AccessKit ([D0003](../decisions/D0003-accesskit-for-accessibility.md)). Dovetail builds the accessibility tree from its components.

The rendering design must say:

- who owns the GPU device and each window's surface
- when frames are drawn (on demand, not in a constant loop, unless something is animating)
- how long GPU resources live
- how DPI (display scale) is handled
- how the framework recovers when the GPU device is lost
- how custom render passes fit into a frame

## Custom GPU content

Applications need a way to draw their own GPU content. A PCB canvas takes part in layout, clipping, focus and input like any other component, but does its own specialised drawing. See [M09](../milestones/overviews/M09-custom-gpu-canvas.md).

## Development iteration

- **First:** a reliable rebuild-and-restart loop ([M05](../milestones/overviews/M05-cli-and-rebuild-loop.md)).
- **Later, open:** live updates of declarative structure, styling and assets while the compiled Rust logic stays the same. This might use a runtime interpreter or a UI description that can be reloaded. With macro-based syntax, the design must explain how that description is produced and what it can and cannot express.

Any live-update design must deal with:

- keeping state across reloads
- invalid edits
- differences between development and release builds

## Repository layout

The project is one repository, organised as a Cargo workspace ([D0004](../decisions/D0004-cargo-workspace-monorepo.md)):

```text
dovetail/
  Cargo.toml
  crates/
    dovetail-ui/
    dovetail-core/
    dovetail-macros/
    dovetail-layout/
    dovetail-render/
    dovetail-desktop/
    dovetail-controls/
    dovetail-cli/
  examples/
    counter/
    document-editor/
    gpu-canvas/
  tools/
  docs/
```

Rules for the repository:

- Each crate can be used and published on its own.
- All crates share one version number and are released together while the API is still changing.
- Dependencies are managed at workspace level.
- CI runs on Windows, Linux and macOS.
- The order in which crates are published, and the compatibility policy, are written down before the first release ([R01](../milestones/overviews/R01-first-release.md)).

## Testing and performance measurement

- **Unit tests:** in every crate.
- **Macro tests:** compile-fail tests that pin the wording of error messages ([M03](../milestones/overviews/M03-declarative-macro.md)).
- **Layout and painting tests:** compare the layout tree, and rendered images where useful.
- **User interface tests:** `dovetail-testing` runs components with no window, finds elements by role and label, sends input through the window, and moves a test clock. It also compares pictures and drives real windows. See [testing](testing.md) (M20).
- **Platform checks:** a short manual checklist on each operating system for IME, accessibility and DPI, written in each milestone plan.
- **Performance:** fixed test scenes with set sizes, for example 1,000 elements in M04 and 10,000 canvas shapes in M09. Frame time and update time are recorded in completion reports.

## Main technical risks

| Risk | Why it is hard | Where it is handled |
|---|---|---|
| Text input and IME | Each OS works differently. Complex scripts need shaping. Building it ourselves is a lot of work. | [M06](../milestones/overviews/M06-text-and-input.md) |
| Accessibility | The tree must stay correct and in step with focus. Each OS needs manual testing. | [M07](../milestones/overviews/M07-accessibility.md) |
| Macro error messages | Errors must point at the user's declaration, not at generated code. | [M03](../milestones/overviews/M03-declarative-macro.md) |
| Live UI updates | Hard to combine with compiled macro syntax. | After the first release |
| Reactive ownership | The handle design affects the whole API and is hard to change later. | [M02](../milestones/overviews/M02-component-and-state-core.md) |
| Our own layout and painting | Large amount of work and a lot to test. | [M04](../milestones/overviews/M04-layout-and-painting.md) |
