# M04 plan — Layout and painting

| Field | Value |
|---|---|
| ID | M04 |
| Status | In progress |
| Overview | [M04 overview](../overviews/M04-layout-and-painting.md) |
| Last updated | 2026-11-20 |

## Goal

Build our own layout engine and 2D painting, plus a first set of basic controls, so the Counter example runs in a real window.

## Starting point

- **M01** gives windows, one shared GPU device, surfaces, drawing on demand, and recovery after device loss. The only drawing is a clear to one colour.
- **M02** gives the component `Tree`. `Tree::update` reports which components changed, but every change gives `Invalidation::ALL`.
- **M03** gives `#[component]` and `ui!`.
  - `Column`, `Text` and `Button` exist with their properties, but nothing visual.
  - A `View` is `Empty`, `Text`, `Component` or `Fragment`. There is nothing for boxes, styles or layout yet.
- **Foundations** are winit, wgpu and AccessKit only ([D0002](../../decisions/D0002-build-own-stack.md)). Text drawing therefore needs a decision ([D0012](../../decisions/D0012-m04-text.md)).

## Steps

1. [x] Accept D0012, how M04 draws text, and decide the name of the text control. Both were decided on 2026-11-20: the built-in stroke font, and the name `Text`.
2. [x] **Elements in `dovetail-core`.** Done in `element.rs`. `Color` moved here from `dovetail-render`. Add `View::Element`: a box with a kind (`Row`, `Column`, `Stack` or `Box`), a `Style` and children.
   - A `Style` has padding, gap, alignment, minimum and maximum size, background, border, corner radius and clipping.
   - It is plain data, so `dovetail-core` keeps no dependencies.
3. [x] **Layout in `dovetail-layout`.** Done with `Layout::update`, `Measure`, `FixedPitch` and `Layout::hit_test`. Boxes are matched between updates by their owning component and their position in its view. Build a box tree from the component tree, then lay it out.
   - Rows and columns place children along one axis, with gap and alignment.
   - Stacks place children on top of each other.
   - Padding applies inside every box. Minimum and maximum sizes are clamped.
   - Text size comes from a measuring function.
   - Results are cached for each box. A change marks the box and its ancestors, and only marked boxes are laid out again.
4. [x] **Fine-grained invalidation.** Done, with a test in `dovetail-core`. Split `Invalidation` so that a change of colour gives paint only, and a change of size, text or structure gives layout and paint.
5. [x] **Painting in `dovetail-render`.** Done in `paint.rs` and `shaders.wgsl`: `Painter`, `Canvas`, `PaintStats` and `read_pixels`.
   - A WGSL pipeline draws instanced rectangles. The rounded corners and border come from a signed distance in the fragment shader.
   - Clipping uses a stack of scissor rectangles.
   - Each component's paint commands are kept between frames.
   - A frame repaints only the damaged region: the union of the old and new rectangles of changed boxes. It does this into a texture that persists between frames, then copies it to the surface.
6. [x] **Text drawing** as D0012 decides: single-line Latin labels, no editing, with a glyph atlas texture. Done in `font.rs`: 95 stroke glyphs and a box for missing ones. `examples/snapshot.rs` saves a PNG to look at.
7. [x] **Pointer input in `dovetail-desktop`.** Done: `CursorMoved`, `CursorLeft` and the left button. `Context::click` takes the same path, for self-tests.
   - Hit testing finds the deepest box with a press handler under the pointer.
   - Pressing and releasing on the same box calls its handler.
8. [x] **Windows run a component tree.** Add `dovetail::run(root)`, which does the following. Done with `Context::open_ui_window`, `dovetail::run(title, root)` and `dovetail_core::set_on_change`.
   - mounts the root in a window and lays it out and paints it;
   - turns state changes into a redraw request through a hook in `dovetail-core`, so the window wakes;
   - recreates GPU resources (pipelines, atlas) after a device loss ([D0009](../../decisions/D0009-replace-surfaces-on-device-loss.md)).
9. [x] **Controls in `dovetail-controls`.** Done, with 5 tests in `crates/dovetail-controls/tests/controls.rs` that need no window or GPU. They are also in `dovetail::prelude`.
   - `Button`, which calls `on_click` when pressed;
   - `Toggle`, with `on` and `on_change`;
   - the text control (see step 1);
   - `Column`, `Row` and `Stack`, built on elements.
10. [x] **`examples/counter`:** the Counter from the syntax document in a window. Add `--self-test`, which presses the button by sending pointer events and checks the count, for CI on all three operating systems. Done. On Windows on 2026-11-20, three clicks showed counts 1, 2 and 3, and it exited with code 0, on Vulkan and on DX12. CI runs it in the GPU job. Linux and macOS have not run it yet.
11. [x] **Tests.** The layout half is done: 13 tests in `crates/dovetail-layout/tests/layout.rs`. Changing one text in a list of 100 items lays out 3 of 201 boxes and reuses 198, and incremental layout equals a full layout.
    - layout tests with no GPU (done);
    - image tests that paint into a texture, read it back, and check chosen pixels (inside, border, corner, clipped area). Done: 5 tests in `crates/dovetail-render/tests/paint.rs`, which also cover text, a colour change repainting only its box, and a moved box leaving nothing behind. CI runs them in the GPU job.
    - a measurement of the boxes laid out and the pixels painted after one change. Done in `examples/scene_1000.rs`.
12. [x] **Measure frame time** for a scene of 1,000 elements: the first frame, and a frame after one change. Record the results in the report. On 2026-11-20, with an NVIDIA TITAN RTX on Vulkan, a 1600 × 1000 canvas and the median of 15 frames, including waiting for the GPU:
    - the first frame took 2.5 ms;
    - one colour change took 1.6 ms, laying out 0 boxes and painting 2,204 pixels;
    - one text change took 1.6 ms, laying out 4 boxes and painting 975 pixels;
    - a frame with nothing changed took 1.1 ms.

## Prototypes and open decisions

- **Text (D0012):** how labels are drawn without a font library (see D0012).
- **Text control name:** decided, `Text`. The overview's criterion 4 was changed from `Label` to `Text`.
- **Repaint into a texture that persists:** swapchain images do not keep their contents between frames, so partial repainting needs its own texture. That costs one copy each frame and memory the size of the window. The alternative is to repaint the whole window, which needs no extra texture but breaks criterion 3. This choice is recorded as a decision with the measurements.
- **Rounded clipping:** M04 clips to rectangles only. Rounded clipping, with a stencil or a mask, waits until a milestone needs it.

## Risks

- **Image tests need a GPU.** CI has none on Linux: install Mesa's software Vulkan (lavapipe), as the window self-test does. Windows CI uses WARP through DX12. Locally, any adapter works.
- **Incremental layout can go stale.** A test lays out every case both incrementally and from nothing, and compares the results.
- **Partial repaint can leave old pixels.** An image test checks that a moved box leaves nothing behind.
- **Device loss** now affects pipelines and the glyph atlas. The M01 self-test must still pass with the new resources.

## Testing

- `cargo test -p dovetail-layout`: layout cases, and incremental against full layout. These need no GPU.
- `cargo test -p dovetail-render -- --ignored`: image tests. They need a GPU adapter, and the CI job installs one.
- `cargo run -p example-counter -- --self-test`: on Windows, Linux (X11 and Wayland) and macOS.
- `cargo run --release -p dovetail-render --example scene_1000`: frame time measurements.

## Done when

Copied from the overview.

1. Rows, columns and stacks with padding, alignment, and minimum and maximum sizes lay out as shown in layout tests.
2. Rectangles, borders, rounded corners and clipping are drawn correctly, as checked by image tests.
3. A measurement shows that only changed regions are laid out and painted again.
4. Button, Toggle and Text work in `dovetail-controls`.
5. `examples/counter` runs on Windows, Linux and macOS, and clicking the button updates the count.
6. The frame time for a 1,000-element test scene is measured and recorded.
