# D0027 — Testing foundations: windowless input in `dovetail-session`, a clock in core, and `dovetail-testing`

| Field | Value |
|---|---|
| ID | D0027 |
| Status | Accepted |
| Date | 2026-11-25 |

## Context

M20 needs tests that run an application's components with no window and no GPU, and still behave as a real window does. Three things stand in the way today ([M20 plan](../milestones/plans/M20-testing-tools.md), Starting point):

- **The input logic is private to `dovetail-desktop`.** `UiWindow` and `Shell` hold:
  - matching presses and releases;
  - click to focus and pointer capture;
  - counting double-clicks;
  - hover and focus-within;
  - Tab, and the context menu keys;
  - scroll bar dragging;
  - settling focus;
  - reporting sizes and scroll positions;
  - drag and drop.

  Headless tests skip all of this and call element handlers directly.
- **The accessibility tree** is built in `dovetail-desktop` as AccessKit's types. Nothing outside the desktop can search it by role or label.
- **Time is real.** The UI thread reads `Instant::now()` for double-clicks, the caret blink, hiding scroll bars, type-ahead, recovery and `wake_after`. `Task::after` sleeps on a thread. Tests of tooltips and animation wait in real time.

[D0026](D0026-real-window-tests.md) decided how tests drive real windows. This record decides the rest of M20's shape.

## Decision

### Windowless input: a new package, `dovetail-session`

A `Session` is one window's component tree, with its layout, focus, hover and input state, and no window. Headless tests and the desktop use the same `Session`, so they run the same code.

- **It depends on `dovetail-core` and `dovetail-layout` only.** No winit, wgpu or AccessKit.
- **What moves into it from `UiWindow`:**
  - the update cycle: building, layout passes, settling focus, hover, and size and scroll reports;
  - pointer input: press and release, click to focus, capture, click counting and the wheel;
  - keys: Tab, the context menu keys, and `dispatch_key`;
  - IME events;
  - scroll bar dragging;
  - drag and drop within one window;
  - the caret blink state;
  - the focus ring;
  - carrying out accessibility requests (focus, click, edit).
- **What it is given instead of reading the system:**
  - the `Measure`;
  - the caret blink time;
  - whether scroll bars overlay;
  - the size and scale.
- **What stays in `dovetail-desktop`:**
  - winit and turning its events into Dovetail's;
  - the canvas and painting;
  - IME areas and window titles;
  - drags between windows.
- **The accessibility tree splits in two.** `dovetail-session` builds a public, windowless description of each node: role, label, value, states, bounds, actions and children. `dovetail-desktop` only turns it into AccessKit's types. Tests search the same description that screen readers are given.

### A clock in `dovetail-core`

- **`dovetail_core::now()`** gives the time on the UI thread. Every `Instant::now()` on the UI thread reads it instead.
- **`set_clock`** installs another clock, as `set_clipboard` and `set_file_dialogs` do. With none installed, it is the real time.
- **Timers on the UI thread, with no thread.** `Task::after` keeps its signature and still returns a `Task` that can be cancelled. The callback waits in a list in core, which gives the time of the next one. The desktop wakes the event loop then, and runs the timers that are due.
- **A test clock** stays still until a test moves it forward. Moving it runs the timers that fall due, in order.
- **Background tasks keep real time.** `TaskContext::sleep` runs off the UI thread, and the test clock does not reach it. `SystemTime`, for recovery's file names and "N minutes ago", stays real too.

### The package for applications: `dovetail-testing`

- **Applications add it as a dev-dependency.** `dovetail-ui` does not re-export it, so test code never goes into an application, and `dovetail-testing` can depend on the rest without a loop.
- **Without features**, it needs `dovetail-core`, `dovetail-layout` and `dovetail-session`. It gives headless tests, the test clock and searching.
- **The `pictures` feature** adds `dovetail-render` and `dovetail-image`, to compare painted pictures with a GPU adapter.
- **The `windows` feature** adds `dovetail-desktop`, for the real-window runner of D0026.
- **Dovetail's own crates** use it as a dev-dependency for their tests. None of them may be a dependency of `dovetail-testing`, so there is no loop.

## Consequences

- **Headless tests behave as a window does:** a click focuses, Tab moves, hover and drag work, and sizes are reported. They still need no window or GPU, so they run in CI on every system.
- **`UiWindow` gets much smaller.** The move touches the busiest code in the desktop. It is done in small steps, with every example's self-test and the `access.rs` tests run after each.
- **One more package:** `dovetail-session` sits between `dovetail-layout` and `dovetail-desktop`. The [crates](../architecture/crates.md) graph and the README's table gain it and `dovetail-testing`.
- **Time in tests is exact.** Tooltips, animation, double-clicks and the caret blink are tested by moving the clock, not by waiting. The examples' self-tests keep real time, so real timing is still tested.
- **Every new use of time** on the UI thread must read `dovetail_core::now()`, not `Instant::now()`. A test that scans the UI crates for `Instant::now()` can keep to this, as one keeps colours from the theme.
- **`dovetail-core` still has no dependencies.** The clock and timers use only the standard library.

## Alternatives considered

- **Put the input logic in `dovetail-layout`**, beside `focus_next` and `dispatch_key`. That needs no new package, but layout would also hold the update cycle, focus history and time. Layout is easier to keep correct while it does only layout.
- **Make `UiWindow` public in `dovetail-desktop`.** Headless tests would then depend on winit, and `UiWindow` is tied to a winit window and a canvas.
- **Re-export `dovetail-testing` from `dovetail-ui` behind a feature.** One dependency line fewer for applications, but `dovetail-testing` could not use `dovetail-ui`, and an application could ship test code by mistake.
- **Pass the time to every function that needs it.** No global hook, but `Task::after`, `Selection` and recovery have no `Session` to ask. A hook in core is how the clipboard and file dialogs already reach code like this.
- **Keep `Task::after` on a sleeping thread, and have the test clock wake it.** The thread still sleeps in real time, so a test would wait.
