# D0025 — Windows paint into a texture kept between frames

| Field | Value |
|---|---|
| ID | D0025 |
| Status | Accepted |
| Date | 2026-11-25 |

## Context

M04 criterion 3 asks that only the parts of a window that change are painted again. That needs the rest of the window's pixels to still be there from the last frame.

The images a window shows come from its swapchain, and they do not keep their contents. After an image is shown, its pixels are undefined, and the next frame may get a different image. Some systems can say how old an image's contents are ("buffer age"), but wgpu does not offer this. So a frame drawn straight into the swapchain must draw the whole window.

The choice was made in M04 on 2026-11-20, and the M04 plan said it would be recorded with its measurements. This record does that.

## Decision

Each window paints into its own texture, a `Canvas`, that keeps its pixels between frames. A frame repaints only the damaged region of the canvas, then copies the whole canvas to the window's swapchain image and shows it.

- **The damaged region** is the union of the old and new rectangles of the boxes that changed. Painting is clipped to it with a scissor rectangle, and the region is filled with the background first.
- **The copy** is one full-window triangle (`Painter::present`), not a texture copy. So the canvas can stay in one format while windows use whatever format their surface offers.
- **The canvas format** is `Rgba8UnormSrgb`, so blending happens on linear values. A surface without an sRGB format gets the colours encoded in the copy (`blit_encode_fs`).
- **A new canvas** is made when the window's size changes, and after a GPU device loss. A new canvas is painted in full.
- **Custom canvases** (M09) draw into the same texture, keeping what is there. They must draw everything inside the region each time they are asked.

## Consequences

- Small changes are cheap to paint. On Windows with an NVIDIA TITAN RTX, in a 1,000-element scene on a 1600 × 1000 canvas:
  - the first frame, painted in full, took 2.5 ms;
  - one colour change took 1.6 ms, and painted 2,204 pixels;
  - one text change took 1.6 ms, and painted 975 pixels;
  - a frame with nothing changed took 1.1 ms.
- **Every frame costs one copy** of the whole window, even when only a few pixels changed.
- **Each window uses extra GPU memory**: 4 bytes a pixel. That is 6.4 MB for a 1600 × 1000 window, and about 33 MB for a 3840 × 2160 one.
- **Damage must be right.** A missed rectangle leaves old pixels on screen. The image test `a_moved_box_leaves_nothing_behind` checks this, and so does a test that a colour change repaints only its box.
- **Tests can read the result.** `Painter::read_pixels` reads a canvas back, with no window, and the GPU image tests use this.
- Resizing a window repaints it in full, because the canvas is made again.

## Alternatives considered

- **Paint the whole window every frame.** Simplest, with no extra texture and no copy. But every frame costs as much as the first, and M04 criterion 3 could not be met.
- **Paint into the swapchain image, keeping what is there.** The image's contents are undefined after it is shown, so old frames' pixels may be wrong or missing.
- **Track damage per swapchain image, using its age.** This is how some toolkits avoid the copy. wgpu does not say how old an image is, and adding it would mean platform code for each system's graphics API.
