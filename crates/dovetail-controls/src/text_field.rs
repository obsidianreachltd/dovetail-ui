//! A box to type text into (M06).

use dovetail::{
  AccessAction, Border, Callback, Edges, Element, ElementKind, ImeEvent,
  KeyCode, KeyEvent, KeyHandler, NamedKey, PointerEvent, PointerHandler,
  PointerKind, Role, Signal, Size, Style, TextCursor, View, clipboard_text,
  set_clipboard_text, use_state,
};
use std::ops::Range;

use dovetail_macros::component;
use dovetail_text::{Fonts, TextBuffer};

use dovetail_theme::theme;

/// A box to type text into, on one line or (with `multiline`) several.
///
/// It shows `value` and calls `on_change` with the new text after every
/// edit. The component that owns the text passes it back as `value`; when
/// `value` changes to something else, the field shows that instead.
///
/// It takes focus with a click or Tab. It has a caret, selection with the
/// pointer (drag, Shift+click, double-click for a word) and the keyboard
/// (Shift with the arrows, Home and End), undo of typing (Ctrl+Z, and
/// Ctrl+Y or Ctrl+Shift+Z to redo), and copy, cut and paste with the
/// system clipboard (Ctrl+C, Ctrl+X and Ctrl+V). On macOS, Command takes
/// the place of Ctrl.
///
/// Input methods (IME) work while it has focus: text being composed shows
/// underlined at the caret, and replaces the selection when it is
/// committed.
///
/// Text longer than the field scrolls, to keep the caret in view, and with
/// the mouse wheel. A multi-line field grows with its text, or with `rows`
/// shows that many lines and scrolls the rest.
///
/// `label` names the field for screen readers, such as "Email"; give every
/// field one, as it usually matches the label shown next to it. With
/// `autofocus`, the field takes focus when it appears.
#[component]
pub fn TextField(
  #[prop(into)] value: String,
  #[prop(default = Callback::new(|_: String| {}))] on_change: Callback<String>,
  #[prop(default)] multiline: bool,
  #[prop(default = 240.0)] width: f32,
  #[prop(default)] size: Option<f32>,
  #[prop(default)] rows: Option<usize>,
  #[prop(into, default = "")] label: String,
  #[prop(default)] autofocus: bool,
) -> View {
  let buffer = use_state(|| TextBuffer::new(&value, multiline));
  let focused = use_state(|| false);
  let shown = use_state(|| value.clone());
  // Text an input method is composing, and its caret within that text.
  let composing = use_state(|| None::<Composing>);
  // The distance from the line's start that Up and Down keep to.
  let goal = use_state(|| None::<f32>);
  // The word a double click selected, while dragging selects by word.
  let words = use_state(|| None::<Range<usize>>);
  // A new value from the owner replaces what the field shows.
  if shown.map(|s| *s != value) {
    shown.set(value.clone());
    if buffer.map(|b| b.text() != value) {
      buffer.update(|b| b.set_text(&value));
    }
  }

  let font_size = size.unwrap_or_else(|| theme().text.body.size);
  let (text, caret, anchor) =
    buffer.map(|b| (b.text().to_owned(), b.caret(), b.anchor()));
  let is_focused = focused.get();

  let keys = {
    let on_change = on_change.clone();
    KeyHandler::new(move |event| {
      edit_with_key(buffer, goal, event, multiline, font_size, &on_change)
    })
  };
  let pointer = PointerHandler::new(move |event| {
    // The text on screen is not the buffer's while composing.
    if composing.map(Option::is_none) {
      goal.set(None);
      edit_with_pointer(buffer, words, event)
    }
  });
  let ime = {
    let on_change = on_change.clone();
    Callback::new(move |event| {
      edit_with_ime(buffer, composing, event, &on_change)
    })
  };

  // While composing, the text being composed is shown at the caret.
  let (text, cursor) = match composing.get() {
    Some(Composing { text: pre, cursor }) => {
      let at = caret.min(anchor);
      let mut shown_text = text;
      shown_text.insert_str(at, &pre);
      let (begin, end) = cursor.unwrap_or((pre.len(), pre.len()));
      (
        shown_text,
        TextCursor {
          caret: at + end,
          anchor: at + begin,
          show_caret: is_focused && cursor.is_some(),
          composing: Some((at, at + pre.len())),
        },
      )
    },
    None => (
      text,
      TextCursor {
        caret,
        anchor,
        show_caret: is_focused,
        composing: None,
      },
    ),
  };

  let (background, border, focused_border, radius) = theme().text_field_look();
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::xy(PADDING_X, PADDING_Y),
      width: Size::Fixed(width),
      height: match rows {
        Some(rows) if multiline => Size::Fixed(
          rows.max(1) as f32 * line_height(font_size) + 2.0 * (PADDING_Y + 1.0),
        ),
        _ => Size::Fit,
      },
      background,
      border: Border {
        width: 1.0,
        color: if is_focused { focused_border } else { border },
      },
      radius,
      // Measured as regular text, whatever is around the field.
      font_weight: Some(dovetail::FontWeight::REGULAR),
      clip: true,
      scroll: true,
      // One line moves with the caret; a bar would cover it.
      scrollbars: false,
      font_size: Some(font_size),
      ..Style::default()
    })
    .role(if multiline {
      Role::MultilineTextInput
    } else {
      Role::TextInput
    })
    .focusable(true)
    .autofocus(autofocus)
    .label(label)
    .on_key(keys)
    .on_pointer(pointer)
    .on_ime(ime)
    .on_access({
      let on_change = on_change.clone();
      Callback::new(move |action| {
        edit_for_tool(buffer, composing, action, &on_change)
      })
    })
    .on_focus(Callback::new(move |f| {
      focused.set(f);
      if !f {
        composing.set(None);
      }
    }))
    .text_cursor(cursor)
    .children([View::text(text)])
    .into()
}

/// Space inside the border, across and up and down.
const PADDING_X: f32 = 8.0;
const PADDING_Y: f32 = 5.0;

/// The height of a one-line field with text of `font_size`.
pub(crate) fn field_height(font_size: f32) -> f32 {
  line_height(font_size) + 2.0 * (PADDING_Y + 1.0)
}

/// The height of a line at `font_size`, as the painter draws it.
fn line_height(font_size: f32) -> f32 {
  let fonts = Fonts::system();
  if fonts.is_empty() {
    // The built-in stroke font (D0012).
    1.25 * font_size
  } else {
    fonts.line_metrics(font_size).line_height()
  }
}

/// Text an input method is composing.
#[derive(Clone, Debug, PartialEq)]
struct Composing {
  text: String,
  /// The input method's caret or selection in `text`, as byte offsets.
  cursor: Option<(usize, usize)>,
}

/// Carries out a text edit an assistive tool asked for.
fn edit_for_tool(
  buffer: Signal<TextBuffer>,
  composing: Signal<Option<Composing>>,
  action: AccessAction,
  on_change: &Callback<String>,
) {
  // Tools work on the text as it is, not on unfinished composing.
  composing.set(None);
  let before = buffer.map(|b| b.text().to_owned());
  buffer.update(|b| match action {
    AccessAction::SetSelection { anchor, caret } => {
      b.set_caret(anchor, false);
      b.set_caret(caret, true);
    },
    AccessAction::ReplaceSelection(text) => b.insert(&text),
    AccessAction::SetValue(text) => {
      // Kept as an edit, so it can be undone.
      b.select_all();
      b.insert(&text);
    },
    // Text has no steps.
    AccessAction::Increment
    | AccessAction::Decrement
    | AccessAction::SetNumber(_) => {},
  });
  let after = buffer.map(|b| b.text().to_owned());
  if after != before {
    on_change.call(after);
  }
}

/// Shows text being composed, or inserts committed text.
fn edit_with_ime(
  buffer: Signal<TextBuffer>,
  composing: Signal<Option<Composing>>,
  event: ImeEvent,
  on_change: &Callback<String>,
) {
  let before = buffer.map(|b| b.text().to_owned());
  match event {
    ImeEvent::Preedit { text, .. } if text.is_empty() => composing.set(None),
    ImeEvent::Preedit { text, cursor } => {
      // Composing starts by replacing the selection.
      if composing.map(Option::is_none) {
        buffer.update(|b| {
          if b.caret() != b.anchor() {
            b.insert("");
          }
        });
      }
      composing.set(Some(Composing { text, cursor }));
    },
    ImeEvent::Commit(text) => {
      composing.set(None);
      buffer.update(|b| b.insert(&text));
    },
  }
  let after = buffer.map(|b| b.text().to_owned());
  if after != before {
    on_change.call(after);
  }
}

/// True for a key that types a character: one with a character and no
/// Ctrl, Alt or Command.
fn types_character(event: &KeyEvent) -> bool {
  let m = event.modifiers;
  matches!(&event.key, KeyCode::Character(_))
    && !m.command()
    && !m.alt
    && !m.control
    && !m.meta
}

/// Applies a key to the buffer. True when the key was used.
fn edit_with_key(
  buffer: Signal<TextBuffer>,
  goal: Signal<Option<f32>>,
  event: &KeyEvent,
  multiline: bool,
  font_size: f32,
  on_change: &Callback<String>,
) -> bool {
  let m = event.modifiers;
  let shift = m.shift;
  let command = m.command();
  // Moving and deleting by word: Alt on macOS, Ctrl elsewhere.
  let word = if cfg!(target_os = "macos") {
    m.alt
  } else {
    m.control
  };
  let before = buffer.map(|b| b.text().to_owned());
  let mut used = true;
  // Up and Down keep to the same distance from the line's start; every
  // other key forgets it.
  let old_goal = goal.get();
  let mut new_goal = None;
  buffer.update(|b| match &event.key {
    KeyCode::Named(NamedKey::Backspace) => b.backspace(word),
    KeyCode::Named(NamedKey::Delete) => b.delete(word),
    // Left and Right move on the screen (M13); by word, in text order.
    KeyCode::Named(NamedKey::ArrowLeft)
      if !word && !Fonts::system().is_empty() =>
    {
      b.move_visually(shift, false, &Fonts::system(), font_size)
    },
    KeyCode::Named(NamedKey::ArrowRight)
      if !word && !Fonts::system().is_empty() =>
    {
      b.move_visually(shift, true, &Fonts::system(), font_size)
    },
    KeyCode::Named(NamedKey::ArrowLeft) => b.move_left(shift, word),
    KeyCode::Named(NamedKey::ArrowRight) => b.move_right(shift, word),
    KeyCode::Named(NamedKey::Home) => b.home(shift, command),
    KeyCode::Named(NamedKey::End) => b.end(shift, command),
    KeyCode::Named(NamedKey::ArrowUp) if multiline => {
      new_goal = Some(move_line(b, false, shift, font_size, old_goal));
    },
    KeyCode::Named(NamedKey::ArrowDown) if multiline => {
      new_goal = Some(move_line(b, true, shift, font_size, old_goal));
    },
    // On one line, Up and Down are left to the parents, such as a list of
    // suggestions under the field.
    KeyCode::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => used = false,
    KeyCode::Named(NamedKey::Enter) if multiline && !command => b.insert("\n"),
    KeyCode::Character(c) if command => match c.to_lowercase().as_str() {
      "a" => b.select_all(),
      "z" if shift => {
        b.redo();
      },
      "z" => {
        b.undo();
      },
      "y" => {
        b.redo();
      },
      "c" | "x" if b.caret() == b.anchor() => {},
      "c" => {
        set_clipboard_text(b.selected_text());
      },
      "x" => {
        if set_clipboard_text(b.selected_text()) {
          b.backspace(false);
        }
      },
      "v" => {
        // Paste with nothing on the clipboard does nothing, but is still
        // the field's key.
        if let Some(text) = clipboard_text() {
          b.insert(&text);
        }
      },
      _ => used = false,
    },
    _ => match &event.text {
      Some(text) if !text.is_empty() => b.insert(text),
      // A key that types, with no text because an input method is
      // composing, is still the field's: a letter must never reach a
      // shortcut such as a tool's `B` (M25, D0030).
      _ if types_character(event) => {},
      _ => used = false,
    },
  });
  goal.set(new_goal);
  let after = buffer.map(|b| b.text().to_owned());
  if after != before {
    on_change.call(after);
  }
  used
}

/// Moves the caret one line down (or up), to the place nearest `goal` from
/// the line's start, or the caret's own distance without one. Returns the
/// distance used, to keep to on the next line.
fn move_line(
  b: &mut TextBuffer,
  down: bool,
  extend: bool,
  font_size: f32,
  goal: Option<f32>,
) -> f32 {
  let caret = b.caret();
  let start = b.line_start(caret);
  let end = b.line_end(caret);
  let x = goal
    .unwrap_or_else(|| line_x(&b.text()[start..end], font_size, caret - start));
  let target = if down {
    if end == b.text().len() {
      b.text().len()
    } else {
      let next = end + 1;
      let next_end = b.line_end(next);
      next + line_byte(&b.text()[next..next_end], font_size, x)
    }
  } else if start == 0 {
    0
  } else {
    let previous = b.line_start(start - 1);
    previous + line_byte(&b.text()[previous..start - 1], font_size, x)
  };
  b.set_caret(target, extend);
  x
}

/// How far from the start of `line` a caret before byte `at` is, as the
/// painter draws it.
fn line_x(
  line: &str,
  font_size: f32,
  at: usize,
) -> f32 {
  let fonts = Fonts::system();
  if fonts.is_empty() {
    // The built-in stroke font (D0012) is fixed-pitch.
    line[..at].chars().count() as f32 * 0.6 * font_size
  } else {
    fonts.caret_x(line, font_size, at)
  }
}

/// The place in `line` nearest to `x`.
fn line_byte(
  line: &str,
  font_size: f32,
  x: f32,
) -> usize {
  let fonts = Fonts::system();
  if fonts.is_empty() {
    let column = (x / (0.6 * font_size)).round().max(0.0) as usize;
    line
      .char_indices()
      .nth(column)
      .map_or(line.len(), |(i, _)| i)
  } else {
    fonts.byte_at(line, font_size, x)
  }
}

/// Places the caret or selects with the pointer.
fn edit_with_pointer(
  buffer: Signal<TextBuffer>,
  words: Signal<Option<Range<usize>>>,
  event: &PointerEvent,
) {
  // Only the main button places the caret and selects.
  if event.button != dovetail::PointerButton::Primary {
    return;
  }
  let Some(at) = event.text_at else {
    return;
  };
  match (event.kind, event.clicks) {
    (PointerKind::Down, 1) => {
      words.set(None);
      buffer.update(|b| b.set_caret(at, event.modifiers.shift))
    },
    (PointerKind::Down, 2) => {
      words.set(Some(buffer.map(|b| b.word_at(at))));
      buffer.update(|b| b.select_word_at(at))
    },
    (PointerKind::Down, _) => {
      words.set(None);
      buffer.update(|b| b.select_all())
    },
    (PointerKind::Move, 1) if buffer.map(|b| b.caret() != at) => {
      buffer.update(|b| b.set_caret(at, true))
    },
    // Dragging after a double click selects whole words.
    (PointerKind::Move, 2) => {
      if let Some(first) = words.get()
        && buffer.map(|b| b.caret() != at)
      {
        buffer.update(|b| b.select_words(first, at))
      }
    },
    _ => {},
  }
}
