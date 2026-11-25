//! Scrolling (M06): elements with `Style::scroll` can show children that
//! are larger than they are, moved into view. Layout keeps each one's
//! offset between updates, keeps it within the content, and, while the
//! element has a shown caret, moves it just enough to show the caret when
//! the caret or the text changes. The mouse wheel moves it through
//! [`Layout::scroll_by`].
//!
//! Scrollbars (M15, D0021) float over the inside edge of each scrolling
//! element whose content is larger than it is: [`Layout::scrollbars`] says
//! where, and windows draw and drag them.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use dovetail_core::TextCursor;

use crate::{
  BoxKey, Content, Layout, LayoutBox, Measure, Rect, cursor_geometry,
  first_text,
};

/// What layout remembers about one scrolling element.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScrollState {
  /// How far the children are moved left and up.
  pub offset: (f32, f32),
  /// The largest offset the content allows.
  pub max: (f32, f32),
  /// The caret and text last seen, while the caret is shown: a change
  /// scrolls to the caret.
  follow: Option<(TextCursor, u64)>,
  /// The serial of the last scroll request carried out (M16).
  applied: Option<u64>,
}

/// The box inside an element's border and padding, where its content is.
fn content_box(b: &LayoutBox) -> Rect {
  let Content::Element(e) = &b.content else {
    return b.rect;
  };
  let s = &e.style;
  let (left, top) = (
    s.padding.left + s.border.width,
    s.padding.top + s.border.width,
  );
  Rect::new(
    b.rect.x + left,
    b.rect.y + top,
    (b.rect.width - left - s.padding.right - s.border.width).max(0.0),
    (b.rect.height - top - s.padding.bottom - s.border.width).max(0.0),
  )
}

/// Moves box `index` and everything inside it.
fn shift(
  boxes: &mut [LayoutBox],
  index: usize,
  dx: f32,
  dy: f32,
) {
  boxes[index].rect.x += dx;
  boxes[index].rect.y += dy;
  for c in boxes[index].children.clone() {
    shift(boxes, c, dx, dy);
  }
}

/// After the boxes are placed with the old offsets: keeps each offset
/// within its content, follows a caret that changed, moves the children to
/// match, and returns the new states.
pub(crate) fn settle(
  boxes: &mut [LayoutBox],
  states: &HashMap<BoxKey, ScrollState>,
  measure: &dyn Measure,
) -> HashMap<BoxKey, ScrollState> {
  let mut next = HashMap::new();
  // Parents come first, so an outer element is settled before the ones
  // inside it, which then move with it.
  for i in 0..boxes.len() {
    let Content::Element(e) = &boxes[i].content else {
      continue;
    };
    if !e.style.scroll {
      continue;
    }
    let cursor = e.text_cursor;
    let request = e.scroll_to;
    let key = boxes[i].key.clone();
    let old = states.get(&key).cloned().unwrap_or_default();
    let (ox, oy) = old.offset;
    let c = content_box(&boxes[i]);

    // How far the content reaches, as if not scrolled.
    let (mut right, mut bottom) = (c.x + c.width, c.y + c.height);
    for &child in &boxes[i].children {
      let r = boxes[child].rect;
      right = right.max(r.x + r.width + ox);
      bottom = bottom.max(r.y + r.height + oy);
    }
    // The caret, as if not scrolled, while it is shown.
    let mut follow = None;
    let mut caret = None;
    if let Some(cursor) = cursor.filter(|c| c.show_caret)
      && let Some(t) = first_text(boxes, i)
      && let Content::Text { text, .. } = &boxes[t].content
    {
      let mut h = DefaultHasher::new();
      text.hash(&mut h);
      follow = Some((cursor, h.finish()));
      if let Some(r) = cursor_geometry(&boxes[t], &cursor, measure).caret {
        let r = Rect::new(r.x + ox, r.y + oy, r.width, r.height);
        right = right.max(r.x + r.width);
        bottom = bottom.max(r.y + r.height);
        caret = Some(r);
      }
    }
    let max = (
      (right - (c.x + c.width)).max(0.0),
      (bottom - (c.y + c.height)).max(0.0),
    );
    // A new request from the element moves it (M16).
    let (mut x, mut y) = (ox, oy);
    let mut applied = old.applied;
    if let Some(r) = request
      && applied != Some(r.serial)
    {
      x = r.x.unwrap_or(x);
      y = r.y.unwrap_or(y);
      applied = Some(r.serial);
    }
    let (mut x, mut y) = (x.clamp(0.0, max.0), y.clamp(0.0, max.1));
    if let Some(r) = caret
      && follow != old.follow
    {
      if r.x - x < c.x {
        x = r.x - c.x;
      } else if r.x + r.width - x > c.x + c.width {
        x = r.x + r.width - (c.x + c.width);
      }
      if r.y - y < c.y {
        y = r.y - c.y;
      } else if r.y + r.height - y > c.y + c.height {
        y = r.y + r.height - (c.y + c.height);
      }
      x = x.clamp(0.0, max.0);
      y = y.clamp(0.0, max.1);
    }
    if (x, y) != (ox, oy) {
      for child in boxes[i].children.clone() {
        shift(boxes, child, ox - x, oy - y);
      }
    }
    next.insert(key, ScrollState { offset: (x, y), max, follow, applied });
  }
  next
}

/// How wide a scrollbar is where it can be grabbed, and while the pointer
/// is over it, in logical pixels.
pub const SCROLLBAR_WIDTH: f32 = 12.0;
/// How wide a scrollbar's thumb is drawn while the pointer is not over it.
pub const SCROLLBAR_THIN: f32 = 4.0;
/// The shortest a thumb gets, so it can always be grabbed.
const MIN_THUMB: f32 = 24.0;

/// A scrollbar on a scrolling element (M15).
#[derive(Clone, Debug, PartialEq)]
pub struct Scrollbar {
  /// The scrolling element.
  pub key: BoxKey,
  /// Up and down, or (false) across.
  pub vertical: bool,
  /// What the thumb runs along, in window coordinates.
  pub track: Rect,
  /// The part that is dragged.
  pub thumb: Rect,
  /// The area it may draw into: the element's, inside every clipping
  /// parent.
  pub clip: Rect,
  /// The box drawn last inside the element, as an index into
  /// [`Layout::boxes`]: the bar is drawn after it, over the content and
  /// under any layer.
  pub after: usize,
  /// How far one page moves: the size of what can be seen.
  pub page: f32,
}

impl Scrollbar {
  /// The scroll offset that puts the thumb's start at `start`, measured
  /// along the track from its start, given the largest offset `max`.
  pub fn offset_for(
    &self,
    start: f32,
    max: f32,
  ) -> f32 {
    let (track, thumb) = if self.vertical {
      (self.track.height, self.thumb.height)
    } else {
      (self.track.width, self.thumb.width)
    };
    let room = (track - thumb).max(0.0);
    if room <= 0.0 {
      0.0
    } else {
      (start / room).clamp(0.0, 1.0) * max
    }
  }
}

/// The last box inside box `i`, itself if it has no children.
fn last_inside(
  boxes: &[LayoutBox],
  i: usize,
) -> usize {
  match boxes[i].children.last() {
    Some(&c) => last_inside(boxes, c),
    None => i,
  }
}

impl Layout {
  /// The scrollbars to show: one for each direction in which a scrolling
  /// element's content is larger than it is, unless the element turns
  /// them off (`Style::scrollbars`). Inner bars come before outer ones.
  pub fn scrollbars(&self) -> Vec<Scrollbar> {
    let mut bars = Vec::new();
    for (i, b) in self.boxes.iter().enumerate().rev() {
      let Content::Element(e) = &b.content else {
        continue;
      };
      if !e.style.scroll || !e.style.scrollbars {
        continue;
      }
      let Some(state) = self.scroll.get(&b.key) else {
        continue;
      };
      let border = e.style.border.width;
      let inner = Rect::new(
        b.rect.x + border,
        b.rect.y + border,
        (b.rect.width - 2.0 * border).max(0.0),
        (b.rect.height - 2.0 * border).max(0.0),
      );
      let view = content_box(b);
      let clip = b.rect.intersect(&b.clip);
      let after = last_inside(&self.boxes, i);
      let both = state.max.0 > 0.5 && state.max.1 > 0.5;
      // Where two bars meet, each stops short of the other.
      let corner = if both { SCROLLBAR_WIDTH } else { 0.0 };
      let thumb = |track: f32, view: f32, max: f32, offset: f32| {
        let length =
          (track * view / (view + max)).clamp(MIN_THUMB.min(track), track);
        let start = if max > 0.0 {
          offset / max * (track - length)
        } else {
          0.0
        };
        (start, length)
      };
      if state.max.1 > 0.5 && inner.height > 0.0 {
        let track = Rect::new(
          inner.x + inner.width - SCROLLBAR_WIDTH,
          inner.y,
          SCROLLBAR_WIDTH,
          (inner.height - corner).max(0.0),
        );
        let (start, length) =
          thumb(track.height, view.height, state.max.1, state.offset.1);
        bars.push(Scrollbar {
          key: b.key.clone(),
          vertical: true,
          track,
          thumb: Rect::new(track.x, track.y + start, track.width, length),
          clip,
          after,
          page: view.height,
        });
      }
      if state.max.0 > 0.5 && inner.width > 0.0 {
        let track = Rect::new(
          inner.x,
          inner.y + inner.height - SCROLLBAR_WIDTH,
          (inner.width - corner).max(0.0),
          SCROLLBAR_WIDTH,
        );
        let (start, length) =
          thumb(track.width, view.width, state.max.0, state.offset.0);
        bars.push(Scrollbar {
          key: b.key.clone(),
          vertical: false,
          track,
          thumb: Rect::new(track.x + start, track.y, length, track.height),
          clip,
          after,
          page: view.width,
        });
      }
    }
    bars
  }

  /// The scrollbar under the window point (`x`, `y`), if nothing else is
  /// drawn over it there.
  pub fn scrollbar_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<Scrollbar> {
    let top = self.top_at(x, y)?;
    self.scrollbars().into_iter().find(|bar| {
      let owner = self.index_of(&bar.key);
      bar.track.contains(x, y)
        && bar.clip.contains(x, y)
        // The topmost box there is the element or inside it, not a layer.
        && owner.is_some_and(|o| (o..=bar.after).contains(&top))
    })
  }

  /// Scrolls the element `key` to `x` across and `y` down, leaving an axis
  /// that is `None` as it is (M15). Returns true when it moved; it shows
  /// after the next [`Layout::update`].
  pub fn scroll_to(
    &mut self,
    key: &BoxKey,
    x: Option<f32>,
    y: Option<f32>,
  ) -> bool {
    let Some(state) = self.scroll.get_mut(key) else {
      return false;
    };
    let (ox, oy) = state.offset;
    let moved = (
      x.unwrap_or(ox).clamp(0.0, state.max.0),
      y.unwrap_or(oy).clamp(0.0, state.max.1),
    );
    if moved == (ox, oy) {
      return false;
    }
    state.offset = moved;
    true
  }

  /// Scrolls the deepest scrolling element under the window point (`x`,
  /// `y`) by (`dx`, `dy`) logical pixels: positive shows more of what is
  /// right and below. When it is already as far as it goes, the element
  /// around it scrolls instead. Returns true when something scrolled; it
  /// shows after the next [`Layout::update`].
  pub fn scroll_by(
    &mut self,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
  ) -> bool {
    // The scrolling elements from the topmost box under the point up.
    let mut under: Vec<BoxKey> = Vec::new();
    let mut at = self.top_at(x, y);
    while let Some(i) = at {
      let b = &self.boxes[i];
      if matches!(&b.content, Content::Element(e) if e.style.scroll) {
        under.push(b.key.clone());
      }
      at = self.parents[i];
    }
    for key in under {
      let Some(state) = self.scroll.get_mut(&key) else {
        continue;
      };
      let (ox, oy) = state.offset;
      let moved = (
        (ox + dx).clamp(0.0, state.max.0),
        (oy + dy).clamp(0.0, state.max.1),
      );
      if moved != (ox, oy) {
        state.offset = moved;
        return true;
      }
    }
    false
  }

  /// How far the scrolling element `key` is scrolled, and the largest
  /// offset its content allows, or `None` when it does not scroll (M15).
  pub fn scroll_extent(
    &self,
    key: &BoxKey,
  ) -> Option<((f32, f32), (f32, f32))> {
    self.scroll.get(key).map(|s| (s.offset, s.max))
  }

  /// Where the element `key` is scrolled to, and the size of its box
  /// inside its border and padding: what can be seen (M16).
  pub fn viewport(
    &self,
    key: &BoxKey,
  ) -> Option<dovetail_core::Viewport> {
    let b = &self.boxes[self.index_of(key)?];
    let c = content_box(b);
    let (x, y) = self.scroll_offset(key);
    Some(dovetail_core::Viewport { x, y, width: c.width, height: c.height })
  }

  /// How far the element `key` is scrolled, or (0, 0).
  pub fn scroll_offset(
    &self,
    key: &BoxKey,
  ) -> (f32, f32) {
    self
      .scroll
      .get(key)
      .map_or((0.0, 0.0), |s| s.offset)
  }
}
