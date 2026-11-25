//! Where editable text's caret and selection go, and the place in the text
//! under the pointer (M06).

use dovetail_core::{Element, TextCursor};

use crate::{BoxKey, Content, Layout, LayoutBox, Measure, Rect};

/// The rectangles that show a [`TextCursor`], in window coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CursorGeometry {
  /// One rectangle for each line the selection covers.
  pub selection: Vec<Rect>,
  /// The caret, when it is shown.
  pub caret: Option<Rect>,
  /// Underlines below text an input method is composing, one for each
  /// line it covers.
  pub composing: Vec<Rect>,
}

/// The first text box inside box `index` of `boxes`, or `index` itself
/// when it is text.
pub fn first_text(
  boxes: &[LayoutBox],
  index: usize,
) -> Option<usize> {
  let b = boxes.get(index)?;
  if matches!(b.content, Content::Text { .. }) {
    return Some(index);
  }
  b.children
    .iter()
    .find_map(|&c| first_text(boxes, c))
}

/// Each line of `text` with the byte where it starts.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
  let mut start = 0;
  text.split('\n').map(move |line| {
    let at = start;
    start += line.len() + 1;
    (at, line)
  })
}

/// Where `cursor` is drawn over `text_box`, a text box laid out with the
/// same `measure`.
pub fn cursor_geometry(
  text_box: &LayoutBox,
  cursor: &TextCursor,
  measure: &dyn Measure,
) -> CursorGeometry {
  let Content::Text { text, font_size, .. } = &text_box.content else {
    return CursorGeometry::default();
  };
  let size = *font_size;
  let font = text_box.content.text_style().expect("text");
  let line_height = measure.line_height(font);
  let (start, end) = (
    cursor.caret.min(cursor.anchor),
    cursor.caret.max(cursor.anchor),
  );
  let r = text_box.rect;
  let mut geometry = CursorGeometry::default();
  for (row, (at, line)) in lines(text).enumerate() {
    let y = r.y + row as f32 * line_height;
    let line_end = at + line.len();
    // Right-to-left lines line up on the right of the box.
    let x = r.x + measure.line_start(line, font, r.width);
    if start < end && start <= line_end && end > at {
      let from = start.max(at) - at;
      let to = end.min(line_end) - at;
      for (x0, x1) in measure.selection(line, font, from, to) {
        geometry
          .selection
          .push(Rect::new(x + x0, y, x1 - x0, line_height));
      }
      if end > line_end {
        // The line break is selected too: show a little of it, at the
        // line's end.
        let width = measure.text(line, font).0;
        let at_end = if measure.is_rtl(line, font) {
          -size * 0.3
        } else {
          width
        };
        geometry.selection.push(Rect::new(
          x + at_end,
          y,
          size * 0.3,
          line_height,
        ));
      }
    }
    if let Some((from, to)) = cursor.composing
      && from < to
      && from <= line_end
      && to > at
    {
      // Just above the bottom of the line, under the descenders.
      let thickness = (size / 16.0).max(1.0);
      for (x0, x1) in
        measure.selection(line, font, from.max(at) - at, to.min(line_end) - at)
      {
        geometry.composing.push(Rect::new(
          x + x0,
          y + line_height - thickness * 2.0,
          x1 - x0,
          thickness,
        ));
      }
    }
    if cursor.show_caret && (at..=line_end).contains(&cursor.caret) {
      let caret = measure.caret_x(line, font, cursor.caret - at);
      geometry.caret = Some(Rect::new(x + caret, y, 1.0, line_height));
    }
  }
  geometry
}

impl Layout {
  /// The byte of the first text inside the element `key` nearest to the
  /// window point (`x`, `y`): where a click there puts the caret. `None`
  /// when there is no such element or it holds no text.
  pub fn text_at(
    &self,
    key: &BoxKey,
    x: f32,
    y: f32,
    measure: &dyn Measure,
  ) -> Option<usize> {
    let index = first_text(&self.boxes, self.index_of(key)?)?;
    let b = &self.boxes[index];
    let Content::Text { text, .. } = &b.content else {
      return None;
    };
    let font = b.content.text_style()?;
    let line_height = measure.line_height(font).max(1.0);
    let rows = lines(text).count();
    let row = ((y - b.rect.y) / line_height).floor().max(0.0) as usize;
    let (at, line) = lines(text).nth(row.min(rows - 1))?;
    let start = measure.line_start(line, font, b.rect.width);
    Some(at + measure.byte_at(line, font, x - b.rect.x - start))
  }
  /// Where the caret of the element `key` is, shown or not: where an
  /// input method puts its candidate window. `None` when the element has
  /// no text cursor.
  pub fn caret_rect(
    &self,
    key: &BoxKey,
    measure: &dyn Measure,
  ) -> Option<Rect> {
    let index = self.index_of(key)?;
    let Content::Element(e) = &self.boxes[index].content else {
      return None;
    };
    let cursor = TextCursor { show_caret: true, ..e.text_cursor? };
    let text = &self.boxes[first_text(&self.boxes, index)?];
    cursor_geometry(text, &cursor, measure).caret
  }

  /// The element `key`, if it is laid out.
  pub fn element(
    &self,
    key: &BoxKey,
  ) -> Option<&Element> {
    match &self.boxes[self.index_of(key)?].content {
      Content::Element(e) => Some(e),
      Content::Text { .. } | Content::Picture { .. } => None,
    }
  }

  /// Where box `key` is, in window coordinates.
  pub fn rect_of(
    &self,
    key: &BoxKey,
  ) -> Option<Rect> {
    Some(self.boxes[self.index_of(key)?].rect)
  }

  /// The element with a wheel handler under the point: the topmost box
  /// there, or the nearest element above it that has one (M09).
  pub fn wheel_target_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<BoxKey> {
    let i = self.above(
      self.top_at(x, y)?,
      |b| matches!(&b.content, Content::Element(e) if e.on_wheel.is_some()),
    )?;
    Some(self.boxes[i].key.clone())
  }

  /// The elements with a drop handler under the point, nearest first: the
  /// topmost box there or the nearest element above it that has one, then
  /// each one further out (M10).
  pub fn drop_targets_at(
    &self,
    x: f32,
    y: f32,
  ) -> Vec<BoxKey> {
    let mut out = Vec::new();
    let mut at = self.top_at(x, y);
    while let Some(i) = at {
      if matches!(&self.boxes[i].content, Content::Element(e) if e.on_drop.is_some())
      {
        out.push(self.boxes[i].key.clone());
      }
      at = self.parents[i];
    }
    out
  }

  /// The element with a pointer handler under the point: the topmost box
  /// there, or the nearest element above it that has one.
  pub fn pointer_target_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<BoxKey> {
    let i = self.above(
      self.top_at(x, y)?,
      |b| matches!(&b.content, Content::Element(e) if e.on_pointer.is_some()),
    )?;
    Some(self.boxes[i].key.clone())
  }
  /// The elements with a hover handler under the point, nearest first:
  /// every one from the topmost box there out (M15). A layer counts as
  /// inside the element it belongs to.
  pub fn hover_targets_at(
    &self,
    x: f32,
    y: f32,
  ) -> Vec<BoxKey> {
    let mut found = Vec::new();
    let mut at = self.top_at(x, y);
    while let Some(i) = at {
      if matches!(&self.boxes[i].content, Content::Element(e) if e.on_hover.is_some())
      {
        found.push(self.boxes[i].key.clone());
      }
      at = self.parents[i];
    }
    found
  }

  /// The elements with a focus-within handler that hold the element
  /// `focus`, nearest first, itself included (M15).
  pub fn focus_within_targets(
    &self,
    focus: &BoxKey,
  ) -> Vec<BoxKey> {
    let mut found = Vec::new();
    let mut at = self.index_of(focus);
    while let Some(i) = at {
      if matches!(&self.boxes[i].content, Content::Element(e) if e.on_focus_within.is_some())
      {
        found.push(self.boxes[i].key.clone());
      }
      at = self.parents[i];
    }
    found
  }
  /// The element with a context menu handler at the point: the topmost
  /// box there, or the nearest element above it that has one (M15).
  pub fn context_menu_target_at(
    &self,
    x: f32,
    y: f32,
  ) -> Option<BoxKey> {
    let i = self.above(
      self.top_at(x, y)?,
      |b| matches!(&b.content, Content::Element(e) if e.on_context_menu.is_some()),
    )?;
    Some(self.boxes[i].key.clone())
  }

  /// The element with a context menu handler that holds the element
  /// `focus`, itself included (M15).
  pub fn context_menu_target_for(
    &self,
    focus: &BoxKey,
  ) -> Option<BoxKey> {
    let i = self.above(
      self.index_of(focus)?,
      |b| matches!(&b.content, Content::Element(e) if e.on_context_menu.is_some()),
    )?;
    Some(self.boxes[i].key.clone())
  }
}
