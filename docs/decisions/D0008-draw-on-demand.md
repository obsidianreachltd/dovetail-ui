# D0008 — Draw only when needed

| Field | Value |
|---|---|
| ID | D0008 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

A window can be drawn in a constant loop, or only when something has changed. Desktop applications are idle most of the time.

## Decision

Windows are drawn only on request. The event loop waits for events (winit `ControlFlow::Wait`). A frame is drawn when:

- a window opens
- a window is resized or changes display scale
- the GPU device has been recovered
- the application calls `Context::request_redraw`

Frames are shown with `PresentMode::AutoVsync`, so they are in step with the display. A minimised window, with zero size, is not drawn.

Continuous drawing, for example during an animation, will be added as an explicit request when a milestone needs it.

## Consequences

- An idle application uses almost no CPU or GPU, which matters on laptops.
- Every state change must ask for a redraw. M02 makes this automatic for reactive state.

## Alternatives considered

- **Constant loop:** simpler, but it wastes power and battery when nothing changes.
