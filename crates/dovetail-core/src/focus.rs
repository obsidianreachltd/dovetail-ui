//! Moving focus from code (M10).

use std::cell::Cell;

use crate::reactive;

/// Names a part of a window that code can move focus to, such as a panel.
/// Give it to an element with
/// [`Element::focus_target`](crate::Element::focus_target), then call
/// [`FocusTarget::focus`]. The same name always gives the same target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusTarget(u64);

impl FocusTarget {
  /// The target called `name`.
  pub fn named(name: &str) -> FocusTarget {
    // FNV-1a: the same in every run and on every system.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.bytes() {
      hash ^= u64::from(byte);
      hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    FocusTarget(hash)
  }

  /// Asks for focus to move to the first element inside the target that
  /// can take it, or to the target itself. The window that shows the
  /// target does this at its next frame, and comes to the front. A later
  /// request replaces one not yet carried out.
  pub fn focus(self) {
    PENDING.set(Some(self));
    reactive::notify_change();
  }
}

thread_local! {
  static PENDING: Cell<Option<FocusTarget>> = const { Cell::new(None) };
}

/// The focus request not yet carried out. Windows check it after layout.
pub fn pending_focus() -> Option<FocusTarget> {
  PENDING.get()
}

/// Marks the request for `target` as carried out.
pub fn focus_done(target: FocusTarget) {
  if PENDING.get() == Some(target) {
    PENDING.set(None);
  }
}
