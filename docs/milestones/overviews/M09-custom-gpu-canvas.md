# M09 — Custom GPU canvas

| Field | Value |
|---|---|
| ID | M09 |
| Status | In progress |
| Depends on | M06 |
| Plan | [M09 plan](../plans/M09-custom-gpu-canvas.md) |
| Report | — |

## Summary

Give applications a canvas component that takes part in layout, clipping, focus and input like any other component, and draws its own content with wgpu.

## Why it matters

A PCB view and similar specialised views need their own drawing. This is a key extension point of the framework.

## What it includes

- A canvas component that records its own wgpu render pass into the frame.
- Layout, clipping, focus and input for the canvas.
- Pan and zoom, with zoom centred on the mouse pointer.
- Selecting shapes with a click or by dragging a box.
- The `examples/gpu-canvas` example with a small PCB-like scene.

## What it leaves out

- Real PCB design features. Dovetail is the product, not the PCB tool ([D0001](../../decisions/D0001-framework-is-the-product.md)).

## Done when

1. The canvas takes part in layout, clipping, focus and input like any other component.
2. The canvas draws with its own wgpu render pass inside the normal frame.
3. Pan and zoom are smooth, and zoom is centred on the mouse pointer.
4. Shapes can be selected with a click or a dragged box, and the selection is shown.
5. The frame time for a scene with 10,000 shapes is measured and recorded.
