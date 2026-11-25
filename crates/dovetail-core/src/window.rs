//! Extra windows opened by components, such as floating panels (M10).
//!
//! The desktop crate installs a [`WindowHost`] with [`set_window_host`].
//! Components then open and close windows through [`window_host`] without
//! depending on the desktop crate.

use std::cell::RefCell;
use std::rc::Rc;

use crate::view::{Callback, View};

/// Where a window is and how big its drawing area is, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowPlace {
  /// The window's top-left corner on the screen, frame included, when the
  /// system says (Wayland does not).
  pub position: Option<(f32, f32)>,
  /// The drawing area's size.
  pub size: (f32, f32),
}

/// A window to open.
pub struct WindowRequest {
  /// The title.
  pub title: String,
  /// Where to put it. Without a position, the system chooses.
  pub place: WindowPlace,
  /// What it shows: a component view.
  pub root: View,
  /// Called when the user asks to close it. The window stays open until
  /// [`WindowHost::close`] is called.
  pub on_close: Callback<()>,
  /// Called when the window moves or changes size.
  pub on_place: Callback<WindowPlace>,
  /// The component whose provided values, such as the theme, the window's
  /// components see (M14). Usually the component that opens it.
  pub context: Option<crate::ComponentId>,
}

/// Names a window opened through the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostWindow(pub u64);

/// Opens and closes windows for components.
///
/// Requests are carried out as soon as the event loop can, so they may be
/// made at any time, even while a component builds.
pub trait WindowHost {
  /// Opens a window. It closes by itself when the application's first
  /// window closes.
  fn open(
    &self,
    request: WindowRequest,
  ) -> HostWindow;

  /// Closes a window opened with [`WindowHost::open`].
  fn close(
    &self,
    window: HostWindow,
  );

  /// Changes a window's title.
  fn set_title(
    &self,
    window: HostWindow,
    title: &str,
  );
}

thread_local! {
  static HOST: RefCell<Option<Rc<dyn WindowHost>>> = const { RefCell::new(None) };
}

/// Installs the window host for this thread, or removes it with `None`.
pub fn set_window_host(host: Option<Rc<dyn WindowHost>>) {
  HOST.with_borrow_mut(|h| *h = host);
}

/// The window host, while a desktop is running.
pub fn window_host() -> Option<Rc<dyn WindowHost>> {
  HOST.with_borrow(Clone::clone)
}

/// Lets a component be asked before the window that shows it closes
/// (M11). Make one with `use_state(CloseGuard::new)`, and give it to an
/// element with [`Element::close_guard`](crate::Element::close_guard).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CloseGuard(u64);

thread_local! {
  static NEXT_GUARD: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
  static CLOSE_NOW: RefCell<Vec<CloseGuard>> = const { RefCell::new(Vec::new()) };
}

impl Default for CloseGuard {
  fn default() -> Self {
    Self::new()
  }
}

impl CloseGuard {
  /// A guard no other guard is equal to.
  pub fn new() -> CloseGuard {
    let id = NEXT_GUARD.get();
    NEXT_GUARD.set(id + 1);
    CloseGuard(id)
  }

  /// Closes the window that shows this guard, without asking again. The
  /// window does it at its next frame.
  pub fn close_now(self) {
    CLOSE_NOW.with_borrow_mut(|c| {
      if !c.contains(&self) {
        c.push(self);
      }
    });
    crate::reactive::notify_change();
  }
}

/// The guards whose windows should close now. The desktop crate takes them
/// after layout.
pub fn take_close_requests() -> Vec<CloseGuard> {
  CLOSE_NOW.with_borrow_mut(std::mem::take)
}
