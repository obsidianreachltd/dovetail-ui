//! The self-test, in a real window (M20, `TestWindows`):
//!
//! - each setting, set from inside the program, reaches the values shown,
//!   and changes what it should: less motion makes the box jump, the text
//!   scale grows the text, and the double-click time decides whether two
//!   clicks are a double-click;
//! - the box ends its move at the time asked, within one frame;
//! - no frames are drawn while nothing moves: with less motion set, when
//!   the spinner is still too.
//!
//! The system's own settings are never changed. Each check has a time
//! limit, so a window that stops drawing fails rather than hangs.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use dovetail::prelude::*;
use dovetail::{Role, SystemSettings, set_system_settings};
use dovetail_testing::windows::{TestWindow, TestWindows};
use dovetail_testing::{role, text};

use crate::{MOVE_BY, MOVE_TIME, MOVES};

/// Sets the settings on the UI thread, as the desktop does when it reads
/// the system's, and waits for the window to show `shows`.
fn set(
  t: &mut TestWindows,
  w: TestWindow,
  settings: SystemSettings,
  shows: &'static str,
) {
  t.with(move |_| set_system_settings(settings));
  t.find(w, text(shows));
  println!("PASS  the window shows \"{shows}\"");
}

/// The box's builds since "Move" was last pressed.
fn moves() -> Vec<(Instant, f32)> {
  MOVES.with_borrow(Clone::clone)
}

/// Presses "Move" and waits for the box to arrive at `to`. Returns how
/// long it took, by the frames' own times, and the longest gap between
/// two frames on the way.
fn move_box(
  t: &mut TestWindows,
  w: TestWindow,
  to: f32,
) -> (Duration, Duration) {
  t.click(w, role(Role::Button).label("Move"));
  // Waits for the exact value: the text is rounded, so "Box at 200 px"
  // shows a frame before the box arrives.
  let shows = format!("Box at {to:.0} px");
  t.wait_until("the box to arrive", |t| {
    t.has(w, text(shows.as_str()))
      && moves().last().is_some_and(|(_, at)| *at == to)
  });
  let seen = moves();
  let start = seen.first().expect("a build when pressed").0;
  let end = seen
    .iter()
    .find(|(_, at)| *at == to)
    .expect("a build at the end")
    .0;
  let gap = seen
    .windows(2)
    .map(|p| p[1].0 - p[0].0)
    .max()
    .unwrap_or_default();
  (end - start, gap)
}

pub fn run() -> ExitCode {
  let mut t = TestWindows::new();
  let w = t.open("Motion self-test", (640.0, 560.0), ui! { crate::App {} });
  let normal = SystemSettings::default();
  set(&mut t, w, normal, "Less motion: off");

  // The box ends on time, within one frame, and frames stop after.
  let (took, gap) = move_box(&mut t, w, MOVE_BY);
  let frames = moves().len();
  assert!(
    took >= MOVE_TIME && took <= MOVE_TIME + gap,
    "the move took {took:?}, with frames up to {gap:?} apart"
  );
  assert!(frames >= 5, "only {frames} frames on the way");
  println!(
    "PASS  the box moved in {took:.0?} for {MOVE_TIME:?}, in {frames} frames, \
     each at most {gap:.0?} apart"
  );
  // Less motion: the box jumps, and nothing is drawn after.
  set(
    &mut t,
    w,
    SystemSettings { reduce_motion: true, ..normal },
    "Less motion: on",
  );
  let (took, _) = move_box(&mut t, w, 0.0);
  assert_eq!(took, Duration::ZERO, "the box jumps");
  assert_eq!(moves().len(), 1, "in one build");
  let still = t.stats(w).presented;
  t.pause(Duration::from_millis(400));
  assert_eq!(
    t.stats(w).presented,
    still,
    "no frames: the spinner is still too"
  );
  println!("PASS  with less motion the box jumps and no frames are drawn");

  // The text scale grows the theme's text, and the controls with it.
  let before = t.find(w, text("Box at 0 px")).rect().height;
  set(
    &mut t,
    w,
    SystemSettings { text_scale: 1.5, ..normal },
    "Text scale: 1.50",
  );
  let after = t.find(w, text("Box at 0 px")).rect().height;
  assert!(
    after > before * 1.3,
    "the label grew from {before} to {after}"
  );
  println!("PASS  the text grew from {before} to {after} pixels high");

  // The caret blink: off, then slower.
  set(
    &mut t,
    w,
    SystemSettings { caret_blink: None, ..normal },
    "Caret blink: off",
  );
  set(
    &mut t,
    w,
    SystemSettings {
      caret_blink: Some(Duration::from_millis(1000)),
      ..normal
    },
    "Caret blink: 1000 ms",
  );

  // Two clicks 150 ms apart: a double-click with 500 ms, two single
  // clicks with 50 ms.
  let pad = || role(Role::Button).label("Pad");
  let two_clicks = |t: &mut TestWindows| {
    t.click(w, pad());
    t.pause(Duration::from_millis(150));
    t.click(w, pad());
  };
  set(&mut t, w, normal, "Double-click: 500 ms, 4 px");
  two_clicks(&mut t);
  t.find(w, text("Pad clicks: 2"));
  set(
    &mut t,
    w,
    SystemSettings {
      double_click_time: Duration::from_millis(50),
      ..normal
    },
    "Double-click: 50 ms, 4 px",
  );
  // A pause first, so the next click starts afresh.
  t.pause(Duration::from_millis(600));
  two_clicks(&mut t);
  t.find(w, text("Pad clicks: 1"));
  println!(
    "PASS  the double-click time decides whether two clicks are a double-click"
  );

  set(
    &mut t,
    w,
    SystemSettings { double_click_distance: 2.0, ..normal },
    "Double-click: 500 ms, 2 px",
  );
  set(
    &mut t,
    w,
    SystemSettings { overlay_scrollbars: true, ..normal },
    "Overlay scroll bars: yes",
  );

  t.with(move |_| set_system_settings(SystemSettings::default()));
  t.close(w);
  println!(
    "PASS  every setting reaches the window, and motion follows less motion"
  );
  ExitCode::SUCCESS
}
