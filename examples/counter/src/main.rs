//! M04 example: the Counter from docs/architecture/syntax.md, in a window.
//!
//! - `cargo run -p example-counter` opens it. Click "Increment".
//! - `cargo run -p example-counter -- --self-test` clicks the button by
//!   itself, checks that the count changes, and exits with a failure code if
//!   it does not (M04 "Done when" item 5).

use std::process::ExitCode;

use dovetail::desktop::NamedKey;
use dovetail::prelude::*;
use dovetail::{Edges, Element, ElementKind, Role, Style};
use dovetail_testing::windows::TestWindows;
use dovetail_testing::{role, text};

/// The example from the syntax document, unchanged.
#[component]
fn Counter() -> View {
  let count = use_state(|| 0);

  ui! {
    Column {
      Text {
        value: count.map(|value| format!("Count: {value}")),
      }

      Button {
        text: "Increment",
        on_click: move |_| count.update(|value| *value += 1),
      }
    }
  }
}

/// The counter with some space around it.
#[component]
fn App() -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(24.0),
      font_size: Some(20.0),
      ..Style::default()
    })
    .children([ui! { Counter {} }])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test();
  }
  match dovetail::run("Counter", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("counter: {e}");
      ExitCode::FAILURE
    },
  }
}

// --- Self-test ---------------------------------------------------------------

/// Clicks "Increment" three times, then presses it with Tab and Enter, and
/// with Space, checking the count after each. It drives a real window
/// through the window itself (M20, `TestWindows`), and a failed check
/// panics with what the window shows.
fn self_test() -> ExitCode {
  let mut t = TestWindows::new();
  let w = t.open("Counter self-test", (400.0, 300.0), ui! { App {} });
  let shows = |t: &mut TestWindows, n: u32| {
    let count = format!("Count: {n}");
    t.find(w, text(count.as_str()));
    println!("PASS  the window shows \"{count}\"");
  };
  shows(&mut t, 0);
  let increment = role(Role::Button).label("Increment");
  for n in 1..=3 {
    t.click(w, increment.clone());
    shows(&mut t, n);
  }
  // Tab focuses the button (the only focusable element), and Enter
  // presses it.
  assert!(t.key(w, NamedKey::Tab), "Tab moved focus");
  assert!(t.key(w, NamedKey::Enter), "Enter pressed the button");
  shows(&mut t, 4);
  assert!(t.key(w, NamedKey::Space), "Space pressed the button");
  shows(&mut t, 5);
  println!(
    "PASS  3 clicks, Tab and Enter, and Space each pressed \"Increment\" once"
  );
  t.close(w);
  ExitCode::SUCCESS
}
