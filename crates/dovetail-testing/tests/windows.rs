//! M20 step 8: tests with real windows, run from `cargo test` on the main
//! thread (D0026).
//!
//! `cargo test -p dovetail-testing --features windows --test windows --
//! --ignored`

use std::process::ExitCode;
use std::time::Duration;

use dovetail::controls::Tooltip;
use dovetail::desktop::{LogicalSize, WindowOptions};
use dovetail::prelude::*;
use dovetail_core::{
  NamedKey, Role, WindowLimits, WindowState, now, request_frame,
};
use dovetail_testing::windows::{TestWindows, run_window_tests};
use dovetail_testing::{label, role, text};

#[component]
fn Counter() -> View {
  let count = use_state(|| 0);
  ui! {
    Column {
      Text { value: count.map(|n| format!("Count: {n}")) }
      Button { text: "Increment", on_click: move |_| count.update(|n| *n += 1) }
    }
  }
}

#[component]
fn Form() -> View {
  let name = use_state(String::new);
  let saved = use_state(|| 0);
  ui! {
    Column {
      TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
      Text { value: name.map(|n| format!("Hello, {n}")) }
      Button { text: "Save", on_click: move |_| saved.update(|n| *n += 1) }
      Text { value: saved.map(|n| format!("Saved {n} times")) }
    }
  }
}

#[component]
fn Tipped() -> View {
  ui! {
    Column {
      Tooltip {
        text: "Saves the settings",
        Button { text: "Save" }
      }
    }
  }
}

/// Moves for 300 ms after it starts, asking for a frame on each build
/// until then (M18).
#[component]
fn Mover() -> View {
  let start = use_state(now).get();
  let elapsed = now() - start;
  let done = elapsed >= Duration::from_millis(300);
  if !done {
    request_frame();
  }
  ui! {
    Text { value: if done { "Done".to_string() } else { format!("{} ms", elapsed.as_millis()) } }
  }
}

fn a_real_window_draws_frames_only_while_asked(t: &mut TestWindows) {
  let w = t.open("Frames", (300.0, 200.0), ui! { Mover {} });
  let before = t.stats(w).presented;
  t.wait_until("the end of the movement", move |t| t.has(w, text("Done")));
  let moving = t.stats(w).presented - before;
  // About 18 frames at 60 a second; fewer on a slow machine, but more
  // than the one or two a change would draw.
  assert!(moving >= 5, "{moving} frames while moving");
  // Nothing moves now, so nothing is drawn.
  let after = t.stats(w).presented;
  t.pause(Duration::from_millis(300));
  assert_eq!(t.stats(w).presented, after, "no frames while still");
  t.close(w);
}

fn a_click_in_a_real_window_changes_the_text(t: &mut TestWindows) {
  let w = t.open("Counter", (300.0, 200.0), ui! { Counter {} });
  t.find(w, text("Count: 0"));
  t.click(w, role(Role::Button).label("Increment"));
  t.click(w, label("Increment"));
  t.find(w, text("Count: 2"));
  assert!(t.stats(w).presented >= 3, "a frame for each change");
  t.close(w);
}

fn typing_and_pasting_go_to_the_window(t: &mut TestWindows) {
  let w = t.open("Form", (400.0, 240.0), ui! { Form {} });
  let name = role(Role::TextInput).label("Name");
  t.click(w, name.clone());
  t.type_text(w, "Ann");
  t.paste(w, " Lee");
  assert_eq!(t.find(w, name.clone()).value(), Some("Ann Lee"));
  t.shortcut(w, 'a');
  t.shortcut(w, 'c');
  assert_eq!(t.clipboard().as_deref(), Some("Ann Lee"));
  t.key(w, NamedKey::Tab);
  t.key(w, NamedKey::Enter);
  t.find(w, text("Saved 1 times"));
  t.close(w);
}

fn two_windows_work_at_once(t: &mut TestWindows) {
  let a = t.open("First", (300.0, 200.0), ui! { Counter {} });
  let b = t.open("Second", (300.0, 200.0), ui! { Counter {} });
  t.click(b, label("Increment"));
  t.find(a, text("Count: 0"));
  t.find(b, text("Count: 1"));
  t.close(b);
  assert_eq!(t.windows().len(), 1, "closing one left the other");
  t.close(a);
}

fn a_tooltip_shows_after_a_real_wait(t: &mut TestWindows) {
  let w = t.open("Tooltip", (300.0, 200.0), ui! { Tipped {} });
  t.hover(w, label("Save"));
  t.wait_until("the tooltip", move |t| {
    t.has(w, role(Role::Tooltip).label("Saves the settings"))
  });
  t.close(w);
}

/// True when `a` is within a pixel of `b`, in each direction.
fn near(
  a: (f32, f32),
  b: (f32, f32),
) -> bool {
  (a.0 - b.0).abs() <= 1.0 && (a.1 - b.1).abs() <= 1.0
}

fn size_limits_hold_in_a_real_window(t: &mut TestWindows) {
  let options = WindowOptions::new("Limits")
    .with_size(LogicalSize::new(400.0, 300.0))
    .with_limits(WindowLimits {
      min_size: Some((320.0, 240.0)),
      max_size: Some((500.0, 400.0)),
      resizable: true,
    });
  let w = t.open_with(options, ui! { Counter {} });
  t.set_size(w, (100.0, 100.0));
  t.wait_until("the smallest size", move |t| {
    near(t.place(w).size, (320.0, 240.0))
  });
  t.set_size(w, (900.0, 900.0));
  t.wait_until("the largest size", move |t| {
    near(t.place(w).size, (500.0, 400.0))
  });
  t.close(w);
}

fn a_real_window_is_maximised_and_restored(t: &mut TestWindows) {
  let w = t.open("Maximise", (400.0, 300.0), ui! { Counter {} });
  assert_eq!(t.state(w), WindowState::Normal);
  t.set_state(w, WindowState::Maximised);
  t.wait_until("the window to grow", move |t| {
    let (width, height) = t.place(w).size;
    width > 400.0 && height > 300.0
  });
  t.set_state(w, WindowState::Normal);
  t.wait_until("the window's own size again", move |t| {
    near(t.place(w).size, (400.0, 300.0))
  });
  // It still works.
  t.click(w, label("Increment"));
  t.find(w, text("Count: 1"));
  t.close(w);
}

fn a_real_window_opens_maximised(t: &mut TestWindows) {
  let options =
    WindowOptions::new("Opens maximised").with_state(WindowState::Maximised);
  let w = t.open_with(options, ui! { Counter {} });
  t.wait_until("the window to be maximised", move |t| {
    t.state(w) == WindowState::Maximised
  });
  t.close(w);
}

fn a_real_window_goes_full_screen_and_back(t: &mut TestWindows) {
  let w = t.open("Full screen", (400.0, 300.0), ui! { Counter {} });
  t.set_state(w, WindowState::FullScreen);
  t.wait_until("the window to fill the screen", move |t| {
    let (width, height) = t.place(w).size;
    width > 400.0 && height > 300.0
  });
  t.set_state(w, WindowState::Normal);
  t.wait_until("the window's own size again", move |t| {
    near(t.place(w).size, (400.0, 300.0))
  });
  t.close(w);
}

fn main() -> ExitCode {
  run_window_tests(&[
    (
      "size_limits_hold_in_a_real_window",
      size_limits_hold_in_a_real_window,
    ),
    (
      "a_real_window_is_maximised_and_restored",
      a_real_window_is_maximised_and_restored,
    ),
    (
      "a_real_window_opens_maximised",
      a_real_window_opens_maximised,
    ),
    (
      "a_real_window_goes_full_screen_and_back",
      a_real_window_goes_full_screen_and_back,
    ),
    (
      "a_click_in_a_real_window_changes_the_text",
      a_click_in_a_real_window_changes_the_text,
    ),
    (
      "typing_and_pasting_go_to_the_window",
      typing_and_pasting_go_to_the_window,
    ),
    ("two_windows_work_at_once", two_windows_work_at_once),
    (
      "a_tooltip_shows_after_a_real_wait",
      a_tooltip_shows_after_a_real_wait,
    ),
    (
      "a_real_window_draws_frames_only_while_asked",
      a_real_window_draws_frames_only_while_asked,
    ),
  ])
}
