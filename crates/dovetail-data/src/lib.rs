//! Data views for Dovetail UI (M16, D0022): lists, trees and tables that
//! show very many rows quickly, and a property grid for inspectors.
//!
//! - [`DataList`], [`DataTree`] and [`DataTable`] make only the rows in
//!   sight, and a few more. Layout tells them where they are scrolled to,
//!   and the window lays out again in the same frame when new rows come
//!   into sight, so scrolling never shows blank rows.
//! - Data comes from functions: a row count, a key for each row and a
//!   function that builds a row. A tree asks for the children of a node
//!   only when the node is open.
//! - [`Selection`] holds the selected rows, by key, and the row the
//!   keyboard is on. Clicks and keys follow each system's habits.
//! - Row heights are fixed, or measured as rows appear ([`RowHeight`]).
//! - A table shows which column it is sorted by and tells the application
//!   when a header is pressed; the application sorts its own data.
//! - [`PropertyGrid`] edits values with the everyday controls, and records
//!   each change in the application's `History`, so it can be undone.
//! - Assistive tools are told where each row is in the whole collection,
//!   not only among the rows made.

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

/// A property made from a closure, compared by identity.
macro_rules! function_prop {
  ($(#[$doc:meta])* $name:ident, ($($arg:ty),*) -> $ret:ty) => {
    $(#[$doc])*
    ///
    /// Made from a closure. Two are equal only if they are the same
    /// closure, so a parent that makes a new one in each build also builds
    /// the view again.
    #[derive(Clone)]
    pub struct $name(std::rc::Rc<dyn Fn($($arg),*) -> $ret>);

    impl PartialEq for $name {
      fn eq(
        &self,
        other: &Self,
      ) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
      }
    }

    impl std::fmt::Debug for $name {
      fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
      ) -> std::fmt::Result {
        f.write_str(stringify!($name))
      }
    }

    impl<F: Fn($($arg),*) -> $ret + 'static> From<F> for $name {
      fn from(f: F) -> Self {
        $name(std::rc::Rc::new(f))
      }
    }
  };
}

mod body;
mod grid;
mod heights;
mod list;
mod selection;
mod table;
mod tree;

pub use body::{KeyFn, RowDrag, RowFn, RowState, RowsDropped, TextFn};
pub use grid::{Property, PropertyGrid, PropertySection, PropertyValue};
pub use heights::{Heights, RowHeight};
pub use list::DataList;
pub use selection::{Selection, SelectionMode};
pub use table::{CellFn, Column, DataTable, Sort};
pub use tree::{
  ChildrenFn, DataTree, Expansion, HasChildrenFn, NodeFn, NodeTextFn,
};
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
