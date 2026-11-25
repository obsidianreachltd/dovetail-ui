# D0024 — Images, icons and animation: our own decoders, SVG outlines in their colours, a texture for each image

| Field | Value |
|---|---|
| ID | D0024 |
| Status | Accepted |
| Date | 2026-11-24 |

## Context

[D0023](D0023-images-and-icons.md) settled how M17 decodes images, reads icons and keeps pictures on the GPU. On the same day, M17 took on its follow-ups:

- icons with more than one colour;
- animated images;
- icons in the macOS menu bar;
- a cache shared by the controls that show the same file.

D0023 gives an icon one colour and reads only PNG and JPEG, so it is replaced by this record. Everything in D0023 that is not changed below still holds, and is repeated here so that this record stands alone.

## Decision

- **Dovetail reads PNG, JPEG and GIF itself**, in the `dovetail-image` crate. It adds no dependency beyond `dovetail-core`, and nothing outside the workspace.
  - **PNG:** every colour type and bit depth, palettes, `tRNS` transparency, interlacing, and its own inflate (zlib). **APNG** animation is read too: its frames, delays, loop count, and blend and dispose operations.
  - **JPEG:** baseline, extended and progressive files with Huffman coding, restart markers, any chroma subsampling, greyscale, Adobe CMYK and YCCK, and EXIF orientation. Arithmetic coding and 12-bit samples are not read.
  - **GIF:** GIF87a and GIF89a, global and local palettes, transparency, interlacing, every disposal method, delays and the loop count.
- **An animation is a list of whole frames,** each a decoded image with its delay, and how many times it plays.
  - Frames are made complete when the file is decoded, off the UI thread.
  - The `Image` control shows each frame for its delay, and can be paused.
  - As in browsers, a delay under 20 ms counts as 100 ms.
- **Icons are read from a small subset of SVG.**
  - **Elements:** `svg` with `viewBox`, `g`, `path` (every command, arcs included), `rect`, `circle`, `ellipse`, `line`, `polyline` and `polygon`.
  - **Painting:** fills with either fill rule; strokes with butt, round or square ends and miter, round or bevel joins; opacity; and `transform`.
  - **Colour:**
    - An icon whose painted parts all have one colour, whatever it is, is drawn in the colour of the text around it, or the colour it is given.
    - An icon with two or more colours keeps them. Its parts painted `currentColor`, or not painted at all, take the text's colour.
    - Colours are read as `#rgb`, `#rrggbb`, with alpha, `rgb()` and `rgba()`, and the basic CSS names.
  - **Not read:** gradients, text, filters, masks, `use` and CSS beyond the `style` attribute.
  - **Dovetail's own icon set** is a set of such files, drawn for Dovetail.
- **Each image, and each animation frame, has a texture of its own, with mipmaps.**
  - The smaller copies are made when the image is decoded, off the UI thread.
  - Large images are uploaded a part at a time, over several frames.
  - Textures are released when their image is dropped, or least recently drawn first when they pass a memory budget.
  - One-colour icons are drawn at the size they are shown, for each display scale, into the glyph atlas, as glyphs are. Icons with several colours, and large icons, are drawn once at that size as images.
- **Pixels are kept premultiplied** in linear light and stored as sRGB. Filtering and blending are therefore correct at transparent edges.
- **Controls that show the same file share it.** A file or bytes being decoded are decoded once for every `Image` that asks. The result is kept only while one of them still shows it.
- **The macOS menu bar shows commands' icons.** Each is drawn at twice its size for sharp displays. A one-colour icon is a template image, so the system colours it as it colours the menu's text.

## Consequences

- No dependency is added, and decoding follows the rest of the stack. Decoding bugs are ours to find, so the decoders are tested against PngSuite, other encoders' files, and files written by encoders in the tests.
- Icons are ordinary `.svg` files, so they can be drawn in design tools, as long as they keep to the subset. Files from icon sets that paint in black still follow the theme, because they have one colour.
- Animations take memory for every frame. Very large ones are refused, as very large images are.
- Icons with one colour stay sharp at every scale, because they are drawn at the size shown. Icons with several colours are too, but each colour of text they appear in is another texture.
- Shrunk images are smooth, because of the mipmaps. Images take a third more memory for them.
- A picture is a leaf in the view tree, as text is. An icon is as tall as the text around it would make it, unless it is given a size.

## Alternatives considered

- **The `png`, `zune-jpeg` and `gif` crates:** less code and well tested, but dependencies outside D0002's foundations.
- **Icon outlines as path data in Rust, with fills only:** smaller, but icons drawn with lines would have to be redrawn as fills, and other tools' files could not be used.
- **Every icon in one colour, as D0023 had it:** simpler, but icons that need two colours, such as a red error badge on a grey shape, could not be shown.
- **Decoding animation frames as they are shown:** less memory, but decoding on the UI thread, or a task for every frame.
- **One shared colour atlas for images:** fewer texture switches, but more code, and shrunk images would look worse without mipmaps.
- **Baseline JPEG only:** less work, but many photos on the web are progressive and would not open.
