//! Finding elements by what they are, as assistive tools do.

use std::fmt;

use dovetail_core::Role;
use dovetail_layout::{BoxKey, Rect};
use dovetail_session::{AccessId, AccessKind, AccessNode, AccessTree, Toggle};

/// What to look for: a role, a label, a text and states, in any mix.
/// Start one with [`role`], [`label`] or [`text`], then add more.
///
/// ```
/// use dovetail_core::Role;
/// use dovetail_testing::role;
///
/// let save = role(Role::Button).label("Save");
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Find {
  role: Option<Role>,
  label: Option<String>,
  text: Option<String>,
  toggled: Option<bool>,
  disabled: Option<bool>,
}

/// Elements with this role. Text matches [`Role::Label`], and a picture
/// with a label matches [`Role::Image`].
pub fn role(role: Role) -> Find {
  Find::default().role(role)
}

/// Elements named `label`: what a screen reader calls them.
pub fn label(label: impl Into<String>) -> Find {
  Find::default().label(label)
}

/// Elements whose text is `text`: their name, or else their value, as the
/// words of a label or what is in a text field.
pub fn text(text: impl Into<String>) -> Find {
  Find::default().text(text)
}

impl Find {
  /// Only elements with this role.
  pub fn role(
    mut self,
    role: Role,
  ) -> Self {
    self.role = Some(role);
    self
  }

  /// Only elements named `label`.
  pub fn label(
    mut self,
    label: impl Into<String>,
  ) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Only elements whose text is `text`.
  pub fn text(
    mut self,
    text: impl Into<String>,
  ) -> Self {
    self.text = Some(text.into());
    self
  }

  /// Only elements switched on (`true`) or off: toggles, check boxes, and
  /// selected tabs, radio buttons and options.
  pub fn toggled(
    mut self,
    on: bool,
  ) -> Self {
    self.toggled = Some(on);
    self
  }

  /// Only elements that are disabled (`true`) or not.
  pub fn disabled(
    mut self,
    disabled: bool,
  ) -> Self {
    self.disabled = Some(disabled);
    self
  }

  /// True when `node` matches.
  pub(crate) fn matches(
    &self,
    node: &AccessNode,
  ) -> bool {
    if node.kind == AccessKind::Window {
      return false;
    }
    if let Some(want) = self.role {
      let role = match node.kind {
        AccessKind::Label => Role::Label,
        AccessKind::Image => Role::Image,
        AccessKind::Element(role) => role,
        AccessKind::Window => return false,
      };
      if role != want {
        return false;
      }
    }
    if let Some(want) = &self.label
      && node.label.as_deref() != Some(want.as_str())
    {
      return false;
    }
    if let Some(want) = &self.text
      && text_of(node) != Some(want.as_str())
    {
      return false;
    }
    if let Some(want) = self.toggled
      && on_of(node) != Some(want)
    {
      return false;
    }
    if let Some(want) = self.disabled
      && node.disabled != want
    {
      return false;
    }
    true
  }
}

impl fmt::Display for Find {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    let mut parts = Vec::new();
    if let Some(role) = self.role {
      parts.push(format!("role {role:?}"));
    }
    if let Some(label) = &self.label {
      parts.push(format!("label {label:?}"));
    }
    if let Some(text) = &self.text {
      parts.push(format!("text {text:?}"));
    }
    if let Some(on) = self.toggled {
      parts.push(if on { "switched on" } else { "switched off" }.into());
    }
    if let Some(disabled) = self.disabled {
      parts.push(if disabled { "disabled" } else { "enabled" }.into());
    }
    if parts.is_empty() {
      write!(f, "any element")
    } else {
      write!(f, "{}", parts.join(", "))
    }
  }
}

/// A node's text: its name, or else its value.
fn text_of(node: &AccessNode) -> Option<&str> {
  node.label.as_deref().or(node.value.as_deref())
}

/// Whether a node is switched on, from its toggle or its selection.
fn on_of(node: &AccessNode) -> Option<bool> {
  match node.toggled {
    Some(Toggle::On) => Some(true),
    Some(Toggle::Off) => Some(false),
    Some(Toggle::Mixed) => None,
    None => node.selected,
  }
}

/// One line about a node, for messages: its role, name and value.
pub(crate) fn describe(node: &AccessNode) -> String {
  let kind = match node.kind {
    AccessKind::Window => "Window".to_owned(),
    AccessKind::Label => "Label".to_owned(),
    AccessKind::Image => "Image".to_owned(),
    AccessKind::Element(role) => format!("{role:?}"),
  };
  let mut line = kind;
  if let Some(label) = &node.label {
    line.push_str(&format!(" {label:?}"));
  }
  if let Some(value) = &node.value {
    line.push_str(&format!(" = {value:?}"));
  }
  match on_of(node) {
    Some(true) => line.push_str(", on"),
    Some(false) => line.push_str(", off"),
    None if node.toggled == Some(Toggle::Mixed) => line.push_str(", mixed"),
    None => {},
  }
  if node.disabled {
    line.push_str(", disabled");
  }
  line
}

/// The whole tree, one node a line, indented under its parent.
pub(crate) fn dump(tree: &AccessTree) -> String {
  fn walk(
    tree: &AccessTree,
    id: AccessId,
    depth: usize,
    out: &mut String,
  ) {
    let Some(node) = tree.get(id) else {
      return;
    };
    let focus = if tree.focus == id && id != AccessId::ROOT {
      " (focused)"
    } else {
      ""
    };
    out.push_str(&format!(
      "{}{}{focus}\n",
      "  ".repeat(depth),
      describe(node)
    ));
    for &child in &node.children {
      walk(tree, child, depth + 1, out);
    }
  }
  let mut out = String::new();
  walk(tree, AccessId::ROOT, 0, &mut out);
  out
}

/// An element that a search found, as it was then.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
  node: AccessNode,
  focused: bool,
}

impl Found {
  pub(crate) fn new(
    node: AccessNode,
    focused: bool,
  ) -> Self {
    Self { node, focused }
  }

  /// Everything assistive tools are told about it.
  pub fn node(&self) -> &AccessNode {
    &self.node
  }

  /// Its role. Text is [`Role::Label`], and a picture [`Role::Image`].
  pub fn role(&self) -> Role {
    match self.node.kind {
      AccessKind::Label => Role::Label,
      AccessKind::Image => Role::Image,
      AccessKind::Element(role) => role,
      AccessKind::Window => Role::None,
    }
  }

  /// Its name: what a screen reader calls it.
  pub fn label(&self) -> Option<&str> {
    self.node.label.as_deref()
  }

  /// Its value: a label's words, or what is in a text field.
  pub fn value(&self) -> Option<&str> {
    self.node.value.as_deref()
  }

  /// Its text: its name, or else its value.
  pub fn text(&self) -> Option<&str> {
    text_of(&self.node)
  }

  /// Whether it is switched on or selected; `None` when it cannot be, or
  /// is mixed.
  pub fn is_on(&self) -> Option<bool> {
    on_of(&self.node)
  }

  /// True when it is disabled.
  pub fn is_disabled(&self) -> bool {
    self.node.disabled
  }

  /// True when it had keyboard focus.
  pub fn is_focused(&self) -> bool {
    self.focused
  }

  /// Where it is, in logical pixels.
  pub fn rect(&self) -> Rect {
    self.node.bounds.unwrap_or_default()
  }

  /// The middle of it, where a click lands.
  pub fn centre(&self) -> (f32, f32) {
    let r = self.rect();
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
  }

  pub(crate) fn key(&self) -> Option<&BoxKey> {
    self.node.key.as_ref()
  }
}

/// Why a search did not find exactly one element.
#[derive(Clone, Debug, PartialEq)]
pub struct FindError {
  /// What was searched for.
  pub search: String,
  /// What matched: none, or more than one.
  pub matches: Vec<String>,
  /// The window's accessibility tree, one node a line.
  pub tree: String,
}

impl fmt::Display for FindError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    if self.matches.is_empty() {
      writeln!(f, "no element matches {}.", self.search)?;
      write!(f, "The window has:\n{}", self.tree)
    } else {
      writeln!(
        f,
        "{} elements match {}, but one was wanted:",
        self.matches.len(),
        self.search
      )?;
      for m in &self.matches {
        writeln!(f, "  {m}")?;
      }
      write!(
        f,
        "Search for more, such as a role or a label, or search within an \
         element."
      )
    }
  }
}

impl std::error::Error for FindError {}

/// Every node under `within` (the window when `None`) that matches
/// `search`, a parent before its children.
pub(crate) fn search(
  tree: &AccessTree,
  within: Option<AccessId>,
  search: &Find,
) -> Vec<Found> {
  let mut out = Vec::new();
  let mut stack = vec![within.unwrap_or(AccessId::ROOT)];
  let mut first = true;
  while let Some(id) = stack.pop() {
    let Some(node) = tree.get(id) else {
      continue;
    };
    if !first && search.matches(node) {
      out.push(Found::new(node.clone(), tree.focus == id));
    }
    first = false;
    stack.extend(node.children.iter().rev());
  }
  out
}

/// The one node under `within` that matches `wanted`, or why there is not
/// exactly one.
pub(crate) fn one(
  tree: &AccessTree,
  within: Option<AccessId>,
  wanted: &Find,
) -> Result<Found, FindError> {
  let mut found = search(tree, within, wanted);
  if found.len() == 1 {
    return Ok(found.remove(0));
  }
  let mut what = wanted.to_string();
  if let Some(id) = within
    && let Some(node) = tree.get(id)
  {
    what.push_str(&format!(" inside {}", describe(node)));
  }
  Err(FindError {
    search: what,
    matches: found
      .iter()
      .map(|f| {
        let r = f.rect();
        format!(
          "{} at ({}, {}), {} by {}",
          describe(f.node()),
          r.x,
          r.y,
          r.width,
          r.height
        )
      })
      .collect(),
    tree: dump(tree),
  })
}

/// The node with keyboard focus, if an element has it.
pub(crate) fn focused(tree: &AccessTree) -> Option<Found> {
  let id = tree.focus;
  if id == AccessId::ROOT {
    return None;
  }
  tree.get(id).cloned().map(|n| Found::new(n, true))
}
