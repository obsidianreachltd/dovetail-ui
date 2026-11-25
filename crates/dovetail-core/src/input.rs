//! Keyboard input: key events and the handlers that receive them.

use std::fmt;
use std::rc::Rc;

/// A key with a name rather than a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
  /// Enter or Return.
  Enter,
  /// Escape.
  Escape,
  /// Tab.
  Tab,
  /// Space.
  Space,
  /// Backspace.
  Backspace,
  /// Delete (forward).
  Delete,
  /// Left arrow.
  ArrowLeft,
  /// Right arrow.
  ArrowRight,
  /// Up arrow.
  ArrowUp,
  /// Down arrow.
  ArrowDown,
  /// Home.
  Home,
  /// End.
  End,
  /// Page Up.
  PageUp,
  /// Page Down.
  PageDown,
  /// Insert.
  Insert,
  /// F1.
  F1,
  /// F2.
  F2,
  /// F3.
  F3,
  /// F4.
  F4,
  /// F5.
  F5,
  /// F6.
  F6,
  /// F7.
  F7,
  /// F8.
  F8,
  /// F9.
  F9,
  /// F10.
  F10,
  /// F11.
  F11,
  /// F12.
  F12,
  /// The Menu key, which opens a context menu (M15).
  ContextMenu,
}

/// Which key was pressed. (`Key` is the list key of [`crate::View::keyed`].)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
  /// A key that types a character, such as `a` or `A`. With Ctrl held, this
  /// is still the character on the key, so shortcuts can match it.
  Character(String),
  /// A key with a name.
  Named(NamedKey),
  /// Any other key.
  Other,
}

/// The modifier keys held during a key press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
  /// Shift.
  pub shift: bool,
  /// Ctrl (Control on macOS).
  pub control: bool,
  /// Alt (Option on macOS).
  pub alt: bool,
  /// The Windows key, or Command on macOS.
  pub meta: bool,
}

impl Modifiers {
  /// The usual shortcut modifier: Command on macOS, Ctrl elsewhere.
  pub fn command(self) -> bool {
    if cfg!(target_os = "macos") {
      self.meta
    } else {
      self.control
    }
  }

  /// True when no modifier is held.
  pub fn is_empty(self) -> bool {
    self == Modifiers::default()
  }
}

/// One key press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
  /// The key.
  pub key: KeyCode,
  /// The modifiers held.
  pub modifiers: Modifiers,
  /// The text the key types, if any. Empty for named keys and shortcuts.
  pub text: Option<String>,
  /// True when the key is held down and the system repeats it.
  pub repeat: bool,
}

impl KeyEvent {
  /// A press of a named key with no modifiers.
  pub fn named(key: NamedKey) -> Self {
    KeyEvent {
      key: KeyCode::Named(key),
      modifiers: Modifiers::default(),
      text: None,
      repeat: false,
    }
  }

  /// A press of a key that types `c`, with no modifiers.
  pub fn character(c: char) -> Self {
    KeyEvent {
      key: KeyCode::Character(c.to_string()),
      modifiers: Modifiers::default(),
      text: Some(c.to_string()),
      repeat: false,
    }
  }

  /// The same event with these modifiers.
  pub fn with(
    mut self,
    modifiers: Modifiers,
  ) -> Self {
    self.modifiers = modifiers;
    self
  }

  /// True for this named key.
  pub fn is(
    &self,
    key: NamedKey,
  ) -> bool {
    self.key == KeyCode::Named(key)
  }
}

/// Receives key events for an element. It returns `true` when it handled
/// the key; otherwise the key goes on to the element's parents.
///
/// Two handlers are equal only if they are the same handler.
pub struct KeyHandler(Rc<dyn Fn(&KeyEvent) -> bool>);

impl KeyHandler {
  /// A handler that runs `handler`.
  pub fn new(handler: impl Fn(&KeyEvent) -> bool + 'static) -> Self {
    KeyHandler(Rc::new(handler))
  }

  /// Offers `event` to the handler. True when it was handled.
  pub fn handle(
    &self,
    event: &KeyEvent,
  ) -> bool {
    (self.0)(event)
  }
}

impl Clone for KeyHandler {
  fn clone(&self) -> Self {
    KeyHandler(Rc::clone(&self.0))
  }
}

impl PartialEq for KeyHandler {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl fmt::Debug for KeyHandler {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("KeyHandler")
  }
}

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerKind {
  /// The main button went down over the element.
  Down,
  /// The pointer moved while the button was held.
  Move,
  /// The button was let go.
  Up,
}

/// Which mouse button a pointer event is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerButton {
  /// The main button, usually the left. The default.
  #[default]
  Primary,
  /// The middle button, or a pressed wheel.
  Middle,
  /// The other button, usually the right.
  Secondary,
}

/// The pointer, as an element with an `on_pointer` handler receives it.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerEvent {
  /// What happened.
  pub kind: PointerKind,
  /// The button pressed, held or let go (M09).
  pub button: PointerButton,
  /// Position from the element's left edge, in logical pixels.
  pub x: f32,
  /// Position from the element's top edge, in logical pixels.
  pub y: f32,
  /// Position in the window, in logical pixels (M10). Unlike `x` and `y`
  /// it does not move with the element, so a drag that moves the element,
  /// such as a splitter's, can measure how far the pointer went.
  pub window_x: f32,
  /// Position in the window, down from its top, in logical pixels (M10).
  pub window_y: f32,
  /// The byte of the element's first text child nearest the pointer, where
  /// a caret would go. `None` when the element has no text.
  pub text_at: Option<usize>,
  /// The modifier keys held.
  pub modifiers: Modifiers,
  /// 1 for a single click, 2 for a double click, and so on. Moves and
  /// releases repeat the count of the press.
  pub clicks: u32,
}

/// Something that receives the pointer. Two handlers are equal only when
/// they are the same handler.
pub struct PointerHandler(Rc<dyn Fn(&PointerEvent)>);

impl PointerHandler {
  /// A handler that runs `handler`.
  pub fn new(handler: impl Fn(&PointerEvent) + 'static) -> Self {
    PointerHandler(Rc::new(handler))
  }

  /// Passes `event` to the handler.
  pub fn handle(
    &self,
    event: &PointerEvent,
  ) {
    (self.0)(event)
  }
}

impl Clone for PointerHandler {
  fn clone(&self) -> Self {
    PointerHandler(Rc::clone(&self.0))
  }
}

impl PartialEq for PointerHandler {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl fmt::Debug for PointerHandler {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("PointerHandler")
  }
}

/// Text from an input method (IME), such as one for Japanese or Chinese,
/// for the focused element (M06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
  /// Text being composed, to show at the caret, underlined, until it is
  /// committed. Empty text means there is nothing being composed.
  Preedit {
    /// The text being composed.
    text: String,
    /// Where the input method's caret (and selection) is in `text`, as
    /// byte offsets. `None` hides the caret.
    cursor: Option<(usize, usize)>,
  },
  /// Finished text, to insert as if typed.
  Commit(String),
}

/// A text edit asked for by an assistive tool, such as a screen reader,
/// for an element that takes text (M07). Offsets are bytes into the
/// element's first text child.
#[derive(Clone, Debug, PartialEq)]
pub enum AccessAction {
  /// Select from `anchor` to `caret`; equal offsets place the caret.
  SetSelection {
    /// Where the selection starts.
    anchor: usize,
    /// Where the caret goes.
    caret: usize,
  },
  /// Replace the selection, or insert at the caret.
  ReplaceSelection(String),
  /// Replace all the text.
  SetValue(String),
  /// Move a value up one step, as a slider's Up arrow does (M15).
  Increment,
  /// Move a value down one step (M15).
  Decrement,
  /// Set a value in a range to this number (M15).
  SetNumber(f32),
}

/// The mouse wheel or a trackpad, as an element with an `on_wheel` handler
/// receives it (M09).
#[derive(Clone, Debug, PartialEq)]
pub struct WheelEvent {
  /// The pointer's position from the element's left edge, in logical
  /// pixels.
  pub x: f32,
  /// The pointer's position from the element's top edge.
  pub y: f32,
  /// How far to scroll across, in logical pixels: positive shows more of
  /// what is to the right.
  pub dx: f32,
  /// How far to scroll down: positive shows more of what is below.
  pub dy: f32,
  /// The movement came from a wheel that turns in steps, rather than a
  /// trackpad or smooth wheel that gives exact distances.
  pub steps: bool,
  /// The modifier keys held. Windows sends a trackpad pinch as the wheel
  /// with Ctrl.
  pub modifiers: Modifiers,
}
