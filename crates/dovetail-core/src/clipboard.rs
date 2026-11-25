//! The clipboard, as controls see it (M06).
//!
//! Controls call [`clipboard_text`] and [`set_clipboard_text`]. The desktop
//! crate installs the system clipboard with [`set_clipboard`]. Until it
//! does, as in tests, the clipboard is a string inside the application.

use std::cell::RefCell;
use std::rc::Rc;

/// A place to copy plain text to and paste it from.
pub trait Clipboard {
  /// The text on the clipboard, or `None` when it holds no text or cannot
  /// be read.
  fn text(&self) -> Option<String>;

  /// Puts `text` on the clipboard. True when it worked.
  fn set_text(
    &self,
    text: &str,
  ) -> bool;
}

/// A clipboard that lives inside the application.
#[derive(Debug, Default)]
pub struct MemoryClipboard {
  text: RefCell<Option<String>>,
}

impl Clipboard for MemoryClipboard {
  fn text(&self) -> Option<String> {
    self.text.borrow().clone()
  }

  fn set_text(
    &self,
    text: &str,
  ) -> bool {
    *self.text.borrow_mut() = Some(text.to_owned());
    true
  }
}

thread_local! {
  static CLIPBOARD: RefCell<Rc<dyn Clipboard>> =
    RefCell::new(Rc::new(MemoryClipboard::default()));
}

/// Makes `clipboard` the clipboard for this thread, or a fresh
/// [`MemoryClipboard`] with `None`.
pub fn set_clipboard(clipboard: Option<Rc<dyn Clipboard>>) {
  let clipboard =
    clipboard.unwrap_or_else(|| Rc::new(MemoryClipboard::default()));
  CLIPBOARD.with_borrow_mut(|c| *c = clipboard);
}

/// The text on the clipboard. Line breaks are always `\n`.
pub fn clipboard_text() -> Option<String> {
  // The clipboard is cloned out first, so it may itself use this module.
  let clipboard = CLIPBOARD.with_borrow(Rc::clone);
  clipboard
    .text()
    .map(|t| t.replace("\r\n", "\n").replace('\r', "\n"))
}

/// Puts `text` on the clipboard. True when it worked.
pub fn set_clipboard_text(text: &str) -> bool {
  let clipboard = CLIPBOARD.with_borrow(Rc::clone);
  clipboard.set_text(text)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_memory_clipboard_is_used_until_another_is_set() {
    set_clipboard(None);
    assert_eq!(clipboard_text(), None);
    assert!(set_clipboard_text("a\r\nb"));
    assert_eq!(clipboard_text().as_deref(), Some("a\nb"));

    struct Fixed;
    impl Clipboard for Fixed {
      fn text(&self) -> Option<String> {
        Some("fixed".into())
      }
      fn set_text(
        &self,
        _: &str,
      ) -> bool {
        false
      }
    }
    set_clipboard(Some(Rc::new(Fixed)));
    assert_eq!(clipboard_text().as_deref(), Some("fixed"));
    assert!(!set_clipboard_text("x"));
    set_clipboard(None);
  }
}
