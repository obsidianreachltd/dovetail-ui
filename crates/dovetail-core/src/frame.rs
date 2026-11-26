//! Frames for animation (M18, decision D0028). A component that moves
//! asks, while it builds, to be built again at the next frame. The window
//! draws frames at the display's rate while any are asked for, and none
//! when nothing is, and starts each by building the components that
//! asked. An animation works out where it is from [`now`], not by counting
//! frames, so it ends on time at any rate.
//!
//! [`now`]: crate::now

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::reactive::{self, ComponentId};

thread_local! {
  /// The components waiting for the next frame.
  static WAITING: RefCell<BTreeSet<ComponentId>> =
    const { RefCell::new(BTreeSet::new()) };
}

/// Asks for the component that is building to be built again at the next
/// frame. A component that moves asks on each build until it stops.
///
/// Outside a build it does nothing: there is no component to build again.
pub fn request_frame() {
  // Builds happen while the window handles events, and it looks for
  // frame requests before it waits again, so it needs no waking.
  if let Some(id) = reactive::current() {
    WAITING.with_borrow_mut(|w| w.insert(id));
  }
}

/// Drops a component's request: it is building now, or has gone.
pub(crate) fn forget(id: ComponentId) {
  WAITING.with_borrow_mut(|w| w.remove(&id));
}

/// True when a component is waiting for the next frame.
pub fn frame_requested() -> bool {
  WAITING.with_borrow(|w| !w.is_empty())
}

/// Starts a frame: the components that asked for it are marked to be
/// built again, and must ask again for the frame after. Returns how many
/// there were. The window calls it at the start of each frame, and
/// headless tests as their clock moves.
pub fn start_frame() -> usize {
  let waiting = WAITING.with_borrow_mut(std::mem::take);
  let count = waiting.len();
  if count > 0 {
    reactive::mark(waiting);
  }
  count
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn outside_a_build_nothing_is_asked_for() {
    request_frame();
    assert!(!frame_requested());
    assert_eq!(start_frame(), 0);
  }
}
