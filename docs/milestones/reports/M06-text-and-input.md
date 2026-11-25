# M06 report — Text and input

| Field | Value |
|---|---|
| ID | M06 |
| Finished on | Not finished: criterion 3 waits for the manual IME checks |
| Commit or tag | Not committed yet |
| Plan | [M06 plan](../plans/M06-text-and-input.md) |

## Summary

Dovetail now has its own text stack:

- it reads TrueType and OpenType CFF fonts, including CID-keyed CJK fonts;
- it draws glyphs with its own rasteriser, with font fallback and kerning;
- it has text fields with a caret, selection, editing, undo, the clipboard and input methods (IME);
- it has keyboard focus, and keys go to the focused element first.

Six of the seven criteria are met, checked on Windows. Criterion 3, IME on every system, is built and tested with simulated input-method events. It still needs the manual checks with real Japanese and Chinese input methods, so the milestone stays In progress. Nothing has been run on Linux or macOS yet.

## What was delivered

- **`dovetail-text`**, with no new dependencies ([D0002](../../decisions/D0002-build-own-stack.md)):
  - **Font reading** (`font.rs`, `read.rs`): TrueType fonts and collections (`head`, `hhea`, `maxp`, `hmtx`, `cmap` formats 4 and 12, `loca`, `glyf` simple and composite, `name`). Every read is bounds-checked, and bad data fails cleanly.
  - **CFF outlines** (`cff.rs`): OpenType fonts that start with `OTTO`, including CID-keyed fonts with groups of glyphs (`FDArray`, and `FDSelect` formats 0 and 3). A Type 2 charstring interpreter draws lines and cubic curves, runs subroutines, skips hints and handles flex. `CFF2` (variable fonts) is refused with a clear error.
  - **Rasteriser** (`raster.rs`): signed-area accumulation, exact for lines, with quadratic and cubic curves split to within 0.02 pixels. There is no hinting.
  - **Fonts and fallback** (`fonts.rs`): `Fonts::system()` with a list for each OS, read when first needed, and `DOVETAIL_FONTS` to override it. Shaping places glyphs with fallback, and `caret_x` and `byte_at` map between caret positions and pixels.
  - **Language** (`language.rs`): Han characters and CJK punctuation use a font made for the text's language. The language comes from kana or Hangul in the text, `DOVETAIL_LANG`, or the system's display language.
  - **Kerning** (`kern.rs`): `GPOS` pair adjustment for the `kern` feature (pair lists, classes and extension lookups), or else the older `kern` table. Each pair is cached once looked up.
  - **Unicode** (`unicode/`): grapheme clusters (UAX #29) from tables that `tools/ucd-gen` makes from UCD 17.0.0 ([D0014](../../decisions/D0014-unicode-data.md)).
  - **`TextBuffer`** (`edit.rs`): text, caret and selection on grapheme boundaries. It moves by grapheme and by word, deletes, and undoes runs of typing in one step.
- **`dovetail-core`:**
  - `KeyEvent`, `KeyCode`, `NamedKey`, `Modifiers` and `KeyHandler`.
  - `PointerEvent` and `PointerHandler`.
  - `ImeEvent`.
  - The `Clipboard` trait, with `clipboard_text`, `set_clipboard_text` and `MemoryClipboard`.
  - `Element` gained `focusable`, `on_key`, `on_pointer`, `on_focus`, `on_ime` and `text_cursor` (`TextCursor`: caret, selection and composing range).
- **`dovetail-layout`:**
  - Scrolling for elements with `Style::scroll`: offsets kept between updates, the mouse wheel (`Layout::scroll_by`), and keeping a shown caret in view.
  - Focus: Tab order, focus by click, and `dispatch_key`, which tries the focused element, then its parents, then Enter or Space to press it.
  - Measuring for several lines of text.
  - `Measure::caret_x`, `byte_at` and `line_height`, and `Layout::text_at` and `caret_rect`.
  - `cursor_geometry`, for the selection, caret and composing underline.
- **`dovetail-render`:**
  - Real fonts in the painter and in measuring (`font_measure`). The D0012 stroke font is used only when no font can be read.
  - Several lines of text, and a focus ring.
  - The selection under the text, and the caret and composing underline over it.
  - Text weight that is the same on light and dark backgrounds: each glyph carries its backdrop to the shader.
- **`dovetail-desktop`:**
  - Key events from winit, with modifiers. Tab and Shift+Tab move focus.
  - Pointer capture with double-click counting, and focus events.
  - Turning the input method on and off, and reporting where the caret is.
  - The system clipboard in `platform/` modules: Windows, macOS and X11 ([D0013](../../decisions/D0013-scoped-unsafe-for-platform-code.md)).
  - `Context::key`, `pointer`, `ime` and `ime_allowed` for self-tests.
- **`dovetail-controls`:** `TextField`, on one line or several, with scrolling, a fixed number of `rows`, Up and Down that keep the column, and dragging by word. `Button` and `Toggle` take focus.
- **The caret blinks**, at the system's rate on Windows, and is hidden while the window is in the background.
- **`examples/text-field`** has a `--self-test` that types, selects by dragging, undoes, adds a line, copies, pastes and composes with an input method. The counter self-test now also presses with Tab and Enter, and with Space.
- **`examples/form`**: a contact form with labelled text fields, a toggle, Submit and Clear buttons, and checks on what is typed. Its `--self-test` fills it in with the keyboard, fixes the email after a click, submits and clears it.
- **`tools/ucd-gen`** generates the Unicode tables and the conformance test data.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | Latin and CJK text shaped correctly, with font fallback | Met | Font tests with fonts built in code, and with Segoe UI and the Windows CJK fonts. CFF tests with tables built in code, and with Source Sans 3 and Source Han Sans JP. Kerning tests. Snapshots of Latin, CJK and kerning pairs. Checked on Windows. |
| 2 | Single-line and multi-line text fields with a caret, mouse and keyboard selection, and editing | Met | 12 `TextField` tests (`dovetail-controls/tests/text_field.rs`), the GPU test for the selection and caret, and the `examples/text-field` self-test. Checked on Windows. |
| 3 | IME composition works on Windows, Linux and macOS | Not met | Built and tested with simulated input-method events: 3 field tests and the self-test. Real typing with the input method on types each letter once on Windows. The manual checklist in the plan, with real Japanese and Chinese input methods on each OS, has not been run. |
| 4 | Copy, cut and paste work with the system clipboard | Met | On Windows: an ignored test round-trips text through the real clipboard, the self-test copies and pastes, and real key presses copied text to PowerShell and pasted text from it. The macOS and X11 code passes `cargo clippy` for those targets but has not run. |
| 5 | Tab and Shift+Tab move focus in a predictable order, and the focused component is visible | Met | 6 focus tests (`dovetail-layout/tests/focus.rs`), the GPU test for the focus ring, and the counter self-test. Real key presses on Windows were captured from the screen with the ring showing. |
| 6 | Keyboard events go to the focused component, and unhandled shortcuts pass to its parents | Met | The `dispatch_key` tests, and the field tests, where Enter in a single-line field goes on to its parents. Real Tab, Enter and Space on Windows. |
| 7 | Text is sharp at 100%, 150% and 200% display scale | Met | `dovetail-render/tests/sharpness.rs`: at each scale, every fully covered pixel is solid ink and no ink falls outside the glyphs, and the ink grows by 2.25× and 4.0×. Checked on Windows. |

## Changes from the plan

- **Complex scripts moved to [M13](../overviews/M13-complex-scripts.md)** on 2026-11-20: Arabic joining, ligatures and bidirectional text (step 7).
- **Clipboard:** our own code with `unsafe` only in `dovetail-desktop/src/platform/`, by [D0013](../../decisions/D0013-scoped-unsafe-for-platform-code.md), not a library.
- **Linux clipboard:** X11 only, which most Wayland desktops pass on through XWayland. A native Wayland clipboard needs winit's Wayland connection, which winit does not share.
- **Unicode data:** generated from the UCD by [D0014](../../decisions/D0014-unicode-data.md). Only grapheme clusters were needed. Line-break data and the script of each character wait until text wraps and fallback considers language.
- **Added to the plan:**
  - Text weight: the glyph shader corrects for blending in linear light.
  - Text measured and drawn as several lines.
  - The clipboard as a trait in `dovetail-core`, so controls need not depend on the desktop crate.
- **CFF was checked without a Linux machine**, using two downloaded open-licence Adobe fonts in `tools/fonts/`, which git ignores.
- **Renames and API changes:**
  - The keyboard key type is `KeyCode`, because `Key` was already the list key.
  - `Painter::paint` takes a `PaintInput`, because the argument list grew too long.
- **AltGr:** on Windows, AltGr arrives as Ctrl+Alt. It now types characters such as "@" instead of being treated as a shortcut.
- **Step 13** (tests and measurements) is done except the manual IME checklists.

## Known issues

- [B0004](../../bugs/B0004-doubled-letter-when-typing.md): once, a typed letter appeared twice in a text field. Closed as cannot be reproduced, after 40 correct runs, 32 of them with input logging.

Other limits:

- **Text fields:**
  - Up and Down measure with the system fonts at the field's own `size`, not a size inherited from a parent.
  - Dragging past the edge of a field scrolls only while the pointer moves.
- **Language:** text cannot set its own language yet; the system's, `DOVETAIL_LANG` or kana and Hangul in the text decide.
- **Kerning** uses every `kern` lookup in the font, without choosing by script or language.
- **CFF:** `CFF2` is refused, and the CFF `FontMatrix` is not read.
- **Clipboard:**
  - No native Wayland support.
  - No X11 `INCR` protocol for very large text.
  - On X11, copied text is not handed to a clipboard manager when the application exits.
  - No Insert-key shortcuts.
- **Text weight:** the backdrop is only known from element backgrounds, so text over an image or a translucent layer may come out lighter or heavier.

## Measurements

On Windows 11 with an NVIDIA TITAN RTX (Vulkan), at 100% unless stated.

| What | Platform | Result |
|---|---|---|
| Grapheme break conformance (UCD 17.0.0) | Windows | 766 of 766 cases |
| 1,000-element scene, first frame (release) | Windows | 2.9–3.3 ms (2.8 ms before kerning; 3.8 ms with kerning but no cache) |
| Caret blink, six captures 150 ms apart | Windows | Two alternating images, at the system's 530 ms |
| 1,000-element scene, one cell's text changed | Windows | 1.6–1.8 ms |
| Sharp pixels, 13 px text: fully covered pixels that are solid at 100%, 150% and 200% | Windows | 31/31, 178/178, 541/541, with no stray ink |
| Ink growth with scale (150% and 200% against 100%) | Windows | 2.25× and 4.0× |
| Text weight, 14 px Segoe UI, black on white, before the fix (GDI+: 618) | Windows | 447 pixels of ink (72%) |
| Same, white on black, before the fix (GDI+: 618) | Windows | 905 (146%) |
| Same, after the fix, for five colour pairs | Windows | 100% of the true coverage (672–674), against GDI+'s 618 at its default contrast and 699 with contrast off |
| CFF glyphs read: Source Sans 3 and Source Han Sans JP | Windows | 2,478 and 17,944, every one, in 0.85 s for both in a debug build |
| Real key presses through to the counter (Tab, Enter, Space) | Windows | Right in 5 of 5 runs |
| Workspace tests | Windows | 163 passed, plus 14 ignored (GPU, clipboard and similar) that pass when run |

## Follow-up

- **To finish M06:** run the IME checklist in the plan with Japanese and Chinese input methods on Windows, macOS, X11 and Wayland. Then mark criterion 3 met, set this report's date, and change the status to Done in the overview, the plan and the matrix.
- **Linux and macOS runs**, once CI or those machines are available:
  - the macOS and X11 clipboard code;
  - the Noto CJK fonts;
  - the text-field self-test.
- **Clipboard:** a native Wayland clipboard, X11 `INCR`, handing text to a clipboard manager, and Insert-key shortcuts.
- **Text fields:** scrolling while the pointer is held still past the edge, and measuring Up and Down with the inherited font size.
- **Text:**
  - a language for each piece of text, with the per-run work in [M13](../overviews/M13-complex-scripts.md);
  - choosing `kern` lookups by script;
  - line-break data, for wrapping;
  - `CFF2` and the CFF `FontMatrix`.
- **[M13](../overviews/M13-complex-scripts.md):** `GPOS` mark positioning, Arabic and bidirectional text.
- **[M07](../overviews/M07-accessibility.md):** text fields and focus in the accessibility tree.
- **[M08](../overviews/M08-commands-and-undo.md):** text editing undo joined to the shared undo system, and clipboard commands in menus.
