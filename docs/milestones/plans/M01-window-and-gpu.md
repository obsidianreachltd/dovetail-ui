# M01 plan — Window and GPU

| Field | Value |
|---|---|
| ID | M01 |
| Status | In progress |
| Overview | [M01 overview](../overviews/M01-window-and-gpu.md) |
| Last updated | 2026-11-25 |

## Goal

Open windows with winit and draw into them with wgpu on all three operating systems, so these two foundations are proven before anything is built on them.

## Starting point

- M00 is complete: the workspace, empty crates and CI on three operating systems are in place.
- No window or rendering code exists yet.

## Steps

1. [x] Add winit and wgpu as workspace dependencies, used only by `dovetail-desktop` and `dovetail-render`. Pinned to winit `=0.30.13`, the latest stable (0.31 is still in beta), and wgpu `=30.0.2`. `rust-version` was raised to 1.87 for wgpu.
2. [x] In `dovetail-desktop`, wrap the winit event loop and application handler behind our own types: `run`, `Application`, `Context`, `Frame`, `WindowId`, `WindowOptions`, `Key`, `LogicalSize` and `WindowInfo`.
3. [x] In `dovetail-render`, create one GPU device and queue for the application, and one surface for each window: `Gpu` and `WindowSurface` ([D0007](../../decisions/D0007-one-shared-gpu-device.md)). A small `block_on` replaces an async runtime dependency.
4. [x] Draw only when a redraw is requested, for example after input, a resize or a state change ([D0008](../../decisions/D0008-draw-on-demand.md)).
5. [x] Handle resizing, and skip drawing while the window is minimised, when its size is zero.
6. [x] Handle changes of display scale, and keep sizes in both logical and physical pixels.
7. [x] Detect when the GPU device is lost, then recreate the device and surfaces and draw again. It is simulated with `device.destroy()` behind the `device-loss-testing` feature.
8. [x] Support more than one window that shares one device.
9. [x] Write `examples/window`, which opens a window and clears it to a colour. Keys: `N` opens a new window, `C` changes the colour, `L` simulates device loss, and `Esc` closes the window.
10. [ ] Write a manual checklist for each operating system, and run it on Windows, Linux (X11 and Wayland) and macOS. The checklist is written below. Items 1, 2 and 4–7 are automated as `--self-test`, which passed in CI on Windows, Linux (X11, under Xvfb) and macOS on 2026-11-25. Still to run by hand: item 3 on every system, and the whole list under Wayland. On Windows it found [B0001](../../bugs/B0001-vulkan-close-hangs-after-device-loss.md) and [B0002](../../bugs/B0002-dx12-panic-after-device-loss.md).
11. [x] Fix B0001 and B0002, so device recovery works and the program exits on every backend. Recovery now replaces the surfaces as well as the device ([D0009](../../decisions/D0009-replace-surfaces-on-device-loss.md), which replaces D0007).

## Prototypes and open decisions

- **Ownership of the device and surfaces**: decided, one shared device ([D0007](../../decisions/D0007-one-shared-gpu-device.md)). Its recovery steps were replaced after B0001 and B0002 ([D0009](../../decisions/D0009-replace-surfaces-on-device-loss.md)).
- **When to draw**: decided, on demand ([D0008](../../decisions/D0008-draw-on-demand.md)).
- **Default wgpu backends**: wgpu's defaults, which can be overridden with the `WGPU_BACKEND` environment variable. Windows chose Vulkan on an NVIDIA TITAN RTX. The macOS CI runner chose Metal on an "Apple Paravirtual device". The Linux CI runner has only software Vulkan (lavapipe) installed; record the name it prints in the report.

## Risks

- **winit changes its API between versions.** Keep it behind our own wrapper and pin the version.
- **Linux behaves differently under X11 and Wayland.** Test both.
- **GPU device loss is hard to cause on purpose.** Simulate it by destroying the device in a test build. This risk happened: recovery from the simulated loss hung on Vulkan (B0001) and panicked on DX12 (B0002). Both are fixed. A real driver reset has not been tested yet.

## Testing

- CI builds and runs unit tests on Windows, Linux and macOS: size conversion, colour conversion, `block_on`, and the empty-size check.
- `cargo test -p dovetail-render --features device-loss-testing -- --ignored` destroys and recovers a headless device. It passed on Windows on 2026-11-20. CI runs it in the `window-self-test` job on all three systems, with software Vulkan on Linux.

- `cargo run -p example-window -- --self-test` runs checklist items 1, 2 and 4–7 without anyone at the keyboard, and exits with a failure code if any fails. CI runs it on all three systems in the `window-self-test` job. It first passed on Linux and macOS on 2026-11-25, and since then the job blocks on all three systems.

Self-test results on Windows (2026-11-20, NVIDIA TITAN RTX), after the fixes for B0001 and B0002, with three runs per backend:

| Backend | 1 | 2 | 4 | 5 | 6 | 7 | Exit |
|---|---|---|---|---|---|---|---|
| Vulkan | Pass | Pass | Pass | Pass | Pass | Pass | Code 0, 3 of 3 runs |
| DX12 | Pass | Pass | Pass | Pass | Pass | Pass | Code 0, 3 of 3 runs |

Self-test results in CI on 2026-11-25:

| System | 1 | 2 | 4 | 5 | 6 | 7 | Notes |
|---|---|---|---|---|---|---|---|
| Windows | Pass | Pass | Pass | Pass | Pass | Pass | |
| Linux | Pass | Pass | Pass | Pass | Pass | Pass | X11 under Xvfb, software Vulkan |
| macOS | Pass | Pass | Pass | Pass | Pass | Pass | Metal, Apple Paravirtual device, 800 × 600 at scale 1 |

On macOS, the restore step first timed out. A frame asked for straight after restoring was skipped, because the window still counted as hidden, and nothing asked again. Windows now draw again when they become visible, and the self-test asks for a frame every half second after restoring.

Manual checklist. Run `cargo run -p example-window` on each operating system, and on Linux under both X11 and Wayland:

1. A window opens and fills with blue. The GPU name and backend are printed. (Windows: passed on 2026-11-20, Vulkan.)
2. Dragging the window edges redraws it at the new size, with no stretching, flicker or crash. The printed physical size changes.
3. Moving the window to a display with a different scale prints the new scale, and the physical size changes to match.
4. Minimising and restoring the window prints no errors, and it is blue again.
5. Pressing `L` prints "GPU device lost" and then "GPU device recreated (generation 1)". The window keeps drawing, and `C` still changes its colour.
6. Pressing `N` opens a second window with a different colour. Both resize and change colour on their own.
7. `Esc` in one window closes only that window. Closing the last window ends the program.

## Done when

Copied from the overview.

1. `examples/window` opens a window and clears it to a colour with wgpu on Windows, Linux and macOS.
2. Resizing the window redraws it at the new size with no stretching or crash.
3. Moving the window between displays with different scales gives the correct size in physical pixels.
4. Minimising and restoring the window works without errors.
5. After a simulated GPU device loss, the device and surfaces are recreated and drawing continues.
6. Two windows can be open at once, each with its own surface, sharing one GPU device.
