# M20 plan — Testing tools

| Field | Value |
|---|---|
| ID | M20 |
| Status | Done |
| Overview | [M20 overview](../overviews/M20-testing-tools.md) |
| Last updated | 2026-11-26 |

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
3. [x] **Move the input logic out of the desktop.** Move the input state and methods of `UiWindow` into `Session`, in a new package, `dovetail-session` (D0027), so that headless tests and real windows run the same code.
   - What moves: press and release, click to focus, pointer capture, click counting, hover, focus-within, Tab and the context menu keys, the wheel, scroll bar dragging, settling focus, size and scroll reports, and drag and drop within one window.
   - What it is given instead of reading the platform: the measure, the caret blink time, and whether scroll bars overlay.
   - What stays in the desktop: winit, the AccessKit adapter, the canvas, IME areas, window titles, and drags between windows.
   - The examples' self-tests must pass unchanged afterwards.
   - Done on 2026-11-25, in one move:
     - `crates/dovetail-session` holds `Session`, with `Settings` (the caret blink time, and a function that says whether scroll bars overlay), `Update` (what an update found, for the window to act on), `Request` (moved from `access.rs`) and `WHEEL_STEP`.
     - `UiWindow` now holds a `Session` and only what the window and painting need: the AccessKit adapter and tree, the canvas, the ring, caret and scroll bar damage, and the IME state. `app.rs` went from 2,466 to 1,785 lines.
     - The session still reads `Instant::now()`, until the clock in step 5.
     - The fmt and clippy checks pass, including for macOS and Linux, and so do the workspace tests and all 12 examples' self-tests on Windows.
     - Two new tests in `crates/dovetail-session/tests/session.rs` drive a session with no window: a click focuses and presses, Tab and Enter work, and a release somewhere else does not press.
     - One change in behaviour: `Context::pointer_leave`, for self-tests, now does what the pointer leaving a real window does, and so also stops highlighting the scroll bar that was under the pointer.
4. [x] **Split the accessibility tree.** Make the node data public and windowless: role, label, value, states, bounds, actions and children. The desktop keeps only the change to AccessKit's types. The tests in `access.rs` must give the same AccessKit tree as before.
   - Done on 2026-11-25:
     - `crates/dovetail-session/src/access.rs` builds an `AccessTree` of `AccessNode`s: kind and role, label, value, description, actions, toggled, selected, expanded, numbers, scrolling, places in lists and tables, children and the active descendant. `Session::access_tree` gives it for a session.
     - A text field's node says which box holds its text and where the selection is (`TextField`). Text runs stay in `dovetail-desktop`: they are AccessKit's way of describing text, and they need `dovetail-text` to find each character, which the session does not depend on (D0027).
     - `crates/dovetail-desktop/src/access.rs` now only turns the description into AccessKit's nodes, and keeps sending only what changed.
     - A third test in `crates/dovetail-session/tests/session.rs` reads the tree with no window: a button is named by its text, and offers to be pressed and focused.
     - The 19 tests in `access.rs` pass unchanged, as do the workspace tests, the fmt and clippy checks (including for macOS and Linux), and all 12 examples' self-tests. The controls self-test's tooltip step timed out once in seven runs, which this step does not touch: recorded as [B0008](../../bugs/B0008-controls-tooltip-step-flaky.md).
5. [x] **A clock in `dovetail-core`.**
   - `now()` and `set_clock`, installed as the clipboard and file dialogs are.
   - Timers on the UI thread: `Task::after` uses them instead of a sleeping thread, and the desktop wakes at the next one.
   - Every `Instant::now()` on the UI thread in the list above reads `now()`.
   - Background tasks and `SystemTime` (recovery's file names and "minutes ago") keep real time.
   - Done on 2026-11-25:
     - `crates/dovetail-core/src/clock.rs`: the `Clock` trait, `now()`, `set_clock`, and `TestClock`, whose `advance` fires each timer that falls due in order, with the clock at the timer's time.
     - `Task::after` keeps its signature and still returns a `Task` that can be cancelled, but it no longer starts a thread. It waits in the task list with a due time. `run_task_messages` fires the timers that are due and ends cancelled ones, so tests that call it in a loop keep working in real time. `next_timer` gives the next due time.
     - The desktop wakes at `next_timer`, along with the caret blink and `wake_after`, and fires due timers when it wakes.
     - `now()` replaces `Instant::now()` in the session (double-clicks, the caret blink, hiding scroll bars), the desktop (`wake_after` and its deadlines), `Selection`'s type-ahead and recovery's idle wait. Input logging keeps real time.
     - Two new tests in `crates/dovetail-core/src/task.rs`: timers fire in order at their own times with the test clock, a cancelled one ends at once, and a timer that sets the next, as an animation does, fires five times in one `advance`. The existing real-time timer test still passes.
     - The workspace tests, the fmt and clippy checks (including for macOS and Linux), and all 12 examples' self-tests pass on Windows. The controls, images and documents self-tests, which use timers for tooltips, animation and recovery, passed four more runs each.
     - Left for later: a test that scans the UI crates for `Instant::now()`, which D0027 suggests, so new code keeps to the clock.
6. [x] **`dovetail-testing`, without a window.** This is the main part of the package.
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
   - Done on 2026-11-25, in `crates/dovetail-testing`:
     - `TestUi::new(view)`, or `TestUi::with(view, Options { size, title, measure, settings })`. Any view can be the root. It installs the memory clipboard and a `TestClock` for the test's thread, and puts the real clock back when it is dropped. `resize` changes the size; scale waits for pictures in step 7.
     - Searches are built with `role(..)`, `label(..)` and `text(..)`, which can be combined and narrowed with `toggled` and `disabled`. `find`, `try_find`, `find_all`, `find_within`, `has` and `focused` search the session's accessibility tree, so a test finds what a screen reader is told. A `Found` gives an element's role, label, value, text, state, rectangle and focus.
     - `find` panics with a `FindError`. With no match, it names the search and prints the window's tree; with several, it lists each one and where it is.
     - Input: `click`, `click_at`, `double_click`, `right_click`, `hover`, `move_to`, `leave`, `press_at`, `release`, `drag`, `scroll`, `focus`, `press`, `key`, `type_text`, `shortcut` (`command()` gives Command on macOS and Ctrl elsewhere), `paste` and `clipboard`.
     - Time: `advance`, `run_tasks` (waits for work on other threads; timers wait for `advance`), `wait_until`, and `PATIENCE`, 10 seconds of real time.
     - `describe` prints the accessibility tree, one element a line.
     - A gap from step 4 was found and fixed: the session did not give a text field's text as its value, which the desktop added itself. The session gives it now.
     - `dovetail-core` gained `timers_waiting`, so `run_tasks` can tell work on other threads from timers.
     - 8 tests in `crates/dovetail-testing/tests/testing.rs`, and 2 examples in its documentation, pass. They meet criteria 2, 3 and 4 on Windows, and criterion 1 once CI runs them on all three systems.
     - Drag and drop moved into the session on 2026-11-25, as D0027 says: `dovetail_session::Drag<W>` offers a drag to the drop targets under the pointer, tells the one it left, and gives the drop and how the drag ended. `W` names a window, so the desktop still drags between windows (`Drag<WindowId>`, with `multi.rs` finding the window under the pointer), and `TestUi` uses `Drag<()>`. `TestUi::drag` now ends a drag and drop with a drop; `dragging`, `drag_end` and Escape work as in a window. A new test drags a card to a bin and gives up a second drag with Escape, with no window. The docking self-test, which drags panels between windows, passed three runs.
     - The workspace tests, the fmt and clippy checks (including for macOS and Linux), and all 12 examples' self-tests pass on Windows.
7. [x] **Pictures.**
   - `picture(element)` paints with `Gpu::headless` and the built-in stroke font, so a picture is the same on every system.
   - `assert_picture(element, "name")` compares the picture with `tests/pictures/name.png`, within a tolerance for each channel.
   - When the check fails, it saves the actual picture and a picture of the difference under `target/dovetail-pictures/`, and says where they are.
   - `DOVETAIL_UPDATE_PICTURES=1` saves new pictures instead of comparing.
   - It uses `encode_png`, and `snapshot.rs` uses it too instead of its own copy.
   - Done on 2026-11-25, behind `dovetail-testing`'s `pictures` feature:
     - `window_picture` paints the whole window as the desktop does (boxes, focus ring, caret, scroll bars and the theme's background) with `Gpu::headless`, at the scale in `Options`. `picture(&found)` keeps the part where an element is.
     - `Painter::use_built_in_font` was added to `dovetail-render`, so pictures draw text with the built-in font, which measures as `FixedPitch` does.
     - `assert_picture`, `assert_picture_within` and `assert_window_picture` compare with `tests/pictures/{name}.png`, within `TOLERANCE`, 2 levels in each channel for now. On a failure, the picture painted now and a difference picture (what changed in red, the rest faded) go to `target/dovetail-pictures/`, and the message gives both paths. A missing saved picture, or one of another size, fails with a message too. `DOVETAIL_UPDATE_PICTURES=1` saves instead.
     - `snapshot.rs` uses `encode_png` now.
     - 3 tests in `crates/dovetail-testing/tests/pictures.rs`, ignored by default as they need a GPU: a button matches its saved picture; "Count: 1" against the saved "Count: 0" fails, saves both pictures, and only the digit is red in the difference; and a window at scale 2 gives pictures twice the size. The saved pictures, `increment-button.png` and `count-zero.png`, were looked at before they were kept.
     - CI runs them in the window job's GPU step, one at a time on Linux as the other GPU tests are (B0007).
     - The tolerance has been tried only on an NVIDIA GPU here. WARP, Metal and lavapipe in CI will show whether 2 is enough.
     - The first CI run found one pixel of 1224 on a letter's edge further off on WARP. So `ODD_PIXELS_PER_THOUSAND`, 5, lets that many pixels in each thousand differ by more. A changed digit still gives 71 different pixels of 1224, where 6 may. A failure now also gives the largest difference, so CI shows how far off the systems are.
8. [x] **Real-window tests**, as the prototype decides:
   - the same finding, input and checking as step 6, through the `Context` hooks, which are extended for typing, pasting, hovering and dragging;
   - waiting for frames instead of acting in `frame_presented`;
   - a time limit on each step.
   - Done on 2026-11-25, behind `dovetail-testing`'s `windows` feature:
     - `dovetail_desktop::Pumped` drives the desktop's own runner a step at a time with `pump_app_events`. `with(work)` queues work that gets a `Context`, and the runner does it before the loop next waits. In pumped mode, closing the last window does not end the loop; the clipboard is kept in memory, and there are no native menus or file dialogs, so a test reaches nothing outside its windows. `run` and `Pumped` share the set-up (`start` and `finish` in `app.rs`).
     - Each window keeps `WindowStats`: redraw requests, frames shown and skipped, and its last hidden and focus events. `Context::window_stats`, `Context::ui_session` and `Context::focus_window` were added.
     - `dovetail_testing::windows::TestWindows` opens windows (brought to the front, as D0026 asks) and finds, clicks, hovers, drags, scrolls, types, pastes and presses shortcuts through the `Context` hooks. Each action waits for the window to draw a frame after it. `wait_until` fails after 10 seconds with what each window has received.
     - `run_window_tests` is `main` for a `harness = false` test program. It runs nothing without `--ignored`, takes a name filter, catches a failing test so the rest run, and fails a test that leaves a window open.
     - Searching moved into functions in `find.rs` that take an accessibility tree, so `TestUi` and `TestWindows` share them.
     - 4 tests in `crates/dovetail-testing/tests/windows.rs` pass on Windows: a click changes the text, typing and pasting, two windows at once, and a tooltip after a real wait. CI runs them in the window job on all three systems, in place of the prototype, and they block.
     - The workspace tests, the fmt and clippy checks (with every feature, and for macOS and Linux), the doc tests and all 12 examples' self-tests pass on Windows.
9. [x] **Use it in Dovetail's own tests.** Move the headless tests in `dovetail-controls`, `dovetail-data` and `dovetail-commands` onto `dovetail-testing` where it makes them shorter. Replace waiting in real time (tooltips, animation) with `advance`.
   - Done on 2026-11-25, in `dovetail-controls`, which uses `dovetail-testing` as a dev-dependency:
     - `tests/media.rs` (334 to 289 lines): the animation tests move the clock one 40 ms frame at a time, so they check every frame in order with no waiting; only decoding waits in real time. They replace the waits that failed on slow CI runners.
     - `tests/everyday.rs`: the tooltip test focuses, hovers and moves the clock through the window, instead of calling the handlers and sleeping.
     - `tests/controls.rs` (170 to 127 lines): finds the button and the switch by role and label, and clicks through the window.
     - `crates/dovetail-testing/tests/ui_clock.rs` scans the UI-thread code for `Instant::now()`, as D0027 suggested. The three places that must read real time (the clock itself, twice, and the input log) say why in a `// real time:` comment.
     - Not moved: the `dovetail-data` and `dovetail-commands` tests, and the rest of `everyday.rs` and `text_field.rs`. They drive handlers at a lower level, and are long; moving them is left as a follow-up, where it makes them shorter.
10. [x] **Use it in the examples' self-tests.** Every example keeps its `--self-test`. Rewrite them on the step 8 tools where that makes them shorter. Record each one's length before and after.
    - Done on 2026-11-25 for two examples, which now use `TestWindows`:
      - `examples/counter`: `main.rs` 179 to 84 lines; the self-test about 130 to 30. CI runs it on all three systems.
      - `examples/form`: `main.rs` 443 to 336 lines; the self-test about 190 to 45.
    - Left as they are: window, text-field, controls, themes, commands, data, images, gpu-canvas, docking and documents. They use what `TestWindows` does not wrap yet: input methods, reading pixels, scroll bars and stepped wheels, window places and drags between windows, file dialogs, resizing, minimising and device loss. Moving them is a follow-up, one example at a time, adding to `TestWindows` what each needs.
11. [x] **`dovetail new`** writes `tests/app.rs`, which builds the counter without a window, clicks its button and checks the count. The `new_project` test also runs `cargo test` in the new project.
    - Done on 2026-11-25, with one change: the test is a `#[cfg(test)]` module in `src/main.rs`, not `tests/app.rs`. A new project is one binary, and a test in `tests/` cannot reach a binary's components. The project's `Cargo.toml` gets `dovetail-testing` as a dev-dependency, by path next to `dovetail-ui`. The `new_project` test, which CI runs on all three systems, now runs the project's `cargo test` and checks that its test passes; it did here, in 32 seconds with everything else it builds.
12. [x] **Documents:**
    - a testing guide, `docs/architecture/testing.md`;
    - the package in `docs/architecture/crates.md`, its graph and the README's packages table;
    - CONTRIBUTING and AGENTS for the new way to test;
    - new words in the glossary.
    - Done on 2026-11-25: [testing](../../architecture/testing.md) covers headless tests, pictures and real windows, and is in the documents index and the architecture overview. The crates page and the README's packages table have both new packages, and the README's features list the testing tools. AGENTS.md says to test UIs with `dovetail-testing`, headless first, to move the clock instead of sleeping, and to read the time with `dovetail_core::now()`. CONTRIBUTING points to the guide. The glossary has headless test, pumped loop, saved picture, session and test clock.

## Prototypes and open decisions

- **Running real windows from tests** (step 2), with `harness = false` and `pump_app_events`. Decided: it works on all three systems ([prototype report](../../architecture/window-tests-prototype.md), [D0026](../../decisions/D0026-real-window-tests.md)).
- **Where the input logic goes, the clock, and the package:** decided in [D0027](../../decisions/D0027-testing-foundations.md):
  - a new package, `dovetail-session`, between `dovetail-layout` and `dovetail-desktop`, with no winit, wgpu or AccessKit;
  - `dovetail_core::now()` and timers on the UI thread, with background tasks kept in real time;
  - `dovetail-testing` as a dev-dependency of its own, not re-exported by `dovetail-ui`, with `pictures` and `windows` features.
- **Picture tolerance:** the same GPU drivers can differ in a channel by one or two levels. Choose the default from runs on WARP, Metal, lavapipe and a real GPU. Decided on 2026-11-26: 2 levels in each channel, and 5 pixels in each thousand may differ by more. It passes on all four.

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
