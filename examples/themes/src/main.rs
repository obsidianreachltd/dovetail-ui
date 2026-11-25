//! M14 example: one window shown in the light, dark, a custom and the
//! system's theme.
//!
//! - `cargo run -p example-themes` opens it. The buttons at the top choose
//!   the theme. "System" follows the system's light or dark mode, and its
//!   high-contrast setting, while the example runs. One section overrides
//!   part of the theme, and a badge uses a theme extension.
//! - `cargo run -p example-themes -- --self-test` chooses each theme,
//!   checks the colours of the window and its controls, checks that the
//!   override changes only its own section, and switches the system mode
//!   from inside the program.

mod self_test;

use std::cell::Cell;
use std::process::ExitCode;

use dovetail::prelude::*;
use dovetail::theme::{Brightness, Theme, ThemeData, ThemeMode, theme};
use dovetail::{Edges, Element, ElementKind, Role, Size, Style};

/// The themes the window can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
  Light,
  Dark,
  Custom,
  System,
}

/// An application's own value in its theme: a badge's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Badge {
  pub(crate) background: Color,
  pub(crate) text: Color,
}

/// The custom theme's seed: a green.
pub(crate) const CUSTOM_SEED: Color = Color::hex(0x2e8b57);

/// The theme for each choice: its data, its dark data, and its mode.
pub(crate) fn themes(
  choice: Choice
) -> (ThemeData, Option<ThemeData>, ThemeMode) {
  let badge = |t: &ThemeData| Badge {
    background: t.colors.secondary_container,
    text: t.colors.on_secondary_container,
  };
  let with_badge = |t: ThemeData| {
    let b = badge(&t);
    t.with_extension(b)
  };
  match choice {
    Choice::Light => (with_badge(ThemeData::light()), None, ThemeMode::Light),
    Choice::Dark => (with_badge(ThemeData::dark()), None, ThemeMode::Light),
    Choice::Custom => {
      let mut t = ThemeData::from_seed(CUSTOM_SEED, Brightness::Light);
      // Roomier, rounder controls.
      t.shapes.medium = 12.0;
      t.spacing.density = 1.25;
      (with_badge(t), None, ThemeMode::Light)
    },
    Choice::System => (
      with_badge(ThemeData::light()),
      Some(with_badge(ThemeData::dark())),
      ThemeMode::System,
    ),
  }
}

thread_local! {
  /// The choice, for the self-test.
  pub(crate) static CHOICE: Cell<Option<Signal<Choice>>> = const { Cell::new(None) };
}

/// A small label in the theme extension's colours.
#[component]
fn BadgeView(#[prop(into)] text: String) -> View {
  let t = theme();
  let b = t
    .extensions
    .get::<Badge>()
    .copied()
    .unwrap_or(Badge {
      background: t.colors.secondary_container,
      text: t.colors.on_secondary_container,
    });
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::xy(8.0, 2.0),
      radius: 10.0,
      background: b.background,
      text_color: Some(b.text),
      font_size: Some(t.text.body_small.size),
      ..Style::default()
    })
    .role(Role::Label)
    .label("Badge")
    .children([View::text(text)])
    .into()
}

/// A heading in the theme's title style.
#[component]
fn Heading(#[prop(into)] text: String) -> View {
  let t = theme();
  Element::new(ElementKind::Box)
    .style(Style {
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      ..Style::default()
    })
    .role(Role::Heading)
    .children([View::text(text)])
    .into()
}

/// A section that overrides the theme's primary colour with its error
/// colour, for buttons that delete things.
#[component]
fn Danger() -> View {
  let mut data = theme();
  data.colors.primary = data.colors.error;
  data.colors.on_primary = data.colors.on_error;
  ui! {
    Theme {
      data: data,
      Row {
        Button { text: "Delete everything" }
        Text { value: "This section overrides the primary colour." }
      }
    }
  }
}

#[component]
fn Sample() -> View {
  let on = use_state(|| true);
  let name = use_state(|| "Dovetail".to_string());
  let t = theme();
  let card: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(3.0),
      padding: Edges::all(t.spacing.of(4.0)),
      background: t.colors.surface_container_low,
      radius: t.shapes.large,
      border: dovetail::Border { width: 1.0, color: t.colors.outline_variant },
      width: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { Heading { text: "Controls" } },
      ui! { Text { value: "Body text, in the theme's body style." } },
      ui! {
        Row {
          Button { text: "Primary" }
          Button { text: "Disabled", enabled: false }
          BadgeView { text: "Extension" }
        }
      },
      ui! { Toggle { on: on.get(), label: "A switch", on_change: move |v| on.set(v) } },
      ui! { TextField { value: name.get(), label: "Name", on_change: move |v| name.set(v) } },
      ui! { dovetail::controls::ProgressBar { value: Some(0.6), label: "Progress" } },
      ui! { Danger {} },
    ])
    .into();
  card
}

#[component]
fn Chooser(choice: Signal<Choice>) -> View {
  let button = |label: &'static str, c: Choice| {
    let chosen = choice.get() == c;
    ui! {
      Button {
        text: if chosen { format!("{label} \u{2713}") } else { label.to_string() },
        on_click: move |_| choice.set(c),
      }
    }
  };
  Element::new(ElementKind::Row)
    .style(Style { gap: 8.0, ..Style::default() })
    .role(Role::Toolbar)
    .label("Themes")
    .children([
      button("Light", Choice::Light),
      button("Dark", Choice::Dark),
      button("Custom", Choice::Custom),
      button("System", Choice::System),
    ])
    .into()
}

#[component]
fn App() -> View {
  let choice = use_state(|| Choice::Light);
  CHOICE.set(Some(choice));
  let (data, dark, mode) = themes(choice.get());
  let content: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 16.0,
      padding: Edges::all(20.0),
      width: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { Chooser { choice: choice } },
      ui! { Sample {} },
    ])
    .into();
  ui! {
    Theme {
      data: data,
      dark: dark,
      mode: mode,
      { content }
    }
  }
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  match dovetail::run("Themes", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("themes: {e}");
      ExitCode::FAILURE
    },
  }
}
