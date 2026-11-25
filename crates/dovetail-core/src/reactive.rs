//! Reactive state with `Copy` arena handles (decision D0010).
//!
//! All state lives in a runtime that belongs to the UI thread. A
//! [`Signal`] is a slot number and a generation. Reading a signal while a
//! component builds subscribes that component, and changing it marks the
//! subscribers to be built again.
//!
//! The runtime is borrowed only while it is changed, never while user code
//! runs, so code inside [`Signal::map`] or [`Signal::update`] may use other
//! signals.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// Identifies one component instance. An identity is never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(u64);

/// Why a signal could not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
  /// The component that owned the state has been removed.
  Disposed,
  /// The state is being changed by code that is still running, for example
  /// a signal read inside its own [`Signal::update`].
  InUse,
}

impl fmt::Display for StateError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      StateError::Disposed => {
        f.write_str("this state belonged to a component that has been removed")
      },
      StateError::InUse => f.write_str(
        "this state is already being changed; it cannot be used inside its own update",
      ),
    }
  }
}

impl std::error::Error for StateError {}

struct Slot {
  generation: u32,
  value: Option<Rc<dyn Any>>,
  subscribers: BTreeSet<ComponentId>,
}

#[derive(Default)]
struct ScopeData {
  owned: Vec<u32>,
  reads: BTreeSet<u32>,
}

#[derive(Default)]
struct Runtime {
  slots: Vec<Slot>,
  free: Vec<u32>,
  scopes: HashMap<ComponentId, ScopeData>,
  next_id: u64,
  current: Option<ComponentId>,
  dirty: BTreeSet<ComponentId>,
}

thread_local! {
  static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
  static ON_CHANGE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Sets what happens when a state change marks a component, on this
/// thread. A window uses it to ask for a redraw, so `Tree::update` runs.
/// `None` removes the hook.
pub fn set_on_change(hook: Option<Box<dyn Fn()>>) {
  ON_CHANGE.with_borrow_mut(|h| *h = hook);
}

/// Runs the state-change hook, so windows redraw. Also used by requests
/// that are not state, such as moving focus.
/// The component that is building, as the reactive runtime sees it.
pub(crate) fn current() -> Option<ComponentId> {
  RT.with_borrow(|rt| rt.current)
}

/// Marks components to be built again, and tells the windows.
pub(crate) fn mark(ids: impl IntoIterator<Item = ComponentId>) {
  let before = RT.with_borrow(|rt| rt.dirty.len());
  let after = RT.with_borrow_mut(|rt| {
    rt.dirty.extend(ids);
    rt.dirty.len()
  });
  if after != before {
    notify_change();
  }
}

pub(crate) fn notify_change() {
  ON_CHANGE.with_borrow(|hook| {
    if let Some(hook) = hook {
      hook();
    }
  });
}

/// Counts of what the runtime holds on this thread. Used to check that
/// nothing is left behind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeStats {
  /// Component instances that own state.
  pub components: usize,
  /// State values that are alive.
  pub values: usize,
  /// Subscriptions from components to state.
  pub subscriptions: usize,
}

/// What the runtime holds on this thread.
pub fn runtime_stats() -> RuntimeStats {
  RT.with_borrow(|rt| RuntimeStats {
    components: rt.scopes.len(),
    values: rt
      .slots
      .iter()
      .filter(|s| s.value.is_some())
      .count(),
    subscriptions: rt.slots.iter().map(|s| s.subscribers.len()).sum(),
  })
}

pub(crate) fn new_component() -> ComponentId {
  RT.with_borrow_mut(|rt| {
    rt.next_id += 1;
    let id = ComponentId(rt.next_id);
    rt.scopes.insert(id, ScopeData::default());
    id
  })
}

/// Runs `build` as component `id`. Its earlier reads are forgotten first,
/// so it subscribes only to what this build reads.
pub(crate) fn building<R>(
  id: ComponentId,
  build: impl FnOnce() -> R,
) -> R {
  let previous = RT.with_borrow_mut(|rt| {
    let rt = &mut *rt;
    let reads = rt
      .scopes
      .get_mut(&id)
      .map(|s| std::mem::take(&mut s.reads))
      .unwrap_or_default();
    for index in reads {
      rt.slots[index as usize].subscribers.remove(&id);
    }
    rt.current.replace(id)
  });
  // Restore the previous component even if `build` panics.
  struct Restore(Option<ComponentId>);
  impl Drop for Restore {
    fn drop(&mut self) {
      let previous = self.0;
      RT.with_borrow_mut(|rt| rt.current = previous);
    }
  }
  let _restore = Restore(previous);
  build()
}

/// Removes component `id`, its state and its subscriptions. Values are
/// dropped after the runtime is released, so their `Drop` may use it.
pub(crate) fn dispose(id: ComponentId) {
  let dropped: Vec<Rc<dyn Any>> = RT.with_borrow_mut(|rt| {
    let rt = &mut *rt;
    let mut dropped = Vec::new();
    if let Some(data) = rt.scopes.remove(&id) {
      for index in data.reads {
        rt.slots[index as usize].subscribers.remove(&id);
      }
      for index in data.owned {
        let slot = &mut rt.slots[index as usize];
        slot.generation = slot.generation.wrapping_add(1);
        for subscriber in std::mem::take(&mut slot.subscribers) {
          if let Some(s) = rt.scopes.get_mut(&subscriber) {
            s.reads.remove(&index);
          }
        }
        dropped.extend(slot.value.take());
        rt.free.push(index);
      }
    }
    rt.dirty.remove(&id);
    dropped
  });
  drop(dropped);
}

/// The components marked since the last call, in creation order.
#[cfg(test)]
pub(crate) fn take_dirty() -> BTreeSet<ComponentId> {
  RT.with_borrow_mut(|rt| std::mem::take(&mut rt.dirty))
}

/// True when a component for which `mine` is true is marked (M16).
pub(crate) fn any_dirty_where(mine: impl Fn(ComponentId) -> bool) -> bool {
  RT.with_borrow(|rt| rt.dirty.iter().any(|&id| mine(id)))
}

/// Takes the marked components for which `mine` is true, in creation
/// order, and leaves the others marked for the trees that hold them
/// (B0005).
pub(crate) fn take_dirty_where(
  mine: impl Fn(ComponentId) -> bool
) -> BTreeSet<ComponentId> {
  RT.with_borrow_mut(|rt| {
    let (taken, left) = std::mem::take(&mut rt.dirty)
      .into_iter()
      .partition(|&id| mine(id));
    rt.dirty = left;
    taken
  })
}

/// A handle to a piece of reactive state. It is `Copy` whatever `T` is, so
/// closures can capture it with `move` and it can still be used afterwards.
///
/// ```
/// use dovetail_core::{BuildContext, Component, Tree, View};
///
/// struct Counter;
///
/// impl Component for Counter {
///   type Props = ();
///
///   fn build(_: &(), cx: &mut BuildContext<'_>) -> View {
///     let count = cx.use_state(|| 0);
///     let on_click = move |_: ()| count.update(|value| *value += 1);
///     let text = count.map(|value| format!("Count: {value}"));
///     let _ = on_click;
///     View::text(text)
///   }
/// }
///
/// let mut tree = Tree::new();
/// tree.mount(View::component::<Counter>(()));
/// assert_eq!(tree.output(tree.root().unwrap()), Some(&View::text("Count: 0")));
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

impl<T> PartialEq for Signal<T> {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.index == other.index && self.generation == other.generation
  }
}

impl<T> Eq for Signal<T> {}

impl<T> fmt::Debug for Signal<T> {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    write!(f, "Signal({}#{})", self.index, self.generation)
  }
}

/// Makes state owned by the component that is building. Most code uses
/// [`BuildContext::use_state`](crate::BuildContext::use_state) instead, which
/// keeps the same state between builds.
///
/// # Panics
///
/// If no component is building.
pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
  let value: Rc<dyn Any> = Rc::new(RefCell::new(value));
  RT.with_borrow_mut(|rt| {
    let owner = rt
      .current
      .expect("state can only be made while a component builds");
    let index = match rt.free.pop() {
      Some(index) => {
        rt.slots[index as usize].value = Some(value);
        index
      },
      None => {
        rt.slots.push(Slot {
          generation: 0,
          value: Some(value),
          subscribers: BTreeSet::new(),
        });
        u32::try_from(rt.slots.len() - 1).expect("too many state values")
      },
    };
    rt.scopes
      .get_mut(&owner)
      .expect("building component")
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
  /// The value's cell, after checking the handle. When `subscribe` is
  /// true and a component is building, that component subscribes.
  fn cell(
    self,
    subscribe: bool,
  ) -> Result<(Rc<RefCell<T>>, BTreeSet<ComponentId>), StateError> {
    RT.with_borrow_mut(|rt| {
      let rt = &mut *rt;
      let slot = rt
        .slots
        .get_mut(self.index as usize)
        .ok_or(StateError::Disposed)?;
      if slot.generation != self.generation {
        return Err(StateError::Disposed);
      }
      let value = slot.value.clone().ok_or(StateError::Disposed)?;
      if subscribe && let Some(current) = rt.current {
        slot.subscribers.insert(current);
        if let Some(scope) = rt.scopes.get_mut(&current) {
          scope.reads.insert(self.index);
        }
      }
      let subscribers = if subscribe {
        BTreeSet::new()
      } else {
        slot.subscribers.clone()
      };
      let cell = value
        .downcast::<RefCell<T>>()
        .unwrap_or_else(|_| unreachable!("type is fixed by the handle"));
      Ok((cell, subscribers))
    })
  }

  /// Reads the value. While a component builds, the read subscribes it.
  pub fn try_map<R>(
    self,
    read: impl FnOnce(&T) -> R,
  ) -> Result<R, StateError> {
    let (cell, _) = self.cell(true)?;
    let value = cell.try_borrow().map_err(|_| StateError::InUse)?;
    Ok(read(&value))
  }

  /// Reads the value and makes something from it, as in `count.map(...)`.
  ///
  /// # Panics
  ///
  /// With a clear message, if [`Signal::try_map`] fails.
  pub fn map<R>(
    self,
    read: impl FnOnce(&T) -> R,
  ) -> R {
    self
      .try_map(read)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// A copy of the value.
  ///
  /// # Panics
  ///
  /// As [`Signal::map`].
  pub fn get(self) -> T
  where
    T: Clone,
  {
    self.map(T::clone)
  }

  /// Changes the value and marks every component that read it.
  pub fn try_update(
    self,
    change: impl FnOnce(&mut T),
  ) -> Result<(), StateError> {
    let (cell, subscribers) = self.cell(false)?;
    {
      let mut value = cell
        .try_borrow_mut()
        .map_err(|_| StateError::InUse)?;
      change(&mut value);
    }
    let marked = !subscribers.is_empty();
    RT.with_borrow_mut(|rt| rt.dirty.extend(subscribers));
    if marked {
      // Called with no borrow held, so the hook may use the runtime.
      notify_change();
    }
    Ok(())
  }

  /// Changes the value, as in `count.update(|value| *value += 1)`.
  ///
  /// # Panics
  ///
  /// With a clear message, if [`Signal::try_update`] fails.
  pub fn update(
    self,
    change: impl FnOnce(&mut T),
  ) {
    self
      .try_update(change)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// Replaces the value.
  ///
  /// # Panics
  ///
  /// As [`Signal::update`].
  pub fn set(
    self,
    value: T,
  ) {
    self.update(|v| *v = value);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn component() -> ComponentId {
    new_component()
  }

  #[test]
  fn update_marks_only_readers() {
    let (a, b) = (component(), component());
    let sa = building(a, || {
      let s = create_signal(1);
      s.get();
      s
    });
    building(b, || create_signal(2).get());
    sa.update(|v| *v += 1);
    assert_eq!(take_dirty().into_iter().collect::<Vec<_>>(), vec![a]);
    dispose(a);
    dispose(b);
  }

  #[test]
  fn user_code_may_use_other_signals() {
    let a = component();
    let (x, y) = building(a, || (create_signal(1), create_signal(10)));
    x.update(|v| *v += y.get());
    assert_eq!(x.map(|v| *v + y.get()), 21);
    dispose(a);
  }

  #[test]
  fn reading_inside_own_update_is_a_clear_error() {
    let a = component();
    let x = building(a, || create_signal(1));
    let mut inner = None;
    x.update(|_| inner = Some(x.try_map(|v| *v)));
    assert_eq!(inner, Some(Err(StateError::InUse)));
    dispose(a);
  }

  #[test]
  fn stale_handle_never_reads_new_state() {
    let a = component();
    let old = building(a, || create_signal(String::from("old")));
    dispose(a);
    let b = component();
    let new = building(b, || create_signal(String::from("new")));
    assert_eq!(new.index, old.index, "the slot is reused");
    assert_eq!(old.try_map(Clone::clone), Err(StateError::Disposed));
    assert_eq!(old.try_update(|_| {}), Err(StateError::Disposed));
    assert_eq!(new.get(), "new");
    dispose(b);
  }

  #[test]
  #[should_panic(expected = "state can only be made while a component builds")]
  fn state_outside_a_build_panics() {
    create_signal(0);
  }

  #[test]
  fn dispose_removes_subscriptions_both_ways() {
    let (owner, reader) = (component(), component());
    let s = building(owner, || create_signal(0));
    building(reader, || s.get());
    assert_eq!(runtime_stats().subscriptions, 1);
    dispose(owner);
    assert_eq!(
      runtime_stats(),
      RuntimeStats {
        components: 1,
        values: 0,
        subscriptions: 0
      }
    );
    dispose(reader);
    assert_eq!(runtime_stats(), RuntimeStats::default());
  }
}
