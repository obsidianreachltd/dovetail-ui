//! The component tree: identity, building, keyed children, updates and
//! removal.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;

use crate::reactive::{self, ComponentId, Signal};
use crate::view::{Component, ComponentView, Key, View};

/// The state handles of the component that is building, in call order.
struct HookFrame {
  id: ComponentId,
  hooks: Vec<Box<dyn Any>>,
  next: usize,
}

thread_local! {
  /// One frame per component that is building. Builds do not nest today,
  /// but a stack keeps that safe if they ever do.
  static FRAMES: RefCell<Vec<HookFrame>> = const { RefCell::new(Vec::new()) };
}

/// State for the component instance that is building. The first build
/// makes it with `init`; later builds return the same state. This is what
/// `#[component]` functions call (M03).
///
/// # Panics
///
/// - If no component is building.
/// - If a build calls `use_state` in a different order, or with a different
///   type, from the first build. Call it the same way every time, not inside
///   conditions or loops.
pub fn use_state<T: 'static>(init: impl FnOnce() -> T) -> Signal<T> {
  let existing = FRAMES.with_borrow_mut(|frames| {
    let frame = frames.last_mut().expect("use_state can only be called while a component builds");
    let index = frame.next;
    frame.next += 1;
    frame.hooks.get(index).map(|hook| {
      *hook.downcast_ref::<Signal<T>>().unwrap_or_else(|| {
        panic!(
          "use_state number {} in component {:?} returned a different type than in \
           the first build; call use_state in the same order every build",
          index + 1,
          frame.id
        )
      })
    })
  });
  if let Some(signal) = existing {
    return signal;
  }
  // `init` runs with no borrow held, so it may use other state.
  let signal = reactive::create_signal(init());
  FRAMES.with_borrow_mut(|frames| {
    frames
      .last_mut()
      .expect("building component")
      .hooks
      .push(Box::new(signal));
  });
  signal
}

/// What a component provides to the components inside it (M14), and its
/// parent, for every component on this thread.
struct ContextNode {
  parent: Option<ComponentId>,
  values: Vec<(TypeId, Rc<dyn Any>)>,
}

thread_local! {
  static CONTEXT: RefCell<HashMap<ComponentId, ContextNode>> =
    RefCell::new(HashMap::new());
}

/// The component that is building, if any.
pub fn current_component() -> Option<ComponentId> {
  FRAMES.with_borrow(|frames| frames.last().map(|f| f.id))
}

/// Gives `value` to the component that is building and every component
/// inside it, which read it with [`use_context`] (M14, D0019). One value of
/// each type: providing another replaces it. Call it on every build.
///
/// To let readers be built again when the value changes, provide a
/// [`Signal`] and change the signal.
///
/// # Panics
///
/// If no component is building.
pub fn provide_context<T: Clone + 'static>(value: T) {
  let id = current_component()
    .expect("provide_context can only be called while a component builds");
  let value: Rc<dyn Any> = Rc::new(value);
  CONTEXT.with_borrow_mut(|nodes| {
    let node = nodes
      .entry(id)
      .or_insert(ContextNode { parent: None, values: Vec::new() });
    let key = TypeId::of::<T>();
    match node.values.iter_mut().find(|v| v.0 == key) {
      Some(slot) => slot.1 = value,
      None => node.values.push((key, value)),
    }
  });
}

/// The nearest value of type `T` provided by the component that is
/// building or a component around it, or `None` (M14, D0019).
pub fn use_context<T: Clone + 'static>() -> Option<T> {
  context_from(current_component()?)
}

/// The nearest value of type `T` provided by `id` or a component around
/// it. Code outside a build, such as an event handler, can use this with
/// an id it kept.
pub fn context_from<T: Clone + 'static>(id: ComponentId) -> Option<T> {
  let key = TypeId::of::<T>();
  CONTEXT.with_borrow(|nodes| {
    let mut at = Some(id);
    while let Some(id) = at {
      let node = nodes.get(&id)?;
      if let Some((_, v)) = node.values.iter().find(|v| v.0 == key) {
        return v.downcast_ref::<T>().cloned();
      }
      at = node.parent;
    }
    None
  })
}

/// Given to [`Component::build`]. Keeps a component's state between builds.
pub struct BuildContext<'a> {
  id: ComponentId,
  _build: PhantomData<&'a mut ()>,
}

impl BuildContext<'_> {
  /// The identity of the component that is building.
  pub fn id(&self) -> ComponentId {
    self.id
  }

  /// The same as the free function [`use_state`].
  ///
  /// # Panics
  ///
  /// As [`use_state`].
  pub fn use_state<T: 'static>(
    &mut self,
    init: impl FnOnce() -> T,
  ) -> Signal<T> {
    use_state(init)
  }
}

/// Runs `build` with a hook frame for `id`, and gives the hooks back after,
/// even if `build` panics.
fn with_frame<R>(
  id: ComponentId,
  hooks: &mut Vec<Box<dyn Any>>,
  build: impl FnOnce() -> R,
) -> R {
  struct Pop<'h>(&'h mut Vec<Box<dyn Any>>);
  impl Drop for Pop<'_> {
    fn drop(&mut self) {
      if let Some(frame) = FRAMES.with_borrow_mut(Vec::pop) {
        *self.0 = frame.hooks;
      }
    }
  }
  FRAMES.with_borrow_mut(|frames| {
    frames.push(HookFrame {
      id,
      hooks: std::mem::take(hooks),
      next: 0,
    });
  });
  let _pop = Pop(hooks);
  let _ = id;
  build()
}

/// Which work a change to a component's own view needs again.
///
/// - Different text, or views added, removed or of another kind: all three.
/// - A style change that can move or resize something
///   ([`Style::affects_layout`](crate::Style::affects_layout)): layout and paint.
/// - Any other style change, such as a colour: paint only.
/// - Gaining or losing a press handler: accessibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Invalidation {
  /// Sizes and positions must be worked out again.
  pub layout: bool,
  /// The component must be painted again.
  pub paint: bool,
  /// The accessibility tree must be updated.
  pub accessibility: bool,
}

impl Invalidation {
  /// Nothing to redo.
  pub const NONE: Invalidation = Invalidation {
    layout: false,
    paint: false,
    accessibility: false,
  };
  /// Everything must be redone.
  pub const ALL: Invalidation = Invalidation {
    layout: true,
    paint: true,
    accessibility: true,
  };

  /// True when nothing needs to be redone.
  pub fn is_none(self) -> bool {
    self == Invalidation::NONE
  }

  /// Everything either needs.
  pub fn or(
    self,
    other: Invalidation,
  ) -> Invalidation {
    Invalidation {
      layout: self.layout || other.layout,
      paint: self.paint || other.paint,
      accessibility: self.accessibility || other.accessibility,
    }
  }
}

/// What one call to [`Tree::mount`], [`Tree::update`] or [`Tree::unmount`]
/// changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Update {
  /// Components made, in the order they were made.
  pub created: Vec<ComponentId>,
  /// Existing components built again.
  pub rebuilt: Vec<ComponentId>,
  /// Components removed.
  pub removed: Vec<ComponentId>,
  /// Components whose own view changed, and what that change needs.
  pub invalidated: Vec<(ComponentId, Invalidation)>,
}

/// How a child is matched between builds: its component type, then its key
/// or, without a key, its position among unkeyed children of that type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ChildKey {
  Keyed(TypeId, Key),
  Position(TypeId, usize),
}

struct Instance {
  view: ComponentView,
  parent: Option<ComponentId>,
  depth: usize,
  output: View,
  children: Vec<ComponentId>,
  child_keys: Vec<ChildKey>,
  hooks: Vec<Box<dyn Any>>,
}

/// A tree of component instances.
///
/// Each instance has an identity that is never reused. State changes mark
/// the components that read the state, and [`Tree::update`] builds only
/// those again.
#[derive(Default)]
pub struct Tree {
  root: Option<ComponentId>,
  instances: HashMap<ComponentId, Instance>,
  /// The component, in another tree, whose provided values this tree's
  /// root sees.
  outer: Option<ComponentId>,
}

impl Tree {
  /// An empty tree.
  pub fn new() -> Self {
    Self::default()
  }

  /// Builds `root`, which must be a component view, and everything inside
  /// it. A tree that already has a root removes it first.
  ///
  /// # Panics
  ///
  /// If `root` is not a component view.
  pub fn mount(
    &mut self,
    root: View,
  ) -> Update {
    let View::Component(view) = root else {
      panic!("the root of a tree must be a component view");
    };
    let mut update = self.unmount();
    self.root = Some(self.create(view, None, 0, &mut update));
    update
  }

  /// Like [`Tree::mount`], but the components see the values `outer`
  /// provides (see [`provide_context`]), as if the root were inside it.
  /// `outer` is usually in another tree, such as the window that opened
  /// this one.
  ///
  /// # Panics
  ///
  /// As [`Tree::mount`].
  pub fn mount_within(
    &mut self,
    root: View,
    outer: Option<ComponentId>,
  ) -> Update {
    self.outer = outer;
    self.mount(root)
  }

  /// Builds again every component marked by a state change, parents first,
  /// until nothing is marked.
  ///
  /// # Panics
  ///
  /// If builds keep marking components after 100 rounds, which means a
  /// component changes state it reads while it builds.
  pub fn update(&mut self) -> Update {
    let mut update = Update::default();
    for _ in 0..100 {
      // Only this tree's components: another window's tree builds its own
      // (B0005).
      let mut dirty: Vec<ComponentId> =
        reactive::take_dirty_where(|id| self.instances.contains_key(&id))
          .into_iter()
          .collect();
      if dirty.is_empty() {
        return update;
      }
      dirty.sort_by_key(|id| self.instances[id].depth);
      let mut done = BTreeSet::new();
      for id in dirty {
        // A parent built earlier in this round may have removed it.
        if self.instances.contains_key(&id) && done.insert(id) {
          self.rebuild(id, &mut update);
        }
      }
    }
    panic!(
      "components are still changing after 100 update rounds; does a build change state it reads?"
    );
  }

  /// True when a state change has marked a component of this tree, so
  /// [`Tree::update`] would build something (M16).
  pub fn needs_update(&self) -> bool {
    reactive::any_dirty_where(|id| self.instances.contains_key(&id))
  }

  /// Removes every component, with their state and subscriptions.
  pub fn unmount(&mut self) -> Update {
    let mut update = Update::default();
    if let Some(root) = self.root.take() {
      self.remove(root, &mut update);
    }
    update
  }

  /// The root component, once mounted.
  pub fn root(&self) -> Option<ComponentId> {
    self.root
  }

  /// How many component instances the tree holds.
  pub fn len(&self) -> usize {
    self.instances.len()
  }

  /// True when the tree holds no components.
  pub fn is_empty(&self) -> bool {
    self.instances.is_empty()
  }

  /// The view a component built last.
  pub fn output(
    &self,
    id: ComponentId,
  ) -> Option<&View> {
    self.instances.get(&id).map(|i| &i.output)
  }

  /// A component's child components, in order.
  pub fn children(
    &self,
    id: ComponentId,
  ) -> &[ComponentId] {
    self
      .instances
      .get(&id)
      .map_or(&[], |i| &i.children)
  }

  /// A component's parent, if it has one.
  pub fn parent(
    &self,
    id: ComponentId,
  ) -> Option<ComponentId> {
    self.instances.get(&id).and_then(|i| i.parent)
  }

  /// A component's properties, if it is of type `C`.
  pub fn props<C: Component>(
    &self,
    id: ComponentId,
  ) -> Option<&C::Props> {
    self
      .instances
      .get(&id)
      .and_then(|i| i.view.props::<C>())
  }

  fn create(
    &mut self,
    view: ComponentView,
    parent: Option<ComponentId>,
    depth: usize,
    update: &mut Update,
  ) -> ComponentId {
    let id = reactive::new_component();
    update.created.push(id);
    let context_parent = parent.or(self.outer);
    CONTEXT.with_borrow_mut(|nodes| {
      nodes.insert(
        id,
        ContextNode {
          parent: context_parent,
          values: Vec::new(),
        },
      )
    });
    self.instances.insert(
      id,
      Instance {
        view,
        parent,
        depth,
        output: View::Empty,
        children: Vec::new(),
        child_keys: Vec::new(),
        hooks: Vec::new(),
      },
    );
    self.build(id, update);
    id
  }

  fn rebuild(
    &mut self,
    id: ComponentId,
    update: &mut Update,
  ) {
    update.rebuilt.push(id);
    let old_output = self.instances[&id].output.clone();
    self.build(id, update);
    let change = view_change(&old_output, &self.instances[&id].output);
    if !change.is_none() {
      update.invalidated.push((id, change));
    }
  }

  /// Runs the component's build and reconciles its children.
  fn build(
    &mut self,
    id: ComponentId,
    update: &mut Update,
  ) {
    let instance = self.instances.get_mut(&id).expect("instance");
    let view = instance.view.clone();
    let mut hooks = std::mem::take(&mut instance.hooks);
    // A frame asked for before is answered by this build; the build asks
    // again if it still moves.
    crate::frame::forget(id);
    let output = with_frame(id, &mut hooks, || {
      reactive::building(id, || {
        let mut cx = BuildContext { id, _build: PhantomData };
        (view.build)(&*view.props, &mut cx)
      })
    });
    let instance = self.instances.get_mut(&id).expect("instance");
    instance.hooks = hooks;
    let depth = instance.depth;

    let mut wanted: Vec<(ChildKey, ComponentView)> = Vec::new();
    collect_children(
      &output,
      &mut wanted,
      &mut HashMap::new(),
      &mut HashSet::new(),
    );
    instance.output = output;

    let old_keys = std::mem::take(&mut instance.child_keys);
    let old_children = std::mem::take(&mut instance.children);
    let mut old: HashMap<ChildKey, ComponentId> =
      old_keys.into_iter().zip(old_children).collect();

    let mut children = Vec::with_capacity(wanted.len());
    let mut keys = Vec::with_capacity(wanted.len());
    for (key, view) in wanted {
      let child = match old.remove(&key) {
        Some(child) => {
          let instance = self
            .instances
            .get_mut(&child)
            .expect("child instance");
          if !instance.view.same_props(&view) {
            instance.view = view;
            self.rebuild(child, update);
          }
          child
        },
        None => self.create(view, Some(id), depth + 1, update),
      };
      children.push(child);
      keys.push(key);
    }
    for (_, child) in old {
      self.remove(child, update);
    }
    let instance = self.instances.get_mut(&id).expect("instance");
    instance.children = children;
    instance.child_keys = keys;
  }

  fn remove(
    &mut self,
    id: ComponentId,
    update: &mut Update,
  ) {
    let Some(instance) = self.instances.remove(&id) else {
      return;
    };
    for child in &instance.children {
      self.remove(*child, update);
    }
    reactive::dispose(id);
    crate::frame::forget(id);
    CONTEXT.with_borrow_mut(|nodes| nodes.remove(&id));
    update.removed.push(id);
    // The instance, with its properties and handlers, drops here, after its
    // state.
    drop(instance);
  }
}

impl Drop for Tree {
  fn drop(&mut self) {
    self.unmount();
  }
}

/// What a change from `old` to `new` in one component's own view needs.
/// Child components are not looked into: each reports its own change.
fn view_change(
  old: &View,
  new: &View,
) -> Invalidation {
  match (old, new) {
    (View::Empty, View::Empty) => Invalidation::NONE,
    (View::Text(a), View::Text(b)) if a == b => Invalidation::NONE,
    (View::Picture(a), View::Picture(b)) if a == b => Invalidation::NONE,
    (View::Component(a), View::Component(b))
      if a.type_id == b.type_id && a.key == b.key =>
    {
      Invalidation::NONE
    },
    (View::Fragment(a), View::Fragment(b)) if a.len() == b.len() => {
      children_change(a, b)
    },
    (View::Element(a), View::Element(b))
      if a.kind == b.kind && a.children.len() == b.children.len() =>
    {
      let mut change = children_change(&a.children, &b.children);
      if a.style.affects_layout(&b.style) {
        change = change.or(Invalidation {
          layout: true,
          paint: true,
          accessibility: false,
        });
      } else if a.style != b.style || a.text_cursor != b.text_cursor {
        change = change.or(Invalidation { paint: true, ..Invalidation::NONE });
      }
      // A handler can be a new closure in every build; only gaining or
      // losing one changes what assistive tools can do with the element.
      if a.on_press.is_some() != b.on_press.is_some()
        || a.focusable != b.focusable
        || a.on_key.is_some() != b.on_key.is_some()
        || a.text_cursor.is_some() != b.text_cursor.is_some()
        || a.on_ime.is_some() != b.on_ime.is_some()
        || a.role != b.role
        || a.disabled != b.disabled
        || a.label != b.label
        || a.toggled != b.toggled
        || a.on_access.is_some() != b.on_access.is_some()
      {
        change = change.or(Invalidation {
          accessibility: true,
          ..Invalidation::NONE
        });
      }
      change
    },
    // Different text, a different kind of view, or children added or
    // removed: everything changes.
    _ => Invalidation::ALL,
  }
}

fn children_change(
  old: &[View],
  new: &[View],
) -> Invalidation {
  old
    .iter()
    .zip(new)
    .fold(Invalidation::NONE, |change, (a, b)| {
      change.or(view_change(a, b))
    })
}

/// Collects the component views in `view`, in order, with their matching
/// keys. Fragments are looked into; components are not, because their
/// contents belong to them.
fn collect_children(
  view: &View,
  out: &mut Vec<(ChildKey, ComponentView)>,
  positions: &mut HashMap<TypeId, usize>,
  seen: &mut HashSet<ChildKey>,
) {
  match view {
    View::Empty | View::Text(_) | View::Picture(_) => {},
    View::Fragment(views) => {
      for view in views.iter() {
        collect_children(view, out, positions, seen);
      }
    },
    View::Element(element) => {
      for view in element.children.iter() {
        collect_children(view, out, positions, seen);
      }
    },
    View::Component(c) => {
      let key = match &c.key {
        Some(key) => ChildKey::Keyed(c.type_id, key.clone()),
        None => {
          let position = positions.entry(c.type_id).or_default();
          *position += 1;
          ChildKey::Position(c.type_id, *position - 1)
        },
      };
      if !seen.insert(key.clone()) {
        panic!(
          "two children of type {} have the same key {:?}",
          c.type_name(),
          c.key
        );
      }
      out.push((key, c.clone()));
    },
  }
}
