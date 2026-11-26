//! M18 step 5: frames for animation, with no window. A component that
//! asks for frames is built again at each, at 60 frames a second of the
//! test's time, and nothing is built when nothing asks.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use dovetail::prelude::*;
use dovetail_core::{frame_requested, now, request_frame};
use dovetail_testing::{FRAME, TestUi, text};

thread_local! {
  /// When the mover was built, each time.
  static BUILDS: RefCell<Vec<Instant>> = const { RefCell::new(Vec::new()) };
}

fn builds() -> Vec<Instant> {
  BUILDS.with_borrow_mut(std::mem::take)
}

const MOVE_FOR: Duration = Duration::from_millis(100);

/// Moves for 100 ms after it starts, asking for a frame each time it is
/// built until then.
#[component]
fn Mover() -> View {
  let start = use_state(now).get();
  let elapsed = now() - start;
  BUILDS.with_borrow_mut(|b| b.push(now()));
  let done = elapsed >= MOVE_FOR;
  if !done {
    request_frame();
  }
  ui! {
    Text { value: if done { "Done".to_string() } else { format!("{} ms", elapsed.as_millis()) } }
  }
}

#[test]
fn frames_come_while_asked_for_and_stop_after() {
  let mut ui = TestUi::new(ui! { Mover {} });
  let start = builds()[0];
  assert!(frame_requested());

  ui.advance(Duration::from_millis(300));
  ui.find(text("Done"));
  let times = builds();
  // A frame every 60th of a second, from the first.
  for (n, at) in times.iter().enumerate() {
    let expected = FRAME * (n as u32 + 1);
    let off = (*at - start).abs_diff(expected);
    assert!(off < Duration::from_micros(10), "frame {n}: {off:?}");
  }
  // It ends within one frame of the time asked.
  let last = *times.last().unwrap() - start;
  assert!(last >= MOVE_FOR && last < MOVE_FOR + FRAME, "{last:?}");
  assert_eq!(times.len(), 6, "frames at 17, 33, 50, 67, 83 and 100 ms");

  // Nothing asks now, so nothing is built.
  assert!(!frame_requested());
  ui.advance(Duration::from_secs(1));
  assert!(builds().is_empty());
}

#[test]
fn a_test_that_moves_the_clock_a_little_gets_the_frames_due() {
  let mut ui = TestUi::new(ui! { Mover {} });
  builds();
  ui.advance(Duration::from_millis(10));
  assert!(builds().is_empty(), "the first frame is at 17 ms");
  ui.advance(Duration::from_millis(10));
  assert_eq!(builds().len(), 1);
  ui.find(text("16 ms"));
}
