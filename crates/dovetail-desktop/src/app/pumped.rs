//! The event loop driven a step at a time, for tests that use real
//! windows (M20, decision D0026).
//!
//! winit allows one event loop for each process, and on macOS it must run
//! on the main thread. A test program made with `harness = false` makes
//! one [`Pumped`] loop in `main`, and its tests take turns with it. Each
//! test asks for work to be done with a [`Context`], and waits by pumping
//! the loop: the work runs the next time the loop goes round, before it
//! waits, so the test keeps control between steps.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use winit::event_loop::EventLoop;
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};

use super::{
  Application, Context, Error, Runner, UserEvent, finish, lifecycle, start,
};

/// How long one step of the loop may wait for events.
const STEP: Duration = Duration::from_millis(5);

/// The application a pumped loop runs: it does nothing of its own.
struct Nothing;

impl Application for Nothing {
  fn started(
    &mut self,
    _: &mut Context<'_>,
  ) {
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: super::WindowId,
    _: &mut super::Frame,
  ) {
  }
}

/// An event loop that a test drives one step at a time.
///
/// Closing the last window does not end it, so one loop serves every test
/// in a program. The clipboard is kept in memory, and there are no native
/// menus or file dialogs, so a test reaches nothing outside its windows.
pub struct Pumped {
  event_loop: EventLoop<UserEvent>,
  runner: Runner<Nothing>,
  _tasks: lifecycle::TaskWaker,
}

impl Pumped {
  /// Makes the event loop. A process may make only one, and on macOS only
  /// on the main thread.
  pub fn new() -> Result<Self, Error> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
      .build()
      .map_err(Error::EventLoop)?;
    let (shell, tasks) = start(&event_loop, true);
    let runner = Runner {
      app: Nothing,
      shell,
      started: false,
      queued: Vec::new(),
    };
    Ok(Self { event_loop, runner, _tasks: tasks })
  }

  /// Lets the loop go round once, waiting a few milliseconds at most for
  /// events.
  pub fn pump(&mut self) -> Result<(), Error> {
    match self
      .event_loop
      .pump_app_events(Some(STEP), &mut self.runner)
    {
      PumpStatus::Continue => Ok(()),
      PumpStatus::Exit(code) => Err(Error::Ended(code)),
    }
  }

  /// Runs `work` with a [`Context`] the next time the loop goes round, and
  /// returns what it gives.
  pub fn with<R: 'static>(
    &mut self,
    work: impl FnOnce(&mut Context<'_>) -> R + 'static,
  ) -> Result<R, Error> {
    let slot = Rc::new(RefCell::new(None));
    let out = Rc::clone(&slot);
    self.runner.queued.push(Box::new(move |cx| {
      *out.borrow_mut() = Some(work(cx));
    }));
    // The work runs before the loop waits, so one or two turns do it.
    for _ in 0..1000 {
      self.pump()?;
      if let Some(result) = slot.borrow_mut().take() {
        return Ok(result);
      }
    }
    Err(Error::Ended(-1))
  }
}

impl Drop for Pumped {
  fn drop(&mut self) {
    finish(&mut self.runner);
  }
}
