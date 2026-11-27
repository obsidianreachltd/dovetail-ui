# Examples

Each folder here is a small program that shows one part of Dovetail UI. Each is its own package in the workspace, named `example-<folder>`.

## Running an example

Run an example from the repository root:

```sh
cargo run -p example-counter
```

Most examples also take options after `--`:

| Option | What it does |
|---|---|
| `--self-test` | Drives the example's window by itself, checks what it shows, and exits with a failure code if a check fails. Every example has one. |
| `--bench` | Measures speed, without a window. Use it with `--release`. Only `example-data` and `example-gpu-canvas` have it. |

```sh
cargo run -p example-counter -- --self-test
cargo run --release -p example-data -- --bench
```

Run every example's self-test after a change to windows, layout or painting (see [CONTRIBUTING.md](../CONTRIBUTING.md#other-tests-and-tools)). In a POSIX shell, from the repository root:

```sh
for d in examples/*/; do cargo run -p "example-$(basename "$d")" -- --self-test || echo "FAILED: $d"; done
```

In PowerShell:

```powershell
Get-ChildItem examples -Directory | ForEach-Object { cargo run -p "example-$($_.Name)" -- --self-test; if ($LASTEXITCODE) { "FAILED: $($_.Name)" } }
```

On Linux without a display, run them under `xvfb-run -a`. The multi-window self-test also needs a window manager, because maximising needs one: CI runs `xvfb-run -a sh -c 'openbox & sleep 1; cargo run -p example-multi-window -- --self-test'`.

A self-test opens real windows and sends its clicks and keys to them through Dovetail, not through the system, so the mouse and keyboard stay free. Its windows can still come to the front while it runs.

## The examples

In rough order, from the simplest:

| Example | Milestone | What it shows |
|---|---|---|
| [`example-window`](window/src/main.rs) | M01 | The smallest program: windows cleared to a colour with wgpu. |
| [`example-counter`](counter/src/main.rs) | M04 | A button and a count, written with `#[component]` and `ui!`. |
| [`example-text-field`](text-field/src/main.rs) | M06, M13 | Text fields on one line and on several, with English and Arabic. |
| [`example-form`](form/src/main.rs) | — | A contact form: labelled fields, a toggle, buttons and checks on what is typed. |
| [`example-controls`](controls/src/main.rs) | M15 | Every everyday control in one window, in light and dark. |
| [`example-themes`](themes/src/main.rs) | M14 | One window in the light, dark, a custom and the system's theme. |
| [`example-commands`](commands/src/main.rs) | M08 | Commands in menus, a toolbar, shortcuts and command search, with undo. |
| [`example-data`](data/src/main.rs) | M16 | A list, a tree and a table of 100,000 rows each, and a property grid. |
| [`example-images`](images/src/main.rs) | M17 | Images, icons in every control that takes them, and animations. |
| [`example-motion`](motion/src/main.rs) | M18 | The system's settings as they change, and controls that move unless less motion is set. |
| [`example-gpu-canvas`](gpu-canvas/src/main.rs) | M09 | A PCB-like scene drawn with its own wgpu pipeline, with pan, zoom and selection. |
| [`example-docking`](docking/src/main.rs) | M10 | An editor-like window with panels that dock, resize and float. |
| [`example-documents`](documents/src/main.rs) | M11 | A small board of parts kept as a document, with saving, undo and recovery. |
| [`example-multi-window`](multi-window/src/main.rs) | M19 | Several windows that keep their places, a modal settings window, and full screen. |

### window

`cargo run -p example-window`

| Key | What it does |
|---|---|
| `N` | Opens another window. |
| `C` | Changes this window's colour. |
| `L` | Destroys the GPU device on purpose, to test recovery. |
| `Esc` | Closes this window. The example stops when the last one closes. |

Each window prints its size and display scale whenever they change.

### counter

`cargo run -p example-counter`

Click "Increment", or press Tab to reach it, then Enter or Space. This is the Counter from [the syntax notes](../docs/architecture/syntax.md).

### text-field

`cargo run -p example-text-field`

Click a field and type. Try selecting, undo, copy and paste, and an input method. The self-test also moves the caret through text that mixes English and Arabic.

### form

`cargo run -p example-form`

Fill it in with the mouse or the keyboard, where Tab moves between the parts, and press Submit. The fields check what is typed.

### controls

`cargo run -p example-controls`

The buttons at the top choose the light or dark theme. The list on the right scrolls. Right-click it, or press Shift+F10 on its button, for a context menu. The status bar shows what changed last.

### themes

`cargo run -p example-themes`

The buttons at the top choose the theme. "System" follows the system's light or dark mode, and its high-contrast setting, while the example runs. One section overrides part of the theme, and a badge uses a theme extension.

### commands

`cargo run -p example-commands`

A list editor. Ctrl+I adds an item, Ctrl+Z undoes, F10 opens the menus, and Ctrl+Shift+P searches for a command. On macOS use Command instead of Ctrl, and the menus are in the system's menu bar. Shortcuts can be changed.

### data

`cargo run -p example-data`

- **Left:** a tree of 100 layers with 1,000 shapes each. A layer's shapes are made only when it opens.
- **Middle:** a table of 100,000 parts. Drag a header's edge to resize its column, and press a header to sort by it.
- **Right:** the inspector edits the part the keyboard is on, and Ctrl+Z undoes. Parts dragged from the table can be dropped in the Picked list. The log has 100,000 entries of different heights.

`cargo run --release -p example-data -- --bench` measures how long each view takes to build and to scroll a frame.

### images

`cargo run -p example-images`

A PNG and a JPEG show at their own size, fitted into a square, and filling one. "Load a large image" makes a 4096 × 4096 picture and loads it while the window keeps drawing. Icons show in the menus, the toolbar, a button, the tabs, a tree and a dialog. An animated GIF and an APNG play, and "Pause" stops them.

### motion

`cargo run -p example-motion`

Change less motion, the text size, the caret blink or the double-click time in the system's settings, then come back to the window. The values at the top follow, and so do the controls. The self-test sets each setting inside the program, so it does not touch the system's own.

### gpu-canvas

`cargo run -p example-gpu-canvas`

Click and drag to select, drag with the right button to pan, and turn the wheel to zoom. `cargo run --release -p example-gpu-canvas -- --bench` measures frame times with 10,000 shapes, without a window.

### docking

`cargo run -p example-docking`

Drag tabs to dock panels beside others, into other stacks, along the edges, or out of the window to float them. Drag the lines between panels to resize them. F6 moves between panels. The layout is saved in the user's settings folder, or in the file named by `DOVETAIL_DOCK_FILE`, and comes back the next time.

### documents

`cargo run -p example-documents`

Add, move, rename and delete parts. Every edit can be undone. Save, Save As and Open use the system's file dialogs. The title shows `*` while there are unsaved changes, and closing asks first. Unsaved work is kept in the user's settings folder, or in the folder named by `DOVETAIL_RECOVERY_DIR`, and is offered back if the program stops without saving. "Check board" is a long task with progress and Cancel.

### multi-window

`cargo run -p example-multi-window`

"New document" opens a window for each document. "Settings" opens a modal window, which blocks the main window until it closes. F11, or Control+Command+F on macOS, or the Full Screen button makes the main window full screen. The list shows the open windows, and a click brings one to the front.

Every window keeps its place, size and state between runs, in `windows.txt` in the application's settings folder. Closing the main window ends the program on Windows and Linux. On macOS it keeps running, and a click on its Dock icon opens the main window again.

## Environment variables

| Variable | Used by | What it does |
|---|---|---|
| `DOVETAIL_DOCK_FILE` | docking | The file the dock layout is kept in. |
| `DOVETAIL_RECOVERY_DIR` | documents | The folder unsaved work is kept in. |
| `DOVETAIL_LOG_INPUT=1` | all | Logs every key, input-method and focus event to stderr ([B0004](../docs/bugs/B0004-doubled-letter-when-typing.md)). |

## Adding an example

A new example is a folder here with its own `Cargo.toml`, named `example-<folder>`. It needs a `--self-test` that drives its own window with `TestWindows` from `dovetail-testing` ([testing](../docs/architecture/testing.md)). Add it to the tables in this file and in the [root README](../README.md#examples).
