# D0009 — One shared GPU device, with surfaces replaced on device loss

| Field | Value |
|---|---|
| ID | D0009 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

[D0007](D0007-one-shared-gpu-device.md) chose one GPU device shared by all windows. For a lost device, it kept the instance and the surfaces and reconfigured each surface with the new device.

Testing in M01 showed that this recovery does not work:

- On Vulkan, a reconfigured surface keeps its old swapchain, and closing the window hangs ([B0001](../bugs/B0001-vulkan-close-hangs-after-device-loss.md)).
- On DX12, the first frame after recovery panics ([B0002](../bugs/B0002-dx12-panic-after-device-loss.md)).

## Decision

The application still has one `Gpu` in `dovetail-render`, shared by every window. Each window still has its own `WindowSurface`. The first window still chooses the adapter.

When the device is lost, the whole GPU and every surface are replaced, in this order:

1. Drop every window surface while the lost GPU still exists.
2. Drop the lost GPU.
3. Make a new GPU, with a new instance, for the first window, and new surfaces for the other windows.
4. Carry the generation number forward with `Gpu::replacing`.

Surfaces are always dropped before the `Gpu`, both in recovery and at shutdown.

## Consequences

- Recovery works on Vulkan and DX12 on Windows, and the program exits normally afterwards.
- Every GPU resource must be recreated after a loss, not only the surfaces. Later milestones that hold textures, buffers or pipelines must rebuild them when the generation changes.
- A window whose new surface cannot be made is closed.

## Alternatives considered

- **Reconfigure old surfaces with the new device (D0007):** fails on Vulkan and DX12, as described above.
- **Keep old surfaces and leak them:** fails, because a window can have only one swapchain.
