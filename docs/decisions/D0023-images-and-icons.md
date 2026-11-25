# D0023 — Images and icons: our own decoders, SVG outlines, a texture for each image

| Field | Value |
|---|---|
| ID | D0023 |
| Status | Replaced by [D0024](D0024-images-icons-and-animation.md) |
| Date | 2026-11-24 |

## Context

M17 shows pictures and icons. That raises four questions:

- **Decoding:** reading PNG and JPEG files needs decoders. Under [D0002](D0002-build-own-stack.md), Dovetail either writes them or records a new dependency.
- **Icon format:** icons need an outline format.
- **GPU storage:** the painter needs a way to keep pictures on the GPU.
- **JPEG kinds:** we had to choose which kinds of JPEG to read.

## Decision

- **Dovetail reads PNG and JPEG itself**, in a new `dovetail-image` crate. It adds no dependency beyond `dovetail-core`, and nothing outside the workspace.
  - **PNG:** every colour type and bit depth, palettes and `tRNS` transparency, interlacing, and its own inflate (zlib).
  - **JPEG:** baseline, extended and progressive files with Huffman coding, restart markers, any chroma subsampling, greyscale, Adobe CMYK and YCCK, and EXIF orientation. Arithmetic coding and 12-bit samples are not read.
- **Icons are read from a small subset of SVG.**
  - **Elements:** `svg` with `viewBox`, `g`, `path` (every command, arcs included), `rect`, `circle`, `ellipse`, `line`, `polyline` and `polygon`.
  - **Painting:** fills with either fill rule; strokes with butt, round or square ends and miter, round or bevel joins; opacity; and `transform`.
  - **Colour:** an icon has one colour, the colour of the text around it or one it is given. Opacity can make parts lighter.
  - **Not read:** gradients, text, filters, masks, `use` and CSS beyond the `style` attribute.
  - **Dovetail's own icon set** is a set of such files, drawn for Dovetail.
- **Each image has a texture of its own, with mipmaps.**
  - The smaller copies are made when the image is decoded, off the UI thread.
  - Large images are uploaded a part at a time, over several frames.
  - Textures are released when their image is dropped, or least recently drawn first when they pass a memory budget.
  - Icons are drawn at the size they are shown, for each display scale, into the glyph atlas, and drawn as glyphs are.
- **Pixels are kept premultiplied** in linear light and stored as sRGB. Filtering and blending are therefore correct at transparent edges.

## Consequences

- No dependency is added, and decoding follows the rest of the stack. Decoding bugs are ours to find, so the decoders are tested against PngSuite and against JPEG files made by other encoders.
- Icons are ordinary `.svg` files, so they can be drawn in design tools, as long as they keep to the subset.
- Icons stay sharp at every scale, because they are drawn at the size shown.
- Shrunk images are smooth, because of the mipmaps. Images take a third more memory for them.
- A picture is a leaf in the view tree, as text is. An icon is as tall as the text around it would make it, unless it is given a size.

## Alternatives considered

- **The `png` and `zune-jpeg` crates:** less code and well tested, but two dependencies outside D0002's foundations.
- **Icon outlines as path data in Rust, with fills only:** smaller, but icons drawn with lines would have to be redrawn as fills, and other tools' files could not be used.
- **One shared colour atlas for images:** fewer texture switches, but more code, and shrunk images would look worse without mipmaps.
- **Baseline JPEG only:** less work, but many photos on the web are progressive and would not open.
