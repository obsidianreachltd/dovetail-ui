# M04 report — Layout and painting

| Field | Value |
|---|---|
| ID | M04 |
| Finished on | 2026-11-25 |
| Commit or tag | `13fd236`, merged into `main` from the `CI_Fixes` branch |
| Plan | [M04 plan](../plans/M04-layout-and-painting.md) |

## Summary

Dovetail has its own layout engine and GPU painting, with Button, Toggle and Text controls, and the Counter example runs in a real window. Only changed boxes are laid out and painted again. All six criteria are met. The last one, the counter running on all three systems, was met when its self-test passed in CI on Windows, Linux and macOS on 2026-11-25.

## What was delivered

- **`dovetail-core`:** elements (`Row`, `Column`, `Stack` and `Box`) with a `Style` of plain data, and an `Invalidation` split so a colour change repaints without laying out again.
- **`dovetail-layout`:** `Layout::update`, `Measure`, `FixedPitch` and `Layout::hit_test`. Each box's result is cached, and a change marks only the box and its ancestors.
- **`dovetail-render`:** `Painter`, `Canvas`, `PaintStats` and `read_pixels`. A WGSL pipeline draws instanced rectangles, with rounded corners and borders from a signed distance, and clips with scissor rectangles. Only the damaged region is repainted, into a texture kept between frames ([D0025](../../decisions/D0025-repaint-into-a-kept-texture.md)).
- **Text:** single-line labels drawn with the built-in stroke font ([D0012](../../decisions/D0012-m04-text.md)). M06 later replaced it with real fonts.
- **`dovetail-desktop`:** pointer input, hit testing, and windows that run a component tree with `dovetail::run(title, root)`. GPU resources are recreated after a device loss.
- **`dovetail-controls`:** `Button`, `Toggle`, `Text`, `Column`, `Row` and `Stack`.
- **`examples/counter`**, with `--self-test`, which clicks the button through the window and checks the count.
- **Tests and tools:** 13 layout tests, the painting image tests, `examples/scene_1000.rs` for frame times, and `examples/snapshot.rs`, which saves a picture of the painter's output.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | Rows, columns and stacks lay out as layout tests show | Met | 13 tests in `crates/dovetail-layout/tests/layout.rs` |
| 2 | Rectangles, borders, rounded corners and clipping are drawn correctly | Met | Image tests in `crates/dovetail-render/tests/paint.rs`, passing in CI on all three systems |
| 3 | Only changed regions are laid out and painted again | Met | Changing one text in a list of 100 items lays out 3 of 201 boxes. A colour change lays out none, and repaints only its box (image test) |
| 4 | Button, Toggle and Text work | Met | 5 tests in `crates/dovetail-controls/tests/controls.rs` |
| 5 | `examples/counter` runs on all three systems and the button updates the count | Met | `--self-test` on Windows (Vulkan and DX12, 2026-11-20), and in CI on Windows, Linux (X11 under Xvfb) and macOS on 2026-11-25 |
| 6 | Frame time for a 1,000-element scene measured and recorded | Met | Below |

## Changes from the plan

- **Text:** the stroke font was a stop-gap. M06 replaced it with font files, shaping and a glyph atlas, and the image tests now cover real text.
- **The decision to repaint into a kept texture** was recorded on 2026-11-25, as [D0025](../../decisions/D0025-repaint-into-a-kept-texture.md), not during the milestone.
- **The painting tests grew** beyond the 5 planned, to cover text, the focus ring, the selection and caret, and scroll bars, as later milestones added them.

## Known issues

- [B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md): the GPU tests sometimes crashed on the Linux CI runner. They now run one at a time there.

## Measurements

Measured on 2026-11-20 with `cargo run --release -p dovetail-render --example scene_1000`: an NVIDIA TITAN RTX on Vulkan, a 1600 × 1000 canvas, and the median of 15 frames, including waiting for the GPU.

| What | Platform | Result |
|---|---|---|
| First frame | Windows | 2.5 ms |
| One colour change | Windows | 1.6 ms, 0 boxes laid out, 2,204 pixels painted |
| One text change | Windows | 1.6 ms, 4 boxes laid out, 975 pixels painted |
| Nothing changed | Windows | 1.1 ms |

## Follow-up

- **Wayland:** the counter has run on Linux only under X11. Run it under Wayland by hand.
- **Rounded clipping** waits until a milestone needs it.
- **Measure frame times on Linux and macOS** on real GPUs.
