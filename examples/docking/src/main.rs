//! M10 example: an editor-like window with dockable panels.
//!
//! - `cargo run -p example-docking` opens it. Drag tabs to dock panels
//!   beside others, into other stacks, along the edges, or out of the
//!   window to float them. Drag the lines between panels to resize them.
//!   F6 moves between panels. The layout is saved in the user's settings
//!   folder, or in the file named by `DOVETAIL_DOCK_FILE`, and comes back
//!   the next time.
//! - `cargo run -p example-docking -- --self-test` docks, resizes, floats
//!   and docks again by itself, moves between panels with F6, and checks
//!   the saved layout. It uses a file of its own in the temporary folder.

mod self_test;

use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;

use dovetail::docking::{
  Axis, Dock, DockArea, DockLayout, Node, PanelInfo, Tabs, placeholder,
};
use dovetail::prelude::*;
use dovetail::{Align, Edges, Element, ElementKind, Size, Style};

thread_local! {
  /// The dock, for the self-test.
  static DOCK: Cell<Option<Dock>> = const { Cell::new(None) };
}

/// The panels, in no particular order.
fn panels() -> Vec<PanelInfo> {
  vec![
    PanelInfo::new("files", "Files"),
    PanelInfo::new("layers", "Layers"),
    PanelInfo::new("board", "Board"),
    PanelInfo::new("console", "Console"),
    PanelInfo::new("inspector", "Inspector"),
  ]
}

/// Files and layers on the left, the board above the console in the
/// middle, and the inspector on the right.
fn default_layout() -> DockLayout {
  DockLayout::new(Node::split(
    Axis::Row,
    vec![
      (0.22, Node::Tabs(Tabs::of(&["files", "layers"]))),
      (
        0.53,
        Node::split(
          Axis::Column,
          vec![
            (0.7, Node::Tabs(Tabs::one("board"))),
            (0.3, Node::Tabs(Tabs::one("console"))),
          ],
        ),
      ),
      (0.25, Node::Tabs(Tabs::one("inspector"))),
    ],
  ))
}

/// Where the layout is kept between runs.
fn layout_file() -> Option<PathBuf> {
  if let Some(path) = std::env::var_os("DOVETAIL_DOCK_FILE") {
    return Some(PathBuf::from(path));
  }
  dovetail::desktop::settings_dir("dovetail-example-docking")
    .map(|d| d.join("layout.txt"))
}

/// The application's own state, which the panels show. It lives outside
/// the panels, so it stays when they move.
#[derive(Clone, Copy)]
struct Parts {
  name: Signal<String>,
  value: Signal<String>,
  layers: Signal<[bool; 4]>,
  log: Signal<Vec<String>>,
}

const LAYER_NAMES: [&str; 4] = [
  "Top copper",
  "Bottom copper",
  "Silkscreen",
  "Outline",
];
const FILES: [&str; 3] = ["main.board", "parts.lib", "notes.txt"];

fn column(children: Vec<View>) -> View {
  Element::new(ElementKind::Column)
    .style(Style {
      padding: Edges::all(10.0),
      gap: 8.0,
      width: Size::Fill,
      ..Style::default()
    })
    .children(children)
    .into()
}

fn panel_view(
  id: &str,
  p: Parts,
) -> View {
  let log = move |line: String| {
    p.log.update(|l| {
      l.push(line);
      if l.len() > 6 {
        l.remove(0);
      }
    })
  };
  match id {
    "files" => column(
      FILES
        .iter()
        .map(|f| {
          let f = f.to_string();
          ui! { Button { text: f.clone(), on_click: move |_| log(format!("Opened {f}")) } }
        })
        .collect(),
    ),
    "layers" => column(
      LAYER_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
          let on = p.layers.get()[i];
          ui! {
            Toggle {
              on: on,
              label: *name,
              on_change: move |on| {
                p.layers.update(|l| l[i] = on);
                log(format!("{} {}", LAYER_NAMES[i], if on { "shown" } else { "hidden" }));
              },
            }
          }
        })
        .collect(),
    ),
    "board" => {
      let shown = p.layers.get().iter().filter(|on| **on).count();
      placeholder(&format!("Board: {} on, {shown} layers shown", p.name.get()))
    },
    "inspector" => column(vec![
      ui! { Text { value: "Selected part", size: Some(13.0) } },
      ui! {
        TextField {
          value: p.name.get(),
          label: "Name",
          width: 150.0,
          on_change: move |v| p.name.set(v),
        }
      },
      ui! {
        TextField {
          value: p.value.get(),
          label: "Value",
          width: 150.0,
          on_change: move |v| p.value.set(v),
        }
      },
    ]),
    "console" => column(
      p.log
        .get()
        .into_iter()
        .map(|line| ui! { Text { value: line, size: Some(12.0) } })
        .collect(),
    ),
    _ => placeholder(id),
  }
}

#[component]
fn App(#[prop(default)] file: Option<PathBuf>) -> View {
  let parts = Parts {
    name: use_state(|| "R1".to_string()),
    value: use_state(|| "10k".to_string()),
    layers: use_state(|| [true, true, true, false]),
    log: use_state(|| vec!["Ready".to_string()]),
  };
  let dock = use_state(move || {
    let dock =
      Dock::new(panels(), default_layout(), move |id| panel_view(id, parts));
    if let Some(file) = &file
      && let Err(e) = dock.keep_in_file(file)
    {
      eprintln!(
        "docking: the saved layout could not be read ({e}); using the default"
      );
    }
    dock
  })
  .get();
  DOCK.set(Some(dock));
  let header: View = Element::new(ElementKind::Row)
    .style(Style {
      padding: Edges::xy(10.0, 6.0),
      gap: 10.0,
      align: Align::Center,
      width: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { Text { value: "Drag tabs to move panels. F6 moves between panels.", size: Some(13.0) } },
      Element::new(ElementKind::Box)
        .style(Style { grow: 1.0, ..Style::default() })
        .into(),
      ui! { Button { text: "Reset layout", on_click: move |_| dock.set_layout(default_layout()) } },
    ])
    .into();
  Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      ..Style::default()
    })
    .children([
      header,
      Element::new(ElementKind::Box)
        .style(Style {
          grow: 1.0,
          width: Size::Fill,
          height: Size::Fill,
          align: Align::Stretch,
          ..Style::default()
        })
        .children([ui! { DockArea { dock: dock } }])
        .into(),
    ])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  let file = layout_file();
  if let Some(f) = &file {
    println!("docking: the layout is kept in {}", f.display());
  }
  match dovetail::run("Docking", ui! { App { file: file } }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("docking: {e}");
      ExitCode::FAILURE
    },
  }
}
