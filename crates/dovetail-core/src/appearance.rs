//! How the user has set up their system: light or dark, the accent
//! colour, and high contrast (M14). The desktop crate sets it, and themes
//! read it.

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::element::Color;
use crate::reactive::{self, ComponentId};

/// The system's appearance settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemAppearance {
  /// The user chose dark mode.
  pub dark: bool,
  /// The user's accent colour, where the system has one.
  pub accent: Option<Color>,
  /// The user asked for high contrast.
  pub high_contrast: bool,
}

thread_local! {
  static APPEARANCE: RefCell<(SystemAppearance, BTreeSet<ComponentId>)> =
    RefCell::new((SystemAppearance::default(), BTreeSet::new()));
}

/// The system's appearance. Read while a component builds, it builds the
/// component again when the appearance changes. Without a desktop, as in
/// tests, it is light, with no accent and no high contrast, until
/// [`set_system_appearance`] changes it.
pub fn system_appearance() -> SystemAppearance {
  let current = reactive::current();
  APPEARANCE.with_borrow_mut(|(appearance, readers)| {
    if let Some(id) = current {
      readers.insert(id);
    }
    *appearance
  })
}

/// Sets the system's appearance. The desktop crate calls it when the
/// system's settings change; components that read it are built again.
pub fn set_system_appearance(appearance: SystemAppearance) {
  let readers = APPEARANCE.with_borrow_mut(|(old, readers)| {
    if *old == appearance {
      return None;
    }
    *old = appearance;
    Some(std::mem::take(readers))
  });
  if let Some(readers) = readers {
    reactive::mark(readers);
  }
}
