# D0017 — Documents: native file dialogs, recovery snapshots, tasks in core

| Field | Value |
|---|---|
| ID | D0017 |
| Status | Accepted |
| Date | 2026-11-23 |

## Context

M11 needs four things that Dovetail does not have yet:

- dialogs to choose a file to open, or a name to save as;
- a way to get work back after a crash;
- long-running work that shows progress and can be cancelled;
- a question before a window with unsaved changes closes.

## Decision

- **File dialogs are the system's own, where there is one.** `dovetail-core` has a `FileDialogs` host, like the clipboard, and `dovetail-desktop` installs it:
  - on Windows, `GetOpenFileNameW` and `GetSaveFileNameW`;
  - on macOS, `NSOpenPanel` and `NSSavePanel`;
  - on Linux, `zenity` or `kdialog` run as programs, when one is found.

  Where none of these is available, `dovetail-documents` shows a simple dialog of its own, drawn with Dovetail controls. The platform code follows D0013.
- **Recovery uses snapshots.** Shortly after edits stop, and at least every 30 seconds while there are unsaved changes, the whole document is written to a recovery file with the application's own save code. The running program holds a lock on the file, so another copy of the program does not take it. On the next start, a recovery file that no one holds is offered back. It is removed when the document is saved, or closed on purpose.
- **Long-running work is general and lives in `dovetail-core`.** A task runs on another thread, reports progress, checks whether it has been cancelled, and gives a result. Its callbacks run on the UI thread. The desktop wakes the event loop when a task sends a message.
- **The question before closing is drawn by Dovetail.** It is a modal box in the window, made from controls: Save, Don't save and Cancel. Components guard a window with `Element::close_guard`, and set its title with `Element::window_title`.

## Consequences

- File dialogs look and behave as users expect on Windows and macOS, and usually on Linux. The drawn dialog is plainer, but it means Open and Save As always work.
- Snapshots work with any kind of document, because they use the same code as saving. Edits made in the last moments before a crash can be lost.
- Any application can use tasks, not only documents. `dovetail-core` still has no dependencies: tasks use threads from the standard library.
- The question before closing is the same on every system, can be read by screen readers through our AccessKit tree, and can be tested by the self-test.

## Alternatives considered

- **Only a file dialog drawn by Dovetail:** no platform code, but it does not look or behave like the system's dialogs.
- **Native dialogs on Linux through the desktop portal:** the best Linux dialog, but it needs D-Bus, which we would have to write ourselves or add as a dependency.
- **A journal of edits for recovery:** loses nothing, but every edit would have to be written as data, which is a heavy demand on applications.
- **Tasks only in `dovetail-documents`:** smaller, but other kinds of long work would need them again later.
- **The system's message box before closing:** looks native, but needs more platform code, and blocks the event loop while it is shown.
