//! M06 "Done when" item 2: a text field takes typing, selection with the
//! keyboard and pointer, and undo. These drive `TextField` through the
//! component tree, layout and key dispatch, with no window or GPU, the way
//! the desktop crate does.

extern crate dovetail_core as dovetail;

use dovetail::{
  ImeEvent, KeyEvent, Modifiers, NamedKey, PointerEvent, PointerKind, Signal,
  TextCursor, Tree, View, use_state,
};
use dovetail_controls::{Column, TextField};
use dovetail_layout::{
  BoxKey, Content, FixedPitch, Layout, Measure, cursor_geometry, first_text,
};
use dovetail_macros::{component, ui};

/// The size of a field's text: the default theme's body text.
const SIZE: f32 = 14.0;

thread_local! {
  static VALUE: std::cell::Cell<Option<Signal<String>>> =
    const { std::cell::Cell::new(None) };
}

#[component]
fn Form(
  #[prop(into)] start: String,
  #[prop(default)] multiline: bool,
  #[prop(default)] rows: Option<usize>,
) -> View {
  let value = use_state(|| start.clone());
  VALUE.set(Some(value));
  ui! {
    Column {
      TextField { value: value.get(), on_change: move |v| value.set(v), multiline: multiline, rows: rows }
    }
  }
}

struct Window {
  tree: Tree,
  layout: Layout,
  focus: Option<BoxKey>,
  measure: FixedPitch,
}

impl Window {
  fn open(
    start: &str,
    multiline: bool,
  ) -> Self {
    Self::open_rows(start, multiline, None)
  }

  fn open_rows(
    start: &str,
    multiline: bool,
    rows: Option<usize>,
  ) -> Self {
    let mut tree = Tree::new();
    tree.mount(ui! { Form { start: start, multiline: multiline, rows: rows } });
    let mut window = Self {
      tree,
      layout: Layout::new(),
      focus: None,
      measure: FixedPitch::default(),
    };
    window.refresh();
    window
  }

  fn refresh(&mut self) {
    // Building the field can change its own state once, when the value
    // from its owner changes.
    for _ in 0..3 {
      self.tree.update();
    }
    self
      .layout
      .update(&self.tree, 400.0, 300.0, &self.measure);
  }

  /// The owner's copy of the text, which `on_change` keeps up to date.
  fn value(&self) -> String {
    VALUE.get().expect("form built").get()
  }

  fn field(&self) -> usize {
    self
      .layout
      .boxes()
      .iter()
      .position(|b| matches!(&b.content, Content::Element(e) if e.text_cursor.is_some()))
      .expect("a text field")
  }

  fn cursor(&self) -> TextCursor {
    match &self.layout.boxes()[self.field()].content {
      Content::Element(e) => e.text_cursor.expect("cursor"),
      _ => unreachable!(),
    }
  }

  fn shown(&self) -> String {
    let t = first_text(self.layout.boxes(), self.field()).expect("text");
    match &self.layout.boxes()[t].content {
      Content::Text { text, .. } => text.to_string(),
      _ => unreachable!(),
    }
  }

  /// Focuses the field as Tab would, telling it so.
  fn focus(&mut self) {
    let key = self
      .layout
      .focus_next(None, false)
      .expect("focusable");
    if let Some(on_focus) = self
      .layout
      .element(&key)
      .and_then(|e| e.on_focus.clone())
    {
      on_focus.call(true);
    }
    self.focus = Some(key);
    self.refresh();
  }

  fn key(
    &mut self,
    event: KeyEvent,
  ) -> bool {
    let used = self
      .layout
      .dispatch_key(self.focus.as_ref(), &event);
    self.refresh();
    used
  }

  fn typed(
    &mut self,
    text: &str,
  ) {
    for c in text.chars() {
      assert!(self.key(KeyEvent::character(c)), "typing {c:?}");
    }
  }

  fn ctrl(
    &mut self,
    c: char,
    shift: bool,
  ) -> bool {
    let modifiers = Modifiers {
      shift,
      control: !cfg!(target_os = "macos"),
      meta: cfg!(target_os = "macos"),
      ..Modifiers::default()
    };
    let mut event = KeyEvent::character(c).with(modifiers);
    event.text = None;
    self.key(event)
  }

  /// Sends input method text to the focused field, as the desktop crate
  /// does.
  fn ime(
    &mut self,
    event: ImeEvent,
  ) {
    let key = self.focus.clone().expect("focus");
    let on_ime = self
      .layout
      .element(&key)
      .and_then(|e| e.on_ime.clone())
      .expect("an IME handler");
    on_ime.call(event);
    self.refresh();
  }

  fn preedit(
    &mut self,
    text: &str,
    cursor: Option<(usize, usize)>,
  ) {
    self.ime(ImeEvent::Preedit { text: text.to_owned(), cursor });
  }

  /// Sends the pointer to the field at window point (`x`, `y`), as the
  /// desktop crate does.
  fn pointer(
    &mut self,
    kind: PointerKind,
    x: f32,
    y: f32,
    clicks: u32,
    shift: bool,
  ) {
    let key = self.layout.boxes()[self.field()].key.clone();
    let rect = self.layout.rect_of(&key).expect("rect");
    let handler = self
      .layout
      .element(&key)
      .and_then(|e| e.on_pointer.clone())
      .expect("pointer handler");
    handler.handle(&PointerEvent {
      kind,
      button: dovetail::PointerButton::Primary,
      x: x - rect.x,
      y: y - rect.y,
      window_x: x,
      window_y: y,
      text_at: self.layout.text_at(&key, x, y, &self.measure),
      modifiers: Modifiers { shift, ..Modifiers::default() },
      clicks,
    });
    self.refresh();
  }

  /// The window position of the caret place before byte `at` of the
  /// field's first line, in the middle of the line.
  fn at(
    &self,
    at: usize,
  ) -> (f32, f32) {
    let t = first_text(self.layout.boxes(), self.field()).expect("text");
    let b = &self.layout.boxes()[t];
    let text = self.shown();
    let x = self.measure.caret_x(&text, SIZE.into(), at);
    (b.rect.x + x, b.rect.y + 8.0)
  }
}

#[test]
fn typing_edits_the_text_and_tells_the_owner() {
  let mut w = Window::open("", false);
  w.focus();
  w.typed("Hello");
  assert_eq!(w.shown(), "Hello");
  assert_eq!(w.value(), "Hello");
  assert_eq!(
    w.cursor(),
    TextCursor {
      caret: 5,
      anchor: 5,
      show_caret: true,
      composing: None
    }
  );

  assert!(w.key(KeyEvent::named(NamedKey::Backspace)));
  assert_eq!(w.value(), "Hell");
  w.key(KeyEvent::named(NamedKey::Home));
  w.typed(">");
  assert_eq!(w.value(), ">Hell");
  w.key(KeyEvent::named(NamedKey::Delete));
  assert_eq!(w.value(), ">ell");
}

#[test]
fn keyboard_selection_replaces_and_undo_restores() {
  let mut w = Window::open("one two", false);
  w.focus();
  let shift = Modifiers { shift: true, ..Modifiers::default() };
  for _ in 0..3 {
    w.key(KeyEvent::named(NamedKey::ArrowLeft).with(shift));
  }
  assert_eq!(w.cursor().anchor, 7);
  assert_eq!(w.cursor().caret, 4);
  w.typed("2");
  assert_eq!(w.value(), "one 2");

  assert!(w.ctrl('z', false));
  assert_eq!(w.value(), "one two");
  assert!(w.ctrl('z', true));
  assert_eq!(w.value(), "one 2");

  assert!(w.ctrl('a', false));
  w.typed("x");
  assert_eq!(w.value(), "x");
}

#[test]
fn single_line_fields_leave_enter_to_their_parents() {
  let mut w = Window::open("a", false);
  w.focus();
  assert!(!w.key(KeyEvent::named(NamedKey::Enter)));
  assert_eq!(w.value(), "a");
}

#[test]
fn multi_line_fields_take_enter_and_move_between_lines() {
  let mut w = Window::open("", true);
  w.focus();
  w.typed("abc");
  assert!(w.key(KeyEvent::named(NamedKey::Enter)));
  w.typed("de");
  assert_eq!(w.value(), "abc\nde");
  // The field grows to two lines.
  let t = first_text(w.layout.boxes(), w.field()).unwrap();
  assert_eq!(w.layout.boxes()[t].rect.height, 2.0 * 1.25 * SIZE);

  w.key(KeyEvent::named(NamedKey::ArrowUp));
  assert_eq!(w.cursor().caret, 2, "same column, line above");
  w.key(KeyEvent::named(NamedKey::ArrowDown));
  assert_eq!(w.cursor().caret, 6);
}

#[test]
fn the_pointer_places_the_caret_and_selects() {
  let mut w = Window::open("hello world", false);
  w.focus();
  let (x, y) = w.at(2);
  w.pointer(PointerKind::Down, x, y, 1, false);
  w.pointer(PointerKind::Up, x, y, 1, false);
  assert_eq!(w.cursor().caret, 2);
  assert_eq!(w.cursor().anchor, 2);

  // Drag to select.
  let (x2, _) = w.at(8);
  w.pointer(PointerKind::Down, x, y, 1, false);
  w.pointer(PointerKind::Move, x2, y, 1, false);
  w.pointer(PointerKind::Up, x2, y, 1, false);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (2, 8));

  // Shift+click grows the selection.
  let (x3, _) = w.at(10);
  w.pointer(PointerKind::Down, x3, y, 1, true);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (2, 10));

  // Double click selects a word.
  let (x4, _) = w.at(7);
  w.pointer(PointerKind::Down, x4, y, 2, false);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (6, 11));
}

#[test]
fn a_new_value_from_the_owner_replaces_the_text() {
  let mut w = Window::open("first", false);
  VALUE.get().unwrap().set("second".to_owned());
  w.refresh();
  assert_eq!(w.shown(), "second");
}

#[test]
fn the_caret_shows_only_with_focus_and_the_selection_has_a_rectangle() {
  let mut w = Window::open("hello", false);
  let field = w.field();
  let t = first_text(w.layout.boxes(), field).unwrap();
  let text_box = &w.layout.boxes()[t];
  let g = cursor_geometry(text_box, &w.cursor(), &w.measure);
  assert!(g.caret.is_none(), "no caret without focus");
  assert!(g.selection.is_empty());

  w.focus();
  w.key(
    KeyEvent::named(NamedKey::ArrowLeft)
      .with(Modifiers { shift: true, ..Modifiers::default() }),
  );
  let field = w.field();
  let t = first_text(w.layout.boxes(), field).unwrap();
  let text_box = &w.layout.boxes()[t];
  let g = cursor_geometry(text_box, &w.cursor(), &w.measure);
  let caret = g.caret.expect("caret with focus");
  let advance = 0.6 * SIZE;
  assert!((caret.x - (text_box.rect.x + 4.0 * advance)).abs() < 1e-3);
  assert_eq!(g.selection.len(), 1);
  assert!((g.selection[0].width - advance).abs() < 1e-3);
}

#[test]
fn copy_cut_and_paste_use_the_clipboard() {
  // No system clipboard is installed in tests, so this is the clipboard
  // inside the application.
  dovetail::set_clipboard(None);
  let mut w = Window::open("one two", false);
  w.focus();
  let shift = Modifiers { shift: true, ..Modifiers::default() };
  for _ in 0..3 {
    w.key(KeyEvent::named(NamedKey::ArrowLeft).with(shift));
  }
  assert!(w.ctrl('c', false));
  assert_eq!(dovetail::clipboard_text().as_deref(), Some("two"));
  assert_eq!(w.value(), "one two", "copy leaves the text");

  assert!(w.ctrl('x', false));
  assert_eq!(w.value(), "one ");
  w.key(KeyEvent::named(NamedKey::Home));
  assert!(w.ctrl('v', false));
  assert_eq!(w.value(), "twoone ");
  assert_eq!(w.cursor().caret, 3, "the caret ends after the pasted text");

  // Undo takes back the paste, then the cut.
  assert!(w.ctrl('z', false));
  assert_eq!(w.value(), "one ");
  assert!(w.ctrl('z', false));
  assert_eq!(w.value(), "one two");
}

#[test]
fn pasted_line_breaks_become_spaces_on_one_line() {
  dovetail::set_clipboard(None);
  dovetail::set_clipboard_text("a\r\nb");
  let mut w = Window::open("", false);
  w.focus();
  w.ctrl('v', false);
  assert_eq!(w.value(), "a b");

  let mut w = Window::open("", true);
  w.focus();
  w.ctrl('v', false);
  assert_eq!(w.value(), "a\nb");
}

#[test]
fn composed_text_shows_underlined_until_it_is_committed() {
  let mut w = Window::open("ab", false);
  w.focus();
  w.key(KeyEvent::named(NamedKey::ArrowLeft));
  // "にほん" being composed, the input method's caret at its end.
  let kana = "にほん";
  w.preedit(kana, Some((kana.len(), kana.len())));
  assert_eq!(w.shown(), "aにほんb");
  assert_eq!(w.value(), "ab", "composing does not change the value");
  let c = w.cursor();
  assert_eq!(c.composing, Some((1, 1 + kana.len())));
  assert_eq!(c.caret, 1 + kana.len());

  let t = first_text(w.layout.boxes(), w.field()).unwrap();
  let g = cursor_geometry(&w.layout.boxes()[t], &c, &w.measure);
  assert_eq!(g.composing.len(), 1, "one underline");
  assert!((g.composing[0].width - 3.0 * 0.6 * SIZE).abs() < 1e-3);

  // The input method converts, then commits.
  w.preedit("日本", None);
  assert_eq!(w.shown(), "a日本b");
  assert!(!w.cursor().show_caret, "the input method hid its caret");
  w.preedit("", None);
  w.ime(ImeEvent::Commit("日本".into()));
  assert_eq!(w.value(), "a日本b");
  assert_eq!(w.cursor().composing, None);
  assert_eq!(w.cursor().caret, 1 + "日本".len());

  assert!(w.ctrl('z', false));
  assert_eq!(w.value(), "ab", "the commit undoes in one step");
}

#[test]
fn composing_replaces_the_selection_and_can_be_cancelled() {
  let mut w = Window::open("hello", false);
  w.focus();
  assert!(w.ctrl('a', false));
  w.preedit("x", Some((1, 1)));
  assert_eq!(w.value(), "", "the selection goes when composing starts");
  assert_eq!(w.shown(), "x");
  // Cancelling leaves nothing.
  w.preedit("", None);
  assert_eq!(w.shown(), "");
  assert_eq!(w.value(), "");
  assert!(w.ctrl('z', false));
  assert_eq!(w.value(), "hello");
}

#[test]
fn losing_focus_ends_composing() {
  let mut w = Window::open("", false);
  w.focus();
  w.preedit("ka", Some((2, 2)));
  assert_eq!(w.shown(), "ka");
  let key = w.focus.clone().unwrap();
  let on_focus = w
    .layout
    .element(&key)
    .and_then(|e| e.on_focus.clone())
    .unwrap();
  on_focus.call(false);
  w.refresh();
  assert_eq!(w.shown(), "");
  assert_eq!(w.value(), "");
}

#[test]
fn up_and_down_keep_the_column_across_a_short_line() {
  let mut w = Window::open("abcdef\nab\nabcdef", true);
  w.focus();
  // To the end of the first line, then one back: before the "f".
  w.key(KeyEvent::named(NamedKey::Home).with(Modifiers {
    control: !cfg!(target_os = "macos"),
    meta: cfg!(target_os = "macos"),
    ..Modifiers::default()
  }));
  w.key(KeyEvent::named(NamedKey::End));
  w.key(KeyEvent::named(NamedKey::ArrowLeft));
  assert_eq!(w.cursor().caret, 5);
  w.key(KeyEvent::named(NamedKey::ArrowDown));
  assert_eq!(w.cursor().caret, 9, "the end of the short line");
  w.key(KeyEvent::named(NamedKey::ArrowDown));
  assert_eq!(w.cursor().caret, 15, "back to the same column");
  // Any other key forgets the column.
  w.key(KeyEvent::named(NamedKey::ArrowLeft));
  w.key(KeyEvent::named(NamedKey::ArrowUp));
  assert_eq!(w.cursor().caret, 9);
}

#[test]
fn dragging_after_a_double_click_selects_by_word() {
  let mut w = Window::open("one two three", false);
  w.focus();
  let (x, y) = w.at(5);
  w.pointer(PointerKind::Down, x, y, 2, false);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (4, 7));
  let (x2, _) = w.at(10);
  w.pointer(PointerKind::Move, x2, y, 2, false);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (4, 13));
  let (x3, _) = w.at(1);
  w.pointer(PointerKind::Move, x3, y, 2, false);
  assert_eq!((w.cursor().anchor, w.cursor().caret), (7, 0));
  w.pointer(PointerKind::Up, x3, y, 2, false);
}

/// The field's box and its text box.
fn field_and_text(
  w: &Window
) -> (dovetail_layout::Rect, dovetail_layout::Rect) {
  let f = w.field();
  let t = first_text(w.layout.boxes(), f).unwrap();
  (w.layout.boxes()[f].rect, w.layout.boxes()[t].rect)
}

#[test]
fn long_text_scrolls_to_keep_the_caret_in_view() {
  let long = "a".repeat(60);
  let mut w = Window::open(&long, false);
  let (field, text) = field_and_text(&w);
  assert_eq!(text.x, field.x + 9.0, "unfocused, the start shows");
  assert!(text.width > field.width, "the text is wider than the field");

  // Focused, with the caret at the end, the end shows.
  w.focus();
  let (field, text) = field_and_text(&w);
  let caret = text.x + 60.0 * 0.6 * SIZE;
  assert!(text.x < field.x, "the text moved left");
  assert!(
    caret >= field.x + 9.0 && caret + 1.0 <= field.x + field.width - 9.0 + 0.01,
    "the caret at {caret} is inside the field {field:?}"
  );

  // Home goes back to the start.
  w.key(KeyEvent::named(NamedKey::Home));
  let (field, text) = field_and_text(&w);
  assert_eq!(text.x, field.x + 9.0);
}

#[test]
fn rows_fix_the_height_and_the_rest_scrolls() {
  let lines = "1\n2\n3\n4\n5\n6";
  let mut w = Window::open_rows(lines, true, Some(2));
  let (field, text) = field_and_text(&w);
  let line = 1.25 * SIZE;
  assert!(text.height > field.height, "six lines in a two-line field");
  w.focus();
  let (field, text) = field_and_text(&w);
  // The caret is on the last line, which shows at the bottom.
  let last_line_bottom = text.y + 6.0 * line;
  assert!(last_line_bottom <= field.y + field.height - 6.0 + 0.01);
  assert!(last_line_bottom > field.y + field.height - 6.0 - line);
  // The wheel scrolls back up to the first line.
  assert!(
    w.layout
      .scroll_by(field.x + 5.0, field.y + 5.0, 0.0, -1000.0)
  );
  w.refresh();
  let (field, text) = field_and_text(&w);
  assert_eq!(text.y, field.y + 6.0);
}

#[test]
fn keys_that_arrive_between_frames_go_in_once_each() {
  // Fast typing: several keys reach the field before the form and the
  // field are built again (B0004). Each must go in once.
  let mut w = Window::open("Grace ", false);
  w.focus();
  for c in "Hopper".chars() {
    let used = w
      .layout
      .dispatch_key(w.focus.as_ref(), &KeyEvent::character(c));
    assert!(used, "{c}");
  }
  w.refresh();
  w.refresh();
  assert_eq!(w.value(), "Grace Hopper");
  assert_eq!(w.shown(), "Grace Hopper");

  // And keys between builds that happen part way, as when a frame is
  // drawn in the middle of the typing.
  for (i, c) in " Mississippi".chars().enumerate() {
    w.layout
      .dispatch_key(w.focus.as_ref(), &KeyEvent::character(c));
    if i % 3 == 0 {
      w.tree.update();
    }
    if i % 4 == 0 {
      w.refresh();
    }
  }
  w.refresh();
  assert_eq!(w.value(), "Grace Hopper Mississippi");
  assert_eq!(w.shown(), "Grace Hopper Mississippi");
}
