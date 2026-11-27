//! Monitors, and fitting a saved window onto the monitors there are (M19,
//! D0029).
//!
//! Places on the desktop are in physical pixels, since monitors side by
//! side can have different scales. A window's size is in logical pixels,
//! so it keeps its size across a change of scale.

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::reactive::{self, ComponentId};
use crate::window::{WindowLimits, WindowPlace, WindowState};

/// A rectangle on the desktop, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenRect {
  /// The left edge.
  pub x: f32,
  /// The top edge.
  pub y: f32,
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
}

impl ScreenRect {
  /// A rectangle from its top-left corner and size.
  pub const fn new(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
  ) -> Self {
    Self { x, y, width, height }
  }

  /// The area this rectangle shares with `other`, in square pixels.
  pub fn overlap(
    &self,
    other: &ScreenRect,
  ) -> f32 {
    let w =
      (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
    let h =
      (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
    w.max(0.0) * h.max(0.0)
  }

  /// True when `point` is inside.
  pub fn contains(
    &self,
    (x, y): (f32, f32),
  ) -> bool {
    x >= self.x
      && y >= self.y
      && x < self.x + self.width
      && y < self.y + self.height
  }
}

/// A monitor, as the system describes it (M19).
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
  /// The system's name for it. It may be empty.
  pub name: String,
  /// Where it is on the desktop, and its size.
  pub bounds: ScreenRect,
  /// The part windows should use: the monitor without the taskbar, the
  /// menu bar or the Dock. Where the system does not say, the whole
  /// monitor.
  pub work_area: ScreenRect,
  /// Physical pixels for each logical pixel.
  pub scale: f32,
  /// Whether it is the main monitor.
  pub primary: bool,
}

thread_local! {
  static MONITORS: RefCell<(Vec<Monitor>, BTreeSet<ComponentId>)> =
    const { RefCell::new((Vec::new(), BTreeSet::new())) };
}

/// The monitors there are, as the desktop last read them (M19). Read while
/// a component builds, it remembers that component, which is built again
/// when a monitor is added, removed or changed. Empty where no desktop
/// runs, as in headless tests that set none.
pub fn monitors() -> Vec<Monitor> {
  let current = reactive::current();
  MONITORS.with_borrow_mut(|(monitors, readers)| {
    if let Some(id) = current {
      readers.insert(id);
    }
    monitors.clone()
  })
}

/// Sets the monitors [`monitors`] gives. The desktop calls it when it reads
/// them; tests call it to try other monitors.
pub fn set_monitors(monitors: Vec<Monitor>) {
  let readers = MONITORS.with_borrow_mut(|(old, readers)| {
    if *old == monitors {
      return None;
    }
    *old = monitors;
    Some(std::mem::take(readers))
  });
  if let Some(readers) = readers {
    reactive::mark(readers);
  }
}

/// The monitor that holds a point on the desktop, in physical pixels, or
/// the nearest one.
pub fn monitor_at(
  monitors: &[Monitor],
  point: (f32, f32),
) -> Option<&Monitor> {
  monitors
    .iter()
    .find(|m| m.bounds.contains(point))
    .or_else(|| {
      monitors.iter().min_by(|a, b| {
        distance(&a.bounds, point).total_cmp(&distance(&b.bounds, point))
      })
    })
}

fn distance(
  r: &ScreenRect,
  (x, y): (f32, f32),
) -> f32 {
  let dx = (r.x - x).max(0.0).max(x - (r.x + r.width));
  let dy = (r.y - y).max(0.0).max(y - (r.y + r.height));
  dx * dx + dy * dy
}

/// The monitor a window saved its place on.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedMonitor {
  /// Its name.
  pub name: String,
  /// Where it was, and its size.
  pub bounds: ScreenRect,
}

/// What is saved of a window between runs (M19).
#[derive(Clone, Debug, PartialEq)]
pub struct SavedWindow {
  /// Its top-left corner, frame included, in logical pixels at `scale`.
  /// `None` where the system does not say, as on Wayland.
  pub position: Option<(f32, f32)>,
  /// Its drawing area's size in its normal state, in logical pixels.
  pub size: (f32, f32),
  /// The scale of the monitor it was on.
  pub scale: f32,
  /// Its state. A window is never saved minimised: it keeps the state it
  /// had before.
  pub state: WindowState,
  /// The monitor it was on, when known.
  pub monitor: Option<SavedMonitor>,
}

/// Room for a window's frame and title bar, in logical pixels, which its
/// drawing area's size leaves out. The frame differs between systems, so
/// fitting keeps this much more room.
pub const FRAME_ROOM: (f32, f32) = (16.0, 40.0);

/// Where a saved window opens, fitted onto the monitors there are (M19).
///
/// - The monitor: the saved one if one with its name and bounds is there;
///   if not, the one the saved place overlaps most; if none, the primary
///   monitor, with the window centred on it.
/// - The size is kept in logical pixels, within `limits`, and made smaller
///   to fit the monitor's work area, but not below the minimum size.
/// - The position is moved so the whole window, with [`FRAME_ROOM`], is
///   on the work area.
///
/// With no monitors, the saved place is used as it is. The returned
/// position is in logical pixels at the scale of the monitor it is on, as
/// [`WindowPlace`] positions are.
pub fn fit(
  saved: &SavedWindow,
  monitors: &[Monitor],
  limits: WindowLimits,
) -> (WindowPlace, WindowState) {
  let state = match saved.state {
    WindowState::Minimised => WindowState::Normal,
    s => s,
  };
  let size = limits.clamp(saved.size);
  if monitors.is_empty() {
    return (WindowPlace { position: saved.position, size }, state);
  }
  let scale = if saved.scale > 0.0 { saved.scale } else { 1.0 };
  // Where it was, in physical pixels.
  let was = saved.position.map(|(x, y)| {
    ScreenRect::new(
      x * scale,
      y * scale,
      (size.0 + FRAME_ROOM.0) * scale,
      (size.1 + FRAME_ROOM.1) * scale,
    )
  });
  let same = saved.monitor.as_ref().and_then(|s| {
    monitors
      .iter()
      .find(|m| m.name == s.name && m.bounds == s.bounds)
  });
  let overlapping = was.and_then(|r| {
    monitors
      .iter()
      .map(|m| (m, m.bounds.overlap(&r)))
      .filter(|(_, area)| *area > 0.0)
      .max_by(|a, b| a.1.total_cmp(&b.1))
      .map(|(m, _)| m)
  });
  let found = same.or(overlapping);
  let target = found
    .or_else(|| monitors.iter().find(|m| m.primary))
    .unwrap_or(&monitors[0]);
  let to = if target.scale > 0.0 {
    target.scale
  } else {
    1.0
  };
  let area = target.work_area;

  // The size, made smaller to fit the work area, but not below the minimum.
  let room = (
    area.width / to - FRAME_ROOM.0,
    area.height / to - FRAME_ROOM.1,
  );
  let mut size = (size.0.min(room.0), size.1.min(room.1));
  size = limits.clamp(size);
  let outer = ((size.0 + FRAME_ROOM.0) * to, (size.1 + FRAME_ROOM.1) * to);

  let position = match (was, found) {
    // Wayland: the system places it.
    (None, _) => None,
    // On a monitor that is there: kept, moved onto the work area if need be.
    (Some(r), Some(_)) => Some(keep_inside((r.x, r.y), outer, &area)),
    // Its monitor is gone: centred on the one chosen.
    _ => Some(keep_inside(
      (
        area.x + (area.width - outer.0) / 2.0,
        area.y + (area.height - outer.1) / 2.0,
      ),
      outer,
      &area,
    )),
  };
  (
    WindowPlace {
      position: position.map(|(x, y)| (x / to, y / to)),
      size,
    },
    state,
  )
}

/// `at` moved so that a window `outer` pixels big is inside `area`, or at
/// its top-left corner when it is bigger.
fn keep_inside(
  (x, y): (f32, f32),
  outer: (f32, f32),
  area: &ScreenRect,
) -> (f32, f32) {
  let x = x.min(area.x + area.width - outer.0).max(area.x);
  let y = y.min(area.y + area.height - outer.1).max(area.y);
  (x, y)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn monitor(
    name: &str,
    bounds: ScreenRect,
    scale: f32,
    primary: bool,
  ) -> Monitor {
    // A taskbar 48 logical pixels high along the bottom.
    let work_area = ScreenRect {
      height: bounds.height - 48.0 * scale,
      ..bounds
    };
    Monitor {
      name: name.into(),
      bounds,
      work_area,
      scale,
      primary,
    }
  }

  fn left() -> Monitor {
    monitor("left", ScreenRect::new(0.0, 0.0, 1920.0, 1080.0), 1.0, true)
  }

  /// A 4K monitor at 200 % to the right of the left one.
  fn right() -> Monitor {
    monitor(
      "right",
      ScreenRect::new(1920.0, 0.0, 3840.0, 2160.0),
      2.0,
      false,
    )
  }

  fn saved_on(
    m: &Monitor,
    position: (f32, f32),
    size: (f32, f32),
  ) -> SavedWindow {
    SavedWindow {
      position: Some(position),
      size,
      scale: m.scale,
      state: WindowState::Normal,
      monitor: Some(SavedMonitor { name: m.name.clone(), bounds: m.bounds }),
    }
  }

  /// The window's outer rectangle in physical pixels.
  fn outer(
    place: &WindowPlace,
    scale: f32,
  ) -> ScreenRect {
    let (x, y) = place.position.expect("a position");
    ScreenRect::new(
      x * scale,
      y * scale,
      (place.size.0 + FRAME_ROOM.0) * scale,
      (place.size.1 + FRAME_ROOM.1) * scale,
    )
  }

  fn fully_on(
    r: &ScreenRect,
    m: &Monitor,
  ) -> bool {
    let a = m.work_area;
    r.x >= a.x
      && r.y >= a.y
      && r.x + r.width <= a.x + a.width + 0.01
      && r.y + r.height <= a.y + a.height + 0.01
  }

  #[test]
  fn a_window_on_a_monitor_that_is_there_comes_back_where_it_was() {
    let r = right();
    let saved = saved_on(&r, (1200.0, 100.0), (800.0, 600.0));
    let (place, state) = fit(&saved, &[left(), r], WindowLimits::default());
    assert_eq!(place.position, Some((1200.0, 100.0)));
    assert_eq!(place.size, (800.0, 600.0));
    assert_eq!(state, WindowState::Normal);
  }

  #[test]
  fn a_window_on_a_monitor_that_is_gone_opens_fully_on_one_that_is() {
    // Criterion 2: saved on the right monitor, which is unplugged.
    let saved = saved_on(&right(), (1500.0, 300.0), (800.0, 600.0));
    let l = left();
    let (place, _) =
      fit(&saved, std::slice::from_ref(&l), WindowLimits::default());
    assert!(fully_on(&outer(&place, l.scale), &l), "{place:?}");
    // Its size in logical pixels is kept, at the new scale.
    assert_eq!(place.size, (800.0, 600.0));
  }

  #[test]
  fn a_window_too_big_for_the_monitor_left_is_made_smaller() {
    let saved = saved_on(&right(), (1000.0, 50.0), (1800.0, 1000.0));
    let l = left();
    let (place, _) =
      fit(&saved, std::slice::from_ref(&l), WindowLimits::default());
    assert!(fully_on(&outer(&place, 1.0), &l), "{place:?}");
    assert_eq!(place.size.0, 1800.0, "it was narrow enough");
    assert!(place.size.1 < 1000.0, "but too high");
  }

  #[test]
  fn making_a_window_smaller_keeps_its_minimum_size() {
    let saved = saved_on(&right(), (1000.0, 50.0), (1800.0, 1000.0));
    let limits = WindowLimits {
      min_size: Some((1900.0, 1100.0)),
      ..WindowLimits::default()
    };
    let (place, _) = fit(&saved, &[left()], limits);
    assert_eq!(place.size, (1900.0, 1100.0));
    // Too high to fit, so its title bar is kept at the top.
    assert_eq!(place.position.map(|p| p.1), Some(0.0));
  }

  #[test]
  fn a_window_half_off_screen_is_moved_onto_the_work_area() {
    let l = left();
    let saved = saved_on(&l, (1700.0, 900.0), (400.0, 300.0));
    let (place, _) =
      fit(&saved, std::slice::from_ref(&l), WindowLimits::default());
    assert!(fully_on(&outer(&place, 1.0), &l), "{place:?}");
    assert_eq!(place.size, (400.0, 300.0));
  }

  #[test]
  fn a_window_keeps_its_logical_size_across_a_change_of_scale() {
    // Saved at 100 %; the same monitor is now at 150 %.
    let l = left();
    let saved = saved_on(&l, (100.0, 100.0), (800.0, 600.0));
    let mut scaled = l.clone();
    scaled.scale = 1.5;
    scaled.work_area.height = scaled.bounds.height - 48.0 * 1.5;
    let (place, _) = fit(
      &saved,
      std::slice::from_ref(&scaled),
      WindowLimits::default(),
    );
    assert_eq!(place.size, (800.0, 600.0));
    assert!(fully_on(&outer(&place, 1.5), &scaled), "{place:?}");
  }

  #[test]
  fn a_window_on_monitors_moved_around_finds_the_one_it_overlaps() {
    // The right monitor is now on the left, and named differently.
    let saved = saved_on(&right(), (1200.0, 100.0), (800.0, 600.0));
    let moved = monitor(
      "other",
      ScreenRect::new(1920.0, 0.0, 1920.0, 1080.0),
      1.0,
      false,
    );
    let (place, _) =
      fit(&saved, &[left(), moved.clone()], WindowLimits::default());
    assert!(fully_on(&outer(&place, 1.0), &moved), "{place:?}");
  }

  #[test]
  fn a_minimised_window_opens_normal_and_others_keep_their_state() {
    let l = left();
    for (saved_state, opens) in [
      (WindowState::Minimised, WindowState::Normal),
      (WindowState::Maximised, WindowState::Maximised),
      (WindowState::FullScreen, WindowState::FullScreen),
    ] {
      let saved = SavedWindow {
        state: saved_state,
        ..saved_on(&l, (0.0, 0.0), (300.0, 200.0))
      };
      assert_eq!(
        fit(&saved, std::slice::from_ref(&l), WindowLimits::default()).1,
        opens
      );
    }
  }

  #[test]
  fn without_a_position_the_system_places_the_window() {
    let saved = SavedWindow {
      position: None,
      ..saved_on(&left(), (0.0, 0.0), (500.0, 400.0))
    };
    let (place, _) = fit(&saved, &[left()], WindowLimits::default());
    assert_eq!(place.position, None);
    assert_eq!(place.size, (500.0, 400.0));
  }

  #[test]
  fn with_no_monitors_the_saved_place_is_used() {
    let saved = saved_on(&right(), (5000.0, 5000.0), (500.0, 400.0));
    let (place, _) = fit(&saved, &[], WindowLimits::default());
    assert_eq!(place.position, Some((5000.0, 5000.0)));
  }

  #[test]
  fn the_monitor_at_a_point_or_the_nearest() {
    let ms = [left(), right()];
    assert_eq!(
      monitor_at(&ms, (2000.0, 10.0)).map(|m| m.name.as_str()),
      Some("right")
    );
    assert_eq!(
      monitor_at(&ms, (-50.0, 10.0)).map(|m| m.name.as_str()),
      Some("left")
    );
  }
}
