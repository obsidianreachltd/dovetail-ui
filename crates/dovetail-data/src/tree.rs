//! A tree of rows that open and close, with children made only when a
//! node opens (M16).

use std::collections::HashSet;
use std::rc::Rc;

use dovetail::{
  Align, Callback, Edges, Element, ElementKind, ItemPlace, Key, KeyCode,
  KeyEvent, NamedKey, Role, Signal, Size, Style, View, use_state,
};
use dovetail_macros::component;
use dovetail_theme::theme;

use crate::body::{
  BodyProps, RowSpec, RowState, RowsDropped, body, scroll_into_view, use_body,
};
use crate::heights::RowHeight;
use crate::selection::Selection;

function_prop!(
  /// The children of a node, or the top-level nodes for `None`. A tree
  /// asks only for the children of open nodes.
  ChildrenFn, (Option<&Key>) -> Vec<Key>
);
function_prop!(
  /// Whether a node has children, without making them: closed nodes show
  /// that they can open.
  HasChildrenFn, (&Key) -> bool
);
function_prop!(
  /// Builds the contents of a node's row.
  NodeFn, (&Key, &RowState) -> View
);

/// A node's text, for finding rows by typing their first letters. Made
/// from a closure; the default has no text.
#[derive(Clone, Default)]
pub struct NodeTextFn(Option<NodeText>);

type NodeText = Rc<dyn Fn(&Key) -> String>;

impl PartialEq for NodeTextFn {
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

impl std::fmt::Debug for NodeTextFn {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    f.write_str("NodeTextFn")
  }
}

impl<F: Fn(&Key) -> String + 'static> From<F> for NodeTextFn {
  fn from(f: F) -> Self {
    NodeTextFn(Some(Rc::new(f)))
  }
}

/// Which nodes of a tree are open. It is `Copy`: make it while a component
/// builds, with `use_state(Expansion::new).get()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expansion(Signal<(HashSet<Key>, u64)>);

impl Expansion {
  /// Every node closed.
  pub fn new() -> Expansion {
    Expansion(dovetail::create_signal((HashSet::new(), 0)))
  }

  /// True when the node `key` is open.
  pub fn is_open(
    self,
    key: &Key,
  ) -> bool {
    self.0.map(|s| s.0.contains(key))
  }

  /// Opens or closes the node `key`.
  pub fn set(
    self,
    key: Key,
    open: bool,
  ) {
    if self.is_open(&key) != open {
      self.0.update(|s| {
        if open {
          s.0.insert(key);
        } else {
          s.0.remove(&key);
        }
        s.1 += 1;
      });
    }
  }

  /// Opens a closed node and closes an open one.
  pub fn toggle(
    self,
    key: Key,
  ) {
    let open = self.is_open(&key);
    self.set(key, !open);
  }

  /// How many nodes are open.
  pub fn len(self) -> usize {
    self.0.map(|s| s.0.len())
  }

  /// A number that changes whenever a node opens or closes.
  pub fn revision(self) -> u64 {
    self.0.map(|s| s.1)
  }

  /// True when no node is open.
  pub fn is_empty(self) -> bool {
    self.len() == 0
  }
}

impl Default for Expansion {
  fn default() -> Self {
    Expansion::new()
  }
}

/// One row of the tree as shown: a node and where it is.
#[derive(Clone, Debug)]
struct Shown {
  key: Key,
  /// From 1 at the top level.
  level: usize,
  /// The row of its parent.
  parent: Option<usize>,
  /// Its position among its parent's children, and how many there are.
  index: usize,
  siblings: usize,
  has_children: bool,
  open: bool,
}

/// The rows shown, kept between builds.
struct Flattened {
  revision: u64,
  nodes: ChildrenFn,
  has_children: HasChildrenFn,
  expansion: Expansion,
  rows: Rc<[Shown]>,
}

/// The rows shown: every node whose parents are all open, in order.
fn flatten(
  children: &ChildrenFn,
  has_children: &HasChildrenFn,
  expansion: Expansion,
) -> Vec<Shown> {
  let mut rows = Vec::new();
  // (parent row, level, the nodes left to show, their count), deepest
  // last.
  let roots = (children.0)(None);
  let mut stack: Vec<(Option<usize>, usize, std::vec::IntoIter<Key>, usize)> =
    vec![(None, 1, roots.clone().into_iter(), roots.len())];
  while let Some((parent, level, nodes, siblings)) = stack.last_mut() {
    let Some(key) = nodes.next() else {
      stack.pop();
      continue;
    };
    let (parent, level, siblings) = (*parent, *level, *siblings);
    let index = siblings - nodes.len() - 1;
    let has = (has_children.0)(&key);
    let open = has && expansion.is_open(&key);
    let row = rows.len();
    rows.push(Shown {
      key: key.clone(),
      level,
      parent,
      index,
      siblings,
      has_children: has,
      open,
    });
    if open {
      let kids = (children.0)(Some(&key));
      let n = kids.len();
      stack.push((Some(row), level + 1, kids.into_iter(), n));
    }
  }
  rows
}

/// A tree of nodes that open and close. Only the rows in sight are made,
/// and only open nodes are asked for their children.
///
/// `nodes` gives a node's children (the top-level nodes for `None`),
/// `has_children` says whether a closed node can open, and `row` builds a
/// node's contents. `expansion` holds which nodes are open; without one,
/// the tree keeps its own.
///
/// The tree asks for children again only when a node opens or closes, or
/// when it is given new functions, not while it scrolls. When the data
/// changes, give it new functions, as a parent that reads the data does
/// when it builds again.
///
/// Keys: Right opens a closed node and moves into an open one; Left closes
/// an open node and moves to the parent of any other. The other keys are
/// as in [`DataList`](crate::DataList). A click on the arrow, a double
/// click, or Enter opens and closes a node, and Enter and a double click
/// also call `on_activate` with the node.
#[component]
pub fn DataTree(
  #[prop(into)] nodes: ChildrenFn,
  #[prop(into)] has_children: HasChildrenFn,
  #[prop(into)] row: NodeFn,
  selection: Selection,
  #[prop(default)] expansion: Option<Expansion>,
  #[prop(into, default)] text: NodeTextFn,
  #[prop(default)] row_height: RowHeight,
  #[prop(into, default = "")] label: String,
  #[prop(into, default = "")] id: String,
  #[prop(default = Callback::new(|_: Key| {}))] on_activate: Callback<Key>,
  #[prop(default)] draggable: bool,
  #[prop(default)] on_drop: Option<Callback<RowsDropped>>,
  #[prop(default = Size::Fill)] width: Size,
  #[prop(default = Size::Fill)] height: Size,
) -> View {
  let own = use_state(Expansion::new).get();
  let expansion = expansion.unwrap_or(own);
  let state = use_body();
  let t = theme();
  // The rows shown are worked out again only when a node opens or closes,
  // or the application gives new functions: not as the tree scrolls.
  let revision = expansion.revision();
  let cache = use_state(|| None::<Flattened>);
  let fresh = cache.map(|c| {
    c.as_ref().is_some_and(|c| {
      c.revision == revision
        && c.nodes == nodes
        && c.has_children == has_children
        && c.expansion == expansion
    })
  });
  let rows: Rc<[Shown]> = if fresh {
    cache
      .map(|c| c.as_ref().map(|c| c.rows.clone()))
      .unwrap_or_default()
  } else {
    let rows: Rc<[Shown]> = flatten(&nodes, &has_children, expansion).into();
    cache.set(Some(Flattened {
      revision,
      nodes: nodes.clone(),
      has_children: has_children.clone(),
      expansion,
      rows: rows.clone(),
    }));
    rows
  };
  let count = rows.len();
  let key_of: Rc<dyn Fn(usize) -> Key> = {
    let rows = rows.clone();
    Rc::new(move |i| rows[i].key.clone())
  };
  let text_of: Option<Rc<dyn Fn(usize) -> String>> = text.0.clone().map(|f| {
    let rows = rows.clone();
    let f: Rc<dyn Fn(usize) -> String> = Rc::new(move |i| f(&rows[i].key));
    f
  });

  let indent = t.spacing.of(4.0);
  let muted = t.colors.on_surface_variant;
  let build = |index: usize, s: &RowState| {
    let r = &rows[index];
    let arrow: View = if r.has_children {
      let key = r.key.clone();
      Element::new(ElementKind::Row)
        .style(Style {
          width: Size::Fixed(indent),
          justify: dovetail::Justify::Center,
          align: Align::Center,
          text_color: Some(if s.highlighted() {
            t.row_look().selected_text
          } else {
            muted
          }),
          ..Style::default()
        })
        .on_press(Callback::new(move |()| expansion.toggle(key.clone())))
        .children([if r.open {
          dovetail_image::icons::chevron_down()
        } else {
          dovetail_image::icons::chevron_right()
        }
        .picture()
        .size(12.0)
        .into()])
        .into()
    } else {
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(indent),
          ..Style::default()
        })
        .into()
    };
    RowSpec {
      role: Role::TreeItem,
      place: ItemPlace {
        index: r.index,
        count: r.siblings,
        level: Some(r.level),
      },
      content: vec![
        Element::new(ElementKind::Box)
          .style(Style {
            width: Size::Fixed(indent * (r.level - 1) as f32),
            ..Style::default()
          })
          .into(),
        arrow,
        Element::new(ElementKind::Row)
          .style(Style {
            gap: t.spacing.of(1.5),
            align: Align::Center,
            padding: Edges::default(),
            ..Style::default()
          })
          .children([(row.0)(&r.key, s)])
          .into(),
      ],
      expanded: r.has_children.then_some(r.open),
      label: None,
    }
  };

  let extra = {
    let rows = rows.clone();
    let on_activate = on_activate.clone();
    Rc::new(move |event: &KeyEvent, at: Option<usize>| -> bool {
      let Some(at) = at else {
        return false;
      };
      let r = &rows[at];
      if event.modifiers.shift || event.modifiers.command() {
        return false;
      }
      match &event.key {
        KeyCode::Named(NamedKey::ArrowRight) => {
          if r.has_children && !r.open {
            expansion.set(r.key.clone(), true);
          } else if r.open && at + 1 < rows.len() {
            selection.select_only(at + 1, rows[at + 1].key.clone());
            scroll_into_view(state, at + 1);
          }
          true
        },
        KeyCode::Named(NamedKey::ArrowLeft) => {
          if r.open {
            expansion.set(r.key.clone(), false);
          } else if let Some(p) = r.parent {
            selection.select_only(p, rows[p].key.clone());
            scroll_into_view(state, p);
          }
          true
        },
        KeyCode::Named(NamedKey::Enter) => {
          if r.has_children {
            expansion.toggle(r.key.clone());
          }
          on_activate.call(r.key.clone());
          true
        },
        _ => false,
      }
    }) as crate::body::ExtraKeys
  };
  let activate = {
    let rows = rows.clone();
    let on_activate = on_activate.clone();
    Callback::new(move |i: usize| {
      let r = &rows[i];
      if r.has_children {
        expansion.toggle(r.key.clone());
      }
      on_activate.call(r.key.clone());
    })
  };
  body(BodyProps {
    state,
    count,
    key_of,
    text_of,
    row: &build,
    row_height,
    selection,
    role: Role::Tree,
    id: if id.is_empty() {
      label.clone()
    } else {
      id.clone()
    },
    label: label.clone(),
    on_activate: activate,
    extra_keys: Some(extra),
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
