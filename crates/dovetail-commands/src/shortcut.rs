//! Keyboard shortcuts: parsed from text such as `Mod+Shift+Z`, matched
//! against key presses, and shown the way the platform shows them.

use std::fmt;
use std::str::FromStr;

use dovetail::{KeyCode, KeyEvent, MenuKey, NamedKey};

/// The key of a shortcut.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShortcutKey {
  /// A key that types a character, kept in lower case.
  Char(char),
  /// A key with a name.
  Named(NamedKey),
}

/// A keyboard shortcut: a key with modifiers.
///
/// `primary` is the platform's main shortcut modifier: Command on macOS,
/// Ctrl on Windows and Linux. Shortcuts are written with `Mod` for it, so
/// one definition suits every system.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
  /// The key.
  pub key: ShortcutKey,
  /// With the main modifier (Command or Ctrl).
  pub primary: bool,
  /// With Shift.
  pub shift: bool,
  /// With Alt (Option on macOS).
  pub alt: bool,
  /// With the other modifier: Control on macOS, the Windows or Super key
  /// elsewhere (M19).
  pub other: bool,
}

/// Why text is not a shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutError {
  /// There was no key, only modifiers, or nothing at all.
  NoKey,
  /// A part was not a modifier or a key name.
  Unknown(String),
  /// There was more than one key.
  TwoKeys,
}

impl fmt::Display for ShortcutError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      ShortcutError::NoKey => f.write_str("a shortcut needs a key"),
      ShortcutError::Unknown(part) => write!(f, "\"{part}\" is not a key"),
      ShortcutError::TwoKeys => f.write_str("a shortcut has only one key"),
    }
  }
}

impl std::error::Error for ShortcutError {}

/// Named keys and the words for them, as written and as shown.
const NAMES: &[(NamedKey, &str)] = &[
  (NamedKey::Enter, "Enter"),
  (NamedKey::Escape, "Esc"),
  (NamedKey::Tab, "Tab"),
  (NamedKey::Space, "Space"),
  (NamedKey::Backspace, "Backspace"),
  (NamedKey::Delete, "Delete"),
  (NamedKey::Insert, "Insert"),
  (NamedKey::ArrowLeft, "Left"),
  (NamedKey::ArrowRight, "Right"),
  (NamedKey::ArrowUp, "Up"),
  (NamedKey::ArrowDown, "Down"),
  (NamedKey::Home, "Home"),
  (NamedKey::End, "End"),
  (NamedKey::PageUp, "PageUp"),
  (NamedKey::PageDown, "PageDown"),
  (NamedKey::F1, "F1"),
  (NamedKey::F2, "F2"),
  (NamedKey::F3, "F3"),
  (NamedKey::F4, "F4"),
  (NamedKey::F5, "F5"),
  (NamedKey::F6, "F6"),
  (NamedKey::F7, "F7"),
  (NamedKey::F8, "F8"),
  (NamedKey::F9, "F9"),
  (NamedKey::F10, "F10"),
  (NamedKey::F11, "F11"),
  (NamedKey::F12, "F12"),
  (NamedKey::ContextMenu, "Menu"),
];

fn named(word: &str) -> Option<NamedKey> {
  let word = word.to_ascii_lowercase();
  let alias = match word.as_str() {
    "return" => "enter",
    "escape" => "esc",
    "del" => "delete",
    "ins" => "insert",
    "arrowleft" => "left",
    "arrowright" => "right",
    "arrowup" => "up",
    "arrowdown" => "down",
    "pgup" => "pageup",
    "pgdn" | "pgdown" => "pagedown",
    other => other,
  };
  NAMES
    .iter()
    .find(|(_, name)| name.eq_ignore_ascii_case(alias))
    .map(|(key, _)| *key)
}

fn name_of(key: NamedKey) -> &'static str {
  NAMES
    .iter()
    .find(|(k, _)| *k == key)
    .map_or("?", |(_, name)| name)
}

impl FromStr for Shortcut {
  type Err = ShortcutError;

  /// Reads text such as `Mod+S`, `Mod+Shift+Z`, `Alt+Enter` or `F5`.
  /// `Mod`, `Ctrl`, `Cmd` and `Command` all mean the main modifier;
  /// `Option` means Alt. `MacCtrl`, `Super` and `Win` mean the other
  /// modifier: Control on macOS, the Windows key elsewhere. `Plus` is the
  /// `+` key.
  fn from_str(text: &str) -> Result<Shortcut, ShortcutError> {
    let mut shortcut = Shortcut {
      key: ShortcutKey::Char(' '),
      primary: false,
      shift: false,
      alt: false,
      other: false,
    };
    let mut key = None;
    for part in text.split('+').map(str::trim) {
      match part.to_ascii_lowercase().as_str() {
        "" => return Err(ShortcutError::Unknown(text.to_owned())),
        "mod" | "ctrl" | "control" | "cmd" | "command" | "primary" => {
          shortcut.primary = true
        },
        "shift" => shortcut.shift = true,
        "alt" | "option" | "opt" => shortcut.alt = true,
        "macctrl" | "super" | "win" | "meta" => shortcut.other = true,
        _ => {
          if key.is_some() {
            return Err(ShortcutError::TwoKeys);
          }
          let mut chars = part.chars();
          key = Some(match (chars.next(), chars.next()) {
            (Some(c), None) => ShortcutKey::Char(c.to_ascii_lowercase()),
            _ if part.eq_ignore_ascii_case("plus") => ShortcutKey::Char('+'),
            _ => ShortcutKey::Named(
              named(part)
                .ok_or_else(|| ShortcutError::Unknown(part.to_owned()))?,
            ),
          });
        },
      }
    }
    shortcut.key = key.ok_or(ShortcutError::NoKey)?;
    Ok(shortcut)
  }
}

impl Shortcut {
  /// Reads a shortcut; see [`Shortcut::from_str`].
  pub fn parse(text: &str) -> Result<Shortcut, ShortcutError> {
    text.parse()
  }

  /// True when `event` is a press of this shortcut. The other modifier
  /// (Ctrl on macOS, the Windows key elsewhere) must not be held.
  pub fn matches(
    &self,
    event: &KeyEvent,
  ) -> bool {
    let m = event.modifiers;
    let other = if cfg!(target_os = "macos") {
      m.control
    } else {
      m.meta
    };
    if m.command() != self.primary
      || m.shift != self.shift
      || m.alt != self.alt
      || other != self.other
    {
      return false;
    }
    match (&self.key, &event.key) {
      (ShortcutKey::Named(a), KeyCode::Named(b)) => a == b,
      (ShortcutKey::Named(NamedKey::Space), KeyCode::Character(s)) => s == " ",
      (ShortcutKey::Char(c), KeyCode::Character(s)) => {
        let mut chars = s.chars().flat_map(char::to_lowercase);
        chars.next() == Some(*c) && chars.next().is_none()
      },
      _ => false,
    }
  }

  /// The text shown for this shortcut: "Ctrl+Shift+Z" on Windows and
  /// Linux, "⇧⌘Z" on macOS.
  pub fn label(&self) -> String {
    let key = match &self.key {
      ShortcutKey::Char('+') => "Plus".to_owned(),
      ShortcutKey::Char(c) => c.to_uppercase().collect(),
      ShortcutKey::Named(k) => name_of(*k).to_owned(),
    };
    if cfg!(target_os = "macos") {
      let mut s = String::new();
      if self.other {
        s.push('⌃');
      }
      if self.alt {
        s.push('⌥');
      }
      if self.shift {
        s.push('⇧');
      }
      if self.primary {
        s.push('⌘');
      }
      s + &key
    } else {
      let mut parts = Vec::new();
      if self.primary {
        parts.push("Ctrl");
      }
      if self.other {
        parts.push("Win");
      }
      if self.alt {
        parts.push("Alt");
      }
      if self.shift {
        parts.push("Shift");
      }
      parts.push(&key);
      parts.join("+")
    }
  }

  /// The shortcut as a system menu shows it.
  pub fn menu_key(&self) -> MenuKey {
    MenuKey {
      key: match &self.key {
        ShortcutKey::Char(c) => c.to_string(),
        ShortcutKey::Named(k) => name_of(*k).to_owned(),
      },
      primary: self.primary,
      shift: self.shift,
      alt: self.alt,
      other: self.other,
    }
  }
}

impl fmt::Display for Shortcut {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str(&self.label())
  }
}

#[cfg(test)]
mod tests {
  use dovetail::Modifiers;

  use super::*;

  fn press(
    c: char,
    primary: bool,
    shift: bool,
  ) -> KeyEvent {
    let mut m = Modifiers { shift, ..Modifiers::default() };
    if cfg!(target_os = "macos") {
      m.meta = primary;
    } else {
      m.control = primary;
    }
    let mut e = KeyEvent::character(c).with(m);
    e.text = None;
    e
  }

  #[test]
  fn shortcuts_are_read_from_text() {
    let s = Shortcut::parse("Mod+Shift+Z").unwrap();
    assert_eq!(s.key, ShortcutKey::Char('z'));
    assert!(s.primary && s.shift && !s.alt);
    assert_eq!(Shortcut::parse("ctrl + s"), Shortcut::parse("Cmd+S"));
    assert_eq!(
      Shortcut::parse("F5").unwrap().key,
      ShortcutKey::Named(NamedKey::F5)
    );
    assert_eq!(
      Shortcut::parse("Alt+Return").unwrap().key,
      ShortcutKey::Named(NamedKey::Enter)
    );
    assert_eq!(
      Shortcut::parse("Mod+Plus").unwrap().key,
      ShortcutKey::Char('+')
    );
    assert_eq!(Shortcut::parse("Mod+Shift"), Err(ShortcutError::NoKey));
    assert_eq!(
      Shortcut::parse(""),
      Err(ShortcutError::Unknown(String::new()))
    );
    assert_eq!(Shortcut::parse("Mod+A+B"), Err(ShortcutError::TwoKeys));
    assert_eq!(
      Shortcut::parse("Hyper+K"),
      Err(ShortcutError::Unknown("Hyper".into()))
    );
  }

  #[test]
  fn shortcuts_match_presses_with_exactly_their_modifiers() {
    let undo = Shortcut::parse("Mod+Z").unwrap();
    let redo = Shortcut::parse("Mod+Shift+Z").unwrap();
    assert!(undo.matches(&press('z', true, false)));
    assert!(
      !undo.matches(&press('z', true, true)),
      "Shift is not wanted"
    );
    assert!(
      redo.matches(&press('Z', true, true)),
      "case does not matter"
    );
    assert!(!undo.matches(&press('z', false, false)));
    assert!(!undo.matches(&press('x', true, false)));
    let f5 = Shortcut::parse("F5").unwrap();
    assert!(f5.matches(&KeyEvent::named(NamedKey::F5)));
    assert!(!f5.matches(&KeyEvent::named(NamedKey::F6)));
  }

  #[test]
  fn shortcuts_are_shown_the_platforms_way() {
    let label = Shortcut::parse("Mod+Alt+Shift+K")
      .unwrap()
      .label();
    if cfg!(target_os = "macos") {
      assert_eq!(label, "⌥⇧⌘K");
    } else {
      assert_eq!(label, "Ctrl+Alt+Shift+K");
    }
    assert_eq!(Shortcut::parse("F2").unwrap().label(), "F2");
    let key = Shortcut::parse("Mod+Shift+Z").unwrap().menu_key();
    assert_eq!(key.key, "z");
    assert!(key.primary && key.shift && !key.alt);
  }
}
