# Testing user interfaces

How to test a Dovetail application, or Dovetail's own controls, with the `dovetail-testing` package (M20). The design is in [D0026](../decisions/D0026-real-window-tests.md) and [D0027](../decisions/D0027-testing-foundations.md).

## The three kinds of test

| Kind | Use it for | Needs | Runs with |
|---|---|---|---|
| **Headless**, `TestUi` | Almost everything: what is shown, input, focus, timers | Nothing | `cargo test` |
| **Pictures**, the `pictures` feature | How something looks | A GPU adapter | `cargo test --features pictures -- --ignored` |
| **Real windows**, the `windows` feature | What only a window has: frames, the desktop's own handling of input | A display | `cargo test --features windows --test windows -- --ignored` |

Start with a headless test. It runs in milliseconds, in CI on every system, with no window or GPU. It still runs the same input code as a real window (`dovetail-session`).

## Adding the package

Applications add it as a dev-dependency. `dovetail new` does this already, and writes a first test in `src/main.rs`.

```toml
[dev-dependencies]
dovetail-testing = { path = "../dovetail/crates/dovetail-testing" }
```

`dovetail-ui` does not re-export it, so test code never goes into an application.

## A headless test

```rust
use dovetail::prelude::*;
use dovetail_core::Role;
use dovetail_testing::{TestUi, role, text};

#[test]
fn increment_counts() {
  let mut ui = TestUi::new(ui! { Counter {} });
  ui.click(role(Role::Button).label("Increment"));
  ui.find(text("Count: 1"));
}
```

`TestUi::new` builds the component and lays it out in 800 by 600 logical pixels. `TestUi::with` and `Options` choose the size, scale, title, text measuring and the system's settings. A test changes the settings while it runs with `dovetail_core::set_system_settings`, and the window follows, as a real one does. Every action lays out again before it returns, so a test can check what changed straight away.

### Finding

A test finds elements as a screen reader does: by **role**, **label** (the name a screen reader reads) and **text** (the name, or else the value). It does not find them by position, so tests keep working when the layout moves.

- Start a search with `role(..)`, `label(..)` or `text(..)`, and add more with `.label(..)`, `.text(..)`, `.toggled(..)` and `.disabled(..)`.
- `find` returns the one match, and panics when there is none or more than one. With none, the message prints what the window has. With several, it lists them and where they are.
- `find_all`, `has`, `try_find`, `find_within` and `focused` cover the other cases.
- `describe()` prints the window's accessibility tree, one element a line: the quickest way to see what a test can find.

A found element gives its role, label, value, text, whether it is on, disabled or focused, and its rectangle.

Give every control a role and a label, as AGENTS.md asks. A test that cannot find a control by them is often a sign that a screen reader cannot either.

### Input

All input goes to the window, never through the system. A test cannot type into another program, and needs no window in front.

- The pointer: `click`, `double_click`, `right_click`, `hover`, `leave`, `drag`, `scroll`, and `click_at`, `move_to`, `press_at` and `release` at points.
- The keyboard: `key(NamedKey::Tab)`, `type_text("Ann\n")`, `shortcut('s')` (Command on macOS, Ctrl elsewhere), `press(event)`.
- `focus(..)` gives focus as an assistive tool would.
- The clipboard is kept in memory: `paste(text)` and `clipboard()`.
- Drag and drop works as in a window. `dragging()` and `drag_end()` say how a drag went, and Escape gives it up.

### Time

A `TestUi` installs a clock that stands still. Nothing on the UI thread moves until the test says:

```rust
ui.hover(label("Save"));
ui.advance(Duration::from_millis(500));
ui.find(role(Role::Tooltip));
```

`advance` fires the timers that fall due, in order, with the clock at each timer's time. While a component asks for frames with `request_frame()`, it also gives a frame every `FRAME`, a 60th of a second of test time. Tooltips, animation frames, double-clicks, the caret blink and hiding scroll bars all follow it. A test never needs to sleep for them.

An animated value from `use_animated` moves with these frames, so a test steps through an animation by moving the clock, frame by frame if it needs to. Each frame is built at its exact time, so an animation of 200 ms ends at the frame on or just after 200 ms. A test that checks what a change shows, not how it moves, can advance past the animation, or set less motion in `Options::settings` so that values jump.

Work on other threads, such as decoding an image or saving a file, takes real time. `run_tasks()` waits for it, and `wait_until(what, condition)` waits for a condition. Both give up after 10 seconds with a message.

Framework code on the UI thread must read the time with `dovetail_core::now()`, not `Instant::now()`. A test (`crates/dovetail-testing/tests/ui_clock.rs`) scans for it.

A test that builds a `Tree` itself, with no `TestUi`, has no frames and a real clock, so animations do not finish in it. Such a test sets less motion with `set_system_settings`.

### Windows

A `TestUi` installs a window host with no windows (M19). Each `Window` a component shows gets a session of its own, so a test finds the windows a tree has open, looks inside them, and closes them:

```rust
let mut ui = TestUi::new(ui! { App {} });
assert_eq!(ui.windows().len(), 1);
ui.switch_to(ui.window("Settings"));
ui.click(role(Role::Button).label("Done"));
assert!(ui.windows().is_empty());
```

- **The window in front:** `switch_to` makes finding, the pointer and the keys act in one window, as a user who brings it to the front. `switch_to_main` goes back. When the window in front closes, the main one is in front again.
- **As the user:** `close_window` closes a window from its title bar, which asks its component. `set_window_state` and `move_window` change its state and place, and tell its component.
- **Modal windows:** a window that a modal window blocks refuses clicks, keys and closing, and a click brings the modal window to the front. `blocked_by` says which window blocks another.
- **Sizes:** each window is laid out at its own size, within its limits. Its text is measured with `FixedPitch`.

## Pictures

With the `pictures` feature, a test can compare how an element looks with a saved PNG:

```rust
let button = ui.find(role(Role::Button).label("Increment"));
ui.assert_picture(&button, "increment-button");
```

- **The saved picture** is `tests/pictures/increment-button.png` in the package.
- **Same on every system:** pictures are painted with the GPU and Dovetail's built-in font, so they look the same on every system. Each colour channel may differ by up to 2 levels, and 5 pixels in each thousand may differ by more, because GPUs cover the edges of letters a little differently. A failure says how many pixels differ and by how much.
- **When a comparison fails:** the picture painted now and a picture of the difference, with changed pixels in red, are saved in `target/dovetail-pictures/`. The message gives both paths.
- **Saving new pictures:** after a change on purpose, run the tests with `DOVETAIL_UPDATE_PICTURES=1`, and look at each new picture before committing it.

These tests need a GPU, so mark them `#[ignore = "needs a GPU adapter"]`. On Linux without a GPU, Mesa's lavapipe works.

## Real windows

With the `windows` feature, `TestWindows` drives real windows. It finds and clicks as `TestUi` does, and waits for the window to draw each change.

On macOS, winit needs the main thread, which `#[test]` functions do not run on. So window tests go in a test program of their own, with `harness = false`:

```toml
[[test]]
name = "windows"
harness = false
```

```rust
fn counts(t: &mut TestWindows) {
  let w = t.open("Counter", (300.0, 200.0), ui! { Counter {} });
  t.click(w, label("Increment"));
  t.find(w, text("Count: 1"));
  t.close(w);
}

fn main() -> ExitCode {
  run_window_tests(&[("counts", counts)])
}
```

How these tests run:
- **Ignored by default:** they run only with `--ignored`, so a plain `cargo test` never opens windows.
- **Picking tests:** a word after `--ignored` runs only the tests whose names contain it.
- **Failures:** a test fails when it panics or leaves a window open.
- **Waiting:** a wait that runs out says what each window has received from the system and the GPU.
- **Time:** time is real here.

The counter's and the form's `--self-test` are written this way, as an example.

A whole application, with its own windows, runs with `set_root`, as `run_windows` runs it (M19). `window_titled` waits for a window a component opened. `set_state`, `set_size` and `move_to` change a window as the system would, and `request_close` asks it to close as its close button does. The multi-window example's `--self-test` closes its windows and starts the application again, to check that they come back where they were.

## Dovetail's own tests

- Controls' headless tests use `TestUi`, as `crates/dovetail-controls/tests/controls.rs`, `everyday.rs` (tooltips) and `media.rs` (animation) do.
- `crates/dovetail-testing/tests/` tests the package itself: `testing.rs` headless, `pictures.rs` and `windows.rs`.
- Examples keep their `--self-test`, as AGENTS.md asks. Write new ones with `TestWindows`.
