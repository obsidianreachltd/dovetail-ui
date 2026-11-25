//! The scrolling body that lists, trees and tables share (M16, D0022).
//!
//! It is a scrolling column that makes only the rows in sight, and a few
//! more, between two spacers as tall as the rows above and below. Layout
//! tells it where it is scrolled to (`Element::on_scroll`), and it moves
//! itself to show the row the keyboard is on (`Element::scroll_to`).
//!
//! Focus stays on the body. The row the keyboard is on is drawn with a
//! line around it, and marked `current` for assistive tools.

use std::fmt;
use std::rc::Rc;

use dovetail::{
  Align, Border, Callback, Color, DragData, DragEnd, DropEvent, DropHandler,
  DropKind, Edges, Element, ElementKind, ItemPlace, Key, KeyEvent, KeyHandler,
  Modifiers, PointerEvent, PointerHandler, PointerKind, Role, ScrollRequest,
  Signal, Size, Style, View, Viewport, start_drag, use_state,
};
use dovetail_theme::{RowLook, theme};

use crate::heights::{Heights, RowHeight};
use crate::selection::Selection;

/// What a row is told when it is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowState {
  /// Its position among all the rows.
  pub index: usize,
  /// It is selected.
  pub selected: bool,
  /// The keyboard is on it.
  pub current: bool,
  /// Its view has focus.
  pub focused: bool,
}

impl RowState {
  /// True when the row is drawn as selected in a view with focus, so its
  /// text should take the selected text colour.
  pub fn highlighted(&self) -> bool {
    self.selected && self.focused
  }
}

function_prop!(
  /// Builds the contents of row `index`.
  RowFn, (usize, &RowState) -> View
);
function_prop!(
  /// The key of row `index`, which stays with its row when the data
  /// changes. Without one, a row's key is its index.
  KeyFn, (usize) -> Key
);

impl RowFn {
  /// Builds row `index`.
  pub fn call(
    &self,
    index: usize,
    state: &RowState,
  ) -> View {
    (self.0)(index, state)
  }
}

impl KeyFn {
  pub(crate) fn function(&self) -> Rc<dyn Fn(usize) -> Key> {
    self.0.clone()
  }

  /// The key of row `index`.
  pub fn call(
    &self,
    index: usize,
  ) -> Key {
    (self.0)(index)
  }
}

impl Default for KeyFn {
  fn default() -> Self {
    KeyFn::from(Key::from)
  }
}

/// The text of row `index`, for finding rows by typing their first
/// letters. Made from a closure; the default has no text, and typing finds
/// nothing.
#[derive(Clone, Default)]
pub struct TextFn(Option<Rc<dyn Fn(usize) -> String>>);

impl TextFn {
  /// The text of row `index`, or `None` without a function.
  pub fn call(
    &self,
    index: usize,
  ) -> Option<String> {
    self.0.as_ref().map(|f| f(index))
  }

  pub(crate) fn function(&self) -> Option<Rc<dyn Fn(usize) -> String>> {
    self.0.clone()
  }
}

impl PartialEq for TextFn {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    match (&self.0, &other.0) {
      (Some(a), Some(b)) => Rc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    }
  }
}

impl fmt::Debug for TextFn {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("TextFn")
  }
}

impl<F: Fn(usize) -> String + 'static> From<F> for TextFn {
  fn from(f: F) -> Self {
    TextFn(Some(Rc::new(f)))
  }
}

/// What is dragged when rows are dragged out of a view (M16).
#[derive(Clone, Debug, PartialEq)]
pub struct RowDrag {
  /// The view they come from: its `id`.
  pub source: String,
  /// The rows' keys, in order.
  pub keys: Vec<Key>,
}

/// Rows dropped on a view (M16).
#[derive(Clone, Debug, PartialEq)]
pub struct RowsDropped {
  /// The view they come from, which may be this one.
  pub source: String,
  /// The rows' keys, in order.
  pub keys: Vec<Key>,
  /// Where they go: before the row at this index, or after the last row
  /// when it is the number of rows.
  pub at: usize,
}

/// The state a body keeps between builds.
#[derive(Clone, Copy)]
pub(crate) struct BodyState {
  pub viewport: Signal<Viewport>,
  pub heights: Signal<Heights>,
  pub focused: Signal<bool>,
  pub request: Signal<ScrollRequest>,
  /// Where dropped rows would go, while a drag is over the body.
  pub drop_at: Signal<Option<usize>>,
  /// A press that may become a drag: where, on which row, and whether the
  /// click should select only that row when let go without dragging.
  pub press: Signal<Option<(f32, f32, usize, bool)>>,
  pub hover: Signal<Option<usize>>,
}

/// The state a body keeps, made while its component builds.
pub(crate) fn use_body() -> BodyState {
  BodyState {
    viewport: use_state(Viewport::default),
    heights: use_state(Heights::default),
    focused: use_state(|| false),
    request: use_state(ScrollRequest::default),
    drop_at: use_state(|| None),
    press: use_state(|| None),
    hover: use_state(|| None),
  }
}

/// How far the pointer moves with the button held before rows are
/// dragged, in logical pixels.
const DRAG_DISTANCE: f32 = 4.0;

/// How a view's row looks and what it is, from the view.
pub(crate) struct RowSpec {
  /// What the row is: an option, a tree item or a table row.
  pub role: Role,
  /// Where it is among its fellows, for assistive tools.
  pub place: ItemPlace,
  /// Its contents, laid out in a row.
  pub content: Vec<View>,
  /// Open or closed, for a tree item with children.
  pub expanded: Option<bool>,
  /// Its name for assistive tools, when its text is not enough.
  pub label: Option<String>,
}

/// Keys a view handles before the body, with the row the keyboard is on.
pub(crate) type ExtraKeys = Rc<dyn Fn(&KeyEvent, Option<usize>) -> bool>;

/// What a view gives the body.
pub(crate) struct BodyProps<'a> {
  pub state: BodyState,
  pub count: usize,
  pub key_of: Rc<dyn Fn(usize) -> Key>,
  pub text_of: Option<Rc<dyn Fn(usize) -> String>>,
  pub row: &'a dyn Fn(usize, &RowState) -> RowSpec,
  pub row_height: RowHeight,
  pub selection: Selection,
  pub role: Role,
  pub label: String,
  /// The view's id, for rows dragged out of it.
  pub id: String,
  pub on_activate: Callback<usize>,
  /// Keys the view handles before the body: a tree opens and closes rows.
  pub extra_keys: Option<ExtraKeys>,
  pub draggable: bool,
  pub on_drop: Option<Callback<RowsDropped>>,
  /// How wide each row is, when wider than the view: a table's columns.
  pub row_width: Option<f32>,
  /// What a table puts before the rows: nothing for lists and trees.
  pub width: Size,
  pub height: Size,
  /// Told where the body is scrolled to, as a table's header follows it.
  pub on_scrolled: Option<Callback<Viewport>>,
  /// How many rows and columns, for a table.
  pub table_size: Option<(usize, usize)>,
  /// Rows have no padding or gaps, so cells line up with headers.
  pub tight: bool,
}

/// The theme's row height, or the view's own.
fn heights_for(
  mode: RowHeight,
  look: &RowLook,
) -> (Option<f32>, f32) {
  match mode {
    RowHeight::Theme => (Some(look.height), look.height),
    RowHeight::Fixed(h) => (Some(h.max(1.0)), h.max(1.0)),
    RowHeight::Measured { estimate } => (None, estimate),
  }
}

/// Asks the body to scroll so row `index` is in sight.
pub(crate) fn scroll_into_view(
  state: BodyState,
  index: usize,
) {
  let view = state.viewport.get();
  let (top, bottom) = state
    .heights
    .map(|h| (h.top(index), h.top(index) + h.height(index)));
  let y = if top < view.y {
    top
  } else if bottom > view.y + view.height {
    bottom - view.height
  } else {
    return;
  };
  state.request.update(|r| {
    r.serial += 1;
    r.y = Some(y);
    r.x = None;
  });
}

/// How many rows fit in the body: how far Page Up and Page Down move.
fn page_rows(state: BodyState) -> usize {
  let view = state.viewport.get();
  state.heights.map(|h| {
    let first = h.index_at(view.y);
    let last = h.index_at(view.y + view.height - 1.0);
    last.saturating_sub(first).max(1)
  })
}

/// The body: a scrolling column with only the rows in sight.
pub(crate) fn body(p: BodyProps<'_>) -> Element {
  let t = theme();
  let look = t.row_look();
  let s = p.state;
  let (fixed, estimate) = heights_for(p.row_height, &look);
  // New counts or a new height mode start the heights again; measured
  // rows that are still there keep theirs.
  let stale = s.heights.map(|h| match fixed {
    Some(f) => h.measured() || h.height(0) != f,
    None => !h.measured() || h.estimate() != estimate.max(1.0),
  });
  if stale {
    s.heights
      .set(Heights::new(p.count, fixed, estimate));
  } else if s.heights.map(|h| h.len()) != p.count {
    s.heights.update(|h| h.resize(p.count));
  }

  let view = s.viewport.get();
  let focused = s.focused.get();
  let drop_at = s.drop_at.get();
  let hover = s.hover.get();
  let current = p.selection.current_index(p.count, &*p.key_of);
  // Rows in sight and half a view more on each side. Before the first
  // layout the view's height is not known: make enough to fill a window.
  let height = if view.height > 0.0 {
    view.height
  } else {
    600.0
  };
  let range = s
    .heights
    .map(|h| h.in_sight(view.y, height, (height / 2.0).max(60.0)));
  let (top, bottom) = s
    .heights
    .map(|h| (h.top(range.start), h.total() - h.top(range.end)));
  let measured = fixed.is_none();

  let mut children: Vec<View> = Vec::with_capacity(range.len() + 2);
  children.push(spacer(top, false, look.line).into());
  for index in range.clone() {
    let key = (p.key_of)(index);
    let state = RowState {
      index,
      selected: p.selection.is_selected(&key),
      current: current == Some(index),
      focused,
    };
    let spec = (p.row)(index, &state);
    let el = row_element(
      &p,
      &look,
      (fixed, measured),
      (index, key.clone()),
      &state,
      spec,
      (drop_at == Some(index), hover == Some(index)),
    );
    // Each row is a component with its row's key, so what is inside keeps
    // its state while rows come and go (M02).
    children.push(View::component::<RowHost>(Rc::new(el)).keyed(key));
  }
  children.push(
    spacer(bottom, drop_at == Some(p.count) && p.count > 0, look.line).into(),
  );

  let count = p.count;
  let selection = p.selection;
  let keys = {
    let key_of = p.key_of.clone();
    let texts = p.text_of.clone();
    let on_activate = p.on_activate.clone();
    let extra = p.extra_keys.clone();
    KeyHandler::new(move |event| {
      let at = selection.current_index(count, &*key_of);
      if let Some(extra) = &extra
        && extra(event, at)
      {
        return true;
      }
      if event.is(dovetail::NamedKey::Enter) {
        if let Some(i) = at {
          on_activate.call(i);
          return true;
        }
        return false;
      }
      let page = page_rows(s);
      let moved = selection.key(event, count, page, &*key_of, texts.as_deref());
      if let Some(i) = moved {
        scroll_into_view(s, i);
        true
      } else {
        false
      }
    })
  };

  let on_scrolled = p.on_scrolled.clone();
  let mut el = Element::new(ElementKind::Column)
    .style(Style {
      width: p.width,
      height: p.height,
      grow: 1.0,
      align: if p.row_width.is_some() {
        Align::Start
      } else {
        Align::Stretch
      },
      clip: true,
      scroll: true,
      ..Style::default()
    })
    .role(p.role)
    .label(p.label.clone())
    .multiselect(selection.mode() == crate::SelectionMode::Multiple)
    .focusable(true)
    .on_key(keys)
    .on_focus(Callback::new(move |on: bool| s.focused.set(on)))
    .on_scroll(Callback::new(move |v: Viewport| {
      s.viewport.set(v);
      if let Some(f) = &on_scrolled {
        f.call(v);
      }
    }))
    .on_hover(Callback::new(move |over: bool| {
      if !over {
        s.hover.set(None);
      }
    }))
    .scroll_to(s.request.get())
    .children(children);
  if let Some((rows, columns)) = p.table_size {
    el = el.table_size(rows, columns);
  }
  if let Some(on_drop) = p.on_drop.clone() {
    el = el.on_drop(drop_handler(s, count, on_drop));
  }
  el
}

/// A row, as a component so it can be keyed.
pub(crate) struct RowHost;

impl dovetail::Component for RowHost {
  type Props = Rc<Element>;

  fn build(
    props: &Rc<Element>,
    _: &mut dovetail::BuildContext<'_>,
  ) -> View {
    View::Element(props.clone())
  }
}

/// A spacer as tall as the rows it stands for, with a line at its top
/// when dropped rows would go there.
fn spacer(
  height: f32,
  line: bool,
  colour: Color,
) -> Element {
  let mut e = Element::new(ElementKind::Column).style(Style {
    height: Size::Fixed(height.max(if line { 2.0 } else { 0.0 })),
    ..Style::default()
  });
  if line {
    e = e.children([drop_line(colour).into()]);
  }
  e
}

fn drop_line(colour: Color) -> Element {
  Element::new(ElementKind::Box).style(Style {
    width: Size::Fill,
    height: Size::Fixed(2.0),
    background: colour,
    ..Style::default()
  })
}

/// One row, around the view's contents.
fn row_element(
  p: &BodyProps<'_>,
  look: &RowLook,
  (fixed, measured): (Option<f32>, bool),
  (index, key): (usize, Key),
  state: &RowState,
  spec: RowSpec,
  (drop_before, hovered): (bool, bool),
) -> Element {
  let s = p.state;
  let background = match (state.selected, state.focused) {
    (true, true) => look.selected,
    (true, false) => look.selected_idle,
    _ if hovered => look.hover,
    _ => Color::TRANSPARENT,
  };
  let text_color = state.highlighted().then_some(look.selected_text);
  let border = if state.current && state.focused {
    Border { width: 1.0, color: look.current }
  } else {
    Border::default()
  };
  let t = theme();
  let inner = Element::new(ElementKind::Row)
    .style(Style {
      width: p.row_width.map_or(Size::Fill, Size::Fixed),
      height: fixed.map_or(Size::Fit, Size::Fixed),
      min_height: if measured { 0.0 } else { fixed.unwrap_or(0.0) },
      padding: if p.tight {
        Edges::default()
      } else {
        Edges::xy(t.spacing.of(2.0), 0.0)
      },
      gap: if p.tight { 0.0 } else { t.spacing.of(1.5) },
      align: Align::Center,
      background,
      border,
      radius: if p.tight {
        0.0
      } else {
        t.shapes.small.min(4.0)
      },
      text_color,
      ..Style::default()
    })
    .children(spec.content);
  let mut children: Vec<View> = vec![inner.into()];
  if drop_before {
    children.push(drop_line(look.line).into());
  }

  let count = p.count;
  let selection = p.selection;
  let on_activate = p.on_activate.clone();
  let draggable = p.draggable;
  let id = p.id.clone();
  let keys_for_drag = {
    let key = key.clone();
    let key_of = p.key_of.clone();
    move || {
      if selection.is_selected(&key) {
        // In the order they are shown.
        (0..count)
          .map(|i| key_of(i))
          .filter(|k| selection.is_selected(k))
          .collect()
      } else {
        vec![key.clone()]
      }
    }
  };
  let key_for_click = key.clone();
  let key_of = p.key_of.clone();
  let pointer = PointerHandler::new(move |e: &PointerEvent| match e.kind {
    PointerKind::Down => {
      if e.button != dovetail::PointerButton::Primary {
        return;
      }
      if e.clicks >= 2 {
        on_activate.call(index);
        return;
      }
      let plain = e.modifiers == Modifiers::default();
      // A plain press on a selected row waits for the release, so the
      // selection can be dragged.
      let wait = plain && draggable && selection.is_selected(&key_for_click);
      if !wait {
        selection.click(index, e.modifiers, count, &*key_of);
      }
      s.press
        .set(Some((e.window_x, e.window_y, index, wait)));
    },
    PointerKind::Move => {
      if let Some((x, y, at, _)) = s.press.get()
        && at == index
        && draggable
        && ((e.window_x - x).abs() > DRAG_DISTANCE
          || (e.window_y - y).abs() > DRAG_DISTANCE)
      {
        s.press.set(None);
        start_drag(
          DragData::new(RowDrag {
            source: id.clone(),
            keys: keys_for_drag(),
          }),
          Callback::new(|_: DragEnd| {}),
        );
      }
    },
    PointerKind::Up => {
      if let Some((_, _, at, true)) = s.press.get()
        && at == index
      {
        selection.select_only(index, key_for_click.clone());
      }
      s.press.set(None);
    },
  });

  let mut row = Element::new(ElementKind::Stack)
    .style(Style {
      width: p.row_width.map_or(Size::Fit, Size::Fixed),
      ..Style::default()
    })
    .role(spec.role)
    .item(spec.place)
    .selected(state.selected)
    .current(state.current)
    .on_pointer(pointer)
    .on_hover(Callback::new(move |over: bool| {
      if over {
        s.hover.set(Some(index));
      }
    }))
    .children(children);
  if let Some(label) = spec.label {
    row = row.label(label);
  }
  if let Some(open) = spec.expanded {
    row = row.expanded(open);
  }
  if measured {
    let key = key.clone();
    row = row.on_size(Callback::new(move |(_, h): (f32, f32)| {
      let changed = s.heights.map(|hs| {
        !hs.is_measured(index, &key) || (hs.height(index) - h).abs() >= 0.5
      });
      if changed {
        s.heights.update(|hs| {
          hs.set(index, key.clone(), h);
        });
      }
    }));
  }
  row
}

/// Receives rows dragged over the body.
fn drop_handler(
  s: BodyState,
  count: usize,
  on_drop: Callback<RowsDropped>,
) -> DropHandler {
  DropHandler::new(move |e: &DropEvent| {
    let Some(rows) = e.data.get::<RowDrag>() else {
      return false;
    };
    let view = s.viewport.get();
    let y = e.y + view.y;
    let at = s.heights.map(|h| {
      if count == 0 {
        return 0;
      }
      let i = h.index_at(y);
      if y > h.top(i) + h.height(i) / 2.0 {
        (i + 1).min(count)
      } else {
        i
      }
    });
    match e.kind {
      DropKind::Over => {
        if s.drop_at.get() != Some(at) {
          s.drop_at.set(Some(at));
        }
        true
      },
      DropKind::Leave => {
        s.drop_at.set(None);
        false
      },
      DropKind::Drop => {
        s.drop_at.set(None);
        on_drop.call(RowsDropped {
          source: rows.source.clone(),
          keys: rows.keys.clone(),
          at,
        });
        true
      },
    }
  })
}
