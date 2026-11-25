//! The time on the UI thread (M20, decision D0027).
//!
//! Code on the UI thread reads the time with [`now`], not
//! `Instant::now()`: double-clicks, the caret blink, hiding scroll bars,
//! type-ahead, recovery's idle wait, and timers from [`Task::after`]. It is
//! the real time unless a test installs another clock with [`set_clock`].
//! A [`TestClock`] stays still until the test moves it forward, and moving
//! it fires the timers that fall due, in order.
//!
//! Work on other threads, such as [`TaskContext::sleep`], keeps real time.
//!
//! [`Task::after`]: crate::Task::after
//! [`TaskContext::sleep`]: crate::TaskContext::sleep

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Where the UI thread reads the time.
pub trait Clock {
  /// The time now.
  fn now(&self) -> Instant;
}

thread_local! {
  static CLOCK: RefCell<Option<Rc<dyn Clock>>> = const { RefCell::new(None) };
}

/// The time now on the UI thread: the real time, or the installed clock's.
pub fn now() -> Instant {
  let clock = CLOCK.with_borrow(Clone::clone);
  match clock {
    Some(clock) => clock.now(),
    // real time: no clock is installed.
    None => Instant::now(),
  }
}

/// Installs a clock for this thread, or goes back to the real time with
/// `None`.
pub fn set_clock(clock: Option<Rc<dyn Clock>>) {
  CLOCK.with_borrow_mut(|c| *c = clock);
}

/// A clock that stays still until it is moved forward, for tests.
///
/// ```
/// use std::time::Duration;
/// use dovetail_core::{TestClock, now};
///
/// let clock = TestClock::install();
/// let start = now();
/// clock.advance(Duration::from_millis(500));
/// assert_eq!(now() - start, Duration::from_millis(500));
/// # dovetail_core::set_clock(None);
/// ```
pub struct TestClock {
  time: Cell<Instant>,
}

impl TestClock {
  /// A clock stopped at the real time now.
  pub fn new() -> Rc<TestClock> {
    // real time: where the test clock starts.
    Rc::new(TestClock { time: Cell::new(Instant::now()) })
  }

  /// A new clock, installed for this thread with [`set_clock`].
  pub fn install() -> Rc<TestClock> {
    let clock = TestClock::new();
    set_clock(Some(Rc::clone(&clock) as Rc<dyn Clock>));
    clock
  }

  /// Moves the clock forward by `time`. Each timer that falls due on the
  /// way fires in order, with the clock at its time. Returns how many
  /// fired.
  pub fn advance(
    &self,
    time: Duration,
  ) -> usize {
    let target = self.time.get() + time;
    let mut fired = 0;
    while let Some(due) = crate::task::next_due_by(target) {
      if due > self.time.get() {
        self.time.set(due);
      }
      fired += crate::task::run_timers_due(self.time.get());
    }
    self.time.set(target);
    fired
  }
}

impl Clock for TestClock {
  fn now(&self) -> Instant {
    self.time.get()
  }
}
