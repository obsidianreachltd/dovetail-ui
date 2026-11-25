# M17 — Icons and images

| Field | Value |
|---|---|
| ID | M17 |
| Status | In progress |
| Depends on | M14 |
| Plan | [Plan](../plans/M17-icons-and-images.md) |
| Report | — |

## Summary

Show pictures and icons: raster images from files and memory, and icons drawn from vector outlines in the theme's colours, sharp at every display scale.

## Why it matters

- **What is missing:** Dovetail cannot show a picture or an icon yet. Toolbars, menus, tree views and tabs look bare without them, and applications need logos, previews and thumbnails.
- **The first release:** the validation editor in [R01](R01-first-release.md) needs icons for its tools and layers.
- **Dependencies:** reading image files needs decoders. Under [D0002](../../decisions/D0002-build-own-stack.md), writing our own or taking a dependency needs a decision first, made in this milestone's plan.

## What it includes

- **An `Image` control:**
  - pictures from a file or from memory, with their size kept, fitted, or filled;
  - loading off the UI thread with M11's tasks, with a placeholder until it is ready;
  - one decoded copy shared by every `Image` that shows the same file;
  - animated images played at their frames' delays, and paused when asked.
- **Image formats:** PNG and JPEG at least, and GIF and APNG with their animation, decoded by code chosen in a decision record.
- **Icons:**
  - drawn from vector outlines at the size shown, so they are sharp at every scale;
  - in the colour of the text around them, or a theme colour;
  - icons with several colours keep them, with their `currentColor` parts in the text's colour.
- **An icon set:** a small set of Dovetail's own icons for the commands every application has: new, open, save, undo, redo, cut, copy, paste, search, close, and arrows and checks for controls. They are drawn for Dovetail and licensed with it.
- **Icons in controls:**
  - buttons, menu items (in the macOS menu bar too), toolbar buttons, tabs, tree rows and dialogs;
  - commands get an optional icon, shown wherever the command is.
- **The painter:**
  - images in the GPU atlas or textures of their own, scaled with good filtering;
  - kept and released by how recently they were drawn.
- **Accessibility:** images and icons have a label, or are marked as decoration so assistive tools skip them.
- **An example** with images, icons in every control that takes them, in light and dark, with a `--self-test`.

## What it leaves out

- A full SVG renderer: only simple outlines and fills are read for icons.
- Editing images.
- Icon fonts and system icon sets.

## Done when

1. An image from a PNG and from a JPEG file shows at its size, fitted and filled, and is sharp at 100%, 150% and 200% display scale.
2. An icon follows the colour of the text around it and the theme, in light and dark, and is sharp at every scale.
3. Buttons, menu items, toolbar buttons, tabs and tree rows show a command's icon.
4. Loading a large image does not stop the window from drawing.
5. Every image and icon has a label for assistive tools or is marked as decoration, checked by tests on the AccessKit tree.
6. The example's self-test checks images and icons in each theme.
7. An animated GIF and APNG play in an `Image` at their frames' delays and loop counts, and stop when paused.
8. An icon with several colours keeps them, with its `currentColor` parts in the text's colour, in light and dark.
