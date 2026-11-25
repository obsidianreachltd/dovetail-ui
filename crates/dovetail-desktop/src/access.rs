//! The accessibility tree (M07): what screen readers and other assistive
//! tools see of a UI window, built with AccessKit (D0003, D0015).
//!
//! The tree is built from the laid-out boxes:
//!
//! - the window is the root, named by its title;
//! - an element with a role, or one that can take focus or be pressed,
//!   becomes a node; other elements pass their children up;
//! - text becomes a label, except inside buttons and switches, which take
//!   their name from it, and text fields, which describe it as text runs
//!   with the selection, so screen readers can follow the caret.
//!
//! Only nodes that changed since the last update are sent.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use accesskit::{
  Action, ActionData, ActionRequest, Affine, Node, NodeId, Rect as AkRect,
  Role as AkRole, SortDirection, TextPosition, TextSelection, Toggled, TreeId,
  TreeInfo, TreeUpdate,
};
use dovetail_core::{AccessAction, Element, Role, SortOrder};
use dovetail_layout::{BoxKey, Content, Layout, LayoutBox, Measure, Rect};

/// The id of the window's own node.
const ROOT: NodeId = NodeId(0);

/// What a node stands for, to carry out actions on it.
#[derive(Clone, Debug)]
enum Target {
  /// An element.
  Element(BoxKey),
  /// One line of a text field: the field, the byte where the line starts,
  /// and the byte offset of each character in the line, with the line's
  /// end last.
  Run {
    field: BoxKey,
    start: usize,
    offsets: Vec<usize>,
  },
}

/// What an action from an assistive tool asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Request {
  /// Move keyboard focus to this element.
  Focus(BoxKey),
  /// Press this element.
  Click(BoxKey),
  /// Edit the text of this element.
  Edit(BoxKey, AccessAction),
}

/// A window's accessibility tree, as last sent.
#[derive(Default)]
pub(crate) struct Access {
  /// The nodes last sent, to send only changes.
  sent: HashMap<NodeId, Node>,
  /// What each node stands for.
  targets: HashMap<NodeId, Target>,
  /// The tree's details still have to be sent: the first update.
  fresh: bool,
}

/// A stable id for a box's node: a hash of its key, never the root's.
fn node_id(
  key: &BoxKey,
  part: u32,
) -> NodeId {
  let mut h = DefaultHasher::new();
  key.hash(&mut h);
  part.hash(&mut h);
  NodeId(h.finish().max(1))
}

fn rect(r: Rect) -> AkRect {
  AkRect::new(
    f64::from(r.x),
    f64::from(r.y),
    f64::from(r.x + r.width),
    f64::from(r.y + r.height),
  )
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

fn ak_role(role: Role) -> AkRole {
  match role {
    Role::None => AkRole::GenericContainer,
    Role::Button => AkRole::Button,
    Role::Switch => AkRole::Switch,
    Role::CheckBox => AkRole::CheckBox,
    Role::TextInput => AkRole::TextInput,
    Role::MultilineTextInput => AkRole::MultilineTextInput,
    Role::Label => AkRole::Label,
    Role::Heading => AkRole::Heading,
    Role::Group => AkRole::Group,
    Role::Image => AkRole::Image,
    Role::MenuBar => AkRole::MenuBar,
    Role::Menu => AkRole::Menu,
    Role::MenuItem => AkRole::MenuItem,
    Role::ListBox => AkRole::ListBox,
    Role::ListBoxOption => AkRole::ListBoxOption,
    Role::Toolbar => AkRole::Toolbar,
    Role::Canvas => AkRole::Canvas,
    Role::TabList => AkRole::TabList,
    Role::Tab => AkRole::Tab,
    Role::TabPanel => AkRole::TabPanel,
    Role::Splitter => AkRole::Splitter,
    Role::ProgressBar => AkRole::ProgressIndicator,
    Role::Dialog => AkRole::Dialog,
    Role::RadioButton => AkRole::RadioButton,
    Role::RadioGroup => AkRole::RadioGroup,
    Role::ComboBox => AkRole::ComboBox,
    Role::SpinButton => AkRole::SpinButton,
    Role::Slider => AkRole::Slider,
    Role::Tooltip => AkRole::Tooltip,
    Role::Link => AkRole::Link,
    Role::StatusBar => AkRole::Status,
    // AccessKit's splitter is ARIA's separator.
    Role::Separator => AkRole::Splitter,
    Role::Tree => AkRole::Tree,
    Role::TreeItem => AkRole::TreeItem,
    Role::Table => AkRole::Table,
    Role::Row => AkRole::Row,
    Role::Cell => AkRole::Cell,
    Role::ColumnHeader => AkRole::ColumnHeader,
  }
}

/// Roles whose name is their text, so their text is not read again.
fn named_by_contents(role: Role) -> bool {
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
  measure: &'a dyn Measure,
  /// A description from an element with no node, for the next node made
  /// inside it.
  description: Option<String>,
  nodes: Vec<(NodeId, Node)>,
  targets: HashMap<NodeId, Target>,
  /// The node of each element, to find the focused one.
  by_key: HashMap<BoxKey, NodeId>,
  /// The node of the last current item seen and not yet given to the
  /// focusable view around it (M16).
  current: Option<NodeId>,
}

impl Builder<'_> {
  /// How far a scrolling element is scrolled, and how far it can go.
  fn scroll_of(
    &self,
    key: &BoxKey,
  ) -> Option<(f32, f32, f32, f32)> {
    let ((x, y), (max_x, max_y)) = self.layout.scroll_extent(key)?;
    Some((x, y, max_x, max_y))
  }

  /// The nodes for box `index`, to go in its parent's children.
  fn visit(
    &mut self,
    index: usize,
  ) -> Vec<NodeId> {
    let b = &self.boxes[index];
    match &b.content {
      // Empty text, such as a blank label, has nothing to read.
      Content::Text { text, .. } if text.trim().is_empty() => Vec::new(),
      // A picture with a label is an image; one without is decoration,
      // which assistive tools skip (M17).
      Content::Picture { picture, .. } => match &picture.label {
        Some(label) => {
          let id = node_id(&b.key, 0);
          let mut node = Node::new(AkRole::Image);
          node.set_label(label.to_string());
          node.set_bounds(rect(b.rect));
          self.nodes.push((id, node));
          vec![id]
        },
        None => Vec::new(),
      },
      Content::Text { text, .. } => {
        let id = node_id(&b.key, 0);
        let mut node = Node::new(AkRole::Label);
        node.set_value(text.to_string());
        node.set_bounds(rect(b.rect));
        self.nodes.push((id, node));
        vec![id]
      },
      Content::Element(e) => {
        let is_node = e.role != Role::None
          || e.focusable
          || e.on_press.is_some()
          || e.disabled;
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
        let id = node_id(&b.key, 0);
        let mut node = Node::new(ak_role(e.role));
        node.set_bounds(rect(b.rect));
        if e.focusable {
          node.add_action(Action::Focus);
        }
        if e.on_press.is_some() {
          node.add_action(Action::Click);
        }
        match (e.role, e.toggled) {
          // A tab and a radio button are selected rather than switched on.
          (Role::Tab | Role::RadioButton | Role::ListBoxOption, Some(on)) => {
            node.set_selected(on)
          },
          (_, _) if e.mixed => node.set_toggled(Toggled::Mixed),
          (_, Some(on)) => node.set_toggled(Toggled::from(on)),
          _ => {},
        }
        match (e.value, e.range) {
          (Some(value), Some(range)) => {
            node.set_numeric_value(f64::from(value));
            node.set_min_numeric_value(f64::from(range.min));
            node.set_max_numeric_value(f64::from(range.max));
            if range.step > 0.0 {
              node.set_numeric_value_step(f64::from(range.step));
            }
            if !e.disabled {
              node.add_action(Action::Increment);
              node.add_action(Action::Decrement);
              node.add_action(Action::SetValue);
            }
          },
          (Some(value), None) => {
            node.set_numeric_value(f64::from(value) * 100.0);
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(100.0);
          },
          _ => {},
        }
        if let Some(open) = e.expanded {
          node.set_expanded(open);
          // Pressing opens or closes it, so tools can do either.
          if e.on_press.is_some() {
            node.add_action(if open {
              Action::Collapse
            } else {
              Action::Expand
            });
          }
        }
        if let Some(value) = &e.text_value {
          node.set_value(value.clone());
        }
        let description = e
          .description
          .clone()
          .or_else(|| self.description.take());
        if let Some(d) = description.filter(|d| !d.is_empty()) {
          node.set_description(d);
        }
        if let Some((x, y, max_x, max_y)) = self.scroll_of(&b.key) {
          if max_x > 0.0 {
            node.set_scroll_x(f64::from(x));
            node.set_scroll_x_min(0.0);
            node.set_scroll_x_max(f64::from(max_x));
          }
          if max_y > 0.0 {
            node.set_scroll_y(f64::from(y));
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(f64::from(max_y));
          }
        }
        // Rows, items and cells (M16).
        if let Some(place) = e.item {
          node.set_position_in_set(place.index + 1);
          node.set_size_of_set(place.count);
          if let Some(level) = place.level {
            node.set_level(level);
          }
          if e.role == Role::Row {
            node.set_row_index(place.index);
          }
        }
        if let Some(on) = e.selected {
          node.set_selected(on);
        }
        if e.multiselect {
          node.set_multiselectable();
        }
        if let Some((rows, columns)) = e.table_size {
          node.set_row_count(rows);
          node.set_column_count(columns);
        }
        if let Some(column) = e.column {
          node.set_column_index(column);
        }
        match e.sorted {
          Some(SortOrder::Ascending) => {
            node.set_sort_direction(SortDirection::Ascending)
          },
          Some(SortOrder::Descending) => {
            node.set_sort_direction(SortDirection::Descending)
          },
          None => {},
        }
        if e.role == Role::Dialog {
          node.set_modal();
        }
        if e.disabled {
          node.set_disabled();
        }
        let name = e.label.clone().or_else(|| {
          named_by_contents(e.role).then(|| text_of(self.boxes, index))
        });
        if let Some(name) = name.filter(|n| !n.is_empty()) {
          node.set_label(name);
        }
        // The current item inside a view that keeps focus itself is the
        // view's active descendant (M16).
        let outer_current = self.current.take();
        if is_text_input(e.role) {
          self.text_input(index, e, &mut node);
        } else if !named_by_contents(e.role) {
          node.set_children(self.children(index));
        }
        let inner_current = self.current.take();
        if e.current {
          self.current = Some(id);
        } else if let Some(c) = inner_current.filter(|_| e.focusable) {
          node.set_active_descendant(c);
        } else {
          self.current = inner_current;
        }
        self.current = self.current.or(outer_current);
        self.by_key.insert(b.key.clone(), id);
        self
          .targets
          .insert(id, Target::Element(b.key.clone()));
        self.nodes.push((id, node));
        vec![id]
      },
    }
  }

  fn children(
    &mut self,
    index: usize,
  ) -> Vec<NodeId> {
    self.boxes[index]
      .children
      .clone()
      .into_iter()
      .flat_map(|c| self.visit(c))
      .collect()
  }

  /// A text field: its value, a text run for each line, and the selection.
  fn text_input(
    &mut self,
    index: usize,
    e: &Element,
    node: &mut Node,
  ) {
    if e.on_access.is_some() {
      node.add_action(Action::SetTextSelection);
      node.add_action(Action::ReplaceSelectedText);
      node.add_action(Action::SetValue);
    }
    let Some(t) = dovetail_layout::first_text(self.boxes, index) else {
      return;
    };
    let text_box = &self.boxes[t];
    let Content::Text { text, .. } = &text_box.content else {
      return;
    };
    node.set_value(text.to_string());
    let key = &self.boxes[index].key;
    let font = text_box.content.text_style().expect("text");
    let line_height = self.measure.line_height(font);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut runs = Vec::with_capacity(lines.len());
    // Each run's id, start and character offsets, to place the selection.
    let mut placed: Vec<(NodeId, usize, Vec<usize>)> = Vec::new();
    let mut start = 0;
    for (row, line) in lines.iter().enumerate() {
      let last = row + 1 == lines.len();
      // Characters are grapheme clusters; a line break is one more.
      let mut offsets = dovetail_text::grapheme_boundaries(line);
      if offsets.first() != Some(&0) {
        offsets.insert(0, 0);
      }
      if offsets.last() != Some(&line.len()) {
        offsets.push(line.len());
      }
      let mut lengths: Vec<u8> = offsets
        .windows(2)
        .map(|w| (w[1] - w[0]).min(255) as u8)
        .collect();
      let mut value = (*line).to_owned();
      // Each grapheme's place, from where the line starts in the box
      // (right-to-left lines line up on the right).
      let line_x = self
        .measure
        .line_start(line, font, text_box.rect.width);
      let boxes = self.measure.graphemes(line, font);
      let place = |at: usize| {
        boxes
          .iter()
          .find(|b| b.0 == at)
          .map_or((0.0, 0.0), |b| (b.1, b.2 - b.1))
      };
      let mut positions: Vec<f32> = offsets[..offsets.len() - 1]
        .iter()
        .map(|&at| line_x + place(at).0)
        .collect();
      let mut widths: Vec<f32> = offsets[..offsets.len() - 1]
        .iter()
        .map(|&at| place(at).1)
        .collect();
      let width = self.measure.text(line, font).0;
      let rtl = self.measure.is_rtl(line, font);
      if !last {
        value.push('\n');
        lengths.push(1);
        positions.push(line_x + if rtl { 0.0 } else { width });
        widths.push(0.0);
        offsets.push(line.len() + 1);
      }
      // Words start after spaces.
      let chars: Vec<char> = offsets
        .windows(2)
        .map(|w| value[w[0]..].chars().next().unwrap_or(' '))
        .collect();
      let word_starts: Vec<u8> = (0..chars.len())
        .filter(|&i| {
          i > 0 && !chars[i].is_whitespace() && chars[i - 1].is_whitespace()
        })
        .filter_map(|i| u8::try_from(i).ok())
        .collect();

      let run_id = node_id(key, row as u32 + 1);
      let mut run = Node::new(AkRole::TextRun);
      run.set_value(value);
      run.set_character_lengths(lengths);
      run.set_character_positions(positions);
      run.set_character_widths(widths);
      run.set_word_starts(word_starts);
      if rtl {
        run.set_text_direction(accesskit::TextDirection::RightToLeft);
      }
      run.set_bounds(rect(Rect::new(
        text_box.rect.x + line_x,
        text_box.rect.y + row as f32 * line_height,
        width,
        line_height,
      )));
      self.nodes.push((run_id, run));
      self.targets.insert(
        run_id,
        Target::Run {
          field: key.clone(),
          start,
          offsets: offsets.clone(),
        },
      );
      placed.push((run_id, start, offsets));
      runs.push(run_id);
      start += line.len() + 1;
    }
    node.set_children(runs);

    if let Some(cursor) = e.text_cursor {
      let position = |byte: usize| {
        // The run that holds the byte; a byte at a line's end belongs to
        // that line, before its line break.
        let (run, line_start, offsets) = placed
          .iter()
          .rev()
          .find(|(_, s, _)| *s <= byte)
          .unwrap_or(&placed[0]);
        let within = byte - line_start;
        let index = offsets
          .iter()
          .position(|&o| o >= within)
          .unwrap_or(offsets.len() - 1);
        TextPosition { node: *run, character_index: index }
      };
      node.set_text_selection(TextSelection {
        anchor: position(cursor.anchor),
        focus: position(cursor.caret),
      });
    }
  }
}

impl Access {
  /// Starts again from nothing: the next update sends the whole tree, as
  /// when an assistive tool first asks for it.
  pub fn reset(&mut self) {
    self.sent.clear();
    self.fresh = true;
  }

  /// The update for the layout as it is now: every node that changed since
  /// the last update, and where focus is.
  pub fn update(
    &mut self,
    layout: &Layout,
    focus: Option<&BoxKey>,
    title: &str,
    scale: f32,
    measure: &dyn Measure,
  ) -> TreeUpdate {
    let boxes = layout.boxes();
    let mut builder = Builder {
      boxes,
      layout,
      measure,
      description: None,
      nodes: Vec::new(),
      targets: HashMap::new(),
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
    let mut window = Node::new(AkRole::Window);
    window.set_label(title.to_owned());
    // Bounds are in logical pixels; the scale makes them physical.
    window.set_transform(Affine::scale(f64::from(scale)));
    window.set_children(children);
    builder.nodes.push((ROOT, window));

    let focus = focus
      .and_then(|k| builder.by_key.get(k).copied())
      .unwrap_or(ROOT);
    let mut changed = Vec::new();
    let mut sent = HashMap::with_capacity(builder.nodes.len());
    for (id, node) in builder.nodes {
      if self.sent.get(&id) != Some(&node) {
        changed.push((id, node.clone()));
      }
      sent.insert(id, node);
    }
    self.sent = sent;
    self.targets = builder.targets;
    let tree = std::mem::take(&mut self.fresh).then(|| {
      let mut info = TreeInfo::new(ROOT);
      info.toolkit_name = Some("Dovetail UI".into());
      info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
      info
    });
    TreeUpdate {
      nodes: changed,
      tree,
      tree_id: TreeId::ROOT,
      focus,
    }
  }

  /// What an action request asks for, if it is one Dovetail carries out.
  pub fn request(
    &self,
    request: &ActionRequest,
  ) -> Option<Request> {
    let target = self.targets.get(&request.target_node)?;
    let Target::Element(key) = target else {
      return None;
    };
    let text = || match &request.data {
      Some(ActionData::Value(v)) => Some(v.to_string()),
      _ => None,
    };
    match request.action {
      Action::Focus => Some(Request::Focus(key.clone())),
      Action::Click => Some(Request::Click(key.clone())),
      // Only an element that opens and closes offers these, and only the
      // one that changes it, so a press does it.
      Action::Expand | Action::Collapse => Some(Request::Click(key.clone())),
      Action::ReplaceSelectedText => Some(Request::Edit(
        key.clone(),
        AccessAction::ReplaceSelection(text()?),
      )),
      Action::SetValue => match &request.data {
        Some(ActionData::NumericValue(n)) => Some(Request::Edit(
          key.clone(),
          AccessAction::SetNumber(*n as f32),
        )),
        _ => Some(Request::Edit(key.clone(), AccessAction::SetValue(text()?))),
      },
      Action::Increment => {
        Some(Request::Edit(key.clone(), AccessAction::Increment))
      },
      Action::Decrement => {
        Some(Request::Edit(key.clone(), AccessAction::Decrement))
      },
      Action::SetTextSelection => {
        let Some(ActionData::SetTextSelection(s)) = &request.data else {
          return None;
        };
        let anchor = self.byte_of(key, &s.anchor)?;
        let caret = self.byte_of(key, &s.focus)?;
        Some(Request::Edit(
          key.clone(),
          AccessAction::SetSelection { anchor, caret },
        ))
      },
      _ => None,
    }
  }

  /// The byte in field `field`'s text at a text position.
  fn byte_of(
    &self,
    field: &BoxKey,
    position: &TextPosition,
  ) -> Option<usize> {
    match self.targets.get(&position.node)? {
      // Character `i` starts at `offsets[i]`, and the last offset is the
      // run's end. A position on a line break is the end of the line's text.
      Target::Run { field: f, start, offsets } if f == field => {
        let i = position
          .character_index
          .min(offsets.len().saturating_sub(1));
        Some(start + offsets.get(i).copied().unwrap_or(0))
      },
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use accesskit::{Action, ActionData, ActionRequest, Role as AkRole, TreeId};
  use dovetail_core::{
    AccessAction, BuildContext, Callback, Component, Element, ElementKind,
    Role, Signal, TextCursor, Tree, View,
  };
  use dovetail_layout::{FixedPitch, Layout};

  use super::*;

  thread_local! {
    static SCENE: Cell<Option<Signal<View>>> = const { Cell::new(None) };
  }

  struct Scene;

  #[derive(PartialEq)]
  struct SceneProps(View);

  impl Component for Scene {
    type Props = SceneProps;

    fn build(
      props: &SceneProps,
      cx: &mut BuildContext<'_>,
    ) -> View {
      let first = props.0.clone();
      let view = cx.use_state(move || first);
      SCENE.set(Some(view));
      view.get()
    }
  }

  struct Window {
    tree: Tree,
    layout: Layout,
    access: Access,
  }

  impl Window {
    fn new(view: View) -> Self {
      let mut tree = Tree::new();
      tree.mount(View::component::<Scene>(SceneProps(view)));
      let mut w = Window {
        tree,
        layout: Layout::new(),
        access: Access::default(),
      };
      w.layout
        .update(&w.tree, 400.0, 300.0, &FixedPitch::default());
      w.access.reset();
      w
    }

    fn show(
      &mut self,
      view: View,
    ) {
      SCENE.get().unwrap().set(view);
      self.tree.update();
      self
        .layout
        .update(&self.tree, 400.0, 300.0, &FixedPitch::default());
    }

    fn update(
      &mut self,
      focus: Option<&BoxKey>,
    ) -> TreeUpdate {
      self.access.update(
        &self.layout,
        focus,
        "Test",
        1.5,
        &FixedPitch::default(),
      )
    }

    fn key_of(
      &self,
      role: Role,
    ) -> BoxKey {
      self
        .layout
        .boxes()
        .iter()
        .find(|b| matches!(&b.content, Content::Element(e) if e.role == role))
        .map(|b| b.key.clone())
        .expect("an element with that role")
    }
  }

  fn find(
    update: &TreeUpdate,
    role: AkRole,
  ) -> Vec<(NodeId, Node)> {
    update
      .nodes
      .iter()
      .filter(|(_, n)| n.role() == role)
      .cloned()
      .collect()
  }

  fn button(text: &str) -> View {
    Element::new(ElementKind::Box)
      .role(Role::Button)
      .focusable(true)
      .on_press(Callback::new(|()| {}))
      .children([View::text(text)])
      .into()
  }

  fn column(children: impl IntoIterator<Item = View>) -> View {
    Element::new(ElementKind::Column)
      .children(children)
      .into()
  }

  #[test]
  fn controls_become_nodes_with_roles_names_and_states() {
    let switch: View = Element::new(ElementKind::Row)
      .role(Role::Switch)
      .toggled(true)
      .focusable(true)
      .on_press(Callback::new(|()| {}))
      .children([View::text("Lights")])
      .into();
    let mut w = Window::new(column([
      View::text("Count: 0"),
      View::text("  "),
      button("Increment"),
      switch,
    ]));
    let update = w.update(None);

    let info = update
      .tree
      .as_ref()
      .expect("the first update has the tree");
    assert_eq!(info.root, ROOT);
    assert_eq!(update.tree_id, TreeId::ROOT);
    assert_eq!(update.focus, ROOT, "nothing has focus");
    let (_, window) = &find(&update, AkRole::Window)[0];
    assert_eq!(window.label(), Some("Test"));
    assert_eq!(
      window.children().len(),
      3,
      "the column passes its children up"
    );

    let labels = find(&update, AkRole::Label);
    assert_eq!(
      labels.len(),
      1,
      "blank text is left out, and text in a button or switch is not read twice"
    );
    assert_eq!(labels[0].1.value(), Some("Count: 0"));

    let (_, b) = &find(&update, AkRole::Button)[0];
    assert_eq!(b.label(), Some("Increment"));
    assert!(b.supports_action(Action::Click));
    assert!(b.supports_action(Action::Focus));
    assert!(b.children().is_empty());

    let (_, s) = &find(&update, AkRole::Switch)[0];
    assert_eq!(s.label(), Some("Lights"));
    assert_eq!(s.toggled(), Some(accesskit::Toggled::True));
  }

  #[test]
  fn later_updates_send_only_what_changed_and_follow_focus() {
    let mut w = Window::new(column([
      View::text("Count: 0"),
      button("Increment"),
    ]));
    let first = w.update(None);
    assert_eq!(first.nodes.len(), 3, "window, label and button");

    // Nothing changed: no nodes, but focus is always given.
    let again = w.update(None);
    assert!(again.nodes.is_empty());
    assert!(again.tree.is_none());

    // The count changes: only its label is sent.
    w.show(column([
      View::text("Count: 1"),
      button("Increment"),
    ]));
    let changed = w.update(None);
    assert_eq!(changed.nodes.len(), 1);
    assert_eq!(changed.nodes[0].1.value(), Some("Count: 1"));

    // Focus is the button's node.
    let key = w.key_of(Role::Button);
    let focused = w.update(Some(&key));
    let (button_id, _) = find(&first, AkRole::Button)[0].clone();
    assert_eq!(focused.focus, button_id);
    assert!(focused.nodes.is_empty(), "focus alone sends no nodes");

    // After a reset, the whole tree is sent again.
    w.access.reset();
    let full = w.update(Some(&key));
    assert_eq!(full.nodes.len(), 3);
    assert!(full.tree.is_some());
  }

  fn field(
    text: &str,
    caret: usize,
    anchor: usize,
  ) -> View {
    Element::new(ElementKind::Box)
      .role(Role::MultilineTextInput)
      .label("Notes")
      .focusable(true)
      .on_access(Callback::new(|_: AccessAction| {}))
      .text_cursor(TextCursor {
        caret,
        anchor,
        show_caret: true,
        composing: None,
      })
      .children([View::text(text)])
      .into()
  }

  #[test]
  fn text_fields_are_described_as_lines_with_the_selection() {
    // The e has a combining accent: 3 bytes, but one character.
    let text = "cafe\u{301} au lait\nok";
    let mut w = Window::new(column([field(text, text.len(), 1)]));
    let update = w.update(None);

    let (field_id, f) = find(&update, AkRole::MultilineTextInput)[0].clone();
    assert_eq!(f.label(), Some("Notes"));
    assert_eq!(f.value(), Some(text));
    assert!(f.supports_action(Action::SetTextSelection));
    let runs = find(&update, AkRole::TextRun);
    assert_eq!(runs.len(), 2, "one run for each line");
    let run = |id: NodeId| {
      runs
        .iter()
        .find(|(r, _)| *r == id)
        .unwrap()
        .1
        .clone()
    };
    let first = run(f.children()[0]);
    assert_eq!(first.value(), Some("cafe\u{301} au lait\n"));
    // c, a, f, e and accent, space, a, u, space, l, a, i, t, line break.
    assert_eq!(
      first.character_lengths(),
      &[1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1][..]
    );
    assert_eq!(first.word_starts(), &[5, 8][..]);
    assert_eq!(first.character_positions().unwrap().len(), 13);
    assert_eq!(run(f.children()[1]).value(), Some("ok"));

    let s = f.text_selection().unwrap();
    assert_eq!(s.anchor.node, f.children()[0]);
    assert_eq!(s.anchor.character_index, 1);
    assert_eq!(s.focus.node, f.children()[1]);
    assert_eq!(s.focus.character_index, 2, "the end of the second line");

    // A selection from a tool comes back as bytes.
    let request = ActionRequest {
      action: Action::SetTextSelection,
      target_tree: TreeId::ROOT,
      target_node: field_id,
      data: Some(ActionData::SetTextSelection(TextSelection {
        anchor: TextPosition {
          node: f.children()[0],
          character_index: 4,
        },
        focus: TextPosition {
          node: f.children()[1],
          character_index: 1,
        },
      })),
    };
    let key = w.key_of(Role::MultilineTextInput);
    let line_two = "cafe\u{301} au lait\n".len();
    assert_eq!(
      w.access.request(&request),
      Some(Request::Edit(
        key.clone(),
        AccessAction::SetSelection { anchor: 6, caret: line_two + 1 }
      ))
    );
    let value = ActionRequest {
      action: Action::SetValue,
      target_tree: TreeId::ROOT,
      target_node: field_id,
      data: Some(ActionData::Value("new".into())),
    };
    assert_eq!(
      w.access.request(&value),
      Some(Request::Edit(key, AccessAction::SetValue("new".into())))
    );
  }

  #[test]
  fn clicks_and_focus_from_tools_name_the_element() {
    let mut w = Window::new(column([button("Go")]));
    let update = w.update(None);
    let (id, _) = find(&update, AkRole::Button)[0].clone();
    let key = w.key_of(Role::Button);
    let ask = |action| ActionRequest {
      action,
      target_tree: TreeId::ROOT,
      target_node: id,
      data: None,
    };
    assert_eq!(
      w.access.request(&ask(Action::Click)),
      Some(Request::Click(key.clone()))
    );
    assert_eq!(
      w.access.request(&ask(Action::Focus)),
      Some(Request::Focus(key))
    );
    assert_eq!(w.access.request(&ask(Action::ScrollIntoView)), None);
  }
  /// A box with a role, a label and nothing else.
  fn with_role(
    role: Role,
    label: &str,
  ) -> Element {
    Element::new(ElementKind::Box)
      .role(role)
      .label(label)
  }

  #[test]
  fn m15_controls_have_their_roles_values_and_states() {
    let range = dovetail_core::ValueRange { min: 0.0, max: 10.0, step: 2.0 };
    let tip: View = Element::new(ElementKind::Box)
      .description("Saves the file")
      .children([button("Save")])
      .into();
    let mut w = Window::new(column([
      with_role(Role::CheckBox, "All")
        .toggled(false)
        .mixed(true)
        .focusable(true)
        .into(),
      with_role(Role::RadioGroup, "Size")
        .children([
          with_role(Role::RadioButton, "Small")
            .toggled(false)
            .into(),
          with_role(Role::RadioButton, "Large")
            .toggled(true)
            .into(),
        ])
        .into(),
      with_role(Role::ComboBox, "Fruit")
        .text_value("Cherry")
        .expanded(false)
        .focusable(true)
        .into(),
      with_role(Role::Slider, "Volume")
        .value_in(4.0, range)
        .focusable(true)
        .on_access(Callback::new(|_: AccessAction| {}))
        .into(),
      with_role(Role::SpinButton, "Copies")
        .value_in(3.0, range)
        .into(),
      with_role(Role::Link, "Help")
        .focusable(true)
        .into(),
      with_role(Role::StatusBar, "Status").into(),
      Element::new(ElementKind::Box)
        .role(Role::Separator)
        .into(),
      with_role(Role::Button, "More")
        .expanded(true)
        .into(),
      tip,
    ]));
    let update = w.update(None);
    let one = |role| {
      let found = find(&update, role);
      assert_eq!(found.len(), 1, "one {role:?}");
      found[0].1.clone()
    };

    let check = one(AkRole::CheckBox);
    assert_eq!(check.toggled(), Some(Toggled::Mixed));
    assert_eq!(one(AkRole::RadioGroup).label(), Some("Size"));
    let radios = find(&update, AkRole::RadioButton);
    let chosen: Vec<Option<bool>> = radios
      .iter()
      .map(|(_, n)| n.is_selected())
      .collect();
    assert_eq!(chosen, [Some(false), Some(true)]);

    let combo = one(AkRole::ComboBox);
    assert_eq!(
      (combo.value(), combo.is_expanded()),
      (Some("Cherry"), Some(false))
    );

    let slider = one(AkRole::Slider);
    assert_eq!(slider.numeric_value(), Some(4.0));
    assert_eq!(slider.min_numeric_value(), Some(0.0));
    assert_eq!(slider.max_numeric_value(), Some(10.0));
    assert_eq!(slider.numeric_value_step(), Some(2.0));
    assert!(slider.supports_action(Action::Increment));
    assert!(slider.supports_action(Action::Decrement));
    assert_eq!(one(AkRole::SpinButton).numeric_value(), Some(3.0));

    assert_eq!(one(AkRole::Link).label(), Some("Help"));
    assert_eq!(one(AkRole::Status).label(), Some("Status"));
    assert_eq!(find(&update, AkRole::Splitter).len(), 1, "the separator");
    let more = find(&update, AkRole::Button)
      .into_iter()
      .find(|(_, n)| n.label() == Some("More"))
      .expect("the section's header");
    assert_eq!(more.1.is_expanded(), Some(true));
    assert!(
      !more.1.supports_action(Action::Collapse),
      "it cannot be pressed"
    );
    // A description with no node of its own goes to the button inside.
    let save = find(&update, AkRole::Button)
      .into_iter()
      .find(|(_, n)| n.label() == Some("Save"))
      .expect("the button in the tooltip");
    assert_eq!(save.1.description(), Some("Saves the file"));
    assert_eq!(more.1.description(), None, "and to nothing else");

    // Tools step a value and set it.
    let (slider_id, _) = find(&update, AkRole::Slider)[0].clone();
    let key = w.key_of(Role::Slider);
    let ask = |action, data| ActionRequest {
      action,
      target_tree: TreeId::ROOT,
      target_node: slider_id,
      data,
    };
    assert_eq!(
      w.access.request(&ask(Action::Increment, None)),
      Some(Request::Edit(key.clone(), AccessAction::Increment))
    );
    assert_eq!(
      w.access.request(&ask(Action::Decrement, None)),
      Some(Request::Edit(key.clone(), AccessAction::Decrement))
    );
    assert_eq!(
      w.access
        .request(&ask(Action::SetValue, Some(ActionData::NumericValue(6.0)))),
      Some(Request::Edit(key, AccessAction::SetNumber(6.0)))
    );
  }

  #[test]
  fn a_scrolling_element_tells_its_scroll_position() {
    let tall: View = Element::new(ElementKind::Box)
      .style(dovetail_core::Style {
        height: dovetail_core::Size::Fixed(400.0),
        ..Default::default()
      })
      .children([button("Inside")])
      .into();
    let scroller: View = Element::new(ElementKind::Box)
      .style(dovetail_core::Style {
        height: dovetail_core::Size::Fixed(100.0),
        clip: true,
        scroll: true,
        ..Default::default()
      })
      .role(Role::Group)
      .label("List")
      .children([tall])
      .into();
    let mut w = Window::new(column([scroller]));
    let key = w.key_of(Role::Group);
    assert!(w.layout.scroll_to(&key, None, Some(120.0)));
    w.layout
      .update(&w.tree, 400.0, 300.0, &FixedPitch::default());
    let update = w.update(None);
    let (_, list) = find(&update, AkRole::Group)[0].clone();
    assert_eq!(list.scroll_y(), Some(120.0));
    assert_eq!(list.scroll_y_min(), Some(0.0));
    assert_eq!(list.scroll_y_max(), Some(300.0));
    assert_eq!(list.scroll_x(), None, "it does not scroll across");
  }

  fn row(
    role: Role,
    text: &str,
    index: usize,
    count: usize,
  ) -> Element {
    Element::new(ElementKind::Row)
      .role(role)
      .item(dovetail_core::ItemPlace { index, count, level: None })
      .children([View::text(text)])
  }

  #[test]
  fn rows_tell_their_place_in_the_whole_collection() {
    // A list of 100,000 rows of which three are made, the middle one
    // selected and current; a tree item; and a table with a header.
    let list: View = Element::new(ElementKind::Column)
      .role(Role::ListBox)
      .label("Parts")
      .focusable(true)
      .multiselect(true)
      .children([
        row(Role::ListBoxOption, "R49999", 49_999, 100_000).into(),
        row(Role::ListBoxOption, "R50000", 50_000, 100_000)
          .selected(true)
          .current(true)
          .into(),
        row(Role::ListBoxOption, "R50001", 50_001, 100_000)
          .selected(false)
          .into(),
      ])
      .into();
    let tree: View = Element::new(ElementKind::Column)
      .role(Role::Tree)
      .focusable(true)
      .children([Element::new(ElementKind::Row)
        .role(Role::TreeItem)
        .item(dovetail_core::ItemPlace { index: 2, count: 7, level: Some(3) })
        .expanded(false)
        .children([View::text("Layer")])
        .into()])
      .into();
    let header = |c: usize, sorted| -> View {
      Element::new(ElementKind::Box)
        .role(Role::ColumnHeader)
        .column(c)
        .sorted(sorted)
        .children([View::text(format!("H{c}"))])
        .into()
    };
    let cell = |c: usize| -> View {
      Element::new(ElementKind::Box)
        .role(Role::Cell)
        .column(c)
        .children([View::text(format!("C{c}"))])
        .into()
    };
    let table: View = Element::new(ElementKind::Column)
      .role(Role::Table)
      .label("Nets")
      .table_size(5001, 2)
      .children([
        Element::new(ElementKind::Row)
          .role(Role::Row)
          .item(dovetail_core::ItemPlace { index: 0, count: 5001, level: None })
          .children([
            header(0, Some(dovetail_core::SortOrder::Descending)),
            header(1, None),
          ])
          .into(),
        Element::new(ElementKind::Row)
          .role(Role::Row)
          .item(dovetail_core::ItemPlace {
            index: 4000,
            count: 5001,
            level: None,
          })
          .children([cell(0), cell(1)])
          .into(),
      ])
      .into();
    let mut w = Window::new(column([list, tree, table]));
    let update = w.update(None);
    let one = |role| find(&update, role)[0].1.clone();

    let options = find(&update, AkRole::ListBoxOption);
    assert_eq!(options.len(), 3);
    let (current_id, middle) = options
      .iter()
      .find(|(_, n)| n.label() == Some("R50000"))
      .cloned()
      .unwrap();
    assert_eq!(middle.position_in_set(), Some(50_001), "counted from 1");
    assert_eq!(middle.size_of_set(), Some(100_000), "of the whole list");
    assert_eq!(middle.is_selected(), Some(true));
    let list = one(AkRole::ListBox);
    assert!(list.is_multiselectable());
    assert_eq!(
      list.active_descendant(),
      Some(current_id),
      "the list says which row the keyboard is on"
    );

    let item = one(AkRole::TreeItem);
    assert_eq!(item.level(), Some(3));
    assert_eq!(item.position_in_set(), Some(3));
    assert_eq!(item.size_of_set(), Some(7));
    assert_eq!(item.is_expanded(), Some(false));
    assert_eq!(item.label(), Some("Layer"));

    let table = one(AkRole::Table);
    assert_eq!(
      (table.row_count(), table.column_count()),
      (Some(5001), Some(2))
    );
    let headers = find(&update, AkRole::ColumnHeader);
    assert_eq!(
      headers[0].1.sort_direction(),
      Some(SortDirection::Descending)
    );
    assert_eq!(headers[1].1.column_index(), Some(1));
    assert_eq!(headers[1].1.sort_direction(), None);
    let rows = find(&update, AkRole::Row);
    assert_eq!(rows[1].1.row_index(), Some(4000));
    let cells = find(&update, AkRole::Cell);
    assert_eq!(cells[1].1.column_index(), Some(1));
    assert_eq!(cells[1].1.label(), Some("C1"));
  }

  #[test]
  fn pictures_are_images_with_labels_or_skipped_as_decoration() {
    use dovetail_core::{Picture, PictureKind};
    let image = |id| {
      Picture::new((), id, PictureKind::Image { width: 40.0, height: 30.0 })
    };
    let icon = |id| Picture::new((), id, PictureKind::Icon { aspect: 1.0 });
    let button = |children: Vec<View>| -> View {
      Element::new(ElementKind::Row)
        .role(Role::Button)
        .focusable(true)
        .children(children)
        .into()
    };
    let view = column([
      // A picture with a label: an image node.
      image(1).label("Company logo").into(),
      // Decoration: nothing.
      image(2).into(),
      icon(3).into(),
      // An icon beside a button's text is decoration; the text names it.
      button(vec![icon(4).into(), View::text("Save")]),
      // An icon-only button is named by its icon's label.
      button(vec![icon(5).label("Close").into()]),
    ]);
    let mut w = Window::new(view);
    let update = w.update(None);
    let images = find(&update, AkRole::Image);
    assert_eq!(images.len(), 1, "only the labelled picture");
    let logo = &images[0].1;
    assert_eq!(logo.label(), Some("Company logo"));
    let b = logo.bounds().expect("bounds");
    assert_eq!((b.width(), b.height()), (40.0, 30.0));
    let names: Vec<_> = find(&update, AkRole::Button)
      .iter()
      .map(|(_, n)| n.label().map(str::to_owned))
      .collect();
    assert_eq!(
      names,
      [
        Some("Save".to_owned()),
        Some("Close".to_owned())
      ]
    );
  }
}
