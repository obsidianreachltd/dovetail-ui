# M17 plan — Icons and images

| Field | Value |
|---|---|
| ID | M17 |
| Status | In progress |
| Overview | [M17 overview](../overviews/M17-icons-and-images.md) |
| Last updated | 2026-11-24 |

## Goal

Show pictures and icons: raster images from files and memory, and icons drawn from vector outlines in the theme's colours, sharp at every display scale.

## Starting point

- **Painting (M04, M06):** the painter draws rounded rectangles and glyphs from a one-channel atlas, with scissor clipping, into a canvas that keeps its pixels. Glyphs carry their backdrop for an even weight (M06).
- **Text (M06):** `dovetail-text` rasterises outlines by exact area, with no overlaps between contours.
- **Tasks (M11):** `spawn_task` runs work off the UI thread and reports back.
- **Theme (M14):** controls read their colours from `theme()`.
- **Controls (M15, M16)** draw their arrows and checks as text (`▼`, `✓`).
- **Commands (M08)** have a name and a shortcut, and no icon.
- **Accessibility (M07):** `Role::Image` exists and is mapped to AccessKit.
- **Decided on 2026-11-24** ([D0023](../../decisions/D0023-images-and-icons.md)):
  - our own PNG and JPEG decoders, JPEG baseline and progressive;
  - icons from a small SVG subset;
  - a texture with mipmaps for each image.
- **Widened on 2026-11-24**, when the first ten steps were done: the follow-ups came into this milestone, and [D0024](../../decisions/D0024-images-icons-and-animation.md) replaced D0023. The additions:
  - GIF and APNG animation;
  - icons with several colours;
  - icons in the macOS menu bar;
  - a decoded file shared by the `Image`s that show it.

  Criteria 7 and 8 were added to the overview.

## Steps

1. [x] Settle the open questions (above), on 2026-11-24.
2. [x] **`dovetail-image`, decoding**, done on 2026-11-24:
   - inflate (zlib), with its Adler-32 check;
   - PNG: every colour type and depth, palettes, `tRNS`, interlacing, and the chunk checks;
   - JPEG: baseline, extended and progressive, restarts, any subsampling (blended between samples), greyscale, RGB, CMYK and YCCK, EXIF orientation, and files cut short;
   - `Bitmap`: premultiplied pixels in linear light, stored as sRGB, with every smaller copy, made smaller when larger than 8192 pixels; `encode_png` for tests and tools.
3. [x] **`dovetail-image`, icons**, done on 2026-11-24:
   - an XML reader and the SVG subset, with groups, `style` and transforms;
   - paths flattened to lines, arcs included; strokes made into pieces wound one way;
   - a rasteriser that counts windings, with either fill rule, exact across, 16 samples down each pixel, so overlapping pieces cover once;
   - Dovetail's icon set: 33 icons on a 24-pixel grid (`icons::ALL`), and `examples/icon_sheet` to look at them.
4. [x] **Core and layout**, done on 2026-11-24:
   - `Picture`, a leaf in the view tree as text is (`View::Picture`), with a label for assistive tools or none for decoration;
   - its natural size, `size`, density (`Bitmap::picture_at`), `Fit` (contain, cover, stretch, none) and colour; `fit_rect` places it;
   - layout's `Content::Picture`, with the inherited text colour and font size.
5. [x] **The painter**, done on 2026-11-24:
   - an image pipeline with premultiplied blending and a smooth sampler with mipmaps, a texture for each image, uploaded at most 16 MB a paint;
   - textures released when their image is dropped, not drawn for 2,000 paints, or past a budget (256 MB, `set_image_budget`), least recently drawn first;
   - icons up to 256 pixels in the glyph atlas at the size shown; larger ones drawn once as images.
6. [x] **Desktop**, done on 2026-11-24:
   - pictures with a label become image nodes in the AccessKit tree, and name an icon-only button; decoration is skipped;
   - the window paints again where an image was still uploading, until it is done;
   - `Context::ui_pixels` reads a window's pixels, for self-tests.
7. [x] **Controls and commands**, done on 2026-11-24:
   - `Image`, which loads a file or bytes with a task, with a placeholder until it is ready, and an error icon and description when it fails;
   - `Icon` and `IconButton`;
   - icons in `Button`, `Tabs` and `Dialog`;
   - icons for arrows and checks in `CheckBox`, `Select`, `Collapsible`, `NumberField`, `DataTree` and `DataTable`;
   - `Commands::with_icon`, shown by `CommandButton` (with `icon_only` for toolbars), `MenuBar`, `ContextMenu` and `CommandSearch`.
8. [x] **`examples/images`**, done on 2026-11-24: a PNG and a JPEG at their size, fitted and filled; a 4096 × 4096 image made and loaded while the window keeps drawing; and icons in the menus, toolbar, a button, the tabs, a tree and a dialog, in light and dark. It has a `--self-test` of 14 steps.
9. [x] **Tests**, done on 2026-11-24:
   - decoders: unit tests (inflate, filters, the IDCT, Huffman codes, markers, orientation), PngSuite, Windows' encoders, every colour type, depth, filter and interlacing written by a small encoder, cut-short and changed files, randomly damaged files, and the Go source tree's files;
   - the SVG subset, paths, strokes, the rasteriser and the icon set;
   - core: sizes and fitting;
   - controls: `Image` loading and failing, icons' sizes and names;
   - painting on the GPU (8): images pixel for pixel at 100%, 150% and 200%, fitted and filled, smooth when shrunk; icons sharp in the text colour at each scale and in their own colour; large icons; uploads over several frames; textures released;
   - desktop: images and decoration in the AccessKit tree (1).
10. [x] Run everything on Windows, on 2026-11-24.
11. [x] **Icons with several colours** (D0024), done on 2026-11-24:
    - SVG colours are read: `#rgb` to `#rrggbbaa`, `rgb()`, `rgba()` and the basic names;
    - an icon with two or more colours keeps them (`Icon::is_multicolour`, `render_rgba`), and its `currentColor` parts, and parts with no paint, take the text's colour;
    - the painter draws such icons once at the size shown, in their colours, as images.
12. [x] **Animation** (D0024), done on 2026-11-24:
    - GIF: LZW, global and local palettes, transparency, interlacing, every disposal method, delays and the loop count;
    - APNG: `acTL`, `fcTL` and `fdAT`, with blend and dispose operations;
    - `Animation`: whole frames with their delays, decoded off the UI thread; a delay under 20 ms is 100 ms, as in browsers;
    - `Image` plays an animation, as many times as it says, with `playing` to pause it, and `ImageSource::Animation`;
    - `encode_gif` and `encode_apng`, which store without compressing, for tests and tools.
13. [x] **A shared image cache**, done on 2026-11-24:
    - `Image`s that show the same file (with the same length and change time) or the same bytes share one decoding, while it runs and afterwards;
    - the result is kept only while something shows it;
    - `image_decodes` counts decodings, for tests.
14. [x] **Icons in the macOS menu bar**, done on 2026-11-24:
    - `MenuItemSpec::Command` carries a `MenuIcon`: pixels drawn at 32 for 16 points;
    - a one-colour icon is a template, so the system colours it;
    - the macOS code makes it an `NSImage`. It is type-checked for macOS, but has not run on a Mac yet.

## Prototypes and open decisions

- **Decoders:** decided, our own (D0023, then D0024 with GIF).
- **Icon format:** decided, an SVG subset (D0023); several colours were added by D0024.
- **GPU storage:** decided, a texture for each image, with mipmaps (D0023), and for each animation frame (D0024).
- **JPEG:** decided, baseline and progressive (D0023).
- **One colour or several:** an icon whose painted parts all have one colour is drawn in the text's colour, whatever its colour is. Files from icon sets that paint in black therefore still follow the theme (D0024).
- **A picture is a leaf, as text is,** so layout can give an icon the inherited text colour and size, also inside layers such as menus. An icon is 8/7 of the font size tall by default: 16 pixels beside 14-pixel text.
- **Accessibility is on the picture:** a picture with a label is an image node, and one without is decoration. An icon beside text in a button is decoration by default; an icon-only button's icon has a label, which names the button.
- **Icons are not drawn with the glyph rasteriser,** as the overview first said. It adds areas, so the overlapping pieces of a stroke would be too dark where they meet. Icons have a rasteriser of their own that counts windings; glyph outlines do not overlap, so text keeps the exact one.
- **Found by the tests**, fixed on 2026-11-24:
  - damaged JPEG files could overflow the coefficients or a Huffman table; coefficients are now kept in 16 bits, as libjpeg keeps them, and tables are checked before they are filled;
  - a non-progressive file whose scan header asked for a refining pass had no table to read; the table is now read in every sequential scan.
- **Subsampled colour is blended between samples,** as libjpeg's default does. Go's decoder takes the nearest sample, so the comparison with its PNG files uses a hidden nearest mode (`decode_jpeg_nearest`), and then matches within 1 in 255 on average.
- **Follow-ups, done in this milestone (steps 11 to 14):**
  - native macOS menus with icons;
  - icons with more than one colour, and animated images;
  - an image cache shared between `Image` controls that show the same file.
- **Animation and less motion:** `Image` pauses when its `playing` is false. Dovetail cannot yet tell whether the system asks for less motion, so applications should offer a pause, as the example does. This is a follow-up for M07's accessibility work.
- **Follow-ups, after M17:**
  - reading the system's request for less motion, and pausing animations for it;
  - playing very long animations without decoding every frame first.

## Risks

- **Decoders are large and must not crash on bad files.** Every read is checked, and the tests include broken, cut-short and randomly damaged files: 24,000 damaged files decoded with no panic.
- **Uploading a very large image can take a long frame.** Images are uploaded at most 16 MB a frame.
- **JPEG decoders differ slightly in rounding.** Tests compare with a small tolerance against other decoders.

## Testing

- **Unit tests** for inflate, PNG filters and interlacing, the JPEG pieces, the SVG reader, path flattening, strokes, the rasteriser and the icon set.
- **Conformance tests:**
  - PngSuite's 15 basic images, committed (their licence allows it, see `NOTICE`): each decodes at its size;
  - files written by Windows' encoders, committed, made by `crates/dovetail-image/tests/fixtures/make.ps1`: the PNG exactly, the JPEG within 1.5 on average of Windows' own decoding;
  - PNG files in every colour type, depth, filter and interlacing, written by a small encoder in the test;
  - when they are in `tools/images/` (git-ignored), the test images from the Go source tree: PngSuite with its `.sng` descriptions, interlaced and broken PNG files, and JPEG files with their PNG decodings. Copy them from a Go installation: `src/image/testdata/*` and `src/image/png/testdata/*.png` into `tools/images/go/`, and `src/image/png/testdata/pngsuite/*` into `tools/images/pngsuite/`. Without them those tests are skipped.
- **Painting tests** on the GPU, `#[ignore]`d like the others: `cargo test -p dovetail-render --test images -- --ignored`.
- **Animation tests:** GIF and APNG files written by the tests' own encoders, with every disposal and blend, interlacing, partial frames, delays and loop counts, against frames put together by the test; Go's GIF files; 6,000 randomly damaged animations; and in controls, an `Image` playing, stopping after its plays, pausing and playing on.
- **Menu icons:** the model given to a stand-in for the system menu bar.
- **AccessKit tree tests** in the desktop.
- **`examples/images --self-test`** in a real window.
- **By hand, on each system:** that images and icons look right in light and dark.
- **Results on 2026-11-24**, Windows 11:
  - 362 workspace tests and 22 GPU tests pass, and clippy is clean, including for macOS and Linux;
  - `examples/images --self-test` passes all 17 steps, and every other example's self-test passes, twice each;
  - Go's GIF files: the grey one within 0.45 of its PNG on average, the colour one within 3.2 (it has fewer colours), and the interlaced one the same as the plain one;
  - Go's JPEG files match their PNG decodings within 0.42 on average (0.24 for CMYK, 0.80 for greyscale); progressive files decode exactly as their baseline twins;
  - the 4096 × 4096 image (64 MB, 85 MB with its smaller copies) is made, decoded and shown in 1.2 s in a release build, over 59 frames, with no frame longer than 20 ms (9.2 s and 129 ms in a debug build);
  - the real example, driven through UI Automation only, with no keys or mouse input: pictures in light and dark look right.

## Done when

Copied from the overview.

1. An image from a PNG and from a JPEG file shows at its size, fitted and filled, and is sharp at 100%, 150% and 200% display scale.
2. An icon follows the colour of the text around it and the theme, in light and dark, and is sharp at every scale.
3. Buttons, menu items, toolbar buttons, tabs and tree rows show a command's icon.
4. Loading a large image does not stop the window from drawing.
5. Every image and icon has a label for assistive tools or is marked as decoration, checked by tests on the AccessKit tree.
6. The example's self-test checks images and icons in each theme.
7. An animated GIF and APNG play in an `Image` at their frames' delays and loop counts, and stop when paused.
8. An icon with several colours keeps them, with its `currentColor` parts in the text's colour, in light and dark.
