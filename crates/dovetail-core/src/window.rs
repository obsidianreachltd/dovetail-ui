//! Extra windows opened by components, such as floating panels (M10).
//!
//! The desktop crate installs a [`WindowHost`] with [`set_window_host`].
//! Components then open and close windows through [`window_host`] without
//! depending on the desktop crate.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::reactive::{self, ComponentId};
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

/// The state a window is in (M19).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowState {
  /// At its own size and place.
  #[default]
  Normal,
  /// Filling the work area of its monitor, with its frame.
  Maximised,
  /// Hidden in the taskbar or the Dock.
  Minimised,
  /// Filling its whole monitor, with no frame.
  FullScreen,
}

/// The sizes a window keeps to, in logical pixels, and whether the user
/// can resize it (M19).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowLimits {
  /// The smallest drawing area the user can make it.
  pub min_size: Option<(f32, f32)>,
  /// The largest drawing area the user can make it.
  pub max_size: Option<(f32, f32)>,
  /// Whether the user can resize it.
  pub resizable: bool,
}

impl Default for WindowLimits {
  fn default() -> Self {
    Self {
      min_size: None,
      max_size: None,
      resizable: true,
    }
  }
}

impl WindowLimits {
  /// `size` kept between the minimum and the maximum.
  pub fn clamp(
    &self,
    size: (f32, f32),
  ) -> (f32, f32) {
    let (mut w, mut h) = size;
    if let Some((x, y)) = self.max_size {
      w = w.min(x);
      h = h.min(y);
    }
    if let Some((x, y)) = self.min_size {
      w = w.max(x);
      h = h.max(y);
    }
    (w, h)
  }
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
  /// Its size limits, and whether it can be resized.
  pub limits: WindowLimits,
  /// The state it opens in.
  pub state: WindowState,
  /// Called when the window's state changes, as when the user maximises
  /// it.
  pub on_state: Callback<WindowState>,
  /// Blocks input to the window that shows `context` until it closes
  /// (M19). It is kept in front of that window, and opens centred on it.
  pub modal: bool,
  /// The component whose provided values, such as the theme, the window's
  /// components see (M14). Usually the component that opens it.
  pub context: Option<crate::ComponentId>,
}

impl WindowRequest {
  /// A request for a window titled `title` that shows `root`, 800 × 600
  /// logical pixels, placed by the system, in its normal state. Its
  /// callbacks do nothing.
  pub fn new(
    title: impl Into<String>,
    root: View,
  ) -> Self {
    Self {
      title: title.into(),
      place: WindowPlace { position: None, size: (800.0, 600.0) },
      root,
      on_close: Callback::new(|()| {}),
      on_place: Callback::new(|_| {}),
      limits: WindowLimits::default(),
      state: WindowState::Normal,
      on_state: Callback::new(|_| {}),
      modal: false,
      context: None,
    }
  }
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

  /// Changes a window's state, as maximising it (M19).
  fn set_state(
    &self,
    window: HostWindow,
    state: WindowState,
  );

  /// Changes a window's size limits, and whether it can be resized (M19).
  fn set_limits(
    &self,
    window: HostWindow,
    limits: WindowLimits,
  );

  /// Brings a window to the front and gives it the keyboard (M19).
  fn focus(
    &self,
    window: HostWindow,
  );
}

/// A window the application has open, opened through the window host
/// (M19).
#[derive(Clone, Debug, PartialEq)]
pub struct AppWindow {
  /// Names it, for [`bring_to_front`].
  pub id: HostWindow,
  /// Its title.
  pub title: String,
  /// Its state.
  pub state: WindowState,
  /// Whether it has the keyboard.
  pub focused: bool,
}

thread_local! {
  static APP_WINDOWS: RefCell<(Vec<AppWindow>, BTreeSet<ComponentId>)> =
    const { RefCell::new((Vec::new(), BTreeSet::new())) };
}

/// The windows the application has open through the window host, in the
/// order they opened (M19). Read while a component builds, it remembers
/// that component, which is built again when the list changes.
pub fn app_windows() -> Vec<AppWindow> {
  let current = reactive::current();
  APP_WINDOWS.with_borrow_mut(|(windows, readers)| {
    if let Some(id) = current {
      readers.insert(id);
    }
    windows.clone()
  })
}

/// Sets the list [`app_windows`] gives. The window host calls it when a
/// window opens, closes, or changes its title, state or focus.
pub fn set_app_windows(windows: Vec<AppWindow>) {
  let readers = APP_WINDOWS.with_borrow_mut(|(old, readers)| {
    if *old == windows {
      return None;
    }
    *old = windows;
    Some(std::mem::take(readers))
  });
  if let Some(readers) = readers {
    reactive::mark(readers);
  }
}

/// Brings a window to the front and gives it the keyboard (M19).
pub fn bring_to_front(window: HostWindow) {
  if let Some(host) = window_host() {
    host.focus(window);
  }
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
