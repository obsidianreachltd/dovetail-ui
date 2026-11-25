//! Drag and drop (M10, D0016), with no window (M20, D0027).
//!
//! A pointer handler starts a drag with `dovetail_core::start_drag`. The
//! window picks it up with [`Drag::take_started`], and from then on tells
//! the drag where the pointer is. The drag offers itself to the drop
//! targets under the pointer, from the nearest outwards, until one would
//! take it, and tells the one it left. When the button is let go, the
//! target under the pointer gets the drop, and the source is told how the
//! drag ended.
//!
//! A drag can cross windows: `W` names a window, and each window has its
//! own [`Session`]. A test with one window uses `()`.

use dovetail_core::{
  Callback, DragData, DragEnd, DropEvent, DropKind, take_started_drag,
};
use dovetail_layout::BoxKey;

use crate::Session;

/// A drag in progress.
pub struct Drag<W> {
  data: DragData,
  on_end: Callback<DragEnd>,
  /// The drop target under the pointer that takes the drag, its window,
  /// and the pointer in that window.
  over: Option<(W, BoxKey, (f32, f32))>,
  /// The pointer is over a window.
  inside: bool,
  /// The pointer on the screen, in logical pixels, when known.
  screen: Option<(f32, f32)>,
}

/// Offers a drag event to drop target `key` in `session`, at `point` in
/// the window. True when it takes it.
fn offer(
  session: &Session,
  key: &BoxKey,
  kind: DropKind,
  point: (f32, f32),
  data: &DragData,
) -> bool {
  let layout = session.layout();
  let (Some(element), Some(rect)) = (layout.element(key), layout.rect_of(key))
  else {
    return false;
  };
  let Some(handler) = &element.on_drop else {
    return false;
  };
  handler.handle(&DropEvent {
    kind,
    x: point.0 - rect.x,
    y: point.1 - rect.y,
    width: rect.width,
    height: rect.height,
    data: data.clone(),
  })
}

impl<W: Copy + PartialEq> Drag<W> {
  /// The drag a pointer handler just started, if one did. The window
  /// should then forget the press it came from
  /// ([`Session::cancel_press`]), and tell the drag where the pointer is.
  pub fn take_started() -> Option<Self> {
    let started = take_started_drag()?;
    Some(Self {
      data: started.data,
      on_end: started.on_end,
      over: None,
      inside: true,
      screen: None,
    })
  }

  /// What is being dragged.
  pub fn data(&self) -> &DragData {
    &self.data
  }

  /// The pointer moved. `at` is the window it is over, that window's
  /// session and the point in it, or `None` outside every window.
  /// `screen` is the point on the screen, in logical pixels, when known.
  /// `session_of` finds a window's session, to tell a target the drag
  /// left.
  pub fn moved<'a>(
    &mut self,
    at: Option<(W, &Session, (f32, f32))>,
    screen: Option<(f32, f32)>,
    session_of: impl Fn(W) -> Option<&'a Session>,
  ) {
    let mut over = None;
    if let Some((w, session, point)) = at {
      for key in session.layout().drop_targets_at(point.0, point.1) {
        if offer(session, &key, DropKind::Over, point, &self.data) {
          over = Some((w, key, point));
          break;
        }
      }
    }
    self.inside = at.is_some();
    self.screen = screen;
    let old = std::mem::replace(&mut self.over, over);
    if let Some(old) = old
      && !self
        .over
        .as_ref()
        .is_some_and(|o| o.0 == old.0 && o.1 == old.1)
    {
      self.leave(&old, &session_of);
    }
  }

  fn leave<'a>(
    &self,
    (w, key, point): &(W, BoxKey, (f32, f32)),
    session_of: &impl Fn(W) -> Option<&'a Session>,
  ) {
    if let Some(session) = session_of(*w) {
      offer(session, key, DropKind::Leave, *point, &self.data);
    }
  }

  /// The button was let go where the pointer last moved to: the target
  /// under it gets the drop, and the source is told how the drag ended,
  /// which is returned too.
  pub fn release<'a>(
    self,
    session_of: impl Fn(W) -> Option<&'a Session>,
  ) -> DragEnd {
    let end = match &self.over {
      Some(over) => {
        let taken = session_of(over.0).is_some_and(|session| {
          offer(session, &over.1, DropKind::Drop, over.2, &self.data)
        });
        if taken {
          DragEnd::Dropped
        } else {
          self.leave(over, &session_of);
          DragEnd::Cancelled
        }
      },
      None if self.inside => DragEnd::Cancelled,
      None => DragEnd::Outside { screen: self.screen },
    };
    self.on_end.call(end);
    end
  }

  /// Gives the drag up, as Escape does: the target under the pointer is
  /// told it left, and the source that the drag was cancelled.
  pub fn cancel<'a>(
    self,
    session_of: impl Fn(W) -> Option<&'a Session>,
  ) {
    if let Some(over) = &self.over {
      self.leave(over, &session_of);
    }
    self.on_end.call(DragEnd::Cancelled);
  }

  /// Window `w` closed: the drag is no longer over anything in it.
  pub fn forget_window(
    &mut self,
    w: W,
  ) {
    if self.over.as_ref().is_some_and(|o| o.0 == w) {
      self.over = None;
    }
  }
}
