//! M19 prototype: how should a modal window block its owner, and how does
//! the program find out that the monitors changed (D0029)?
//!
//! An owner window (blue) opens a modal window (green) with `M`. How the
//! owner is blocked depends on `--block`:
//!
//! - `dovetail`: the program refuses the owner's presses, wheel, keys,
//!   text and close itself, and brings the modal window to the front when
//!   the owner is pressed. The system knows nothing of the modal window.
//! - `system`: the system's ownership only. On Windows the modal window is
//!   owned by the owner (`with_owner_window`); on X11 it has the dialog
//!   window type. Nothing is refused.
//! - `both` (the default): both, as D0029 proposes.
//!
//! While it is blocked the owner turns grey. Every press, key and focus
//! change is logged to stderr, with whether it was refused.
//!
//! The monitor list is read again on each focus, move, resize and scale
//! change, and on a timer (`--poll-ms`, 1000 by default). Each change is
//! logged with what saw it first, which shows whether window events alone
//! would have found it.
//!
//! `--check` runs without a person: it opens both windows, checks that the
//! owner is blocked and the modal window is centred on it, closes the
//! modal window, and checks the owner is free again.
//!
//! Throw-away code, with no `unsafe`: it answers the open questions in the
//! M19 plan. The work area and the Windows `EnableWindow` fallback need
//! platform code, so they are not tried here.

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dovetail_render::{Color, FrameOutcome, Gpu, PhysicalSize, WindowSurface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{
  ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle,
};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const BLUE: Color = Color::hex(0x1e66f5);
const GREY: Color = Color::hex(0x6c6f85);
const GREEN: Color = Color::hex(0x40a02b);

/// How the owner is blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mode {
  /// The program refuses the owner's input.
  dovetail: bool,
  /// The system is told the modal window belongs to the owner.
  system: bool,
}

/// The input a window can get, as the blocking rule sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
  Press,
  Release,
  Wheel,
  Key,
  Text,
  Close,
  Focus,
  Other,
}

/// What to do with input to the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
  Pass,
  Refuse,
  /// Refuse it, and bring the modal window to the front.
  RefuseAndRaise,
}

/// The blocking rule D0029 proposes. Hover, focus and painting go on, so
/// the owner still draws; everything that would change it is refused.
fn judge(
  input: Input,
  blocked: bool,
) -> Verdict {
  if !blocked {
    return Verdict::Pass;
  }
  match input {
    Input::Press | Input::Close => Verdict::RefuseAndRaise,
    Input::Release | Input::Wheel | Input::Key | Input::Text => Verdict::Refuse,
    Input::Focus | Input::Other => Verdict::Pass,
  }
}

fn input_of(event: &WindowEvent) -> Input {
  match event {
    WindowEvent::MouseInput { state: ElementState::Pressed, .. } => {
      Input::Press
    },
    WindowEvent::MouseInput { .. } => Input::Release,
    WindowEvent::MouseWheel { .. } => Input::Wheel,
    WindowEvent::KeyboardInput { .. } => Input::Key,
    WindowEvent::Ime(_) => Input::Text,
    WindowEvent::CloseRequested => Input::Close,
    WindowEvent::Focused(_) => Input::Focus,
    _ => Input::Other,
  }
}

/// One monitor, as winit gives it.
#[derive(Clone, Debug, PartialEq)]
struct Monitor {
  name: Option<String>,
  position: (i32, i32),
  size: (u32, u32),
  scale: f64,
  primary: bool,
}

fn read_monitors(event_loop: &ActiveEventLoop) -> Vec<Monitor> {
  let primary = event_loop.primary_monitor();
  event_loop
    .available_monitors()
    .map(|m| Monitor {
      name: m.name(),
      position: (m.position().x, m.position().y),
      size: (m.size().width, m.size().height),
      scale: m.scale_factor(),
      primary: primary.as_ref() == Some(&m),
    })
    .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
  Owner,
  Modal,
}

struct Win {
  window: Arc<Window>,
  surface: WindowSurface,
  role: Role,
}

/// What `--check` is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
  OwnerOpen,
  ModalOpen,
  ModalClosed,
  Done,
}

struct App {
  mode: Mode,
  check: Option<Step>,
  failures: Vec<String>,
  start: Instant,
  display: OwnedDisplayHandle,
  gpu: Option<Gpu>,
  windows: HashMap<WindowId, Win>,
  owner: Option<WindowId>,
  modal: Option<WindowId>,
  open_modal: bool,
  close_modal: bool,
  monitors: Vec<Monitor>,
  poll: Duration,
  next_poll: Instant,
  deadline: Option<Instant>,
}

fn physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
  PhysicalSize { width: size.width, height: size.height }
}

/// Tells the system the new window belongs to `owner`, where winit can
/// without platform code.
#[cfg(windows)]
fn owned_by(
  attributes: WindowAttributes,
  owner: &Window,
) -> WindowAttributes {
  use winit::platform::windows::WindowAttributesExtWindows;
  use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
  match owner.window_handle().map(|h| h.as_raw()) {
    Ok(RawWindowHandle::Win32(h)) => attributes.with_owner_window(h.hwnd.get()),
    _ => attributes,
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn owned_by(
  attributes: WindowAttributes,
  _owner: &Window,
) -> WindowAttributes {
  use winit::platform::x11::{WindowAttributesExtX11, WindowType};
  // Ignored on Wayland. `WM_TRANSIENT_FOR` and `_NET_WM_STATE_MODAL` need
  // platform code.
  attributes.with_x11_window_type(vec![WindowType::Dialog])
}

/// macOS needs `addChildWindow`, which is platform code.
#[cfg(target_os = "macos")]
fn owned_by(
  attributes: WindowAttributes,
  _owner: &Window,
) -> WindowAttributes {
  attributes
}

impl App {
  fn log(
    &self,
    what: impl AsRef<str>,
  ) {
    let at = self.start.elapsed().as_secs_f64();
    eprintln!("{at:8.3}  {}", what.as_ref());
  }

  fn blocked(&self) -> bool {
    self.mode.dovetail && self.modal.is_some()
  }

  fn open(
    &mut self,
    event_loop: &ActiveEventLoop,
    attributes: WindowAttributes,
    role: Role,
  ) -> Result<WindowId, String> {
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
    window.request_redraw();
    self
      .windows
      .insert(id, Win { window, surface, role });
    Ok(id)
  }

  fn open_modal(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    let Some(owner) = self.owner.and_then(|id| self.windows.get(&id)) else {
      return;
    };
    let owner = Arc::clone(&owner.window);
    let (w, h) = (360.0, 240.0);
    let mut attributes = WindowAttributes::default()
      .with_title("Modal")
      .with_inner_size(LogicalSize::new(w, h))
      .with_resizable(false);
    // Centred on the owner, in physical pixels.
    if let Ok(at) = owner.outer_position() {
      let size = owner.outer_size();
      let scale = owner.scale_factor();
      let x = at.x + (size.width as i32 - (w * scale) as i32) / 2;
      let y = at.y + (size.height as i32 - (h * scale) as i32) / 2;
      attributes = attributes.with_position(PhysicalPosition::new(x, y));
    }
    if self.mode.system {
      attributes = owned_by(attributes, &owner);
    }
    match self.open(event_loop, attributes, Role::Modal) {
      Ok(id) => {
        self.modal = Some(id);
        self.log(format!("modal opened, blocked: {}", self.blocked()));
        self.redraw_owner();
      },
      Err(e) => self.log(e),
    }
  }

  fn close_modal(&mut self) {
    if let Some(id) = self.modal.take() {
      // The surface goes before its window.
      self.windows.remove(&id);
      self.log("modal closed, the owner is free");
      self.redraw_owner();
      if let Some(owner) = self.owner.and_then(|id| self.windows.get(&id)) {
        owner.window.focus_window();
      }
    }
  }

  fn redraw_owner(&self) {
    if let Some(owner) = self.owner.and_then(|id| self.windows.get(&id)) {
      owner.window.request_redraw();
    }
  }

  fn raise_modal(&self) {
    if let Some(modal) = self.modal.and_then(|id| self.windows.get(&id)) {
      modal.window.focus_window();
    }
  }

  /// Reads the monitors again, and logs a change with what saw it.
  fn reread(
    &mut self,
    event_loop: &ActiveEventLoop,
    seen_by: &str,
  ) {
    let now = read_monitors(event_loop);
    if now != self.monitors {
      self.log(format!("monitors changed, seen by {seen_by}:"));
      for m in &now {
        self.log(format!("    {m:?}"));
      }
      self.monitors = now;
    }
  }

  fn fail(
    &mut self,
    what: String,
  ) {
    self.log(format!("FAIL  {what}"));
    self.failures.push(what);
  }

  /// The `--check` steps, run between events.
  fn run_check(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    let Some(step) = self.check else {
      return;
    };
    let late = self.deadline.is_some_and(|d| Instant::now() > d);
    match step {
      Step::OwnerOpen if self.owner.is_some() => {
        self.open_modal(event_loop);
        self.check = Some(Step::ModalOpen);
        self.deadline = Some(Instant::now() + Duration::from_secs(2));
      },
      Step::ModalOpen if late => {
        if self.mode.dovetail && !self.blocked() {
          self.fail("the owner is not blocked".into());
        }
        for input in [
          Input::Press,
          Input::Key,
          Input::Text,
          Input::Close,
        ] {
          if self.mode.dovetail && judge(input, self.blocked()) == Verdict::Pass
          {
            self.fail(format!("{input:?} to the owner was let through"));
          }
        }
        self.check_centred();
        self.close_modal();
        self.check = Some(Step::ModalClosed);
        self.deadline = Some(Instant::now() + Duration::from_millis(500));
      },
      Step::ModalClosed if late => {
        if self.blocked()
          || judge(Input::Press, self.blocked()) != Verdict::Pass
        {
          self.fail("the owner is still blocked".into());
        }
        self.check = Some(Step::Done);
        event_loop.exit();
      },
      _ => {},
    }
  }

  fn check_centred(&mut self) {
    let place = |id: Option<WindowId>| {
      let w = &self.windows.get(&id?)?.window;
      let at = w.outer_position().ok()?;
      let size = w.outer_size();
      Some((
        at.x + size.width as i32 / 2,
        at.y + size.height as i32 / 2,
        w.scale_factor(),
      ))
    };
    match (place(self.owner), place(self.modal)) {
      (Some((ox, oy, scale)), Some((mx, my, _))) => {
        // Within 40 logical pixels: frames differ between the two.
        let off = (ox - mx).abs().max((oy - my).abs()) as f64 / scale;
        self.log(format!(
          "the modal window's centre is {off:.0} px from the owner's"
        ));
        if off > 40.0 {
          self.fail(format!("the modal window is {off:.0} px off centre"));
        }
      },
      _ => self.log("window positions are not known (Wayland), not checked"),
    }
  }
}

impl ApplicationHandler for App {
  fn resumed(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    if self.owner.is_some() {
      return;
    }
    self.monitors = read_monitors(event_loop);
    self.log(format!("{} monitors at start:", self.monitors.len()));
    for m in &self.monitors.clone() {
      self.log(format!("    {m:?}"));
    }
    let attributes = WindowAttributes::default()
      .with_title("Owner: M opens the modal window, Q quits")
      .with_inner_size(LogicalSize::new(720.0, 480.0));
    match self.open(event_loop, attributes, Role::Owner) {
      Ok(id) => self.owner = Some(id),
      Err(e) => {
        self.fail(e);
        event_loop.exit();
      },
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: WindowId,
    event: WindowEvent,
  ) {
    let Some(role) = self.windows.get(&id).map(|w| w.role) else {
      return;
    };
    match &event {
      WindowEvent::Focused(true) => self.reread(event_loop, "a focus change"),
      WindowEvent::Moved(_) => self.reread(event_loop, "a move"),
      WindowEvent::Resized(_) => self.reread(event_loop, "a resize"),
      WindowEvent::ScaleFactorChanged { .. } => {
        self.reread(event_loop, "a scale change");
      },
      _ => {},
    }

    let input = input_of(&event);
    if role == Role::Owner && !matches!(input, Input::Other) {
      let verdict = judge(input, self.blocked());
      if input == Input::Focus {
        if let WindowEvent::Focused(true) = event
          && self.modal.is_some()
        {
          self.log("owner: came to the front while the modal window is open");
        }
      } else if let WindowEvent::KeyboardInput {
        event: key,
        is_synthetic,
        ..
      } = &event
      {
        self.log(format!(
          "owner: key {:?} {:?}, synthetic {is_synthetic}, {verdict:?}",
          key.logical_key, key.state
        ));
      } else if input != Input::Release {
        self.log(format!("owner: {input:?}, {verdict:?}"));
      }
      match verdict {
        Verdict::Pass => {},
        Verdict::Refuse => return,
        Verdict::RefuseAndRaise => {
          self.raise_modal();
          return;
        },
      }
    }

    match event {
      WindowEvent::RedrawRequested => {
        let Some(gpu) = &self.gpu else {
          return;
        };
        let colour = match role {
          Role::Modal => GREEN,
          Role::Owner if self.modal.is_some() => GREY,
          Role::Owner => BLUE,
        };
        if let Some(win) = self.windows.get_mut(&id)
          && win.surface.draw_clear(gpu, colour) == FrameOutcome::Retry
        {
          win.window.request_redraw();
        }
      },
      WindowEvent::Resized(size) => {
        if let (Some(gpu), Some(win)) = (&self.gpu, self.windows.get_mut(&id)) {
          win.surface.resize(gpu, physical(size));
          win.window.request_redraw();
        }
      },
      WindowEvent::Focused(focused) if role == Role::Modal => {
        self.log(format!("modal: focused {focused}"));
      },
      WindowEvent::CloseRequested => match role {
        Role::Modal => self.close_modal(),
        Role::Owner => event_loop.exit(),
      },
      WindowEvent::KeyboardInput { event, .. }
        if event.state == ElementState::Pressed =>
      {
        match (role, event.logical_key.as_ref()) {
          (Role::Owner, Key::Character("m" | "M")) => self.open_modal = true,
          (Role::Owner, Key::Character("q" | "Q")) => event_loop.exit(),
          (Role::Modal, Key::Named(NamedKey::Escape)) => {
            self.close_modal = true
          },
          (Role::Modal, key) => self.log(format!("modal: key {key:?}")),
          _ => {},
        }
      },
      _ => {},
    }
  }

  fn about_to_wait(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    if std::mem::take(&mut self.open_modal) && self.modal.is_none() {
      self.open_modal(event_loop);
    }
    if std::mem::take(&mut self.close_modal) {
      self.close_modal();
    }
    self.run_check(event_loop);
    if Instant::now() >= self.next_poll {
      self.reread(event_loop, "the timer");
      self.next_poll = Instant::now() + self.poll;
    }
    let wake = self
      .deadline
      .filter(|_| self.check.is_some())
      .map_or(self.next_poll, |d| d.min(self.next_poll));
    event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
  }
}

fn main() -> ExitCode {
  let mut mode = Mode { dovetail: true, system: true };
  let mut check = false;
  let mut poll = Duration::from_millis(1000);
  let mut args = std::env::args().skip(1);
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--block" => {
        mode = match args.next().as_deref() {
          Some("dovetail") => Mode { dovetail: true, system: false },
          Some("system") => Mode { dovetail: false, system: true },
          Some("both") => Mode { dovetail: true, system: true },
          other => {
            eprintln!("--block takes dovetail, system or both, not {other:?}");
            return ExitCode::FAILURE;
          },
        }
      },
      "--check" => check = true,
      "--poll-ms" => {
        let Some(ms) = args.next().and_then(|a| a.parse().ok()) else {
          eprintln!("--poll-ms takes a number of milliseconds");
          return ExitCode::FAILURE;
        };
        poll = Duration::from_millis(ms);
      },
      other => {
        eprintln!(
          "unknown argument {other:?}. Use --block dovetail|system|both, \
           --check and --poll-ms N"
        );
        return ExitCode::FAILURE;
      },
    }
  }

  let event_loop = match EventLoop::new() {
    Ok(l) => l,
    Err(e) => {
      eprintln!("could not make the event loop: {e}");
      return ExitCode::FAILURE;
    },
  };
  let mut app = App {
    mode,
    check: check.then_some(Step::OwnerOpen),
    failures: Vec::new(),
    start: Instant::now(),
    display: event_loop.owned_display_handle(),
    gpu: None,
    windows: HashMap::new(),
    owner: None,
    modal: None,
    open_modal: false,
    close_modal: false,
    monitors: Vec::new(),
    poll,
    next_poll: Instant::now() + poll,
    deadline: None,
  };
  app.log(format!("blocking: {mode:?}"));
  if let Err(e) = event_loop.run_app(&mut app) {
    eprintln!("the event loop failed: {e}");
    return ExitCode::FAILURE;
  }
  // The surfaces go before their windows, and both before the GPU.
  app.windows.clear();
  if check && app.check != Some(Step::Done) {
    app
      .failures
      .push(format!("stopped at {:?}", app.check));
  }
  if app.failures.is_empty() {
    if check {
      eprintln!(
        "PASS  the modal window opened centred on its owner, which was {} \
         while it was open and free after",
        if mode.dovetail {
          "blocked"
        } else {
          "not blocked by Dovetail"
        }
      );
    }
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn nothing_is_refused_while_the_owner_is_free() {
    for input in [
      Input::Press,
      Input::Release,
      Input::Wheel,
      Input::Key,
      Input::Text,
      Input::Close,
      Input::Focus,
      Input::Other,
    ] {
      assert_eq!(judge(input, false), Verdict::Pass, "{input:?}");
    }
  }

  #[test]
  fn a_blocked_owner_refuses_what_would_change_it() {
    assert_eq!(judge(Input::Press, true), Verdict::RefuseAndRaise);
    assert_eq!(judge(Input::Close, true), Verdict::RefuseAndRaise);
    for input in [
      Input::Release,
      Input::Wheel,
      Input::Key,
      Input::Text,
    ] {
      assert_eq!(judge(input, true), Verdict::Refuse, "{input:?}");
    }
    // Hover and focus go on, so the owner still draws.
    assert_eq!(judge(Input::Focus, true), Verdict::Pass);
    assert_eq!(judge(Input::Other, true), Verdict::Pass);
  }
}
