//! Commands for Dovetail UI (M08): actions defined once and used from
//! menus, toolbar buttons, keyboard shortcuts and command search, with
//! undo and redo.
//!
//! - [`Commands`] holds the application's commands: each has an id, a
//!   name, an optional [`Shortcut`] and what it does, and can be enabled or
//!   disabled, or enabled by a rule.
//! - [`History`] does edits so they can be undone and redone, and defines
//!   the Undo and Redo commands.
//! - [`CommandScope`] runs commands from their shortcuts.
//! - [`MenuBar`], [`ContextMenu`], [`Toolbar`] with [`CommandButton`]s,
//!   and [`CommandSearch`] show the commands. A command that is disabled is
//!   greyed out in all of them.
//!
//! On macOS the menu bar goes to the system's own menu bar.

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

mod commands;
mod context;
mod history;
mod palette;
mod shortcut;
mod views;
mod window;

pub use commands::{CommandInfo, Commands, ShortcutConflict};
pub use context::ContextMenu;
pub use history::History;
pub use palette::{LONG_PRESS, ToolPalette};
pub use shortcut::{Shortcut, ShortcutError, ShortcutKey};
pub use views::{
  CommandButton, CommandGroup, CommandScope, CommandSearch, Menu, MenuBar,
  MenuItem, Toolbar,
};
pub use window::{FULL_SCREEN, add_full_screen, full_screen_shortcut};

/// State for unit tests: made while a component builds, as all state must
/// be, and kept while the returned tree lives.
#[cfg(test)]
pub(crate) fn owned<T: Clone + 'static>(
  make: impl FnOnce() -> T + 'static
) -> (dovetail::Tree, T) {
  use std::any::Any;
  use std::cell::RefCell;

  type Maker = Box<dyn FnOnce() -> Box<dyn Any>>;
  thread_local! {
    static MAKE: RefCell<Option<Maker>> = RefCell::new(None);
    static MADE: RefCell<Option<Box<dyn Any>>> = RefCell::new(None);
  }
  struct Host;
  impl dovetail::Component for Host {
    type Props = ();

    fn build(
      _: &(),
      _: &mut dovetail::BuildContext<'_>,
    ) -> dovetail::View {
      if let Some(make) = MAKE.with_borrow_mut(Option::take) {
        let made = make();
        MADE.with_borrow_mut(|m| *m = Some(made));
      }
      dovetail::View::Empty
    }
  }
  MAKE.with_borrow_mut(|m| {
    *m = Some(Box::new(move || Box::new(make()) as Box<dyn Any>));
  });
  let mut tree = dovetail::Tree::new();
  tree.mount(dovetail::View::component::<Host>(()));
  let made = MADE
    .with_borrow_mut(Option::take)
    .expect("made while building")
    .downcast::<T>()
    .expect("the type asked for");
  (tree, *made)
}
