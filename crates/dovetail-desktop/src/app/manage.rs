//! Window management (M19, D0029): the monitors, the list of windows the
//! application has open, bringing a window to the front, modal windows
//! that block their owners, and when the application ends.

use std::time::{Duration, Instant};

use dovetail_core::{AppWindow, ComponentId, Monitor, ScreenRect};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use super::Shell;

type WinitId = winit::window::WindowId;

/// Whether the application ends when its last window closes (M19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitPolicy {
  /// It ends when its last window closes.
  AfterLastWindow,
  /// It keeps running with no windows, as macOS applications do, until it
  /// is told to quit.
  KeepRunning,
}

impl Default for ExitPolicy {
  /// What users of each system expect: keep running on macOS, end
  /// elsewhere.
  fn default() -> Self {
    if cfg!(target_os = "macos") {
      ExitPolicy::KeepRunning
    } else {
      ExitPolicy::AfterLastWindow
    }
  }
}

/// How long after the program brings a window to the front its Alt key
/// events are ignored. winit's `focus_window` on Windows presses Alt with
/// `SendInput`, so that the system lets it change the window in front,
/// and the window gets that press.
const FOCUS_ALT: Duration = Duration::from_millis(250);

/// The monitors as winit and the system describe them now.
pub(super) fn read_monitors(event_loop: &ActiveEventLoop) -> Vec<Monitor> {
  let primary = event_loop.primary_monitor();
  event_loop
    .available_monitors()
    .map(|m| {
      let (p, s) = (m.position(), m.size());
      let bounds = ScreenRect::new(
        p.x as f32,
        p.y as f32,
        s.width as f32,
        s.height as f32,
      );
      Monitor {
        name: m.name().unwrap_or_default(),
        bounds,
        work_area: crate::platform::work_area(&m).unwrap_or(bounds),
        scale: m.scale_factor() as f32,
        primary: primary.as_ref() == Some(&m),
      }
    })
    .collect()
}

/// Where a window is in its normal state, in physical pixels: its frame's
/// position, where the system says, and its inner size (M19).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NormalPlace {
  pub(crate) position: Option<winit::dpi::PhysicalPosition<i32>>,
  pub(crate) size: winit::dpi::PhysicalSize<u32>,
}

impl NormalPlace {
  /// Whether `window` is at this place, give or take a few pixels.
  fn holds(
    &self,
    window: &winit::window::Window,
  ) -> bool {
    let near = |a: i64, b: i64| (a - b).abs() <= 4;
    let size = window.inner_size();
    let sized = near(i64::from(size.width), i64::from(self.size.width))
      && near(i64::from(size.height), i64::from(self.size.height));
    let placed = match (self.position, window.outer_position().ok()) {
      (Some(a), Some(b)) => {
        near(i64::from(a.x), i64::from(b.x))
          && near(i64::from(a.y), i64::from(b.y))
      },
      _ => true,
    };
    sized && placed
  }

  /// Puts `window` at this place.
  fn restore(
    &self,
    window: &winit::window::Window,
  ) {
    let _ = window.request_inner_size(self.size);
    if let Some(at) = self.position {
      window.set_outer_position(at);
    }
  }
}

/// Whether the desktop puts a window at its normal place itself, when it
/// opens and when it leaves maximised, if the system has not. On macOS a
/// resizable window unzooms to where AppKit remembers, which can be its
/// zoomed frame when it opened zoomed, and winit then sees it as normal
/// already; and a window has been seen to open away from where it was
/// asked to (B0013). Elsewhere the system does it.
pub(crate) const PUT_BACK_NORMAL: bool = cfg!(target_os = "macos");

/// How long to wait before asking again for a window's state.
const ASK_AGAIN: Duration = Duration::from_millis(300);

/// A state a window was asked for, and how many more times to ask (M19).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Wanted {
  state: dovetail_core::WindowState,
  left: u8,
  next: Instant,
}

impl Wanted {
  /// A state just asked for. It is asked again every `ASK_AGAIN`, up to
  /// eight times, which is longer than the full-screen animation on macOS.
  pub(crate) fn new(state: dovetail_core::WindowState) -> Self {
    Self {
      state,
      left: 8,
      // real time: the system's changes take real time.
      next: Instant::now() + ASK_AGAIN,
    }
  }
}

impl Shell {
  /// Asks again for the states windows were asked for, until the system
  /// shows them or the asks run out, and says when to look next. A window
  /// manager on X11 can miss an ask made before it took the window, and
  /// AppKit refuses a change while a window is still changing to or from
  /// full screen (M19).
  pub(super) fn ask_wanted_states(&self) -> Option<Instant> {
    // real time: the system's changes take real time, as its animations do.
    let now = Instant::now();
    let mut next = None::<Instant>;
    for entry in self.windows.values() {
      let Some(mut wanted) = entry.wanted.get() else {
        continue;
      };
      let reached = super::state_of(&entry.window) == wanted.state;
      // Back in the normal state, but not at its normal place (M19).
      let misplaced = reached
        && PUT_BACK_NORMAL
        && wanted.state == dovetail_core::WindowState::Normal
        && entry
          .normal
          .get()
          .is_some_and(|n| !n.holds(&entry.window));
      if reached && !misplaced {
        entry.wanted.set(None);
        continue;
      }
      if now >= wanted.next {
        if wanted.left == 0 {
          entry.wanted.set(None);
          continue;
        }
        wanted.left -= 1;
        wanted.next = now + ASK_AGAIN;
        match entry.normal.get() {
          Some(normal) if misplaced => normal.restore(&entry.window),
          _ => super::apply_state(&entry.window, wanted.state),
        }
        entry.wanted.set(Some(wanted));
      }
      next = Some(next.map_or(wanted.next, |n| n.min(wanted.next)));
    }
    next
  }

  /// Remembers where window `id` is, while it is in its normal state and
  /// nothing is asked of it, to go back there from maximised (M19).
  pub(super) fn keep_normal_place(
    &self,
    id: WinitId,
  ) {
    let Some(entry) = self.windows.get(&id) else {
      return;
    };
    if entry.wanted.get().is_some()
      || super::state_of(&entry.window) != dovetail_core::WindowState::Normal
    {
      return;
    }
    entry.normal.set(Some(NormalPlace {
      position: entry.window.outer_position().ok(),
      size: entry.window.inner_size(),
    }));
  }

  /// Reads the monitors again. Components that read them are built again
  /// when they changed (D0029: on window events, with no timer).
  pub(super) fn refresh_monitors(
    &self,
    event_loop: &ActiveEventLoop,
  ) {
    dovetail_core::set_monitors(read_monitors(event_loop));
  }

  /// Gives components the windows opened through the host, in the order
  /// they opened. A comparison when nothing changed.
  pub(super) fn publish_windows(&self) {
    let mut windows: Vec<AppWindow> = self
      .windows
      .values()
      .filter_map(|e| {
        let hosted = e.hosted.as_ref()?;
        Some(AppWindow {
          id: hosted.id,
          title: e.title.clone(),
          state: e.state,
          focused: e.stats.focused == Some(true),
        })
      })
      .collect();
    windows.sort_by_key(|w| w.id.0);
    dovetail_core::set_app_windows(windows);
  }

  /// Brings a window to the front and gives it the keyboard, and ignores
  /// the Alt press that winit sends to do it.
  pub(super) fn bring_to_front(
    &mut self,
    id: WinitId,
  ) {
    if let Some(entry) = self.windows.get_mut(&id) {
      // The system sends the Alt press now, whatever a test's clock says.
      // real time: as the system sends it.
      entry.focus_alt_until = Some(Instant::now() + FOCUS_ALT);
      entry.window.focus_window();
    }
  }

  /// True for a key event of Alt soon after the program brought window
  /// `id` to the front.
  pub(super) fn alt_from_focus(
    &self,
    id: WinitId,
    event: &WindowEvent,
  ) -> bool {
    let WindowEvent::KeyboardInput { event, .. } = event else {
      return false;
    };
    matches!(
      event.logical_key,
      Key::Named(NamedKey::Alt | NamedKey::AltGraph)
    ) && self
      .windows
      .get(&id)
      .and_then(|e| e.focus_alt_until)
      // real time: as above.
      .is_some_and(|until| Instant::now() < until)
  }

  /// The modal window that blocks window `id`, if one does.
  pub(super) fn blocker(
    &self,
    id: WinitId,
  ) -> Option<WinitId> {
    self
      .windows
      .iter()
      .find(|(_, e)| e.modal && e.owner == Some(id))
      .map(|(w, _)| *w)
  }

  /// True when `event` to window `id` is refused because a modal window
  /// blocks it (D0029): presses, the wheel, keys, text and its close
  /// button. A press or a close brings the modal window to the front.
  /// Hover and painting go on.
  pub(super) fn refuse(
    &mut self,
    id: WinitId,
    event: &WindowEvent,
  ) -> bool {
    let Some(modal) = self.blocker(id) else {
      return false;
    };
    match event {
      WindowEvent::MouseInput { state: ElementState::Pressed, .. }
      | WindowEvent::CloseRequested => {
        self.bring_to_front(modal);
        true
      },
      WindowEvent::MouseInput { .. }
      | WindowEvent::MouseWheel { .. }
      | WindowEvent::KeyboardInput { .. }
      | WindowEvent::Ime(_) => true,
      _ => false,
    }
  }

  /// The window whose components include `component`: the one a modal
  /// window opened by that component belongs to.
  pub(super) fn window_holding(
    &self,
    component: Option<ComponentId>,
  ) -> Option<WinitId> {
    let component = component?;
    self
      .windows
      .iter()
      .find(|(_, e)| {
        e.ui
          .as_ref()
          .is_some_and(|ui| ui.session.contains(component))
      })
      .map(|(w, _)| *w)
  }

  /// True when the application should end now: no windows are left, and
  /// it does not keep running without them. A test that pumps the loop
  /// ends it itself.
  pub(super) fn should_end(&self) -> bool {
    self.windows.is_empty()
      && !self.pumped
      && self.exit_policy == ExitPolicy::AfterLastWindow
  }
}
