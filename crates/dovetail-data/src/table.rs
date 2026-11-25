//! A table: rows with columns under headers that resize and sort (M16).

use std::rc::Rc;

use dovetail::{
  Align, Border, Callback, Edges, Element, ElementKind, ItemPlace, Justify,
  KeyCode, KeyHandler, NamedKey, PointerEvent, PointerHandler, PointerKind,
  Role, ScrollRequest, Size, SortOrder, Style, View, Viewport, use_state,
};
use dovetail_macros::component;
use dovetail_theme::theme;

use crate::body::{
  BodyProps, KeyFn, RowSpec, RowState, RowsDropped, TextFn, body, use_body,
};
use crate::heights::RowHeight;
use crate::selection::Selection;

function_prop!(
  /// Builds the contents of the cell at a row and column.
  CellFn, (usize, usize, &RowState) -> View
);

/// One column of a table.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
  /// The header's text.
  pub title: String,
  /// How wide it starts, in logical pixels.
  pub width: f32,
  /// The narrowest it can be made.
  pub min_width: f32,
  /// Pressing the header sorts by it.
  pub sortable: bool,
  /// Where the contents go across the cell.
  pub justify: Justify,
}

impl Column {
  /// A sortable column with this title and width, its contents at the
  /// start.
  pub fn new(
    title: impl Into<String>,
    width: f32,
  ) -> Column {
    Column {
      title: title.into(),
      width,
      min_width: 32.0,
      sortable: true,
      justify: Justify::Start,
    }
  }

  /// The same column, with its contents at the end, as numbers usually
  /// are.
  pub fn at_end(mut self) -> Column {
    self.justify = Justify::End;
    self
  }

  /// The same column, which pressing its header does not sort by.
  pub fn unsorted(mut self) -> Column {
    self.sortable = false;
    self
  }
}

/// How a table is sorted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
  /// The column, from 0.
  pub column: usize,
  /// Which way.
  pub order: SortOrder,
}

/// How far Ctrl+Left and Ctrl+Right change a column's width.
const RESIZE_STEP: f32 = 16.0;
/// How wide the part of a header's edge that is dragged is.
const HANDLE: f32 = 6.0;

/// The sort a press on `column`'s header asks for: ascending, or the other
/// way when it is already sorted by that column.
fn next_sort(
  sort: Option<Sort>,
  column: usize,
) -> Sort {
  match sort {
    Some(s) if s.column == column => Sort {
      column,
      order: match s.order {
        SortOrder::Ascending => SortOrder::Descending,
        SortOrder::Descending => SortOrder::Ascending,
      },
    },
    _ => Sort { column, order: SortOrder::Ascending },
  }
}

/// A table of `count` rows under `columns`, of which only the rows in
/// sight are made. The header stays in place while the rows scroll up and
/// down, and moves with them across.
///
/// `cell` builds the contents of a cell from its row and column. `sort` is
/// how the application has sorted its data, shown by an arrow in the
/// header. Pressing a sortable header calls `on_sort` with the sort it
/// asks for, and the application sorts its data to match (D0022).
///
/// Dragging a header's right edge resizes its column. The header row is
/// one Tab stop: Left and Right move between headers, Enter or Space
/// sorts, and Ctrl+Left and Ctrl+Right (Command on macOS) make the column
/// narrower and wider. `on_resize` is told each column's new width.
///
/// Rows are chosen as in [`DataList`](crate::DataList).
#[component]
pub fn DataTable(
  columns: Vec<Column>,
  count: usize,
  #[prop(into)] cell: CellFn,
  selection: Selection,
  #[prop(into, default)] key: KeyFn,
  #[prop(into, default)] text: TextFn,
  #[prop(default)] sort: Option<Sort>,
  #[prop(default = Callback::new(|_: Sort| {}))] on_sort: Callback<Sort>,
  #[prop(default = Callback::new(|_: (usize, f32)| {}))] on_resize: Callback<(
    usize,
    f32,
  )>,
  #[prop(default)] row_height: RowHeight,
  #[prop(into, default = "")] label: String,
  #[prop(into, default = "")] id: String,
  #[prop(default = Callback::new(|_: usize| {}))] on_activate: Callback<usize>,
  #[prop(default)] draggable: bool,
  #[prop(default)] on_drop: Option<Callback<RowsDropped>>,
  #[prop(default = Size::Fill)] width: Size,
  #[prop(default = Size::Fill)] height: Size,
) -> View {
  let t = theme();
  let look = t.row_look();
  let state = use_body();
  let widths = use_state(|| {
    columns
      .iter()
      .map(|c| c.width)
      .collect::<Vec<_>>()
  });
  if widths.map(|w| w.len()) != columns.len() {
    widths.set(columns.iter().map(|c| c.width).collect());
  }
  let header_x = use_state(|| 0.0f32);
  let header_at = use_state(|| 0usize);
  let header_focused = use_state(|| false);
  // A drag on a header's edge: the column, the pointer's start and the
  // column's width then.
  let resizing = use_state(|| None::<(usize, f32, f32)>);
  let w = widths.get();
  let total: f32 = w.iter().sum();
  let view = state.viewport.get();
  let row_width = total.max(view.width);
  let columns: Rc<[Column]> = columns.into();
  let n = columns.len();

  let set_width = {
    let columns = columns.clone();
    let on_resize = on_resize.clone();
    Rc::new(move |c: usize, width: f32| {
      let width = width.max(columns[c].min_width).round();
      if widths.map(|w| w[c]) != width {
        widths.update(|w| w[c] = width);
        on_resize.call((c, width));
      }
    })
  };

  // The header row.
  let pad = t.spacing.of(2.0);
  let headers: Vec<View> = (0..n)
    .map(|c| {
      let col = &columns[c];
      let order = sort.filter(|s| s.column == c).map(|s| s.order);
      let arrow: View = match order {
        Some(SortOrder::Ascending) => dovetail_image::icons::arrow_up()
          .picture()
          .size(12.0)
          .into(),
        Some(SortOrder::Descending) => dovetail_image::icons::arrow_down()
          .picture()
          .size(12.0)
          .into(),
        None => View::Empty,
      };
      let handle = {
        let set_width = set_width.clone();
        Element::new(ElementKind::Box)
          .style(Style {
            width: Size::Fixed(HANDLE),
            height: Size::Fill,
            ..Style::default()
          })
          .role(Role::None)
          // Its own press, so a drag on the edge does not sort.
          .on_press(Callback::new(|()| {}))
          .on_pointer(PointerHandler::new(move |e: &PointerEvent| {
            match e.kind {
              PointerKind::Down => {
                resizing.set(Some((c, e.window_x, widths.map(|w| w[c]))))
              },
              PointerKind::Move => {
                if let Some((col, x, start)) = resizing.get()
                  && col == c
                {
                  set_width(c, start + e.window_x - x);
                }
              },
              _ => resizing.set(None),
            }
          }))
      };
      let mut header = Element::new(ElementKind::Row)
        .style(Style {
          width: Size::Fixed(w[c]),
          height: Size::Fill,
          padding: Edges { left: pad, ..Edges::default() },
          align: Align::Center,
          border: if header_focused.get() && header_at.get() == c {
            Border { width: 1.0, color: look.current }
          } else {
            Border::default()
          },
          clip: true,
          ..Style::default()
        })
        .role(Role::ColumnHeader)
        .column(c)
        .sorted(order)
        .label(col.title.clone())
        .children([
          Element::new(ElementKind::Row)
            .style(Style {
              grow: 1.0,
              justify: col.justify,
              align: dovetail::Align::Center,
              gap: 4.0,
              ..Style::default()
            })
            .children([View::text(col.title.clone()), arrow])
            .into(),
          // A thin line at the column's edge, in a wider part that is
          // dragged.
          handle
            .style(Style {
              width: Size::Fixed(HANDLE),
              height: Size::Fill,
              padding: Edges {
                left: HANDLE - 1.0,
                top: 6.0,
                bottom: 6.0,
                ..Edges::default()
              },
              ..Style::default()
            })
            .children([Element::new(ElementKind::Box)
              .style(Style {
                width: Size::Fixed(1.0),
                height: Size::Fill,
                background: look.line,
                ..Style::default()
              })
              .into()])
            .into(),
        ]);
      if col.sortable {
        let on_sort = on_sort.clone();
        header = header.on_press(Callback::new(move |()| {
          header_at.set(c);
          on_sort.call(next_sort(sort, c));
        }));
      }
      header.into()
    })
    .collect();
  let header_keys = {
    let columns = columns.clone();
    let on_sort = on_sort.clone();
    let set_width = set_width.clone();
    KeyHandler::new(move |event| {
      if n == 0 {
        return false;
      }
      let at = header_at.get().min(n - 1);
      let wide = event.modifiers.command();
      match &event.key {
        KeyCode::Named(NamedKey::ArrowLeft) if wide => {
          set_width(at, widths.map(|w| w[at]) - RESIZE_STEP)
        },
        KeyCode::Named(NamedKey::ArrowRight) if wide => {
          set_width(at, widths.map(|w| w[at]) + RESIZE_STEP)
        },
        KeyCode::Named(NamedKey::ArrowLeft) => {
          header_at.set(at.saturating_sub(1))
        },
        KeyCode::Named(NamedKey::ArrowRight) => {
          header_at.set((at + 1).min(n - 1))
        },
        KeyCode::Named(NamedKey::Home) => header_at.set(0),
        KeyCode::Named(NamedKey::End) => header_at.set(n - 1),
        KeyCode::Named(NamedKey::Enter | NamedKey::Space) => {
          if columns[at].sortable {
            on_sort.call(next_sort(sort, at));
          }
        },
        _ => return false,
      }
      true
    })
  };
  let x = header_x.get();
  let header = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fill,
      height: Size::Fixed(look.height + t.spacing.of(1.0)),
      background: look.header,
      font_weight: Some(dovetail::FontWeight::SEMIBOLD),
      text_color: Some(t.colors.on_surface_variant),
      clip: true,
      scroll: true,
      scrollbars: false,
      ..Style::default()
    })
    .scroll_to(ScrollRequest {
      serial: u64::from(x.to_bits()) + 1,
      x: Some(x),
      y: None,
    })
    .focusable(true)
    .on_key(header_keys)
    .on_focus(Callback::new(move |on: bool| header_focused.set(on)))
    .children([Element::new(ElementKind::Row)
      .style(Style {
        width: Size::Fixed(row_width),
        height: Size::Fill,
        ..Style::default()
      })
      .role(Role::Row)
      .item(ItemPlace { index: 0, count: count + 1, level: None })
      .children(headers)
      .into()]);

  let build = |index: usize, s: &RowState| RowSpec {
    role: Role::Row,
    // The header is the table's first row.
    place: ItemPlace {
      index: index + 1,
      count: count + 1,
      level: None,
    },
    content: (0..n)
      .map(|c| {
        Element::new(ElementKind::Row)
          .style(Style {
            width: Size::Fixed(w[c]),
            height: Size::Fill,
            padding: Edges::xy(pad, 0.0),
            align: Align::Center,
            justify: columns[c].justify,
            clip: true,
            ..Style::default()
          })
          .role(Role::Cell)
          .column(c)
          .children([(cell.0)(index, c, s)])
          .into()
      })
      .collect(),
    expanded: None,
    label: None,
  };
  let rows = body(BodyProps {
    state,
    count,
    key_of: key.function(),
    text_of: text.function(),
    row: &build,
    row_height,
    selection,
    role: Role::None,
    id: if id.is_empty() {
      label.clone()
    } else {
      id.clone()
    },
    label: label.clone(),
    on_activate: on_activate.clone(),
    extra_keys: None,
    draggable,
    on_drop: on_drop.clone(),
    row_width: Some(row_width),
    width: Size::Fill,
    height: Size::Fill,
    on_scrolled: Some(Callback::new(move |v: Viewport| {
      if header_x.get() != v.x {
        header_x.set(v.x);
      }
    })),
    table_size: None,
    tight: true,
  });
  Element::new(ElementKind::Column)
    .style(Style {
      width,
      height,
      grow: 1.0,
      align: Align::Stretch,
      ..Style::default()
    })
    .role(Role::Table)
    .label(label)
    .table_size(count + 1, n)
    .children([header.into(), rows.into()])
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_header_sorts_up_then_down() {
    let up = next_sort(None, 2);
    assert_eq!(up, Sort { column: 2, order: SortOrder::Ascending });
    let down = next_sort(Some(up), 2);
    assert_eq!(down.order, SortOrder::Descending);
    assert_eq!(next_sort(Some(down), 2).order, SortOrder::Ascending);
    assert_eq!(next_sort(Some(down), 0).order, SortOrder::Ascending);
  }
}
