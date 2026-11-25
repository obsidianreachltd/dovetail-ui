//! A contact form: labelled text fields, a toggle and buttons, with
//! checks on what is typed. It is for trying input and controls together.
//!
//! - `cargo run -p example-form` opens it. Fill it in with the mouse or
//!   the keyboard (Tab moves between the parts), and press Submit.
//! - `cargo run -p example-form -- --self-test` fills it in, submits it,
//!   corrects it and clears it by itself, and exits with a failure code if
//!   the window does not show what it should.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, NamedKey, WindowId, WindowOptions,
};
use dovetail::layout::{Content, LayoutBox, first_text};
use dovetail::prelude::*;
use dovetail::{Align, Children, Edges, Element, ElementKind, Size, Style};

/// Grey for hints, red for problems and green for success.
const MUTED: Color = Color::hex(0x6c6f85);
const ERROR: Color = Color::hex(0xd20f39);
const SUCCESS: Color = Color::hex(0x40a02b);

/// How wide the labels and the fields are, in logical pixels.
const LABEL_WIDTH: f32 = 90.0;
const FIELD_WIDTH: f32 = 300.0;

/// The most characters a message may have.
const MESSAGE_LIMIT: usize = 200;

/// A label on the left and its control on the right. With `top`, the label
/// lines up with the top of the control, for tall controls.
#[component]
fn Labelled(
  #[prop(into)] label: String,
  #[prop(default)] top: bool,
  children: Children,
) -> View {
  let label: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(LABEL_WIDTH),
      // Matches the text field's own padding and border, so the label
      // sits level with the text in it.
      padding: Edges {
        top: if top { 6.0 } else { 0.0 },
        ..Edges::default()
      },
      ..Style::default()
    })
    .children([View::text(label)])
    .into();
  Element::new(ElementKind::Row)
    .style(Style {
      gap: 12.0,
      align: if top { Align::Start } else { Align::Center },
      ..Style::default()
    })
    .children(std::iter::once(label).chain(children))
    .into()
}

/// With `DOVETAIL_LOG_INPUT` set, writes each change to a field to stderr,
/// next to the window's input log.
fn log(
  field: &str,
  value: &str,
) {
  if std::env::var_os("DOVETAIL_LOG_INPUT").is_some() {
    eprintln!("form  {field} = {value:?}");
  }
}

/// What the line under the buttons says.
#[derive(Clone, PartialEq)]
enum Status {
  Empty,
  Problem(&'static str),
  Sent(String),
}

/// Checks the form, and says what was sent or what is wrong.
fn check(
  name: &str,
  email: &str,
  message: &str,
  subscribe: bool,
) -> Status {
  let name = name.trim();
  let email = email.trim();
  let (user, domain) = email.split_once('@').unwrap_or(("", ""));
  if name.is_empty() {
    Status::Problem("Enter your name.")
  } else if user.is_empty() || !domain.contains('.') || domain.ends_with('.') {
    Status::Problem("Check the email address.")
  } else if message.chars().count() > MESSAGE_LIMIT {
    Status::Problem("The message is too long.")
  } else {
    let lines = if message.is_empty() {
      0
    } else {
      message.split('\n').count()
    };
    Status::Sent(format!(
      "Sent: {name} <{email}>, {lines} {}{}.",
      if lines == 1 { "line" } else { "lines" },
      if subscribe { ", subscribed" } else { "" },
    ))
  }
}

#[component]
fn ContactForm() -> View {
  let name = use_state(String::new);
  let email = use_state(String::new);
  let message = use_state(String::new);
  let subscribe = use_state(|| false);
  let status = use_state(|| Status::Empty);

  let count = message.map(|m| {
    let lines = m.split('\n').count();
    format!(
      "{} of {MESSAGE_LIMIT} characters, {lines} {}",
      m.chars().count(),
      if lines == 1 { "line" } else { "lines" }
    )
  });
  let (status_text, status_color) = match status.get() {
    Status::Empty => ("Fill in the form and press Submit.".to_owned(), MUTED),
    Status::Problem(p) => (p.to_owned(), ERROR),
    Status::Sent(s) => (s, SUCCESS),
  };

  ui! {
    Column {
      gap: 14.0,
      Text { value: "Contact us", size: Some(22.0) }
      Labelled {
        label: "Name",
        TextField { value: name.get(), on_change: move |v| { log("name", &v); name.set(v) }, width: FIELD_WIDTH, label: "Name" }
      }
      Labelled {
        label: "Email",
        TextField { value: email.get(), on_change: move |v| { log("email", &v); email.set(v) }, width: FIELD_WIDTH, label: "Email" }
      }
      Labelled {
        label: "Message",
        top: true,
        TextField {
          value: message.get(),
          on_change: move |v| {
            log("message", &v);
            message.set(v)
          },
          multiline: true,
          rows: Some(4),
          width: FIELD_WIDTH,
          label: "Message",
        }
      }
      Labelled {
        label: "",
        Text { value: count, size: Some(13.0), color: Some(MUTED) }
      }
      Labelled {
        label: "",
        Toggle {
          on: subscribe.get(),
          on_change: move |on| subscribe.set(on),
          label: "Send me updates",
        }
      }
      Labelled {
        label: "",
        Row {
          Button {
            text: "Submit",
            on_click: move |_| status.set(check(
              &name.get(),
              &email.get(),
              &message.get(),
              subscribe.get(),
            )),
          }
          Button {
            text: "Clear",
            on_click: move |_| {
              name.set(String::new());
              email.set(String::new());
              message.set(String::new());
              subscribe.set(false);
              status.set(Status::Empty);
            },
          }
        }
      }
      Text { value: status_text, color: Some(status_color) }
    }
  }
}

/// The form with some space around it.
#[component]
fn App() -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(24.0),
      ..Style::default()
    })
    .children([ui! { ContactForm {} }])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test();
  }
  match dovetail::run("Form", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("form: {e}");
      ExitCode::FAILURE
    },
  }
}

// --- Self-test ---------------------------------------------------------------

/// What each step waits to see before doing the next thing.
const STEPS: &[&str] = &[
  "Fill in the form and press Submit.",
  "Check the email address.",
  "Sent: Ada Lovelace <ada@example.com>, 2 lines, subscribed.",
  "Fill in the form and press Submit.",
];

struct SelfTest {
  window: Option<WindowId>,
  step: usize,
}

fn find_text<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().find(
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == wanted),
  )
}

/// The text in each field, in order.
fn field_texts(boxes: &[LayoutBox]) -> Vec<String> {
  boxes
    .iter()
    .enumerate()
    .filter(|(_, b)| matches!(&b.content, Content::Element(e) if e.text_cursor.is_some()))
    .filter_map(|(i, _)| match &boxes[first_text(boxes, i)?].content {
      Content::Text { text, .. } => Some(text.to_string()),
      _ => None,
    })
    .collect()
}

/// The fields' boxes, in order.
fn fields(boxes: &[LayoutBox]) -> Vec<&LayoutBox> {
  boxes
    .iter()
    .filter(
      |b| matches!(&b.content, Content::Element(e) if e.text_cursor.is_some()),
    )
    .collect()
}

/// Presses each key in turn; `\n` is Enter and `\t` is Tab.
fn keys(
  cx: &mut Context<'_>,
  window: WindowId,
  text: &str,
) -> bool {
  text.chars().all(|c| {
    let event = match c {
      '\n' => KeyEvent::named(NamedKey::Enter),
      '\t' => KeyEvent::named(NamedKey::Tab),
      ' ' => KeyEvent::character(' '),
      c => KeyEvent::character(c),
    };
    cx.key(window, &event)
  })
}

impl Application for SelfTest {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Form self-test");
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
    let Some(expected) = STEPS.get(self.step) else {
      return;
    };
    let boxes = cx.ui_boxes(window);
    if find_text(boxes, expected).is_none() {
      // An earlier frame; a later one shows the change.
      return;
    }
    println!("PASS  the window shows \"{expected}\"");
    let email = fields(boxes)[1].rect;
    let texts = field_texts(boxes);
    self.step += 1;
    let ok = match self.step {
      1 => {
        // Tab into the name, fill everything in with the keyboard, turn
        // the toggle on with Space, and press Submit with Enter. The email
        // has no domain ending yet.
        keys(cx, window, "\tAda Lovelace\tada@example\tHello\nWorld\t")
          && cx.key(window, &KeyEvent::named(NamedKey::Space))
          && keys(cx, window, "\t\n")
      },
      2 => {
        // Click at the end of the email, finish it, and submit again.
        cx.click(
          window,
          email.x + email.width - 4.0,
          email.y + email.height / 2.0,
        );
        keys(cx, window, ".com\t\t\t\n")
      },
      3 => {
        // Tab from Submit to Clear, and press it.
        keys(cx, window, "\t\n")
      },
      _ => {
        if texts.iter().all(String::is_empty) {
          PASSED.store(true, Ordering::Relaxed);
          println!("PASS  Clear emptied every field");
          println!(
            "PASS  typing, Tab, Space, Enter, clicks, checks and clearing work"
          );
        } else {
          println!("FAIL  the fields still hold {texts:?}");
        }
        cx.close_window(window);
        true
      },
    };
    if !ok {
      println!("FAIL  a key was not used at step {}", self.step);
      cx.exit();
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS.get(self.step).copied().unwrap_or("the end");
    println!("FAIL  timed out waiting for \"{waiting}\"");
    cx.exit();
  }
}

/// Set when every check passed; the application is gone by the time
/// `run` returns.
static PASSED: AtomicBool = AtomicBool::new(false);

fn self_test() -> ExitCode {
  let result = dovetail::desktop::run(SelfTest { window: None, step: 0 });
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("form: {e}");
      ExitCode::FAILURE
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_checks_say_what_is_wrong() {
    assert!(matches!(
      check("", "a@b.c", "", false),
      Status::Problem("Enter your name.")
    ));
    for bad in [
      "",
      "ada",
      "ada@",
      "@example.com",
      "ada@example",
      "ada@example.",
    ] {
      assert!(
        matches!(
          check("Ada", bad, "", false),
          Status::Problem("Check the email address.")
        ),
        "{bad}"
      );
    }
    let long = "x".repeat(MESSAGE_LIMIT + 1);
    assert!(matches!(
      check("Ada", "ada@example.com", &long, false),
      Status::Problem("The message is too long.")
    ));
    assert!(
      check("Ada", "ada@example.com", "Hi", false)
        == Status::Sent("Sent: Ada <ada@example.com>, 1 line.".into())
    );
    assert!(
      check(" Ada ", "ada@example.com", "a\nb", true)
        == Status::Sent(
          "Sent: Ada <ada@example.com>, 2 lines, subscribed.".into()
        )
    );
  }
}
