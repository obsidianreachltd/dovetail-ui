# B0001 — Closing a window hangs after a GPU device loss (Vulkan)

| Field | Value |
|---|---|
| ID | B0001 |
| Status | Fixed |
| Severity | High |
| Milestone | M01 |
| Found | 2026-11-20 |
| Platform | Windows (Vulkan) |

## Summary

After a simulated GPU device loss and recovery, closing a window that was open before the loss never finishes. The program hangs and has to be killed.

## Steps to reproduce

1. `cargo run -p example-window -- --self-test` (Vulkan is the default backend on this machine).
2. Wait for "6 of 6 automatic checks passed".

## Expected

The last window closes and the program exits with code 0.

## Actual

All six checks pass, then the program stops responding. Tracing shows the hang is in dropping that window's `wgpu::Surface`, the first window's surface. The second window was created after recovery, and it closes normally.

## Notes

- The machine was Windows 11, an NVIDIA TITAN RTX, wgpu 30.0.2 and winit 0.30.13.
- The loss is simulated with `wgpu::Device::destroy()`. A real driver reset may behave differently.
- The surface was configured on the destroyed device and then reconfigured on the new one. Dropping it appears to wait on the old device forever.
- One attempt was to create new surfaces and a new instance on recovery and leak the old ones. That fails because a window can have only one swapchain, so the new surface cannot be configured while the old one exists.
- Possible next steps:
  - Check the wgpu issue tracker for surface drop after `Device::destroy`.
  - Try unconfiguring or dropping the surface before the device is destroyed, in the simulation only.
  - Test a real device removal, for example a driver restart, to see whether only the simulation is affected.
- Related to [B0002](B0002-dx12-panic-after-device-loss.md).

## Fix

The cause was found in wgpu-core 30.0.2. When a surface is configured with a new device, wgpu-core drops its record of the old device without unconfiguring the surface (`device/resource.rs`, `configure_surface`). The Vulkan backend then keeps the old swapchain, which belongs to the destroyed device. Dropping the surface later waits on it forever.

Recovery no longer reconfigures old surfaces. `Shell::recover_gpu` in `dovetail-desktop` now:

1. drops every window surface while the lost GPU still exists, so each one unconfigures on its own device;
2. drops the lost GPU (needed for [B0002](B0002-dx12-panic-after-device-loss.md));
3. makes a new GPU and new surfaces for the same windows;
4. calls `Gpu::replacing` to carry the generation number forward.

The steps are documented on `Gpu::replacing` in `dovetail-render`.

Checked on 2026-11-20: `cargo run -p example-window -- --self-test` passed all 6 checks and exited with code 0 three times in a row on Vulkan. CI runs the same self-test in the `window-self-test` job. The commit is not made yet.

