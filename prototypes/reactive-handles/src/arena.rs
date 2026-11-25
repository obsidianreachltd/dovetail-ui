//! Prototype A: arena handles.
//!
//! All state lives in a runtime that belongs to the UI thread. A
//! [`Signal`] is an index into the runtime and a generation number. It is
//! `Copy`, so closures can capture it with `move` and it can still be used
//! afterwards. When its scope is removed, the slot's generation changes, and
//! the old handle gives a clear error instead of reading new data.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;

use crate::ScopeId;

/// The error when a handle's scope has been removed.
#[derive(Debug, PartialEq, Eq)]
pub struct Disposed;

impl fmt::Display for Disposed {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("this state belonged to a component that has been removed")
  }
}

struct Slot {
  generation: u32,
  value: Option<Box<dyn Any>>,
  subscribers: BTreeSet<ScopeId>,
}

struct ScopeData {
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
  owned: Vec<u32>,
  reads: BTreeSet<u32>,
}

#[derive(Default)]
struct Runtime {
  slots: Vec<Slot>,
  free: Vec<u32>,
  scopes: std::collections::HashMap<ScopeId, ScopeData>,
  next_scope: u64,
  current: Option<ScopeId>,
  dirty: BTreeSet<ScopeId>,
}

thread_local! {
  static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
}

/// A handle to state. `Copy`, whatever `T` is.
///
/// The `ui!` example from docs/architecture/syntax.md, written by hand
/// exactly as in the document, compiles: `count` is copied into the closure.
///
/// ```
/// use prototype_reactive_handles::arena::*;
/// let root = create_scope(None);
/// build(root, || {
///   let count = use_state(|| 0);
///   let on_click = move |_: ()| count.update(|value| *value += 1);
///   let text = count.map(|value| format!("Count: {value}"));
///   (text, on_click)
/// });
/// dispose(root);
/// ```
pub struct Signal<T> {
  index: u32,
  generation: u32,
  _type: PhantomData<fn() -> T>,
}

impl<T> Clone for Signal<T> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<T> Copy for Signal<T> {}

/// Makes a scope (a component instance) under `parent`.
pub fn create_scope(parent: Option<ScopeId>) -> ScopeId {
  RT.with_borrow_mut(|rt| {
    rt.next_scope += 1;
    let id = ScopeId(rt.next_scope);
    rt.scopes.insert(
      id,
      ScopeData {
        parent,
        children: Vec::new(),
        owned: Vec::new(),
        reads: BTreeSet::new(),
      },
    );
    if let Some(p) = parent.and_then(|p| rt.scopes.get_mut(&p)) {
      p.children.push(id);
    }
    id
  })
}

/// Runs `build` as `scope`: state made in it belongs to `scope`, and reads in
/// it subscribe `scope`. Earlier reads by `scope` are forgotten first.
pub fn build<R>(
  scope: ScopeId,
  build: impl FnOnce() -> R,
) -> R {
  let previous = RT.with_borrow_mut(|rt| {
    let reads = rt
      .scopes
      .get_mut(&scope)
      .map(|s| std::mem::take(&mut s.reads))
      .unwrap_or_default();
    for index in reads {
      rt.slots[index as usize]
        .subscribers
        .remove(&scope);
    }
    rt.current.replace(scope)
  });
  let result = build();
  RT.with_borrow_mut(|rt| rt.current = previous);
  result
}

/// Makes state owned by the scope that is building.
///
/// # Panics
///
/// If no scope is building.
pub fn use_state<T: 'static>(init: impl FnOnce() -> T) -> Signal<T> {
  let value: Box<dyn Any> = Box::new(init());
  RT.with_borrow_mut(|rt| {
    let owner = rt
      .current
      .expect("use_state must be called while a component builds");
    let index = match rt.free.pop() {
      Some(index) => {
        let slot = &mut rt.slots[index as usize];
        slot.value = Some(value);
        index
      },
      None => {
        rt.slots.push(Slot {
          generation: 0,
          value: Some(value),
          subscribers: BTreeSet::new(),
        });
        (rt.slots.len() - 1) as u32
      },
    };
    rt.scopes
      .get_mut(&owner)
      .expect("building scope")
      .owned
      .push(index);
    Signal {
      index,
      generation: rt.slots[index as usize].generation,
      _type: PhantomData,
    }
  })
}

impl<T: 'static> Signal<T> {
  /// Reads the value. While a scope builds, the read subscribes it.
  pub fn try_with<R>(
    self,
    read: impl FnOnce(&T) -> R,
  ) -> Result<R, Disposed> {
    RT.with_borrow_mut(|rt| {
      let rt = &mut *rt;
      let current = rt.current;
      let slot = rt
        .slots
        .get_mut(self.index as usize)
        .ok_or(Disposed)?;
      if slot.generation != self.generation {
        return Err(Disposed);
      }
      if let Some(scope) = current {
        slot.subscribers.insert(scope);
        if let Some(data) = rt.scopes.get_mut(&scope) {
          data.reads.insert(self.index);
        }
      }
      let value = slot
        .value
        .as_ref()
        .ok_or(Disposed)?
        .downcast_ref::<T>()
        .expect("type");
      Ok(read(value))
    })
  }

  /// Reads the value and makes something from it, as in `count.map(...)`.
  ///
  /// # Panics
  ///
  /// If the state's component has been removed.
  pub fn map<R>(
    self,
    read: impl FnOnce(&T) -> R,
  ) -> R {
    self
      .try_with(read)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// A copy of the value.
  pub fn get(self) -> T
  where
    T: Clone,
  {
    self.map(T::clone)
  }

  /// Changes the value and marks every scope that read it.
  pub fn try_update(
    self,
    change: impl FnOnce(&mut T),
  ) -> Result<(), Disposed> {
    RT.with_borrow_mut(|rt| {
      let rt = &mut *rt;
      let slot = rt
        .slots
        .get_mut(self.index as usize)
        .ok_or(Disposed)?;
      if slot.generation != self.generation {
        return Err(Disposed);
      }
      let value = slot
        .value
        .as_mut()
        .ok_or(Disposed)?
        .downcast_mut::<T>()
        .expect("type");
      change(value);
      rt.dirty.extend(slot.subscribers.iter().copied());
      Ok(())
    })
  }

  /// Changes the value, as in `count.update(|v| *v += 1)`.
  ///
  /// # Panics
  ///
  /// If the state's component has been removed.
  pub fn update(
    self,
    change: impl FnOnce(&mut T),
  ) {
    self
      .try_update(change)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// How many scopes are subscribed. For tests.
  pub fn subscriber_count(self) -> usize {
    RT.with_borrow(|rt| {
      rt.slots
        .get(self.index as usize)
        .filter(|s| s.generation == self.generation)
        .map_or(0, |s| s.subscribers.len())
    })
  }
}

/// Removes a scope, its children, their state and their subscriptions.
pub fn dispose(scope: ScopeId) {
  // Values are dropped after the runtime is released, so a value's Drop may
  // use the runtime.
  let mut dropped: Vec<Box<dyn Any>> = Vec::new();
  RT.with_borrow_mut(|rt| {
    let mut stack = vec![scope];
    if let Some(parent) = rt.scopes.get(&scope).and_then(|s| s.parent)
      && let Some(p) = rt.scopes.get_mut(&parent)
    {
      p.children.retain(|c| *c != scope);
    }
    while let Some(id) = stack.pop() {
      let Some(data) = rt.scopes.remove(&id) else {
        continue;
      };
      stack.extend(data.children);
      for index in data.reads {
        rt.slots[index as usize].subscribers.remove(&id);
      }
      for index in data.owned {
        let slot = &mut rt.slots[index as usize];
        slot.generation += 1;
        slot.subscribers.clear();
        dropped.extend(slot.value.take());
        rt.free.push(index);
      }
      rt.dirty.remove(&id);
    }
  });
  drop(dropped);
}

/// The scopes marked since the last call, in creation order.
pub fn take_dirty() -> Vec<ScopeId> {
  RT.with_borrow_mut(|rt| {
    std::mem::take(&mut rt.dirty)
      .into_iter()
      .collect()
  })
}

/// How many scopes and live state values the runtime holds. For tests.
pub fn live_counts() -> (usize, usize) {
  RT.with_borrow(|rt| {
    (
      rt.scopes.len(),
      rt.slots
        .iter()
        .filter(|s| s.value.is_some())
        .count(),
    )
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::drop_counter::Tracked;
  use std::cell::Cell;
  use std::rc::Rc;

  /// The `ui!` example from docs/architecture/syntax.md, by hand, exactly
  /// as written: `count` is used after being moved into the closure.
  #[test]
  fn counter_example_compiles_as_written() {
    let root = create_scope(None);
    let (text, on_click) = build(root, || {
      let count = use_state(|| 0);
      let on_click = move |_: ()| count.update(|value| *value += 1);
      let text = count.map(|value| format!("Count: {value}"));
      (text, on_click)
    });
    assert_eq!(text, "Count: 0");
    on_click(());
    on_click(());
    assert_eq!(take_dirty(), vec![root]);
    let text = build(root, || "rebuilt").to_owned();
    assert_eq!(text, "rebuilt");
    dispose(root);
  }

  #[test]
  fn update_marks_only_scopes_that_read() {
    let root = create_scope(None);
    let a = create_scope(Some(root));
    let b = create_scope(Some(root));
    let state_a = build(a, || {
      let s = use_state(|| 1);
      s.get();
      s
    });
    build(b, || {
      let s = use_state(|| 2);
      s.get();
    });
    state_a.update(|v| *v += 1);
    assert_eq!(take_dirty(), vec![a]);
    dispose(root);
  }

  #[test]
  fn rebuild_forgets_old_reads() {
    let a = create_scope(None);
    let s = build(a, || {
      let s = use_state(|| 0);
      s.get();
      s
    });
    build(a, || {}); // No longer reads `s`.
    s.update(|v| *v += 1);
    assert!(take_dirty().is_empty());
    dispose(a);
  }

  #[test]
  fn dispose_drops_state_and_subscriptions() {
    let drops = Rc::new(Cell::new(0));
    let parent = create_scope(None);
    let child = create_scope(Some(parent));
    let shared = build(parent, || use_state(|| 0));
    build(child, || {
      use_state(|| Tracked(drops.clone()));
      shared.get();
    });
    assert_eq!(shared.subscriber_count(), 1);
    dispose(child);
    assert_eq!(drops.get(), 1);
    assert_eq!(shared.subscriber_count(), 0);
    dispose(parent);
    assert_eq!(live_counts(), (0, 0));
  }

  #[test]
  fn stale_handle_gives_clear_error_and_never_reads_new_state() {
    let a = create_scope(None);
    let old = build(a, || use_state(|| String::from("old")));
    dispose(a);
    assert_eq!(old.try_with(|s| s.clone()), Err(Disposed));

    // The slot is reused by new state; the old handle must not see it.
    let b = create_scope(None);
    let new = build(b, || use_state(|| String::from("new")));
    assert_eq!(new.index, old.index);
    assert_eq!(old.try_with(|s| s.clone()), Err(Disposed));
    assert_eq!(new.get(), "new");
    dispose(b);
  }

  #[test]
  #[should_panic(expected = "component that has been removed")]
  fn stale_handle_panics_with_clear_message() {
    let a = create_scope(None);
    let s = build(a, || use_state(|| 0));
    dispose(a);
    s.update(|v| *v += 1);
  }

  /// State that holds a closure capturing its own handle. With arena
  /// handles there is no reference cycle, so removal still drops it.
  #[test]
  fn self_referencing_state_is_still_dropped() {
    let drops = Rc::new(Cell::new(0));
    let a = create_scope(None);
    build(a, || {
      let s: Signal<Option<Box<dyn Fn()>>> = use_state(|| None);
      let tracked = Tracked(drops.clone());
      s.update(move |slot| {
        *slot = Some(Box::new(move || {
          let _keep = &tracked;
          s.subscriber_count();
        }))
      });
    });
    dispose(a);
    assert_eq!(drops.get(), 1);
    assert_eq!(live_counts(), (0, 0));
  }
}
