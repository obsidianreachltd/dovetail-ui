//! `--self-test`: runs the M01 checks without anyone at the keyboard.
//!
//! It covers checklist items 1, 2 and 4–7 in the M01 plan. Item 3 (moving
//! the window to a display with a different scale) still needs a person.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail_desktop::{
  Application, Context, Frame, LogicalSize, WindowId, WindowInfo, WindowOptions,
};
use dovetail_render::Color;

/// Set when any check fails, so `main` can return a failure exit code.
pub static FAILED: AtomicBool = AtomicBool::new(false);

/// How long one step may take before it counts as failed.
const STEP_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the window stays minimised.
const MINIMISED_FOR: Duration = Duration::from_secs(1);
/// How often to ask a restored window for a frame. Straight after
/// restoring, macOS may still count it as hidden and skip the frame.
const RESTORE_RETRY: Duration = Duration::from_millis(500);

const RESIZED_TO: LogicalSize = LogicalSize::new(640.0, 480.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
  Open,
  Resize,
  Minimise,
  Restore,
  DeviceLoss,
  SecondWindow,
  Finished,
}

pub struct SelfTest {
  step: Step,
  first: Option<WindowId>,
  second: Option<WindowId>,
  new_size_seen: bool,
  zero_size_seen: bool,
  /// Frames asked for since restoring.
  restore_asks: u32,
  passed: usize,
}

impl SelfTest {
  pub fn new() -> Self {
    Self {
      step: Step::Open,
      first: None,
      second: None,
      new_size_seen: false,
      zero_size_seen: false,
      restore_asks: 0,
      passed: 0,
    }
  }

  fn pass(
    &mut self,
    item: u8,
    what: &str,
  ) {
    self.passed += 1;
    println!("PASS  {item}. {what}");
  }

  fn fail(
    &mut self,
    cx: &mut Context<'_>,
    what: &str,
  ) {
    FAILED.store(true, Ordering::Relaxed);
    println!("FAIL  {what}");
    self.finish(cx);
  }

  fn finish(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    self.step = Step::Finished;
    println!(
      "{} of 6 automatic checks passed. Item 3 (display scale) is manual.",
      self.passed
    );
    for window in [self.second.take(), self.first.take()]
      .into_iter()
      .flatten()
    {
      cx.close_window(window);
    }
    cx.exit();
  }

  fn go(
    &mut self,
    cx: &Context<'_>,
    step: Step,
    timeout: Duration,
  ) {
    self.step = step;
    cx.wake_after(timeout);
  }
}

impl Application for SelfTest {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    match cx.open_window(WindowOptions::new("Dovetail self-test")) {
      Ok(id) => {
        self.first = Some(id);
        self.go(cx, Step::Open, STEP_TIMEOUT);
      },
      Err(e) => self.fail(cx, &format!("1. could not open a window: {e}")),
    }
  }

  fn draw(
    &mut self,
    _cx: &mut Context<'_>,
    window: WindowId,
    frame: &mut Frame,
  ) {
    let colour = if Some(window) == self.second {
      Color::hex(0x40a02b)
    } else {
      Color::hex(0x1e66f5)
    };
    frame.clear(colour);
  }

  fn resized(
    &mut self,
    _cx: &mut Context<'_>,
    window: WindowId,
    info: WindowInfo,
  ) {
    if Some(window) != self.first {
      return;
    }
    if info.size.is_empty() {
      self.zero_size_seen = true;
    }
    let logical = info.logical_size();
    let close = |a: f64, b: f64| (a - b).abs() <= 1.0;
    if self.step == Step::Resize
      && close(logical.width, RESIZED_TO.width)
      && close(logical.height, RESIZED_TO.height)
      && info.size == RESIZED_TO.to_physical(info.scale_factor)
    {
      self.new_size_seen = true;
    }
  }

  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    let first = Some(window) == self.first;
    match self.step {
      Step::Open if first => {
        let gpu = cx
          .gpu_info()
          .map(|i| format!("{} ({:?})", i.name, i.backend))
          .unwrap_or_default();
        let info = cx.window_info(window).expect("open window");
        self.pass(
          1,
          &format!(
            "window opened and cleared: {gpu}, {}x{} px at scale {}",
            info.size.width, info.size.height, info.scale_factor
          ),
        );
        cx.set_window_size(window, RESIZED_TO);
        self.go(cx, Step::Resize, STEP_TIMEOUT);
      },
      Step::Resize if first && self.new_size_seen => {
        let info = cx.window_info(window).expect("open window");
        self.pass(
          2,
          &format!(
            "resized and redrawn at {}x{} px",
            info.size.width, info.size.height
          ),
        );
        cx.set_minimized(window, true);
        self.go(cx, Step::Minimise, MINIMISED_FOR);
      },
      Step::Restore if first => {
        let note = if self.zero_size_seen {
          "the platform reported a zero size, and drawing was skipped"
        } else {
          "the platform did not report a zero size"
        };
        self.pass(
          4,
          &format!("minimised and restored without errors ({note})"),
        );
        cx.simulate_device_loss();
        self.go(cx, Step::DeviceLoss, STEP_TIMEOUT);
      },
      Step::DeviceLoss if first && cx.gpu_generation() == 1 => {
        self.pass(5, "GPU device lost, recreated, and the window drew again");
        match cx.open_window(WindowOptions::new("Dovetail self-test 2")) {
          Ok(id) => {
            self.second = Some(id);
            self.go(cx, Step::SecondWindow, STEP_TIMEOUT);
          },
          Err(e) => {
            self.fail(cx, &format!("6. could not open a second window: {e}"))
          },
        }
      },
      Step::SecondWindow if !first => {
        if cx.window_count() == 2 && cx.gpu_generation() == 1 {
          self.pass(6, "second window drew with the same GPU device");
        } else {
          return self
            .fail(cx, "6. second window did not share the GPU device");
        }
        let second = self.second.take().expect("second window");
        cx.close_window(second);
        if cx.window_count() == 1 {
          self.pass(7, "closing one window left the other open");
        } else {
          return self
            .fail(cx, "7. closing one window closed the wrong number");
        }
        self.finish(cx);
      },
      _ => {},
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    match self.step {
      Step::Minimise => {
        let first = self.first.expect("first window");
        cx.set_minimized(first, false);
        // Wayland ignores minimising, so no resize may follow. Ask anyway.
        cx.request_redraw(first);
        self.restore_asks = 1;
        self.go(cx, Step::Restore, RESTORE_RETRY);
      },
      Step::Restore if RESTORE_RETRY * self.restore_asks < STEP_TIMEOUT => {
        let first = self.first.expect("first window");
        cx.request_redraw(first);
        self.restore_asks += 1;
        self.go(cx, Step::Restore, RESTORE_RETRY);
      },
      Step::Finished => {},
      step => self.fail(cx, &format!("step {step:?} timed out")),
    }
  }

  fn close_requested(
    &mut self,
    cx: &mut Context<'_>,
    _window: WindowId,
  ) {
    self.fail(cx, "a window was closed during the self-test");
  }
}
