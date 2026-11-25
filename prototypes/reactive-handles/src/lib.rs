//! M02 prototypes: two designs for reactive state handles.
//!
//! Both give the same small API: scopes (one per component instance),
//! state owned by a scope, reads that are tracked while a scope builds,
//! updates that mark the scopes that read the state, and removal.
//!
//! - [`arena`] (prototype A): state lives in a runtime. The handle is a
//!   small `Copy` value: an index and a generation.
//! - [`shared`] (prototype B): state lives in `Rc<RefCell<T>>`. The handle
//!   is `Clone`, but not `Copy`.
//!
//! This is throw-away code to choose a design (decision D0010). It is not
//! part of the framework.

pub mod arena;
pub mod shared;

/// Identifies a scope (one component instance). Never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u64);

#[cfg(test)]
pub(crate) mod drop_counter {
  use std::cell::Cell;
  use std::rc::Rc;

  /// A value that counts how many times it has been dropped.
  #[derive(Clone)]
  pub struct Tracked(pub Rc<Cell<u32>>);

  impl Drop for Tracked {
    fn drop(&mut self) {
      self.0.set(self.0.get() + 1);
    }
  }
}
