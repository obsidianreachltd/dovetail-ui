# M21 — Files from the system

| Field | Value |
|---|---|
| ID | M21 |
| Status | Not started |
| Depends on | M11, M19 |
| Plan | — |
| Report | — |

## Summary

Let files reach an application from outside it: dragged onto a window, given on the command line, opened from the system's file manager, and listed as recent files.

## Why it matters

- **What users expect:** dropping a file on a window opens it, double-clicking a document in the file manager opens it in its application, and the File menu lists recent files.
- **What exists:** [M11](M11-documents.md) opens files through the file dialogs only. Drag and drop works inside and between Dovetail's own windows, but not with files from the system.
- **One instance:** opening a second document from the file manager should open it in the running application, not start another copy.
- **The first release:** the validation editor in [R01](R01-first-release.md) is a document application, and should open its files in all these ways.

## What it includes

- **Files dropped from the system:** onto any element with a drop handler, with the files' paths, and a hover state while they are dragged over.
- **Dragging files out** from the application to the file manager and other applications, where the system allows it.
- **Files on the command line**, read by the application at start.
- **Opening files from the system:**
  - macOS's open-file events;
  - one running copy: a second start passes its files to the first and closes;
  - registering file types and their icons is left to packaging in R01.
- **Recent files:**
  - kept by the application, saved between runs;
  - added to the system's own list, such as Windows' jump list and macOS's recent documents, where there is one;
  - a ready-made menu section for the File menu, made with `dovetail-commands`.
- **An example** with a `--self-test` that drops files on it, starts a second copy with a file, and checks the recent list.

## What it leaves out

- Watching files for changes made by other programs.
- Packaging and file associations, which are part of [R01](R01-first-release.md).
- Dragging content other than files to and from the system, such as text and images.

## Done when

1. A file dragged from the system onto an element with a drop handler reaches it with its path, on Windows, Linux and macOS.
2. A file given on the command line, or opened from the file manager on macOS, opens in the application.
3. Starting a second copy with a file opens it in the first copy, and the second copy closes.
4. Opened files appear in the application's recent list and in the system's list where there is one, and the list is kept between runs.
5. The example's self-test checks dropped files, a second copy and the recent list.
