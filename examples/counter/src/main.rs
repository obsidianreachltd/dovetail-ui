//! M04 example: the Counter from docs/architecture/syntax.md, in a window.
//!
//! - `cargo run -p example-counter` opens it. Click "Increment".
//! - `cargo run -p example-counter -- --self-test` clicks the button by
//!   itself, checks that the count changes, and exits with a failure code if
//!   it does not (M04 "Done when" item 5).

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, NamedKey, WindowId, WindowOptions,
};
use dovetail::layout::{Content, LayoutBox};
use dovetail::prelude::*;
use dovetail::{Edges, Element, ElementKind, Style};

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

/// Presses of "Increment": three clicks, then Tab and Enter, then Space.
const PRESSES: u32 = 5;

struct SelfTest {
  window: Option<WindowId>,
  clicks: u32,
}

fn find_text<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().find(
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == wanted),
  )
}

impl Application for SelfTest {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Counter self-test");
    match cx.open_ui_window(options, ui! { App {} }) {
      Ok(id) => self.window = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(15));
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: WindowId,
    _: &mut Frame,
  ) {
  }

  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    let boxes = cx.ui_boxes(window);
    let expected = format!("Count: {}", self.clicks);
    if find_text(boxes, &expected).is_none() {
      // An earlier frame; the next one shows the new count.
      return;
    }
    println!("PASS  the window shows \"{expected}\"");
    if self.clicks == PRESSES {
      PASSED.store(true, Ordering::Relaxed);
      println!(
        "PASS  3 clicks, Tab and Enter, and Space each pressed \"Increment\" once"
      );
      cx.close_window(window);
      return;
    }
    let button = find_text(boxes, "Increment")
      .expect("the button's text")
      .rect;
    let (x, y) = (
      button.x + button.width / 2.0,
      button.y + button.height / 2.0,
    );
    self.clicks += 1;
    match self.clicks {
      1..=3 => cx.click(window, x, y),
      4 => {
        // Tab focuses the button (the only focusable element), and Enter
        // presses it.
        let tabbed = cx.key(window, &KeyEvent::named(NamedKey::Tab));
        let entered = cx.key(window, &KeyEvent::named(NamedKey::Enter));
        if !(tabbed && entered) {
          println!("FAIL  Tab used: {tabbed}, Enter used: {entered}");
          cx.exit();
        }
      },
      _ => {
        if !cx.key(window, &KeyEvent::named(NamedKey::Space)) {
          println!("FAIL  Space did not press the focused button");
          cx.exit();
        }
      },
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    println!("FAIL  timed out waiting for \"Count: {}\"", self.clicks);
    cx.exit();
  }
}

/// Set when every check passed; the application is gone by the time
/// `run` returns.
static PASSED: AtomicBool = AtomicBool::new(false);

fn self_test() -> ExitCode {
  let result = dovetail::desktop::run(SelfTest { window: None, clicks: 0 });
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("counter: {e}");
      ExitCode::FAILURE
    },
  }
}
