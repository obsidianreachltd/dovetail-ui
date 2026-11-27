//! Support for `dovetail dev` (M05), switched on by environment variables
//! that only `dovetail dev` sets.
//!
//! - `DOVETAIL_DEV_STATE`: a file that keeps every window's place and
//!   state, as saved windows' text (M19), so a restarted program's windows
//!   open where the last one's were.
//! - `DOVETAIL_DEV_READY`: a file written after the first frame is shown, so
//!   `dovetail dev` can measure how long a restart took.

use std::path::PathBuf;

use dovetail_core::{
  Monitor, SavedMonitor, SavedWindow, SavedWindows, WindowLimits, WindowPlace,
  WindowState,
};
use winit::window::Window;

#[derive(Default)]
pub(crate) struct DevState {
  /// Every window's place and state, kept in `DOVETAIL_DEV_STATE` as
  /// saved windows' text (M19, D0029).
  saved: Option<SavedWindows>,
  /// How many windows have opened, which names the next one.
  opened: usize,
  ready: Option<PathBuf>,
}

impl DevState {
  pub fn from_env() -> Self {
    let path = |name| {
      std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    };
    let saved = path("DOVETAIL_DEV_STATE").map(|p| {
      let saved = SavedWindows::new();
      // A file from before M19, or one that cannot be read, is written
      // anew when a window next moves.
      let _ = saved.keep_in_file(p);
      saved
    });
    Self {
      saved,
      opened: 0,
      ready: path("DOVETAIL_DEV_READY"),
    }
  }

  /// The name the next window opened is kept under, while `dovetail dev`
  /// runs the program: the order it opened in, which a restart keeps.
  pub fn next_name(&mut self) -> Option<String> {
    self.saved.as_ref()?;
    self.opened += 1;
    Some(format!("window {}", self.opened))
  }

  /// Where the window named `name` should open, and in what state, if a
  /// last run saved it, fitted onto `monitors`.
  pub fn restore(
    &self,
    name: &str,
    monitors: &[Monitor],
    limits: WindowLimits,
  ) -> Option<(WindowPlace, WindowState)> {
    let saved = self.saved.as_ref()?.get(name)?;
    Some(dovetail_core::fit(&saved, monitors, limits))
  }

  /// Saves a window's place and state. Errors are ignored: this is a
  /// convenience.
  pub fn save(
    &self,
    name: &str,
    window: &Window,
  ) {
    let Some(saved) = &self.saved else {
      return;
    };
    let state = crate::app::state_of(window);
    match state {
      WindowState::Minimised => {},
      WindowState::Normal => {
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
          return;
        }
        let scale = window.scale_factor();
        let position = window.outer_position().ok().map(|p| {
          (
            (f64::from(p.x) / scale) as f32,
            (f64::from(p.y) / scale) as f32,
          )
        });
        let monitor = window.current_monitor().map(|m| SavedMonitor {
          name: m.name().unwrap_or_default(),
          bounds: dovetail_core::ScreenRect::new(
            m.position().x as f32,
            m.position().y as f32,
            m.size().width as f32,
            m.size().height as f32,
          ),
        });
        saved.set(
          name,
          SavedWindow {
            position,
            size: (
              (f64::from(size.width) / scale) as f32,
              (f64::from(size.height) / scale) as f32,
            ),
            scale: scale as f32,
            state,
            monitor,
          },
        );
      },
      // The normal place stays as it was saved.
      _ => saved.set_state(name, state),
    }
  }

  /// Writes the ready file once, after the first frame.
  pub fn ready(&mut self) {
    if let Some(path) = self.ready.take() {
      let _ = std::fs::write(path, b"ready\n");
    }
  }
}
