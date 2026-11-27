# M26 — Rich text

| Field | Value |
|---|---|
| ID | M26 |
| Status | Planned |
| Depends on | M08, M13, M15, M25 |
| Plan | [M26 plan](../plans/M26-rich-text.md) |
| Report | — |

## Summary

Add text with styles: a rich text view and editor, with bold, italic, sizes, colours, links, headings and lists, its own toolbar, and copy and paste with other applications.

## Why it matters

- **What exists:** plain text fields on one line or several, with selection, undo, input methods and text in both directions, from [M06](M06-text-and-input.md) and [M13](M13-complex-scripts.md).
- **What is missing:**
  - Help text, notes, comments and messages need styles within a paragraph.
  - Applications show formatted text, such as release notes, in a read-only view.
  - Text copied from a web browser or a word processor loses its styles.
- **Why on its own:** a rich text editor is at least as large as all of M25. It needs its own text model, and its own way to lay out and edit text in styles.

## What it includes

- **A text model:**
  - paragraphs of runs. Each run has a font, a size, bold, italic, underline, strikethrough, a colour, a background and a link;
  - paragraphs with alignment, indents, spacing, headings, and lists with bullets or numbers;
  - every change made with `edit`, so it can be undone with `History`.
- **Layout:** lines broken across runs of different fonts and sizes, in both directions, with input methods working across styles.
- **A rich text view:** read-only, with links that open and text that can be selected and copied.
- **A rich text editor:**
  - the styles of the text the caret is in, used for what is typed next;
  - shortcuts for bold, italic and underline, and commands for every style;
  - a formatting toolbar made from M25's toggle buttons and button groups, that shows the styles of the selection.
- **Copy and paste:** plain text always, and styled text from and to other applications in each system's formats: HTML, and RTF where it is used.
- **Reading and writing:** a format of Dovetail's own, and Markdown in and out for the styles it has.
- **Accessibility:** assistive tools read the text, its styles and its links, as AccessKit allows.
- **An example** with a view and an editor, with a `--self-test`.

## What it leaves out

- Tables, pictures and other objects inside the text.
- Pages, columns, headers and footers: a word processor's layout.
- Spelling checks.
- Several people editing the same text at once.
- Printing: [M24](M24-printing.md).

## Done when

1. Text can be made bold, italic, underlined, larger and coloured, from the toolbar and with shortcuts, and every change can be undone.
2. Headings and lists with bullets or numbers can be made and taken away again.
3. Text with styles, in English and Arabic, is laid out and edited correctly, including with an input method.
4. Text with styles copied from a web browser keeps its styles when pasted, and text copied from the editor keeps them in a word processor, on Windows, Linux and macOS.
5. Text written to Markdown and read back has the same styles, for the styles Markdown has.
6. Assistive tools read the text, its links, and which parts are bold or italic.
