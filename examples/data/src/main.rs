//! M16 example: a list, a tree and a table of 100,000 rows each, and a
//! property grid.
//!
//! - `cargo run -p example-data` opens it. On the left is a tree of 100
//!   layers with 1,000 shapes each, made only when a layer opens. In the
//!   middle is a table of 100,000 parts: drag a header's edge to resize a
//!   column, and press a header to sort by it. On the right, the inspector
//!   edits the part the keyboard is on (Ctrl+Z undoes), parts dragged from
//!   the table are dropped in the Picked list, and the log has 100,000
//!   entries of different heights.
//! - `cargo run -p example-data -- --self-test` uses each view with the
//!   pointer and the keyboard in a real window.
//! - `cargo run --release -p example-data -- --bench` measures the time to
//!   build each view and to scroll a frame, without a window.

mod bench;
mod self_test;

use std::cell::Cell;
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;

use dovetail::commands::{CommandScope, Commands, History};
use dovetail::data::{
  Column as TableColumn, DataList, DataTable, DataTree, Expansion, Property,
  PropertyGrid, PropertySection, RowHeight, RowState, RowsDropped, Selection,
  SelectionMode, Sort,
};
use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData, theme};
use dovetail::{Edges, Element, ElementKind, Role, Size, SortOrder, Style};

/// How many parts the table has, and log entries the list has.
pub(crate) const PARTS: usize = 100_000;
/// Layers in the tree, and shapes in each.
pub(crate) const LAYERS: u64 = 100;
pub(crate) const SHAPES: u64 = 1000;
/// Keys of shapes start here, after the layers' keys.
const SHAPE_KEYS: u64 = 1_000_000;

pub(crate) const FOOTPRINTS: [&str; 5] = [
  "0402", "0603", "0805", "SOT-23", "SOIC-8",
];
pub(crate) const SIDES: [&str; 2] = ["Top", "Bottom"];

/// One part on the board.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Part {
  pub(crate) name: String,
  pub(crate) value: f32,
  pub(crate) footprint: usize,
  pub(crate) side: usize,
  pub(crate) locked: bool,
}

/// Part `i` as it is made: the application's data, not stored.
pub(crate) fn base_part(i: u32) -> Part {
  let kind = ["R", "C", "U", "L"][i as usize % 4];
  Part {
    name: format!("{kind}{}", i + 1),
    value: ((i.wrapping_mul(7919)) % 1000) as f32 / 10.0,
    footprint: (i as usize * 31) % FOOTPRINTS.len(),
    side: (i as usize / 7) % 2,
    locked: i.is_multiple_of(11),
  }
}

/// The text of log entry `i`: every fifth has a second line.
pub(crate) fn log_entry(i: usize) -> String {
  if i.is_multiple_of(5) {
    format!("{:06}  Checked net N{}\n        2 warnings", i, i / 5)
  } else {
    format!("{i:06}  Placed part {}", i + 1)
  }
}

/// A layer's key and a shape's key, and back.
pub(crate) fn shape_key(
  layer: u64,
  shape: u64,
) -> Key {
  Key::from(SHAPE_KEYS + layer * SHAPES + shape)
}

pub(crate) fn node_name(key: &Key) -> String {
  match key.number() {
    Some(n) if n >= SHAPE_KEYS => {
      let n = n - SHAPE_KEYS;
      format!("Shape {}.{}", n / SHAPES + 1, n % SHAPES + 1)
    },
    Some(n) => format!("Layer {}", n + 1),
    None => String::new(),
  }
}

/// Everything the window shows, kept together so the self-test can read
/// it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct State {
  pub(crate) dark: Signal<bool>,
  pub(crate) parts: Selection,
  pub(crate) layers: Selection,
  pub(crate) open: Expansion,
  pub(crate) log: Selection,
  pub(crate) picked_rows: Selection,
  pub(crate) sort: Signal<Option<Sort>>,
  /// The parts in the order shown: the application sorts (D0022).
  pub(crate) order: Signal<Rc<Vec<u32>>>,
  /// Parts changed in the inspector.
  pub(crate) edits: Signal<HashMap<u32, Part>>,
  pub(crate) picked: Signal<Vec<u32>>,
  pub(crate) history: History,
  pub(crate) widths: Signal<Vec<f32>>,
}

impl State {
  pub(crate) fn part(
    self,
    i: u32,
  ) -> Part {
    self
      .edits
      .map(|e| e.get(&i).cloned())
      .unwrap_or_else(|| base_part(i))
  }

  /// Sorts the parts' order to match `sort`.
  fn sort_by(
    self,
    sort: Sort,
  ) {
    let mut order: Vec<u32> = (0..PARTS as u32).collect();
    let edits = self.edits.get();
    let part = |i: u32| {
      edits
        .get(&i)
        .cloned()
        .unwrap_or_else(|| base_part(i))
    };
    match sort.column {
      0 => {
        let names: Vec<String> = order.iter().map(|&i| part(i).name).collect();
        order.sort_by(|&a, &b| names[a as usize].cmp(&names[b as usize]));
      },
      1 => {
        let values: Vec<f32> = order.iter().map(|&i| part(i).value).collect();
        order
          .sort_by(|&a, &b| values[a as usize].total_cmp(&values[b as usize]));
      },
      2 => order.sort_by_key(|&i| part(i).footprint),
      _ => order.sort_by_key(|&i| part(i).side),
    }
    if sort.order == SortOrder::Descending {
      order.reverse();
    }
    self.order.set(Rc::new(order));
    self.sort.set(Some(sort));
  }
}

thread_local! {
  /// The state, for the self-test.
  pub(crate) static STATE: Cell<Option<State>> = const { Cell::new(None) };
}

/// A heading in the theme's title style.
#[component]
fn Heading(#[prop(into)] text: String) -> View {
  let t = theme();
  Element::new(ElementKind::Box)
    .style(Style {
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      padding: Edges::xy(0.0, t.spacing.of(1.0)),
      ..Style::default()
    })
    .role(Role::Heading)
    .children([View::text(text)])
    .into()
}

/// A panel: a raised column `width` wide, or as wide as is left.
fn panel(
  width: Option<f32>,
  children: Vec<View>,
) -> View {
  let t = theme();
  Element::new(ElementKind::Column)
    .style(Style {
      width: width.map_or(Size::Fit, Size::Fixed),
      height: Size::Fill,
      grow: if width.is_none() { 1.0 } else { 0.0 },
      gap: t.spacing.of(2.0),
      padding: Edges::all(t.spacing.of(3.0)),
      align: dovetail::Align::Stretch,
      background: t.colors.surface_container_low,
      radius: t.shapes.large,
      border: dovetail::Border {
        width: 1.0,
        color: t.colors.outline_variant,
      },
      clip: true,
      ..Style::default()
    })
    .children(children)
    .into()
}

/// The layer tree.
#[component]
fn Layers(state: State) -> View {
  let s = state;
  panel(
    Some(260.0),
    vec![
      ui! { Heading { text: "Layers" } },
      ui! {
        DataTree {
          nodes: |node: Option<&Key>| match node.and_then(Key::number) {
            None => (0..LAYERS).map(Key::from).collect(),
            Some(layer) => (0..SHAPES).map(|i| shape_key(layer, i)).collect(),
          },
          has_children: |k: &Key| k.number().is_some_and(|n| n < SHAPE_KEYS),
          row: |k: &Key, _: &RowState| View::text(node_name(k)),
          text: |k: &Key| node_name(k),
          selection: s.layers,
          expansion: Some(s.open),
          label: "Layers",
          height: Size::Fit,
        }
      },
    ],
  )
}

/// The table of parts.
#[component]
fn Parts(state: State) -> View {
  let s = state;
  let order = s.order.get();
  let edits = s.edits.get();
  let part = move |row: usize| {
    let i = order[row];
    edits
      .get(&i)
      .cloned()
      .unwrap_or_else(|| base_part(i))
  };
  let names = part.clone();
  let order_for_keys = s.order.get();
  let widths = s.widths.get();
  let columns = vec![
    TableColumn::new("Name", widths[0]),
    TableColumn::new("Value", widths[1]).at_end(),
    TableColumn::new("Footprint", widths[2]),
    TableColumn::new("Side", widths[3]),
    TableColumn::new("Locked", widths[4]).unsorted(),
  ];
  panel(
    None,
    vec![
      ui! { Heading { text: "Parts" } },
      ui! {
        DataTable {
          columns: columns,
          count: PARTS,
          cell: move |row: usize, column: usize, _: &RowState| {
            let p = part(row);
            View::text(match column {
              0 => p.name,
              1 => format!("{:.1}", p.value),
              2 => FOOTPRINTS[p.footprint].to_string(),
              3 => SIDES[p.side].to_string(),
              _ => if p.locked { "Yes".into() } else { String::new() },
            })
          },
          key: move |row: usize| Key::from(u64::from(order_for_keys[row])),
          text: move |row: usize| names(row).name,
          selection: s.parts,
          sort: s.sort.get(),
          on_sort: move |sort: Sort| s.sort_by(sort),
          on_resize: move |(c, w): (usize, f32)| s.widths.update(|v| v[c] = w),
          label: "Parts",
          draggable: true,
          height: Size::Fit,
        }
      },
    ],
  )
}

/// The inspector for the part the keyboard is on, the picked parts, and
/// the log.
#[component]
fn Side(state: State) -> View {
  let s = state;
  let current = s
    .parts
    .current()
    .and_then(|k| k.number())
    .map(|n| n as u32);
  let inspector: View = match current {
    Some(i) => {
      let p = s.part(i);
      let edit = move |f: Box<dyn Fn(&mut Part)>| {
        let mut p = s.part(i);
        f(&mut p);
        s.edits.update(|e| {
          e.insert(i, p);
        });
      };
      let (e1, e2, e3, e4, e5) = (edit, edit, edit, edit, edit);
      let sections = vec![
        PropertySection::new(
          "Part",
          vec![
            Property::text("Name", p.name.clone(), move |v| {
              e1(Box::new(move |p| p.name.clone_from(&v)))
            }),
            Property::number(
              "Value",
              p.value,
              (0.0, 1000.0, 0.5),
              1,
              move |v| e2(Box::new(move |p| p.value = v)),
            ),
          ],
        ),
        PropertySection::new(
          "Placement",
          vec![
            Property::choice(
              "Footprint",
              FOOTPRINTS.iter().map(|s| s.to_string()).collect(),
              p.footprint,
              move |v| e3(Box::new(move |p| p.footprint = v)),
            ),
            Property::choice(
              "Side",
              SIDES.iter().map(|s| s.to_string()).collect(),
              p.side,
              move |v| e4(Box::new(move |p| p.side = v)),
            ),
            Property::check("Locked", p.locked, move |v| {
              e5(Box::new(move |p| p.locked = v))
            }),
          ],
        ),
      ];
      ui! {
        PropertyGrid {
          sections: sections,
          history: s.history,
          name_width: 80.0,
          value_width: 170.0,
          label: "Inspector",
        }
      }
    },
    None => View::text("Choose a part in the table."),
  };
  let picked = s.picked.get();
  let picked_count = picked.len();
  let picked_names = picked.clone();
  let picked_keys = picked.clone();
  panel(
    Some(320.0),
    vec![
      ui! { Heading { text: "Inspector" } },
      inspector,
      ui! { Heading { text: "Picked" } },
      Element::new(ElementKind::Column)
        .style(Style {
          height: Size::Fixed(120.0),
          align: dovetail::Align::Stretch,
          border: dovetail::Border {
            width: 1.0,
            color: theme().colors.outline_variant,
          },
          radius: 4.0,
          ..Style::default()
        })
        .children([ui! {
          DataList {
            count: picked_count,
            row: move |i: usize, _: &RowState| View::text(s.part(picked_names[i]).name),
            key: move |i: usize| Key::from(u64::from(picked_keys[i])),
            selection: s.picked_rows,
            label: "Picked",
            draggable: true,
            on_drop: Some(Callback::new(move |d: RowsDropped| drop_parts(s, d))),
          }
        }])
        .into(),
      ui! { Heading { text: "Log" } },
      ui! {
        DataList {
          count: PARTS,
          row: |i: usize, _: &RowState| View::text(log_entry(i)),
          text: |i: usize| log_entry(i),
          selection: s.log,
          row_height: RowHeight::Measured { estimate: 26.0 },
          label: "Log",
          height: Size::Fit,
        }
      },
    ],
  )
}

/// Parts dropped on the Picked list: from the table they are added, and
/// from the list itself they move.
fn drop_parts(
  s: State,
  d: RowsDropped,
) {
  let ids: Vec<u32> = d
    .keys
    .iter()
    .filter_map(Key::number)
    .map(|n| n as u32)
    .collect();
  s.picked.update(|list| {
    let mut at = d.at.min(list.len());
    if d.source == "Picked" {
      for id in &ids {
        if let Some(p) = list.iter().position(|x| x == id) {
          list.remove(p);
          if p < at {
            at -= 1;
          }
        }
      }
    }
    for (k, id) in ids.iter().enumerate() {
      if d.source == "Picked" || !list.contains(id) {
        list.insert((at + k).min(list.len()), *id);
      }
    }
  });
}

#[component]
pub(crate) fn App() -> View {
  let state = State {
    dark: use_state(|| false),
    parts: use_state(|| Selection::new(SelectionMode::Multiple)).get(),
    layers: use_state(|| Selection::new(SelectionMode::Single)).get(),
    open: use_state(Expansion::new).get(),
    log: use_state(|| Selection::new(SelectionMode::Multiple)).get(),
    picked_rows: use_state(|| Selection::new(SelectionMode::Multiple)).get(),
    sort: use_state(|| None),
    order: use_state(|| Rc::new((0..PARTS as u32).collect())),
    edits: use_state(HashMap::new),
    picked: use_state(Vec::new),
    history: use_state(History::new).get(),
    widths: use_state(|| vec![110.0, 80.0, 100.0, 80.0, 70.0]),
  };
  STATE.set(Some(state));
  let commands =
    use_state(move || state.history.add_commands(Commands::new())).get();
  let data = if state.dark.get() {
    ThemeData::dark()
  } else {
    ThemeData::light()
  };
  let s = state;
  let bar = ui! {
    Row {
      Button { text: "Light", on_click: move |_| s.dark.set(false) }
      Button { text: "Dark", on_click: move |_| s.dark.set(true) }
      Button { text: "Undo", on_click: move |_| { s.history.undo(); }, enabled: s.history.can_undo() }
      Button { text: "Redo", on_click: move |_| { s.history.redo(); }, enabled: s.history.can_redo() }
      Text { value: format!("{} parts selected", s.parts.len()) }
    }
  };
  let main: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: 12.0,
      grow: 1.0,
      width: Size::Fill,
      align: dovetail::Align::Stretch,
      ..Style::default()
    })
    .children([
      ui! { Layers { state: state } },
      ui! { Parts { state: state } },
      ui! { Side { state: state } },
    ])
    .into();
  let window: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      gap: 12.0,
      padding: Edges::all(16.0),
      ..Style::default()
    })
    .children([bar, main])
    .into();
  ui! {
    Theme {
      data: data,
      CommandScope { commands: commands, { window } }
    }
  }
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().collect();
  if args.iter().any(|a| a == "--self-test") {
    return self_test::run();
  }
  if args.iter().any(|a| a == "--bench") {
    return bench::run();
  }
  match dovetail::run("Data views", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("data: {e}");
      ExitCode::FAILURE
    },
  }
}
