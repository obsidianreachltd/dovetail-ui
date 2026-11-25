//! M20 prototype: can a test written as straight-line code drive real
//! windows, from `cargo test`, on Windows, Linux and macOS?
//!
//! A [`Driver`] owns the one winit event loop a process may have. A test
//! asks it to open, resize and close windows, and waits for what it
//! expects. Waiting runs the event loop a step at a time with winit's
//! `pump_app_events`, so the test keeps control between steps, instead of
//! handing the loop to `run_app` and reacting in callbacks as the
//! examples' self-tests do.
//!
//! Throw-away code: it answers the questions in the M20 plan's step 2.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dovetail_render::{Color, FrameOutcome, Gpu, PhysicalSize, WindowSurface};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{
  ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle,
};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowAttributes, WindowId};

/// How long one pump may wait for events.
const PUMP: Duration = Duration::from_millis(5);

/// What a test can see of one window.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seen {
  /// Frames drawn and shown.
  pub frames: u32,
  /// The last size reported, in physical pixels.
  pub size: PhysicalSize,
  /// The display scale.
  pub scale: f64,
  /// The window has been closed.
  pub closed: bool,
  /// `RedrawRequested` events received.
  pub redraws: u32,
  /// Frames asked for that were skipped, as when the surface had no image.
  pub skipped: u32,
  /// `Occluded` events: hidden (true) or shown (false), the last one.
  pub occluded: Option<bool>,
  /// The last `Focused` event.
  pub focused: Option<bool>,
}

enum Command {
  Open {
    title: String,
    size: (f64, f64),
    colour: Color,
  },
  Resize(WindowId, (f64, f64)),
  Close(WindowId),
}

struct Win {
  window: Arc<Window>,
  surface: WindowSurface,
  colour: Color,
}

struct Handler {
  display: OwnedDisplayHandle,
  gpu: Option<Gpu>,
  windows: HashMap<WindowId, Win>,
  seen: HashMap<WindowId, Seen>,
  commands: Vec<Command>,
  opened: Vec<WindowId>,
  errors: Vec<String>,
  ready: bool,
  /// Bring each new window to the front and give it focus.
  front: bool,
}

fn physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
  PhysicalSize { width: size.width, height: size.height }
}

impl Handler {
  /// Commands need an `ActiveEventLoop`, which only exists inside a
  /// callback, so the test queues them and they run here.
  fn run_commands(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    if !self.ready {
      return;
    }
    for command in std::mem::take(&mut self.commands) {
      match command {
        Command::Open { title, size, colour } => {
          match self.open(event_loop, &title, size, colour) {
            Ok(id) => self.opened.push(id),
            Err(e) => self.errors.push(e),
          }
        },
        Command::Resize(id, (w, h)) => {
          if let Some(win) = self.windows.get(&id) {
            let _ = win
              .window
              .request_inner_size(winit::dpi::LogicalSize::new(w, h));
            win.window.request_redraw();
          }
        },
        Command::Close(id) => {
          // The surface goes before its window.
          self.windows.remove(&id);
          if let Some(seen) = self.seen.get_mut(&id) {
            seen.closed = true;
          }
        },
      }
    }
  }

  fn open(
    &mut self,
    event_loop: &ActiveEventLoop,
    title: &str,
    (w, h): (f64, f64),
    colour: Color,
  ) -> Result<WindowId, String> {
    let attributes = WindowAttributes::default()
      .with_title(title)
      .with_inner_size(winit::dpi::LogicalSize::new(w, h));
    let window = Arc::new(
      event_loop
        .create_window(attributes)
        .map_err(|e| format!("could not open a window: {e}"))?,
    );
    let size = physical(window.inner_size());
    let surface = match &self.gpu {
      Some(gpu) => WindowSurface::new(gpu, Arc::clone(&window), size)
        .map_err(|e| format!("could not make a surface: {e:?}"))?,
      None => {
        let (gpu, surface) = Gpu::with_window(
          Box::new(self.display.clone()),
          Arc::clone(&window),
          size,
        )
        .map_err(|e| format!("could not set up the GPU: {e:?}"))?;
        self.gpu = Some(gpu);
        surface
      },
    };
    let id = window.id();
    self.seen.insert(
      id,
      Seen {
        size,
        scale: window.scale_factor(),
        ..Seen::default()
      },
    );
    if self.front {
      window.focus_window();
    }
    window.request_redraw();
    self
      .windows
      .insert(id, Win { window, surface, colour });
    Ok(id)
  }
}

impl ApplicationHandler for Handler {
  fn resumed(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    self.ready = true;
    self.run_commands(event_loop);
  }

  fn new_events(
    &mut self,
    event_loop: &ActiveEventLoop,
    _: StartCause,
  ) {
    self.run_commands(event_loop);
  }

  fn window_event(
    &mut self,
    _: &ActiveEventLoop,
    id: WindowId,
    event: WindowEvent,
  ) {
    let (Some(gpu), Some(win)) = (&self.gpu, self.windows.get_mut(&id)) else {
      return;
    };
    match event {
      WindowEvent::RedrawRequested => {
        let outcome = win.surface.draw_clear(gpu, win.colour);
        if let Some(seen) = self.seen.get_mut(&id) {
          seen.redraws += 1;
          match outcome {
            FrameOutcome::Presented => seen.frames += 1,
            FrameOutcome::Skipped => seen.skipped += 1,
            FrameOutcome::Retry | FrameOutcome::DeviceLost => {},
          }
        }
        if outcome == FrameOutcome::Retry {
          win.window.request_redraw();
        }
      },
      WindowEvent::Occluded(hidden) => {
        if let Some(seen) = self.seen.get_mut(&id) {
          seen.occluded = Some(hidden);
        }
        if !hidden {
          win.window.request_redraw();
        }
      },
      WindowEvent::Focused(focused) => {
        if let Some(seen) = self.seen.get_mut(&id) {
          seen.focused = Some(focused);
        }
      },
      WindowEvent::Resized(size) => {
        let size = physical(size);
        win.surface.resize(gpu, size);
        if let Some(seen) = self.seen.get_mut(&id) {
          seen.size = size;
        }
        win.window.request_redraw();
      },
      WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
        if let Some(seen) = self.seen.get_mut(&id) {
          seen.scale = scale_factor;
        }
      },
      _ => {},
    }
  }

  fn about_to_wait(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    self.run_commands(event_loop);
    event_loop.set_control_flow(ControlFlow::Wait);
  }
}

/// Drives windows for tests, one step at a time.
pub struct Driver {
  event_loop: EventLoop<()>,
  handler: Handler,
}

impl Driver {
  /// Makes the event loop. A process may make only one.
  pub fn new() -> Result<Self, String> {
    let event_loop = EventLoop::new()
      .map_err(|e| format!("could not make the event loop: {e}"))?;
    let handler = Handler {
      display: event_loop.owned_display_handle(),
      gpu: None,
      windows: HashMap::new(),
      seen: HashMap::new(),
      commands: Vec::new(),
      opened: Vec::new(),
      errors: Vec::new(),
      ready: false,
      front: std::env::var_os("PROTOTYPE_FRONT").is_some(),
    };
    Ok(Self { event_loop, handler })
  }

  /// Runs the event loop once, for at most a few milliseconds.
  pub fn pump(&mut self) -> Result<(), String> {
    match self
      .event_loop
      .pump_app_events(Some(PUMP), &mut self.handler)
    {
      PumpStatus::Continue => {},
      PumpStatus::Exit(code) => {
        return Err(format!("the event loop ended with code {code}"));
      },
    }
    match self.handler.errors.pop() {
      Some(e) => Err(e),
      None => Ok(()),
    }
  }

  /// Pumps until `done` is true, or fails with `what` after `limit`.
  pub fn wait_until(
    &mut self,
    what: &str,
    limit: Duration,
    mut done: impl FnMut(&Self) -> bool,
  ) -> Result<Duration, String> {
    let start = Instant::now();
    loop {
      if done(self) {
        return Ok(start.elapsed());
      }
      if start.elapsed() > limit {
        return Err(format!("timed out after {limit:?} waiting for {what}"));
      }
      self.pump()?;
    }
  }

  /// Opens a window of `size` logical pixels, cleared to `colour`, and
  /// waits until it exists.
  pub fn open(
    &mut self,
    title: &str,
    size: (f64, f64),
    colour: Color,
  ) -> Result<WindowId, String> {
    let before = self.handler.opened.len();
    self.handler.commands.push(Command::Open {
      title: title.to_owned(),
      size,
      colour,
    });
    self.wait_until("the window to open", Duration::from_secs(10), |d| {
      d.handler.opened.len() > before
    })?;
    Ok(*self.handler.opened.last().expect("opened"))
  }

  /// Asks for a new size, in logical pixels.
  pub fn resize(
    &mut self,
    id: WindowId,
    size: (f64, f64),
  ) {
    self
      .handler
      .commands
      .push(Command::Resize(id, size));
  }

  /// Closes a window and waits until it is gone.
  pub fn close(
    &mut self,
    id: WindowId,
  ) -> Result<(), String> {
    self.handler.commands.push(Command::Close(id));
    self.wait_until("the window to close", Duration::from_secs(10), |d| {
      d.seen(id).closed
    })?;
    Ok(())
  }

  /// What the test can see of a window.
  pub fn seen(
    &self,
    id: WindowId,
  ) -> Seen {
    self
      .handler
      .seen
      .get(&id)
      .copied()
      .unwrap_or_default()
  }

  /// The GPU adapter, once a window has opened.
  pub fn adapter(&self) -> Option<String> {
    self.handler.gpu.as_ref().map(|g| {
      let info = g.adapter_info();
      format!("{} ({:?})", info.name, info.backend)
    })
  }

  /// Windows still open.
  pub fn open_windows(&self) -> usize {
    self.handler.windows.len()
  }

  /// Whether new windows are brought to the front (`PROTOTYPE_FRONT`).
  pub fn brings_to_front(&self) -> bool {
    self.handler.front
  }

  /// One line for each open window: what it has received, for a failure
  /// message.
  pub fn describe(&self) -> Vec<String> {
    self
      .handler
      .windows
      .iter()
      .map(|(id, win)| {
        let s = self.seen(*id);
        format!(
          "{:?}: {} redraws, {} frames, {} skipped, occluded {:?},            focused {:?}, visible {:?}, {}x{} px",
          win.window.title(),
          s.redraws,
          s.frames,
          s.skipped,
          s.occluded,
          s.focused,
          win.window.is_visible(),
          s.size.width,
          s.size.height
        )
      })
      .collect()
  }

  /// Closes every open window, so the next test starts with none.
  pub fn close_all(&mut self) -> Result<(), String> {
    let ids: Vec<WindowId> = self.handler.windows.keys().copied().collect();
    for id in ids {
      self.close(id)?;
    }
    Ok(())
  }
}

impl Drop for Driver {
  fn drop(&mut self) {
    // Surfaces go before the GPU.
    self.handler.windows.clear();
    self.handler.gpu = None;
  }
}

/// Tries to make a second event loop, which winit should refuse.
pub fn second_event_loop() -> Result<(), String> {
  EventLoop::new()
    .map(|_| ())
    .map_err(|e| e.to_string())
}
