//! The dock on screen: [`Dock`] holds the state, [`DockArea`] shows the
//! main window's panels, and each floating stack gets a window of its own,
//! a [`Window`](dovetail::Window) that the dock area shows (M19).

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use dovetail::{
  Align, Callback, Color, DragData, DragEnd, DropEvent, DropHandler, DropKind,
  Edges, Element, ElementKind, FocusTarget, KeyEvent, KeyHandler, NamedKey,
  PointerButton, PointerHandler, PointerKind, Role, SavedWindow, Signal, Size,
  Style, View, Window, WindowLimits, WindowPlace, WindowState, create_signal,
  start_drag, use_state,
};
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

use crate::model::{Axis, DockLayout, Node, Place, Side, StackAt};
use crate::text::ReadError;

/// The tab bar's background.
/// Colours from the nearest theme (M14), named for what they are here.
/// Tab bars.
fn bar_colour() -> Color {
  theme().tab_look().0
}
/// Tabs that are not chosen, and panel headers.
fn tab() -> Color {
  theme().tab_look().1
}
/// The chosen tab.
fn shown_tab() -> Color {
  theme().colors.surface
}
/// Lines between panels.
fn line() -> Color {
  theme().tab_look().2
}
/// Where a dragged panel will go.
fn preview_colour() -> Color {
  Color { a: 0.28, ..theme().colors.primary }
}
/// Markers and lit drop zones.
fn accent() -> Color {
  theme().colors.primary
}
/// Panel text.
fn ink() -> Color {
  theme().colors.on_surface
}
/// How thick splitters and the dock's edges are, in logical pixels.
const SPLITTER: f32 = 6.0;
/// The least a part of a split shrinks to, in logical pixels.
const MIN_PART: f32 = 48.0;
/// How far a tab moves with the button held before it is dragged.
const DRAG_START: f32 = 6.0;
/// A new floating window's size, when nothing better is known.
const FLOAT_SIZE: (f32, f32) = (360.0, 300.0);

/// A panel the dock can show: its id, used in the saved layout, and the
/// title on its tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelInfo {
  /// Names the panel in the layout. No spaces.
  pub id: String,
  /// Shown on its tab.
  pub title: String,
}

impl PanelInfo {
  /// A panel called `id`, with the title `title`.
  pub fn new(
    id: impl Into<String>,
    title: impl Into<String>,
  ) -> PanelInfo {
    PanelInfo { id: id.into(), title: title.into() }
  }
}

/// What a dragged tab carries. Other drop targets in the application can
/// take it too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraggedPanel(pub String);

/// What a drop would do, shown while a tab is dragged.
#[derive(Clone, Debug, PartialEq)]
enum Preview {
  /// Over a stack's content: into it (`None`) or beside it.
  Zone(StackAt, Option<Side>),
  /// Into a stack's tab bar, before this tab.
  Tab(StackAt, usize),
  /// Along an edge of the dock.
  Edge(Side),
}

/// What the views show.
struct Shown {
  layout: DockLayout,
  /// The preview, and the drop target that showed it.
  preview: Option<(String, Preview)>,
  panels: Rc<[PanelInfo]>,
  content: Rc<dyn Fn(&str) -> View>,
}

/// What the views do not show, so changing it builds nothing again.
#[derive(Default)]
struct Quiet {
  /// Where floating windows are now.
  places: HashMap<u64, WindowPlace>,
  /// The file the layout is kept in, and what it holds.
  file: Option<(PathBuf, String)>,
}

/// The docking state: the layout, the panels, and what they show. Make it
/// once, while a component builds:
///
/// ```ignore
/// let dock = use_state(|| Dock::new(panels, default_layout, show_panel)).get();
/// ```
///
/// A panel's content is built again whenever it moves to another stack or
/// window, so keep its state outside it, in the application's signals.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Dock {
  shown: Signal<Shown>,
  quiet: Signal<Quiet>,
}

impl fmt::Debug for Dock {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("Dock")
  }
}

/// Where focus goes for a panel's content.
fn panel_target(panel: &str) -> FocusTarget {
  FocusTarget::named(&format!("dovetail-dock-panel:{panel}"))
}

/// Where focus goes for a panel's tab.
fn tab_target(panel: &str) -> FocusTarget {
  FocusTarget::named(&format!("dovetail-dock-tab:{panel}"))
}

/// The id a drop target uses for the previews it shows.
fn owner(
  kind: &str,
  at: &StackAt,
) -> String {
  format!("{kind}:{at:?}")
}

impl Dock {
  /// A dock with these panels, laid out as `layout`. `content` builds a
  /// panel's content from its id. Panels in `layout` that are not in
  /// `panels` are left out, and panels missing from it are added.
  ///
  /// # Panics
  ///
  /// If no component is building.
  pub fn new(
    panels: Vec<PanelInfo>,
    mut layout: DockLayout,
    content: impl Fn(&str) -> View + 'static,
  ) -> Dock {
    let known: Vec<&str> = panels.iter().map(|p| p.id.as_str()).collect();
    layout.keep_known(&known);
    let dock = Dock {
      shown: create_signal(Shown {
        layout,
        preview: None,
        panels: panels.into(),
        content: Rc::new(content),
      }),
      quiet: create_signal(Quiet::default()),
    };
    dock.sync_windows();
    dock
  }

  /// Reads the layout kept in `path`, if the file is there and can be
  /// read, and from then on writes the layout there whenever it changes.
  /// A file that cannot be read is left as it is until the layout next
  /// changes, and the error is returned.
  pub fn keep_in_file(
    self,
    path: impl Into<PathBuf>,
  ) -> Result<(), ReadError> {
    let path = path.into();
    let mut result = Ok(());
    let mut written = String::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
      match DockLayout::from_text(&text) {
        Ok(layout) => {
          written = text;
          self.set_layout(layout);
        },
        Err(e) => result = Err(e),
      }
    }
    self
      .quiet
      .update(|q| q.file = Some((path, written)));
    self.save();
    result
  }

  /// The layout now, with floating windows where they are.
  pub fn layout(self) -> DockLayout {
    let mut layout = self.shown.map(|s| s.layout.clone());
    self.quiet.map(|q| {
      for (id, place) in &q.places {
        layout.set_place(*id, *place);
      }
    });
    layout
  }

  /// Replaces the layout. Unknown panels are left out, and missing ones
  /// added.
  pub fn set_layout(
    self,
    layout: DockLayout,
  ) {
    self.change(|l| *l = layout);
  }

  /// The title of `panel`, or its id if it is not known.
  pub fn title(
    self,
    panel: &str,
  ) -> String {
    self.shown.map(|s| {
      s.panels
        .iter()
        .find(|p| p.id == panel)
        .map_or_else(|| panel.to_string(), |p| p.title.clone())
    })
  }

  /// Moves `panel` to `place`.
  pub fn move_panel(
    self,
    panel: &str,
    place: Place,
  ) {
    self.change(|l| l.move_panel(panel, place));
  }

  /// Shows `panel` in its stack.
  pub fn activate(
    self,
    panel: &str,
  ) {
    self.change(|l| l.activate(panel));
  }

  /// Puts floating stack `id`'s panels back in the main window.
  pub fn dock_floating(
    self,
    id: u64,
  ) {
    self.change(|l| l.dock_floating(id));
  }

  /// Sets the shares of the split at `path`.
  pub fn set_shares(
    self,
    path: &[usize],
    shares: &[f32],
  ) {
    self.change(|l| l.set_shares(path, shares));
  }

  /// Moves keyboard focus into `panel`'s content, showing it first.
  pub fn focus_panel(
    self,
    panel: &str,
  ) {
    self.activate(panel);
    panel_target(panel).focus();
  }

  /// Moves focus to the next panel shown after the one holding `from`, in
  /// the order of [`DockLayout::stacks`], or to the first with no `from`.
  /// `back` goes the other way. This is what F6 and Shift+F6 do.
  pub fn focus_next_panel(
    self,
    from: Option<&str>,
    back: bool,
  ) {
    let stacks = self.shown.map(|s| s.layout.stacks());
    if stacks.is_empty() {
      return;
    }
    let at = from.and_then(|f| {
      stacks
        .iter()
        .position(|(_, t)| t.panels.iter().any(|p| p == f))
    });
    let n = stacks.len();
    let next = match (at, back) {
      (None, false) => 0,
      (None, true) => n - 1,
      (Some(i), false) => (i + 1) % n,
      (Some(i), true) => (i + n - 1) % n,
    };
    panel_target(stacks[next].1.active_panel()).focus();
  }

  /// The tab after (or before, with `back`) `panel` in its stack, going
  /// round at the end.
  fn neighbour_tab(
    self,
    panel: &str,
    back: bool,
  ) -> Option<String> {
    let layout = self.shown.map(|s| s.layout.clone());
    let (at, i) = layout.find(panel)?;
    let t = layout.tabs(&at)?;
    let n = t.panels.len();
    let j = if back { (i + n - 1) % n } else { (i + 1) % n };
    Some(t.panels[j].clone())
  }

  /// Changes the layout, then opens and closes floating windows to match
  /// it, and saves it.
  fn change(
    self,
    f: impl FnOnce(&mut DockLayout),
  ) {
    let places = self.quiet.map(|q| q.places.clone());
    self.shown.update(|s| {
      for (id, place) in places {
        s.layout.set_place(id, place);
      }
      f(&mut s.layout);
      let known: Vec<String> = s.panels.iter().map(|p| p.id.clone()).collect();
      let known: Vec<&str> = known.iter().map(String::as_str).collect();
      s.layout.keep_known(&known);
      s.preview = None;
    });
    self.sync_windows();
    self.save();
  }

  /// A floating window moved or changed size.
  fn set_place(
    self,
    id: u64,
    place: WindowPlace,
  ) {
    self.quiet.update(|q| {
      q.places.insert(id, place);
    });
    self.save();
  }

  /// Writes the layout to its file, if it has one and the text changed.
  fn save(self) {
    let Some((path, written)) = self.quiet.map(|q| q.file.clone()) else {
      return;
    };
    let text = self.layout().to_text();
    if text == written {
      return;
    }
    if let Some(dir) = path.parent() {
      let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, &text) {
      Ok(()) => self.quiet.update(|q| q.file = Some((path, text))),
      Err(e) => eprintln!(
        "dovetail: could not save the docking layout to {}: {e}",
        path.display()
      ),
    }
  }

  /// Forgets the places of floating stacks that are gone. Their windows
  /// are `Window`s the dock area shows, so they close by themselves (M19).
  fn sync_windows(self) {
    let floating = self.shown.map(|s| s.layout.floating.clone());
    self.quiet.update(|q| {
      q.places
        .retain(|id, _| floating.iter().any(|f| f.id == *id));
    });
  }

  /// A window for each floating stack, opened where the layout says,
  /// fitted onto the monitors there are (M19).
  fn floating_windows(self) -> View {
    let floating = self.shown.map(|s| s.layout.floating.clone());
    let monitors = dovetail::monitors();
    View::fragment(floating.into_iter().map(|f| {
      let id = f.id;
      let dock = self;
      let place = f
        .place
        .unwrap_or(WindowPlace { position: None, size: FLOAT_SIZE });
      // The layout keeps logical pixels at the scale of the monitor the
      // window was on.
      let scale = place.position.and_then(|(x, y)| {
        monitors
          .iter()
          .find(|m| m.bounds.contains((x * m.scale, y * m.scale)))
          .map(|m| m.scale)
      });
      let saved = SavedWindow {
        position: place.position,
        size: place.size,
        scale: scale.unwrap_or(1.0),
        state: WindowState::Normal,
        monitor: None,
      };
      let (fitted, _) =
        dovetail::fit(&saved, &monitors, WindowLimits::default());
      let title = self.title(f.tabs.active_panel());
      let window: View = match fitted.position {
        Some(position) => ui! {
          Window {
            title: title,
            size: fitted.size,
            position: position,
            on_close: move |()| dock.dock_floating(id),
            on_place: move |p| dock.set_place(id, p),
            FloatingStack { dock: dock, id: id }
          }
        },
        None => ui! {
          Window {
            title: title,
            size: fitted.size,
            on_close: move |()| dock.dock_floating(id),
            on_place: move |p| dock.set_place(id, p),
            FloatingStack { dock: dock, id: id }
          }
        },
      };
      View::keyed(window, id)
    }))
  }

  /// Shows a preview for the drop target `owner`, if it is not shown yet.
  fn show_preview(
    self,
    owner: String,
    preview: Preview,
  ) {
    let new = Some((owner, preview));
    if self.shown.map(|s| s.preview != new) {
      self.shown.update(|s| s.preview = new);
    }
  }

  /// Hides the preview, if drop target `owner` showed it.
  fn hide_preview(
    self,
    owner: &str,
  ) {
    if self
      .shown
      .map(|s| s.preview.as_ref().is_some_and(|p| p.0 == owner))
    {
      self.shown.update(|s| s.preview = None);
    }
  }

  /// A dragged tab was let go.
  fn drag_ended(
    self,
    panel: &str,
    end: DragEnd,
  ) {
    if self.shown.map(|s| s.preview.is_some()) {
      self.shown.update(|s| s.preview = None);
    }
    if let DragEnd::Outside { screen } = end {
      // The new window goes where the tab was let go, with the pointer
      // over its tab.
      let place = WindowPlace {
        position: screen.map(|(x, y)| (x - 40.0, y - 16.0)),
        size: FLOAT_SIZE,
      };
      self.move_panel(panel, Place::Float(Some(place)));
    }
  }

  /// A drop handler for a target that moves the dragged panel to `place`,
  /// showing `preview` while it is over it. `at` works out both from the
  /// event; `None` declines.
  fn drop_handler(
    self,
    owner: String,
    at: impl Fn(&DropEvent) -> Option<(Preview, Place)> + 'static,
  ) -> DropHandler {
    DropHandler::new(move |e| {
      let Some(DraggedPanel(panel)) = e.data.get::<DraggedPanel>() else {
        return false;
      };
      match e.kind {
        DropKind::Leave => {
          self.hide_preview(&owner);
          false
        },
        DropKind::Over => match at(e) {
          Some((preview, _)) => {
            self.show_preview(owner.clone(), preview);
            true
          },
          None => {
            self.hide_preview(&owner);
            false
          },
        },
        DropKind::Drop => match at(e) {
          Some((_, place)) => {
            self.move_panel(panel, place);
            true
          },
          None => false,
        },
      }
    })
  }

  /// Keys for a stack: F6 and Shift+F6 go to the next and previous panel,
  /// and Ctrl+Page Down and Ctrl+Page Up to the next and previous tab.
  fn stack_keys(
    self,
    active: String,
  ) -> KeyHandler {
    KeyHandler::new(move |e| {
      let m = e.modifiers;
      if e.is(NamedKey::F6) && !m.control && !m.alt && !m.meta {
        self.focus_next_panel(Some(&active), m.shift);
        return true;
      }
      let page = e.is(NamedKey::PageDown) || e.is(NamedKey::PageUp);
      if page && m.control && !m.alt && !m.shift {
        if let Some(next) = self.neighbour_tab(&active, e.is(NamedKey::PageUp))
        {
          self.focus_panel(&next);
        }
        return true;
      }
      false
    })
  }
}

/// F6 with nothing focused goes to the first panel (or the last, with
/// Shift).
fn unfocused_keys(dock: Dock) -> KeyHandler {
  KeyHandler::new(move |e: &KeyEvent| {
    let m = e.modifiers;
    if e.is(NamedKey::F6) && !m.control && !m.alt && !m.meta {
      dock.focus_next_panel(None, m.shift);
      return true;
    }
    false
  })
}

fn fill() -> Style {
  Style {
    width: Size::Fill,
    height: Size::Fill,
    ..Style::default()
  }
}

/// The main window's dock: every docked panel, with a thin edge round it
/// to drop panels along the sides. It fills the space it is given.
#[component]
pub fn DockArea(dock: Dock) -> View {
  let root = dock.shown.map(|s| s.layout.root.is_some());
  let preview = dock.shown.map(|s| s.preview.clone().map(|p| p.1));
  let edge = |side: Side| -> View {
    let lit = preview == Some(Preview::Edge(side));
    let (width, height) = match side.axis() {
      Axis::Row => (Size::Fixed(SPLITTER), Size::Fill),
      Axis::Column => (Size::Fill, Size::Fixed(SPLITTER)),
    };
    Element::new(ElementKind::Box)
      .style(Style {
        width,
        height,
        background: if lit { accent() } else { bar_colour() },
        ..Style::default()
      })
      .on_drop(dock.drop_handler(format!("edge:{side:?}"), move |_| {
        Some((Preview::Edge(side), Place::Edge(side)))
      }))
      .into()
  };
  let middle: View = if root {
    ui! { DockNode { dock: dock, path: Vec::new() } }
  } else {
    // Every panel floats: the whole area takes a panel back.
    Element::new(ElementKind::Box)
      .style(Style {
        grow: 1.0,
        align: Align::Center,
        background: if preview == Some(Preview::Edge(Side::Left)) {
          preview_colour()
        } else {
          Color::TRANSPARENT
        },
        ..fill()
      })
      .on_drop(dock.drop_handler("empty".into(), |_| {
        Some((Preview::Edge(Side::Left), Place::Edge(Side::Left)))
      }))
      .children([View::text("Drag a panel here")])
      .into()
  };
  let row: View = Element::new(ElementKind::Row)
    .style(Style { grow: 1.0, ..fill() })
    .children([
      edge(Side::Left),
      Element::new(ElementKind::Box)
        .style(Style { grow: 1.0, clip: true, ..fill() })
        .children([middle])
        .into(),
      edge(Side::Right),
    ])
    .into();
  let area: View = Element::new(ElementKind::Column)
    .style(fill())
    .on_key(unfocused_keys(dock))
    .children([edge(Side::Top), row, edge(Side::Bottom)])
    .into();
  // Floating stacks' windows belong to the dock area: they close when it
  // goes, as when its window closes (M19).
  View::fragment([area, dock.floating_windows()])
}

/// The part of the main window's layout at `path`.
#[component]
fn DockNode(
  dock: Dock,
  path: Vec<usize>,
) -> View {
  let node = dock.shown.map(|s| {
    s.layout
      .root
      .as_ref()
      .and_then(|r| r.at(&path))
      .cloned()
  });
  match node {
    Some(Node::Tabs(_)) => {
      ui! { DockStack { dock: dock, at: StackAt::Docked(path.clone()) } }
    },
    Some(Node::Split { .. }) => {
      ui! { DockSplit { dock: dock, path: path.clone() } }
    },
    None => View::Empty,
  }
}

/// A split: its parts, with a splitter between each two that resizes them
/// when dragged.
#[component]
fn DockSplit(
  dock: Dock,
  path: Vec<usize>,
) -> View {
  let size = use_state(|| (0.0f32, 0.0f32));
  // The splitter held, where the pointer was, and the shares then.
  let grab = use_state(|| None::<(usize, f32, Vec<f32>)>);
  let Some(Node::Split { axis, parts }) = dock.shown.map(|s| {
    s.layout
      .root
      .as_ref()
      .and_then(|r| r.at(&path))
      .cloned()
  }) else {
    return View::Empty;
  };
  let row = axis == Axis::Row;
  let shares: Vec<f32> = parts.iter().map(|p| p.0).collect();
  let mut children = Vec::new();
  for (i, share) in shares.iter().enumerate() {
    if i > 0 {
      let path = path.clone();
      let shares = shares.clone();
      let splitter = Element::new(ElementKind::Box)
        .style(Style {
          width: if row {
            Size::Fixed(SPLITTER)
          } else {
            Size::Fill
          },
          height: if row {
            Size::Fill
          } else {
            Size::Fixed(SPLITTER)
          },
          background: line(),
          ..Style::default()
        })
        .role(Role::Splitter)
        .label("Resize")
        .on_pointer(PointerHandler::new(move |e| {
          let along = if row { e.window_x } else { e.window_y };
          match e.kind {
            PointerKind::Down if e.button == PointerButton::Primary => {
              grab.set(Some((i - 1, along, shares.clone())));
            },
            PointerKind::Move => {
              let Some((left, from, start)) = grab.get() else {
                return;
              };
              let (w, h) = size.get();
              let total = if row { w } else { h };
              let free = total - SPLITTER * (start.len() - 1) as f32;
              if free <= 0.0 {
                return;
              }
              let pair = (start[left] + start[left + 1]) * free;
              let low = MIN_PART.min(pair / 2.0);
              let a =
                (start[left] * free + along - from).clamp(low, pair - low);
              let mut new = start.clone();
              new[left] = a / free;
              new[left + 1] = (pair - a) / free;
              dock.set_shares(&path, &new);
            },
            PointerKind::Up => grab.set(None),
            PointerKind::Down => {},
          }
        }));
      children.push(splitter.into());
    }
    let mut part_path = path.clone();
    part_path.push(i);
    children.push(
      Element::new(ElementKind::Box)
        .style(Style { grow: *share, clip: true, ..fill() })
        .children([ui! { DockNode { dock: dock, path: part_path } }])
        .into(),
    );
  }
  Element::new(if row {
    ElementKind::Row
  } else {
    ElementKind::Column
  })
  .style(Style { gap: 0.0, ..fill() })
  .on_size(Callback::new(move |s| size.set(s)))
  .children(children)
  .into()
}

/// Which part of a stack's content the point is over: the middle, or near
/// a side.
fn zone(e: &DropEvent) -> Option<Side> {
  if e.width <= 0.0 || e.height <= 0.0 {
    return None;
  }
  let (x, y) = (e.x / e.width, e.y / e.height);
  let sides = [
    (x, Side::Left),
    (1.0 - x, Side::Right),
    (y, Side::Top),
    (1.0 - y, Side::Bottom),
  ];
  let (d, side) = sides
    .into_iter()
    .fold((f32::MAX, Side::Left), |a, b| if b.0 < a.0 { b } else { a });
  (d < 0.3).then_some(side)
}

/// The preview over a stack's content: half of it on a side, or all of it.
fn preview_box(side: Option<Side>) -> View {
  let lit: View = Element::new(ElementKind::Box)
    .style(Style {
      grow: 1.0,
      background: preview_colour(),
      ..fill()
    })
    .into();
  let Some(side) = side else {
    return Element::new(ElementKind::Box)
      .style(fill())
      .children([lit])
      .into();
  };
  let blank: View = Element::new(ElementKind::Box)
    .style(Style { grow: 1.0, ..fill() })
    .into();
  let children = if side.first() {
    [lit, blank]
  } else {
    [blank, lit]
  };
  let kind = match side.axis() {
    Axis::Row => ElementKind::Row,
    Axis::Column => ElementKind::Column,
  };
  Element::new(kind)
    .style(Style { gap: 0.0, ..fill() })
    .children(children)
    .into()
}

/// A small button in the tab bar.
fn bar_button(
  text: &str,
  on_press: impl Fn() + 'static,
) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::xy(8.0, 3.0),
      radius: 4.0,
      background: tab(),
      text_color: Some(ink()),
      font_size: Some(12.0),
      ..Style::default()
    })
    .role(Role::Button)
    .focusable(true)
    .on_press(Callback::new(move |()| on_press()))
    .children([View::text(text.to_string())])
    .into()
}

/// A tab stack: its tab bar and the panel shown. Used for docked stacks
/// and floating windows.
#[component]
fn DockStack(
  dock: Dock,
  at: StackAt,
) -> View {
  // The tab the button went down on, and where, until a drag starts.
  let pressed = use_state(|| None::<(String, f32, f32)>);
  let Some(tabs) = dock.shown.map(|s| s.layout.tabs(&at).cloned()) else {
    return View::Empty;
  };
  let preview = dock.shown.map(|s| s.preview.clone().map(|p| p.1));
  let content = dock.shown.map(|s| Rc::clone(&s.content));
  let active = tabs.active_panel().to_string();
  let floating = matches!(at, StackAt::Floating(_));

  // The tab bar: tabs, a marker where a dropped tab would go, and the
  // button that floats or docks the stack.
  let marker = || -> View {
    Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(3.0),
        height: Size::Fixed(22.0),
        background: accent(),
        ..Style::default()
      })
      .into()
  };
  let mut bar = Vec::new();
  let count = tabs.panels.len();
  for (i, panel) in tabs.panels.iter().enumerate() {
    if preview == Some(Preview::Tab(at.clone(), i)) {
      bar.push(marker());
    }
    let shown = *panel == active;
    let (click, key_panel, drag_panel) =
      (panel.clone(), panel.clone(), panel.clone());
    let drop_at = at.clone();
    let tab = Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::xy(12.0, 5.0),
        radius: 4.0,
        background: if shown { shown_tab() } else { tab() },
        text_color: Some(ink()),
        font_size: Some(13.0),
        ..Style::default()
      })
      .role(Role::Tab)
      .toggled(shown)
      .focusable(true)
      .focus_target(tab_target(panel))
      .on_press(Callback::new(move |()| dock.activate(&click)))
      .on_key(KeyHandler::new(move |e| {
        let plain = e.modifiers == dovetail::Modifiers::default();
        let back = e.is(NamedKey::ArrowLeft);
        if plain && (back || e.is(NamedKey::ArrowRight)) {
          if let Some(next) = dock.neighbour_tab(&key_panel, back) {
            dock.activate(&next);
            tab_target(&next).focus();
          }
          return true;
        }
        false
      }))
      .on_pointer(PointerHandler::new(move |e| match e.kind {
        PointerKind::Down if e.button == PointerButton::Primary => {
          pressed.set(Some((drag_panel.clone(), e.window_x, e.window_y)));
        },
        PointerKind::Move => {
          let Some((panel, x, y)) = pressed.get() else {
            return;
          };
          if (e.window_x - x).hypot(e.window_y - y) >= DRAG_START {
            pressed.set(None);
            let ended = panel.clone();
            start_drag(
              DragData::new(DraggedPanel(panel)),
              Callback::new(move |end| dock.drag_ended(&ended, end)),
            );
          }
        },
        PointerKind::Up => pressed.set(None),
        PointerKind::Down => {},
      }))
      .on_drop(dock.drop_handler(owner(&format!("tab{i}"), &at), move |e| {
        let index = if e.x < e.width / 2.0 { i } else { i + 1 };
        Some((
          Preview::Tab(drop_at.clone(), index),
          Place::Tabs { stack: drop_at.clone(), index },
        ))
      }))
      .children([View::text(dock.title(panel))]);
    bar.push(tab.into());
  }
  if preview == Some(Preview::Tab(at.clone(), count)) {
    bar.push(marker());
  }
  bar.push(
    Element::new(ElementKind::Box)
      .style(Style { grow: 1.0, ..Style::default() })
      .into(),
  );
  bar.push(match &at {
    StackAt::Floating(id) => {
      let id = *id;
      bar_button("Dock", move || dock.dock_floating(id))
    },
    StackAt::Docked(_) => {
      let panel = active.clone();
      bar_button("Float", move || dock.move_panel(&panel, Place::Float(None)))
    },
  });
  let bar_at = at.clone();
  let bar: View = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fill,
      gap: 2.0,
      padding: Edges::xy(4.0, 3.0),
      align: Align::Center,
      background: bar_colour(),
      ..Style::default()
    })
    .role(Role::TabList)
    .label("Tabs")
    .on_drop(dock.drop_handler(owner("bar", &at), move |_| {
      Some((
        Preview::Tab(bar_at.clone(), count),
        Place::Tabs { stack: bar_at.clone(), index: count },
      ))
    }))
    .children(bar)
    .into();

  // The panel shown, with the preview over it while a tab is dragged
  // there.
  let panel: View = Element::new(ElementKind::Box)
    .style(Style { clip: true, ..fill() })
    .role(Role::TabPanel)
    .label(dock.title(&active))
    .focusable(true)
    .focus_target(panel_target(&active))
    .children([content(&active)])
    .into();
  let mut layers = vec![panel];
  if let Some(Preview::Zone(p, side)) = &preview
    && *p == at
  {
    layers.push(preview_box(*side));
  }
  let zone_at = at.clone();
  let body: View = Element::new(ElementKind::Stack)
    .style(Style {
      grow: 1.0,
      align: Align::Stretch,
      ..fill()
    })
    .on_drop(dock.drop_handler(owner("zone", &at), move |e| {
      let side = if floating { None } else { zone(e) };
      let place = match (side, &zone_at) {
        (Some(side), StackAt::Docked(path)) => {
          Place::Beside { stack: path.clone(), side }
        },
        _ => Place::Tabs {
          stack: zone_at.clone(),
          index: usize::MAX,
        },
      };
      Some((Preview::Zone(zone_at.clone(), side), place))
    }))
    .children(layers)
    .into();
  Element::new(ElementKind::Column)
    .style(Style { gap: 0.0, ..fill() })
    .on_key(dock.stack_keys(active))
    .children([bar, body])
    .into()
}

/// What a floating window shows: its stack. F6 with nothing focused goes
/// to the first panel.
#[component]
pub fn FloatingStack(
  dock: Dock,
  id: u64,
) -> View {
  let there = dock
    .shown
    .map(|s| s.layout.floating.iter().any(|f| f.id == id));
  let inner: View = if there {
    ui! { DockStack { dock: dock, at: StackAt::Floating(id) } }
  } else {
    View::Empty
  };
  Element::new(ElementKind::Box)
    .style(Style { align: Align::Stretch, ..fill() })
    .on_key(unfocused_keys(dock))
    .children([inner])
    .into()
}

/// Shown in a panel's place while it is not filled in: a muted line of
/// text in the middle. Handy for examples.
pub fn placeholder(text: &str) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      align: Align::Center,
      text_color: Some(theme().colors.on_surface_variant),
      ..fill()
    })
    .children([View::text(text.to_string())])
    .into()
}
