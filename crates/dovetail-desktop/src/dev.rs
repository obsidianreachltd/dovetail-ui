//! Support for `dovetail dev` (M05), switched on by environment variables
//! that only `dovetail dev` sets.
//!
//! - `DOVETAIL_DEV_STATE`: a file that keeps the first window's position and
//!   size, so a restarted program opens where the last one was.
//! - `DOVETAIL_DEV_READY`: a file written after the first frame is shown, so
//!   `dovetail dev` can measure how long a restart took.

use std::path::PathBuf;

use winit::window::Window;

/// The first window's place on screen, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
  pub x: i32,
  pub y: i32,
  pub width: u32,
  pub height: u32,
}

impl Placement {
  fn parse(text: &str) -> Option<Self> {
    let mut parts = text.split_whitespace();
    let placement = Placement {
      x: parts.next()?.parse().ok()?,
      y: parts.next()?.parse().ok()?,
      width: parts.next()?.parse().ok()?,
      height: parts.next()?.parse().ok()?,
    };
    (placement.width > 0 && placement.height > 0).then_some(placement)
  }

  fn format(self) -> String {
    format!("{} {} {} {}\n", self.x, self.y, self.width, self.height)
  }
}

#[derive(Default)]
pub(crate) struct DevState {
  state: Option<PathBuf>,
  ready: Option<PathBuf>,
}

impl DevState {
  pub fn from_env() -> Self {
    let path = |name| {
      std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    };
    Self {
      state: path("DOVETAIL_DEV_STATE"),
      ready: path("DOVETAIL_DEV_READY"),
    }
  }

  /// Where the first window should open, if a last run saved it.
  pub fn placement(&self) -> Option<Placement> {
    let text = std::fs::read_to_string(self.state.as_ref()?).ok()?;
    Placement::parse(&text)
  }

  /// Saves the window's place. Errors are ignored: this is a convenience.
  pub fn save(
    &self,
    window: &Window,
  ) {
    let Some(path) = &self.state else {
      return;
    };
    let Ok(position) = window.outer_position() else {
      return;
    };
    let size = window.inner_size();
    if size.width == 0
      || size.height == 0
      || window.is_minimized() == Some(true)
    {
      return;
    }
    let placement = Placement {
      x: position.x,
      y: position.y,
      width: size.width,
      height: size.height,
    };
    let _ = std::fs::write(path, placement.format());
  }

  /// Writes the ready file once, after the first frame.
  pub fn ready(&mut self) {
    if let Some(path) = self.ready.take() {
      let _ = std::fs::write(path, b"ready\n");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn placement_round_trips_through_text() {
    let p = Placement { x: -12, y: 40, width: 800, height: 600 };
    assert_eq!(Placement::parse(&p.format()), Some(p));
  }

  #[test]
  fn bad_or_empty_placement_is_ignored() {
    assert_eq!(Placement::parse(""), None);
    assert_eq!(Placement::parse("1 2 3"), None);
    assert_eq!(Placement::parse("1 2 0 600"), None);
    assert_eq!(Placement::parse("a b c d"), None);
  }
}
