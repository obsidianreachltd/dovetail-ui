# D0002 — Build our own stack on a few foundations

| Field | Value |
|---|---|
| ID | D0002 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

Rust already has UI frameworks (for example Dioxus, Leptos, Xilem, Iced, Slint, Makepad, egui) and libraries for layout, text and 2D drawing. We had to decide how much to reuse and how much to build ourselves.

## Decision

Dovetail builds most of its stack itself. The only foundation libraries allowed are:

- **winit**, for windows, the event loop and input
- **wgpu**, for GPU access
- **AccessKit**, for platform accessibility ([D0003](D0003-accesskit-for-accessibility.md))

Layout, text, 2D painting, the component model and the macros are Dovetail's own code.

## Consequences

- Full control over the design, behaviour and public API.
- Text and IME, layout, and painting are large pieces of work and carry the most schedule risk ([M04](../milestones/overviews/M04-layout-and-painting.md), [M06](../milestones/overviews/M06-text-and-input.md)).
- Adding any other foundation library needs a new decision.
- The licences of these three libraries are checked in [M00](../milestones/overviews/M00-workspace-foundation.md).

## Alternatives considered

- **Reuse more foundations** (for example Taffy for layout, Parley or cosmic-text for text, Vello for drawing): faster, but gives less control.
- **Build on an existing framework** such as Dioxus or Xilem: fastest, but Dovetail would then not be its own framework.
