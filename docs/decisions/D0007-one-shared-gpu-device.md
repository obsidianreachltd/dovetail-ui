# D0007 — One GPU device shared by all windows

| Field | Value |
|---|---|
| ID | D0007 |
| Status | Replaced by [D0009](D0009-replace-surfaces-on-device-loss.md) |
| Date | 2026-11-20 |

## Context

An application can have several windows. Each window needs a surface to draw into. The GPU device and queue could be shared by all windows, or each window could have its own.

## Decision

The application has one `Gpu` in `dovetail-render`. It holds the wgpu instance, the adapter, one device and one queue. Each window has its own `WindowSurface`, made from the shared instance and set up with the shared device.

- The first window chooses the adapter, so the adapter can draw into it.
- If the device is lost, `Gpu::recover` replaces the adapter, device and queue. The instance and the surfaces stay, and every surface is set up again with `WindowSurface::reconfigure`.
- Surfaces are dropped before the `Gpu`.

## Consequences

- GPU resources such as textures and fonts can be shared between windows.
- Recovery happens in one place for the whole application.
- When the device is lost, every window is affected at once.

## Alternatives considered

- **One device per window:** a lost device affects only one window, but resources cannot be shared and memory use is higher.
