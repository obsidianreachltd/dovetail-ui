# M09 plan — Custom GPU canvas

| Field | Value |
|---|---|
| ID | M09 |
| Status | In progress |
| Overview | [M09 overview](../overviews/M09-custom-gpu-canvas.md) |
| Last updated | 2026-11-22 |

## Goal

Give applications a canvas component that takes part in layout, clipping, focus and input like any other component, and draws its own content with wgpu.

## Starting point

- **The painter** (`dovetail-render`) draws every box in one render pass, from a list of batches, into a persistent texture. It repaints only the damaged region.
- **Elements** already have layout, clipping, focus, keys, pointer events relative to the element, and layers above them (M08). A canvas is an element, so it gets all of these.
- **What is missing:**
  - a way for an element to draw with its own render pass, in its place in the frame;
  - the mouse wheel and the middle and right buttons for elements (today the wheel only scrolls, and only the left button reaches elements);
  - a pan and zoom camera, and box selection.
- **`dovetail-core` has no dependencies**, so it cannot name wgpu types. A canvas's drawing is attached to an element as an opaque value that only `dovetail-render` opens.
- **Decided on 2026-11-22:**
  - `dovetail-render` re-exports wgpu as `dovetail::render::wgpu`, so applications use exactly the version Dovetail pins;
  - the drawing hook goes in `dovetail-render`, and the camera and selection helpers in `dovetail-controls`, with no new crate.

## Steps

1. [x] Settle the open questions (above), on 2026-11-22.
2. [x] **Drawing**, done on 2026-11-22:
   - `dovetail-render` has `CanvasPainter`, `CanvasFrame` and `canvas()`. The frame gives the device, queue, encoder, target, format, the canvas's viewport and scissor, the scale and the GPU's generation. `CanvasFrame::begin` starts a pass that keeps what is drawn, with the viewport and scissor set.
   - `dovetail-core` holds a canvas's drawing as an opaque `CanvasSource` (`Element::canvas`).
   - The painter ends its render pass at a canvas, records the canvas's own pass, then starts again, so what comes after it is drawn over it.
   - A new source repaints the canvas, because layout counts it in the element's look.
   - wgpu is re-exported as `dovetail::render::wgpu`, and the facade exports `dovetail::render`.
3. [x] **Input**, done on 2026-11-22:
   - `Element::on_wheel` with `WheelEvent`, which says whether the movement came in notches. An element that takes the wheel gets it before any scrolling element (`Layout::wheel_target_at`).
   - `PointerEvent::button`. The middle and right buttons reach elements as pointer events, without clicking or moving focus.
   - For self-tests, `Context::pointer_button` and `Context::wheel_steps`.
4. [x] **Camera and selection**, done on 2026-11-22: `Camera` (`to_world`, `to_screen`, `zoom_at`, `pan`, `fit`, `wheel_factor`) and `SelectionBox` (`rect`, `is_click`, `touches`) in `dovetail-controls`, with 5 tests.
5. [x] **Accessibility**, done on 2026-11-22: `Role::Canvas`, which UI Automation reports as an image named by the canvas's label ("Board: 66 shapes, selected: none").
6. [x] **`examples/gpu-canvas`**, done on 2026-11-22:
   - **The scenes:** a small board with 66 shapes (pads, traces, vias), or a large one with exactly 10,000.
   - **Drawing:** its own wgpu pipeline (`shapes.wgsl`) draws each shape as an instanced quad shaded by its distance, with soft edges and a ring when selected. The shapes' buffer is rebuilt only when the scene or selection changes; panning and zooming only rewrite the camera.
   - **Using it:**
     - click, Shift+click and a dragged box select;
     - the middle or right button, a trackpad or the arrow keys pan;
     - the wheel or a pinch zooms about the pointer, and + and - zoom about the middle;
     - 0 fits the board, Ctrl+A selects everything, and Escape clears.
7. [x] **Tests**, done on 2026-11-22:
   - the camera and selection box (5);
   - finding the wheel's element (`dovetail-layout/tests/scroll.rs`);
   - a GPU image test (`dovetail-render/tests/canvas.rs`): a canvas's own pass draws only inside its 50 × 50 clip, under a box drawn after it, and a new painter repaints it while the rest stays;
   - `examples/gpu-canvas --self-test`. A box round a chip selects exactly the shapes it touches. A click selects Pad 9; after zooming three notches at it, a click at the same place still selects it; after a middle-button pan, a click where it moved to selects it. Shift+click adds a pad, and Ctrl+A selects everything.
8. [x] **Measurement**, done on 2026-11-22, with `cargo run --release -p example-gpu-canvas -- --bench` (see Testing).
## Prototypes and open decisions

- **How apps get wgpu:** decided, re-exported.
- **Where the helpers live:** decided, existing crates.
- **Pan and zoom input:**
  - a mouse wheel zooms about the pointer;
  - a trackpad's two-finger movement pans, and with Ctrl it zooms, which is how Windows sends a pinch;
  - the middle or right button drags to pan.

  This matches common drawing tools.

## Risks

- **Splitting the render pass** costs a little for each canvas. A frame usually has one or two.
- **Device loss:** a canvas's own pipelines belong to the old device. The canvas is told the GPU's generation, so it can make them again.
- **Partial repaints:** when only part of a canvas is damaged, the canvas is asked to draw everything, and the scissor keeps the rest of the canvas as it was.

## Testing

- **Unit tests** for the camera and the selection rectangle.
- **Tests without a window** for the wheel and buttons reaching the right element.
- **A GPU image test** for drawing, clipping, order and repainting.
- **`examples/gpu-canvas --self-test`:** zooms at a point and checks that it stays put, pans, selects with a click and with a box, selects everything and clears.
- **`--bench`:** frame time with 10,000 shapes while panning and zooming, headless.
- **By hand, on each system:** that pan and zoom with a mouse and a trackpad feel smooth. Not done yet.
- **Results on 2026-11-22**, Windows 11, NVIDIA TITAN RTX on Vulkan, a 1600 × 1000 canvas with 10,000 shapes, median of 15. A frame is building, layout, painting and waiting for the GPU:

  | Frame | Time |
  |---|---|
  | First frame (pipeline, buffer and everything) | 19.29 ms |
  | Zoom by one wheel notch (canvas redrawn) | 0.29 ms |
  | Select all or none (shapes' buffer rebuilt) | 1.26 ms |
  | Nothing changed | 0.01 ms |

- **Screenshots** of the small board, the 10,000-shape board and a full selection look right. They were taken through UI Automation, with one Ctrl+A sent after checking the window was in front.

## Done when

Copied from the overview.

1. The canvas takes part in layout, clipping, focus and input like any other component.
2. The canvas draws with its own wgpu render pass inside the normal frame.
3. Pan and zoom are smooth, and zoom is centred on the mouse pointer.
4. Shapes can be selected with a click or a dragged box, and the selection is shown.
5. The frame time for a scene with 10,000 shapes is measured and recorded.
