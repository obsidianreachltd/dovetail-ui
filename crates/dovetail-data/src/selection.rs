//! Which rows are selected, and which one the keyboard is on (M16).
//!
//! Rows are found by key, so a selection survives changes to the data.
//! Clicks and keys follow each system's habits: the command modifier
//! (Ctrl, or Command on macOS) adds or removes one row, and Shift selects
//! from the row clicked first to this one.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use dovetail::{Key, KeyCode, KeyEvent, Modifiers, NamedKey, Signal};

/// How many rows can be selected at once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
  /// One at most.
  Single,
  /// Any number. The default.
  #[default]
  Multiple,
}

/// Letters typed within this time of each other make one search.
const TYPE_AHEAD: Duration = Duration::from_millis(1000);

/// The data behind a [`Selection`].
#[derive(Clone, Debug, Default)]
struct State {
  mode: SelectionMode,
  chosen: HashSet<Key>,
  /// Where Shift ranges start: the last row clicked or moved to without
  /// Shift, with its index when last seen.
  anchor: Option<(usize, Key)>,
  /// The row the keyboard is on, with its index when last seen.
  current: Option<(usize, Key)>,
  /// Letters typed so far, and when the last one was.
  typed: String,
  typed_at: Option<Instant>,
}

/// The index of the row `key`, starting at the index it last had: the
/// data may have changed since.
fn find(
  hint: usize,
  key: &Key,
  count: usize,
  key_of: &dyn Fn(usize) -> Key,
) -> Option<usize> {
  if hint < count && key_of(hint) == *key {
    return Some(hint);
  }
  (0..count).find(|&i| key_of(i) == *key)
}

/// The rows selected in a list, tree or table, and the row the keyboard
/// is on. It is `Copy`: make it while a component builds, with
/// `use_state(|| Selection::new(SelectionMode::Multiple)).get()`, and give
/// it to the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection(Signal<State>);

impl Selection {
  /// Nothing selected yet.
  pub fn new(mode: SelectionMode) -> Selection {
    Selection(dovetail::create_signal(State { mode, ..State::default() }))
  }

  /// How many rows can be selected.
  pub fn mode(self) -> SelectionMode {
    self.0.map(|s| s.mode)
  }

  /// True when the row `key` is selected.
  pub fn is_selected(
    self,
    key: &Key,
  ) -> bool {
    self.0.map(|s| s.chosen.contains(key))
  }

  /// How many rows are selected.
  pub fn len(self) -> usize {
    self.0.map(|s| s.chosen.len())
  }

  /// True when nothing is selected.
  pub fn is_empty(self) -> bool {
    self.len() == 0
  }

  /// The keys of the selected rows, in no order.
  pub fn keys(self) -> Vec<Key> {
    self.0.map(|s| s.chosen.iter().cloned().collect())
  }

  /// The row the keyboard is on.
  pub fn current(self) -> Option<Key> {
    self
      .0
      .map(|s| s.current.as_ref().map(|c| c.1.clone()))
  }

  /// The index of the row the keyboard is on, if it is still among the
  /// `count` rows.
  pub fn current_index(
    self,
    count: usize,
    key_of: &dyn Fn(usize) -> Key,
  ) -> Option<usize> {
    let (hint, key) = self.0.map(|s| s.current.clone())?;
    find(hint, &key, count, key_of)
  }

  /// Selects only the row `key`, at `index`, and puts the keyboard on it.
  pub fn select_only(
    self,
    index: usize,
    key: Key,
  ) {
    self.0.update(|s| {
      s.chosen.clear();
      s.chosen.insert(key.clone());
      s.anchor = Some((index, key.clone()));
      s.current = Some((index, key));
    });
  }

  /// Selects exactly these rows, leaving the keyboard where it is.
  pub fn set(
    self,
    keys: impl IntoIterator<Item = Key>,
  ) {
    self.0.update(|s| {
      s.chosen = keys.into_iter().collect();
      if s.mode == SelectionMode::Single && s.chosen.len() > 1 {
        let one = s.chosen.iter().next().cloned();
        s.chosen = one.into_iter().collect();
      }
    });
  }

  /// Selects nothing.
  pub fn clear(self) {
    self.0.update(|s| s.chosen.clear());
  }

  /// Puts the keyboard on row `index` without changing what is selected.
  pub fn set_current(
    self,
    index: usize,
    key: Key,
  ) {
    self.0.update(|s| s.current = Some((index, key)));
  }

  /// A click on row `index` with `modifiers` held.
  pub fn click(
    self,
    index: usize,
    modifiers: Modifiers,
    count: usize,
    key_of: &dyn Fn(usize) -> Key,
  ) {
    let key = key_of(index);
    let many = self.mode() == SelectionMode::Multiple;
    if many && modifiers.shift {
      let add = modifiers.command();
      self.extend_to(index, add, count, key_of);
    } else if many && modifiers.command() {
      self.0.update(|s| {
        if !s.chosen.remove(&key) {
          s.chosen.insert(key.clone());
        }
        s.anchor = Some((index, key.clone()));
        s.current = Some((index, key));
      });
    } else {
      self.select_only(index, key);
    }
  }

  /// Selects from the anchor to row `index`, adding to what is selected
  /// when `add` is true, and puts the keyboard on `index`.
  fn extend_to(
    self,
    index: usize,
    add: bool,
    count: usize,
    key_of: &dyn Fn(usize) -> Key,
  ) {
    let anchor = self
      .0
      .map(|s| s.anchor.clone())
      .and_then(|(hint, key)| find(hint, &key, count, key_of))
      .unwrap_or(index);
    let (from, to) = (anchor.min(index), anchor.max(index));
    self.0.update(|s| {
      if !add {
        s.chosen.clear();
      }
      s.chosen.extend((from..=to).map(key_of));
      if s.anchor.is_none() {
        s.anchor = Some((anchor, key_of(anchor)));
      }
      s.current = Some((index, key_of(index)));
    });
  }

  /// Selects every row, when many can be.
  pub fn select_all(
    self,
    count: usize,
    key_of: &dyn Fn(usize) -> Key,
  ) {
    if self.mode() == SelectionMode::Multiple {
      self
        .0
        .update(|s| s.chosen = (0..count).map(key_of).collect());
    }
  }

  /// A key pressed in a view of `count` rows, `page` of which fit in it.
  /// `text_of` gives a row's text, for typing its first letters. Returns
  /// the row the keyboard moved to, or `Some(current)` when the key was
  /// used without moving; `None` when the view does not use the key.
  pub fn key(
    self,
    event: &KeyEvent,
    count: usize,
    page: usize,
    key_of: &dyn Fn(usize) -> Key,
    text_of: Option<&dyn Fn(usize) -> String>,
  ) -> Option<usize> {
    if count == 0 {
      return None;
    }
    let m = event.modifiers;
    let current = self.current_index(count, key_of);
    let at = current.unwrap_or(0);
    let last = count - 1;
    let page = page.max(1);
    let mac = cfg!(target_os = "macos");
    let to = match &event.key {
      KeyCode::Named(NamedKey::ArrowDown) if mac && m.meta => last,
      KeyCode::Named(NamedKey::ArrowUp) if mac && m.meta => 0,
      KeyCode::Named(NamedKey::ArrowDown) => {
        current.map_or(0, |c| (c + 1).min(last))
      },
      KeyCode::Named(NamedKey::ArrowUp) => {
        current.map_or(0, |c| c.saturating_sub(1))
      },
      KeyCode::Named(NamedKey::PageDown) => (at + page).min(last),
      KeyCode::Named(NamedKey::PageUp) => at.saturating_sub(page),
      KeyCode::Named(NamedKey::Home) => 0,
      KeyCode::Named(NamedKey::End) => last,
      KeyCode::Named(NamedKey::Space) => {
        let key = key_of(at);
        if m.command() && self.mode() == SelectionMode::Multiple {
          self.0.update(|s| {
            if !s.chosen.remove(&key) {
              s.chosen.insert(key.clone());
            }
            s.anchor = Some((at, key.clone()));
            s.current = Some((at, key));
          });
        } else if !self.is_selected(&key) || self.len() != 1 {
          self.select_only(at, key);
        }
        return Some(at);
      },
      KeyCode::Character(c) if m.command() && c.eq_ignore_ascii_case("a") => {
        self.select_all(count, key_of);
        return Some(at);
      },
      KeyCode::Character(c) if !m.command() && !m.alt => {
        let text_of = text_of?;
        let now = Instant::now();
        let typed = self.0.map(|s| {
          let fresh = s
            .typed_at
            .is_none_or(|t| now.duration_since(t) > TYPE_AHEAD);
          if fresh {
            c.clone()
          } else {
            format!("{}{c}", s.typed)
          }
        });
        self.0.update(|s| {
          s.typed.clone_from(&typed);
          s.typed_at = Some(now);
        });
        let found = type_ahead(&typed, current, count, text_of);
        match found {
          Some(i) => i,
          None => return Some(at),
        }
      },
      _ => return None,
    };
    let key = key_of(to);
    let many = self.mode() == SelectionMode::Multiple;
    if many && m.shift {
      self.extend_to(to, false, count, key_of);
    } else if many && m.command() && !mac {
      // Ctrl and an arrow move the keyboard, not the selection, as on
      // Windows and Linux.
      self.set_current(to, key);
    } else {
      self.select_only(to, key);
    }
    Some(to)
  }
}

/// The row whose text starts with `typed`, ignoring case: from the row
/// after `current` for a single letter, so typing it again moves on, and
/// from `current` itself for more.
fn type_ahead(
  typed: &str,
  current: Option<usize>,
  count: usize,
  text_of: &dyn Fn(usize) -> String,
) -> Option<usize> {
  let typed = typed.to_lowercase();
  // The same letter over and over steps through rows that start with it.
  let mut letters = typed.chars();
  let first = letters.next()?;
  let repeated = letters.all(|c| c == first);
  let (needle, skip) = if repeated {
    (first.to_string(), 1)
  } else {
    (typed.clone(), 0)
  };
  let start = current.map_or(0, |c| c + skip);
  (0..count)
    .map(|k| (start + k) % count)
    .find(|&i| text_of(i).to_lowercase().starts_with(&needle))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn key_of(i: usize) -> Key {
    Key::from(i)
  }

  fn with<R>(f: impl FnOnce(Selection) -> R) -> R {
    let (_tree, s) = crate::owned(|| Selection::new(SelectionMode::Multiple));
    f(s)
  }

  fn command() -> Modifiers {
    if cfg!(target_os = "macos") {
      Modifiers { meta: true, ..Modifiers::default() }
    } else {
      Modifiers { control: true, ..Modifiers::default() }
    }
  }

  fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::default() }
  }

  fn chosen(s: Selection) -> Vec<usize> {
    let mut v: Vec<usize> = (0..20)
      .filter(|&i| s.is_selected(&key_of(i)))
      .collect();
    v.sort_unstable();
    v
  }

  #[test]
  fn clicks_with_each_modifier() {
    with(|s| {
      s.click(3, Modifiers::default(), 20, &key_of);
      assert_eq!(chosen(s), [3]);
      s.click(6, command(), 20, &key_of);
      assert_eq!(chosen(s), [3, 6], "the command key adds one");
      s.click(3, command(), 20, &key_of);
      assert_eq!(chosen(s), [6], "and takes one away");
      s.click(9, shift(), 20, &key_of);
      assert_eq!(
        chosen(s),
        [3, 4, 5, 6, 7, 8, 9],
        "Shift selects from the anchor, the row clicked last"
      );
      s.click(4, shift(), 20, &key_of);
      assert_eq!(chosen(s), [3, 4], "a new range replaces the old");
      let both = Modifiers { shift: true, ..command() };
      s.click(12, command(), 20, &key_of);
      s.click(14, both, 20, &key_of);
      assert_eq!(chosen(s), [3, 4, 12, 13, 14], "and adds with both");
      assert_eq!(s.current(), Some(key_of(14)));
    });
  }

  #[test]
  fn one_at_most_in_single_mode() {
    let (_tree, s) = crate::owned(|| Selection::new(SelectionMode::Single));
    s.click(2, Modifiers::default(), 20, &key_of);
    s.click(5, command(), 20, &key_of);
    s.click(8, shift(), 20, &key_of);
    assert_eq!(chosen(s), [8]);
    s.select_all(20, &key_of);
    assert_eq!(chosen(s), [8]);
  }

  fn press(
    s: Selection,
    key: KeyEvent,
  ) -> Option<usize> {
    let text = |i: usize| ["apple", "banana", "blue", "cherry"][i % 4].into();
    s.key(&key, 20, 5, &key_of, Some(&text))
  }

  #[test]
  fn keys_move_and_select() {
    with(|s| {
      assert_eq!(press(s, KeyEvent::named(NamedKey::ArrowDown)), Some(0));
      assert_eq!(press(s, KeyEvent::named(NamedKey::ArrowDown)), Some(1));
      assert_eq!(press(s, KeyEvent::named(NamedKey::PageDown)), Some(6));
      assert_eq!(chosen(s), [6]);
      let mut up = KeyEvent::named(NamedKey::ArrowUp);
      up.modifiers = shift();
      press(s, up.clone());
      press(s, up);
      assert_eq!(chosen(s), [4, 5, 6], "Shift and arrows make a range");
      assert_eq!(press(s, KeyEvent::named(NamedKey::End)), Some(19));
      assert_eq!(press(s, KeyEvent::named(NamedKey::Home)), Some(0));
      let mut all = KeyEvent::character('a');
      all.modifiers = command();
      press(s, all);
      assert_eq!(s.len(), 20, "Ctrl+A selects every row");
      assert_eq!(press(s, KeyEvent::named(NamedKey::Escape)), None);
    });
  }

  #[test]
  fn typing_finds_rows_by_their_first_letters() {
    with(|s| {
      assert_eq!(press(s, KeyEvent::character('b')), Some(1));
      assert_eq!(press(s, KeyEvent::character('l')), Some(2), "\"bl\"");
      s.0.update(|st| st.typed_at = None);
      assert_eq!(press(s, KeyEvent::character('b')), Some(5), "moves on");
      assert_eq!(press(s, KeyEvent::character('b')), Some(6), "again");
      assert_eq!(chosen(s), [6]);
    });
  }

  #[test]
  fn a_selection_follows_its_rows_when_the_data_changes() {
    with(|s| {
      s.click(2, Modifiers::default(), 20, &key_of);
      // The rows are reversed: row 2 is now at index 17.
      let reversed = |i: usize| key_of(19 - i);
      assert_eq!(s.current_index(20, &reversed), Some(17));
      assert!(s.is_selected(&key_of(2)));
    });
  }
}
