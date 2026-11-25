//! A list of rows that makes only those in sight (M16).

use dovetail::{Callback, ItemPlace, Role, Size, View};
use dovetail_macros::component;

use crate::body::{
  BodyProps, KeyFn, RowFn, RowSpec, RowState, RowsDropped, TextFn, body,
  use_body,
};
use crate::heights::RowHeight;
use crate::selection::Selection;

/// A list of `count` rows, of which only those in sight are made.
///
/// `row` builds the contents of a row from its index. `key` says which row
/// is which when the data changes, so the selection stays with its rows;
/// without it, a row's key is its index. `text` gives a row's text, so
/// typing its first letters moves to it.
///
/// Rows are chosen with the pointer and the keyboard, as each system does
/// (see [`Selection`]). Enter or a double click calls `on_activate`. With
/// `draggable`, rows can be dragged out, and with `on_drop`, rows dragged
/// from this view or another are dropped in.
#[component]
pub fn DataList(
  count: usize,
  #[prop(into)] row: RowFn,
  selection: Selection,
  #[prop(into, default)] key: KeyFn,
  #[prop(into, default)] text: TextFn,
  #[prop(default)] row_height: RowHeight,
  #[prop(into, default = "")] label: String,
  #[prop(into, default = "")] id: String,
  #[prop(default = Callback::new(|_: usize| {}))] on_activate: Callback<usize>,
  #[prop(default)] draggable: bool,
  #[prop(default)] on_drop: Option<Callback<RowsDropped>>,
  #[prop(default = Size::Fill)] width: Size,
  #[prop(default = Size::Fill)] height: Size,
) -> View {
  let state = use_body();
  let build = |index: usize, s: &RowState| RowSpec {
    role: Role::ListBoxOption,
    place: ItemPlace { index, count, level: None },
    content: vec![row.call(index, s)],
    expanded: None,
    label: None,
  };
  body(BodyProps {
    state,
    count,
    key_of: key.function(),
    text_of: text.function(),
    row: &build,
    row_height,
    selection,
    role: Role::ListBox,
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
    row_width: None,
    width,
    height,
    on_scrolled: None,
    table_size: None,
    tight: false,
  })
  .into()
}
