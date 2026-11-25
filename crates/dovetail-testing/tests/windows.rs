//! M20 step 8: tests with real windows, run from `cargo test` on the main
//! thread (D0026).
//!
//! `cargo test -p dovetail-testing --features windows --test windows --
//! --ignored`

use std::process::ExitCode;

use dovetail::controls::Tooltip;
use dovetail::prelude::*;
use dovetail_core::{NamedKey, Role};
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

fn main() -> ExitCode {
  run_window_tests(&[
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
  ])
}
