//! M16: lists, trees, tables and the property grid, through the component
//! tree and layout with no window. Scroll positions and sizes are passed
//! back after each layout, and the tree built again, as the window does.

extern crate dovetail_core as dovetail;

use std::cell::{Cell, RefCell};

use dovetail::{
  Callback, Element, KeyEvent, Modifiers, NamedKey, PointerButton,
  PointerEvent, PointerKind, Role, Signal, SortOrder, Tree, View, use_state,
};
use dovetail_commands::History;
use dovetail_data::{
  Column, DataList, DataTable, DataTree, Expansion, Property, PropertyGrid,
  PropertySection, RowHeight, RowsDropped, Selection, SelectionMode, Sort,
};
use dovetail_layout::{BoxKey, Content, FixedPitch, Layout};
use dovetail_macros::{component, ui};

const ROWS: usize = 100_000;
const HEIGHT: f32 = 20.0;

struct Window {
  tree: Tree,
  layout: Layout,
  viewports: Vec<(BoxKey, dovetail::Viewport)>,
  sizes: Vec<(BoxKey, (f32, f32))>,
}

impl Window {
  fn open(root: View) -> Self {
    let mut tree = Tree::new();
    tree.mount(root);
    let mut w = Window {
      tree,
      layout: Layout::new(),
      viewports: Vec::new(),
      sizes: Vec::new(),
    };
    w.refresh();
    w
  }

  /// Builds and lays out, telling scroll positions and sizes, until
  /// nothing changes: as the window does in one frame.
  fn refresh(&mut self) {
    for _ in 0..8 {
      self.tree.update();
      self
        .layout
        .update(&self.tree, 400.0, 300.0, &FixedPitch::default());
      let mut calls: Vec<Box<dyn Fn()>> = Vec::new();
      let mut viewports = Vec::new();
      let mut sizes = Vec::new();
      for b in self.layout.boxes() {
        let Content::Element(e) = &b.content else {
          continue;
        };
        if let (Some(f), Some(v)) = (&e.on_scroll, self.layout.viewport(&b.key))
        {
          if !self.viewports.contains(&(b.key.clone(), v)) {
            let f = f.clone();
            calls.push(Box::new(move || f.call(v)));
          }
          viewports.push((b.key.clone(), v));
        }
        if let Some(f) = &e.on_size {
          let size = (b.rect.width, b.rect.height);
          if !self.sizes.contains(&(b.key.clone(), size)) {
            let f = f.clone();
            calls.push(Box::new(move || f.call(size)));
          }
          sizes.push((b.key.clone(), size));
        }
      }
      self.viewports = viewports;
      self.sizes = sizes;
      for call in &calls {
        call();
      }
      if !self.tree.needs_update() {
        return;
      }
    }
    panic!("layout did not settle");
  }

  fn all(
    &self,
    role: Role,
  ) -> Vec<(BoxKey, Element)> {
    self
      .layout
      .boxes()
      .iter()
      .filter_map(|b| match &b.content {
        Content::Element(e) if e.role == role => {
          Some((b.key.clone(), (**e).clone()))
        },
        _ => None,
      })
      .collect()
  }

  fn one(
    &self,
    role: Role,
  ) -> (BoxKey, Element) {
    self
      .all(role)
      .into_iter()
      .next()
      .unwrap_or_else(|| panic!("no {role:?}"))
  }

  /// The view that takes keys: the focusable element with `role`.
  fn view(
    &self,
    role: Role,
  ) -> BoxKey {
    self
      .all(role)
      .into_iter()
      .find(|(_, e)| e.focusable)
      .map(|(k, _)| k)
      .unwrap_or_else(|| panic!("no focusable {role:?}"))
  }

  fn texts(&self) -> Vec<String> {
    self
      .layout
      .boxes()
      .iter()
      .filter_map(|b| match &b.content {
        Content::Text { text, .. } => Some(text.to_string()),
        _ => None,
      })
      .collect()
  }

  fn has_text(
    &self,
    text: &str,
  ) -> bool {
    self.texts().iter().any(|t| t == text)
  }

  fn key(
    &mut self,
    view: &BoxKey,
    event: KeyEvent,
  ) -> bool {
    let used = self.layout.dispatch_key(Some(view), &event);
    self.refresh();
    used
  }

  /// The row with `role` whose item index is `index`.
  fn row(
    &self,
    role: Role,
    index: usize,
  ) -> (BoxKey, Element) {
    self
      .all(role)
      .into_iter()
      .find(|(_, e)| e.item.is_some_and(|p| p.index == index))
      .unwrap_or_else(|| panic!("no {role:?} at {index}"))
  }

  fn pointer(
    &mut self,
    e: &Element,
    kind: PointerKind,
    (x, y): (f32, f32),
    modifiers: Modifiers,
    clicks: u32,
  ) {
    e.on_pointer
      .clone()
      .expect("a pointer handler")
      .handle(&PointerEvent {
        kind,
        button: PointerButton::Primary,
        x: 0.0,
        y: 0.0,
        window_x: x,
        window_y: y,
        text_at: None,
        modifiers,
        clicks,
      });
    self.refresh();
  }

  /// A click on the row of `role` at `index`.
  fn click(
    &mut self,
    role: Role,
    index: usize,
    modifiers: Modifiers,
  ) {
    let (_, e) = self.row(role, index);
    self.pointer(&e, PointerKind::Down, (0.0, 0.0), modifiers, 1);
    let (_, e) = self.row(role, index);
    self.pointer(&e, PointerKind::Up, (0.0, 0.0), modifiers, 1);
  }

  fn press(
    &mut self,
    e: &Element,
  ) {
    e.on_press.clone().expect("pressable").call(());
    self.refresh();
  }
}

fn shift() -> Modifiers {
  Modifiers { shift: true, ..Modifiers::default() }
}

fn command() -> Modifiers {
  if cfg!(target_os = "macos") {
    Modifiers { meta: true, ..Modifiers::default() }
  } else {
    Modifiers { control: true, ..Modifiers::default() }
  }
}

// ---------------------------------------------------------------- lists

thread_local! {
  static LIST: Cell<Option<(Selection, Signal<u32>)>> = const { Cell::new(None) };
  static DROPS: RefCell<Vec<RowsDropped>> = const { RefCell::new(Vec::new()) };
  static BUILT: Cell<usize> = const { Cell::new(0) };
}

#[component]
fn List() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Multiple)).get();
  let activated = use_state(|| 0u32);
  LIST.set(Some((selection, activated)));
  ui! {
    DataList {
      count: ROWS,
      row: |i: usize, _: &dovetail_data::RowState| {
        BUILT.set(BUILT.get() + 1);
        View::text(format!("Row {i}"))
      },
      text: |i: usize| format!("Row {i}"),
      selection: selection,
      row_height: RowHeight::Fixed(HEIGHT),
      label: "Parts",
      on_activate: move |_| activated.update(|n| *n += 1),
      draggable: true,
      on_drop: Some(Callback::new(|d: RowsDropped| {
        DROPS.with_borrow_mut(|v| v.push(d))
      })),
    }
  }
}

fn list() -> Window {
  Window::open(ui! { List {} })
}

#[test]
fn a_long_list_makes_only_the_rows_in_sight() {
  BUILT.set(0);
  let mut w = list();
  let rows = w.all(Role::ListBoxOption).len();
  assert!(
    (15..=40).contains(&rows),
    "{rows} rows made for a 300-pixel view"
  );
  assert!(
    BUILT.get() < 200,
    "only those rows were built: {}",
    BUILT.get()
  );
  assert!(w.has_text("Row 0"));
  let (key, list) = w.one(Role::ListBox);
  assert!(list.focusable && list.multiselect);
  // The scrollbar covers every row.
  let (_, (_, max)) = w.layout.scroll_extent(&key).expect("scrolls");
  assert_eq!(max, ROWS as f32 * HEIGHT - 300.0);

  // Scrolling far down makes the rows there, in the same frame.
  w.layout
    .scroll_by(10.0, 10.0, 0.0, 50_000.0 * HEIGHT);
  w.refresh();
  assert!(w.has_text("Row 50000"), "{:?}", &w.texts()[..3]);
  assert!(!w.has_text("Row 0"));
  let first = w.row(Role::ListBoxOption, 50_000);
  let rect = w.layout.rect_of(&first.0).expect("placed");
  assert!(rect.y.abs() < 1.0, "row 50000 at the top, at {}", rect.y);
  // Each row says where it is in the whole list.
  let place = first.1.item.expect("a place");
  assert_eq!((place.index, place.count), (50_000, ROWS));
}

#[test]
fn rows_are_chosen_with_the_pointer_as_each_system_does() {
  let mut w = list();
  let (selection, _) = LIST.get().unwrap();
  w.click(Role::ListBoxOption, 2, Modifiers::default());
  w.click(Role::ListBoxOption, 5, shift());
  assert_eq!(selection.len(), 4, "Shift selects a range");
  w.click(Role::ListBoxOption, 8, command());
  assert_eq!(selection.len(), 5, "the command key adds one");
  let (_, row) = w.row(Role::ListBoxOption, 8);
  assert_eq!(row.selected, Some(true));
  assert!(row.current, "the keyboard is on the row clicked last");
  // A double click activates.
  let (_, e) = w.row(Role::ListBoxOption, 8);
  w.pointer(&e, PointerKind::Down, (0.0, 0.0), Modifiers::default(), 2);
  assert_eq!(LIST.get().unwrap().1.get(), 1);
}

#[test]
fn rows_are_chosen_with_the_keyboard_and_kept_in_sight() {
  let mut w = list();
  let (selection, activated) = LIST.get().unwrap();
  let view = w.view(Role::ListBox);
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowDown)));
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowDown)));
  assert!(w.row(Role::ListBoxOption, 1).1.current);
  assert!(w.key(&view, KeyEvent::named(NamedKey::PageDown)));
  let at = selection
    .current_index(ROWS, &|i| i.into())
    .unwrap();
  assert!(at > 10, "a page moved {at} rows");
  assert!(w.key(&view, KeyEvent::named(NamedKey::End)));
  assert!(w.has_text("Row 99999"), "End scrolls to the last row");
  let rect = w
    .layout
    .rect_of(&w.row(Role::ListBoxOption, ROWS - 1).0)
    .unwrap();
  assert!(rect.y + rect.height <= 300.5, "the last row is in sight");
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowUp).with(shift())));
  assert_eq!(selection.len(), 2);
  assert!(w.key(&view, KeyEvent::named(NamedKey::Home)));
  assert!(w.has_text("Row 0"));
  assert!(w.key(&view, KeyEvent::character('5')));
  assert!(w.has_text("Row 5"), "typing finds a row");
  assert!(w.key(&view, KeyEvent::named(NamedKey::Enter)));
  assert_eq!(activated.get(), 1);
}

#[test]
fn rows_are_dragged_and_dropped() {
  let mut w = list();
  DROPS.with_borrow_mut(Vec::clear);
  w.click(Role::ListBoxOption, 3, Modifiers::default());
  w.click(Role::ListBoxOption, 4, command());
  let (_, e) = w.row(Role::ListBoxOption, 4);
  w.pointer(&e, PointerKind::Down, (10.0, 90.0), Modifiers::default(), 1);
  let (_, e) = w.row(Role::ListBoxOption, 4);
  w.pointer(
    &e,
    PointerKind::Move,
    (10.0, 130.0),
    Modifiers::default(),
    1,
  );
  let drag = dovetail::take_started_drag().expect("a drag started");
  let rows = drag
    .data
    .get::<dovetail_data::RowDrag>()
    .expect("rows");
  assert_eq!(rows.keys.len(), 2, "both selected rows are dragged");
  let (_, list) = w.one(Role::ListBox);
  let target = list.on_drop.clone().expect("takes drops");
  let event = |kind, y| dovetail::DropEvent {
    kind,
    x: 10.0,
    y,
    width: 400.0,
    height: 300.0,
    data: drag.data.clone(),
  };
  assert!(target.handle(&event(dovetail::DropKind::Over, 205.0)));
  w.refresh();
  assert!(target.handle(&event(dovetail::DropKind::Drop, 205.0)));
  w.refresh();
  let drops = DROPS.with_borrow(Clone::clone);
  assert_eq!(drops.len(), 1);
  assert_eq!(drops[0].at, 10, "dropped before row 10");
  assert_eq!(drops[0].source, "Parts");
}

// ------------------------------------------------------ measured heights

#[component]
fn Measured() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Single)).get();
  ui! {
    DataList {
      count: 1000,
      row: |i: usize, _: &dovetail_data::RowState| {
        if i.is_multiple_of(3) {
          View::text(format!("Row {i}\nsecond line"))
        } else {
          View::text(format!("Row {i}"))
        }
      },
      selection: selection,
      row_height: RowHeight::Measured { estimate: 20.0 },
    }
  }
}

#[test]
fn rows_of_different_heights_are_measured_as_they_appear() {
  let w = Window::open(ui! { Measured {} });
  let (key, _) = w.one(Role::ListBox);
  let tall = w
    .layout
    .rect_of(&w.row(Role::ListBoxOption, 0).0)
    .unwrap();
  let short = w
    .layout
    .rect_of(&w.row(Role::ListBoxOption, 1).0)
    .unwrap();
  assert!(tall.height > short.height + 5.0, "{tall:?} {short:?}");
  // Row 1 starts where row 0 ends: the measured height is used.
  assert!((short.y - (tall.y + tall.height)).abs() < 0.5);
  let (_, (_, max)) = w.layout.scroll_extent(&key).unwrap();
  assert!(
    max > 1000.0 * 20.0 - 300.0,
    "measured rows add to the length"
  );
}

// ---------------------------------------------------------------- trees

thread_local! {
  static TREE: Cell<Option<(Selection, Expansion)>> = const { Cell::new(None) };
  static ASKED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn name(key: &dovetail::Key) -> String {
  format!("{key:?}")
}

#[component]
fn Folders() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Single)).get();
  let expansion = use_state(Expansion::new).get();
  TREE.set(Some((selection, expansion)));
  ui! {
    DataTree {
      // 100 groups of 1,000; a group's items are made only when it opens.
      nodes: |node: Option<&dovetail::Key>| match node {
        None => (0..100u64).map(dovetail::Key::from).collect(),
        Some(k) => {
          ASKED.with_borrow_mut(|a| a.push(name(k)));
          (0..1000u64)
            .map(|i| dovetail::Key::from(format!("{}/{i}", name(k))))
            .collect()
        },
      },
      has_children: |k: &dovetail::Key| !name(k).contains('/'),
      row: |k: &dovetail::Key, _: &dovetail_data::RowState| View::text(name(k)),
      selection: selection,
      expansion: Some(expansion),
      row_height: RowHeight::Fixed(HEIGHT),
      label: "Layers",
    }
  }
}

#[test]
fn tree_nodes_open_and_close_with_the_keyboard_and_the_pointer() {
  ASKED.with_borrow_mut(Vec::clear);
  let mut w = Window::open(ui! { Folders {} });
  let (selection, expansion) = TREE.get().unwrap();
  assert!(
    ASKED.with_borrow(Vec::is_empty),
    "closed nodes are not asked"
  );
  let view = w.view(Role::Tree);
  w.key(&view, KeyEvent::named(NamedKey::ArrowDown));
  let (_, first) = w.row(Role::TreeItem, 0);
  assert_eq!(first.expanded, Some(false));
  assert_eq!(first.item.unwrap().level, Some(1));
  // Right opens, and only that node is asked for its children.
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowRight)));
  assert!(expansion.is_open(&0u64.into()));
  let asked = ASKED.with_borrow(Clone::clone);
  assert!(!asked.is_empty() && asked.iter().all(|a| *a == name(&0u64.into())));
  // Right again moves into it.
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowRight)));
  let child = selection.current().unwrap();
  assert_eq!(
    child,
    dovetail::Key::from(format!("{}/0", name(&0u64.into())))
  );
  let row = w
    .all(Role::TreeItem)
    .into_iter()
    .find(|(_, e)| e.current)
    .unwrap()
    .1;
  let place = row.item.unwrap();
  assert_eq!((place.index, place.count, place.level), (0, 1000, Some(2)));
  // Left moves to the parent, then closes it.
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowLeft)));
  assert_eq!(selection.current(), Some(0u64.into()));
  assert!(w.key(&view, KeyEvent::named(NamedKey::ArrowLeft)));
  assert!(!expansion.is_open(&0u64.into()));
  // A click on the arrow opens a node.
  let (_, second) = w.row(Role::TreeItem, 1);
  let arrow = find_pressable(&w, &second).expect("an arrow");
  w.press(&arrow);
  assert!(expansion.is_open(&1u64.into()));
  // 100 groups and 1,000 items in the second: the list is that long.
  let (key, _) = w.one(Role::Tree);
  let (_, (_, max)) = w.layout.scroll_extent(&key).unwrap();
  assert_eq!(max, 1100.0 * HEIGHT - 300.0);
}

/// The first element inside `row` that can be pressed.
fn find_pressable(
  w: &Window,
  row: &Element,
) -> Option<Element> {
  let boxes = w.layout.boxes();
  let at = boxes.iter().position(|b| match &b.content {
    Content::Element(e) => **e == *row,
    _ => false,
  })?;
  let mut stack = boxes[at].children.clone();
  while let Some(i) = stack.pop() {
    if let Content::Element(e) = &boxes[i].content
      && e.on_press.is_some()
    {
      return Some((**e).clone());
    }
    stack.extend(boxes[i].children.iter().copied());
  }
  None
}

// --------------------------------------------------------------- tables

thread_local! {
  static TABLE: RefCell<Vec<Sort>> = const { RefCell::new(Vec::new()) };
  static WIDTHS: RefCell<Vec<(usize, f32)>> = const { RefCell::new(Vec::new()) };
}

#[component]
fn Parts() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Multiple)).get();
  let sort = use_state(|| None::<Sort>);
  ui! {
    DataTable {
      columns: vec![
        Column::new("Name", 160.0),
        Column::new("Value", 120.0).at_end(),
        Column::new("Notes", 300.0).unsorted(),
      ],
      count: ROWS,
      cell: |r: usize, c: usize, _: &dovetail_data::RowState| {
        View::text(format!("r{r}c{c}"))
      },
      selection: selection,
      sort: sort.get(),
      on_sort: move |s: Sort| {
        TABLE.with_borrow_mut(|v| v.push(s));
        sort.set(Some(s));
      },
      on_resize: |c: (usize, f32)| WIDTHS.with_borrow_mut(|v| v.push(c)),
      row_height: RowHeight::Fixed(HEIGHT),
      label: "Nets",
    }
  }
}

#[test]
fn table_columns_sort_and_resize_with_the_pointer_and_the_keyboard() {
  TABLE.with_borrow_mut(Vec::clear);
  WIDTHS.with_borrow_mut(Vec::clear);
  let mut w = Window::open(ui! { Parts {} });
  let (_, table) = w.one(Role::Table);
  assert_eq!(table.table_size, Some((ROWS + 1, 3)));
  let headers = w.all(Role::ColumnHeader);
  assert_eq!(headers.len(), 3);
  // A press sorts up, then down, and the header says so.
  w.press(&headers[0].1);
  w.press(&w.all(Role::ColumnHeader)[0].1);
  let sorts = TABLE.with_borrow(Clone::clone);
  assert_eq!(sorts[0].order, SortOrder::Ascending);
  assert_eq!(sorts[1].order, SortOrder::Descending);
  assert_eq!(
    w.all(Role::ColumnHeader)[0].1.sorted,
    Some(SortOrder::Descending)
  );
  assert!(
    w.all(Role::ColumnHeader)[2].1.on_press.is_none(),
    "unsorted"
  );

  // Dragging the first header's edge makes it wider.
  let edge = find_pointer(&w, &w.all(Role::ColumnHeader)[0].1).expect("edge");
  w.pointer(
    &edge,
    PointerKind::Down,
    (160.0, 5.0),
    Modifiers::default(),
    1,
  );
  w.pointer(
    &edge,
    PointerKind::Move,
    (200.0, 5.0),
    Modifiers::default(),
    1,
  );
  w.pointer(
    &edge,
    PointerKind::Up,
    (200.0, 5.0),
    Modifiers::default(),
    1,
  );
  assert_eq!(WIDTHS.with_borrow(|v| *v.last().unwrap()), (0, 200.0));
  let (cell, _) = w
    .all(Role::Cell)
    .into_iter()
    .find(|(_, e)| e.column == Some(0))
    .unwrap();
  assert_eq!(w.layout.rect_of(&cell).unwrap().width, 200.0);

  // The header row is one Tab stop: Right moves, Ctrl+Right widens, Enter
  // sorts.
  let header_row = w
    .layout
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Element(e)
        if e.focusable
          && e.on_key.is_some()
          && e.role == Role::None
          && !e.style.scrollbars =>
      {
        Some(b.key.clone())
      },
      _ => None,
    })
    .expect("the header row");
  assert!(w.key(&header_row, KeyEvent::named(NamedKey::ArrowRight)));
  assert!(w.key(
    &header_row,
    KeyEvent::named(NamedKey::ArrowRight).with(command())
  ));
  assert_eq!(WIDTHS.with_borrow(|v| *v.last().unwrap()), (1, 136.0));
  assert!(w.key(&header_row, KeyEvent::named(NamedKey::Enter)));
  assert_eq!(TABLE.with_borrow(|v| v.last().unwrap().column), 1);

  // Rows are chosen as in a list, and say where they are in the table:
  // the header is row 0.
  w.click(Role::Row, 3, Modifiers::default());
  let (_, row) = w.row(Role::Row, 3);
  assert_eq!(row.selected, Some(true));
  assert_eq!(row.item.unwrap().count, ROWS + 1);
}

#[test]
fn a_table_header_stays_while_rows_scroll_and_moves_across_with_them() {
  let mut w = Window::open(ui! { Parts {} });
  let header = w.all(Role::ColumnHeader)[0].0.clone();
  let before = w.layout.rect_of(&header).unwrap();
  let body = w
    .all(Role::None)
    .into_iter()
    .find(|(_, e)| e.focusable && e.style.scroll && e.style.scrollbars)
    .unwrap()
    .0;
  let rect = w.layout.rect_of(&body).unwrap();
  w.layout
    .scroll_by(rect.x + 5.0, rect.y + 5.0, 100.0, 5000.0);
  w.refresh();
  let after = w.layout.rect_of(&header).unwrap();
  assert_eq!(after.y, before.y, "the header does not scroll down");
  assert_eq!(after.x, before.x - 100.0, "but moves across with the rows");
  assert!(w.has_text("r250c0"));
}

/// The first element inside `e` with a pointer handler.
fn find_pointer(
  w: &Window,
  e: &Element,
) -> Option<Element> {
  let boxes = w.layout.boxes();
  let at = boxes.iter().position(|b| match &b.content {
    Content::Element(x) => **x == *e,
    _ => false,
  })?;
  let mut stack = boxes[at].children.clone();
  while let Some(i) = stack.pop() {
    if let Content::Element(x) = &boxes[i].content
      && x.on_pointer.is_some()
    {
      return Some((**x).clone());
    }
    stack.extend(boxes[i].children.iter().copied());
  }
  None
}

// -------------------------------------------------------- property grid

thread_local! {
  static SHAPE: Cell<Option<(History, Signal<Shape>)>> = const { Cell::new(None) };
}

#[derive(Clone, Debug, PartialEq)]
struct Shape {
  name: String,
  width: f32,
  locked: bool,
  layer: usize,
}

#[component]
fn Inspector() -> View {
  let history = use_state(History::new).get();
  let shape = use_state(|| Shape {
    name: "R1".into(),
    width: 2.0,
    locked: false,
    layer: 0,
  });
  SHAPE.set(Some((history, shape)));
  let s = shape.get();
  let sections = vec![
    PropertySection::new(
      "Shape",
      vec![
        Property::text("Name", s.name.clone(), move |v| {
          shape.update(|s| s.name = v)
        }),
        Property::number("Width", s.width, (0.0, 10.0, 0.5), 1, move |v| {
          shape.update(|s| s.width = v)
        }),
      ],
    ),
    PropertySection::new(
      "Placement",
      vec![
        Property::check("Locked", s.locked, move |v| {
          shape.update(|s| s.locked = v)
        }),
        Property::choice(
          "Layer",
          vec!["Top".into(), "Bottom".into()],
          s.layer,
          move |v| shape.update(|s| s.layer = v),
        ),
      ],
    ),
  ];
  ui! {
    PropertyGrid { sections: sections, history: history }
  }
}

#[test]
fn the_property_grid_edits_values_of_each_kind_and_each_edit_undoes() {
  let mut w = Window::open(ui! { Inspector {} });
  let (history, shape) = SHAPE.get().unwrap();
  // A check box change is one step.
  let (_, check) = w.one(Role::CheckBox);
  w.press(&check);
  assert!(shape.get().locked);
  assert_eq!(history.undo_name().as_deref(), Some("Change Locked"));
  // A drop-down change is one step: open it and choose.
  let (combo_key, _) = w.one(Role::ComboBox);
  w.key(&combo_key, KeyEvent::named(NamedKey::ArrowDown));
  assert_eq!(shape.get().layer, 1);
  // A number: two steps up, kept as one edit when Enter is pressed.
  let (spin, _) = w.one(Role::SpinButton);
  w.key(&spin, KeyEvent::named(NamedKey::ArrowUp));
  w.key(&spin, KeyEvent::named(NamedKey::ArrowUp));
  assert_eq!(shape.get().width, 3.0);
  w.key(&spin, KeyEvent::named(NamedKey::Enter));
  assert_eq!(history.undo_name().as_deref(), Some("Change Width"));
  // Undo takes back each edit in turn.
  assert!(history.undo());
  w.refresh();
  assert_eq!(shape.get().width, 2.0, "both steps in one undo");
  assert!(history.undo());
  assert_eq!(shape.get().layer, 0);
  assert!(history.undo());
  assert!(!shape.get().locked);
  assert!(history.redo());
  assert!(shape.get().locked);
  // Sections open and close.
  w.refresh();
  assert!(w.has_text("Name"));
  let header = w
    .all(Role::Button)
    .into_iter()
    .find(|(_, e)| e.label.as_deref() == Some("Shape"))
    .unwrap()
    .1;
  w.press(&header);
  assert!(!w.has_text("Name"), "the Shape section closed");
}
