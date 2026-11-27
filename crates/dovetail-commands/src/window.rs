//! Standard commands for windows (M19).

use std::cell::Cell;
use std::rc::Rc;

use dovetail::{Signal, WindowState};

use crate::Commands;

/// The id of the full-screen command.
pub const FULL_SCREEN: &str = "view.full_screen";

/// The full-screen shortcut: F11 on Windows and Linux, Control+Command+F on
/// macOS, as each system's own applications use.
pub fn full_screen_shortcut() -> &'static str {
  if cfg!(target_os = "macos") {
    "MacCtrl+Mod+F"
  } else {
    "F11"
  }
}

/// Defines Full Screen (`view.full_screen`) for a window whose state is
/// kept in `state`, the signal given to its `Window` as `state` and set by
/// its `on_state`. It enters full screen, and leaves it for the state the
/// window was in before. Its shortcut is [`full_screen_shortcut`]. Add it
/// to a menu with its id; on macOS that puts it in the menu bar.
pub fn add_full_screen(
  commands: Commands,
  state: Signal<WindowState>,
) -> Commands {
  let before = Rc::new(Cell::new(WindowState::Normal));
  commands.define(
    FULL_SCREEN,
    "Full Screen",
    Some(full_screen_shortcut()),
    move || {
      let Ok(now) = state.try_map(|s| *s) else {
        return;
      };
      if now == WindowState::FullScreen {
        let _ = state.try_update(|s| *s = before.get());
      } else {
        before.set(now);
        let _ = state.try_update(|s| *s = WindowState::FullScreen);
      }
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Shortcut;
  use dovetail::create_signal;

  #[test]
  fn full_screen_goes_in_and_back_to_the_state_before() {
    let (_tree, (state, commands)) = crate::owned(|| {
      let state = create_signal(WindowState::Maximised);
      (state, add_full_screen(Commands::new(), state))
    });
    assert!(commands.run(FULL_SCREEN));
    assert_eq!(state.get(), WindowState::FullScreen);
    assert!(commands.run(FULL_SCREEN));
    assert_eq!(state.get(), WindowState::Maximised, "back as it was");
  }

  #[test]
  fn the_shortcut_suits_each_system() {
    let s = Shortcut::parse(full_screen_shortcut()).unwrap();
    if cfg!(target_os = "macos") {
      assert!(s.primary && s.other && !s.shift && !s.alt);
      assert_eq!(s.label(), "⌃⌘F");
    } else {
      assert!(!s.primary && !s.other);
      assert_eq!(s.label(), "F11");
    }
  }

  #[test]
  fn the_other_modifier_is_its_own() {
    let s = Shortcut::parse("MacCtrl+Mod+F").unwrap();
    let mut m = dovetail::Modifiers::default();
    if cfg!(target_os = "macos") {
      m.meta = true;
      m.control = true;
    } else {
      m.control = true;
      m.meta = true;
    }
    let press = |modifiers| dovetail::KeyEvent {
      key: dovetail::KeyCode::Character("f".into()),
      modifiers,
      text: None,
      repeat: false,
    };
    assert!(s.matches(&press(m)));
    let mut without = m;
    if cfg!(target_os = "macos") {
      without.control = false;
    } else {
      without.meta = false;
    }
    assert!(!s.matches(&press(without)), "the other modifier is needed");
  }
}
