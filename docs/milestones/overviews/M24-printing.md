# M24 — Printing

| Field | Value |
|---|---|
| ID | M24 |
| Status | Not started |
| Depends on | R01 |
| Plan | — |
| Report | — |

## Summary

Let applications print and save as PDF: pages laid out and drawn by the application, the system's print dialog, and a preview.

## Why it matters

- **What applications need:** document applications print. A PCB tool prints drawings and assembly sheets, and saves them as PDF to send.
- **What exists:** the painter draws to the screen only. Printers and PDF files need sharp vector output, not pictures of the screen.
- **Why after the first release:** none of the ten acceptance points in [R01](R01-first-release.md) needs it, and it needs a second way of drawing.

## What it includes

- **Pages:** the application says how many pages there are and draws each one, with the page size, margins and orientation.
- **Drawing pages** with the same shapes, text and images as the painter, as vectors: text stays text, and shapes stay sharp at the printer's resolution.
- **PDF output** written by Dovetail, with fonts embedded, which also serves as the way to print on systems that print PDFs.
- **The system's print dialog**, and page setup, on each system.
- **A print preview** control.
- **An example** that prints a few pages, with a `--self-test` that saves them as PDF and checks the file.

## What it leaves out

- Reading or editing PDF files.
- Printing the screen as it is, without the application laying out pages.
- Printer-specific features beyond what the system's dialog offers.

## Done when

1. An application's pages save as a PDF that opens in common readers, with text that can be selected and searched.
2. Printing through the system's dialog prints the same pages on Windows, Linux and macOS.
3. Text and shapes are drawn as vectors, sharp at the printer's resolution.
4. The print preview shows the pages as they will print.
5. The example's self-test saves a PDF and checks its pages, text and fonts.
