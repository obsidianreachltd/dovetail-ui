# M11 plan — Documents

| Field | Value |
|---|---|
| ID | M11 |
| Status | In progress |
| Overview | [M11 overview](../overviews/M11-documents.md) |
| Last updated | 2026-11-23 |

## Goal

Build the document lifecycle in `dovetail-documents`: saving, reopening, tracking unsaved changes, recovery after a crash, and long-running work.

## Starting point

- **Commands and undo** (M08): `Commands` runs named actions from menus, buttons and shortcuts, and `History` does edits so they can be undone and redone.
- **State** lives in signals outside the widgets, and is shared by every window on the UI thread (M02, M10).
- **Windows:**
  - components can open windows through the window host (M10);
  - modal layers keep focus and clicks inside them (M08);
  - there is no way for a component to set its window's title, or to be asked before its window closes.
- **What is missing:**
  - file dialogs;
  - work on other threads that reports back to the UI thread, and a way to wake the event loop when it does;
  - a progress bar;
  - somewhere to keep work for recovery, and a lock so that two copies of a program do not share it. `std::fs::File::try_lock` is stable from Rust 1.89, so the workspace's `rust-version` goes from 1.88 to 1.89.
- **Decided on 2026-11-23** ([D0017](../../decisions/D0017-documents-foundations.md)):
  - file dialogs are the system's own, with `zenity` or `kdialog` on Linux, and a dialog drawn by Dovetail where there is none;
  - recovery uses snapshots of the whole document;
  - long-running work is general and lives in `dovetail-core`;
  - the question before closing is drawn by Dovetail.
- **The new package** `dovetail-documents` is named in the [crate map](../../architecture/crates.md). It depends on `dovetail-core`, `dovetail-controls`, `dovetail-commands` and `dovetail-macros`.

## Steps

1. [x] Settle the open questions (above), on 2026-11-23.
2. [x] **Core**, done on 2026-11-23:
   - tasks (`task.rs`): `spawn_task` runs work on another thread with a `TaskContext` that reports progress, says whether the task was cancelled, and sleeps until it is. `Task` cancels it, and `Task::after` is a timer. Progress and the end (`TaskEnd`: done, cancelled, or failed when the work panics) are given to callbacks on the UI thread by `run_task_messages`. Only the latest progress is kept, so a busy task wakes the UI thread once;
   - `FileDialogs`, a host for open and save dialogs (`dialogs.rs`), with `FileDialog` (title, folder, name, filters) and `show_file_dialog`, which says when there is no dialog to show;
   - `Element::window_title`, and `Element::close_guard` with a `CloseGuard`, whose `close_now` closes the window without asking again;
   - `Role::ProgressBar`, `Role::Dialog`, and `Element::value`.
3. [x] **Desktop**, done on 2026-11-23 (`dovetail-desktop/src/app/lifecycle.rs`):
   - a task wakes the event loop through its proxy, and the tasks' callbacks run there;
   - each window's title follows its elements after layout;
   - a close request goes to the window's close guard first, and windows whose guard calls `close_now` close at the end of the turn;
   - `Context::request_close` closes a window as the user would;
   - file dialogs: `GetOpenFileNameW` and `GetSaveFileNameW` on Windows, owned by the window in front; `NSOpenPanel` and `NSSavePanel` on macOS; `zenity` or `kdialog` on Linux, run on a task's thread (`dialogs.rs`, with tests of their arguments);
   - AccessKit: a progress bar has its value from 0 to 100, and a dialog is modal.
4. [x] **Controls**, done on 2026-11-23: `ProgressBar`, and `Dialog` with `DialogButton`s. The first button takes focus, and Escape calls `on_cancel`.
5. [x] **The model**, done on 2026-11-23: `Document<T>` and `DocumentFile` in `dovetail-documents`. `edit` keeps the data from before and after each edit, and each state has a number, so undoing back to the saved state counts as saved. `DocumentKind` holds the format. Saving writes `NAME.saving` and renames it over the old file.
6. [x] **Recovery**, done on 2026-11-23: `Recovery` and `Recovered`. Each document with unsaved changes has `NAME.recover`, written through a new file, and `NAME.lock`, which `File::try_lock` holds while the program runs. `found` skips held files and removes stray locks. Saving, undoing back to the saved state, and closing with Don't save remove the snapshot.
7. [x] **The views**, done on 2026-11-23: `DocumentFrame`, the questions (unsaved changes, failure, recovery), the file dialog drawn by Dovetail, and `TaskProgress`. `add_commands` defines New, Open, Save, Save As, Undo and Redo.
8. [x] **`examples/documents`**, done on 2026-11-23: a board of parts in its own text format (`dovetail-board 1`). Add, delete, move and rename are commands. "Check board" is a task of 40 steps with progress and Cancel. Unsaved work is kept in the user's settings folder, or in the folder named by `DOVETAIL_RECOVERY_DIR`.
9. [x] **Tests**, done on 2026-11-23:
   - tasks (4) and dialogs (2) in core, the Linux dialog arguments (3), recovery files and locks (3), and the document model (7): unsaved changes through undo and redo, saving and opening, failures that change nothing, the file commands and their questions, the frame's own dialog, closing, and recovery after the program's state goes;
   - `examples/documents --self-test`, 17 checks: edits through commands and the `*` in the title; Undo and Redo; Save As; Open asks first, and Don't save opens the saved file, which reads back as the same board; the check shows progress, is cancelled, and runs to the end; closing asks, Escape keeps the window, and Save saves and closes it; a copy of the program edits until its work is kept and is killed, and the next copy offers the work back and recovers it.

## Prototypes and open decisions

- **File dialogs:** decided, the system's own with a fallback (D0017).
- **Recovery:** decided, snapshots (D0017).
- **Long-running work:** decided, general, in core (D0017).
- **Close question:** decided, drawn by Dovetail (D0017).
- **Undo by snapshot:** `Document::edit` keeps the data from before and after each edit, which is simple and works for any type that can be cloned. Big documents will want to share unchanged parts, for example with `Rc`. That is the application's choice of data type.
- **Several documents in one program:** each `Document` is separate, so a program can have several. Tabs of documents are left to applications, using M10's docking.

## Risks

- **Modal native dialogs inside the event loop:** the system dialogs run their own loop while they are open. winit holds back the events it gets meanwhile. If this goes wrong on a system, the dialog can be shown from the next turn of the event loop.
- **macOS and Linux code not run:** it is type-checked here, but must be run on those systems.
- **Linux without `zenity` or `kdialog`:** the drawn dialog is plain, but works.
- **Killing a process on purpose in a test:** it is a child process of the self-test, started and stopped by it, and only writes in a folder of its own in the temporary folder.

## Testing

- **Unit tests** for tasks, the document model, saving, and recovery files and locks.
- **`examples/documents --self-test`** for the whole lifecycle in a real window, with a test host for file dialogs so that nothing waits for a person.
- **The crash check** in the self-test: the work comes back after the process is killed.
- **By hand, on each system:** the real file dialogs, open and save, cancelled and chosen. Not done yet.
- **Results on 2026-11-23**, Windows 11:
  - 229 workspace tests pass, and every example's self-test passes, `examples/documents` included;
  - the real example, driven through UI Automation only, with no keys or mouse input: Save As opened the system's Save dialog, titled "Save Board", with the name Untitled and the filter "Board files (*.board)". Cancelling it saved nothing. Closing the window with unsaved changes showed the question, and Don't save closed it. A run that was stopped after an edit left its work, and the next start offered it back and recovered it. Pictures of the questions look right;
  - choosing a file in the system's dialog could not be done through UI Automation, which does not reach the dialog's name box. It is left for the check by hand. The file dialog drawn by Dovetail is tested without a window, but has not been seen on a system without one of its own.

## Done when

Copied from the overview.

1. Document state lives outside widget state, and every edit goes through a command.
2. A round-trip test shows that saving and reopening gives the same document.
3. Unsaved changes are shown in the window title, and closing asks first.
4. A test that stops the process during editing shows the work is recovered on the next start.
5. A long operation shows progress and can be cancelled.
