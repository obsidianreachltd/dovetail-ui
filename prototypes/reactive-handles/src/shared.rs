//! Prototype B: shared handles.
//!
//! State lives in `Rc<RefCell<T>>`. A [`State`] is `Clone` but not `Copy`.
//! A closure that is moved somewhere needs its own clone, made before the
//! closure. Handles never dangle: a handle keeps its state alive, even after
//! its component is removed.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::{Rc, Weak};

use crate::ScopeId;

struct Inner<T> {
  value: RefCell<T>,
  subscribers: RefCell<BTreeSet<ScopeId>>,
}

/// A handle to state. `Clone`, not `Copy`.
///
/// The `ui!` example from docs/architecture/syntax.md, written by hand
/// exactly as in the document, does not compile: `count` is moved into the
/// closure and then used again.
///
/// ```compile_fail,E0382
/// use prototype_reactive_handles::shared::*;
/// let root = create_scope(None);
/// build(root, || {
///   let count = use_state(|| 0);
///   let on_click = move |_: ()| count.update(|value| *value += 1);
///   let text = count.map(|value| format!("Count: {value}"));
///   (text, on_click)
/// });
/// ```
///
/// It needs a clone before the closure:
///
/// ```
/// use prototype_reactive_handles::shared::*;
/// let root = create_scope(None);
/// build(root, || {
///   let count = use_state(|| 0);
///   let on_click = {
///     let count = count.clone();
///     move |_: ()| count.update(|value| *value += 1)
///   };
///   let text = count.map(|value| format!("Count: {value}"));
///   (text, on_click)
/// });
/// dispose(root);
/// ```
pub struct State<T> {
  inner: Rc<Inner<T>>,
}

impl<T> Clone for State<T> {
  fn clone(&self) -> Self {
    Self { inner: Rc::clone(&self.inner) }
  }
}

/// What a scope must do to forget that it read some state.
type Unsubscribe = Box<dyn Fn(ScopeId)>;

struct ScopeData {
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
  owned: Vec<Rc<dyn std::any::Any>>,
  reads: Vec<Unsubscribe>,
}

#[derive(Default)]
struct Runtime {
  scopes: RefCell<HashMap<ScopeId, ScopeData>>,
  next_scope: Cell<u64>,
  current: Cell<Option<ScopeId>>,
  dirty: RefCell<BTreeSet<ScopeId>>,
}

thread_local! {
  static RT: Runtime = Runtime::default();
}

/// Makes a scope (a component instance) under `parent`.
pub fn create_scope(parent: Option<ScopeId>) -> ScopeId {
  RT.with(|rt| {
    rt.next_scope.set(rt.next_scope.get() + 1);
    let id = ScopeId(rt.next_scope.get());
    let mut scopes = rt.scopes.borrow_mut();
    scopes.insert(
      id,
      ScopeData {
        parent,
        children: Vec::new(),
        owned: Vec::new(),
        reads: Vec::new(),
      },
    );
    if let Some(p) = parent.and_then(|p| scopes.get_mut(&p)) {
      p.children.push(id);
    }
    id
  })
}

/// Runs `build` as `scope`. Earlier reads by `scope` are forgotten first.
pub fn build<R>(
  scope: ScopeId,
  build: impl FnOnce() -> R,
) -> R {
  let reads = RT.with(|rt| {
    rt.scopes
      .borrow_mut()
      .get_mut(&scope)
      .map(|s| std::mem::take(&mut s.reads))
      .unwrap_or_default()
  });
  for unsubscribe in reads {
    unsubscribe(scope);
  }
  let previous = RT.with(|rt| rt.current.replace(Some(scope)));
  let result = build();
  RT.with(|rt| rt.current.set(previous));
  result
}

/// Makes state owned by the scope that is building.
///
/// # Panics
///
/// If no scope is building.
pub fn use_state<T: 'static>(init: impl FnOnce() -> T) -> State<T> {
  let inner = Rc::new(Inner {
    value: RefCell::new(init()),
    subscribers: RefCell::default(),
  });
  RT.with(|rt| {
    let owner = rt
      .current
      .get()
      .expect("use_state must be called while a component builds");
    let owned: Rc<dyn std::any::Any> = inner.clone();
    rt.scopes
      .borrow_mut()
      .get_mut(&owner)
      .expect("building scope")
      .owned
      .push(owned);
  });
  State { inner }
}

impl<T: 'static> State<T> {
  /// Reads the value and makes something from it, as in `count.map(...)`.
  /// While a scope builds, the read subscribes it.
  pub fn map<R>(
    &self,
    read: impl FnOnce(&T) -> R,
  ) -> R {
    if let Some(scope) = RT.with(|rt| rt.current.get())
      && self.inner.subscribers.borrow_mut().insert(scope)
    {
      let weak: Weak<Inner<T>> = Rc::downgrade(&self.inner);
      let unsubscribe: Unsubscribe = Box::new(move |scope| {
        if let Some(inner) = weak.upgrade() {
          inner.subscribers.borrow_mut().remove(&scope);
        }
      });
      RT.with(|rt| {
        if let Some(data) = rt.scopes.borrow_mut().get_mut(&scope) {
          data.reads.push(unsubscribe);
        }
      });
    }
    read(&self.inner.value.borrow())
  }

  /// A copy of the value.
  pub fn get(&self) -> T
  where
    T: Clone,
  {
    self.map(T::clone)
  }

  /// Changes the value and marks every scope that read it.
  pub fn update(
    &self,
    change: impl FnOnce(&mut T),
  ) {
    change(&mut self.inner.value.borrow_mut());
    let subscribers = self.inner.subscribers.borrow().clone();
    RT.with(|rt| rt.dirty.borrow_mut().extend(subscribers));
  }

  /// How many scopes are subscribed. For tests.
  pub fn subscriber_count(&self) -> usize {
    self.inner.subscribers.borrow().len()
  }

  /// How many handles and owners keep the state alive. For tests.
  pub fn strong_count(&self) -> usize {
    Rc::strong_count(&self.inner)
  }
}

/// Removes a scope, its children, their state and their subscriptions.
/// State that a handle still points to stays alive until the handle drops.
pub fn dispose(scope: ScopeId) {
  let mut removed = Vec::new();
  RT.with(|rt| {
    let mut scopes = rt.scopes.borrow_mut();
    if let Some(parent) = scopes.get(&scope).and_then(|s| s.parent)
      && let Some(p) = scopes.get_mut(&parent)
    {
      p.children.retain(|c| *c != scope);
    }
    let mut stack = vec![scope];
    while let Some(id) = stack.pop() {
      if let Some(data) = scopes.remove(&id) {
        stack.extend(data.children.iter().copied());
        rt.dirty.borrow_mut().remove(&id);
        removed.push((id, data));
      }
    }
  });
  // Unsubscribing and dropping happen outside the runtime borrow, because
  // a value's Drop may use the runtime.
  for (id, data) in removed {
    for unsubscribe in &data.reads {
      unsubscribe(id);
    }
    drop(data);
  }
}

/// The scopes marked since the last call, in creation order.
pub fn take_dirty() -> Vec<ScopeId> {
  RT.with(|rt| {
    std::mem::take(&mut *rt.dirty.borrow_mut())
      .into_iter()
      .collect()
  })
}

/// How many scopes the runtime holds. For tests.
pub fn scope_count() -> usize {
  RT.with(|rt| rt.scopes.borrow().len())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::drop_counter::Tracked;

  /// The `ui!` example from docs/architecture/syntax.md, by hand, with the
  /// clone it needs. The doc tests on [`State`] show that the example as
  /// written does not compile.
  #[test]
  fn counter_example_needs_a_clone() {
    let root = create_scope(None);
    let (text, on_click) = build(root, || {
      let count = use_state(|| 0);
      let on_click = {
        let count = count.clone();
        move |_: ()| count.update(|value| *value += 1)
      };
      let text = count.map(|value| format!("Count: {value}"));
      (text, on_click)
    });
    assert_eq!(text, "Count: 0");
    on_click(());
    on_click(());
    assert_eq!(take_dirty(), vec![root]);
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
    build(a, || {});
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
    assert_eq!(scope_count(), 0);
    assert_eq!(shared.strong_count(), 1, "only this handle is left");
  }

  /// A handle kept after its component is removed still works. It reads
  /// and changes state that nothing displays any more, with no error.
  #[test]
  fn stale_handle_keeps_state_alive_silently() {
    let drops = Rc::new(Cell::new(0));
    let a = create_scope(None);
    let old = build(a, || use_state(|| Tracked(drops.clone())));
    dispose(a);
    assert_eq!(drops.get(), 0, "the value is still alive");
    old.update(|_| {});
    assert!(take_dirty().is_empty());
    drop(old);
    assert_eq!(drops.get(), 1);
  }

  /// State holding a closure that captures its own handle forms a
  /// reference cycle. Removing the component does not drop it: it leaks.
  #[test]
  fn self_referencing_state_leaks() {
    let drops = Rc::new(Cell::new(0));
    let a = create_scope(None);
    let weak = build(a, || {
      let s: State<Option<Box<dyn Fn()>>> = use_state(|| None);
      let tracked = Tracked(drops.clone());
      let inner = s.clone();
      s.update(move |slot| {
        *slot = Some(Box::new(move || {
          let _keep = &tracked;
          inner.subscriber_count();
        }))
      });
      Rc::downgrade(&s.inner)
    });
    dispose(a);
    assert_eq!(drops.get(), 0, "leaked: the cycle keeps it alive");
    assert!(weak.upgrade().is_some());
  }
}
