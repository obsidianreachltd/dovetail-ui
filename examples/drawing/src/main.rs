//! M25 example: a small drawing tool.
//!
//! - `cargo run -p example-drawing` opens it. Choose a tool from the
//!   palette on the left, then drag on the canvas to draw. Each tool has a
//!   letter key, shown in its tooltip: V, R, L, P and H. Shift with the
//!   letter goes through the tools behind one button, and a long press, a
//!   right-click or Alt+Down opens them. The View menu and the toolbar turn
//!   the grid, snapping and the dark theme on and off, with ticks and
//!   pressed buttons. The palette is a dock panel: drag its tab out to make
//!   it float, and it changes from a column to a row as its panel changes
//!   shape. A narrow window puts toolbar buttons behind a More button.
//! - `cargo run -p example-drawing -- --self-test` does all of that in a
//!   real window, and checks each step.

mod self_test;

use std::cell::Cell;
use std::process::ExitCode;

use dovetail::commands::{
  CommandButton, CommandScope, Commands, History, Menu, MenuBar, ToolPalette,
  Toolbar,
};
use dovetail::controls::{ColourButton, StatusBar, TextField};
use dovetail::docking::{
  Axis, Dock, DockArea, DockLayout, Node, PanelInfo, Tabs,
};
use dovetail::image::icons;
use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData, theme};
use dovetail::{
  Align, Border, Edges, Element, ElementKind, PointerHandler, PointerKind,
  Role, Size, Style,
};

/// The space between grid lines, and what snapping rounds to.
pub(crate) const GRID: f32 = 20.0;

/// What a tool draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  Rectangle,
  Ellipse,
  Line,
  Pen,
  Brush,
}

/// A shape on the canvas.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shape {
  pub(crate) kind: Kind,
  pub(crate) from: (f32, f32),
  pub(crate) to: (f32, f32),
  /// The points a pen or brush went through.
  pub(crate) points: Vec<(f32, f32)>,
  pub(crate) colour: Color,
}

/// The application's state, kept together so the self-test can read it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct State {
  pub(crate) commands: Commands,
  pub(crate) shapes: Signal<Vec<Shape>>,
  pub(crate) title: Signal<String>,
  pub(crate) fill: Signal<Color>,
  pub(crate) grid: Signal<bool>,
  pub(crate) snap: Signal<bool>,
  pub(crate) dark: Signal<bool>,
}

thread_local! {
  /// The state, for the self-test.
  pub(crate) static STATE: Cell<Option<State>> = const { Cell::new(None) };
  /// The dock, for the self-test.
  pub(crate) static DOCK: Cell<Option<Dock>> = const { Cell::new(None) };
}

/// The tools, in the palette's slots. A slot with several is a group.
pub(crate) fn tool_slots() -> Vec<Vec<String>> {
  [
    &["tool.select"][..],
    &["tool.rectangle", "tool.ellipse"],
    &["tool.line"],
    &["tool.pen", "tool.brush"],
    &["tool.hand"],
  ]
  .iter()
  .map(|slot| slot.iter().map(|s| s.to_string()).collect())
  .collect()
}

/// The commands: the tools, the view's toggles, and editing.
fn define(
  history: History,
  shapes: Signal<Vec<Shape>>,
  grid: Signal<bool>,
  snap: Signal<bool>,
  dark: Signal<bool>,
) -> Commands {
  let tools = [
    ("tool.select", "Select", Some("V"), icons::pointer()),
    ("tool.rectangle", "Rectangle", Some("R"), icons::rectangle()),
    ("tool.ellipse", "Ellipse", None, icons::ellipse()),
    ("tool.line", "Line", Some("L"), icons::line()),
    ("tool.pen", "Pen", Some("P"), icons::pen()),
    ("tool.brush", "Brush", None, icons::brush()),
    ("tool.hand", "Hand", Some("H"), icons::hand()),
  ];
  let mut c = Commands::new();
  for (id, name, key, icon) in tools {
    c = c
      .define_choice(id, "tools", name, key, || {})
      .with_icon(id, icon);
  }
  let c = c
    .define_toggle("view.grid", "Grid", Some("Mod+G"), move |on| grid.set(on))
    .on_when("view.grid", move || grid.get())
    .with_icon("view.grid", icons::grid())
    .define_toggle(
      "view.snap",
      "Snap to grid",
      Some("Mod+Shift+G"),
      move |on| snap.set(on),
    )
    .on_when("view.snap", move || snap.get())
    .with_icon("view.snap", icons::magnet())
    .define_toggle("view.dark", "Dark theme", None, move |on| dark.set(on))
    .on_when("view.dark", move || dark.get())
    .define("edit.clear", "Clear", None, move || {
      let before = shapes.get();
      if before.is_empty() {
        return;
      }
      history.apply(
        "Clear",
        move || shapes.set(Vec::new()),
        move || shapes.set(before.clone()),
      );
    })
    .enabled_when("edit.clear", move || !shapes.get().is_empty())
    .with_icon("edit.clear", icons::eraser());
  history
    .add_commands(c)
    .with_icon("edit.undo", icons::undo())
    .with_icon("edit.redo", icons::redo())
}

/// The tool chosen now.
fn tool(commands: Commands) -> Option<Kind> {
  match commands.chosen("tools").as_deref() {
    Some("tool.rectangle") => Some(Kind::Rectangle),
    Some("tool.ellipse") => Some(Kind::Ellipse),
    Some("tool.line") => Some(Kind::Line),
    Some("tool.pen") => Some(Kind::Pen),
    Some("tool.brush") => Some(Kind::Brush),
    _ => None,
  }
}

/// The points along a line, close enough to draw it as dots.
fn along(
  from: (f32, f32),
  to: (f32, f32),
) -> Vec<(f32, f32)> {
  let steps = ((to.0 - from.0).hypot(to.1 - from.1) / 2.0)
    .ceil()
    .max(1.0) as usize;
  (0..=steps)
    .map(|i| {
      let t = i as f32 / steps as f32;
      (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
    })
    .collect()
}

/// A shape as boxes, in the canvas's stack.
fn draw(shape: &Shape) -> Vec<View> {
  let (x0, y0) = (shape.from.0.min(shape.to.0), shape.from.1.min(shape.to.1));
  let (w, h) = (
    (shape.to.0 - shape.from.0).abs(),
    (shape.to.1 - shape.from.1).abs(),
  );
  let dot = |(x, y): (f32, f32), size: f32| -> View {
    Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(size),
        height: Size::Fixed(size),
        radius: size / 2.0,
        background: shape.colour,
        offset: (x - size / 2.0, y - size / 2.0),
        ..Style::default()
      })
      .into()
  };
  match shape.kind {
    Kind::Rectangle | Kind::Ellipse => vec![
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(w.max(1.0)),
          height: Size::Fixed(h.max(1.0)),
          radius: if shape.kind == Kind::Ellipse {
            w.min(h) / 2.0
          } else {
            2.0
          },
          background: Color { a: 0.3, ..shape.colour },
          border: Border { width: 2.0, color: shape.colour },
          offset: (x0, y0),
          ..Style::default()
        })
        .into(),
    ],
    Kind::Line => along(shape.from, shape.to)
      .into_iter()
      .map(|p| dot(p, 3.0))
      .collect(),
    Kind::Pen => shape
      .points
      .iter()
      .map(|&p| dot(p, 3.0))
      .collect(),
    Kind::Brush => shape
      .points
      .iter()
      .map(|&p| dot(p, 10.0))
      .collect(),
  }
}

/// The canvas: the shapes, the grid when it is on, and the shape being
/// drawn.
#[component]
fn Canvas(
  state: State,
  history: History,
) -> View {
  let s = state;
  let t = theme();
  let drawing = use_state(|| None::<Shape>);
  let size = use_state(|| (0.0f32, 0.0f32));
  let snap = move |(x, y): (f32, f32)| {
    if s.snap.get() {
      ((x / GRID).round() * GRID, (y / GRID).round() * GRID)
    } else {
      (x, y)
    }
  };
  let pointer = PointerHandler::new(move |e| {
    let at = (e.x, e.y);
    match e.kind {
      PointerKind::Down => {
        if let Some(kind) = tool(s.commands) {
          let start = snap(at);
          drawing.set(Some(Shape {
            kind,
            from: start,
            to: start,
            points: vec![at],
            colour: s.fill.get(),
          }));
        }
      },
      PointerKind::Move => {
        drawing.update(|d| {
          if let Some(d) = d {
            d.to = snap(at);
            d.points.push(at);
          }
        });
      },
      PointerKind::Up => {
        if let Some(shape) = drawing.get() {
          drawing.set(None);
          let name = format!("Draw {:?}", shape.kind);
          history.apply(
            name,
            move || s.shapes.update(|v| v.push(shape.clone())),
            move || {
              s.shapes.update(|v| {
                v.pop();
              })
            },
          );
        }
      },
    }
  });
  let mut children: Vec<View> = Vec::new();
  if s.grid.get() {
    let (w, h) = size.get();
    let line = |x: f32, y: f32, across: bool| -> View {
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(if across { w } else { 1.0 }),
          height: Size::Fixed(if across { 1.0 } else { h }),
          background: t.colors.outline_variant,
          offset: (x, y),
          ..Style::default()
        })
        .into()
    };
    let mut x = GRID;
    while x < w {
      children.push(line(x, 0.0, false));
      x += GRID;
    }
    let mut y = GRID;
    while y < h {
      children.push(line(0.0, y, true));
      y += GRID;
    }
  }
  for shape in s.shapes.get().iter() {
    children.extend(draw(shape));
  }
  if let Some(shape) = drawing.get() {
    children.extend(draw(&shape));
  }
  Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      clip: true,
      background: t.colors.surface,
      ..Style::default()
    })
    .role(Role::Canvas)
    .label("Drawing")
    .on_pointer(pointer)
    .on_size(Callback::new(move |wh: (f32, f32)| {
      if size.get() != wh {
        size.set(wh);
      }
    }))
    .children(children)
    .into()
}

/// What each dock panel shows.
fn panel(
  id: &str,
  state: State,
  history: History,
) -> View {
  match id {
    "tools" => ui! {
      ToolPalette { commands: state.commands, tools: tool_slots(), label: "Tools" }
    },
    _ => ui! { Canvas { state: state, history: history } },
  }
}

#[component]
pub(crate) fn App() -> View {
  let history = use_state(History::new).get();
  let shapes = use_state(Vec::<Shape>::new);
  let grid = use_state(|| false);
  let snap = use_state(|| false);
  let dark = use_state(|| false);
  let commands =
    use_state(move || define(history, shapes, grid, snap, dark)).get();
  let state = State {
    commands,
    shapes,
    title: use_state(|| "Untitled".to_string()),
    fill: use_state(|| theme().colors.primary),
    grid,
    snap,
    dark,
  };
  STATE.set(Some(state));
  let dock = use_state(move || {
    Dock::new(
      vec![
        PanelInfo::new("tools", "Tools"),
        PanelInfo::new("canvas", "Canvas"),
      ],
      DockLayout::new(Node::split(
        Axis::Row,
        vec![
          (0.12, Node::Tabs(Tabs::one("tools"))),
          (0.88, Node::Tabs(Tabs::one("canvas"))),
        ],
      )),
      move |id| panel(id, state, history),
    )
  })
  .get();
  DOCK.set(Some(dock));
  let mut tools = Menu::new("Tools");
  for slot in tool_slots() {
    for id in slot {
      tools = tools.command(id);
    }
  }
  let menus = vec![
    Menu::new("Edit")
      .command("edit.undo")
      .command("edit.redo")
      .separator()
      .command("edit.clear"),
    Menu::new("View")
      .command("view.grid")
      .command("view.snap")
      .separator()
      .command("view.dark"),
    tools,
  ];
  let t = theme();
  let toolbar = ui! {
    Toolbar {
      overflow: true,
      CommandButton { commands: commands, id: "edit.undo", icon_only: true }
      CommandButton { commands: commands, id: "edit.redo", icon_only: true }
      CommandButton { commands: commands, id: "edit.clear", icon_only: true }
      CommandButton { commands: commands, id: "view.grid", icon_only: true }
      CommandButton { commands: commands, id: "view.snap", icon_only: true }
      ColourButton {
        value: state.fill.get(),
        label: "Fill",
        swatches: vec![t.colors.primary, t.colors.error, t.colors.on_surface],
        on_change: move |c| state.fill.set(c),
      }
      TextField {
        value: state.title.get(),
        width: 160.0,
        label: "Title",
        on_change: move |v| state.title.set(v),
      }
    }
  };
  let tool_name = commands
    .chosen("tools")
    .and_then(|id| commands.info(&id))
    .map_or_else(|| "none".to_string(), |i| i.name);
  let count = shapes.get().len();
  let status = format!(
    "{} · Tool: {tool_name} · {count} shape{}",
    state.title.get(),
    if count == 1 { "" } else { "s" }
  );
  let window: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      ..Style::default()
    })
    .children([
      ui! { MenuBar { commands: commands, menus: menus } },
      toolbar,
      Element::new(ElementKind::Box)
        .style(Style {
          grow: 1.0,
          width: Size::Fill,
          height: Size::Fill,
          align: Align::Stretch,
          padding: Edges::default(),
          ..Style::default()
        })
        .children([ui! { DockArea { dock: dock } }])
        .into(),
      ui! { StatusBar { Text { value: status } } },
    ])
    .into();
  let data = if dark.get() {
    ThemeData::dark()
  } else {
    ThemeData::light()
  };
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
  match dovetail::run("Drawing", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("drawing: {e}");
      ExitCode::FAILURE
    },
  }
}
