# M06 plan — Text and input

| Field | Value |
|---|---|
| ID | M06 |
| Status | In progress |
| Overview | [M06 overview](../overviews/M06-text-and-input.md) |
| Last updated | 2026-11-22 |

## Goal

Build full text handling in `dovetail-text`: real fonts, shaping, font fallback, editing, selection, IME and the clipboard. Also build keyboard focus and the routing of keyboard input.

## Starting point

- **M04** draws text with a built-in stroke font for ASCII ([D0012](../../decisions/D0012-m04-text.md)). Layout measures text at a fixed pitch.
- **M04** gives pointer clicks and hit testing. Keys reach only `Application::key_pressed`, and only as `Key::Character`, `Key::Escape` or `Key::Other`.
- **winit 0.30** provides IME on Windows, macOS, X11 and Wayland:
  - `Window::set_ime_allowed` and `set_ime_cursor_area`;
  - `Ime::Preedit` and `Ime::Commit` events.

  It provides **no clipboard**.
- [D0002](../../decisions/D0002-build-own-stack.md) allows only winit, wgpu and AccessKit, and the workspace denies `unsafe` code. A clipboard needs the platform's own interfaces, which are only reachable through `unsafe` calls or a library.
- This machine has these fonts: Segoe UI and Arial (Latin and Arabic), and Microsoft YaHei, Yu Gothic and Malgun Gothic (Chinese, Japanese and Korean).

**Size of the work.** This is the largest milestone so far and the biggest risk named in the architecture. Building our own stack means writing all of these ourselves:

| Part | What it needs |
|---|---|
| Font files | A reader for TrueType and OpenType files and collections: tables `cmap`, `hmtx`, `glyf`, `loca`, `head`, `hhea`, `maxp`, `OS/2` and `name`. `CFF` is also needed for many Linux CJK fonts. |
| Rasteriser | Quadratic and cubic outlines to coverage, with anti-aliasing, at any pixel size |
| Font fallback | Finding system fonts on each OS, and choosing a font for each character that the first font lacks |
| Shaping | Latin kerning (`GPOS` pair adjustment), Arabic joining forms (`GSUB` `init`, `medi`, `fina`, `isol`) and ligatures (`rlig`) |
| Bidirectional text | Ordering right-to-left and left-to-right runs on a line (Unicode bidirectional algorithm, a subset) |
| Unicode data | Tables for bidirectional classes, Arabic joining types, grapheme clusters and line breaks, made from the Unicode Character Database |
| Editing | A text model with grapheme-aware caret movement, selection, deleting, and undo of typing |
| IME | Preedit display, commit, and telling the IME where the caret is |
| Clipboard | Copy and paste of plain text, with code for each OS |
| Focus | A focus order, Tab and Shift+Tab, a visible focus ring, and routing keys to the focused component with fallback to parents |

## Steps

1. [x] Settle the open decisions below, and split the milestone if agreed. Decided on 2026-11-20:
   - complex scripts move to [M13](../overviews/M13-complex-scripts.md);
   - the clipboard uses our own code with scoped `unsafe` ([D0013](../../decisions/D0013-scoped-unsafe-for-platform-code.md));
   - the Unicode data comes from unicode.org ([D0014](../../decisions/D0014-unicode-data.md)).
2. [x] **Unicode data** ([D0014](../../decisions/D0014-unicode-data.md)): grapheme clusters done on 2026-11-22. `tools/ucd-gen` reads UCD 17.0.0 (downloaded into `tools/ucd/data`, which git ignores) and writes `dovetail-text/src/unicode/tables.rs` and the conformance test data. All 766 cases of `GraphemeBreakTest.txt` pass. Line breaks and the script of each character are not needed until text wraps and fallback considers language; they will be added by the same tool when they are. The bidirectional classes and joining types come in M13.
3. [x] **Font reading** in `dovetail-text`: tables, collections (`.ttc`), glyph outlines (`glyf`, then `CFF`), and metrics. TrueType done on 2026-11-20: `Font` reads `head`, `hhea`, `maxp`, `hmtx`, `cmap` (formats 4 and 12), `loca`, `glyf` (simple and composite) and `name`, from single fonts and collections, and fails cleanly on bad data.
   - **CFF** done on 2026-11-22 (`cff.rs`): OpenType fonts that start with `OTTO` now read. It reads the CFF INDEX and DICT structures, the Private DICT and its subroutines, and for CID-keyed fonts (such as the CJK fonts on Linux) the groups of glyphs (`FDArray`, and `FDSelect` formats 0 and 3). A Type 2 charstring interpreter draws lines and cubic curves (`Segment::Cubic`, which the rasteriser now splits to within 0.02 pixels). It runs local and global subroutines, skips the width, hints and hint masks, and handles the flex operators. It is limited to 10 levels of subroutines and 100,000 operators per glyph, and a glyph with bad data draws nothing.
   - **Checked:** tests with CFF tables built in code (an ordinary font and a CID-keyed one): exact segments, the same pixels as the TrueType test square, subroutines from the right group, damaged data without panics, and a clear refusal of `CFF2`. Two real Adobe fonts (SIL Open Font License) were read too: Source Sans 3 (ordinary CFF) and Source Han Sans JP (CID-keyed), from `https://raw.githubusercontent.com/adobe-fonts/source-sans/release/OTF/SourceSans3-Regular.otf` and `https://raw.githubusercontent.com/adobe-fonts/source-han-sans/release/SubsetOTF/JP/SourceHanSansJP-Regular.otf`. Put them in `tools/fonts/` (which git ignores) to run `tests/cff_fonts.rs`; without them those tests are skipped. Every glyph of both reads, the shapes of "o", "H" and 日 have the right strokes, and a snapshot drawn with only these two fonts looks right, kerning included.
   - **Not done:** `CFF2` (variable fonts) is refused with `FontError::UnsupportedOutlines`, and the CFF `FontMatrix` is not read: outlines are taken to be in the units of `head`, as they are in normal fonts.
4. [x] **Rasteriser:** done (`raster.rs`): signed-area accumulation, with curves split to within 0.02 pixels. Tests check exact coverage of squares, half pixels and curves. Its output feeds the glyph atlas, which replaces the D0012 stroke font.
5. [x] **System fonts and fallback:** done (`Fonts::system`, with `DOVETAIL_FONTS` to override). The default UI font is the platform's own: Segoe UI, San Francisco, or a common sans font on Linux. Fonts are read when first needed. On Windows the Latin and CJK fallback works (see the snapshot in the report). Fallback considers language since 2026-11-22 (`language.rs`). For Han characters and CJK punctuation, a font made for the text's language comes first, by the region at the end of its family name ("Noto Sans CJK JP") or else its `OS/2` code pages. The language comes from kana or Hangul in the text, then `DOVETAIL_LANG`, then the system's display language (Windows `GetUserDefaultUILanguage`, macOS `CFLocaleCopyPreferredLanguages`, `LC_ALL`, `LC_MESSAGES` or `LANG` elsewhere). Microsoft JhengHei and the regional faces of Noto Sans CJK were added to the font lists. Tested with the Windows CJK fonts: 直 comes from Yu Gothic, Microsoft JhengHei, Malgun Gothic or Microsoft YaHei by language, and kana make text Japanese. Text cannot yet set its own language; that waits for per-run languages in M13.
6. [x] **Shaping and measuring:** done on 2026-11-22. Advances and fallback come first, and layout measures with real fonts (`dovetail_render::font_measure`), which replaces `FixedPitch` when a font can be read. Text with line breaks is measured and drawn as several lines.
   - **Kerning** (`kern.rs`): `GPOS` pair adjustment for the `kern` feature, as lists of pairs (format 1) and as classes (format 2), also inside extension lookups. Fonts without it use the older `kern` table (format 0). Only the first glyph's x advance is used, which is how fonts kern Latin text. Kerning applies between glyphs of the same font, and `Fonts` caches each pair once looked up.
   - **Checked:** tests with fonts built in code for each format, and that broken tables do no harm; Segoe UI and Arial set "AV" closer than "A" and "V" apart; and a snapshot of "AVAWA Today Yes LT Wave" looks right.
   - **Measured:** the 1,000-element scene's first frame took 3.8 ms before the cache and 2.9 to 3.3 ms with it (2.8 ms before kerning). A text change takes 1.6 ms.
   - **Not done:** the script and language of the `kern` feature are not chosen (every `kern` lookup is used), and other `GPOS` features, such as mark positioning, wait for [M13](../overviews/M13-complex-scripts.md).
7. [ ] **Moved to M13:** Arabic joining and ligatures, and bidirectional ordering on a line. M06 keeps the text model ready for them: runs carry a direction, and caret positions come from the shaper.
8. [x] **Keyboard events:** done on 2026-11-22.
   - **Core:** `KeyEvent`, `KeyCode`, `NamedKey` and `Modifiers`, and elements with `focusable` and `on_key` (`KeyHandler`, which returns whether it took the key).
   - **Layout:** `focus_order`, `focus_next`, `focusable_at`, `dispatch_key` and `focus_ring`, with 6 tests that need no window.
   - **Desktop:**
     - winit key events become `KeyEvent`s, and modifiers are tracked.
     - Tab and Shift+Tab move focus; a click focuses without showing the ring.
     - Keys go to the focused element, then its parents; Enter and Space press the focused element.
     - Keys nobody takes go to `Application::key_pressed`.
   - **Painter:** a focus ring, painted with an image test.
   - **Controls:** `Button` and `Toggle` can take focus.
   - **Self-test:** the counter self-test now presses with Tab and Enter, and with Space.

   The step asked for full key events from winit (named keys, modifiers and text), and focus with Tab and Shift+Tab. Keys go to the focused component, and unhandled shortcuts go to its parents.
9. [x] **Text model and fields:** done on 2026-11-22.
   - `dovetail_text::TextBuffer` holds the text, caret and selection on grapheme boundaries. It moves by grapheme and by word, deletes, and undoes runs of typing in one step.
   - `Fonts::caret_x` and `Fonts::byte_at` turn caret positions into x positions and back. The layout `Measure` trait has the same two, so `FixedPitch` and the real fonts agree.
   - `Element` has `text_cursor` (caret and selection to draw), `on_pointer` (press, then moves and release while the button is held) and `on_focus`.
   - The desktop crate captures the pointer, counts double clicks, and finds the place in the text under the pointer (`Layout::text_at`). `Context::pointer` sends the pointer in self-tests.
   - The painter draws the selection over the element's background and under the text, and the caret over everything, at least one physical pixel wide.
   - `TextField` in `dovetail-controls`, on one line or several. Keys: typing, Backspace, Delete, the arrows, Home and End, with Shift to select and Ctrl to move or delete by word (Alt on macOS). Ctrl+A, Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z (Command on macOS). Enter adds a line only in a multi-line field. Pointer: click, Shift+click, drag, double click for a word and triple click for everything.
   - `examples/text-field` has a `--self-test` that types, selects by dragging, undoes and adds a line.
   - Follow-ups done on 2026-11-22:
     - **Scrolling:** `Style::scroll` makes an element scroll. Layout keeps its offset between updates, limits it to the content, and, while a caret shows, scrolls just enough to keep it in view when the caret or text changes. The mouse wheel scrolls the element under the pointer (`Layout::scroll_by`; 48 pixels a notch, Shift for sideways), and an outer element when the inner one is at its end. `TextField` scrolls, and its new `rows` property fixes a multi-line field's height. Tested in `dovetail-layout/tests/scroll.rs` and the field tests.
     - **Up and Down keep the column** across short lines; any other key or a click forgets it.
     - **Dragging after a double click selects whole words** (`TextBuffer::word_at` and `select_words`).
     - **The caret blinks**, at the Windows setting (`GetCaretBlinkTime`, which also allows no blinking) and 530 ms elsewhere. It shows again at once after any key, click or IME event, and is hidden while the window is in the background. The event loop sleeps until the next blink or the application's own timer. The `examples/text-field` self-test checks that it blinks; six screen captures 150 ms apart showed two alternating images.
   - Still to do: Up and Down in a multi-line field measure with the system fonts, so they assume the field's own `size` (16 by default) rather than one inherited from a parent. Dragging past the edge of a field scrolls only while the pointer moves.
10. [ ] **IME:** built on 2026-11-22; the checks with real input methods are still to do.
   - **Core:** `ImeEvent` (`Preedit` with the text and the input method's caret, and `Commit`), `Element::on_ime`, and `TextCursor::composing`, the byte range of text being composed.
   - **Desktop:** the window turns the input method on (`set_ime_allowed`) while the focused element has `on_ime`, and off otherwise. It tells the input method where the caret is (`set_ime_cursor_area`, from `Layout::caret_rect`) whenever the caret moves, so the candidate window sits by the text. winit's `Ime` events go to the focused element. `Context::ime` and `Context::ime_allowed` are there for self-tests.
   - **Painter:** text being composed is underlined in the text's colour.
   - **`TextField`:** shows the composed text at the caret without changing the value, shows the input method's caret or selection in it, and inserts committed text as one undo step. Composing replaces the selection; cancelling, or losing focus, leaves the text as it was.
   - **Checked on Windows (English keyboard only):** 3 field tests and the `examples/text-field` self-test send composing and committing as an input method would. Real key presses with the input method on still type exactly once (the clipboard check, which types "Copied from Dovetail"). A snapshot shows the underline.
   - **Still to do:** the manual checklist under Testing, with a real Japanese and Chinese input method on each OS. This machine has no such input method, and adding one changes system settings, so it is left for a person to run.
11. [x] **Clipboard:** done on 2026-11-22 for Windows; the macOS and X11 code is written and type-checked but not yet run.
   - **Core:** a `Clipboard` trait with `clipboard_text` and `set_clipboard_text`. Until the desktop crate installs the system clipboard, as in tests, it is a string inside the application (`MemoryClipboard`). Line breaks always read as `\n`.
   - **Platform modules** in `dovetail-desktop/src/platform/`, with our own declarations and no new dependencies ([D0013](../../decisions/D0013-scoped-unsafe-for-platform-code.md)):
     - Windows: `CF_UNICODETEXT` through `user32` and `kernel32`.
     - macOS: the general `NSPasteboard`, through the Objective-C runtime.
     - X11: `libX11` loaded with `dlopen`. A thread with its own connection and a hidden window owns the `CLIPBOARD` selection and answers other programs. Most Wayland desktops pass this on through XWayland. Without a display or libX11, the clipboard stays inside the application.
   - **`TextField`:** Ctrl+C, Ctrl+X and Ctrl+V (Command on macOS). Cut and paste each undo in one step. On one line, pasted line breaks become spaces.
   - **Checked on Windows:** an ignored test round-trips text with "✓", Chinese and a line break through the real clipboard; the `examples/text-field` self-test copies the notes and pastes them into the name; and real key presses (SendKeys) copied text out to PowerShell's `Get-Clipboard` and pasted text in from `Set-Clipboard`.
   - **Checked for macOS and Linux:** `cargo clippy -p dovetail-desktop --target aarch64-apple-darwin` and `--target x86_64-unknown-linux-gnu` pass. Nothing has run there yet.
   - **Still to do:** a native Wayland clipboard (`wl_data_device`), which needs winit's Wayland connection; the X11 `INCR` protocol for very large text; handing copied text to a clipboard manager when an X11 application exits; and Insert-key shortcuts (Ctrl+Insert, Shift+Insert, Shift+Delete).
12. [x] **Sharpness:** glyphs rasterised at the window's scale and placed on whole pixels. Checked by image tests at 100%, 150% and 200%. Done on 2026-11-22.
   - `crates/dovetail-render/tests/sharpness.rs` paints "Hamburg Sixty 日本" at 13 logical pixels, starting at an odd logical position, so at 150% the painter has to round half pixels. At each scale it compares every canvas pixel with the rasteriser's coverage at that scale, placed on whole pixels: every fully covered pixel must be solid ink, and every uncovered one pure background. Results: 31 of 31, 178 of 178 and 541 of 541 solid pixels, and no stray ink at all.
   - The ink on the canvas, measured in linear light, grows by 2.25 times at 150% and 4.0 times at 200%: glyphs are drawn at the display's size, not stretched.
   - Windows draw with the window's own scale factor (`PaintInput::scale` from winit), and snapshots at 100%, 150% and 200% look sharp.
   - **Text weight**, done on 2026-11-22. Small text looked lighter than the system's own text, because coverage was blended in linear light. Measured with "The quick brown fox jumps over the lazy dog" in Segoe UI at 14 pixels, black on white showed 72% of the ink Windows' GDI+ draws, and white on black 146%.
     - The painter now works out what each box is drawn over: the background of the nearest element below it. It passes that backdrop with each glyph. The glyph shader changes the coverage so the brightness comes out as blending in sRGB would give, for that text colour over that backdrop.
     - `crates/dovetail-render/tests/text_weight.rs` checks five pairs: black on white, white on black, white on the accent blue, dark grey on light grey, and blue on white. Each shows 100% of the glyphs' true coverage, with the text in an element whose background differs from the canvas.
     - For comparison, GDI+ draws the same text with 618 pixels of ink at its default contrast and 699 with contrast off. Dovetail now draws 672, between the two, the same for light and dark text. The sharpness test still matches exactly, since fully covered and empty pixels do not change.
     - Not handled: text over an image or a translucent layer, where the painter cannot know the backdrop; it uses the element backgrounds it knows.
13. [ ] **Tests and measurements:** shaping tests against known glyph sequences, image tests, focus and editing tests without a GPU, and manual IME checklists for each OS.

## Prototypes and open decisions

- **Splitting the milestone:** decided. Complex scripts are [M13](../overviews/M13-complex-scripts.md).
- **Clipboard and `unsafe`:** decided, our own code with scoped `unsafe` ([D0013](../../decisions/D0013-scoped-unsafe-for-platform-code.md)).
- **Unicode data:** decided, generated from the Unicode Character Database, downloaded from unicode.org ([D0014](../../decisions/D0014-unicode-data.md)).
- **CFF outlines:** done in step 3. They were checked with two downloaded Adobe fonts in place of a Linux machine; the Noto CJK fonts that Linux systems have are still to be tried there.
- **Rasteriser approach:** a signed-area accumulation rasteriser, which is exact and has no hinting. It is prototyped against known glyph shapes before the D0012 font is replaced.

## Risks

- **Size.** Even split, M06 is the largest milestone. Every step has its own tests, so progress is visible step by step.
- **Font quality.** A rasteriser without hinting can look soft at small sizes on Windows. Grey-scale anti-aliasing at the exact scale is the target. Sub-pixel rendering is out of scope.
- **IME differences** between systems. A manual checklist for each OS, and an X11 and Wayland run before M06 is done.
- **System font licences.** We read the user's installed fonts and never ship them, so their licences do not affect Dovetail.

## Testing

- Unit tests for each font table, the rasteriser (known glyphs to coverage), fallback choice, the text model and focus order.
- Image tests (GPU, ignored by default) for sharpness at three scales, and for a text field with a selection and a caret. The selection and caret test is done (`the_selection_is_under_the_text_and_the_caret_is_drawn`), and so is the sharpness test (`tests/sharpness.rs`).
- An IME checklist for each OS, run by hand with `cargo run -p example-text-field`: Japanese (Microsoft IME, Kotoeri, ibus or fcitx) and Chinese pinyin. For each input method:
  1. Click the name field and type "nihon" in Japanese mode, or "zhongwen" in pinyin. The letters or kana show underlined at the caret, and the greeting above does not change yet.
  2. The candidate window opens next to the caret, not in a corner of the screen or window.
  3. Convert with Space and commit with Enter. The chosen characters replace the underlined text once, with nothing doubled, and the greeting shows them.
  4. Ctrl+Z (Command+Z on macOS) removes the committed text in one step.
  5. Select some text, then compose: the selection goes when composing starts. Press Escape while composing: the text is as it was.
  6. Switch the input method to English or direct input and type: each letter appears once. On macOS, also check a dead key, such as Option+E then E for "é".
  7. Compose in the notes field, then click the name field part way through: the unfinished text does not stay behind.
- `examples/form` tries the controls together; its `--self-test` fills in, corrects, submits and clears the form. A doubled letter seen once while sending keys to it is [B0004](../../bugs/B0004-doubled-letter-when-typing.md).
- Tests that send real key presses must check, before every key, that the window in front is the one under test, and stop if it is not. Otherwise the keys go to another program.
- Real keyboard input on Windows, checked on 2026-11-22: Tab, Enter and Space were sent through `keybd_event` to `examples/counter`, and the window was captured from the screen. The focus ring showed, and the count was right in 5 of 5 runs, at 50 ms and 400 ms between keys. One earlier run showed one press too few; it could not be repeated, and was most likely the capture racing the last frame.

## Done when

Copied from the overview.

1. Latin text and Chinese, Japanese or Korean text are shaped correctly, with font fallback. Complex scripts such as Arabic moved to [M13](../overviews/M13-complex-scripts.md) on 2026-11-20.
2. Single-line and multi-line text fields support a caret, selection with mouse and keyboard, and editing.
3. IME composition works on Windows, Linux and macOS.
4. Copy, cut and paste work with the system clipboard.
5. Tab and Shift+Tab move focus in a predictable order, and the focused component is visible.
6. Keyboard events go to the focused component, and unhandled shortcuts pass to its parents.
7. Text is sharp at 100%, 150% and 200% display scale.
