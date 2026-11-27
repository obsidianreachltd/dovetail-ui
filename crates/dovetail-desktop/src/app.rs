use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dovetail_core::{
  Element, ImeEvent, KeyCode, KeyEvent, Modifiers, NamedKey, PointerButton,
  PointerKind, View, WindowLimits, WindowPlace, WindowState,
};
use dovetail_layout::{LayoutBox, Rect};
use dovetail_render::{
  Canvas, Color, FrameOutcome, Gpu, GpuError, Painter, PhysicalSize,
  WindowSurface, font_measure,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{
  ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle,
};
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey};
use winit::window::{Window, WindowAttributes};

mod lifecycle;
mod manage;
mod multi;
mod pumped;
mod root;

pub use manage::ExitPolicy;
pub use root::{run_windows, run_windows_with};

pub use pumped::Pumped;

use dovetail_session::{Session, Settings, WHEEL_STEP};

use crate::access::Access;
use crate::dev::DevState;
use crate::geometry::{LogicalSize, WindowInfo};

/// Why the event loop or a window could not be created.
#[derive(Debug)]
pub enum Error {
  /// The platform event loop could not start or stopped with an error.
  EventLoop(winit::error::EventLoopError),
  /// The operating system refused to create a window.
  Window(winit::error::OsError),
  /// The GPU could not be set up.
  Gpu(GpuError),
  /// A pumped event loop ended, with this code, or did not do the work
  /// asked of it (-1).
  Ended(i32),
}

impl fmt::Display for Error {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      Error::EventLoop(e) => write!(f, "event loop error: {e}"),
      Error::Window(e) => write!(f, "could not create a window: {e}"),
      Error::Gpu(e) => write!(f, "{e}"),
      Error::Ended(code) => write!(f, "the event loop ended ({code})"),
    }
  }
}

impl std::error::Error for Error {}

impl From<GpuError> for Error {
  fn from(e: GpuError) -> Self {
    Error::Gpu(e)
  }
}

/// Identifies one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(winit::window::WindowId);

/// How a new window should look.
#[derive(Clone, Debug)]
pub struct WindowOptions {
  /// The title shown in the title bar.
  pub title: String,
  /// The starting size of the drawing area.
  pub size: LogicalSize,
  /// Where the window's top-left corner starts on the screen, frame
  /// included, in logical pixels. Without it the system chooses.
  pub position: Option<(f32, f32)>,
  /// The sizes it keeps to, and whether the user can resize it (M19).
  pub limits: WindowLimits,
  /// The state it opens in (M19).
  pub state: WindowState,
  /// The window it belongs to, which keeps it in front (M19).
  pub owner: Option<WindowId>,
  /// Whether it blocks its owner's input until it closes (M19). It opens
  /// centred on its owner.
  pub modal: bool,
}

impl WindowOptions {
  /// A window with the given title and a starting size of 800 × 600.
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title: title.into(),
      size: LogicalSize::new(800.0, 600.0),
      position: None,
      limits: WindowLimits::default(),
      state: WindowState::Normal,
      owner: None,
      modal: false,
    }
  }

  /// Makes it belong to `owner`, and, with `modal`, block its owner's
  /// input until it closes.
  pub fn with_owner(
    mut self,
    owner: WindowId,
    modal: bool,
  ) -> Self {
    self.owner = Some(owner);
    self.modal = modal;
    self
  }

  /// Sets the sizes it keeps to, and whether it can be resized.
  pub fn with_limits(
    mut self,
    limits: WindowLimits,
  ) -> Self {
    self.limits = limits;
    self
  }

  /// Sets the state it opens in.
  pub fn with_state(
    mut self,
    state: WindowState,
  ) -> Self {
    self.state = state;
    self
  }

  /// Sets where the window's top-left corner starts on the screen.
  pub fn with_position(
    mut self,
    position: (f32, f32),
  ) -> Self {
    self.position = Some(position);
    self
  }

  /// Sets the starting size.
  pub fn with_size(
    mut self,
    size: LogicalSize,
  ) -> Self {
    self.size = size;
    self
  }
}

/// What to draw in one frame of a window. In M01 a frame only clears the
/// window to one colour; M04 adds painting.
#[derive(Debug)]
pub struct Frame {
  info: WindowInfo,
  clear: Color,
}

impl Frame {
  /// Fills the whole window with `color`.
  pub fn clear(
    &mut self,
    color: Color,
  ) {
    self.clear = color;
  }

  /// The size and display scale of the window being drawn.
  pub fn info(&self) -> WindowInfo {
    self.info
  }
}

/// An application run by [`run`].
///
/// Every method gets a [`Context`] to open, close and redraw windows.
pub trait Application: 'static {
  /// Called once, when the platform is ready. Open the first window here.
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  );

  /// Called when a window must be drawn.
  fn draw(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    frame: &mut Frame,
  );

  /// Called when a key is pressed in a window and nothing in the window's
  /// component tree took it. Repeats of a held key are not passed on.
  fn key_pressed(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    key: &KeyEvent,
  ) {
    let _ = (cx, window, key);
  }

  /// Called when a window's size or display scale changes.
  fn resized(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    info: WindowInfo,
  ) {
    let _ = (cx, window, info);
  }

  /// Called after a frame of a window has been drawn and shown.
  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    let _ = (cx, window);
  }

  /// Called when the time set with [`Context::wake_after`] has passed.
  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let _ = cx;
  }

  /// The user clicked the application's Dock icon while no window was
  /// open (macOS, M19).
  fn reopen(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let _ = cx;
  }

  /// A window's state changed, as when the user maximised it (M19).
  fn state_changed(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    state: WindowState,
  ) {
    let _ = (cx, window, state);
  }

  /// Called when the user asks to close a window. By default it closes.
  fn close_requested(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    cx.close_window(window);
  }
}

/// Lets an application work with its windows. Given to every
/// [`Application`] method.
pub struct Context<'a> {
  event_loop: &'a ActiveEventLoop,
  shell: &'a mut Shell,
}

impl Context<'_> {
  /// Opens a new window and draws it as soon as possible.
  pub fn open_window(
    &mut self,
    options: WindowOptions,
  ) -> Result<WindowId, Error> {
    self
      .shell
      .open_window(self.event_loop, options, true)
  }

  /// Opens a window that shows `root`, a component view, and keeps it up
  /// to date: state changes redraw it, and pointer presses reach the
  /// elements' handlers. [`Application::draw`] is not called for it.
  ///
  /// # Panics
  ///
  /// If `root` is not a component view.
  pub fn open_ui_window(
    &mut self,
    options: WindowOptions,
    root: View,
  ) -> Result<WindowId, Error> {
    self
      .shell
      .open_ui(self.event_loop, options, root, None)
  }

  /// The windows that are open, including those that components opened.
  pub fn windows(&self) -> Vec<WindowId> {
    self
      .shell
      .windows
      .keys()
      .map(|w| WindowId(*w))
      .collect()
  }

  /// Where a window is on the screen and how big its drawing area is.
  pub fn window_place(
    &self,
    window: WindowId,
  ) -> Option<WindowPlace> {
    self
      .shell
      .windows
      .get(&window.0)
      .map(|e| multi::place_of(&e.window))
  }

  /// Where a window's drawing area starts on the screen, in logical
  /// pixels, when the system says. For self-tests that drag between
  /// windows.
  pub fn window_origin(
    &self,
    window: WindowId,
  ) -> Option<(f32, f32)> {
    let e = self.shell.windows.get(&window.0)?;
    let scale = e.window.scale_factor();
    let o = e.window.inner_position().ok()?;
    Some((
      (f64::from(o.x) / scale) as f32,
      (f64::from(o.y) / scale) as f32,
    ))
  }

  /// The element with keyboard focus in a UI window. For self-tests.
  pub fn focused_element(
    &self,
    window: WindowId,
  ) -> Option<&Element> {
    let ui = self.shell.windows.get(&window.0)?.ui.as_ref()?;
    ui.session.focused_element()
  }

  /// The scrollbars a UI window shows now, each with whether it is wide,
  /// as under the pointer (M15). For tests and self-tests.
  pub fn scrollbars(
    &self,
    window: WindowId,
  ) -> Vec<(dovetail_layout::Scrollbar, bool)> {
    self
      .shell
      .windows
      .get(&window.0)
      .and_then(|e| e.ui.as_ref())
      .map(|ui| ui.session.scrollbars())
      .unwrap_or_default()
  }

  /// What a window has received from the system and the GPU: requests to
  /// draw, frames shown and skipped, and whether it was last hidden or had
  /// the keyboard. For tests that wait in vain (M20).
  pub fn window_stats(
    &self,
    window: WindowId,
  ) -> Option<WindowStats> {
    Some(self.shell.windows.get(&window.0)?.stats)
  }

  /// A UI window's session: its components, layout, focus and input
  /// state (D0027). For tests.
  pub fn ui_session(
    &self,
    window: WindowId,
  ) -> Option<&dovetail_session::Session> {
    Some(
      &self
        .shell
        .windows
        .get(&window.0)?
        .ui
        .as_ref()?
        .session,
    )
  }

  /// Brings a window to the front and gives it the keyboard.
  pub fn focus_window(
    &mut self,
    window: WindowId,
  ) {
    self.shell.bring_to_front(window.0);
  }

  /// True when a modal window blocks `window`, so input sent to it here
  /// is refused, as the system's would be. With `press`, the modal window
  /// comes to the front, as a press on its owner brings it (M19).
  fn refused(
    &mut self,
    window: WindowId,
    press: bool,
  ) -> bool {
    match self.shell.blocker(window.0) {
      Some(modal) => {
        if press {
          self.shell.bring_to_front(modal);
        }
        true
      },
      None => false,
    }
  }

  /// Moves a window's top-left corner, frame included, to `position` in
  /// logical pixels at the scale of the monitor it lands on, as windows
  /// report their places (M19). Wayland ignores this.
  pub fn set_window_position(
    &self,
    window: WindowId,
    (x, y): (f32, f32),
  ) {
    let Some(entry) = self.shell.windows.get(&window.0) else {
      return;
    };
    let monitors = manage::read_monitors(self.event_loop);
    let scale = monitors
      .iter()
      .find(|m| m.bounds.contains((x * m.scale, y * m.scale)))
      .map_or(entry.window.scale_factor() as f32, |m| m.scale);
    entry
      .window
      .set_outer_position(winit::dpi::PhysicalPosition::new(
        (x * scale).round() as i32,
        (y * scale).round() as i32,
      ));
  }

  /// True when no windows are open and the exit policy ends the
  /// application after its last window: what a program that is not a test
  /// would do now (M19).
  pub fn would_end(&self) -> bool {
    self.shell.windows.is_empty()
      && self.shell.exit_policy == ExitPolicy::AfterLastWindow
  }

  /// The modal window that blocks a window's input, if one does (M19).
  pub fn blocked_by(
    &self,
    window: WindowId,
  ) -> Option<WindowId> {
    self.shell.blocker(window.0).map(WindowId)
  }

  /// The windows a component opened, with the ids components know them
  /// by (M19).
  pub fn hosted_windows(&self) -> Vec<(WindowId, dovetail_core::HostWindow)> {
    let mut list: Vec<_> = self
      .shell
      .windows
      .iter()
      .filter_map(|(w, e)| Some((WindowId(*w), e.hosted.as_ref()?.id)))
      .collect();
    list.sort_by_key(|(_, h)| h.0);
    list
  }

  /// True while something is being dragged (M10).
  pub fn dragging(&self) -> bool {
    self.shell.dragging()
  }

  /// The title of a window, as last set.
  pub fn window_title(
    &self,
    window: WindowId,
  ) -> Option<String> {
    Some(self.shell.windows.get(&window.0)?.window.title())
  }
}

impl Shell {
  /// Opens a window that shows `root`; see [`Context::open_ui_window`].
  fn open_ui(
    &mut self,
    event_loop: &ActiveEventLoop,
    options: WindowOptions,
    root: View,
    context: Option<dovetail_core::ComponentId>,
  ) -> Result<WindowId, Error> {
    // Hidden until AccessKit's adapter is in place, as it requires.
    let title = options.title.clone();
    let id = self.open_window(event_loop, options, false)?;
    let entry = self
      .windows
      .get_mut(&id.0)
      .expect("window just opened");
    let adapter = accesskit_winit::Adapter::with_event_loop_proxy(
      event_loop,
      &entry.window,
      self.proxy.clone(),
    );
    entry.window.set_visible(true);
    // The system's settings, read with its appearance (M18).
    let settings = Settings::from(dovetail_core::system_settings());
    let mut session =
      Session::new(title, root, context, settings, Box::new(font_measure()));
    session.set_active(entry.window.has_focus());
    entry.ui = Some(UiWindow {
      session,
      adapter,
      access: Access::default(),
      canvas: None,
      ring: None,
      ime_allowed: false,
      ime_area: None,
      painted_caret: true,
      waiting: None,
      painted_bars: Vec::new(),
    });
    Ok(id)
  }

  /// The pointer moved to (`x`, `y`) in logical pixels in window `id`.
  fn pointer_moved(
    &mut self,
    id: winit::window::WindowId,
    (x, y): (f32, f32),
    modifiers: Modifiers,
  ) {
    let dragging = self.dragging();
    if let Some(ui) = self
      .windows
      .get_mut(&id)
      .and_then(|e| e.ui.as_mut())
    {
      ui.session.set_cursor((x, y));
      if !dragging && ui.session.pointer_moved(modifiers) {
        self.windows[&id].window.request_redraw();
      }
    }
    if dragging {
      self.drag_moved(id);
    } else {
      self.pick_up_drag(id);
    }
  }

  /// A button went down in window `id`, where the pointer is.
  fn pointer_pressed(
    &mut self,
    id: winit::window::WindowId,
    modifiers: Modifiers,
    button: PointerButton,
  ) {
    if let Some(entry) = self.windows.get_mut(&id)
      && let Some(ui) = entry.ui.as_mut()
      && ui.session.press(modifiers, button)
    {
      entry.window.request_redraw();
    }
    self.pick_up_drag(id);
  }

  /// A button was let go in window `id`, where the pointer is. A drag
  /// ends with the main button.
  fn pointer_released(
    &mut self,
    id: winit::window::WindowId,
    modifiers: Modifiers,
    button: PointerButton,
  ) {
    if button == PointerButton::Primary {
      self.drag_released(id);
    }
    if let Some(ui) = self
      .windows
      .get_mut(&id)
      .and_then(|e| e.ui.as_mut())
    {
      ui.session.release(modifiers, button);
    }
  }
}

impl Context<'_> {
  /// The placed boxes of a UI window, as last drawn.
  pub fn ui_boxes(
    &self,
    window: WindowId,
  ) -> &[LayoutBox] {
    self
      .shell
      .windows
      .get(&window.0)
      .and_then(|e| e.ui.as_ref())
      .map_or(&[], |ui| ui.session.layout().boxes())
  }

  /// A UI window's pixels as last painted (M17): its width and height in
  /// physical pixels, and sRGB red, green, blue and alpha bytes for each
  /// pixel, row by row. Slow: for tests and self-tests.
  pub fn ui_pixels(
    &self,
    window: WindowId,
  ) -> Option<(u32, u32, Vec<[u8; 4]>)> {
    let gpu = self.shell.gpu.as_ref()?;
    let painter = self.shell.painter.as_ref()?;
    let canvas = self
      .shell
      .windows
      .get(&window.0)?
      .ui
      .as_ref()?
      .canvas
      .as_ref()?;
    Some((
      canvas.width(),
      canvas.height(),
      painter.read_pixels(gpu, canvas),
    ))
  }

  /// Sends a key press to a UI window, as the keyboard would: Tab moves
  /// focus, and other keys go to the focused element and its parents.
  /// Returns whether the window used it. For tests and self-tests.
  pub fn key(
    &mut self,
    window: WindowId,
    event: &KeyEvent,
  ) -> bool {
    if self.shell.dragging() && event.is(NamedKey::Escape) {
      self.shell.cancel_drag();
      return true;
    }
    if self.refused(window, false) {
      return false;
    }
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let used = entry
      .ui
      .as_mut()
      .is_some_and(|ui| ui.session.key(event));
    if used {
      entry.window.request_redraw();
    }
    used
  }

  /// Presses and releases the pointer at (`x`, `y`) in logical pixels in a
  /// UI window, as a click would. For tests and self-tests.
  pub fn click(
    &mut self,
    window: WindowId,
    x: f32,
    y: f32,
  ) {
    let none = Modifiers::default();
    self.shell.pointer_moved(window.0, (x, y), none);
    if self.refused(window, true) {
      return;
    }
    self
      .shell
      .pointer_pressed(window.0, none, PointerButton::Primary);
    self
      .shell
      .pointer_released(window.0, none, PointerButton::Primary);
  }

  /// Turns the mouse wheel over (`x`, `y`) in logical pixels in a UI
  /// window, scrolling by (`dx`, `dy`) logical pixels; positive shows more
  /// of what is right and below. True when something scrolled. For tests
  /// and self-tests.
  pub fn wheel(
    &mut self,
    window: WindowId,
    (x, y): (f32, f32),
    (dx, dy): (f32, f32),
  ) -> bool {
    if self.refused(window, false) {
      return false;
    }
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let scrolled = entry.ui.as_mut().is_some_and(|ui| {
      ui.session.set_cursor((x, y));
      ui.session
        .wheel(dx, dy, false, Modifiers::default())
    });
    if scrolled {
      entry.window.request_redraw();
    }
    scrolled
  }

  /// Turns a stepped mouse wheel `steps` notches over (`x`, `y`) in a UI
  /// window: positive is towards the user, which scrolls down or zooms
  /// out. True when something used it. For tests and self-tests.
  pub fn wheel_steps(
    &mut self,
    window: WindowId,
    (x, y): (f32, f32),
    steps: f32,
    modifiers: Modifiers,
  ) -> bool {
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let used = entry.ui.as_mut().is_some_and(|ui| {
      ui.session.set_cursor((x, y));
      ui.session
        .wheel(0.0, steps * WHEEL_STEP, true, modifiers)
    });
    if used {
      entry.window.request_redraw();
    }
    used
  }

  /// Sends input method text to the focused element of a UI window, as an
  /// input method would. True when an element took it. For tests and
  /// self-tests.
  pub fn ime(
    &mut self,
    window: WindowId,
    event: ImeEvent,
  ) -> bool {
    if self.refused(window, false) {
      return false;
    }
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let used = entry
      .ui
      .as_mut()
      .is_some_and(|ui| ui.session.ime(event));
    if used {
      entry.window.request_redraw();
    }
    used
  }

  /// While a UI window's caret blinks, whether it is in its on half;
  /// `None` when it does not blink (no caret, the window is in the
  /// background, or the user turned blinking off). For self-tests.
  pub fn caret_blink(
    &self,
    window: WindowId,
  ) -> Option<bool> {
    self
      .shell
      .windows
      .get(&window.0)
      .and_then(|e| e.ui.as_ref())
      .and_then(|ui| ui.session.caret_blink())
  }

  /// True when a UI window has its input method turned on, because the
  /// focused element takes text. For tests and self-tests.
  pub fn ime_allowed(
    &self,
    window: WindowId,
  ) -> bool {
    self
      .shell
      .windows
      .get(&window.0)
      .and_then(|e| e.ui.as_ref())
      .is_some_and(|ui| ui.ime_allowed)
  }

  /// Sends the pointer to a UI window, as the mouse would: `Down` presses
  /// at (`x`, `y`) in logical pixels, `Move` moves there with the button
  /// held, and `Up` lets go there. For tests and self-tests.
  pub fn pointer(
    &mut self,
    window: WindowId,
    kind: PointerKind,
    x: f32,
    y: f32,
    modifiers: Modifiers,
  ) {
    self.pointer_button(
      window,
      kind,
      (x, y),
      modifiers,
      PointerButton::Primary,
    );
  }

  /// As [`Context::pointer`], with any mouse button. For tests and
  /// self-tests.
  pub fn pointer_button(
    &mut self,
    window: WindowId,
    kind: PointerKind,
    (x, y): (f32, f32),
    modifiers: Modifiers,
    button: PointerButton,
  ) {
    if self
      .shell
      .windows
      .get(&window.0)
      .is_none_or(|e| e.ui.is_none())
    {
      return;
    }
    match kind {
      PointerKind::Down if self.refused(window, true) => {},
      PointerKind::Down => {
        if let Some(ui) = self
          .shell
          .windows
          .get_mut(&window.0)
          .and_then(|e| e.ui.as_mut())
        {
          ui.session.set_cursor((x, y));
        }
        self
          .shell
          .pointer_pressed(window.0, modifiers, button);
      },
      PointerKind::Move => {
        self
          .shell
          .pointer_moved(window.0, (x, y), modifiers)
      },
      PointerKind::Up => {
        self
          .shell
          .pointer_moved(window.0, (x, y), modifiers);
        self
          .shell
          .pointer_released(window.0, modifiers, button);
      },
    }
  }

  /// Takes the pointer out of a UI window, as when the mouse leaves it.
  /// For tests and self-tests.
  pub fn pointer_leave(
    &mut self,
    window: WindowId,
  ) {
    if let Some(entry) = self.shell.windows.get_mut(&window.0)
      && let Some(ui) = entry.ui.as_mut()
      && ui.session.pointer_left()
    {
      entry.window.request_redraw();
    }
  }

  /// Closes a window. When the last window closes, the application stops.
  pub fn close_window(
    &mut self,
    window: WindowId,
  ) {
    self.shell.windows.remove(&window.0);
    // Without a root, the first window's own windows close with it. With
    // one, each window closes when its component goes (M19).
    if self.shell.primary == Some(window.0) && self.shell.root.is_none() {
      self.shell.close_hosted();
    }
    if self.shell.should_end() {
      self.event_loop.exit();
    }
  }

  /// Asks a window to close as the user would: a component that guards it
  /// is asked first (see `Element::close_guard`), then a component that
  /// opened it. Otherwise it closes, without asking
  /// [`Application::close_requested`]. For tests.
  pub fn request_close(
    &mut self,
    window: WindowId,
  ) {
    // A window a modal window blocks cannot be closed (M19).
    if self.refused(window, true) {
      return;
    }
    if self.shell.ask_guard(window.0) {
      return;
    }
    if let Some(hosted) = self
      .shell
      .windows
      .get(&window.0)
      .and_then(|e| e.hosted.as_ref())
    {
      hosted.on_close.call(());
      return;
    }
    self.close_window(window);
  }

  /// Asks for a window to be drawn again.
  pub fn request_redraw(
    &self,
    window: WindowId,
  ) {
    if let Some(entry) = self.shell.windows.get(&window.0) {
      entry.window.request_redraw();
    }
  }

  /// Asks for a window's drawing area to have a new size, kept within its
  /// limits (M19). The platform may refuse or adjust it;
  /// [`Application::resized`] reports the result.
  pub fn set_window_size(
    &self,
    window: WindowId,
    size: LogicalSize,
  ) {
    if let Some(entry) = self.shell.windows.get(&window.0) {
      let size = entry
        .limits
        .clamp((size.width as f32, size.height as f32));
      let _ = entry.window.request_inner_size(logical(size));
    }
  }

  /// The state a window is in now (M19). Where the system does not say
  /// whether a window is minimised, as on Wayland, it is not.
  pub fn window_state(
    &self,
    window: WindowId,
  ) -> Option<WindowState> {
    Some(state_of(&self.shell.windows.get(&window.0)?.window))
  }

  /// Puts a window in a state: normal, maximised, minimised or full
  /// screen (M19). The system may refuse, as Wayland refuses to minimise.
  pub fn set_window_state(
    &self,
    window: WindowId,
    state: WindowState,
  ) {
    if let Some(entry) = self.shell.windows.get(&window.0) {
      apply_state(&entry.window, state);
    }
  }

  /// Changes the sizes a window keeps to, and whether it can be resized
  /// (M19).
  pub fn set_window_limits(
    &mut self,
    window: WindowId,
    limits: WindowLimits,
  ) {
    if let Some(entry) = self.shell.windows.get_mut(&window.0) {
      apply_limits(&entry.window, limits);
      entry.limits = limits;
    }
  }

  /// Minimises or restores a window. Some platforms, such as Wayland,
  /// ignore this.
  pub fn set_minimized(
    &self,
    window: WindowId,
    minimized: bool,
  ) {
    if let Some(entry) = self.shell.windows.get(&window.0) {
      entry.window.set_minimized(minimized);
    }
  }

  /// Calls [`Application::woke`] once `delay` has passed. Setting a new
  /// delay replaces the previous one.
  pub fn wake_after(
    &self,
    delay: Duration,
  ) {
    self
      .shell
      .wake_at
      .set(Some(dovetail_core::now() + delay));
  }

  /// The size and display scale of a window, if it is open.
  pub fn window_info(
    &self,
    window: WindowId,
  ) -> Option<WindowInfo> {
    self
      .shell
      .windows
      .get(&window.0)
      .map(WindowEntry::info)
  }

  /// How many windows are open.
  pub fn window_count(&self) -> usize {
    self.shell.windows.len()
  }

  /// Facts about the GPU adapter in use, once a window has been opened.
  pub fn gpu_info(&self) -> Option<dovetail_render::AdapterInfo> {
    self.shell.gpu.as_ref().map(Gpu::adapter_info)
  }

  /// How many times the GPU device has been replaced after a loss.
  pub fn gpu_generation(&self) -> u64 {
    self.shell.gpu.as_ref().map_or(0, Gpu::generation)
  }

  /// Stops the application.
  pub fn exit(&self) {
    self.event_loop.exit();
  }

  /// Destroys the GPU device on purpose, to test recovery. Every window
  /// is drawn again, which recovers the device first.
  #[cfg(feature = "device-loss-testing")]
  pub fn simulate_device_loss(&self) {
    if let Some(gpu) = &self.shell.gpu {
      gpu.simulate_device_loss();
    }
    self.shell.redraw_all();
  }
}

struct WindowEntry {
  window: Arc<Window>,
  surface: WindowSurface,
  ui: Option<UiWindow>,
  /// Set for a window that a component opened through the window host.
  hosted: Option<multi::Hosted>,
  /// What the window has received, for tests (M20).
  stats: WindowStats,
  /// Its state when last looked at (M19).
  state: WindowState,
  /// The sizes it keeps to (M19).
  limits: WindowLimits,
  /// The window it belongs to (M19).
  owner: Option<winit::window::WindowId>,
  /// Whether it blocks its owner (M19).
  modal: bool,
  /// Until when Alt key events are ignored, after the program brought it
  /// to the front (M19).
  focus_alt_until: Option<Instant>,
  /// The name `dovetail dev` keeps its place under (M19).
  dev_name: Option<String>,
}

/// What a window has received from the system and the GPU, so a test that
/// waits in vain can say why (M20, D0026).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowStats {
  /// Requests to draw.
  pub redraws: u64,
  /// Frames drawn and shown.
  pub presented: u64,
  /// Frames asked for but skipped, as when the window could not be seen.
  pub skipped: u64,
  /// The last time the system said whether the window is hidden.
  pub occluded: Option<bool>,
  /// The last time the system said whether the window has the keyboard.
  pub focused: Option<bool>,
}

/// A window that shows a component tree. What it shows and how it answers
/// input is in its session (D0027); the rest is what the window and its
/// painting need.
struct UiWindow {
  /// The component tree, layout, focus and input state.
  session: Session,
  /// Connects the window to assistive tools (M07).
  adapter: accesskit_winit::Adapter,
  /// The accessibility tree, as last sent.
  access: Access,
  /// Made on the first draw, and again after a resize or device loss.
  canvas: Option<Canvas>,
  /// Where the focus ring was last drawn, so it can be cleared.
  ring: Option<Rect>,
  /// The input method is on, because the focused element takes text.
  ime_allowed: bool,
  /// Where the input method was last told the caret is.
  ime_area: Option<Rect>,
  /// Whether the caret was shown in the last paint.
  painted_caret: bool,
  /// Where images still being uploaded were left out of the last paint
  /// (M17): painted again in the next frame.
  waiting: Option<Rect>,
  /// The scrollbars as last painted, each with whether it was wide.
  painted_bars: Vec<(dovetail_layout::Scrollbar, bool)>,
}

/// When input logging started, if the environment variable
/// `DOVETAIL_LOG_INPUT` is set. Logging writes every key, input method and
/// focus event to stderr, to look into input bugs such as B0004.
fn input_log() -> Option<Instant> {
  thread_local! {
    static START: Option<Instant> =
      // real time: the input log times what really happened.
      std::env::var_os("DOVETAIL_LOG_INPUT").map(|_| Instant::now());
  }
  START.with(|s| *s)
}

/// Logs a key, input method or focus event, when input logging is on.
fn log_input(event: &WindowEvent) {
  let Some(start) = input_log() else {
    return;
  };
  let ms = start.elapsed().as_secs_f64() * 1000.0;
  match event {
    WindowEvent::KeyboardInput { event, is_synthetic, .. } => eprintln!(
      "input {ms:>10.3} key {:?} {:?} text={:?} repeat={} synthetic={}",
      event.state,
      event.logical_key,
      event.text.as_deref(),
      event.repeat,
      is_synthetic
    ),
    WindowEvent::Ime(ime) => eprintln!("input {ms:>10.3} ime {ime:?}"),
    WindowEvent::Focused(f) => eprintln!("input {ms:>10.3} focused {f}"),
    WindowEvent::ModifiersChanged(m) => {
      eprintln!("input {ms:>10.3} modifiers {:?}", m.state())
    },
    _ => {},
  }
}

impl WindowEntry {
  fn info(&self) -> WindowInfo {
    WindowInfo {
      size: self.surface.size(),
      scale_factor: self.window.scale_factor(),
    }
  }
}

/// Everything the event loop owns apart from the application.
struct Shell {
  display: OwnedDisplayHandle,
  gpu: Option<Gpu>,
  /// Shared by every UI window; made for the current GPU device.
  painter: Option<Painter>,
  windows: HashMap<winit::window::WindowId, WindowEntry>,
  /// Set by the state-change hook; UI windows redraw when it is.
  state_changed: Rc<Cell<bool>>,
  /// The modifier keys held now.
  modifiers: Modifiers,
  /// When to call [`Application::woke`], if set.
  wake_at: Cell<Option<Instant>>,
  /// The soonest the next frame for animation may start (M18).
  frame_at: Option<Instant>,
  /// Sends events to the event loop, for AccessKit.
  proxy: EventLoopProxy<UserEvent>,
  /// The window in front, which owns file dialogs on Windows (M11).
  dialog_owner: Rc<Cell<isize>>,
  /// The first window opened; `dovetail dev` keeps its place.
  primary: Option<winit::window::WindowId>,
  dev: DevState,
  /// The drag in progress (M10).
  drag: Option<multi::DragState>,
  /// Windows that components asked for (M10).
  host: Rc<multi::HostQueue>,
  /// The loop is pumped by a test (`Pumped`), which ends it itself:
  /// closing the last window does not.
  pumped: bool,
  /// Whether the application ends after its last window (M19).
  exit_policy: ExitPolicy,
  /// The application's root, a tree with no window, when it has one
  /// (M19, `run_windows`).
  root: Option<Session>,
}

impl Shell {
  /// The least time between two frames for animation: three quarters of
  /// the display's frame, so a frame is ready when the display wants it.
  /// Showing a frame waits for the display, which keeps the pace. 60
  /// frames a second when the display does not say.
  fn frame_gap(&self) -> Duration {
    let millihertz = self
      .primary
      .and_then(|id| self.windows.get(&id))
      .or_else(|| self.windows.values().next())
      .and_then(|entry| entry.window.current_monitor())
      .and_then(|monitor| monitor.refresh_rate_millihertz())
      .filter(|mhz| *mhz >= 1000)
      .unwrap_or(60_000);
    Duration::from_secs_f64(0.75 * 1000.0 / f64::from(millihertz))
  }

  fn open_window(
    &mut self,
    event_loop: &ActiveEventLoop,
    mut options: WindowOptions,
    visible: bool,
  ) -> Result<WindowId, Error> {
    // Under `dovetail dev`, each window opens where it was before the
    // restart (M19).
    let dev_name = self.dev.next_name();
    if let Some(name) = &dev_name
      && let Some((place, state)) =
        self
          .dev
          .restore(name, &dovetail_core::monitors(), options.limits)
    {
      options.size =
        LogicalSize::new(f64::from(place.size.0), f64::from(place.size.1));
      options.position = place.position;
      options.state = state;
    }
    let mut attributes = WindowAttributes::default()
      .with_visible(visible)
      .with_title(options.title)
      .with_inner_size(winit::dpi::LogicalSize::new(
        options.size.width,
        options.size.height,
      ));
    let first = self.primary.is_none();
    if let Some(min) = options.limits.min_size {
      attributes = attributes.with_min_inner_size(logical(min));
    }
    if let Some(max) = options.limits.max_size {
      attributes = attributes.with_max_inner_size(logical(max));
    }
    attributes = attributes
      .with_resizable(options.limits.resizable)
      .with_maximized(options.state == WindowState::Maximised);
    if options.state == WindowState::FullScreen {
      attributes = attributes
        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    }
    // A position in logical pixels is at the scale of the monitor it is
    // on, as windows report theirs (M19).
    if let Some((x, y)) = options.position {
      let monitors = manage::read_monitors(event_loop);
      attributes = match monitors
        .iter()
        .find(|m| m.bounds.contains((x * m.scale, y * m.scale)))
      {
        Some(m) => attributes.with_position(winit::dpi::PhysicalPosition::new(
          f64::from(x * m.scale).round() as i32,
          f64::from(y * m.scale).round() as i32,
        )),
        None => attributes.with_position(winit::dpi::LogicalPosition::new(
          f64::from(x),
          f64::from(y),
        )),
      };
    }
    // A window with an owner belongs to it; a modal one opens centred on
    // it (M19).
    let owner = options
      .owner
      .and_then(|o| self.windows.get(&o.0))
      .map(|e| Arc::clone(&e.window));
    if let Some(owner) = &owner {
      attributes = owned_attributes(attributes, owner, options.modal);
      if options.modal
        && options.position.is_none()
        && let Ok(at) = owner.outer_position()
      {
        let size = owner.outer_size();
        let scale = owner.scale_factor();
        let w = f64::from(options.size.width as f32) * scale;
        let h = (f64::from(options.size.height as f32) + 30.0) * scale;
        attributes =
          attributes.with_position(winit::dpi::PhysicalPosition::new(
            at.x + ((f64::from(size.width) - w) / 2.0) as i32,
            at.y + ((f64::from(size.height) - h) / 2.0) as i32,
          ));
      }
    }
    let window = Arc::new(
      event_loop
        .create_window(attributes)
        .map_err(Error::Window)?,
    );
    if options.state == WindowState::Minimised {
      window.set_minimized(true);
    }
    if let Some(owner) = &owner {
      crate::platform::make_owned(&window, owner);
    }
    let size = physical(window.inner_size());

    let surface = match &self.gpu {
      Some(gpu) => WindowSurface::new(gpu, Arc::clone(&window), size)?,
      None => {
        let (gpu, surface) = Gpu::with_window(
          Box::new(self.display.clone()),
          Arc::clone(&window),
          size,
        )?;
        self.gpu = Some(gpu);
        surface
      },
    };
    let id = window.id();
    if first {
      self.primary = Some(id);
    }
    window.request_redraw();
    self.windows.insert(
      id,
      WindowEntry {
        window,
        surface,
        ui: None,
        hosted: None,
        stats: WindowStats::default(),
        state: options.state,
        limits: options.limits,
        owner: owner.as_ref().map(|o| o.id()),
        modal: options.modal && owner.is_some(),
        focus_alt_until: None,
        dev_name,
      },
    );
    self.refresh_monitors(event_loop);
    Ok(WindowId(id))
  }

  fn redraw_ui_windows(&self) {
    for entry in self.windows.values().filter(|e| e.ui.is_some()) {
      entry.window.request_redraw();
    }
  }

  #[cfg(feature = "device-loss-testing")]
  fn redraw_all(&self) {
    for entry in self.windows.values() {
      entry.window.request_redraw();
    }
  }

  /// Replaces a lost GPU, following the steps in
  /// [`Gpu::replacing`]: drop every surface while the lost device still
  /// exists, drop the lost GPU, then make a new GPU and new surfaces.
  fn recover_gpu(&mut self) {
    let Some(lost) = self.gpu.take() else {
      return;
    };
    // 1. Drop the surfaces. Each one unconfigures itself on the lost device.
    //    Canvases and the painter belong to the lost device too.
    self.painter = None;
    type Kept = (
      winit::window::WindowId,
      Arc<Window>,
      Option<UiWindow>,
      Option<multi::Hosted>,
      (
        WindowLimits,
        Option<winit::window::WindowId>,
        bool,
        Option<String>,
      ),
    );
    let windows: Vec<Kept> = self
      .windows
      .drain()
      .map(|(id, entry)| {
        let mut ui = entry.ui;
        if let Some(ui) = &mut ui {
          ui.canvas = None;
        }
        (
          id,
          entry.window,
          ui,
          entry.hosted,
          (entry.limits, entry.owner, entry.modal, entry.dev_name),
        )
      })
      .collect();

    // 2. Drop the lost GPU, so nothing holds the old swapchains.
    let lost_generation = lost.generation();
    drop(lost);

    // 3. Make a new GPU with the first window, and surfaces for the others.
    let mut new_gpu: Option<Gpu> = None;
    for (id, window, ui, hosted, (limits, owner, modal, dev_name)) in windows {
      let size = physical(window.inner_size());
      let surface = match &new_gpu {
        Some(gpu) => WindowSurface::new(gpu, Arc::clone(&window), size),
        None => match Gpu::with_window(
          Box::new(self.display.clone()),
          Arc::clone(&window),
          size,
        ) {
          Ok((gpu, surface)) => {
            new_gpu = Some(gpu);
            Ok(surface)
          },
          Err(e) => Err(e),
        },
      };
      match surface {
        Ok(surface) => {
          window.request_redraw();
          let state = state_of(&window);
          self.windows.insert(
            id,
            WindowEntry {
              window,
              surface,
              ui,
              hosted,
              stats: WindowStats::default(),
              state,
              limits,
              owner,
              modal,
              focus_alt_until: None,
              dev_name,
            },
          );
        },
        // Without a surface the window cannot be drawn, so it is closed.
        Err(e) => {
          eprintln!(
            "dovetail: closed a window that could not be set up after GPU loss: {e}"
          )
        },
      }
    }

    // 4. The new GPU replaces the lost one.
    match new_gpu {
      Some(gpu) => {
        let gpu = gpu.replacing(lost_generation);
        eprintln!(
          "dovetail: GPU device recreated (generation {})",
          gpu.generation()
        );
        self.gpu = Some(gpu);
      },
      None => eprintln!("dovetail: GPU recovery failed"),
    }
  }
}

/// Window attributes that make a new window belong to `owner` (M19): an
/// owned window on Windows, and the dialog window type on X11 for a modal
/// one. macOS and X11 are told more once the window is made
/// (`platform::make_owned`).
fn owned_attributes(
  attributes: WindowAttributes,
  owner: &Window,
  modal: bool,
) -> WindowAttributes {
  #[cfg(windows)]
  {
    use winit::platform::windows::WindowAttributesExtWindows;
    let _ = modal;
    match lifecycle::native_handle(owner) {
      0 => attributes,
      hwnd => attributes.with_owner_window(hwnd),
    }
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    use winit::platform::x11::{WindowAttributesExtX11, WindowType};
    let _ = owner;
    if modal {
      attributes.with_x11_window_type(vec![WindowType::Dialog])
    } else {
      attributes
    }
  }
  #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
  {
    let _ = (owner, modal);
    attributes
  }
}

/// A size in logical pixels, as winit takes it.
fn logical((w, h): (f32, f32)) -> winit::dpi::LogicalSize<f64> {
  winit::dpi::LogicalSize::new(f64::from(w), f64::from(h))
}

/// The state a window is in, as winit says (M19).
pub(crate) fn state_of(window: &Window) -> WindowState {
  if window.fullscreen().is_some() {
    WindowState::FullScreen
  } else if window.is_minimized() == Some(true) {
    WindowState::Minimised
  } else if window.is_maximized() {
    WindowState::Maximised
  } else {
    WindowState::Normal
  }
}

/// Puts a window in `state` with winit (M19).
pub(crate) fn apply_state(
  window: &Window,
  state: WindowState,
) {
  if state_of(window) == state {
    return;
  }
  if state != WindowState::FullScreen && window.fullscreen().is_some() {
    window.set_fullscreen(None);
  }
  match state {
    WindowState::FullScreen => {
      window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
    },
    WindowState::Minimised => window.set_minimized(true),
    WindowState::Maximised => {
      window.set_minimized(false);
      window.set_maximized(true);
    },
    WindowState::Normal => {
      window.set_minimized(false);
      window.set_maximized(false);
    },
  }
}

/// Gives a window its size limits with winit (M19).
pub(crate) fn apply_limits(
  window: &Window,
  limits: WindowLimits,
) {
  window.set_min_inner_size(limits.min_size.map(logical));
  window.set_max_inner_size(limits.max_size.map(logical));
  window.set_resizable(limits.resizable);
}

fn physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
  PhysicalSize { width: size.width, height: size.height }
}

/// Events the event loop carries besides the system's own.
pub(crate) enum UserEvent {
  /// A request from an assistive tool, through AccessKit (M07).
  AccessKit(accesskit_winit::Event),
  /// A task has progress or an end to give to its callbacks (M11).
  Tasks,
  /// The user clicked the Dock icon with no window open (macOS, M19).
  Reopen,
}

impl From<accesskit_winit::Event> for UserEvent {
  fn from(event: accesskit_winit::Event) -> Self {
    UserEvent::AccessKit(event)
  }
}

/// Work a test queued for the next turn of the loop.
type Queued = Box<dyn FnOnce(&mut Context<'_>)>;

struct Runner<A: Application> {
  app: A,
  shell: Shell,
  started: bool,
  /// Work a test queued, run with a `Context` before the loop waits
  /// (`Pumped`).
  queued: Vec<Queued>,
}

impl<A: Application> Runner<A> {
  fn context<'a>(
    shell: &'a mut Shell,
    event_loop: &'a ActiveEventLoop,
  ) -> Context<'a> {
    Context { event_loop, shell }
  }

  fn draw(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: winit::window::WindowId,
  ) {
    if self.shell.gpu.as_ref().is_some_and(Gpu::is_lost) {
      self.shell.recover_gpu();
    }
    let Some(entry) = self.shell.windows.get(&id) else {
      return;
    };
    if entry.ui.is_some() {
      return self.draw_ui(event_loop, id);
    }
    let info = entry.info();
    let mut frame = Frame { info, clear: Color::BLACK };
    self.app.draw(
      &mut Self::context(&mut self.shell, event_loop),
      WindowId(id),
      &mut frame,
    );

    let shell = &mut self.shell;
    let (Some(gpu), Some(entry)) = (&shell.gpu, shell.windows.get_mut(&id))
    else {
      return;
    };
    match entry.surface.draw_clear(gpu, frame.clear) {
      FrameOutcome::Presented => {
        entry.stats.presented += 1;
        self.shell.dev.ready();
        self.app.frame_presented(
          &mut Self::context(&mut self.shell, event_loop),
          WindowId(id),
        )
      },
      FrameOutcome::Skipped => entry.stats.skipped += 1,
      FrameOutcome::Retry => entry.window.request_redraw(),
      FrameOutcome::DeviceLost => {
        shell.recover_gpu();
      },
    }
  }

  /// Tells the component that opened window `id` where it is now.
  fn tell_place(
    &self,
    id: winit::window::WindowId,
  ) {
    if let Some(entry) = self.shell.windows.get(&id)
      && let Some(hosted) = &entry.hosted
    {
      hosted
        .on_place
        .call(multi::place_of(&entry.window));
    }
  }

  /// Tells the component that opened window `id`, and the application,
  /// when its state has changed (M19).
  fn check_state(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: winit::window::WindowId,
  ) {
    let Some(entry) = self.shell.windows.get_mut(&id) else {
      return;
    };
    let state = state_of(&entry.window);
    if state == entry.state {
      return;
    }
    entry.state = state;
    if let Some(hosted) = &entry.hosted {
      hosted.on_state.call(state);
    }
    self.app.state_changed(
      &mut Self::context(&mut self.shell, event_loop),
      WindowId(id),
      state,
    );
  }

  /// Saves a window's place and state for `dovetail dev`.
  fn save_placement(
    &self,
    id: winit::window::WindowId,
  ) {
    if let Some(entry) = self.shell.windows.get(&id)
      && let Some(name) = &entry.dev_name
    {
      self.shell.dev.save(name, &entry.window);
    }
  }

  /// Builds what changed, lays out, repaints the damaged region of the
  /// window's canvas, and shows it.
  fn draw_ui(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: winit::window::WindowId,
  ) {
    let Shell { gpu, painter, windows, .. } = &mut self.shell;
    let (Some(gpu), Some(entry)) = (gpu.as_ref(), windows.get_mut(&id)) else {
      return;
    };
    let info = entry.info();
    let Some(ui) = entry.ui.as_mut() else {
      return;
    };
    let logical = info.logical_size();
    let shown = (!info.size.is_empty())
      .then_some((logical.width as f32, logical.height as f32));
    let update = ui.session.update(shown);
    if shown.is_none() {
      return;
    }
    let mut damage = update.damage;
    if update.focus_requested && !ui.session.active() {
      entry.window.focus_window();
    }
    if let Some(title) = update.title {
      entry.window.set_title(&title);
      // The accessibility tree names the window by its title.
      ui.access.reset();
    }
    if let Some(style) = update.style {
      // Dark windows get a dark title bar where the system can.
      entry.window.set_theme(Some(if style.dark {
        winit::window::Theme::Dark
      } else {
        winit::window::Theme::Light
      }));
      // A new background: everything is painted again.
      ui.canvas = None;
    }
    // The focus ring is drawn over the boxes: repaint where it was and
    // where it is now.
    let ring = ui.session.focus_ring();
    if ring.map(|r| r.0) != ui.ring {
      for r in [ui.ring, ring.map(|r| r.0)].into_iter().flatten() {
        damage = Some(damage.map_or(r, |d| d.union(&r)));
      }
    }
    ui.ring = ring.map(|r| r.0);

    // Assistive tools get what changed, while any is listening.
    {
      let UiWindow { adapter, access, session, .. } = &mut *ui;
      adapter.update_if_active(|| {
        access.update(
          session.layout(),
          session.focus(),
          session.title(),
          info.scale_factor as f32,
          session.measure(),
        )
      });
    }
    let caret_visible = ui.session.caret_visible();
    if caret_visible != ui.painted_caret
      && let Some(r) = ui.session.caret_rect()
    {
      // Repaint the caret's pixels, with a pixel to spare for rounding.
      let r = Rect::new(r.x - 1.0, r.y - 1.0, r.width + 2.0, r.height + 2.0);
      damage = Some(damage.map_or(r, |d| d.union(&r)));
    }
    ui.painted_caret = caret_visible;

    // Scrollbars are drawn over the boxes: repaint the old and new tracks
    // when any bar moved, widened or showed (M15).
    let bars = ui.session.scrollbars();
    if bars != ui.painted_bars {
      for (bar, _) in ui.painted_bars.iter().chain(&bars) {
        damage = Some(damage.map_or(bar.track, |d| d.union(&bar.track)));
      }
    }
    ui.painted_bars.clone_from(&bars);

    // The input method is on while the focused element takes text, and is
    // told where the caret is, for its candidate window.
    let wants_ime = ui.session.wants_ime();
    if wants_ime != ui.ime_allowed {
      entry.window.set_ime_allowed(wants_ime);
      ui.ime_allowed = wants_ime;
      ui.ime_area = None;
    }
    if wants_ime {
      let area = ui.session.caret_rect();
      if area.is_some() && area != ui.ime_area {
        let r = area.unwrap_or_default();
        entry.window.set_ime_cursor_area(
          winit::dpi::LogicalPosition::new(r.x, r.y),
          winit::dpi::LogicalSize::new(r.width.max(1.0), r.height),
        );
        ui.ime_area = area;
      }
    }

    if !painter
      .as_ref()
      .is_some_and(|p| p.is_current(gpu))
    {
      *painter = Some(Painter::new(gpu));
      ui.canvas = None;
    }
    let painter = painter.as_mut().expect("painter");
    let size = info.size;
    if !ui.canvas.as_ref().is_some_and(|c| {
      c.width() == size.width && c.height() == size.height && c.is_current(gpu)
    }) {
      ui.canvas = Some(painter.canvas(gpu, size.width, size.height));
    }
    let canvas = ui.canvas.as_mut().expect("canvas");
    if let Some(w) = ui.waiting.take() {
      damage = Some(damage.map_or(w, |d| d.union(&w)));
    }
    let region = damage.or(Some(Rect::default()));
    let stats = painter.paint(
      gpu,
      canvas,
      &dovetail_render::PaintInput {
        boxes: ui.session.layout().boxes(),
        region,
        scale: info.scale_factor as f32,
        background: ui.session.style().background,
        focus_ring: ring,
        caret_visible,
        colors: ui.session.style(),
        scrollbars: &bars,
      },
    );
    // A large image is uploaded over several frames, so ask for the next.
    if stats.waiting.is_some() {
      ui.waiting = stats.waiting;
      entry.window.request_redraw();
    }
    match painter.present(gpu, canvas, &mut entry.surface) {
      FrameOutcome::Presented => {
        entry.stats.presented += 1;
        self.shell.dev.ready();
        self.app.frame_presented(
          &mut Self::context(&mut self.shell, event_loop),
          WindowId(id),
        )
      },
      FrameOutcome::Skipped => entry.stats.skipped += 1,
      FrameOutcome::Retry => entry.window.request_redraw(),
      FrameOutcome::DeviceLost => self.shell.recover_gpu(),
    }
  }

  fn resized(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: winit::window::WindowId,
  ) {
    let shell = &mut self.shell;
    let (Some(gpu), Some(entry)) = (&shell.gpu, shell.windows.get_mut(&id))
    else {
      return;
    };
    entry
      .surface
      .resize(gpu, physical(entry.window.inner_size()));
    entry.window.request_redraw();
    let info = entry.info();
    self.app.resized(
      &mut Self::context(&mut self.shell, event_loop),
      WindowId(id),
      info,
    );
  }
}

impl<A: Application> ApplicationHandler<UserEvent> for Runner<A> {
  fn user_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    event: UserEvent,
  ) {
    let event = match event {
      UserEvent::AccessKit(event) => event,
      UserEvent::Tasks => {
        lifecycle::run_tasks(event_loop);
        return;
      },
      UserEvent::Reopen => {
        self
          .app
          .reopen(&mut Self::context(&mut self.shell, event_loop));
        return;
      },
    };
    let Some(entry) = self.shell.windows.get_mut(&event.window_id) else {
      return;
    };
    let Some(ui) = entry.ui.as_mut() else {
      return;
    };
    match event.window_event {
      accesskit_winit::WindowEvent::InitialTreeRequested => {
        // The next draw sends the whole tree.
        ui.access.reset();
        entry.window.request_redraw();
      },
      accesskit_winit::WindowEvent::ActionRequested(request) => {
        if let Some(request) = ui.access.request(&request) {
          ui.session.carry_out(request);
          entry.window.request_redraw();
        }
      },
      accesskit_winit::WindowEvent::AccessibilityDeactivated => {},
    }
  }

  fn about_to_wait(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    for work in std::mem::take(&mut self.queued) {
      work(&mut Self::context(&mut self.shell, event_loop));
    }
    // The root, which has no window, builds what changed: it may open or
    // close windows (M19).
    if let Some(root) = &mut self.shell.root
      && (self.shell.state_changed.get() || root.needs_update())
    {
      root.update(Some(root::ROOT_SIZE));
    }
    self.shell.serve_host(event_loop);
    self.shell.publish_windows();
    if self.started && self.shell.should_end() {
      event_loop.exit();
    }
    // Components whose close guard agreed close their windows.
    for window in self.shell.guarded_to_close() {
      Self::context(&mut self.shell, event_loop).close_window(window);
    }
    // Settings the program set itself, as a self-test does (M18).
    self.shell.pass_settings();
    // Frames for animation (M18, D0028): while a component asks for one,
    // start a frame, but no sooner than the display could show the last.
    // Showing it waits for the display, so frames follow its rate.
    let mut frame_at = None;
    if dovetail_core::frame_requested() {
      let now = dovetail_core::now();
      match self.shell.frame_at {
        Some(at) if at > now => frame_at = Some(at),
        _ => {
          dovetail_core::start_frame();
          self.shell.frame_at = Some(now + self.shell.frame_gap());
        },
      }
    }
    // State changed while handling events: show it.
    if self.shell.state_changed.replace(false) {
      self.shell.redraw_ui_windows();
    }
    // Sleep until the application's timer or the next caret blink.
    let next = self
      .shell
      .windows
      .values()
      .filter_map(|e| e.ui.as_ref())
      .filter_map(|ui| ui.session.next_deadline())
      .chain(self.shell.wake_at.get())
      // Timers from Task::after, such as a tooltip's wait (D0027).
      .chain(dovetail_core::next_timer())
      .chain(frame_at)
      .min();
    event_loop.set_control_flow(match next {
      Some(at) => ControlFlow::WaitUntil(at),
      None => ControlFlow::Wait,
    });
  }

  fn new_events(
    &mut self,
    event_loop: &ActiveEventLoop,
    _: StartCause,
  ) {
    // The UI thread's clock, which tests can move (D0027).
    let now = dovetail_core::now();
    // Carets whose time has come turn on or off, and scrollbars that show
    // only while scrolling hide (M15).
    for entry in self.shell.windows.values_mut() {
      if let Some(ui) = entry.ui.as_mut()
        && ui.session.tick(now)
      {
        entry.window.request_redraw();
      }
    }
    // Timers that are due fire, as a tooltip's wait ends (D0027). What
    // they change draws the windows again.
    if dovetail_core::next_timer().is_some_and(|at| at <= now) {
      dovetail_core::run_task_messages();
    }
    if self
      .shell
      .wake_at
      .get()
      .is_some_and(|at| at <= now)
    {
      self.shell.wake_at.set(None);
      self
        .app
        .woke(&mut Self::context(&mut self.shell, event_loop));
    }
  }

  fn resumed(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    if self.started {
      return;
    }
    self.started = true;
    event_loop.set_control_flow(ControlFlow::Wait);
    self.shell.refresh_monitors(event_loop);
    self
      .app
      .started(&mut Self::context(&mut self.shell, event_loop));
    if self.shell.should_end() {
      event_loop.exit();
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    id: winit::window::WindowId,
    event: WindowEvent,
  ) {
    if !self.shell.windows.contains_key(&id) {
      return;
    }
    log_input(&event);
    if let Some(entry) = self.shell.windows.get_mut(&id)
      && let Some(ui) = entry.ui.as_mut()
    {
      ui.adapter.process_event(&entry.window, &event);
    }
    // A window a modal window blocks takes no input, and the Alt press
    // that bringing a window to the front sends is not a key (M19).
    if self.shell.refuse(id, &event) || self.shell.alt_from_focus(id, &event) {
      return;
    }
    // The monitors are read again on window events, as D0029 decides.
    if matches!(
      event,
      WindowEvent::Focused(true)
        | WindowEvent::Moved(_)
        | WindowEvent::ScaleFactorChanged { .. }
    ) {
      self.shell.refresh_monitors(event_loop);
    }
    match event {
      WindowEvent::RedrawRequested => {
        if let Some(entry) = self.shell.windows.get_mut(&id) {
          entry.stats.redraws += 1;
        }
        self.draw(event_loop, id)
      },
      WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
        self.resized(event_loop, id);
        self.save_placement(id);
        // The state first, so a window maximised is not saved at its
        // maximised size as its normal one (M19).
        self.check_state(event_loop, id);
        self.tell_place(id);
      },
      WindowEvent::Moved(_) => {
        self.save_placement(id);
        self.check_state(event_loop, id);
        self.tell_place(id);
      },
      // A frame asked for while the window was hidden or minimised is
      // skipped, and nothing asks again. So draw once it can be seen.
      WindowEvent::Occluded(hidden) => {
        if let Some(entry) = self.shell.windows.get_mut(&id) {
          entry.stats.occluded = Some(hidden);
          if !hidden {
            entry.window.request_redraw();
          }
        }
        // macOS says a window is minimised only this way.
        self.check_state(event_loop, id);
      },
      WindowEvent::CloseRequested => {
        // A component that guards the window is asked first.
        if self.shell.ask_guard(id) {
          return;
        }
        // A window a component opened asks the component.
        if let Some(hosted) = self
          .shell
          .windows
          .get(&id)
          .and_then(|e| e.hosted.as_ref())
        {
          hosted.on_close.call(());
          return;
        }
        self.app.close_requested(
          &mut Self::context(&mut self.shell, event_loop),
          WindowId(id),
        );
      },
      // A key held while a window gains focus arrives as a synthetic
      // press (Windows and X11). It is not a new press: without this check,
      // a key that opens a window opens one after another while held (B0003).
      WindowEvent::ModifiersChanged(modifiers) => {
        let state = modifiers.state();
        self.shell.modifiers = Modifiers {
          shift: state.shift_key(),
          control: state.control_key(),
          alt: state.alt_key(),
          meta: state.super_key(),
        };
      },
      WindowEvent::ThemeChanged(_) => self.shell.refresh_system(),
      WindowEvent::Focused(active) => {
        if let Some(entry) = self.shell.windows.get_mut(&id) {
          entry.stats.focused = Some(active);
        }
        // Settings may have changed while another window was in front.
        if active {
          self.shell.refresh_system();
        }
        if active && let Some(entry) = self.shell.windows.get(&id) {
          self
            .shell
            .dialog_owner
            .set(lifecycle::native_handle(&entry.window));
        }
        if let Some(entry) = self.shell.windows.get_mut(&id)
          && let Some(ui) = entry.ui.as_mut()
        {
          ui.session.set_active(active);
          entry.window.request_redraw();
        }
      },
      WindowEvent::MouseWheel { delta, .. } => {
        let modifiers = self.shell.modifiers;
        if let Some(entry) = self.shell.windows.get_mut(&id)
          && let Some(ui) = entry.ui.as_mut()
        {
          // winit's deltas move the content: positive shows what is left
          // and above, so they are turned round.
          let (dx, dy, steps) = match delta {
            winit::event::MouseScrollDelta::LineDelta(x, y) => {
              (-x * WHEEL_STEP, -y * WHEEL_STEP, true)
            },
            winit::event::MouseScrollDelta::PixelDelta(p) => {
              let scale = entry.window.scale_factor();
              (-(p.x / scale) as f32, -(p.y / scale) as f32, false)
            },
          };
          if ui.session.wheel(dx, dy, steps, modifiers) {
            entry.window.request_redraw();
          }
        }
      },
      WindowEvent::Ime(ime) => {
        let event = match ime {
          winit::event::Ime::Preedit(text, cursor) => {
            Some(ImeEvent::Preedit { text, cursor })
          },
          winit::event::Ime::Commit(text) => Some(ImeEvent::Commit(text)),
          // Turning off ends any composing.
          winit::event::Ime::Disabled => {
            Some(ImeEvent::Preedit { text: String::new(), cursor: None })
          },
          winit::event::Ime::Enabled => None,
        };
        if let Some(entry) = self.shell.windows.get_mut(&id)
          && let Some(ui) = &mut entry.ui
          && let Some(event) = event
          && ui.session.ime(event)
        {
          entry.window.request_redraw();
        }
      },
      WindowEvent::KeyboardInput { event, is_synthetic: false, .. }
        if event.state == ElementState::Pressed =>
      {
        let key = key_event(&event, self.shell.modifiers);
        // Escape gives up a drag.
        if self.shell.dragging() && key.is(NamedKey::Escape) {
          self.shell.cancel_drag();
          return;
        }
        let used = match self
          .shell
          .windows
          .get_mut(&id)
          .and_then(|e| e.ui.as_mut())
        {
          Some(ui) => {
            let used = ui.session.key(&key);
            if input_log().is_some() {
              eprintln!("input            used by the window: {used}");
            }
            if used {
              // Focus may have moved; state changes redraw by themselves.
              self.shell.windows[&id].window.request_redraw();
            }
            used
          },
          None => false,
        };
        if !used && !key.repeat {
          self.app.key_pressed(
            &mut Self::context(&mut self.shell, event_loop),
            WindowId(id),
            &key,
          );
        }
      },
      WindowEvent::CursorMoved { position, .. } => {
        let modifiers = self.shell.modifiers;
        if let Some(entry) = self.shell.windows.get(&id) {
          let scale = entry.window.scale_factor();
          let at = ((position.x / scale) as f32, (position.y / scale) as f32);
          self.shell.pointer_moved(id, at, modifiers);
        }
      },
      // During a drag the pointer is still followed outside the window.
      WindowEvent::CursorLeft { .. } if self.shell.dragging() => {},
      WindowEvent::CursorLeft { .. } => {
        if let Some(entry) = self.shell.windows.get_mut(&id)
          && let Some(ui) = entry.ui.as_mut()
          && ui.session.pointer_left()
        {
          entry.window.request_redraw();
        }
      },
      WindowEvent::MouseInput { state, button, .. } => {
        let button = match button {
          MouseButton::Left => PointerButton::Primary,
          MouseButton::Middle => PointerButton::Middle,
          MouseButton::Right => PointerButton::Secondary,
          _ => return,
        };
        let modifiers = self.shell.modifiers;
        match state {
          ElementState::Pressed => {
            self.shell.pointer_pressed(id, modifiers, button)
          },
          ElementState::Released => {
            self.shell.pointer_released(id, modifiers, button)
          },
        }
      },
      _ => {},
    }
  }
}

/// Runs an application until its last window closes.
pub fn run(app: impl Application) -> Result<(), Error> {
  let event_loop = EventLoop::<UserEvent>::with_user_event()
    .build()
    .map_err(Error::EventLoop)?;
  let (shell, _tasks) = start(&event_loop, false);
  let mut runner = Runner {
    app,
    shell,
    started: false,
    queued: Vec::new(),
  };
  let result = event_loop
    .run_app(&mut runner)
    .map_err(Error::EventLoop);
  finish(&mut runner);
  result
}

/// Installs the hooks the framework needs while a loop runs, and makes the
/// shell. For a test (`pumped`), the clipboard is kept in memory, and there
/// are no native menus or file dialogs, so nothing reaches the system.
fn start(
  event_loop: &EventLoop<UserEvent>,
  pumped: bool,
) -> (Shell, lifecycle::TaskWaker) {
  let state_changed = Rc::new(Cell::new(false));
  let flag = Rc::clone(&state_changed);
  dovetail_core::set_on_change(Some(Box::new(move || flag.set(true))));
  let dialog_owner = Rc::new(Cell::new(0));
  if pumped {
    dovetail_core::set_clipboard(None);
    dovetail_core::set_native_menus(None);
    dovetail_core::set_file_dialogs(None);
  } else {
    // Controls copy and paste through the system clipboard while it runs.
    dovetail_core::set_clipboard(crate::platform::clipboard());
    // Menu bars use the system's own where it has one (macOS).
    dovetail_core::set_native_menus(crate::platform::native_menus());
    // Open and Save As use the system's dialogs, owned by the window in
    // front.
    dovetail_core::set_file_dialogs(crate::platform::file_dialogs(Rc::clone(
      &dialog_owner,
    )));
  }
  // Components open windows, such as floating panels, through the host.
  let host = Rc::new(multi::HostQueue::default());
  dovetail_core::set_window_host(Some(Rc::new(multi::DesktopHost(Rc::clone(
    &host,
  )))));
  // Han characters are drawn the way the system's language draws them,
  // unless DOVETAIL_LANG chose a language already.
  if std::env::var_os("DOVETAIL_LANG").is_none()
    && let Some(tag) = crate::platform::language()
  {
    dovetail_text::Fonts::system()
      .set_language(dovetail_text::Language::from_tag(&tag));
  }
  let shell = Shell {
    display: event_loop.owned_display_handle(),
    gpu: None,
    painter: None,
    windows: HashMap::new(),
    state_changed,
    primary: None,
    modifiers: Modifiers::default(),
    wake_at: Cell::new(None),
    frame_at: None,
    proxy: event_loop.create_proxy(),
    dev: DevState::from_env(),
    drag: None,
    host,
    dialog_owner,
    pumped,
    // Programs written against `Application` end with their last window,
    // as they always have; `run_windows` chooses by the system.
    exit_policy: ExitPolicy::AfterLastWindow,
    root: None,
  };
  // Themes follow the system's light or dark mode, accent colour and
  // contrast (M14).
  lifecycle::refresh_appearance();
  // Tasks wake the event loop while it runs.
  let tasks = lifecycle::TaskWaker::install(event_loop.create_proxy());
  (shell, tasks)
}

/// Takes the hooks away again, and closes the windows before the GPU.
fn finish<A: Application>(runner: &mut Runner<A>) {
  dovetail_core::set_on_change(None);
  dovetail_core::set_clipboard(None);
  dovetail_core::set_native_menus(None);
  dovetail_core::set_window_host(None);
  // Component trees, canvases and surfaces must go before the GPU.
  runner.shell.root = None;
  runner.shell.windows.clear();
  runner.shell.painter = None;
}

/// A winit key press as a Dovetail [`KeyEvent`].
fn key_event(
  event: &winit::event::KeyEvent,
  modifiers: Modifiers,
) -> KeyEvent {
  let named = |k| KeyCode::Named(k);
  let key = match &event.logical_key {
    WinitKey::Character(text) => KeyCode::Character(text.to_string()),
    WinitKey::Named(n) => match n {
      WinitNamedKey::Enter => named(NamedKey::Enter),
      WinitNamedKey::Escape => named(NamedKey::Escape),
      WinitNamedKey::Tab => named(NamedKey::Tab),
      WinitNamedKey::Space => named(NamedKey::Space),
      WinitNamedKey::Backspace => named(NamedKey::Backspace),
      WinitNamedKey::Delete => named(NamedKey::Delete),
      WinitNamedKey::ArrowLeft => named(NamedKey::ArrowLeft),
      WinitNamedKey::ArrowRight => named(NamedKey::ArrowRight),
      WinitNamedKey::ArrowUp => named(NamedKey::ArrowUp),
      WinitNamedKey::ArrowDown => named(NamedKey::ArrowDown),
      WinitNamedKey::Home => named(NamedKey::Home),
      WinitNamedKey::End => named(NamedKey::End),
      WinitNamedKey::PageUp => named(NamedKey::PageUp),
      WinitNamedKey::PageDown => named(NamedKey::PageDown),
      WinitNamedKey::Insert => named(NamedKey::Insert),
      WinitNamedKey::F1 => named(NamedKey::F1),
      WinitNamedKey::F2 => named(NamedKey::F2),
      WinitNamedKey::F3 => named(NamedKey::F3),
      WinitNamedKey::F4 => named(NamedKey::F4),
      WinitNamedKey::F5 => named(NamedKey::F5),
      WinitNamedKey::F6 => named(NamedKey::F6),
      WinitNamedKey::F7 => named(NamedKey::F7),
      WinitNamedKey::F8 => named(NamedKey::F8),
      WinitNamedKey::F9 => named(NamedKey::F9),
      WinitNamedKey::F10 => named(NamedKey::F10),
      WinitNamedKey::F11 => named(NamedKey::F11),
      WinitNamedKey::F12 => named(NamedKey::F12),
      WinitNamedKey::ContextMenu => named(NamedKey::ContextMenu),
      _ => KeyCode::Other,
    },
    _ => KeyCode::Other,
  };
  // Shortcuts type nothing, and control characters are not text. AltGr
  // arrives as Ctrl+Alt on Windows, and does type.
  let shortcut = (modifiers.control && !modifiers.alt) || modifiers.meta;
  let text = event
    .text
    .as_ref()
    .map(|t| {
      t.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
    })
    .filter(|t| !t.is_empty() && !shortcut);
  KeyEvent {
    key,
    modifiers,
    text,
    repeat: event.repeat,
  }
}
