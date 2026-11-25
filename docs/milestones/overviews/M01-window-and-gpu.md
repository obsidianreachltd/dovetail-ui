# M01 — Window and GPU

| Field | Value |
|---|---|
| ID | M01 |
| Status | In progress |
| Depends on | M00 |
| Plan | [M01 plan](../plans/M01-window-and-gpu.md) |
| Report | — |

## Summary

Open windows with winit and draw into them with wgpu on all three operating systems, so these two foundations are proven before anything is built on them.

## Why it matters

Everything that is drawn later goes through this layer. Problems with DPI, resizing or the GPU are cheaper to find now.

## What it includes

- An event loop and window wrapper in `dovetail-desktop`.
- A GPU device, queue and window surfaces in `dovetail-render`.
- Deciding who owns the device and the surfaces.
- Handling resize, minimise, display scale changes and GPU device loss.
- Drawing only when needed.
- More than one window.
- An example that opens a window, `examples/window`.

## What it leaves out

- Layout and painting of components ([M04](M04-layout-and-painting.md)).
- Text ([M06](M06-text-and-input.md)).
- Custom render passes ([M09](M09-custom-gpu-canvas.md)).

## Done when

1. `examples/window` opens a window and clears it to a colour with wgpu on Windows, Linux and macOS.
2. Resizing the window redraws it at the new size with no stretching or crash.
3. Moving the window between displays with different scales gives the correct size in physical pixels.
4. Minimising and restoring the window works without errors.
5. After a simulated GPU device loss, the device and surfaces are recreated and drawing continues.
6. Two windows can be open at once, each with its own surface, sharing one GPU device.
