# M26 plan — Rich text

| Field | Value |
|---|---|
| ID | M26 |
| Status | Planned |
| Overview | [M26 overview](../overviews/M26-rich-text.md) |
| Last updated | 2026-11-27 |

## Goal

Add text with styles: a rich text view and editor, with bold, italic, sizes, colours, links, headings and lists, its own toolbar, and copy and paste with other applications.

## Starting point

- **Text** in `dovetail-text`, from M06 and M13:
  - `Font`, `Fonts`, shaping with `GSUB` and `GPOS`, kerning, fallback, and the bidirectional algorithm;
  - `Fonts::system_weight` finds a font for a weight, and thickens glyphs where no bold file exists. There is no italic: fonts are found by weight alone;
  - `TextBuffer` holds a plain `String` with a caret, a selection, and its own undo of typing. It does not use `History`.
- **Unicode tables** from `tools/ucd-gen` (D0014): grapheme clusters, bidirectional classes and joining types. There are no line break classes (UAX #14) yet.
- **Layout:**
  - `Content::Text` has one font size, one colour and one weight for the whole box;
  - text breaks into lines only at `\n`. Lines are never wrapped to a width;
  - `cursor_geometry` and `text_at` find the caret, the selection and the place under the pointer in such a box.
- **`TextField`** in `dovetail-controls` turns keys, the pointer and input methods into `TextBuffer` calls, on one line or several.
- **Clipboard:** `dovetail_core::Clipboard` has `text` and `set_text` only. Windows uses `CF_UNICODETEXT`, macOS `NSPasteboardTypeString`, and Linux the X11 `CLIPBOARD` selection, which reaches Wayland through XWayland.
- **Commands and undo** (M08): `Commands`, shortcuts, `History`, and `Document::edit`.
- **Accessibility:** `Role::Link` exists, used by the `Link` control. Text fields give AccessKit their value, caret and selection.
- **M25** gives toggle buttons, button groups, commands that are on or off, a toolbar that overflows into a menu, and `ColourButton`.
- Nothing opens a web address in the user's browser.

## Steps

1. [ ] **Settle the open questions** below, in a decision record (D0031).
2. [ ] **Unicode:** line break classes (UAX #14) from the UCD, made by `tools/ucd-gen` as D0014 says, with the conformance test `LineBreakTest.txt`.
3. [ ] **Wrapping:**
   - text wraps to the width it is given, at the places UAX #14 allows, and breaks inside a word only when a word is wider than the line;
   - in both directions, with each line put in order by the bidirectional algorithm after it is broken (rules L1 to L4);
   - plain text boxes and `TextField` can ask for it too. Text that does not ask keeps its present layout.
4. [ ] **Fonts:**
   - italic: find the system's italic and oblique files, and slant the glyphs where there are none, as bold does;
   - a family by name, with its weights and styles, so a run can ask for another font than the theme's.
5. [ ] **The text model**, in a new `dovetail-richtext` package, with the rules in `crates.md` updated:
   - a document of paragraphs, each with runs. A run has a font, a size, a weight, italic, underline, strikethrough, a colour, a background and a link;
   - paragraph styles: alignment, indents, spacing, headings of six levels, and lists with bullets or numbers, nested;
   - positions that stay on grapheme cluster boundaries, and ranges across paragraphs;
   - every change as a step that can be undone, so the model goes through `History` and `Document::edit`;
   - styles of a range: all bold, none, or mixed, for the toolbar.
6. [ ] **Rich layout:**
   - lines broken across runs of different fonts and sizes, with each line as tall as its largest run;
   - the caret, the selection and the place under the pointer across runs and lines, in both directions;
   - list marks and numbers, indents, and the space between paragraphs;
   - painting of underlines, strikethroughs and backgrounds.
7. [ ] **`RichTextView`:**
   - read-only, in a scroll view;
   - text can be selected with the pointer and the keyboard, and copied;
   - links can be followed by a click and by Enter, and are Tab stops. The application chooses what a link does; a web address opens in the user's browser.
8. [ ] **Platform:** opening a web address in the browser on Windows, Linux and macOS, in `crates/dovetail-desktop/src/platform/`, with a `// SAFETY:` comment on every `unsafe` block.
9. [ ] **`RichTextEditor`:**
   - typing, deleting, moving by character, word, line and paragraph, and selecting, as in `TextField`;
   - input methods across runs: composed text takes the style at the caret;
   - the style at the caret is used for what is typed next, and can be changed with no selection;
   - Enter starts a paragraph, and in a list a new item. Enter on an empty item, or Backspace at its start, ends the list;
   - Tab and Shift+Tab nest list items, and Ctrl+Tab leaves the editor;
   - typing runs, as one undo step each, as `TextBuffer` does now.
10. [ ] **Commands:** bold, italic, underline and strikethrough, with their usual shortcuts; size; colour; heading levels; bullets and numbers; alignment; and making and removing a link. Each is on or off for the selection, through M25's command state.
11. [ ] **Formatting toolbar:**
    - made from M25's toggle buttons, button groups and `ColourButton`;
    - follows the selection, and shows a mixed state where the selection is mixed;
    - overflows into a menu when narrow, as M25's toolbar does.
12. [ ] **Clipboard with styles:**
    - `Clipboard` gains formats besides text. Plain text always goes with styled text;
    - HTML in and out: Windows' `HTML Format`, macOS's `public.html`, and X11's `text/html`;
    - RTF in and out on Windows and macOS, where word processors use it;
    - on paste, styles the model has are kept, and the rest are dropped. A plain paste command pastes text alone.
13. [ ] **Reading and writing:**
    - Dovetail's own format, which keeps every style, with a version number;
    - Markdown in and out, for headings, bold, italic, strikethrough, links and lists;
    - HTML and RTF read only as much as step 12 needs. Each parser is Dovetail's own code ([D0002](../../decisions/D0002-build-own-stack.md)).
14. [ ] **Accessibility:** the editor and view as AccessKit text, with their runs, so assistive tools read the text, the caret, the selection, bold and italic, and the links.
15. [ ] **Theme:** text styles for the six headings, list marks, link colours, and the editor's look, from `theme()`, with `..._look` functions as in M15.
16. [ ] **Example:** a new `examples/rich-text` with a view of release notes and an editor with its toolbar, reading and writing Markdown. Its `--self-test` uses every style, with `TestWindows`. Add it to `examples/README.md` and the root README.
17. [ ] **Tests**, listed under Testing below.
18. [ ] **Checks by hand** on Windows, Linux and macOS, then the report.

## Prototypes and open decisions

- **Where wrapping lives:**
  - either layout learns to wrap any text box, which every control could then use, or rich text has a layout of its own;
  - a box that wraps has a height that depends on its width, so layout must measure it at the width it gets;
  - prototype wrapping in layout first, on plain text, before the model.
- **One editor or two:** whether `TextField` is built on the rich model with one style, or stays apart. Two editors would each need caret and input method fixes. Decide in D0031.
- **The model's shape:** a list of paragraphs with runs, or one string with style ranges over it. The first is simpler to edit; the second is simpler to search. Prototype both against a 100,000-word text.
- **Undo:** `TextBuffer` has its own undo. Rich text goes through `History`, so typing in it undoes with the rest of a document. Decide how runs of typing join into one step there.
- **Clipboard formats on Linux:** X11 offers many targets for one selection. Decide which are offered and read. Wayland without XWayland is still left out.
- **What a link does:** the application is given the link first. Decide what happens when it does nothing, and how a link that is not a web address is shown.
- **AccessKit's text runs:** how far its runs and their properties carry bold, italic and links on each system, with the pinned version (D0015). Check with each system's screen reader before the editor is built on it.
- **Large text:** whether layout is done again only for the paragraphs that changed. A release note is small; a long manual is not.

## Risks

- **Wrapping touches layout.** Every box with text could move. Keep wrapping to text that asks for it, and run every self-test.
- **Rich text is large.** It is at least as much work as all of M25. Wrap plain text first, then the model and the view, then the editor, then the clipboard and the formats. Each can be checked on its own.
- **HTML from browsers is messy.** Pasted HTML has styles in many shapes. Read only what the model has, test with copies from the main browsers on each system, and keep the plain text when reading fails.
- **Clipboard formats are platform code.** The Linux and macOS formats can only be type-checked here. They need CI, and a check by hand with a word processor and a browser on each system.
- **Input methods across runs** may show composed text in the wrong style or place. Test with the Windows Japanese input method, and on Linux and macOS by hand.
- **Assistive tools differ** in how much of the styles they read. Done when 6 needs a check by hand with Narrator, Orca and VoiceOver.

## Testing

- **Unit tests:**
  - all of `LineBreakTest.txt`;
  - wrapping: widths, long words, spaces at the ends of lines, and lines that mix directions;
  - the model: inserting and deleting across runs and paragraphs, styles of a range, and lists;
  - undo and redo of every kind of change, through `History`;
  - Markdown, HTML and RTF there and back, against files kept in the tests, with English and Arabic text.
- **Headless tests with `TestUi`:**
  - the editor and view with the pointer and with the keyboard alone, found by role and label;
  - every command and shortcut, and the toolbar following the selection, mixed state included;
  - typing after a style change with no selection;
  - lists: Enter, Backspace, Tab and Shift+Tab;
  - copy and paste through the clipboard inside the application.
- **AccessKit tree tests:** the text, its runs, bold and italic, the caret and selection, and links.
- **Self-tests:** `examples/rich-text`, in light and dark, in real windows. Run every example's self-test after the wrapping change.
- **By hand, on each system:**
  - copy from a web browser and paste into the editor, and copy from the editor into a word processor;
  - an input method typing into styled text;
  - links opening in the browser;
  - a screen reader reading the text, its styles and its links.

## Done when

Copied from the overview.

1. Text can be made bold, italic, underlined, larger and coloured, from the toolbar and with shortcuts, and every change can be undone.
2. Headings and lists with bullets or numbers can be made and taken away again.
3. Text with styles, in English and Arabic, is laid out and edited correctly, including with an input method.
4. Text with styles copied from a web browser keeps its styles when pasted, and text copied from the editor keeps them in a word processor, on Windows, Linux and macOS.
5. Text written to Markdown and read back has the same styles, for the styles Markdown has.
6. Assistive tools read the text, its links, and which parts are bold or italic.
