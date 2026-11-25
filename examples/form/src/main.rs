//! A contact form: labelled text fields, a toggle and buttons, with
//! checks on what is typed. It is for trying input and controls together.
//!
//! - `cargo run -p example-form` opens it. Fill it in with the mouse or
//!   the keyboard (Tab moves between the parts), and press Submit.
//! - `cargo run -p example-form -- --self-test` fills it in, submits it,
//!   corrects it and clears it by itself, and exits with a failure code if
//!   the window does not show what it should.

use std::process::ExitCode;

use dovetail::desktop::NamedKey;
use dovetail::prelude::*;
use dovetail::{
  Align, Children, Edges, Element, ElementKind, Role, Size, Style,
};
use dovetail_testing::windows::TestWindows;
use dovetail_testing::{role, text};

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

/// Fills the form in with the keyboard, corrects the email with a click,
/// submits it and clears it, checking what the window says after each. It
/// drives a real window through the window itself (M20, `TestWindows`),
/// and a failed check panics with what the window shows.
fn self_test() -> ExitCode {
  let mut t = TestWindows::new();
  let w = t.open("Form self-test", (800.0, 600.0), ui! { App {} });
  let shows = |t: &mut TestWindows, wanted: &str| {
    t.find(w, text(wanted));
    println!("PASS  the window shows \"{wanted}\"");
  };
  shows(&mut t, "Fill in the form and press Submit.");
  // Tab into the name, fill everything in with the keyboard, turn the
  // toggle on with Space, and press Submit with Enter. The email has no
  // domain ending yet.
  t.type_text(
    w,
    "	Ada Lovelace	ada@example	Hello
World	",
  );
  assert!(t.key(w, NamedKey::Space), "Space turned the toggle on");
  t.type_text(
    w, "	
",
  );
  shows(&mut t, "Check the email address.");
  // Click at the end of the email, finish it, and submit again.
  let email = t
    .find(w, role(Role::TextInput).label("Email"))
    .rect();
  t.click_at(
    w,
    (email.x + email.width - 4.0, email.y + email.height / 2.0),
  );
  t.type_text(
    w, ".com			
",
  );
  shows(
    &mut t,
    "Sent: Ada Lovelace <ada@example.com>, 2 lines, subscribed.",
  );
  // Tab from Submit to Clear, and press it.
  t.type_text(
    w, "	
",
  );
  shows(&mut t, "Fill in the form and press Submit.");
  let mut fields = t.find_all(w, role(Role::TextInput));
  fields.extend(t.find_all(w, role(Role::MultilineTextInput)));
  assert_eq!(fields.len(), 3, "three fields");
  for field in &fields {
    assert_eq!(
      field.value().unwrap_or(""),
      "",
      "{:?} was cleared",
      field.label()
    );
  }
  println!("PASS  Clear emptied every field");
  println!("PASS  typing, Tab, Space, Enter, clicks, checks and clearing work");
  t.close(w);
  ExitCode::SUCCESS
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
