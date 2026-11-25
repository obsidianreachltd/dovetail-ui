//! Drag and drop (M10, decision D0016).
//!
//! A pointer handler starts a drag with [`start_drag`], usually once the
//! pointer has moved a few pixels with the button held. From then on the
//! window sends [`DropEvent`]s to the elements with an
//! [`Element::on_drop`](crate::Element::on_drop) handler under the pointer,
//! in this window or in another Dovetail window, and the source element
//! gets no more pointer events. When the button is let go, the target
//! under the pointer gets the drop, and the source is told how the drag
//! ended.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::view::Callback;

/// What is being dragged: a value of any type, such as a panel's id.
/// Targets look inside with [`DragData::get`]. Two are equal only if they
/// are the same value.
#[derive(Clone)]
pub struct DragData(Rc<dyn Any>);

impl DragData {
  /// Wraps `value`.
  pub fn new<T: 'static>(value: T) -> DragData {
    DragData(Rc::new(value))
  }

  /// The value, if it is a `T`.
  pub fn get<T: 'static>(&self) -> Option<&T> {
    self.0.downcast_ref()
  }
}

impl PartialEq for DragData {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl fmt::Debug for DragData {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("DragData")
  }
}

/// How a drag ended, as its source is told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragEnd {
  /// A target took the drop.
  Dropped,
  /// The drag was given up with Escape, or let go over a window but not
  /// over anything that takes it.
  Cancelled,
  /// Let go outside every Dovetail window. `screen` is the point on the
  /// screen, in logical pixels, when the system says where windows are
  /// (Wayland does not).
  Outside {
    /// Where on the screen it was let go.
    screen: Option<(f32, f32)>,
  },
}

/// A drag that has been started and not yet picked up by the window.
pub struct StartedDrag {
  /// What is dragged.
  pub data: DragData,
  /// Told how the drag ended.
  pub on_end: Callback<DragEnd>,
}

thread_local! {
  static STARTED: RefCell<Option<StartedDrag>> = const { RefCell::new(None) };
}

/// Starts dragging `data`. Call it from a pointer handler while the button
/// is held. `on_end` is told how the drag ends.
pub fn start_drag(
  data: DragData,
  on_end: Callback<DragEnd>,
) {
  STARTED.with_borrow_mut(|s| *s = Some(StartedDrag { data, on_end }));
}

/// The drag started since the last call, if any. The window calls this
/// after each pointer event.
pub fn take_started_drag() -> Option<StartedDrag> {
  STARTED.with_borrow_mut(Option::take)
}

/// What a drag did over a drop target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropKind {
  /// The drag is over the target, at a new point. Return true if the
  /// target would take a drop there.
  Over,
  /// The drag left the target, or ended somewhere else.
  Leave,
  /// The drag was let go over the target. Return true if it was taken.
  Drop,
}

/// A drag over a drop target. The point is relative to the target's
/// top-left corner, and the target's size is given, so it can tell which
/// part of it the pointer is over.
#[derive(Clone, Debug, PartialEq)]
pub struct DropEvent {
  /// What happened.
  pub kind: DropKind,
  /// The pointer, across from the target's left edge, in logical pixels.
  pub x: f32,
  /// The pointer, down from the target's top edge, in logical pixels.
  pub y: f32,
  /// The target's width, in logical pixels.
  pub width: f32,
  /// The target's height, in logical pixels.
  pub height: f32,
  /// What is dragged.
  pub data: DragData,
}

/// Receives drags over an element. For [`DropKind::Over`] and
/// [`DropKind::Drop`] it returns whether it takes the drag at that point;
/// when it does not, the nearest element around it with a handler is asked
/// next.
pub struct DropHandler(Rc<dyn Fn(&DropEvent) -> bool>);

impl DropHandler {
  /// A handler that runs `handler`.
  pub fn new(handler: impl Fn(&DropEvent) -> bool + 'static) -> Self {
    DropHandler(Rc::new(handler))
  }

  /// Offers `event` to the handler.
  pub fn handle(
    &self,
    event: &DropEvent,
  ) -> bool {
    (self.0)(event)
  }
}

impl Clone for DropHandler {
  fn clone(&self) -> Self {
    DropHandler(Rc::clone(&self.0))
  }
}

impl PartialEq for DropHandler {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl fmt::Debug for DropHandler {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.write_str("DropHandler")
  }
}
