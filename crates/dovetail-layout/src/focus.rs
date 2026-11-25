//! Keyboard focus over laid-out boxes: the Tab order, focus by pointer, and
//! passing a key from the focused element up to its parents (M06).
//!
//! Focus is identified by a [`BoxKey`], which stays the same between
//! updates for as long as the element exists.

use dovetail_core::{FocusTarget, KeyEvent, Modifiers, NamedKey};

use crate::{BoxKey, Content, Layout, Rect};

impl Layout {
  pub(crate) fn index_of(
    &self,
    key: &BoxKey,
  ) -> Option<usize> {
    self.boxes.iter().position(|b| &b.key == key)
  }

  fn is_focusable(
    &self,
    i: usize,
  ) -> bool {
    let b = &self.boxes[i];
    matches!(&b.content, Content::Element(e) if e.focusable)
      && !b.rect.intersect(&b.clip).is_empty()
  }

  /// True when box `i` is `ancestor` or inside it, by parent.
  fn is_inside(
    &self,
    i: usize,
    ancestor: usize,
  ) -> bool {
    self
      .above(i, |b| std::ptr::eq(b, &self.boxes[ancestor]))
      .is_some()
  }

  /// The focusable elements, in Tab order: the order they appear in the
  /// view, from top to bottom, then those in layers. While a modal layer
  /// shows, only those inside the topmost one.
  pub fn focus_order(&self) -> Vec<&BoxKey> {
    let modal = self.layers.iter().rev().copied().find(|&l| {
      matches!(&self.boxes[l].content, Content::Element(e) if e.style.modal)
    });
    (0..self.boxes.len())
      .filter(|&i| self.is_focusable(i))
      .filter(|&i| modal.is_none_or(|m| self.is_inside(i, m)))
      .map(|i| &self.boxes[i].key)
      .collect()
  }

  /// Where focus goes for `target` (M10): the first element inside the
  /// element that has the target, in Tab order, that can take focus, or
  /// that element itself. `None` when no element has the target, or
  /// nothing there can take focus.
  pub fn focus_for(
    &self,
    target: FocusTarget,
  ) -> Option<BoxKey> {
    let at = self.boxes.iter().position(|b| {
      matches!(&b.content, Content::Element(e) if e.focus_target == Some(target))
    })?;
    (0..self.boxes.len())
      .filter(|&i| i != at && self.is_focusable(i) && self.is_inside(i, at))
      .chain(self.is_focusable(at).then_some(at))
      .map(|i| self.boxes[i].key.clone())
      .next()
  }

  /// True when some element has `target`.
  pub fn has_focus_target(
    &self,
    target: FocusTarget,
  ) -> bool {
    self.boxes.iter().any(|b| {
      matches!(&b.content, Content::Element(e) if e.focus_target == Some(target))
    })
  }

  /// Where Tab (or Shift+Tab, `backwards`) moves focus from `current`. It
  /// wraps round at the ends. `None` when nothing can take focus.
  pub fn focus_next(
    &self,
    current: Option<&BoxKey>,
    backwards: bool,
  ) -> Option<BoxKey> {
    let order = self.focus_order();
    if order.is_empty() {
      return None;
    }
    let at = current.and_then(|c| order.iter().position(|k| *k == c));
    let next = match (at, backwards) {
      (None, false) => 0,
      (None, true) => order.len() - 1,
      (Some(i), false) => (i + 1) % order.len(),
      (Some(i), true) => (i + order.len() - 1) % order.len(),
    };
    Some(order[next].clone())
  }

  /// The focusable element under the point, for focus on click: the
  /// topmost box there, or the nearest focusable element above it.
  pub fn focusable_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<BoxKey> {
    let top = self.top_at(x, y)?;
    let mut at = Some(top);
    while let Some(i) = at {
      if self.is_focusable(i) {
        return Some(self.boxes[i].key.clone());
      }
      at = self.parents[i];
    }
    None
  }

  /// True when `key` is an element that can have focus now.
  pub fn can_focus(
    &self,
    key: &BoxKey,
  ) -> bool {
    self
      .index_of(key)
      .is_some_and(|i| self.is_focusable(i))
  }

  /// Where the focused element is, and its corner radius, for drawing the
  /// focus ring.
  pub fn focus_ring(
    &self,
    key: &BoxKey,
  ) -> Option<(Rect, f32)> {
    let b = &self.boxes[self.index_of(key)?];
    let radius = match &b.content {
      Content::Element(e) => e.style.radius,
      Content::Text { .. } | Content::Picture { .. } => 0.0,
    };
    Some((b.rect, radius))
  }

  /// Offers `event` to the focused element, then to each parent in turn,
  /// until a key handler takes it. If none does, Enter or Space presses the
  /// focused element, as a click would. Returns true when something handled
  /// the key.
  ///
  /// With nothing focused, the key goes to the elements that handle keys
  /// but cannot take focus, such as shortcut scopes, topmost first.
  pub fn dispatch_key(
    &self,
    focus: Option<&BoxKey>,
    event: &KeyEvent,
  ) -> bool {
    let Some(start) = focus.and_then(|f| self.index_of(f)) else {
      return (0..self.boxes.len()).rev().any(|i| {
        matches!(
          &self.boxes[i].content,
          Content::Element(e) if !e.focusable && e.on_key.as_ref().is_some_and(|h| h.handle(event))
        )
      });
    };
    let mut at = Some(start);
    while let Some(i) = at {
      if let Content::Element(e) = &self.boxes[i].content
        && let Some(handler) = &e.on_key
        && handler.handle(event)
      {
        return true;
      }
      at = self.parents[i];
    }
    let activate = (event.is(NamedKey::Enter) || event.is(NamedKey::Space))
      && event.modifiers == Modifiers::default();
    if activate
      && let Content::Element(e) = &self.boxes[start].content
      && let Some(on_press) = &e.on_press
    {
      on_press.call(());
      return true;
    }
    false
  }
}
