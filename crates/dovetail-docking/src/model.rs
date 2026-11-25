//! The docking layout as plain data: splits, tab stacks and floating
//! stacks, and the moves that change it. No windows or drawing here.

use dovetail::WindowPlace;

/// Which way a split places its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
  /// Side by side, left to right.
  Row,
  /// One above another, top to bottom.
  Column,
}

/// A side of a stack or of the whole dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
  /// The left.
  Left,
  /// The right.
  Right,
  /// The top.
  Top,
  /// The bottom.
  Bottom,
}

impl Side {
  /// The axis a split along this side has.
  pub fn axis(self) -> Axis {
    match self {
      Side::Left | Side::Right => Axis::Row,
      Side::Top | Side::Bottom => Axis::Column,
    }
  }

  /// True for the left and top: the new part goes first.
  pub fn first(self) -> bool {
    matches!(self, Side::Left | Side::Top)
  }
}

/// Panels shown as tabs, one at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Tabs {
  /// The panels' ids, in tab order. Never empty.
  pub panels: Vec<String>,
  /// The index of the panel shown.
  pub active: usize,
}

impl Tabs {
  /// One panel.
  pub fn one(panel: &str) -> Tabs {
    Tabs {
      panels: vec![panel.to_string()],
      active: 0,
    }
  }

  /// These panels, the first one shown.
  pub fn of(panels: &[&str]) -> Tabs {
    Tabs {
      panels: panels.iter().map(|p| p.to_string()).collect(),
      active: 0,
    }
  }

  /// The panel shown.
  pub fn active_panel(&self) -> &str {
    &self.panels[self.active.min(self.panels.len() - 1)]
  }
}

/// A part of the docked layout.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
  /// Parts side by side or one above another. Each has a share of the
  /// space; the shares add up to 1.
  Split {
    /// Which way.
    axis: Axis,
    /// Each part's share, and the part. At least two.
    parts: Vec<(f32, Node)>,
  },
  /// A stack of tabs.
  Tabs(Tabs),
}

impl Node {
  /// A split of these parts.
  pub fn split(
    axis: Axis,
    parts: Vec<(f32, Node)>,
  ) -> Node {
    Node::Split { axis, parts }
  }

  /// The node at `path`, where each number picks a split's part.
  pub fn at(
    &self,
    path: &[usize],
  ) -> Option<&Node> {
    match (path.split_first(), self) {
      (None, _) => Some(self),
      (Some((&i, rest)), Node::Split { parts, .. }) => parts.get(i)?.1.at(rest),
      (Some(_), Node::Tabs(_)) => None,
    }
  }

  fn at_mut(
    &mut self,
    path: &[usize],
  ) -> Option<&mut Node> {
    match path.split_first() {
      None => Some(self),
      Some((&i, rest)) => match self {
        Node::Split { parts, .. } => parts.get_mut(i)?.1.at_mut(rest),
        Node::Tabs(_) => None,
      },
    }
  }

  /// Every tab stack with its path, in reading order.
  fn stacks(
    &self,
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, Tabs)>,
  ) {
    match self {
      Node::Tabs(t) => out.push((path.clone(), t.clone())),
      Node::Split { parts, .. } => {
        for (i, (_, part)) in parts.iter().enumerate() {
          path.push(i);
          part.stacks(path, out);
          path.pop();
        }
      },
    }
  }
}

/// A tab stack in its own window.
#[derive(Clone, Debug, PartialEq)]
pub struct Floating {
  /// Names the stack while the program runs. Not saved.
  pub id: u64,
  /// The panels.
  pub tabs: Tabs,
  /// Where its window is, when known.
  pub place: Option<WindowPlace>,
  /// A docked panel to put it back next to, the one it was beside when it
  /// floated.
  pub home: Option<String>,
}

/// Where a tab stack is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StackAt {
  /// In the main window, at this path.
  Docked(Vec<usize>),
  /// In the floating window with this id.
  Floating(u64),
}

/// Where a moved panel goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
  /// Into a tab stack, as a tab before `index`.
  Tabs {
    /// The stack.
    stack: StackAt,
    /// The tab it goes before; the number of tabs puts it last.
    index: usize,
  },
  /// Beside a docked stack, in a new stack.
  Beside {
    /// The stack's path.
    stack: Vec<usize>,
    /// Which side of it.
    side: Side,
  },
  /// Along a side of the whole dock.
  Edge(Side),
  /// In a new floating window.
  Float(Option<WindowPlace>),
}

/// The share a new part gets along an edge of the dock.
const EDGE_SHARE: f32 = 0.25;

/// The whole docking layout: the main window's tree, and the floating
/// stacks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockLayout {
  /// The main window's layout; `None` when every panel floats.
  pub root: Option<Node>,
  /// The floating stacks.
  pub floating: Vec<Floating>,
}

impl DockLayout {
  /// A layout with `root` docked and nothing floating.
  pub fn new(root: Node) -> DockLayout {
    DockLayout { root: Some(root), floating: Vec::new() }
  }

  /// Every tab stack, docked ones first in reading order, then floating
  /// ones.
  pub fn stacks(&self) -> Vec<(StackAt, Tabs)> {
    let mut docked = Vec::new();
    if let Some(root) = &self.root {
      root.stacks(&mut Vec::new(), &mut docked);
    }
    docked
      .into_iter()
      .map(|(p, t)| (StackAt::Docked(p), t))
      .chain(
        self
          .floating
          .iter()
          .map(|f| (StackAt::Floating(f.id), f.tabs.clone())),
      )
      .collect()
  }

  /// Every panel, in the order of [`DockLayout::stacks`].
  pub fn panels(&self) -> Vec<String> {
    self
      .stacks()
      .into_iter()
      .flat_map(|(_, t)| t.panels)
      .collect()
  }

  /// The stack `at`.
  pub fn tabs(
    &self,
    at: &StackAt,
  ) -> Option<&Tabs> {
    match at {
      StackAt::Docked(path) => match self.root.as_ref()?.at(path)? {
        Node::Tabs(t) => Some(t),
        Node::Split { .. } => None,
      },
      StackAt::Floating(id) => self
        .floating
        .iter()
        .find(|f| f.id == *id)
        .map(|f| &f.tabs),
    }
  }

  fn tabs_mut(
    &mut self,
    at: &StackAt,
  ) -> Option<&mut Tabs> {
    match at {
      StackAt::Docked(path) => match self.root.as_mut()?.at_mut(path)? {
        Node::Tabs(t) => Some(t),
        Node::Split { .. } => None,
      },
      StackAt::Floating(id) => self
        .floating
        .iter_mut()
        .find(|f| f.id == *id)
        .map(|f| &mut f.tabs),
    }
  }

  /// Where `panel` is: its stack and its index there.
  pub fn find(
    &self,
    panel: &str,
  ) -> Option<(StackAt, usize)> {
    self.stacks().into_iter().find_map(|(at, t)| {
      t.panels
        .iter()
        .position(|p| p == panel)
        .map(|i| (at, i))
    })
  }

  /// Shows `panel` in its stack.
  pub fn activate(
    &mut self,
    panel: &str,
  ) {
    if let Some((at, i)) = self.find(panel)
      && let Some(t) = self.tabs_mut(&at)
    {
      t.active = i;
    }
  }

  /// Sets the shares of the split at `path`. They are scaled to add up
  /// to 1; a wrong count is ignored.
  pub fn set_shares(
    &mut self,
    path: &[usize],
    shares: &[f32],
  ) {
    let Some(Node::Split { parts, .. }) =
      self.root.as_mut().and_then(|r| r.at_mut(path))
    else {
      return;
    };
    let total: f32 = shares.iter().sum();
    if shares.len() != parts.len() || total <= 0.0 {
      return;
    }
    for (part, share) in parts.iter_mut().zip(shares) {
      part.0 = share / total;
    }
  }

  /// Sets where floating stack `id`'s window is.
  pub fn set_place(
    &mut self,
    id: u64,
    place: WindowPlace,
  ) {
    if let Some(f) = self.floating.iter_mut().find(|f| f.id == id) {
      f.place = Some(place);
    }
  }

  /// Takes `panel` out, and tidies what is left. True when it was there.
  pub fn remove(
    &mut self,
    panel: &str,
  ) -> bool {
    let Some((at, i)) = self.find(panel) else {
      return false;
    };
    let t = self.tabs_mut(&at).expect("found");
    t.panels.remove(i);
    if t.active > i || t.active >= t.panels.len() {
      t.active = t.active.saturating_sub(1);
    }
    self
      .floating
      .retain(|f| !f.tabs.panels.is_empty());
    self.root = self.root.take().and_then(tidy);
    true
  }

  /// Moves `panel` to `place`. A panel not in the layout yet is added.
  /// Moving a stack's only panel beside or into that stack does nothing.
  pub fn move_panel(
    &mut self,
    panel: &str,
    place: Place,
  ) {
    let from = self.find(panel);
    // The target is named by a panel that stays, since paths change when
    // the panel leaves.
    let anchor = |layout: &DockLayout, at: &StackAt| {
      layout.tabs(at).and_then(|t| {
        t.panels
          .iter()
          .find(|p| p.as_str() != panel)
          .cloned()
      })
    };
    match place {
      Place::Tabs { stack, index } => {
        let Some(anchor) = anchor(self, &stack) else {
          return;
        };
        let index = match &from {
          Some((at, i)) if *at == stack && *i < index => index - 1,
          _ => index,
        };
        self.remove(panel);
        let Some((at, _)) = self.find(&anchor) else {
          return;
        };
        let t = self.tabs_mut(&at).expect("found");
        let index = index.min(t.panels.len());
        t.panels.insert(index, panel.to_string());
        t.active = index;
      },
      Place::Beside { stack, side } => {
        let Some(anchor) = anchor(self, &StackAt::Docked(stack)) else {
          return;
        };
        self.remove(panel);
        let Some((StackAt::Docked(path), _)) = self.find(&anchor) else {
          return;
        };
        self.insert_beside(&path, side, Node::Tabs(Tabs::one(panel)));
      },
      Place::Edge(side) => {
        self.remove(panel);
        self.insert_at_edge(side, Node::Tabs(Tabs::one(panel)));
      },
      Place::Float(place) => {
        // Remember a neighbour, to go back to.
        let home = from.as_ref().and_then(|(at, _)| match at {
          StackAt::Docked(_) => anchor(self, at).or_else(|| {
            self
              .stacks()
              .into_iter()
              .filter(|(a, _)| matches!(a, StackAt::Docked(_)) && a != at)
              .map(|(_, t)| t.active_panel().to_string())
              .next()
          }),
          StackAt::Floating(id) => self
            .floating
            .iter()
            .find(|f| f.id == *id)
            .and_then(|f| f.home.clone()),
        });
        self.remove(panel);
        let id = self
          .floating
          .iter()
          .map(|f| f.id)
          .max()
          .unwrap_or(0)
          + 1;
        self.floating.push(Floating {
          id,
          tabs: Tabs::one(panel),
          place,
          home,
        });
      },
    }
  }

  /// Puts floating stack `id`'s panels back in the main window: as tabs
  /// next to the panel it floated from, or along the right edge.
  pub fn dock_floating(
    &mut self,
    id: u64,
  ) {
    let Some(i) = self.floating.iter().position(|f| f.id == id) else {
      return;
    };
    let f = self.floating.remove(i);
    let home = f
      .home
      .as_deref()
      .and_then(|h| match self.find(h) {
        Some((at @ StackAt::Docked(_), _)) => Some(at),
        _ => None,
      });
    match home {
      Some(at) => {
        let t = self.tabs_mut(&at).expect("found");
        let first = t.panels.len();
        t.panels.extend(f.tabs.panels);
        t.active = first + f.tabs.active;
      },
      None => self.insert_at_edge(Side::Right, Node::Tabs(f.tabs)),
    }
  }

  fn insert_beside(
    &mut self,
    path: &[usize],
    side: Side,
    new: Node,
  ) {
    let axis = side.axis();
    // In a split that already goes this way, the new part takes half of
    // the target's share.
    if let Some((&last, parent)) = path.split_last()
      && let Some(Node::Split { axis: a, parts }) =
        self.root.as_mut().and_then(|r| r.at_mut(parent))
      && *a == axis
    {
      let half = parts[last].0 / 2.0;
      parts[last].0 = half;
      let at = if side.first() { last } else { last + 1 };
      parts.insert(at, (half, new));
      return;
    }
    let Some(target) = self.root.as_mut().and_then(|r| r.at_mut(path)) else {
      return;
    };
    let old = std::mem::replace(target, Node::Tabs(Tabs::one("")));
    let parts = if side.first() {
      vec![(0.5, new), (0.5, old)]
    } else {
      vec![(0.5, old), (0.5, new)]
    };
    *target = Node::Split { axis, parts };
  }

  fn insert_at_edge(
    &mut self,
    side: Side,
    new: Node,
  ) {
    let axis = side.axis();
    self.root = Some(match self.root.take() {
      None => new,
      Some(Node::Split { axis: a, mut parts }) if a == axis => {
        for part in &mut parts {
          part.0 *= 1.0 - EDGE_SHARE;
        }
        let at = if side.first() { 0 } else { parts.len() };
        parts.insert(at, (EDGE_SHARE, new));
        Node::Split { axis, parts }
      },
      Some(old) => {
        let parts = if side.first() {
          vec![
            (EDGE_SHARE, new),
            (1.0 - EDGE_SHARE, old),
          ]
        } else {
          vec![
            (1.0 - EDGE_SHARE, old),
            (EDGE_SHARE, new),
          ]
        };
        Node::Split { axis, parts }
      },
    });
  }

  /// Keeps only the panels in `known`, and adds the known panels that are
  /// missing to the first stack, or as the only stack. Used after reading
  /// a saved layout, when the application's panels may have changed.
  pub fn keep_known(
    &mut self,
    known: &[&str],
  ) {
    for panel in self.panels() {
      if !known.contains(&panel.as_str()) {
        self.remove(&panel);
      }
    }
    let present = self.panels();
    let missing: Vec<&str> = known
      .iter()
      .copied()
      .filter(|k| !present.iter().any(|p| p == k))
      .collect();
    if missing.is_empty() {
      return;
    }
    let first = self
      .stacks()
      .into_iter()
      .map(|(at, _)| at)
      .find(|at| matches!(at, StackAt::Docked(_)));
    match first {
      Some(at) => {
        let t = self.tabs_mut(&at).expect("found");
        t.panels
          .extend(missing.iter().map(|m| m.to_string()));
      },
      None => self.insert_at_edge(Side::Left, Node::Tabs(Tabs::of(&missing))),
    }
  }
}

/// Removes empty stacks and splits, puts a split's only part in its place,
/// and merges a split into a parent that goes the same way.
fn tidy(node: Node) -> Option<Node> {
  match node {
    Node::Tabs(t) if t.panels.is_empty() => None,
    Node::Tabs(t) => Some(Node::Tabs(t)),
    Node::Split { axis, parts } => {
      let mut kept: Vec<(f32, Node)> = Vec::new();
      for (share, part) in parts {
        match tidy(part) {
          None => {},
          Some(Node::Split { axis: a, parts: inner }) if a == axis => {
            kept.extend(inner.into_iter().map(|(s, n)| (s * share, n)));
          },
          Some(part) => kept.push((share, part)),
        }
      }
      let total: f32 = kept.iter().map(|p| p.0).sum();
      let count = kept.len() as f32;
      for part in &mut kept {
        part.0 = if total > 0.0 {
          part.0 / total
        } else {
          1.0 / count
        };
      }
      match kept.len() {
        0 => None,
        1 => kept.pop().map(|p| p.1),
        _ => Some(Node::Split { axis, parts: kept }),
      }
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// files | (board / console), with inspector beside board's stack.
  fn sample() -> DockLayout {
    DockLayout::new(Node::split(
      Axis::Row,
      vec![
        (0.25, Node::Tabs(Tabs::of(&["files", "layers"]))),
        (
          0.75,
          Node::split(
            Axis::Column,
            vec![
              (0.7, Node::Tabs(Tabs::one("board"))),
              (0.3, Node::Tabs(Tabs::one("console"))),
            ],
          ),
        ),
      ],
    ))
  }

  fn path_of(
    layout: &DockLayout,
    panel: &str,
  ) -> Vec<usize> {
    match layout.find(panel) {
      Some((StackAt::Docked(p), _)) => p,
      other => panic!("{panel} is not docked: {other:?}"),
    }
  }

  #[test]
  fn a_panel_moves_into_another_stack_as_a_tab() {
    let mut l = sample();
    l.move_panel(
      "console",
      Place::Tabs {
        stack: StackAt::Docked(vec![0]),
        index: 1,
      },
    );
    // The column with one part left becomes the board's stack.
    assert_eq!(
      l,
      DockLayout::new(Node::split(
        Axis::Row,
        vec![
          (
            0.25,
            Node::Tabs(Tabs {
              panels: vec![
                "files".into(),
                "console".into(),
                "layers".into()
              ],
              active: 1,
            })
          ),
          (0.75, Node::Tabs(Tabs::one("board"))),
        ]
      ))
    );
  }

  #[test]
  fn a_tab_moves_along_its_own_stack() {
    let mut l = sample();
    l.move_panel(
      "files",
      Place::Tabs {
        stack: StackAt::Docked(vec![0]),
        index: 2,
      },
    );
    let t = l.tabs(&StackAt::Docked(vec![0])).unwrap();
    assert_eq!(t.panels, ["layers", "files"]);
    assert_eq!(t.active, 1);
  }

  #[test]
  fn a_panel_docks_beside_a_stack_on_each_side() {
    for side in [
      Side::Left,
      Side::Right,
      Side::Top,
      Side::Bottom,
    ] {
      let mut l = sample();
      l.move_panel("layers", Place::Beside { stack: vec![1, 0], side });
      let board = path_of(&l, "board");
      let layers = path_of(&l, "layers");
      // They are neighbours in the same split, in the right order.
      assert_eq!(board[..board.len() - 1], layers[..layers.len() - 1]);
      let (b, s) = (board[board.len() - 1], layers[layers.len() - 1]);
      if side.first() {
        assert_eq!(s + 1, b, "{side:?}");
      } else {
        assert_eq!(b + 1, s, "{side:?}");
      }
      let parent = l
        .root
        .as_ref()
        .unwrap()
        .at(&board[..board.len() - 1]);
      let Some(Node::Split { axis, parts }) = parent else {
        panic!("a split");
      };
      assert_eq!(*axis, side.axis());
      let total: f32 = parts.iter().map(|p| p.0).sum();
      assert!((total - 1.0).abs() < 1e-5);
    }
  }

  #[test]
  fn docking_below_in_a_column_halves_the_target() {
    let mut l = sample();
    l.move_panel(
      "layers",
      Place::Beside { stack: vec![1, 0], side: Side::Bottom },
    );
    let Some(Node::Split { parts, .. }) = l.root.as_ref().unwrap().at(&[1])
    else {
      panic!("a split");
    };
    let shares: Vec<f32> = parts.iter().map(|p| p.0).collect();
    assert_eq!(shares, [0.35, 0.35, 0.3]);
  }

  #[test]
  fn a_panel_docks_along_an_edge() {
    let mut l = sample();
    l.move_panel("console", Place::Edge(Side::Right));
    let Some(Node::Split { axis, parts }) = &l.root else {
      panic!("a split");
    };
    assert_eq!(*axis, Axis::Row);
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[2].1, Node::Tabs(Tabs::one("console")));
    assert!((parts[2].0 - EDGE_SHARE).abs() < 1e-5);

    l.move_panel("files", Place::Edge(Side::Bottom));
    let Some(Node::Split { axis, parts }) = &l.root else {
      panic!("a split");
    };
    assert_eq!(*axis, Axis::Column);
    assert_eq!(parts[1].1, Node::Tabs(Tabs::one("files")));
  }

  #[test]
  fn a_panel_floats_and_docks_back_beside_its_neighbour() {
    let mut l = sample();
    l.move_panel("layers", Place::Float(None));
    assert_eq!(l.floating.len(), 1);
    assert_eq!(l.floating[0].home.as_deref(), Some("files"));
    assert!(
      !l.tabs(&StackAt::Docked(vec![0]))
        .unwrap()
        .panels
        .contains(&"layers".into())
    );

    let id = l.floating[0].id;
    l.dock_floating(id);
    assert!(l.floating.is_empty());
    let t = l.tabs(&StackAt::Docked(vec![0])).unwrap();
    assert_eq!(t.panels, ["files", "layers"]);
    assert_eq!(t.active_panel(), "layers");
  }

  #[test]
  fn the_last_panel_of_a_floating_stack_closes_it() {
    let mut l = sample();
    l.move_panel("console", Place::Float(None));
    let id = l.floating[0].id;
    l.move_panel(
      "console",
      Place::Tabs {
        stack: StackAt::Docked(vec![0]),
        index: 0,
      },
    );
    assert!(l.floating.is_empty());
    assert_eq!(l.find("console"), Some((StackAt::Docked(vec![0]), 0)));
    let _ = id;
  }

  #[test]
  fn moving_a_lone_panel_beside_itself_does_nothing() {
    let mut l = sample();
    let before = l.clone();
    l.move_panel(
      "board",
      Place::Beside { stack: vec![1, 0], side: Side::Left },
    );
    assert_eq!(l, before);
  }

  #[test]
  fn shares_are_scaled_to_add_up_to_one() {
    let mut l = sample();
    l.set_shares(&[], &[1.0, 3.0]);
    let Some(Node::Split { parts, .. }) = &l.root else {
      panic!("a split");
    };
    assert_eq!(parts[0].0, 0.25);
    assert_eq!(parts[1].0, 0.75);
    l.set_shares(&[], &[1.0]);
    assert_eq!(l, {
      let mut s = sample();
      s.set_shares(&[], &[1.0, 3.0]);
      s
    });
  }

  #[test]
  fn unknown_panels_go_and_missing_ones_are_added() {
    let mut l = sample();
    l.keep_known(&["files", "board", "console", "inspector"]);
    let panels = l.panels();
    assert!(!panels.contains(&"layers".into()));
    assert!(panels.contains(&"inspector".into()));
    assert_eq!(
      l.tabs(&StackAt::Docked(vec![0])).unwrap().panels,
      ["files", "inspector"]
    );
  }
}
