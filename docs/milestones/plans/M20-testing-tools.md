# M20 plan — Testing tools

| Field | Value |
|---|---|
| ID | M20 |
| Status | In progress |
| Overview | [M20 overview](../overviews/M20-testing-tools.md) |
| Last updated | 2026-11-25 |

## Goal

Give application developers a supported way to test their user interface: find elements by role and label, click, type and scroll through the window itself, and check what is shown, with or without a GPU.

## Starting point

- **Hooks for self-tests (M01 onwards):** `Context` in `dovetail-desktop` has `click`, `key`, `pointer`, `pointer_button`, `wheel`, `wheel_steps`, `ime`, `ui_boxes`, `ui_pixels`, `focused_element`, `caret_blink` and more.
  - They go through the same code as real input, but skip the step that turns winit's events into Dovetail's.
  - They need a real window and event loop. Nothing is drawn until the next frame, so self-tests act in `frame_presented` and check on the frame after.
  - There is no helper for typing text, pasting, hovering or dragging. Examples write their own.
- **Input logic is private to the desktop.** `UiWindow` and `Shell` in `crates/dovetail-desktop/src/app.rs` and `app/multi.rs` hold the logic for:
  - pressing and releasing, and matching the two;
  - click to focus and pointer capture;
  - double-click counting;
  - hover and focus-within;
  - Tab, and the context menu keys;
  - scroll bar dragging;
  - reporting sizes and scroll positions;
  - settling focus;
  - drag and drop.

  It reads the platform for the caret blink time and overlay scroll bars, and measures with `font_measure()`.
- **What works without a window**, in `dovetail-layout`: `Layout::update`, `hit_test`, `focus_next`, `dispatch_key`, `text_at`, `scroll_by` and the scroll bar queries.
  - Headless tests use these and call element handlers directly, with `FixedPitch` measuring.
  - So they miss everything the desktop adds: press and release matching, click to focus, Tab, hover, and size and scroll reports.
- **The accessibility tree:** `Access` in `crates/dovetail-desktop/src/access.rs` is `pub(crate)`. It builds the AccessKit tree from a `Layout` and a `Measure`, and needs no window, as its own tests show. Nothing outside the desktop can look a node up by role or label.
- **Time:** there is no clock of our own. The UI thread reads `Instant::now()` for:
  - `wake_after`;
  - double-clicks;
  - hiding overlay scroll bars;
  - the caret blink;
  - type-ahead in `Selection`;
  - recovery's idle timer.

  `Task::after` sleeps on a thread. It is used for the tooltip delay, the animation frame timer and recovery. Tests that use them wait in real time: the tooltip test, the media tests and the documents test.
- **Pictures:** `Gpu::headless`, `Painter::paint` and `Painter::read_pixels` paint without a window, but need a GPU adapter. CI has one on every system (WARP, Metal, and lavapipe on Linux).
  - `dovetail_image::encode_png` and `decode_rgba` exist.
  - `examples/snapshot.rs` has its own copy of a PNG encoder.
  - There are no saved pictures, and nothing to compare against them.
- **Fonts:** `Fonts::system()` reads the system's font files. Setting `DOVETAIL_FONTS` to a missing file makes measuring and painting fall back to the built-in stroke font, the same on every system. `FixedPitch` measures as that font does.
- **Repeated helpers:**
  - at least 9 test files define their own `Window { tree, layout }` with `open` and `refresh`;
  - `texts`, `has_text`, `all`, `one`, `key` and `command()` appear again and again;
  - the 12 examples' self-tests repeat `element`, `text_of`, `find`, `centre`, `type_text`, a pixel reader, and the same machine of steps. They are 130 to 770 lines each.
- **`dovetail new`** writes `Cargo.toml`, `src/main.rs` (a counter) and `.gitignore`, and no test.

## Steps

1. [x] **Decide the shape**, after the prototype in step 2:
   - [x] how real-window tests run (step 8): decided in [D0026](../../decisions/D0026-real-window-tests.md) on 2026-11-25;
   - [x] where the window-free input logic goes (step 3), how the clock works (step 5), and the package and how applications add it: decided in [D0027](../../decisions/D0027-testing-foundations.md) on 2026-11-25.
2. [x] **Prototype a real-window test.** winit allows one event loop for each process. On macOS it must run on the main thread, but `cargo test` runs tests on other threads. Try a test target with `harness = false` that drives the event loop one step at a time with winit's `pump_app_events`, opening a window, clicking and checking. Run it in CI on all three systems before step 8 is committed.
   - Built on 2026-11-25 in `prototypes/window-tests`, with results in the [prototype report](../../architecture/window-tests-prototype.md). All six of its tests pass on Windows, Linux and macOS. One macOS run failed: windows opened after the last one closed did not draw. The next run passed, with and without bringing windows to the front, so the cause is not known yet. The prototype opens, resizes and closes windows, but sends no input: clicking goes through the `Context` hooks in step 8, which the self-tests already use on all three systems.
3. [ ] **Move the input logic out of the desktop.** Move the input state and methods of `UiWindow` into `Session`, in a new package, `dovetail-session` (D0027), so that headless tests and real windows run the same code.
   - What moves: press and release, click to focus, pointer capture, click counting, hover, focus-within, Tab and the context menu keys, the wheel, scroll bar dragging, settling focus, size and scroll reports, and drag and drop within one window.
   - What it is given instead of reading the platform: the measure, the caret blink time, and whether scroll bars overlay.
   - What stays in the desktop: winit, the AccessKit adapter, the canvas, IME areas, window titles, and drags between windows.
   - The examples' self-tests must pass unchanged afterwards.
4. [ ] **Split the accessibility tree.** Make the node data public and windowless: role, label, value, states, bounds, actions and children. The desktop keeps only the change to AccessKit's types. The tests in `access.rs` must give the same AccessKit tree as before.
5. [ ] **A clock in `dovetail-core`.**
   - `now()` and `set_clock`, installed as the clipboard and file dialogs are.
   - Timers on the UI thread: `Task::after` uses them instead of a sleeping thread, and the desktop wakes at the next one.
   - Every `Instant::now()` on the UI thread in the list above reads `now()`.
   - Background tasks and `SystemTime` (recovery's file names and "minutes ago") keep real time.
6. [ ] **`dovetail-testing`, without a window.** This is the main part of the package.
   - **Building:** `TestUi::new(view)` at a chosen size and scale, with `FixedPitch` measuring by default, or the system's fonts. It uses the memory clipboard and the test clock.
   - **Finding:** by role, label, text and state, from the step 4 data:
     - `find` returns one element, `find_all` returns any number, and `within` searches inside an element;
     - when nothing or more than one matches, the message names the search and lists the elements that are there, with their roles and labels.
   - **Input through the window** (through `Session`), with no system key presses:
     - click, double-click, right-click, hover, drag and scroll;
     - `type_text`, `key` and `shortcut` (with the command key for each system), and paste from the memory clipboard;
     - every action builds and lays out again before it returns.
   - **Waiting and time:**
     - `advance(duration)` moves the test clock and fires the timers that are due;
     - `run_tasks()` runs background tasks to the end, with a time limit;
     - `wait_until(condition)` gives up with a message after a time limit.
   - **Checking:**
     - an element's text, value, state and rectangle;
     - focus;
     - a text dump of the accessibility tree.
7. [ ] **Pictures.**
   - `picture(element)` paints with `Gpu::headless` and the built-in stroke font, so a picture is the same on every system.
   - `assert_picture(element, "name")` compares the picture with `tests/pictures/name.png`, within a tolerance for each channel.
   - When the check fails, it saves the actual picture and a picture of the difference under `target/dovetail-pictures/`, and says where they are.
   - `DOVETAIL_UPDATE_PICTURES=1` saves new pictures instead of comparing.
   - It uses `encode_png`, and `snapshot.rs` uses it too instead of its own copy.
8. [ ] **Real-window tests**, as the prototype decides:
   - the same finding, input and checking as step 6, through the `Context` hooks, which are extended for typing, pasting, hovering and dragging;
   - waiting for frames instead of acting in `frame_presented`;
   - a time limit on each step.
9. [ ] **Use it in Dovetail's own tests.** Move the headless tests in `dovetail-controls`, `dovetail-data` and `dovetail-commands` onto `dovetail-testing` where it makes them shorter. Replace waiting in real time (tooltips, animation) with `advance`.
10. [ ] **Use it in the examples' self-tests.** Every example keeps its `--self-test`. Rewrite them on the step 8 tools where that makes them shorter. Record each one's length before and after.
11. [ ] **`dovetail new`** writes `tests/app.rs`, which builds the counter without a window, clicks its button and checks the count. The `new_project` test also runs `cargo test` in the new project.
12. [ ] **Documents:**
    - a testing guide, `docs/architecture/testing.md`;
    - the package in `docs/architecture/crates.md`, its graph and the README's packages table;
    - CONTRIBUTING and AGENTS for the new way to test;
    - new words in the glossary.

## Prototypes and open decisions

- **Running real windows from tests** (step 2), with `harness = false` and `pump_app_events`. Decided: it works on all three systems ([prototype report](../../architecture/window-tests-prototype.md), [D0026](../../decisions/D0026-real-window-tests.md)).
- **Where the input logic goes, the clock, and the package:** decided in [D0027](../../decisions/D0027-testing-foundations.md):
  - a new package, `dovetail-session`, between `dovetail-layout` and `dovetail-desktop`, with no winit, wgpu or AccessKit;
  - `dovetail_core::now()` and timers on the UI thread, with background tasks kept in real time;
  - `dovetail-testing` as a dev-dependency of its own, not re-exported by `dovetail-ui`, with `pictures` and `windows` features.
- **Picture tolerance:** the same GPU drivers can differ in a channel by one or two levels. Choose the default from runs on WARP, Metal, lavapipe and a real GPU.

## Risks

- **Moving the input logic can break input.** It is a large move inside the busiest code. Move it in small steps, with every example's self-test and the `access.rs` tests run after each.
- **Real-window tests may not fit `cargo test` on macOS.** The prototype showed they do, but one macOS run failed for a reason not yet known. CI keeps running the prototype until M20's own window tests replace it. If the failure comes back and cannot be fixed, the fallback in D0026 is a program for each test.
- **Pictures differ between systems.** The built-in font, a fixed scale and a tolerance should be enough. If they are not, pictures are saved for each system.
- **Lavapipe crashes** ([B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md)). Picture tests on Linux CI may need to run one at a time, as the GPU tests do.
- **A test clock can hide timing bugs** that only real time shows. The examples' self-tests keep real time, so both are tested.
- **The package can grow too wide.** Add only what Dovetail's own tests and examples use.

## Testing

- **The package's own tests:**
  - every finder, with the messages for no match and for several matches;
  - every action, with the handler it should call;
  - the clock firing timers in order;
  - a picture that matches, and one that does not, which saves the difference.
- **Behaviour must stay the same after steps 3 and 4:**
  - the `access.rs` tests;
  - the headless tests in every crate;
  - every example's self-test, on Windows and in CI.
- **Pictures** are compared in CI on Windows, Linux and macOS, with the GPU tests.
- **`dovetail new`:** the `new_project` test runs the project's tests.
- **By hand:** nothing on any system beyond the self-tests. The package only drives windows; it does not change how they look or feel.

## Done when

Copied from the overview.

1. A test can build a component without a window or GPU, find a button by its label, click it, and check the text that changes, on Windows, Linux and macOS in CI.
2. Finding an element that matches nothing or several gives a message that names what was searched for and what is there.
3. A test can type into a text field, press shortcuts and paste, with no key presses sent to the system.
4. A test moves the clock forward to finish an animation or a delay without waiting for it.
5. A painted element can be compared with a saved picture, and the difference saved when it fails.
6. A project made by `dovetail new` includes a test that passes.
