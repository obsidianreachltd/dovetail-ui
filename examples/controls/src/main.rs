//! M15 example: every everyday control in one window.
//!
//! - `cargo run -p example-controls` opens it. The buttons at the top
//!   choose the light or dark theme. The list on the right scrolls, with a
//!   scrollbar, and right-clicking it (or Shift+F10 on its button) opens a
//!   context menu. The status bar at the bottom shows what changed last.
//! - `cargo run -p example-controls -- --self-test` uses every control
//!   with the pointer and with the keyboard, drags the scrollbar, opens the
//!   context menu, and checks the theme's colours in light and dark.

mod self_test;

use std::cell::Cell;
use std::process::ExitCode;

use dovetail::commands::{CommandScope, Commands, ContextMenu, MenuItem};
use dovetail::controls::{
  CheckBox, Collapsible, Link, NumberField, RadioGroup, Select, Separator,
  Slider, StatusBar, Tabs, Tooltip,
};
use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData, theme};
use dovetail::{Edges, Element, ElementKind, Role, Size, Style};

/// Everything the window shows, kept together so the self-test can read
/// it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct State {
  pub(crate) dark: Signal<bool>,
  pub(crate) wrap: Signal<bool>,
  pub(crate) size: Signal<Option<usize>>,
  pub(crate) fruit: Signal<Option<usize>>,
  pub(crate) copies: Signal<f32>,
  pub(crate) volume: Signal<f32>,
  pub(crate) tab: Signal<usize>,
  pub(crate) more: Signal<bool>,
  pub(crate) status: Signal<String>,
}

thread_local! {
  /// The state, for the self-test.
  pub(crate) static STATE: Cell<Option<State>> = const { Cell::new(None) };
}

pub(crate) const SIZES: [&str; 3] = ["Small", "Medium", "Large"];
pub(crate) const FRUIT: [&str; 4] = ["Apple", "Banana", "Blueberry", "Cherry"];
/// How many rows the scrolling list has.
pub(crate) const ROWS: usize = 40;

fn strings(names: &[&str]) -> Vec<String> {
  names.iter().map(|s| s.to_string()).collect()
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

/// A card `width` wide: a column on a slightly raised surface.
fn card(
  width: f32,
  children: Vec<View>,
) -> View {
  let t = theme();
  Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fixed(width),
      gap: t.spacing.of(3.0),
      padding: Edges::all(t.spacing.of(4.0)),
      background: t.colors.surface_container_low,
      radius: t.shapes.large,
      border: dovetail::Border {
        width: 1.0,
        color: t.colors.outline_variant,
      },
      ..Style::default()
    })
    .children(children)
    .into()
}

/// The controls on the left.
#[component]
fn Controls(state: State) -> View {
  let s = state;
  let say = move |what: String| s.status.set(what);
  let all_on = s.wrap.get() && s.more.get();
  let some_on = s.wrap.get() || s.more.get();
  card(
    440.0,
    vec![
      ui! { Heading { text: "Choices" } },
      ui! {
        CheckBox {
          checked: s.wrap.get(),
          label: "Wrap lines",
          on_change: move |on| { s.wrap.set(on); say(format!("Wrap lines: {on}")); },
        }
      },
      ui! {
        CheckBox {
          checked: all_on,
          mixed: some_on && !all_on,
          label: "Everything",
          on_change: move |on| { s.wrap.set(on); s.more.set(on); },
        }
      },
      ui! { CheckBox { checked: true, enabled: false, label: "Locked" } },
      ui! {
        RadioGroup {
          options: strings(&SIZES),
          selected: s.size.get(),
          label: "Size",
          across: true,
          on_change: move |i| { s.size.set(Some(i)); say(format!("Size: {}", SIZES[i])); },
        }
      },
      ui! {
        Select {
          options: strings(&FRUIT),
          selected: s.fruit.get(),
          label: "Fruit",
          placeholder: "Choose a fruit",
          on_change: move |i| { s.fruit.set(Some(i)); say(format!("Fruit: {}", FRUIT[i])); },
        }
      },
      ui! { Separator {} },
      ui! { Heading { text: "Values" } },
      ui! {
        Row {
          NumberField {
            value: s.copies.get(),
            min: 1.0,
            max: 99.0,
            label: "Copies",
            on_change: move |v| { s.copies.set(v); say(format!("Copies: {v}")); },
          }
          Slider {
            value: s.volume.get(),
            step: 5.0,
            width: 216.0,
            label: "Volume",
            on_change: move |v| { s.volume.set(v); say(format!("Volume: {v}")); },
          }
        }
      },
      ui! {
        Row {
          Tooltip {
            text: "Saves the settings",
            Button { text: "Save", on_click: move |_| say("Saved".into()) }
          }
          Link { text: "Read the help", on_click: move |_| say("Help opened".into()) }
        }
      },
      ui! {
        Collapsible {
          title: "More options",
          open: s.more.get(),
          on_toggle: move |o| s.more.set(o),
          Text { value: "Shown while the section is open." }
        }
      },
    ],
  )
}

/// The tabs and the scrolling list with a context menu, on the right.
#[component]
fn Side(
  state: State,
  commands: Commands,
) -> View {
  let s = state;
  let rows: Vec<View> = (1..=ROWS)
    .map(|i| View::text(format!("Row {i}")))
    .collect();
  let mut inside = vec![ui! {
    Button { text: "First item", on_click: move |_| s.status.set("First item".into()) }
  }];
  inside.extend(rows);
  let list: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fixed(220.0),
      height: Size::Fixed(220.0),
      padding: Edges::all(8.0),
      gap: 4.0,
      clip: true,
      scroll: true,
      border: dovetail::Border {
        width: 1.0,
        color: theme().colors.outline_variant,
      },
      radius: 6.0,
      ..Style::default()
    })
    .role(Role::Group)
    .label("Rows")
    .children(inside)
    .into();
  let items = vec![
    MenuItem::Command("row.copy".into()),
    MenuItem::Separator,
    MenuItem::Command("row.paste".into()),
  ];
  card(
    300.0,
    vec![
      ui! {
        Tabs {
          tabs: strings(&["General", "Advanced"]),
          selected: s.tab.get(),
          on_change: move |i| s.tab.set(i),
          Text { value: if s.tab.get() == 0 { "General settings" } else { "Advanced settings" } }
        }
      },
      ui! { Heading { text: "A list" } },
      ui! {
        ContextMenu {
          commands: commands,
          items: items,
          label: "Row menu",
          { list }
        }
      },
    ],
  )
}

#[component]
fn App() -> View {
  let state = State {
    dark: use_state(|| false),
    wrap: use_state(|| false),
    size: use_state(|| Some(1)),
    fruit: use_state(|| None),
    copies: use_state(|| 1.0),
    volume: use_state(|| 50.0),
    tab: use_state(|| 0),
    more: use_state(|| false),
    status: use_state(|| "Ready".to_string()),
  };
  STATE.set(Some(state));
  let status = state.status;
  let commands = use_state(move || {
    let c = Commands::new()
      .define("row.copy", "Copy", Some("Mod+C"), move || {
        status.set("Copied".into())
      })
      .define("row.paste", "Paste", Some("Mod+V"), || {});
    // Nothing has been copied yet, so Paste is greyed out.
    c.set_enabled("row.paste", false);
    c
  })
  .get();
  let data = if state.dark.get() {
    ThemeData::dark()
  } else {
    ThemeData::light()
  };
  let chooser = ui! {
    Row {
      Button { text: "Light", on_click: move |_| state.dark.set(false) }
      Button { text: "Dark", on_click: move |_| state.dark.set(true) }
    }
  };
  let body: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: 16.0,
      align: dovetail::Align::Start,
      ..Style::default()
    })
    .children([
      ui! { Controls { state: state } },
      ui! { Side { state: state, commands: commands } },
    ])
    .into();
  let content: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 16.0,
      padding: Edges::all(20.0),
      width: Size::Fill,
      grow: 1.0,
      ..Style::default()
    })
    .children([chooser, body])
    .into();
  let status_text = state.status.get();
  let window: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      ..Style::default()
    })
    .children([
      content,
      ui! { StatusBar { Text { value: status_text } } },
    ])
    .into();
  ui! {
    Theme {
      data: data,
      CommandScope { commands: commands, { window } }
    }
  }
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  match dovetail::run("Controls", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("controls: {e}");
      ExitCode::FAILURE
    },
  }
}
