//! Documents for Dovetail UI (M11): saving, opening, unsaved changes,
//! recovery after a crash, and long-running work (decision D0017).
//!
//! - [`Document`] holds an application's data outside every widget. The
//!   data changes only through [`Document::edit`], which makes an undoable
//!   step, so every edit can be a command.
//! - [`DocumentKind`] says how the data is written to a file and read back:
//!   the format is the application's own.
//! - [`DocumentFile`] knows the document's file and whether it has unsaved
//!   changes. [`DocumentFile::add_commands`] defines New, Open, Save, Save
//!   As, Undo and Redo.
//! - [`DocumentFrame`] goes round the window's content. It sets the
//!   window's title, asks before closing with unsaved changes, and shows
//!   the document's questions.
//! - [`Recovery`] keeps snapshots of unsaved work, and finds what a program
//!   that stopped left behind.
//! - [`TaskProgress`] shows a task's progress, from
//!   `dovetail_core::spawn_task`, with a Cancel button.
//!
//! Open and Save As use the system's file dialogs where there are some,
//! and a dialog of Dovetail's own elsewhere.

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

mod document;
mod recovery;
mod views;

pub use document::{
  After, Document, DocumentFile, DocumentKind, FileState, Question,
};
pub use recovery::{Recovered, Recovery};
pub use views::{DocumentFrame, TaskProgress};

/// State for tests: made while a component builds, as all state must be,
/// and kept while the returned tree lives.
#[doc(hidden)]
pub fn owned<T: Clone + 'static>(
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
