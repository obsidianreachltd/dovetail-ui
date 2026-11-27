//! Sizing, alignment and layout primitives for Dovetail UI.
//!
//! [`Layout::update`] turns a component [`Tree`] into a box tree and places
//! every box. Results are cached for each box between updates. A box is
//! laid out again only when its own content, a child's, or the space it is
//! offered has changed; everything else is reused.
//!
//! Units are logical pixels. Painting multiplies by the display scale.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

mod focus;
mod scroll;
mod text;

pub use dovetail_core::{FontWeight, Picture};
pub use scroll::{SCROLLBAR_THIN, SCROLLBAR_WIDTH, Scrollbar};
pub use text::{CursorGeometry, cursor_geometry, first_text};

use dovetail_core::{
  Align, Callback, Color, ComponentId, Element, ElementKind, Justify, Layer,
  Size, Style, Tree, View,
};

/// The default size of text with no `font_size` in any parent.
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// The size and weight text is shown at (M14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
  /// Font size in logical pixels.
  pub size: f32,
  /// How heavy it is.
  pub weight: FontWeight,
}

impl From<f32> for TextStyle {
  /// Regular text at this size.
  fn from(size: f32) -> TextStyle {
    TextStyle { size, weight: FontWeight::REGULAR }
  }
}

/// Measures text for layout.
pub trait Measure {
  /// The width and height of `text` in `font`. Each `\n` starts a new
  /// line; lines do not wrap.
  fn text(
    &self,
    text: &str,
    font: TextStyle,
  ) -> (f32, f32);

  /// The height of one line at `font`.
  fn line_height(
    &self,
    font: TextStyle,
  ) -> f32 {
    self.text("", font).1
  }

  /// How far from the start of `line` (one line, with no line break) a
  /// caret before byte `at` goes.
  fn caret_x(
    &self,
    line: &str,
    font: TextStyle,
    at: usize,
  ) -> f32 {
    self.text(&line[..at], font).0
  }

  /// The pieces of `line` that bytes `from..to` cover, as left and right
  /// edges from the line's left end, from left to right (M13): one piece
  /// for text in one direction, more when it mixes directions.
  fn selection(
    &self,
    line: &str,
    font: TextStyle,
    from: usize,
    to: usize,
  ) -> Vec<(f32, f32)> {
    let a = self.caret_x(line, font, from);
    let b = self.caret_x(line, font, to);
    vec![(a.min(b), a.max(b))]
  }

  /// True when `line` is a paragraph that runs right to left (M13).
  fn is_rtl(
    &self,
    _line: &str,
    _font: TextStyle,
  ) -> bool {
    false
  }

  /// Where `line` starts in a box `width` wide: a line that runs right to
  /// left lines up on the right (M13).
  fn line_start(
    &self,
    line: &str,
    font: TextStyle,
    width: f32,
  ) -> f32 {
    if self.is_rtl(line, font) {
      (width - self.text(line, font).0).max(0.0)
    } else {
      0.0
    }
  }

  /// Each grapheme cluster of `line`, in text order: its first byte, and
  /// its left and right edges (M13).
  fn graphemes(
    &self,
    line: &str,
    font: TextStyle,
  ) -> Vec<(usize, f32, f32)> {
    let mut bounds: Vec<usize> = line.char_indices().map(|(i, _)| i).collect();
    bounds.push(line.len());
    bounds
      .windows(2)
      .map(|w| {
        let a = self.caret_x(line, font, w[0]);
        let b = self.caret_x(line, font, w[1]);
        (w[0], a.min(b), a.max(b))
      })
      .collect()
  }

  /// The place in `line` (one line, with no line break) nearest to `x`:
  /// where a click there puts the caret.
  fn byte_at(
    &self,
    line: &str,
    font: TextStyle,
    x: f32,
  ) -> usize {
    line
      .char_indices()
      .map(|(i, _)| i)
      .chain([line.len()])
      .min_by(|&a, &b| {
        (self.caret_x(line, font, a) - x)
          .abs()
          .total_cmp(&(self.caret_x(line, font, b) - x).abs())
      })
      .unwrap_or(0)
  }
}

/// Fixed-pitch measuring, as the built-in M04 font draws (D0012): every
/// character is `advance` × the font size wide, and a line is `line` × the
/// font size high.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedPitch {
  /// Width of each character, as a share of the font size.
  pub advance: f32,
  /// Height of a line, as a share of the font size.
  pub line: f32,
}

impl Default for FixedPitch {
  fn default() -> Self {
    Self { advance: 0.6, line: 1.25 }
  }
}

impl Measure for FixedPitch {
  fn text(
    &self,
    text: &str,
    font: TextStyle,
  ) -> (f32, f32) {
    let font_size = font.size;
    // One line per `\n`; the widest line sets the width.
    let widest = text
      .split('\n')
      .map(|l| l.chars().count())
      .max()
      .unwrap_or(0) as f32;
    let lines = text.split('\n').count() as f32;
    (
      widest * self.advance * font_size,
      lines * self.line * font_size,
    )
  }
}

/// A rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Width.
  pub width: f32,
  /// Height.
  pub height: f32,
}

impl Rect {
  /// A rectangle from its left, top, width and height.
  pub const fn new(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
  ) -> Self {
    Self { x, y, width, height }
  }

  /// True when the point is inside.
  pub fn contains(
    &self,
    x: f32,
    y: f32,
  ) -> bool {
    x >= self.x
      && y >= self.y
      && x < self.x + self.width
      && y < self.y + self.height
  }

  /// The overlap of two rectangles, which may be empty.
  pub fn intersect(
    &self,
    other: &Rect,
  ) -> Rect {
    let x = self.x.max(other.x);
    let y = self.y.max(other.y);
    let right = (self.x + self.width).min(other.x + other.width);
    let bottom = (self.y + self.height).min(other.y + other.height);
    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
  }

  /// The smallest rectangle around both.
  pub fn union(
    &self,
    other: &Rect,
  ) -> Rect {
    if self.is_empty() {
      return *other;
    }
    if other.is_empty() {
      return *self;
    }
    let x = self.x.min(other.x);
    let y = self.y.min(other.y);
    let right = (self.x + self.width).max(other.x + other.width);
    let bottom = (self.y + self.height).max(other.y + other.height);
    Rect::new(x, y, right - x, bottom - y)
  }

  /// True when it has no area.
  pub fn is_empty(&self) -> bool {
    self.width <= 0.0 || self.height <= 0.0
  }
}

/// Identifies a box between updates: the component whose view made it, and
/// its position inside that view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoxKey {
  /// The component whose view contains the box.
  pub owner: ComponentId,
  /// The child positions from the top of that view down to the box.
  pub path: Rc<[u32]>,
}

/// What a box shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
  /// An element, with its style.
  Element(Rc<Element>),
  /// A line of text, with the size, colour and weight it inherited.
  Text {
    /// The text.
    text: Rc<str>,
    /// Font size in logical pixels.
    font_size: f32,
    /// Text colour.
    color: Color,
    /// How heavy it is (M14).
    weight: FontWeight,
  },
  /// An image or icon (M17), with the text colour and size around it.
  Picture {
    /// The picture.
    picture: Picture,
    /// The colour an icon is drawn in: its own, or the text's around it.
    color: Color,
    /// The font size around it, which an icon's size follows.
    font_size: f32,
  },
}

impl Content {
  /// The size and weight of text, or `None` for an element.
  pub fn text_style(&self) -> Option<TextStyle> {
    match self {
      Content::Text { font_size, weight, .. } => {
        Some(TextStyle { size: *font_size, weight: *weight })
      },
      Content::Element(_) | Content::Picture { .. } => None,
    }
  }
}

/// A placed box.
#[derive(Clone, Debug)]
pub struct LayoutBox {
  /// Its identity between updates.
  pub key: BoxKey,
  /// What it shows.
  pub content: Content,
  /// Where it is, in window coordinates.
  pub rect: Rect,
  /// The area it may draw into, after every clipping parent.
  pub clip: Rect,
  /// Its children, as indexes into [`Layout::boxes`].
  pub children: Vec<usize>,
  /// How solid it is, from 0 to 1: its own `Style::opacity` times its
  /// parents' (M18).
  pub opacity: f32,
}

/// Counts from one [`Layout::update`], to check that only changed parts are
/// laid out again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutStats {
  /// Boxes in the tree.
  pub boxes: usize,
  /// Boxes whose size was worked out again.
  pub laid_out: usize,
  /// Boxes whose earlier result was reused.
  pub reused: usize,
}

/// What one [`Layout::update`] changed.
#[derive(Clone, Debug, Default)]
pub struct LayoutChanges {
  /// Areas that must be painted again: the old and new places of every box
  /// that moved, changed size, or changed how it looks, and of every box
  /// added or removed.
  pub damage: Vec<Rect>,
  /// The counts of work done.
  pub stats: LayoutStats,
}

impl LayoutChanges {
  /// All the damage in one rectangle, or `None` when nothing changed.
  pub fn damage_bounds(&self) -> Option<Rect> {
    let bounds = self
      .damage
      .iter()
      .fold(Rect::default(), |a, b| a.union(b));
    (!bounds.is_empty()).then_some(bounds)
  }
}

/// The space offered to a box, and any size its parent requires.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Offer {
  width: f32,
  height: f32,
  force_width: Option<f32>,
  force_height: Option<f32>,
}

/// A box's result, kept between updates.
#[derive(Clone, Debug)]
struct Cached {
  signature: u64,
  offer: Offer,
  size: (f32, f32),
  /// Children's positions inside this box.
  offsets: Vec<(f32, f32)>,
}

struct Node {
  key: BoxKey,
  content: Content,
  signature: u64,
  children: Vec<usize>,
  /// This box, or something inside it, changed since the last update.
  changed: bool,
}

/// The box tree and its cached layout.
#[derive(Default)]
pub struct Layout {
  boxes: Vec<LayoutBox>,
  /// Each box's parent, as an index into `boxes`.
  parents: Vec<Option<usize>>,
  cache: HashMap<BoxKey, Cached>,
  /// Where each box was painted last time, and how it looked.
  painted: HashMap<BoxKey, (Rect, u64)>,
  /// How far each scrolling element is scrolled.
  scroll: HashMap<BoxKey, scroll::ScrollState>,
  /// The boxes that are layers, in drawing order.
  layers: Vec<usize>,
}

impl Layout {
  /// An empty layout.
  pub fn new() -> Self {
    Self::default()
  }

  /// The placed boxes, in painting order: parents before children. The
  /// root is the first.
  pub fn boxes(&self) -> &[LayoutBox] {
    &self.boxes
  }

  /// Lays out `tree` in a window of `width` × `height`. Reuses every
  /// earlier result that is still right.
  pub fn update(
    &mut self,
    tree: &Tree,
    width: f32,
    height: f32,
    measure: &dyn Measure,
  ) -> LayoutChanges {
    // Children are built before their parent, so the root is the last box.
    let mut nodes = Vec::new();
    let mut root_box = None;
    let mut layers = Vec::new();
    if let Some(root) = tree.root() {
      let mut builder = Builder {
        tree,
        nodes: &mut nodes,
        cache: &self.cache,
        layers: Vec::new(),
        layer_depth: 0,
      };
      let roots = builder.component(root, Inherited::default());
      root_box = match roots[..] {
        [] => None,
        [only] => Some(only),
        // Several top-level boxes are placed in one stack.
        _ => Some(builder.push_key(
          root_key(root),
          Content::Element(Rc::new(Element::new(ElementKind::Stack))),
          roots,
        )),
      };
      layers = std::mem::take(&mut builder.layers);
    }
    // Outer layers before the layers inside them, which go on top.
    layers.sort_by_key(|l| l.depth);

    let mut stats = LayoutStats {
      boxes: nodes.len(),
      ..LayoutStats::default()
    };
    let mut cache = HashMap::with_capacity(nodes.len());
    let mut boxes: Vec<LayoutBox> = Vec::with_capacity(nodes.len());
    let mut order = vec![usize::MAX; nodes.len()];
    if let Some(root) = root_box {
      let offer = Offer {
        width,
        height,
        force_width: Some(width),
        force_height: Some(height),
      };
      let mut solver = Solver {
        nodes: &nodes,
        old: &self.cache,
        new: &mut cache,
        measure,
        stats: &mut stats,
      };
      solver.size(root, offer);
      // Layers are sized against the window; one that fills it is forced
      // to its size.
      for layer in &layers {
        let fill = matches!(
          &nodes[layer.node].content,
          Content::Element(e) if e.style.layer == Some(Layer::Fill)
        );
        let offer = Offer {
          width,
          height,
          force_width: fill.then_some(width),
          force_height: fill.then_some(height),
        };
        solver.size(layer.node, offer);
      }
      // Place every box in window coordinates: the tree, then the layers
      // above it, so they are drawn last.
      let window = Rect::new(0.0, 0.0, width, height);
      place(
        &nodes,
        &cache,
        &self.scroll,
        root,
        (0.0, 0.0),
        window,
        1.0,
        &mut boxes,
        &mut order,
      );
      for layer in &layers {
        let size = cache[&nodes[layer.node].key].size;
        let parent = layer
          .parent
          .and_then(|p| order.get(p).copied())
          .and_then(|i| boxes.get(i))
          .map(|b| b.rect);
        let at =
          layer_position(&nodes[layer.node].content, parent, size, window);
        // A layer fades with the element it belongs to.
        let opacity = layer
          .parent
          .and_then(|p| order.get(p).copied())
          .and_then(|i| boxes.get(i))
          .map_or(1.0, |b| b.opacity);
        place(
          &nodes,
          &cache,
          &self.scroll,
          layer.node,
          at,
          window,
          opacity,
          &mut boxes,
          &mut order,
        );
      }
    }
    for b in &mut boxes {
      for c in &mut b.children {
        *c = order[*c];
      }
    }
    self.scroll = scroll::settle(&mut boxes, &self.scroll, measure);

    // Damage: what moved, changed size or changed look, added or removed.
    let mut damage = Vec::new();
    let mut seen = HashSet::with_capacity(boxes.len());
    let mut painted = HashMap::with_capacity(boxes.len());
    for b in &boxes {
      let look =
        look_of(&b.content) ^ u64::from(b.opacity.to_bits()).rotate_left(32);
      let visible = b.rect.intersect(&b.clip);
      match self.painted.get(&b.key) {
        Some((old, old_look)) if *old == visible && *old_look == look => {},
        Some((old, _)) => damage.extend([*old, visible]),
        None => damage.push(visible),
      }
      seen.insert(b.key.clone());
      painted.insert(b.key.clone(), (visible, look));
    }
    for (key, (old, _)) in &self.painted {
      if !seen.contains(key) {
        damage.push(*old);
      }
    }
    damage.retain(|r| !r.is_empty());

    let mut parents = vec![None; boxes.len()];
    for (i, b) in boxes.iter().enumerate() {
      for &child in &b.children {
        parents[child] = Some(i);
      }
    }
    // A layer's parent is the element it belongs to, so keys reach it.
    self.layers.clear();
    for layer in &layers {
      let Some(&i) = order
        .get(layer.node)
        .filter(|&&i| i != usize::MAX)
      else {
        continue;
      };
      parents[i] = layer.parent.and_then(|p| order.get(p).copied());
      self.layers.push(i);
    }
    self.parents = parents;
    self.boxes = boxes;
    self.cache = cache;
    self.painted = painted;
    LayoutChanges { damage, stats }
  }

  /// The press handler of the element under the point, if any: the topmost
  /// box there, or the nearest element above it that has one.
  pub fn hit_test(
    &self,
    x: f32,
    y: f32,
  ) -> Option<&Callback<()>> {
    let key = self.press_target_at(x, y)?;
    self.element(&key)?.on_press.as_ref()
  }

  /// The element whose press handler [`Layout::hit_test`] finds at the
  /// point. Its key stays the same when the element is built again, as
  /// when a press moves focus and the focus changes what is shown (B0015).
  pub fn press_target_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<BoxKey> {
    let i = self.above(
      self.top_at(x, y)?,
      |b| matches!(&b.content, Content::Element(e) if e.on_press.is_some()),
    )?;
    Some(self.boxes[i].key.clone())
  }

  /// The topmost box under the point: the last one drawn there.
  pub(crate) fn top_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<usize> {
    (0..self.boxes.len()).rev().find(|&i| {
      let b = &self.boxes[i];
      b.rect.contains(x, y) && b.clip.contains(x, y)
    })
  }

  /// Box `i` or the nearest box above it, by parent, that matches `test`.
  pub(crate) fn above(
    &self,
    i: usize,
    test: impl Fn(&LayoutBox) -> bool,
  ) -> Option<usize> {
    let mut at = Some(i);
    while let Some(i) = at {
      if test(&self.boxes[i]) {
        return Some(i);
      }
      at = self.parents[i];
    }
    None
  }

  /// The boxes that are layers, in the order they are drawn.
  pub fn layers(&self) -> &[usize] {
    &self.layers
  }
}

/// Where a layer goes in the window, kept inside it.
fn layer_position(
  content: &Content,
  parent: Option<Rect>,
  (w, h): (f32, f32),
  window: Rect,
) -> (f32, f32) {
  let Content::Element(e) = content else {
    return (0.0, 0.0);
  };
  let fit = |x: f32, y: f32| {
    (
      x.min(window.width - w).max(0.0),
      y.min(window.height - h).max(0.0),
    )
  };
  match e.style.layer {
    Some(Layer::Fill) | None => (0.0, 0.0),
    Some(Layer::At(x, y)) => fit(x, y),
    Some(Layer::TopCentre(top)) => fit((window.width - w) / 2.0, top),
    Some(Layer::Below) => {
      let p = parent.unwrap_or_default();
      let below = p.y + p.height;
      // Above the parent when there is no room below it.
      let y = if below + h > window.height && p.y - h >= 0.0 {
        p.y - h
      } else {
        below
      };
      fit(p.x, y)
    },
  }
}

fn root_key(root: ComponentId) -> BoxKey {
  BoxKey { owner: root, path: Rc::from([u32::MAX]) }
}

/// Text size and colour, passed down from parents.
#[derive(Clone, Copy)]
struct Inherited {
  font_size: f32,
  color: Color,
  weight: FontWeight,
}

impl Default for Inherited {
  fn default() -> Self {
    Self {
      font_size: DEFAULT_FONT_SIZE,
      color: Color::BLACK,
      weight: FontWeight::REGULAR,
    }
  }
}

struct Builder<'a> {
  tree: &'a Tree,
  nodes: &'a mut Vec<Node>,
  cache: &'a HashMap<BoxKey, Cached>,
  /// Layers lifted out of their parents, to place against the window.
  layers: Vec<LayerNode>,
  /// How many layers the element being built is inside.
  layer_depth: u32,
}

/// A layer: its node, the node of the element it belongs to, and how many
/// layers it is inside.
struct LayerNode {
  node: usize,
  parent: Option<usize>,
  depth: u32,
}

impl Builder<'_> {
  /// The boxes made by component `id`'s view.
  fn component(
    &mut self,
    id: ComponentId,
    inherited: Inherited,
  ) -> Vec<usize> {
    let Some(output) = self.tree.output(id) else {
      return Vec::new();
    };
    let mut children = self.tree.children(id).iter().copied();
    let mut out = Vec::new();
    self.view(
      output,
      id,
      &mut Vec::new(),
      inherited,
      &mut children,
      &mut out,
    );
    out
  }

  fn view(
    &mut self,
    view: &View,
    owner: ComponentId,
    path: &mut Vec<u32>,
    inherited: Inherited,
    components: &mut dyn Iterator<Item = ComponentId>,
    out: &mut Vec<usize>,
  ) {
    match view {
      View::Empty => {},
      View::Text(text) => {
        let content = Content::Text {
          text: text.clone(),
          font_size: inherited.font_size,
          color: inherited.color,
          weight: inherited.weight,
        };
        out.push(self.push(owner, path, content, Vec::new()));
      },
      View::Picture(picture) => {
        let content = Content::Picture {
          picture: picture.clone(),
          color: picture.color.unwrap_or(inherited.color),
          font_size: inherited.font_size,
        };
        out.push(self.push(owner, path, content, Vec::new()));
      },
      View::Fragment(views) => {
        for (i, view) in views.iter().enumerate() {
          path.push(i as u32);
          self.view(view, owner, path, inherited, components, out);
          path.pop();
        }
      },
      View::Component(_) => {
        if let Some(child) = components.next() {
          out.extend(self.component(child, inherited));
        }
      },
      View::Element(element) => {
        let inherited = Inherited {
          weight: element
            .style
            .font_weight
            .unwrap_or(inherited.weight),
          font_size: element
            .style
            .font_size
            .unwrap_or(inherited.font_size),
          color: element
            .style
            .text_color
            .unwrap_or(inherited.color),
        };
        let is_layer = element.style.layer.is_some();
        let first_layer = self.layers.len();
        if is_layer {
          self.layer_depth += 1;
        }
        let mut children = Vec::new();
        for (i, view) in element.children.iter().enumerate() {
          path.push(i as u32);
          self.view(view, owner, path, inherited, components, &mut children);
          path.pop();
        }
        if is_layer {
          self.layer_depth -= 1;
        }
        let index =
          self.push(owner, path, Content::Element(element.clone()), children);
        // Layers found inside belong to this element.
        for layer in &mut self.layers[first_layer..] {
          if layer.parent.is_none() {
            layer.parent = Some(index);
          }
        }
        if is_layer {
          self.layers.push(LayerNode {
            node: index,
            parent: None,
            depth: self.layer_depth,
          });
        } else {
          out.push(index);
        }
      },
    }
  }

  fn push(
    &mut self,
    owner: ComponentId,
    path: &[u32],
    content: Content,
    children: Vec<usize>,
  ) -> usize {
    self.push_key(BoxKey { owner, path: path.into() }, content, children)
  }

  fn push_key(
    &mut self,
    key: BoxKey,
    content: Content,
    children: Vec<usize>,
  ) -> usize {
    let signature = signature_of(&content, &children, self.nodes);
    let changed = children.iter().any(|&c| self.nodes[c].changed)
      || self
        .cache
        .get(&key)
        .is_none_or(|c| c.signature != signature);
    self.nodes.push(Node {
      key,
      content,
      signature,
      children,
      changed,
    });
    self.nodes.len() - 1
  }
}

/// What a box's own size depends on: its content, its layout style, and
/// which children it has.
fn signature_of(
  content: &Content,
  children: &[usize],
  nodes: &[Node],
) -> u64 {
  let mut h = DefaultHasher::new();
  match content {
    Content::Element(e) => {
      0u8.hash(&mut h);
      hash_layout_style(e.kind, &e.style, &mut h);
    },
    Content::Text { text, font_size, .. } => {
      1u8.hash(&mut h);
      text.hash(&mut h);
      font_size.to_bits().hash(&mut h);
    },
    Content::Picture { picture, font_size, .. } => {
      2u8.hash(&mut h);
      let (w, h2) = picture.natural_size(*font_size);
      w.to_bits().hash(&mut h);
      h2.to_bits().hash(&mut h);
    },
  }
  for &c in children {
    nodes[c].key.hash(&mut h);
  }
  h.finish()
}

fn hash_layout_style(
  kind: ElementKind,
  s: &Style,
  h: &mut DefaultHasher,
) {
  kind.hash(h);
  for v in [
    s.padding.top,
    s.padding.right,
    s.padding.bottom,
    s.padding.left,
    s.gap,
    s.min_width,
    s.min_height,
    s.max_width,
    s.max_height,
    s.grow,
    s.border.width,
  ] {
    v.to_bits().hash(h);
  }
  s.align.hash(h);
  s.justify.hash(h);
  for size in [s.width, s.height] {
    match size {
      Size::Fit => 0u8.hash(h),
      Size::Fill => 1u8.hash(h),
      Size::Fixed(v) => {
        2u8.hash(h);
        v.to_bits().hash(h);
      },
    }
  }
}

/// How a box looks, apart from where it is.
fn look_of(content: &Content) -> u64 {
  let mut h = DefaultHasher::new();
  match content {
    Content::Element(e) => {
      let s = &e.style;
      for v in [
        s.background.r,
        s.background.g,
        s.background.b,
        s.background.a,
        s.border.width,
        s.border.color.r,
        s.border.color.g,
        s.border.color.b,
        s.border.color.a,
        s.radius,
      ] {
        v.to_bits().hash(&mut h);
      }
      s.clip.hash(&mut h);
      e.text_cursor.hash(&mut h);
      // A canvas with new drawing is painted again.
      e.canvas.as_ref().map(|c| c.id()).hash(&mut h);
    },
    Content::Text { text, font_size, color, weight } => {
      text.hash(&mut h);
      weight.hash(&mut h);
      for v in [
        *font_size, color.r, color.g, color.b, color.a,
      ] {
        v.to_bits().hash(&mut h);
      }
    },
    Content::Picture { picture, color, .. } => {
      picture.id().hash(&mut h);
      picture.fit.hash(&mut h);
      for v in [color.r, color.g, color.b, color.a] {
        v.to_bits().hash(&mut h);
      }
    },
  }
  h.finish()
}

struct Solver<'a> {
  nodes: &'a [Node],
  old: &'a HashMap<BoxKey, Cached>,
  new: &'a mut HashMap<BoxKey, Cached>,
  measure: &'a dyn Measure,
  stats: &'a mut LayoutStats,
}

impl Solver<'_> {
  /// The size of box `index` for `offer`, reusing the last result when
  /// nothing it depends on has changed.
  fn size(
    &mut self,
    index: usize,
    offer: Offer,
  ) -> (f32, f32) {
    let (nodes, old) = (self.nodes, self.old);
    let node = &nodes[index];
    if let Some(cached) = self.new.get(&node.key)
      && cached.offer == offer
    {
      return cached.size;
    }
    if !node.changed
      && let Some(cached) = old.get(&node.key)
      && cached.offer == offer
    {
      self.reuse(index);
      return cached.size;
    }
    self.stats.laid_out += 1;
    let (size, offsets) = match &node.content {
      Content::Text { text, .. } => {
        let style = node.content.text_style().expect("text");
        let (w, h) = self.measure.text(text, style);
        (
          (
            offer.force_width.unwrap_or(w),
            offer.force_height.unwrap_or(h),
          ),
          Vec::new(),
        )
      },
      Content::Picture { picture, font_size, .. } => {
        let (w, h) = picture.natural_size(*font_size);
        (
          (
            offer.force_width.unwrap_or(w),
            offer.force_height.unwrap_or(h),
          ),
          Vec::new(),
        )
      },
      Content::Element(element) => self.element(index, element, offer),
    };
    let signature = node.signature;
    self
      .new
      .insert(node.key.clone(), Cached { signature, offer, size, offsets });
    size
  }

  /// Copies a whole unchanged subtree's results from the last update.
  fn reuse(
    &mut self,
    index: usize,
  ) {
    let (nodes, old) = (self.nodes, self.old);
    let node = &nodes[index];
    let Some(cached) = old.get(&node.key) else {
      return;
    };
    self.stats.reused += 1;
    self.new.insert(node.key.clone(), cached.clone());
    for &child in &node.children {
      self.reuse(child);
    }
  }

  fn element(
    &mut self,
    index: usize,
    element: &Element,
    offer: Offer,
  ) -> ((f32, f32), Vec<(f32, f32)>) {
    let s = &element.style;
    let inset_x = s.padding.horizontal() + 2.0 * s.border.width;
    let inset_y = s.padding.vertical() + 2.0 * s.border.width;

    // The box's own size where it is already known.
    let own =
      |size: Size, force: Option<f32>, available: f32, min: f32, max: f32| {
        force
          .or(match size {
            Size::Fixed(v) => Some(v),
            Size::Fill if available.is_finite() => Some(available),
            _ => None,
          })
          .map(|v| v.clamp(min, max.max(min)))
      };
    let known_w = own(
      s.width,
      offer.force_width,
      offer.width,
      s.min_width,
      s.max_width,
    );
    let known_h = own(
      s.height,
      offer.force_height,
      offer.height,
      s.min_height,
      s.max_height,
    );
    let inner_w =
      (known_w.unwrap_or(offer.width.min(s.max_width)) - inset_x).max(0.0);
    let inner_h =
      (known_h.unwrap_or(offer.height.min(s.max_height)) - inset_y).max(0.0);

    let children = self.nodes[index].children.clone();
    let (content, mut offsets) = match element.kind {
      ElementKind::Row => {
        self.line(&children, s, true, inner_w, inner_h, known_h.is_some())
      },
      ElementKind::Column => {
        self.line(&children, s, false, inner_h, inner_w, known_w.is_some())
      },
      ElementKind::Stack | ElementKind::Box => self.stack(
        &children,
        s,
        inner_w,
        inner_h,
        known_w.is_some(),
        known_h.is_some(),
      ),
    };

    let width = known_w.unwrap_or(
      (content.0 + inset_x).clamp(s.min_width, s.max_width.max(s.min_width)),
    );
    let height = known_h.unwrap_or(
      (content.1 + inset_y).clamp(s.min_height, s.max_height.max(s.min_height)),
    );
    let origin = (
      s.padding.left + s.border.width,
      s.padding.top + s.border.width,
    );

    // Justify and align now that the final size is known.
    let (inner_w, inner_h) =
      ((width - inset_x).max(0.0), (height - inset_y).max(0.0));
    // A row or column that stretches its children, but did not know its
    // size across until they were measured (as a menu with a smallest
    // width), stretches them now (M15).
    let row = element.kind == ElementKind::Row;
    let cross_known = if row {
      known_h.is_some()
    } else {
      known_w.is_some()
    };
    if matches!(element.kind, ElementKind::Row | ElementKind::Column)
      && s.align == Align::Stretch
      && !cross_known
    {
      let cross = if row { inner_h } else { inner_w };
      for &c in &children {
        let (w, h) = self.new[&self.nodes[c].key].size;
        let (along, across) = if row { (w, h) } else { (h, w) };
        if (across - cross).abs() > 0.01 {
          let (fw, fh) = if row { (along, cross) } else { (cross, along) };
          self.size(
            c,
            Offer {
              width: fw,
              height: fh,
              force_width: Some(fw),
              force_height: Some(fh),
            },
          );
        }
      }
    }
    let sizes: Vec<(f32, f32)> = children
      .iter()
      .map(|c| self.new[&self.nodes[*c].key].size)
      .collect();
    match element.kind {
      ElementKind::Row | ElementKind::Column => {
        let row = element.kind == ElementKind::Row;
        let (main, cross) = if row {
          (inner_w, inner_h)
        } else {
          (inner_h, inner_w)
        };
        let used: f32 = sizes
          .iter()
          .map(|&(w, h)| if row { w } else { h })
          .sum::<f32>()
          + s.gap * children.len().saturating_sub(1) as f32;
        let free = (main - used).max(0.0);
        let (mut at, between) = match s.justify {
          Justify::Start => (0.0, s.gap),
          Justify::Center => (free / 2.0, s.gap),
          Justify::End => (free, s.gap),
          Justify::SpaceBetween if children.len() > 1 => {
            (0.0, s.gap + free / (children.len() - 1) as f32)
          },
          Justify::SpaceBetween => (0.0, s.gap),
        };
        for (offset, &(w, h)) in offsets.iter_mut().zip(&sizes) {
          let (along, across) = if row { (w, h) } else { (h, w) };
          let cross_at = align(s.align, cross, across);
          *offset = if row { (at, cross_at) } else { (cross_at, at) };
          at += along + between;
        }
      },
      ElementKind::Stack | ElementKind::Box => {
        for ((offset, &(w, h)), &child) in offsets
          .iter_mut()
          .zip(&sizes)
          .zip(children.iter())
        {
          // Text whose paragraph runs right to left lines up on the right
          // (M13, D0018), unless the parent places it otherwise.
          let across = match (&self.nodes[child].content, s.align) {
            (
              content @ Content::Text { text, .. },
              Align::Start | Align::Stretch,
            ) if self.measure.is_rtl(
              text.split('\n').next().unwrap_or(""),
              content.text_style().expect("text"),
            ) =>
            {
              Align::End
            },
            _ => s.align,
          };
          *offset = (align(across, inner_w, w), align(s.align, inner_h, h));
        }
      },
    }
    for offset in &mut offsets {
      offset.0 += origin.0;
      offset.1 += origin.1;
    }
    ((width, height), offsets)
  }

  /// Lays out a row (`row`) or column along its main axis.
  fn line(
    &mut self,
    children: &[usize],
    s: &Style,
    row: bool,
    main: f32,
    cross: f32,
    cross_known: bool,
  ) -> ((f32, f32), Vec<(f32, f32)>) {
    let offer_for = |along: Option<f32>, stretch: bool| {
      let across = (stretch && cross_known).then_some(cross);
      if row {
        Offer {
          width: main,
          height: cross,
          force_width: along,
          force_height: across,
        }
      } else {
        Offer {
          width: cross,
          height: main,
          force_width: across,
          force_height: along,
        }
      }
    };
    let gaps = s.gap * children.len().saturating_sub(1) as f32;
    let grow = |c: usize| match &self.nodes[c].content {
      Content::Element(e) => e.style.grow,
      Content::Text { .. } | Content::Picture { .. } => 0.0,
    };

    // Children that do not grow take their own size first.
    let mut used = gaps;
    let mut sizes = vec![(0.0, 0.0); children.len()];
    for (i, &c) in children.iter().enumerate() {
      if grow(c) <= 0.0 {
        sizes[i] = self.size(c, offer_for(None, s.align == Align::Stretch));
        used += if row { sizes[i].0 } else { sizes[i].1 };
      }
    }
    // Growing children share what is left.
    let total: f32 = children.iter().map(|&c| grow(c).max(0.0)).sum();
    if total > 0.0 {
      let free = if main.is_finite() {
        (main - used).max(0.0)
      } else {
        0.0
      };
      for (i, &c) in children.iter().enumerate() {
        if grow(c) > 0.0 {
          sizes[i] = self.size(
            c,
            offer_for(Some(free * grow(c) / total), s.align == Align::Stretch),
          );
        }
      }
    }
    let along: f32 = sizes
      .iter()
      .map(|&(w, h)| if row { w } else { h })
      .sum::<f32>()
      + gaps;
    let across = sizes
      .iter()
      .map(|&(w, h)| if row { h } else { w })
      .fold(0.0, f32::max);
    let content = if row {
      (along, across)
    } else {
      (across, along)
    };
    (content, vec![(0.0, 0.0); children.len()])
  }

  fn stack(
    &mut self,
    children: &[usize],
    s: &Style,
    inner_w: f32,
    inner_h: f32,
    width_known: bool,
    height_known: bool,
  ) -> ((f32, f32), Vec<(f32, f32)>) {
    let stretch = s.align == Align::Stretch;
    let offer = Offer {
      width: inner_w,
      height: inner_h,
      force_width: (stretch && width_known).then_some(inner_w),
      force_height: (stretch && height_known).then_some(inner_h),
    };
    let mut content = (0.0f32, 0.0f32);
    for &c in children {
      let (w, h) = self.size(c, offer);
      content = (content.0.max(w), content.1.max(h));
    }
    (content, vec![(0.0, 0.0); children.len()])
  }
}

fn align(
  align: Align,
  space: f32,
  size: f32,
) -> f32 {
  let free = if space.is_finite() {
    (space - size).max(0.0)
  } else {
    0.0
  };
  match align {
    Align::Start | Align::Stretch => 0.0,
    Align::Center => free / 2.0,
    Align::End => free,
  }
}

#[allow(clippy::too_many_arguments)]
fn place(
  nodes: &[Node],
  cache: &HashMap<BoxKey, Cached>,
  scrolled: &HashMap<BoxKey, scroll::ScrollState>,
  index: usize,
  (x, y): (f32, f32),
  clip: Rect,
  opacity: f32,
  boxes: &mut Vec<LayoutBox>,
  order: &mut [usize],
) {
  let node = &nodes[index];
  let cached = &cache[&node.key];
  // An offset moves the box and everything in it (M18).
  let (x, y, opacity) = match &node.content {
    Content::Element(e) => (
      x + e.style.offset.0,
      y + e.style.offset.1,
      opacity * e.style.opacity.clamp(0.0, 1.0),
    ),
    _ => (x, y, opacity),
  };
  let rect = Rect::new(x, y, cached.size.0, cached.size.1);
  let inner_clip = match &node.content {
    Content::Element(e) if e.style.clip => clip.intersect(&rect),
    _ => clip,
  };
  // Scrolling moves the children, not the element.
  let (sx, sy) = match &node.content {
    Content::Element(e) if e.style.scroll => scrolled
      .get(&node.key)
      .map_or((0.0, 0.0), |s| s.offset),
    _ => (0.0, 0.0),
  };
  order[index] = boxes.len();
  boxes.push(LayoutBox {
    key: node.key.clone(),
    content: node.content.clone(),
    rect,
    clip,
    children: node.children.clone(),
    opacity,
  });
  for (&child, &(dx, dy)) in node.children.iter().zip(&cached.offsets) {
    place(
      nodes,
      cache,
      scrolled,
      child,
      (x + dx - sx, y + dy - sy),
      inner_clip,
      opacity,
      boxes,
      order,
    );
  }
}
