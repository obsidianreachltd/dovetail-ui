//! What assistive tools are told about a window (M07, M20, D0027).
//!
//! [`Session::access_tree`] describes every node in the window's
//! accessibility tree: its role, name, value, states and actions, and its
//! children. `dovetail-desktop` turns the description into AccessKit's
//! types for screen readers; tests search it by role and label.
//!
//! The tree is built from the laid-out boxes:
//!
//! - the window is the root, named by its title;
//! - an element with a role, or one that can take focus or be pressed,
//!   becomes a node; other elements pass their children up;
//! - text becomes a label, except inside roles named by their contents,
//!   such as buttons, which take their name from it, and text fields,
//!   whose text is their value.
//!
//! A text field also says which box holds its text and where the
//! selection is, so a screen reader's view of it can follow the caret.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use dovetail_core::{Element, Role, SortOrder};
use dovetail_layout::{BoxKey, Content, Layout, LayoutBox, Rect};

use crate::Session;

/// A node's id: stable while its element stays, so only nodes that change
/// need to be sent again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AccessId(pub u64);

impl AccessId {
  /// The window's own node.
  pub const ROOT: AccessId = AccessId(0);

  /// The id of part `part` of box `key`'s node: 0 for the node itself.
  /// Never the root's.
  pub fn of(
    key: &BoxKey,
    part: u32,
  ) -> Self {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    part.hash(&mut h);
    AccessId(h.finish().max(1))
  }
}

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
  /// The window, the root of the tree.
  Window,
  /// Text, read as it is.
  Label,
  /// A picture with a label.
  Image,
  /// An element, with its role.
  Element(Role),
}

/// Whether a node is switched on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
  /// Off.
  Off,
  /// On.
  On,
  /// Partly on, as a check box over a mixed selection.
  Mixed,
}

/// A number a node shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Numeric {
  /// A value that can be set within a range, as a slider's.
  Range {
    /// The value.
    value: f32,
    /// The smallest it can be.
    min: f32,
    /// The largest it can be.
    max: f32,
    /// How far one step moves it, when it moves in steps.
    step: Option<f32>,
  },
  /// A share from 0 to 1, as a progress bar's, read as a percentage.
  Fraction(f32),
}

/// How far a node is scrolled one way, and how far it can go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
  /// Where it is scrolled to, in logical pixels.
  pub at: f32,
  /// The furthest it can go.
  pub max: f32,
}

/// What an assistive tool can ask a node to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions {
  /// Take keyboard focus.
  pub focus: bool,
  /// Be pressed.
  pub click: bool,
  /// Step its number up.
  pub increment: bool,
  /// Step its number down.
  pub decrement: bool,
  /// Have its value set.
  pub set_value: bool,
  /// Open, by a press.
  pub expand: bool,
  /// Close, by a press.
  pub collapse: bool,
  /// Have its text selection set and the selection replaced.
  pub edit_text: bool,
}

/// Where a text field's text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextField {
  /// The box that holds the text, in [`Layout::boxes`].
  pub text_box: usize,
  /// The selection, as byte offsets in the text: where it started, and
  /// the caret. They are equal when nothing is selected.
  pub selection: Option<(usize, usize)>,
}

/// One node of the accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
  /// Its id.
  pub id: AccessId,
  /// What it is.
  pub kind: AccessKind,
  /// The element or text it stands for; `None` for the window.
  pub key: Option<BoxKey>,
  /// Where it is, in logical pixels; `None` for the window.
  pub bounds: Option<Rect>,
  /// Its name: what a screen reader calls it.
  pub label: Option<String>,
  /// Its value, as the text of a label or a text field.
  pub value: Option<String>,
  /// More about it, as a tooltip says.
  pub description: Option<String>,
  /// What it can be asked to do.
  pub actions: Actions,
  /// Whether it is switched on.
  pub toggled: Option<Toggle>,
  /// Whether it is selected, as a tab, an option or a row.
  pub selected: Option<bool>,
  /// Whether it is open, as a drop-down or a tree item.
  pub expanded: Option<bool>,
  /// The number it shows.
  pub numeric: Option<Numeric>,
  /// How far it is scrolled sideways.
  pub scroll_x: Option<Scroll>,
  /// How far it is scrolled down.
  pub scroll_y: Option<Scroll>,
  /// Its place among the items of a list, tree or table: counted from 1.
  pub position_in_set: Option<usize>,
  /// How many items there are.
  pub size_of_set: Option<usize>,
  /// Its depth in a tree, counted from 1.
  pub level: Option<usize>,
  /// A row's index in its table, counted from 0.
  pub row_index: Option<usize>,
  /// More than one of its items can be selected.
  pub multiselectable: bool,
  /// A table's rows and columns.
  pub table_size: Option<(usize, usize)>,
  /// A cell's or header's column, counted from 0.
  pub column_index: Option<usize>,
  /// A column header's sort order.
  pub sorted: Option<SortOrder>,
  /// It is a dialog that holds the keyboard until it closes.
  pub modal: bool,
  /// It is shown but cannot be used now.
  pub disabled: bool,
  /// The current item inside a view that keeps focus itself (M16).
  pub active_descendant: Option<AccessId>,
  /// Its children, in order.
  pub children: Vec<AccessId>,
  /// Where a text field's text is.
  pub text_field: Option<TextField>,
}

impl AccessNode {
  fn new(
    id: AccessId,
    kind: AccessKind,
    key: Option<BoxKey>,
    bounds: Option<Rect>,
  ) -> Self {
    Self {
      id,
      kind,
      key,
      bounds,
      label: None,
      value: None,
      description: None,
      actions: Actions::default(),
      toggled: None,
      selected: None,
      expanded: None,
      numeric: None,
      scroll_x: None,
      scroll_y: None,
      position_in_set: None,
      size_of_set: None,
      level: None,
      row_index: None,
      multiselectable: false,
      table_size: None,
      column_index: None,
      sorted: None,
      modal: false,
      disabled: false,
      active_descendant: None,
      children: Vec::new(),
      text_field: None,
    }
  }

  /// Its role, when it is an element.
  pub fn role(&self) -> Option<Role> {
    match self.kind {
      AccessKind::Element(role) => Some(role),
      _ => None,
    }
  }
}

/// A window's accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessTree {
  /// Every node. A node comes after its children, and the window comes
  /// last.
  pub nodes: Vec<AccessNode>,
  /// The node with keyboard focus; the window when no element has it.
  pub focus: AccessId,
}

impl AccessTree {
  /// The node with id `id`.
  pub fn get(
    &self,
    id: AccessId,
  ) -> Option<&AccessNode> {
    self.nodes.iter().find(|n| n.id == id)
  }

  /// The window's node.
  pub fn root(&self) -> &AccessNode {
    self
      .nodes
      .last()
      .expect("the tree always has the window")
  }
}

/// The text inside box `index`, joined with spaces.
fn text_of(
  boxes: &[LayoutBox],
  index: usize,
) -> String {
  let mut parts = Vec::new();
  let mut stack = vec![index];
  while let Some(i) = stack.pop() {
    match &boxes[i].content {
      Content::Text { text, .. } => parts.push(text.to_string()),
      // A picture with a label is named by it, as an icon-only button is
      // (M17); decoration adds nothing.
      Content::Picture { picture, .. } => {
        if let Some(label) = &picture.label {
          parts.push(label.to_string());
        }
      },
      Content::Element(_) => {
        stack.extend(boxes[i].children.iter().rev());
      },
    }
  }
  parts.join(" ")
}

/// Roles whose name is their text, so their text is not read again.
pub fn named_by_contents(role: Role) -> bool {
  matches!(
    role,
    Role::Button
      | Role::Switch
      | Role::CheckBox
      | Role::Heading
      | Role::MenuItem
      | Role::ListBoxOption
      | Role::Tab
      | Role::RadioButton
      | Role::Link
      | Role::Tooltip
      | Role::TreeItem
      | Role::Cell
      | Role::ColumnHeader
  )
}

fn is_text_input(role: Role) -> bool {
  matches!(role, Role::TextInput | Role::MultilineTextInput)
}

/// Builds the nodes, one tree walk.
struct Builder<'a> {
  boxes: &'a [LayoutBox],
  layout: &'a Layout,
  /// A description from an element with no node, for the next node made
  /// inside it.
  description: Option<String>,
  nodes: Vec<AccessNode>,
  /// The node of each element, to find the focused one.
  by_key: HashMap<BoxKey, AccessId>,
  /// The node of the last current item seen and not yet given to the
  /// focusable view around it (M16).
  current: Option<AccessId>,
}

impl Builder<'_> {
  /// The nodes for box `index`, to go in its parent's children.
  fn visit(
    &mut self,
    index: usize,
  ) -> Vec<AccessId> {
    let b = &self.boxes[index];
    match &b.content {
      // Empty text, such as a blank label, has nothing to read.
      Content::Text { text, .. } if text.trim().is_empty() => Vec::new(),
      // A picture with a label is an image; one without is decoration,
      // which assistive tools skip (M17).
      Content::Picture { picture, .. } => match &picture.label {
        Some(label) => {
          let id = AccessId::of(&b.key, 0);
          let mut node = AccessNode::new(
            id,
            AccessKind::Image,
            Some(b.key.clone()),
            Some(b.rect),
          );
          node.label = Some(label.to_string());
          self.nodes.push(node);
          vec![id]
        },
        None => Vec::new(),
      },
      Content::Text { text, .. } => {
        let id = AccessId::of(&b.key, 0);
        let mut node = AccessNode::new(
          id,
          AccessKind::Label,
          Some(b.key.clone()),
          Some(b.rect),
        );
        node.value = Some(text.to_string());
        self.nodes.push(node);
        vec![id]
      },
      Content::Element(e) => self.element(index, e),
    }
  }

  fn element(
    &mut self,
    index: usize,
    e: &Element,
  ) -> Vec<AccessId> {
    let b = &self.boxes[index];
    let is_node =
      e.role != Role::None || e.focusable || e.on_press.is_some() || e.disabled;
    if !is_node {
      // A description with no node of its own goes to the first node
      // inside, as a tooltip's does to what it wraps.
      let outer = self.description.clone();
      if self.description.is_none() {
        self.description.clone_from(&e.description);
      }
      let nodes = self.children(index);
      self.description = outer;
      return nodes;
    }
    let id = AccessId::of(&b.key, 0);
    let mut node = AccessNode::new(
      id,
      AccessKind::Element(e.role),
      Some(b.key.clone()),
      Some(b.rect),
    );
    node.actions.focus = e.focusable;
    node.actions.click = e.on_press.is_some();
    match (e.role, e.toggled) {
      // A tab and a radio button are selected rather than switched on.
      (Role::Tab | Role::RadioButton | Role::ListBoxOption, Some(on)) => {
        node.selected = Some(on);
      },
      (_, _) if e.mixed => node.toggled = Some(Toggle::Mixed),
      (_, Some(on)) => {
        node.toggled = Some(if on { Toggle::On } else { Toggle::Off });
      },
      _ => {},
    }
    match (e.value, e.range) {
      (Some(value), Some(range)) => {
        node.numeric = Some(Numeric::Range {
          value,
          min: range.min,
          max: range.max,
          step: (range.step > 0.0).then_some(range.step),
        });
        if !e.disabled {
          node.actions.increment = true;
          node.actions.decrement = true;
          node.actions.set_value = true;
        }
      },
      (Some(value), None) => node.numeric = Some(Numeric::Fraction(value)),
      _ => {},
    }
    if let Some(open) = e.expanded {
      node.expanded = Some(open);
      // Pressing opens or closes it, so tools can do either.
      if e.on_press.is_some() {
        if open {
          node.actions.collapse = true;
        } else {
          node.actions.expand = true;
        }
      }
    }
    node.value.clone_from(&e.text_value);
    let description = e
      .description
      .clone()
      .or_else(|| self.description.take());
    node.description = description.filter(|d| !d.is_empty());
    if let Some(((x, y), (max_x, max_y))) = self.layout.scroll_extent(&b.key) {
      if max_x > 0.0 {
        node.scroll_x = Some(Scroll { at: x, max: max_x });
      }
      if max_y > 0.0 {
        node.scroll_y = Some(Scroll { at: y, max: max_y });
      }
    }
    // Rows, items and cells (M16).
    if let Some(place) = e.item {
      node.position_in_set = Some(place.index + 1);
      node.size_of_set = Some(place.count);
      node.level = place.level;
      if e.role == Role::Row {
        node.row_index = Some(place.index);
      }
    }
    if e.selected.is_some() {
      node.selected = e.selected;
    }
    node.multiselectable = e.multiselect;
    node.table_size = e.table_size;
    node.column_index = e.column;
    node.sorted = e.sorted;
    node.modal = e.role == Role::Dialog;
    node.disabled = e.disabled;
    let name = e.label.clone().or_else(|| {
      named_by_contents(e.role).then(|| text_of(self.boxes, index))
    });
    node.label = name.filter(|n| !n.is_empty());
    // The current item inside a view that keeps focus itself is the
    // view's active descendant (M16).
    let outer_current = self.current.take();
    if is_text_input(e.role) {
      node.actions.edit_text = e.on_access.is_some();
      node.text_field =
        dovetail_layout::first_text(self.boxes, index).map(|text_box| {
          TextField {
            text_box,
            selection: e.text_cursor.map(|c| (c.anchor, c.caret)),
          }
        });
      // Its value is the text in it.
      if let Some(field) = node.text_field
        && let Content::Text { text, .. } = &self.boxes[field.text_box].content
      {
        node.value = Some(text.to_string());
      }
    } else if !named_by_contents(e.role) {
      node.children = self.children(index);
    }
    let inner_current = self.current.take();
    if e.current {
      self.current = Some(id);
    } else if let Some(c) = inner_current.filter(|_| e.focusable) {
      node.active_descendant = Some(c);
    } else {
      self.current = inner_current;
    }
    self.current = self.current.or(outer_current);
    self.by_key.insert(b.key.clone(), id);
    self.nodes.push(node);
    vec![id]
  }

  fn children(
    &mut self,
    index: usize,
  ) -> Vec<AccessId> {
    self.boxes[index]
      .children
      .clone()
      .into_iter()
      .flat_map(|c| self.visit(c))
      .collect()
  }
}

/// The accessibility tree of `layout`, with focus on `focus`, in a window
/// called `title`.
pub fn access_tree(
  layout: &Layout,
  focus: Option<&BoxKey>,
  title: &str,
) -> AccessTree {
  let boxes = layout.boxes();
  let mut builder = Builder {
    boxes,
    layout,
    description: None,
    nodes: Vec::new(),
    by_key: HashMap::new(),
    current: None,
  };
  // The tree, then the layers above it, such as open menus.
  let mut children = if boxes.is_empty() {
    Vec::new()
  } else {
    builder.visit(0)
  };
  for &layer in layout.layers() {
    children.extend(builder.visit(layer));
  }
  let mut window =
    AccessNode::new(AccessId::ROOT, AccessKind::Window, None, None);
  window.label = Some(title.to_owned());
  window.children = children;
  builder.nodes.push(window);
  let focus = focus
    .and_then(|k| builder.by_key.get(k).copied())
    .unwrap_or(AccessId::ROOT);
  AccessTree { nodes: builder.nodes, focus }
}

impl Session {
  /// The window's accessibility tree, as last laid out: what assistive
  /// tools are told.
  pub fn access_tree(&self) -> AccessTree {
    access_tree(&self.layout, self.focus.as_ref(), &self.title)
  }
}
