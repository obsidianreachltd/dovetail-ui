//! Editable text: a caret and selection, editing by grapheme cluster, and
//! undo of typing. It knows nothing of keys or windows; `TextField` in
//! `dovetail-controls` turns keys into these calls.

use std::ops::Range;

use crate::unicode::{grapheme_boundaries, next_grapheme, previous_grapheme};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
  text: String,
  caret: usize,
  anchor: usize,
}

/// What the last edit was, so that runs of typing or deleting undo as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
  Typing,
  Deleting,
  Other,
}

/// Text with a caret, a selection, and undo.
///
/// Positions are byte offsets into the text, always on grapheme cluster
/// boundaries. The selection runs between the `anchor`, where it started,
/// and the `caret`, which moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextBuffer {
  text: String,
  caret: usize,
  anchor: usize,
  multiline: bool,
  undo: Vec<Snapshot>,
  redo: Vec<Snapshot>,
  last: Option<EditKind>,
}

impl TextBuffer {
  /// A buffer holding `text`, with the caret at the end. A single-line
  /// buffer turns line breaks it is given into spaces.
  pub fn new(
    text: &str,
    multiline: bool,
  ) -> Self {
    let text = clean(text, multiline);
    let end = text.len();
    Self {
      text,
      caret: end,
      anchor: end,
      multiline,
      undo: Vec::new(),
      redo: Vec::new(),
      last: None,
    }
  }

  /// The text.
  pub fn text(&self) -> &str {
    &self.text
  }

  /// Whether line breaks are allowed.
  pub fn is_multiline(&self) -> bool {
    self.multiline
  }

  /// Where the caret is.
  pub fn caret(&self) -> usize {
    self.caret
  }

  /// Where the selection started. Equal to the caret when nothing is
  /// selected.
  pub fn anchor(&self) -> usize {
    self.anchor
  }

  /// The selected bytes, from the smaller position to the larger.
  pub fn selection(&self) -> Range<usize> {
    self.caret.min(self.anchor)..self.caret.max(self.anchor)
  }

  /// The selected text.
  pub fn selected_text(&self) -> &str {
    &self.text[self.selection()]
  }

  /// Replaces all the text from outside, such as when the value a field
  /// shows is changed by the application. Undo history is cleared.
  pub fn set_text(
    &mut self,
    text: &str,
  ) {
    self.text = clean(text, self.multiline);
    self.caret = snap(&self.text, self.caret.min(self.text.len()));
    self.anchor = snap(&self.text, self.anchor.min(self.text.len()));
    self.undo.clear();
    self.redo.clear();
    self.last = None;
  }

  /// Moves the caret to `at` (snapped to a grapheme boundary). With
  /// `extend`, the selection grows to it; otherwise nothing is selected.
  pub fn set_caret(
    &mut self,
    at: usize,
    extend: bool,
  ) {
    self.caret = snap(&self.text, at.min(self.text.len()));
    if !extend {
      self.anchor = self.caret;
    }
    self.last = None;
  }

  /// Selects all the text.
  pub fn select_all(&mut self) {
    self.anchor = 0;
    self.caret = self.text.len();
    self.last = None;
  }

  /// The word, or the run of spaces or punctuation, at `at`, as a byte
  /// range.
  pub fn word_at(
    &self,
    at: usize,
  ) -> Range<usize> {
    let at = snap(&self.text, at.min(self.text.len()));
    let kind = |p: usize| self.text[p..].chars().next().map(class);
    let target = kind(at).or_else(|| kind(previous_grapheme(&self.text, at)));
    let mut start = at;
    while start > 0 && kind(previous_grapheme(&self.text, start)) == target {
      start = previous_grapheme(&self.text, start);
    }
    let mut end = at;
    while end < self.text.len() && kind(end) == target {
      end = next_grapheme(&self.text, end);
    }
    start..end
  }

  /// Selects the word, or the run of spaces or punctuation, at `at`.
  pub fn select_word_at(
    &mut self,
    at: usize,
  ) {
    let word = self.word_at(at);
    self.anchor = word.start;
    self.caret = word.end;
    self.last = None;
  }

  /// Selects whole words from the word `first` (as [`TextBuffer::word_at`]
  /// gave it) to the word at `at`, as dragging after a double click does.
  pub fn select_words(
    &mut self,
    first: Range<usize>,
    at: usize,
  ) {
    let word = self.word_at(at);
    if word.start >= first.start {
      self.anchor = first.start.min(self.text.len());
      self.caret = word.end;
    } else {
      self.anchor = first.end.min(self.text.len());
      self.caret = word.start;
    }
    self.last = None;
  }

  /// Moves one grapheme (or, with `word`, one word) to the left. Without
  /// `extend`, a selection collapses to its left end instead.
  pub fn move_left(
    &mut self,
    extend: bool,
    word: bool,
  ) {
    let target = if !extend && self.caret != self.anchor && !word {
      self.selection().start
    } else if word {
      self.word_left(self.caret)
    } else {
      previous_grapheme(&self.text, self.caret)
    };
    self.set_caret(target, extend);
  }

  /// Moves one grapheme (or, with `word`, one word) to the right. Without
  /// `extend`, a selection collapses to its right end instead.
  pub fn move_right(
    &mut self,
    extend: bool,
    word: bool,
  ) {
    let target = if !extend && self.caret != self.anchor && !word {
      self.selection().end
    } else if word {
      self.word_right(self.caret)
    } else {
      next_grapheme(&self.text, self.caret)
    };
    self.set_caret(target, extend);
  }

  /// Moves one place left or right on the screen, as the text is shown
  /// with `fonts` at `size` pixels (M13): Left always moves left, whichever
  /// way the text runs. Without `extend`, a selection collapses to its
  /// left or right end on the screen instead.
  pub fn move_visually(
    &mut self,
    extend: bool,
    right: bool,
    fonts: &crate::Fonts,
    size: f32,
  ) {
    let start = self.line_start(self.caret);
    let end = self.line_end(self.caret);
    let line = &self.text[start..end];
    let shaped = fonts.shape(line, size);
    if !extend && self.caret != self.anchor {
      let sel = self.selection();
      let (a, b) = (sel.start.max(start), sel.end.min(end));
      let (xa, xb) = (shaped.caret_x(a - start), shaped.caret_x(b - start));
      let target = if (xa < xb) == right { b } else { a };
      self.set_caret(target, false);
      return;
    }
    match shaped.move_visually(self.caret - start, right) {
      Some(at) => self.set_caret(start + at, extend),
      // At either end of a line, on to the line before or after, in the
      // order the text runs.
      None => {
        if right != shaped.rtl {
          self.move_right(extend, false);
        } else {
          self.move_left(extend, false);
        }
      },
    }
  }

  /// The start of the caret's line, or of the text with `whole`.
  pub fn line_start(
    &self,
    at: usize,
  ) -> usize {
    self.text[..at].rfind('\n').map_or(0, |i| i + 1)
  }

  /// The end of the caret's line, before its line break.
  pub fn line_end(
    &self,
    at: usize,
  ) -> usize {
    self.text[at..]
      .find('\n')
      .map_or(self.text.len(), |i| at + i)
  }

  /// Moves to the start of the line, or of the text with `whole`.
  pub fn home(
    &mut self,
    extend: bool,
    whole: bool,
  ) {
    let target = if whole {
      0
    } else {
      self.line_start(self.caret)
    };
    self.set_caret(target, extend);
  }

  /// Moves to the end of the line, or of the text with `whole`.
  pub fn end(
    &mut self,
    extend: bool,
    whole: bool,
  ) {
    let target = if whole {
      self.text.len()
    } else {
      self.line_end(self.caret)
    };
    self.set_caret(target, extend);
  }

  /// Types `text` over the selection. Runs of typing undo together.
  pub fn insert(
    &mut self,
    text: &str,
  ) {
    let text = clean(text, self.multiline);
    if text.is_empty() && self.caret == self.anchor {
      return;
    }
    let single = text.chars().count() == 1 && text != "\n";
    // Typing over a selection starts a run, so undo brings the selected
    // text back in one step.
    self.record(if single {
      EditKind::Typing
    } else {
      EditKind::Other
    });
    let range = self.selection();
    self.text.replace_range(range.clone(), &text);
    self.caret = range.start + text.len();
    self.anchor = self.caret;
  }

  /// Deletes the selection, or the grapheme (or word) before the caret.
  pub fn backspace(
    &mut self,
    word: bool,
  ) {
    if self.caret == self.anchor {
      if self.caret == 0 {
        return;
      }
      let start = if word {
        self.word_left(self.caret)
      } else {
        previous_grapheme(&self.text, self.caret)
      };
      self.record(EditKind::Deleting);
      self.text.replace_range(start..self.caret, "");
      self.caret = start;
    } else {
      self.record(EditKind::Other);
      let range = self.selection();
      self.text.replace_range(range.clone(), "");
      self.caret = range.start;
    }
    self.anchor = self.caret;
  }

  /// Deletes the selection, or the grapheme (or word) after the caret.
  pub fn delete(
    &mut self,
    word: bool,
  ) {
    if self.caret == self.anchor {
      if self.caret == self.text.len() {
        return;
      }
      let end = if word {
        self.word_right(self.caret)
      } else {
        next_grapheme(&self.text, self.caret)
      };
      self.record(EditKind::Deleting);
      self.text.replace_range(self.caret..end, "");
    } else {
      self.record(EditKind::Other);
      let range = self.selection();
      self.text.replace_range(range.clone(), "");
      self.caret = range.start;
    }
    self.anchor = self.caret;
  }

  /// Undoes the last edit, or run of typing. Returns whether it did.
  pub fn undo(&mut self) -> bool {
    let Some(before) = self.undo.pop() else {
      return false;
    };
    self.redo.push(self.snapshot());
    self.restore(before);
    true
  }

  /// Redoes what [`TextBuffer::undo`] undid. Returns whether it did.
  pub fn redo(&mut self) -> bool {
    let Some(after) = self.redo.pop() else {
      return false;
    };
    self.undo.push(self.snapshot());
    self.restore(after);
    true
  }

  fn snapshot(&self) -> Snapshot {
    Snapshot {
      text: self.text.clone(),
      caret: self.caret,
      anchor: self.anchor,
    }
  }

  fn restore(
    &mut self,
    s: Snapshot,
  ) {
    self.text = s.text;
    self.caret = s.caret;
    self.anchor = s.anchor;
    self.last = None;
  }

  /// Saves the state for undo, unless this edit continues a run of the same
  /// kind.
  fn record(
    &mut self,
    kind: EditKind,
  ) {
    let continues = kind != EditKind::Other && self.last == Some(kind);
    if !continues {
      self.undo.push(self.snapshot());
      if self.undo.len() > 200 {
        self.undo.remove(0);
      }
    }
    self.redo.clear();
    self.last = Some(kind);
  }

  fn word_left(
    &self,
    from: usize,
  ) -> usize {
    let mut at = from;
    // Skip spaces, then a run of one class.
    while at > 0 && self.class_before(at) == Some(Class::Space) {
      at = previous_grapheme(&self.text, at);
    }
    let target = self.class_before(at);
    while at > 0 && self.class_before(at) == target {
      at = previous_grapheme(&self.text, at);
    }
    at
  }

  fn word_right(
    &self,
    from: usize,
  ) -> usize {
    let mut at = from;
    let kind = |p: usize| self.text[p..].chars().next().map(class);
    let target = kind(at);
    while at < self.text.len()
      && kind(at) == target
      && target != Some(Class::Space)
    {
      at = next_grapheme(&self.text, at);
    }
    while at < self.text.len() && kind(at) == Some(Class::Space) {
      at = next_grapheme(&self.text, at);
    }
    at
  }

  fn class_before(
    &self,
    at: usize,
  ) -> Option<Class> {
    self.text[previous_grapheme(&self.text, at)..at]
      .chars()
      .next()
      .map(class)
  }
}

/// Characters that make words, spaces, and everything else, for moving and
/// selecting by word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
  Word,
  Space,
  Other,
}

fn class(c: char) -> Class {
  if c.is_alphanumeric() || c == '_' {
    Class::Word
  } else if c.is_whitespace() {
    Class::Space
  } else {
    Class::Other
  }
}

/// Line breaks become `\n` in multi-line text, and spaces otherwise.
/// Other control characters are dropped.
fn clean(
  text: &str,
  multiline: bool,
) -> String {
  let text = text.replace("\r\n", "\n").replace('\r', "\n");
  text
    .chars()
    .filter_map(|c| match c {
      '\n' if multiline => Some('\n'),
      '\n' => Some(' '),
      '\t' => Some(' '),
      c if c.is_control() => None,
      c => Some(c),
    })
    .collect()
}

/// The grapheme boundary at or before `at`.
fn snap(
  text: &str,
  at: usize,
) -> usize {
  grapheme_boundaries(text)
    .into_iter()
    .rev()
    .find(|&b| b <= at)
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn buffer(text: &str) -> TextBuffer {
    TextBuffer::new(text, false)
  }

  #[test]
  fn typing_and_moving_work_by_grapheme() {
    let mut b = buffer("ae\u{301}b"); // a, e + accent, b
    b.move_left(false, false);
    assert_eq!(b.caret(), 4, "before b");
    b.move_left(false, false);
    assert_eq!(b.caret(), 1, "over the accented e in one step");
    b.backspace(false);
    assert_eq!(b.text(), "e\u{301}b");
    b.insert("x");
    assert_eq!((b.text(), b.caret()), ("xe\u{301}b", 1));
    b.delete(false);
    assert_eq!(b.text(), "xb", "Delete removes the accent with its letter");
  }

  #[test]
  fn selection_extends_collapses_and_is_replaced() {
    let mut b = buffer("hello world");
    b.home(false, false);
    b.move_right(true, true);
    assert_eq!(b.selected_text(), "hello ");
    b.move_right(false, false);
    assert_eq!(
      (b.caret(), b.anchor()),
      (6, 6),
      "Right collapses to the end"
    );
    b.end(true, false);
    assert_eq!(b.selected_text(), "world");
    b.insert("there");
    assert_eq!(b.text(), "hello there");
    b.select_all();
    b.backspace(false);
    assert_eq!(b.text(), "");
  }

  #[test]
  fn word_moves_skip_spaces_and_punctuation_runs() {
    let mut b = buffer("one, two  three");
    b.move_left(false, true);
    assert_eq!(b.caret(), 10, "start of three");
    b.move_left(false, true);
    assert_eq!(b.caret(), 5, "start of two");
    b.move_left(false, true);
    assert_eq!(b.caret(), 3, "the comma");
    b.backspace(true);
    assert_eq!(b.text(), ", two  three", "Ctrl+Backspace deletes the word");
    b.select_word_at(6);
    assert_eq!(b.selected_text(), "  ", "the two spaces at 5 and 6");
    b.select_word_at(7);
    assert_eq!(b.selected_text(), "three");
  }

  #[test]
  fn typing_undoes_as_one_run_and_redoes() {
    let mut b = buffer("");
    for c in "hi there".chars() {
      b.insert(&c.to_string());
    }
    b.backspace(false);
    b.backspace(false);
    assert_eq!(b.text(), "hi the");
    assert!(b.undo());
    assert_eq!(b.text(), "hi there", "the run of deletes undoes together");
    assert!(b.undo());
    assert_eq!(b.text(), "", "the run of typing undoes together");
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "hi there");
    b.insert("!");
    assert!(!b.redo(), "a new edit clears redo");
  }

  #[test]
  fn moving_the_caret_ends_a_typing_run() {
    let mut b = buffer("");
    b.insert("a");
    b.insert("b");
    b.move_left(false, false);
    b.insert("c");
    assert_eq!(b.text(), "acb");
    b.undo();
    assert_eq!(b.text(), "ab");
  }

  #[test]
  fn single_line_text_turns_line_breaks_into_spaces() {
    let mut b = buffer("a\r\nb");
    assert_eq!(b.text(), "a b");
    b.insert("c\nd");
    assert_eq!(b.text(), "a bc d");
    let mut m = TextBuffer::new("one\r\ntwo", true);
    assert_eq!(m.text(), "one\ntwo");
    m.home(false, false);
    assert_eq!(m.caret(), 4, "start of line two");
    m.home(false, true);
    m.end(false, false);
    assert_eq!(m.caret(), 3, "end of line one");
  }

  #[test]
  fn set_text_keeps_the_caret_in_range_and_clears_undo() {
    let mut b = buffer("abcdef");
    b.insert("g");
    b.set_text("ab");
    assert_eq!((b.caret(), b.anchor()), (2, 2));
    assert!(!b.undo());
  }

  #[test]
  fn dragging_after_a_double_click_selects_whole_words() {
    let mut b = buffer("one two three four");
    let first = b.word_at(5);
    assert_eq!(first, 4..7, "two");
    b.select_words(first.clone(), 10);
    assert_eq!((b.anchor(), b.caret()), (4, 13), "two three");
    b.select_words(first.clone(), 1);
    assert_eq!((b.anchor(), b.caret()), (7, 0), "one two, backwards");
    b.select_words(first, 5);
    assert_eq!(b.selected_text(), "two");
  }

  #[test]
  fn typing_over_a_selection_undoes_in_one_step() {
    let mut b = buffer("Ada");
    b.select_all();
    for c in ["G", "r", "a", "c", "e"] {
      b.insert(c);
    }
    assert_eq!(b.text(), "Grace");
    assert!(b.undo());
    assert_eq!(b.text(), "Ada");
    assert_eq!(b.selected_text(), "Ada", "the selection comes back too");
  }
}
