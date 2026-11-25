# B0002 — wgpu panics after GPU device recovery (DX12)

| Field | Value |
|---|---|
| ID | B0002 |
| Status | Fixed |
| Severity | High |
| Milestone | M01 |
| Found | 2026-11-20 |
| Platform | Windows (DX12) |

## Summary

With the DX12 backend, the first frame after a simulated device loss and recovery stops the program with a panic inside wgpu.

## Steps to reproduce

1. `$env:WGPU_BACKEND = 'dx12'`
2. `cargo run -p example-window -- --self-test`

## Expected

Check 5 passes. The window draws again after the device is recreated.

## Actual

Checks 1, 2 and 4 pass. "GPU device recreated (generation 1)" is printed, then:

```text
thread 'main' panicked at wgpu-core-30.0.2\src\device\queue.rs:503:22:
called `Result::unwrap()` on an `Err` value: Lost
```

## Notes

- The machine was Windows 11, an NVIDIA TITAN RTX, wgpu 30.0.2 and winit 0.30.13.
- The panic is in wgpu-core's internal `PendingWrites::activate`, which unwraps a "device lost" error.
- A likely cause is that D3D12 shares one native device per adapter. When the old device is destroyed, the "new" device is broken too.
- GitHub's Windows CI runners have no Vulkan, so they use DX12. The window self-test therefore fails there until this is fixed.
- Possible next steps:
  - Ask for a different adapter, or make a new instance, on recovery.
  - Check the wgpu issue tracker.
  - Test a real device removal.
- Related to [B0001](B0001-vulkan-close-hangs-after-device-loss.md).

## Fix

This had the same cause as [B0001](B0001-vulkan-close-hangs-after-device-loss.md): old surfaces were reconfigured with the new device. On DX12 the reconfigured surface was left in a broken state, and the next frame used it, which made wgpu panic.

The B0001 fix also needed one more step for DX12. While the lost GPU's command queue exists, it keeps the old swapchains alive. A window can have only one swapchain, so creating the new surface failed with "Invalid surface". The lost GPU is now dropped after the old surfaces and before the new ones are made.

The suspected cause in the notes, one shared native device per adapter, was wrong.

Checked on 2026-11-20: `$env:WGPU_BACKEND = 'dx12'; cargo run -p example-window -- --self-test` passed all 6 checks and exited with code 0 three times in a row. CI runs the same self-test in the `window-self-test` job. The commit is not made yet.

