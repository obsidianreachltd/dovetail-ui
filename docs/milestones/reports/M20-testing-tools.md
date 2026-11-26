# M20 report — Testing tools

| Field | Value |
|---|---|
| ID | M20 |
| Finished on | 2026-11-26 |
| Commit or tag | `8d22e5c` |
| Plan | [M20 plan](../plans/M20-testing-tools.md) |

## Summary

Applications can now test their user interface with `dovetail-testing`. A test builds a component with no window or GPU, finds elements by role and label as a screen reader does, and clicks, types and scrolls through the window's own input. It moves a test clock instead of waiting, and can compare what is painted with a saved picture. The same tools drive real windows.

All six criteria are met. CI ran the headless tests, the picture tests and the real-window tests on Windows, Linux and macOS, and they passed.

## What was delivered

- **`dovetail-session`**, a new package between `dovetail-layout` and `dovetail-desktop` ([D0027](../../decisions/D0027-testing-foundations.md)). It holds the input logic that was private to the desktop, so a test and a real window run the same code:
  - `Session`: press and release, click to focus, pointer capture, click counting, hover, focus-within, Tab and the context menu keys, the wheel, scroll bar dragging, settling focus, and size and scroll reports;
  - `Drag<W>`: drag and drop, within one window or, with the desktop, between windows;
  - `AccessTree`: the accessibility tree with no AccessKit types. The desktop only turns it into AccessKit's nodes.
- **A clock in `dovetail-core`:** `now()`, `set_clock` and `TestClock`. `Task::after` waits in the task list instead of on a thread, and the desktop wakes at the next timer. Double-clicks, the caret blink, hiding scroll bars, type-ahead and recovery's idle wait read `now()`.
- **`dovetail-testing`**, a dev-dependency, not re-exported by `dovetail-ui`:
  - `TestUi`: building with no window, finding (`find`, `find_all`, `try_find`, `find_within`, `has`, `focused`, `describe`), input (pointer, keyboard, shortcuts, paste, drag and drop), and time (`advance`, `run_tasks`, `wait_until`);
  - the `pictures` feature: `picture`, `window_picture`, `assert_picture`, `assert_picture_within` and `assert_window_picture`, with saved pictures in `tests/pictures/`, failures in `target/dovetail-pictures/`, and `DOVETAIL_UPDATE_PICTURES=1` to save;
  - the `windows` feature: `TestWindows` and `run_window_tests`, for real windows in a `harness = false` test program ([D0026](../../decisions/D0026-real-window-tests.md)).
- **`dovetail-desktop`:** `Pumped` drives the desktop's runner a step at a time; `WindowStats` counts what each window received; `Context::window_stats`, `ui_session` and `focus_window`.
- **`dovetail-render`:** `Painter::use_built_in_font`, so pictures look the same on every system.
- **Dovetail's own tests:** the controls' `controls.rs`, `media.rs` and tooltip tests use `TestUi`, and the animation and tooltip tests move the clock instead of sleeping. `ui_clock.rs` scans the UI-thread code for `Instant::now()`.
- **Examples:** the counter and form self-tests use `TestWindows`.
- **`dovetail new`** writes a test in `src/main.rs` and adds `dovetail-testing` as a dev-dependency.
- **CI:** the window job runs the picture tests with the GPU tests, and the window tests, on all three systems. Both block.
- **Documents:** the [testing guide](../../architecture/testing.md), the [prototype report](../../architecture/window-tests-prototype.md), D0026 and D0027, and updates to the crates page, README, AGENTS.md, CONTRIBUTING and the glossary.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | A component with no window or GPU: find a button by its label, click it, and check the text, on all three systems in CI | Met | `a_click_on_a_button_found_by_its_label_changes_the_text` in `crates/dovetail-testing/tests/testing.rs`, and the controls' `controls.rs`, in CI's `cargo test --workspace` on Windows, Linux and macOS. |
| 2 | Finding nothing or several gives a message naming the search and what is there | Met | `a_search_with_no_match_says_what_the_window_has` and `a_search_with_several_matches_lists_them` in `testing.rs`. |
| 3 | Typing, shortcuts and paste, with no key presses sent to the system | Met | `typing_shortcuts_and_pasting_go_through_the_window` in `testing.rs`, headless, and `typing_and_pasting_go_to_the_window` in `tests/windows.rs`, in real windows on all three systems in CI. |
| 4 | Moving the clock finishes an animation or a delay without waiting | Met | `moving_the_clock_shows_a_tooltip_without_waiting` in `testing.rs`, the animation tests in `dovetail-controls/tests/media.rs`, the tooltip test in `everyday.rs`, and the clock tests in `dovetail-core/src/task.rs`. |
| 5 | A painted element compared with a saved picture, and the difference saved on failure | Met | `crates/dovetail-testing/tests/pictures.rs`, on WARP, Metal and lavapipe in CI and an NVIDIA GPU here. |
| 6 | A project made by `dovetail new` has a test that passes | Met | `crates/dovetail-cli/tests/new_project.rs` runs the new project's `cargo test`, in CI on all three systems. |

## Changes from the plan

- **Drag and drop moved into the session** (step 6) as `Drag<W>`, so headless tests drag as windows do. The plan had left drags between windows in the desktop; the desktop still finds the window, with `Drag<WindowId>`.
- **The `dovetail new` test** is a `#[cfg(test)]` module in `src/main.rs`, not `tests/app.rs` (step 11). A test in `tests/` cannot reach a binary's components.
- **Picture tolerance** (step 7): 2 levels in each channel was not enough on WARP, where one pixel of 1224 on a letter's edge was further off. `ODD_PIXELS_PER_THOUSAND`, 5, now lets that many pixels in each thousand differ by more. A failure also gives the largest difference.
- **Fewer tests moved** than steps 9 and 10 hoped. Only the controls' tests and two examples moved; see Follow-up.
- **The prototype's CI step** was replaced by M20's own window tests, as the plan said.

## Known issues

- [B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md): lavapipe crashed in the GPU tests on Linux CI. The GPU and picture tests run one at a time there.
- [B0008](../../bugs/B0008-controls-tooltip-step-flaky.md): the controls self-test's tooltip step timed out once in seven runs.

## Measurements

| What | Platform | Result |
|---|---|---|
| `dovetail-desktop/src/app.rs` after moving the input logic out | All | 2,466 to 1,785 lines |
| `examples/counter/src/main.rs` | All | 179 to 84 lines |
| `examples/form/src/main.rs` | All | 443 to 336 lines |
| `dovetail-controls/tests/media.rs` | All | 334 to 289 lines |
| `dovetail-controls/tests/controls.rs` | All | 170 to 127 lines |
| The `new_project` test, with the project's `cargo test` | Windows | 32 s |
| Pixels past the tolerance in `count-zero` | Windows, WARP | 1 of 1224 (6 may) |
| Pixels changed by "Count: 1" against "Count: 0" | Windows, NVIDIA | 71 of 1224 |

## Follow-up

- Move the `dovetail-data` and `dovetail-commands` tests, and the rest of `everyday.rs` and `text_field.rs`, onto `TestUi` where it makes them shorter. Recorded in step 9 of the M20 plan.
- Move the other 10 examples' self-tests onto `TestWindows`, one at a time, adding what each needs: input methods, reading pixels, scroll bars, window places, drags between windows, file dialogs, resizing, minimising and device loss. Recorded in step 10 of the M20 plan.
- [B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md) and [B0008](../../bugs/B0008-controls-tooltip-step-flaky.md) stay open.
