//! Views: what a component's build returns.

use std::any::{Any, TypeId};
use std::fmt;
use std::rc::Rc;

use crate::element::Element;
use crate::tree::BuildContext;

/// A reusable part of a UI.
///
/// A component has typed properties, its [`Component::Props`], and builds a
/// [`View`] from them. It keeps state between builds with
/// [`BuildContext::use_state`]. Built-in and third-party components use this
/// same trait.
pub trait Component: 'static {
  /// The component's inputs. Children are passed as a property of type
  /// [`Children`]. A component is built again only when its properties
  /// change, or when state it read changes.
  type Props: PartialEq + 'static;

  /// Builds the component's view.
  fn build(
    props: &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View;
}

/// The views a component receives as children.
pub type Children = Vec<View>;

/// Tells items in a list apart, so each keeps its component instance and
/// state when the list changes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key(KeyValue);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum KeyValue {
  Number(u64),
  Text(Rc<str>),
}

impl Key {
  /// The number the key was made from, if it was made from one (M16).
  pub fn number(&self) -> Option<u64> {
    match &self.0 {
      KeyValue::Number(n) => Some(*n),
      KeyValue::Text(_) => None,
    }
  }

  /// The text the key was made from, if it was made from text (M16).
  pub fn text(&self) -> Option<&str> {
    match &self.0 {
      KeyValue::Number(_) => None,
      KeyValue::Text(t) => Some(t),
    }
  }
}

impl From<u64> for Key {
  fn from(value: u64) -> Self {
    Key(KeyValue::Number(value))
  }
}

impl From<u32> for Key {
  fn from(value: u32) -> Self {
    Key(KeyValue::Number(value.into()))
  }
}

impl From<usize> for Key {
  fn from(value: usize) -> Self {
    Key(KeyValue::Number(value as u64))
  }
}

impl From<&str> for Key {
  fn from(value: &str) -> Self {
    Key(KeyValue::Text(value.into()))
  }
}

impl From<String> for Key {
  fn from(value: String) -> Self {
    Key(KeyValue::Text(value.into()))
  }
}

/// A description of part of a UI. Cheap to clone.
#[derive(Clone, PartialEq, Debug)]
pub enum View {
  /// Nothing.
  Empty,
  /// Plain text.
  Text(Rc<str>),
  /// An image or icon (M17), placed as text is.
  Picture(crate::Picture),
  /// A box that layout places and painting draws.
  Element(Rc<Element>),
  /// A component with its properties.
  Component(ComponentView),
  /// Several views in order, for example a list.
  Fragment(Rc<[View]>),
}

impl View {
  /// A text view.
  pub fn text(text: impl Into<Rc<str>>) -> View {
    View::Text(text.into())
  }

  /// A component view with its properties.
  pub fn component<C: Component>(props: C::Props) -> View {
    View::Component(ComponentView {
      type_id: TypeId::of::<C>(),
      type_name: std::any::type_name::<C>(),
      key: None,
      props: Rc::new(props),
      build: build_erased::<C>,
      props_eq: props_eq_erased::<C>,
    })
  }

  /// Several views in order.
  pub fn fragment(views: impl IntoIterator<Item = View>) -> View {
    View::Fragment(views.into_iter().collect())
  }

  /// Gives a component view a key. A key has no effect on other views.
  pub fn keyed(
    self,
    key: impl Into<Key>,
  ) -> View {
    match self {
      View::Component(mut c) => {
        c.key = Some(key.into());
        View::Component(c)
      },
      other => other,
    }
  }

  /// Shows a view only when `condition` is true.
  pub fn when(
    condition: bool,
    view: impl FnOnce() -> View,
  ) -> View {
    if condition { view() } else { View::Empty }
  }
}

impl From<&str> for View {
  fn from(text: &str) -> Self {
    View::text(text)
  }
}

impl From<String> for View {
  fn from(text: String) -> Self {
    View::text(text)
  }
}

/// A component and its properties, with its type erased.
#[derive(Clone)]
pub struct ComponentView {
  pub(crate) type_id: TypeId,
  type_name: &'static str,
  pub(crate) key: Option<Key>,
  pub(crate) props: Rc<dyn Any>,
  pub(crate) build: fn(&dyn Any, &mut BuildContext<'_>) -> View,
  props_eq: fn(&dyn Any, &dyn Any) -> bool,
}

impl ComponentView {
  /// The component's type name, for messages and debugging.
  pub fn type_name(&self) -> &'static str {
    self.type_name
  }

  /// The component's key, if it has one.
  pub fn key(&self) -> Option<&Key> {
    self.key.as_ref()
  }

  /// The properties, if the component is of type `C`.
  pub fn props<C: Component>(&self) -> Option<&C::Props> {
    self.props.downcast_ref()
  }

  pub(crate) fn same_props(
    &self,
    other: &ComponentView,
  ) -> bool {
    self.type_id == other.type_id
      && (self.props_eq)(&*self.props, &*other.props)
  }
}

impl PartialEq for ComponentView {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.key == other.key && self.same_props(other)
  }
}

impl fmt::Debug for ComponentView {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    let mut d = f.debug_struct(self.type_name);
    if let Some(key) = &self.key {
      d.field("key", key);
    }
    d.finish_non_exhaustive()
  }
}

fn build_erased<C: Component>(
  props: &dyn Any,
  cx: &mut BuildContext<'_>,
) -> View {
  let props = props
    .downcast_ref::<C::Props>()
    .expect("properties match the component type");
  C::build(props, cx)
}

fn props_eq_erased<C: Component>(
  a: &dyn Any,
  b: &dyn Any,
) -> bool {
  match (a.downcast_ref::<C::Props>(), b.downcast_ref::<C::Props>()) {
    (Some(a), Some(b)) => a == b,
    _ => false,
  }
}

/// A typed event handler. A child component calls it to send a value to
/// the component that gave it.
///
/// Two callbacks are equal only if they are the same handler, so a parent
/// that makes a new handler in each build also builds its child again.
pub struct Callback<T>(Rc<dyn Fn(T)>);

impl<T> Callback<T> {
  /// A callback that runs `handler`.
  pub fn new(handler: impl Fn(T) + 'static) -> Self {
    Callback(Rc::new(handler))
  }

  /// Sends `value` to the handler.
  pub fn call(
    &self,
    value: T,
  ) {
    (self.0)(value);
  }
}

impl<T> Clone for Callback<T> {
  fn clone(&self) -> Self {
    Callback(Rc::clone(&self.0))
  }
}

impl<T> PartialEq for Callback<T> {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl<T> fmt::Debug for Callback<T> {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("Callback")
  }
}

impl<T, F: Fn(T) + 'static> From<F> for Callback<T> {
  fn from(handler: F) -> Self {
    Callback::new(handler)
  }
}
