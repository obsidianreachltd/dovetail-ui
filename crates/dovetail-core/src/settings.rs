//! System settings that shape how an application behaves: less motion,
//! the text scale, the caret blink, double-clicks and overlay scroll bars
//! (M18, decision D0028). The desktop crate sets them, and components,
//! controls and the session read them.
//!
//! Each setting keeps its own readers. A component that reads one while
//! it builds is built again when that one changes, and not when another
//! does.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::time::Duration;

use crate::reactive::{self, ComponentId};

/// The smallest text scale kept. Smaller values from the system are
/// raised to it.
pub const MIN_TEXT_SCALE: f32 = 0.5;

/// The largest text scale kept. Larger values from the system are
/// lowered to it.
pub const MAX_TEXT_SCALE: f32 = 3.0;

/// The system's settings for behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemSettings {
  /// The user asked for less motion: transitions jump to their end, and
  /// animated images do not play.
  pub reduce_motion: bool,
  /// How much larger, or smaller, the user wants text, from
  /// [`MIN_TEXT_SCALE`] to [`MAX_TEXT_SCALE`]. 1 is the usual size.
  pub text_scale: f32,
  /// How long a text caret stays on, and then off. `None` when the user
  /// turned blinking off.
  pub caret_blink: Option<Duration>,
  /// The longest time between two clicks that make a double-click.
  pub double_click_time: Duration,
  /// How far, in logical pixels, the pointer may move between two clicks
  /// that make a double-click, across and down.
  pub double_click_distance: f32,
  /// Scroll bars show only while scrolling, as macOS does with its usual
  /// setting.
  pub overlay_scrollbars: bool,
}

impl Default for SystemSettings {
  /// Motion on, the usual text size, a 530 ms blink, and 500 ms and 4
  /// pixels for a double-click: the values used where no system sets
  /// them, as in tests.
  fn default() -> SystemSettings {
    SystemSettings {
      reduce_motion: false,
      text_scale: 1.0,
      caret_blink: Some(Duration::from_millis(530)),
      double_click_time: Duration::from_millis(500),
      double_click_distance: 4.0,
      overlay_scrollbars: false,
    }
  }
}

/// The settings, one place for each, in the order of the readers' lists.
#[derive(Clone, Copy)]
enum Setting {
  ReduceMotion,
  TextScale,
  CaretBlink,
  DoubleClickTime,
  DoubleClickDistance,
  OverlayScrollbars,
}

const SETTINGS: usize = 6;

impl Setting {
  const ALL: [Setting; SETTINGS] = [
    Setting::ReduceMotion,
    Setting::TextScale,
    Setting::CaretBlink,
    Setting::DoubleClickTime,
    Setting::DoubleClickDistance,
    Setting::OverlayScrollbars,
  ];

  /// True when this setting differs between `a` and `b`.
  fn differs(
    self,
    a: &SystemSettings,
    b: &SystemSettings,
  ) -> bool {
    match self {
      Setting::ReduceMotion => a.reduce_motion != b.reduce_motion,
      Setting::TextScale => a.text_scale != b.text_scale,
      Setting::CaretBlink => a.caret_blink != b.caret_blink,
      Setting::DoubleClickTime => a.double_click_time != b.double_click_time,
      Setting::DoubleClickDistance => {
        a.double_click_distance != b.double_click_distance
      },
      Setting::OverlayScrollbars => {
        a.overlay_scrollbars != b.overlay_scrollbars
      },
    }
  }
}

struct Store {
  settings: SystemSettings,
  readers: [BTreeSet<ComponentId>; SETTINGS],
}

thread_local! {
  static STORE: RefCell<Store> = RefCell::new(Store {
    settings: SystemSettings::default(),
    readers: Default::default(),
  });
}

/// Reads the settings, and makes the component that is building, if any,
/// a reader of `which`.
fn read(which: &[Setting]) -> SystemSettings {
  let current = reactive::current();
  STORE.with_borrow_mut(|store| {
    if let Some(id) = current {
      for setting in which {
        store.readers[*setting as usize].insert(id);
      }
    }
    store.settings
  })
}

/// All the system's settings. Read while a component builds, it builds
/// the component again when any of them changes. Read only the setting a
/// component needs, with [`reduce_motion`] and the others, so it is not
/// built again for the rest.
pub fn system_settings() -> SystemSettings {
  read(&Setting::ALL)
}

/// True when the user asked for less motion. Read while a component
/// builds, it builds the component again when this changes.
pub fn reduce_motion() -> bool {
  read(&[Setting::ReduceMotion]).reduce_motion
}

/// How much larger, or smaller, the user wants text: 1 is the usual
/// size. Read while a component builds, it builds the component again
/// when this changes.
pub fn text_scale() -> f32 {
  read(&[Setting::TextScale]).text_scale
}

/// How long a text caret stays on, and then off, or `None` when it does
/// not blink. Read while a component builds, it builds the component
/// again when this changes.
pub fn caret_blink() -> Option<Duration> {
  read(&[Setting::CaretBlink]).caret_blink
}

/// The longest time between two clicks that make a double-click. Read
/// while a component builds, it builds the component again when this
/// changes.
pub fn double_click_time() -> Duration {
  read(&[Setting::DoubleClickTime]).double_click_time
}

/// How far, in logical pixels, the pointer may move between the clicks of
/// a double-click. Read while a component builds, it builds the component
/// again when this changes.
pub fn double_click_distance() -> f32 {
  read(&[Setting::DoubleClickDistance]).double_click_distance
}

/// True when scroll bars show only while scrolling. Read while a
/// component builds, it builds the component again when this changes.
pub fn overlay_scrollbars() -> bool {
  read(&[Setting::OverlayScrollbars]).overlay_scrollbars
}

/// Sets the system's settings. The desktop crate calls it when it reads
/// them, and tests call it to try a setting. Only the readers of the
/// settings that changed are built again.
///
/// The text scale is kept between [`MIN_TEXT_SCALE`] and
/// [`MAX_TEXT_SCALE`], and a scale that is not a number is taken as 1.
/// A negative double-click distance is taken as 0.
pub fn set_system_settings(settings: SystemSettings) {
  let mut settings = settings;
  settings.text_scale = if settings.text_scale.is_nan() {
    1.0
  } else {
    settings
      .text_scale
      .clamp(MIN_TEXT_SCALE, MAX_TEXT_SCALE)
  };
  if settings.double_click_distance.is_nan() {
    settings.double_click_distance = 0.0;
  }
  settings.double_click_distance = settings.double_click_distance.max(0.0);
  let changed = STORE.with_borrow_mut(|store| {
    let mut changed = BTreeSet::new();
    for setting in Setting::ALL {
      if setting.differs(&store.settings, &settings) {
        changed.append(&mut store.readers[setting as usize]);
      }
    }
    store.settings = settings;
    changed
  });
  if !changed.is_empty() {
    reactive::mark(changed);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_defaults_are_used_until_settings_are_set() {
    set_system_settings(SystemSettings::default());
    assert!(!reduce_motion());
    assert_eq!(text_scale(), 1.0);
    assert_eq!(caret_blink(), Some(Duration::from_millis(530)));
    assert_eq!(double_click_time(), Duration::from_millis(500));
    assert_eq!(double_click_distance(), 4.0);
    assert!(!overlay_scrollbars());
  }

  #[test]
  fn odd_values_are_kept_in_range() {
    let set = |scale, distance| {
      set_system_settings(SystemSettings {
        text_scale: scale,
        double_click_distance: distance,
        ..SystemSettings::default()
      });
      (text_scale(), double_click_distance())
    };
    assert_eq!(set(10.0, -3.0), (MAX_TEXT_SCALE, 0.0));
    assert_eq!(set(0.1, 6.0), (MIN_TEXT_SCALE, 6.0));
    assert_eq!(set(f32::NAN, f32::NAN), (1.0, 0.0));
    assert_eq!(set(1.25, 4.0), (1.25, 4.0));
    set_system_settings(SystemSettings::default());
  }
}
