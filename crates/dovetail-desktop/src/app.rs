use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dovetail_core::{
  Callback, Element, ImeEvent, KeyCode, KeyEvent, Modifiers, NamedKey,
  PointerButton, PointerEvent, PointerKind, Tree, View, WheelEvent,
  WindowPlace,
};
use dovetail_layout::{BoxKey, Layout, LayoutBox, Rect};
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
mod multi;

use crate::access::{Access, Request};
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
}

impl WindowOptions {
  /// A window with the given title and a starting size of 800 × 600.
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title: title.into(),
      size: LogicalSize::new(800.0, 600.0),
      position: None,
    }
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
    ui.layout.element(ui.focus.as_ref()?)
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
      .map(UiWindow::bars_to_paint)
      .unwrap_or_default()
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
    let mut tree = Tree::new();
    tree.mount_within(root, context);
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
    entry.ui = Some(UiWindow {
      title,
      adapter,
      access: Access::default(),
      tree,
      layout: Layout::new(),
      canvas: None,
      cursor: (f32::NAN, f32::NAN),
      pressed: None,
      focus: None,
      focus_visible: false,
      ring: None,
      captured: None,
      last_click: None,
      ime_allowed: false,
      ime_area: None,
      caret_on: true,
      blink_at: None,
      focus_history: Vec::new(),
      autofocused: HashSet::new(),
      painted_caret: true,
      waiting: None,
      active: entry.window.has_focus(),
      sizes: HashMap::new(),
      viewports: HashMap::new(),
      guard: None,
      style: dovetail_core::WindowStyle::default(),
      hovered: Vec::new(),
      within: Vec::new(),
      bar_hot: None,
      bar_drag: None,
      painted_bars: Vec::new(),
      bars_hide_at: None,
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
      ui.cursor = (x, y);
      if !dragging && ui.moved(modifiers) {
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
    {
      let before = (ui.focus.clone(), ui.focus_visible);
      ui.press(modifiers, button);
      if (ui.focus.clone(), ui.focus_visible) != before || ui.bar_hot.is_some()
      {
        entry.window.request_redraw();
      }
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
      ui.release(modifiers, button);
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
      .map_or(&[], |ui| ui.layout.boxes())
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
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let used = entry.ui.as_mut().is_some_and(|ui| ui.key(event));
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
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let scrolled = entry.ui.as_mut().is_some_and(|ui| {
      ui.cursor = (x, y);
      ui.wheel(dx, dy, false, Modifiers::default())
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
      ui.cursor = (x, y);
      ui.wheel(0.0, steps * WHEEL_STEP, true, modifiers)
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
    let Some(entry) = self.shell.windows.get_mut(&window.0) else {
      return false;
    };
    let used = entry.ui.as_mut().is_some_and(|ui| ui.ime(event));
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
      .filter(|ui| ui.blink_at.is_some())
      .map(|ui| ui.caret_on)
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
      PointerKind::Down => {
        if let Some(ui) = self
          .shell
          .windows
          .get_mut(&window.0)
          .and_then(|e| e.ui.as_mut())
        {
          ui.cursor = (x, y);
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
    if let Some(ui) = self
      .shell
      .windows
      .get_mut(&window.0)
      .and_then(|e| e.ui.as_mut())
    {
      ui.cursor = (f32::NAN, f32::NAN);
      ui.update_hover();
    }
  }

  /// Closes a window. When the last window closes, the application stops.
  pub fn close_window(
    &mut self,
    window: WindowId,
  ) {
    self.shell.windows.remove(&window.0);
    if self.shell.primary == Some(window.0) {
      self.shell.close_hosted();
    }
    if self.shell.windows.is_empty() {
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

  /// Asks for a window's drawing area to have a new size. The platform may
  /// refuse or adjust it; [`Application::resized`] reports the result.
  pub fn set_window_size(
    &self,
    window: WindowId,
    size: LogicalSize,
  ) {
    if let Some(entry) = self.shell.windows.get(&window.0) {
      let _ = entry
        .window
        .request_inner_size(winit::dpi::LogicalSize::new(
          size.width,
          size.height,
        ));
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
      .set(Some(Instant::now() + delay));
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
}

/// A window that shows a component tree.
struct UiWindow {
  /// The window's title, which names it for assistive tools.
  title: String,
  /// Connects the window to assistive tools (M07).
  adapter: accesskit_winit::Adapter,
  /// The accessibility tree, as last sent.
  access: Access,
  tree: Tree,
  layout: Layout,
  /// Made on the first draw, and again after a resize or device loss.
  canvas: Option<Canvas>,
  /// The pointer, in logical pixels.
  cursor: (f32, f32),
  /// The handler under the pointer when it was pressed.
  pressed: Option<Callback<()>>,
  /// The element with keyboard focus.
  focus: Option<BoxKey>,
  /// Focus came from the keyboard, so the focus ring is shown.
  focus_visible: bool,
  /// Where the focus ring was last drawn, so it can be cleared.
  ring: Option<Rect>,
  /// The element with a pointer handler that the pointer went down on,
  /// and the click count. It receives moves and the release.
  captured: Option<(BoxKey, u32, PointerButton)>,
  /// When and where the last press was, and its click count, for double
  /// clicks.
  last_click: Option<(Instant, (f32, f32), u32)>,
  /// The input method is on, because the focused element takes text.
  ime_allowed: bool,
  /// Where the input method was last told the caret is.
  ime_area: Option<Rect>,
  /// The blinking caret is in its on half.
  caret_on: bool,
  /// Elements that had focus before, most recent last, to give focus back
  /// to when the focused element goes away.
  focus_history: Vec<BoxKey>,
  /// The elements that ask for focus when they appear, as last laid out.
  autofocused: HashSet<BoxKey>,
  /// When the caret next turns on or off, while it blinks.
  blink_at: Option<Instant>,
  /// Whether the caret was shown in the last paint.
  painted_caret: bool,
  /// Where images still being uploaded were left out of the last paint
  /// (M17): painted again in the next frame.
  waiting: Option<Rect>,
  /// The window has the keyboard: it is in front. Carets show only then.
  active: bool,
  /// The sizes last told to elements with a size handler.
  sizes: HashMap<BoxKey, (f32, f32)>,
  /// The viewports last told to elements with a scroll handler (M16).
  viewports: HashMap<BoxKey, dovetail_core::Viewport>,
  /// The close guard of an element in the window, as last laid out (M11).
  guard: Option<(dovetail_core::CloseGuard, Callback<()>)>,
  /// How the window looks, from its outermost theme (M14).
  style: dovetail_core::WindowStyle,
  /// The elements with a hover handler under the pointer, as last told
  /// (M15).
  hovered: Vec<BoxKey>,
  /// The elements with a focus-within handler that hold focus, as last
  /// told (M15).
  within: Vec<BoxKey>,
  /// The scrollbar under the pointer, by element and direction (M15).
  bar_hot: Option<(BoxKey, bool)>,
  /// The scrollbar whose thumb is being dragged, and how far along the
  /// thumb it was grabbed (M15).
  bar_drag: Option<(BoxKey, bool, f32)>,
  /// The scrollbars as last painted, each with whether it was wide.
  painted_bars: Vec<(dovetail_layout::Scrollbar, bool)>,
  /// When scrollbars that show only while scrolling hide again (M15).
  bars_hide_at: Option<Instant>,
}

/// When input logging started, if the environment variable
/// `DOVETAIL_LOG_INPUT` is set. Logging writes every key, input method and
/// focus event to stderr, to look into input bugs such as B0004.
fn input_log() -> Option<Instant> {
  thread_local! {
    static START: Option<Instant> =
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

/// How long the caret stays on, and then off, read from the system once.
fn blink_rate() -> Option<Duration> {
  thread_local! {
    static RATE: Option<Duration> = crate::platform::caret_blink_time();
  }
  RATE.with(|r| *r)
}

/// How long scrollbars that show only while scrolling stay after it stops.
const BARS_SHOWN_FOR: Duration = Duration::from_millis(1000);
/// How many times a frame builds and lays out while what layout reports
/// changes the tree (M16).
const LAYOUT_PASSES: usize = 4;

/// How far one notch of the mouse wheel scrolls, in logical pixels.
const WHEEL_STEP: f32 = 48.0;

/// Presses closer together than this in time count as one double click.
const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(500);
/// Presses closer together than this, in logical pixels, count as one
/// double click.
const DOUBLE_CLICK_DISTANCE: f32 = 4.0;

impl UiWindow {
  /// Moves focus to `focus`, telling the element that loses it and the one
  /// that gains it.
  fn set_focus(
    &mut self,
    focus: Option<BoxKey>,
  ) {
    if focus == self.focus {
      return;
    }
    let old = std::mem::replace(&mut self.focus, focus);
    if let Some(old) = &old {
      self.focus_history.retain(|k| k != old);
      self.focus_history.push(old.clone());
      if self.focus_history.len() > 32 {
        self.focus_history.remove(0);
      }
    }
    let tell = |key: &Option<BoxKey>, focused: bool| {
      if let Some(on_focus) = key
        .as_ref()
        .and_then(|k| self.layout.element(k))
        .and_then(|e| e.on_focus.as_ref())
      {
        on_focus.call(focused);
      }
    };
    tell(&old, false);
    tell(&self.focus, true);
    self.update_focus_within();
  }

  /// Tells elements with a hover handler that the pointer came onto them
  /// or left them (M15). Run after the pointer moves and after layout,
  /// since things can move under a still pointer.
  fn update_hover(&mut self) {
    let (x, y) = self.cursor;
    let now = if x.is_nan() || y.is_nan() {
      Vec::new()
    } else {
      self.layout.hover_targets_at(x, y)
    };
    if now == self.hovered {
      return;
    }
    let before = std::mem::replace(&mut self.hovered, now);
    self.tell_changes(&before, |e| e.on_hover.as_ref(), true);
  }

  /// Tells elements with a focus-within handler that focus came into them
  /// or went out (M15).
  fn update_focus_within(&mut self) {
    let now = self
      .focus
      .as_ref()
      .map(|f| self.layout.focus_within_targets(f))
      .unwrap_or_default();
    if now == self.within {
      return;
    }
    let before = std::mem::replace(&mut self.within, now);
    self.tell_changes(&before, |e| e.on_focus_within.as_ref(), false);
  }

  /// Calls `handler` with `false` on elements in `before` that are not in
  /// the new list, then with `true` on new ones.
  fn tell_changes(
    &self,
    before: &[BoxKey],
    handler: impl Fn(&dovetail_core::Element) -> Option<&Callback<bool>>,
    hover: bool,
  ) {
    let now = if hover { &self.hovered } else { &self.within };
    let call = |key: &BoxKey, on: bool| {
      if let Some(h) = self.layout.element(key).and_then(&handler) {
        h.call(on);
      }
    };
    for key in before.iter().filter(|k| !now.contains(k)) {
      call(key, false);
    }
    for key in now.iter().filter(|k| !before.contains(k)) {
      call(key, true);
    }
  }

  fn press(
    &mut self,
    modifiers: Modifiers,
    button: PointerButton,
  ) {
    self.wake_caret();
    let (x, y) = self.cursor;
    if self.captured.is_some() || self.bar_drag.is_some() {
      // Another button is already held; this one is ignored.
      return;
    }
    if button == PointerButton::Primary && self.press_scrollbar(x, y) {
      return;
    }
    if button == PointerButton::Primary {
      // A click focuses what it lands on, without showing the ring.
      self.set_focus(self.layout.focusable_at(x, y));
      self.focus_visible = false;
      self.pressed = self.layout.hit_test(x, y).cloned();
    }

    let now = Instant::now();
    let clicks = match self.last_click {
      Some((at, (px, py), n))
        if now.duration_since(at) <= DOUBLE_CLICK_TIME
          && (px - x).abs() <= DOUBLE_CLICK_DISTANCE
          && (py - y).abs() <= DOUBLE_CLICK_DISTANCE =>
      {
        n + 1
      },
      _ => 1,
    };
    self.last_click = Some((now, (x, y), clicks));
    if button == PointerButton::Secondary
      && let Some(on_menu) = self
        .layout
        .context_menu_target_at(x, y)
        .and_then(|k| self.layout.element(&k))
        .and_then(|e| e.on_context_menu.clone())
    {
      on_menu.call((x, y));
    }
    self.captured = self
      .layout
      .pointer_target_at(x, y)
      .map(|key| (key, clicks, button));
    self.send_pointer(PointerKind::Down, modifiers);
  }

  /// The wheel over the pointer: an element that handles the wheel gets it
  /// (as a canvas zooms); otherwise what is under the pointer scrolls by
  /// (`dx`, `dy`) logical pixels, sideways with Shift. True when something
  /// used it.
  fn wheel(
    &mut self,
    dx: f32,
    dy: f32,
    steps: bool,
    modifiers: Modifiers,
  ) -> bool {
    let (x, y) = self.cursor;
    if x.is_nan() {
      return false;
    }
    if let Some(key) = self.layout.wheel_target_at(x, y)
      && let (Some(element), Some(rect)) =
        (self.layout.element(&key), self.layout.rect_of(&key))
      && let Some(on_wheel) = element.on_wheel.clone()
    {
      on_wheel.call(WheelEvent {
        x: x - rect.x,
        y: y - rect.y,
        dx,
        dy,
        steps,
        modifiers,
      });
      return true;
    }
    let (dx, dy) = if modifiers.shift && dx == 0.0 {
      (dy, 0.0)
    } else {
      (dx, dy)
    };
    let scrolled = self.layout.scroll_by(x, y, dx, dy);
    if scrolled {
      self.show_bars();
    }
    scrolled
  }

  /// A press with the main button on a scrollbar (M15): on the thumb it
  /// starts a drag, and on the track it moves one page towards the
  /// pointer. True when there was a bar there.
  fn press_scrollbar(
    &mut self,
    x: f32,
    y: f32,
  ) -> bool {
    let Some(bar) = self.layout.scrollbar_at(x, y) else {
      return false;
    };
    let Some(((ox, oy), _)) = self.layout.scroll_extent(&bar.key) else {
      return false;
    };
    let (along, start, length) = if bar.vertical {
      (y, bar.thumb.y, bar.thumb.height)
    } else {
      (x, bar.thumb.x, bar.thumb.width)
    };
    if (start..start + length).contains(&along) {
      self.bar_drag = Some((bar.key.clone(), bar.vertical, along - start));
    } else {
      let page = if along < start { -bar.page } else { bar.page };
      if bar.vertical {
        self
          .layout
          .scroll_to(&bar.key, None, Some(oy + page));
      } else {
        self
          .layout
          .scroll_to(&bar.key, Some(ox + page), None);
      }
    }
    self.bar_hot = Some((bar.key, bar.vertical));
    self.show_bars();
    true
  }

  /// Drags a scrollbar's thumb to the pointer. True when it moved.
  fn drag_scrollbar(&mut self) -> bool {
    let Some((key, vertical, grab)) = self.bar_drag.clone() else {
      return false;
    };
    let (x, y) = self.cursor;
    let Some(bar) = self
      .layout
      .scrollbars()
      .into_iter()
      .find(|b| b.key == key && b.vertical == vertical)
    else {
      return false;
    };
    let Some((_, (max_x, max_y))) = self.layout.scroll_extent(&key) else {
      return false;
    };
    self.show_bars();
    if vertical {
      let offset = bar.offset_for(y - grab - bar.track.y, max_y);
      self.layout.scroll_to(&key, None, Some(offset))
    } else {
      let offset = bar.offset_for(x - grab - bar.track.x, max_x);
      self.layout.scroll_to(&key, Some(offset), None)
    }
  }

  /// Shows scrollbars that show only while scrolling, for a second.
  fn show_bars(&mut self) {
    if crate::platform::overlay_scrollbars() {
      self.bars_hide_at = Some(Instant::now() + BARS_SHOWN_FOR);
    }
  }

  /// The scrollbars to paint now, each with whether it is wide.
  fn bars_to_paint(&self) -> Vec<(dovetail_layout::Scrollbar, bool)> {
    let overlay = crate::platform::overlay_scrollbars();
    let shown = self.bars_hide_at.is_some()
      || self.bar_drag.is_some()
      || self.bar_hot.is_some();
    if overlay && !shown {
      return Vec::new();
    }
    let dragged = self.bar_drag.as_ref().map(|d| (&d.0, d.1));
    let hot = self.bar_hot.as_ref().map(|h| (&h.0, h.1));
    self
      .layout
      .scrollbars()
      .into_iter()
      .map(|b| {
        let this = Some((&b.key, b.vertical));
        let wide = this == dragged || this == hot;
        (b, wide)
      })
      .collect()
  }

  /// After a layout: an element that asks for focus and has just appeared
  /// takes it; otherwise, if the focused element went away, focus goes back
  /// to the one focused before it that is still there.
  fn settle_focus(&mut self) {
    let wanting: Vec<BoxKey> = self
      .layout
      .boxes()
      .iter()
      .filter(|b| {
        matches!(&b.content, dovetail_layout::Content::Element(e) if e.autofocus)
      })
      .map(|b| b.key.clone())
      .collect();
    let new = wanting
      .iter()
      .find(|k| !self.autofocused.contains(*k) && self.layout.can_focus(k))
      .cloned();
    self.autofocused = wanting.into_iter().collect();
    if let Some(key) = new {
      self.set_focus(Some(key));
      self.focus_visible = true;
      return;
    }
    if self
      .focus
      .as_ref()
      .is_some_and(|f| !self.layout.can_focus(f))
    {
      // The focused element is gone: it is not kept in the history.
      self.focus = None;
      while let Some(previous) = self.focus_history.pop() {
        if self.layout.can_focus(&previous) {
          self.set_focus(Some(previous));
          return;
        }
      }
    }
  }

  /// Carries out a request from an assistive tool.
  fn carry_out(
    &mut self,
    request: Request,
  ) {
    self.wake_caret();
    match request {
      Request::Focus(key) => {
        if self.layout.can_focus(&key) {
          self.set_focus(Some(key));
          self.focus_visible = true;
        }
      },
      Request::Click(key) => {
        if let Some(on_press) = self
          .layout
          .element(&key)
          .and_then(|e| e.on_press.clone())
        {
          on_press.call(());
        }
      },
      Request::Edit(key, action) => {
        if let Some(on_access) = self
          .layout
          .element(&key)
          .and_then(|e| e.on_access.clone())
        {
          on_access.call(action);
        }
      },
    }
  }

  /// Shows the caret and starts its blink again, as after typing or a
  /// click, so it does not vanish while in use.
  fn wake_caret(&mut self) {
    self.caret_on = true;
    self.blink_at = None;
  }

  /// True when the focused element has a caret showing.
  fn has_caret(&self) -> bool {
    self
      .focus
      .as_ref()
      .and_then(|f| self.layout.element(f))
      .and_then(|e| e.text_cursor)
      .is_some_and(|c| c.show_caret)
  }

  /// True when the focused element takes text from an input method.
  fn wants_ime(&self) -> bool {
    self
      .focus
      .as_ref()
      .and_then(|f| self.layout.element(f))
      .is_some_and(|e| e.on_ime.is_some())
  }

  /// Passes input method text to the focused element. True when it took
  /// it.
  fn ime(
    &mut self,
    event: ImeEvent,
  ) -> bool {
    self.wake_caret();
    match self
      .focus
      .as_ref()
      .and_then(|f| self.layout.element(f))
      .and_then(|e| e.on_ime.as_ref())
    {
      Some(on_ime) => {
        on_ime.call(event);
        true
      },
      None => false,
    }
  }

  /// Passes the pointer to the captured element, if there is one.
  fn send_pointer(
    &self,
    kind: PointerKind,
    modifiers: Modifiers,
  ) {
    let Some((key, clicks, button)) = &self.captured else {
      return;
    };
    let (Some(element), Some(rect)) =
      (self.layout.element(key), self.layout.rect_of(key))
    else {
      return;
    };
    let Some(handler) = &element.on_pointer else {
      return;
    };
    let (x, y) = self.cursor;
    if x.is_nan() || y.is_nan() {
      return;
    }
    handler.handle(&PointerEvent {
      kind,
      button: *button,
      x: x - rect.x,
      y: y - rect.y,
      window_x: x,
      window_y: y,
      text_at: self.layout.text_at(key, x, y, &font_measure()),
      modifiers,
      clicks: *clicks,
    });
  }

  /// The pointer moved. True when a scrollbar changed, so the window must
  /// be drawn again.
  fn moved(
    &mut self,
    modifiers: Modifiers,
  ) -> bool {
    self.send_pointer(PointerKind::Move, modifiers);
    self.update_hover();
    let (x, y) = self.cursor;
    let hot = match &self.bar_drag {
      Some((key, vertical, _)) => Some((key.clone(), *vertical)),
      None if x.is_nan() => None,
      None => self
        .layout
        .scrollbar_at(x, y)
        .map(|b| (b.key, b.vertical)),
    };
    let changed = hot != self.bar_hot;
    self.bar_hot = hot;
    self.drag_scrollbar() || changed
  }

  /// A click happens when the pointer is released over the handler it was
  /// pressed on.
  fn release(
    &mut self,
    modifiers: Modifiers,
    button: PointerButton,
  ) {
    if button == PointerButton::Primary && self.bar_drag.take().is_some() {
      return;
    }
    if self
      .captured
      .as_ref()
      .is_some_and(|c| c.2 == button)
    {
      self.send_pointer(PointerKind::Up, modifiers);
      self.captured = None;
    }
    if button != PointerButton::Primary {
      return;
    }
    let Some(pressed) = self.pressed.take() else {
      return;
    };
    if self.layout.hit_test(self.cursor.0, self.cursor.1) == Some(&pressed) {
      pressed.call(());
    }
  }

  /// Offers a key to the window: Tab and Shift+Tab move focus, and other
  /// keys go to the focused element and its parents. Returns whether the
  /// window used the key.
  fn key(
    &mut self,
    event: &KeyEvent,
  ) -> bool {
    self.wake_caret();
    let m = event.modifiers;
    if event.is(NamedKey::Tab) && !m.control && !m.alt && !m.meta {
      let next = self
        .layout
        .focus_next(self.focus.as_ref(), m.shift);
      self.set_focus(next);
      self.focus_visible = true;
      return true;
    }
    // The Menu key and Shift+F10 open a context menu below the focused
    // element (M15).
    let menu_key = event.is(NamedKey::ContextMenu)
      || (event.is(NamedKey::F10) && m.shift && !m.control && !m.alt);
    if menu_key
      && let Some(focus) = self.focus.clone()
      && let Some(key) = self.layout.context_menu_target_for(&focus)
      && let Some(on_menu) = self
        .layout
        .element(&key)
        .and_then(|e| e.on_context_menu.clone())
    {
      let r = self.layout.rect_of(&focus).unwrap_or_default();
      on_menu.call((r.x, r.y + r.height));
      return true;
    }
    self
      .layout
      .dispatch_key(self.focus.as_ref(), event)
  }

  /// The focus ring to draw: just outside the focused element, when focus
  /// came from the keyboard.
  fn focus_ring(&self) -> Option<(Rect, f32)> {
    if !self.focus_visible {
      return None;
    }
    let (r, radius) = self.layout.focus_ring(self.focus.as_ref()?)?;
    let out = FOCUS_RING_GAP + FOCUS_RING_WIDTH;
    Some((
      Rect::new(
        r.x - out,
        r.y - out,
        r.width + 2.0 * out,
        r.height + 2.0 * out,
      ),
      radius + out,
    ))
  }
}

/// Space between an element and its focus ring, in logical pixels.
const FOCUS_RING_GAP: f32 = 1.0;
/// Width of the focus ring, in logical pixels.
const FOCUS_RING_WIDTH: f32 = 2.0;

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
}

impl Shell {
  fn open_window(
    &mut self,
    event_loop: &ActiveEventLoop,
    options: WindowOptions,
    visible: bool,
  ) -> Result<WindowId, Error> {
    let mut attributes = WindowAttributes::default()
      .with_visible(visible)
      .with_title(options.title)
      .with_inner_size(winit::dpi::LogicalSize::new(
        options.size.width,
        options.size.height,
      ));
    let first = self.primary.is_none();
    if let Some((x, y)) = options.position {
      attributes = attributes.with_position(winit::dpi::LogicalPosition::new(
        f64::from(x),
        f64::from(y),
      ));
    }
    if first && let Some(p) = self.dev.placement() {
      attributes = attributes
        .with_position(winit::dpi::PhysicalPosition::new(p.x, p.y))
        .with_inner_size(winit::dpi::PhysicalSize::new(p.width, p.height));
    }
    let window = Arc::new(
      event_loop
        .create_window(attributes)
        .map_err(Error::Window)?,
    );
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
    self
      .windows
      .insert(id, WindowEntry { window, surface, ui: None, hosted: None });
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
    );
    let windows: Vec<Kept> = self
      .windows
      .drain()
      .map(|(id, entry)| {
        let mut ui = entry.ui;
        if let Some(ui) = &mut ui {
          ui.canvas = None;
        }
        (id, entry.window, ui, entry.hosted)
      })
      .collect();

    // 2. Drop the lost GPU, so nothing holds the old swapchains.
    let lost_generation = lost.generation();
    drop(lost);

    // 3. Make a new GPU with the first window, and surfaces for the others.
    let mut new_gpu: Option<Gpu> = None;
    for (id, window, ui, hosted) in windows {
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
          self
            .windows
            .insert(id, WindowEntry { window, surface, ui, hosted });
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

fn physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
  PhysicalSize { width: size.width, height: size.height }
}

/// Events the event loop carries besides the system's own.
pub(crate) enum UserEvent {
  /// A request from an assistive tool, through AccessKit (M07).
  AccessKit(accesskit_winit::Event),
  /// A task has progress or an end to give to its callbacks (M11).
  Tasks,
}

impl From<accesskit_winit::Event> for UserEvent {
  fn from(event: accesskit_winit::Event) -> Self {
    UserEvent::AccessKit(event)
  }
}

struct Runner<A: Application> {
  app: A,
  shell: Shell,
  started: bool,
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
        self.shell.dev.ready();
        self.app.frame_presented(
          &mut Self::context(&mut self.shell, event_loop),
          WindowId(id),
        )
      },
      FrameOutcome::Skipped => {},
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

  /// Saves the first window's place for `dovetail dev`.
  fn save_placement(
    &self,
    id: winit::window::WindowId,
  ) {
    if self.shell.primary == Some(id)
      && let Some(entry) = self.shell.windows.get(&id)
    {
      self.shell.dev.save(&entry.window);
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
    ui.tree.update();
    if info.size.is_empty() {
      return;
    }
    let logical = info.logical_size();
    // Sizes and scroll positions told after layout can change what is
    // built, as a long list makes the rows that came into sight (M16,
    // D0022): build and lay out again, a few times at most, so the frame
    // shows them. Anything left waits for the next frame.
    let mut damage: Option<Rect> = None;
    for pass in 0..LAYOUT_PASSES {
      if pass > 0 {
        if !ui.tree.needs_update() {
          break;
        }
        ui.tree.update();
      }
      let changes = ui.layout.update(
        &ui.tree,
        logical.width as f32,
        logical.height as f32,
        &font_measure(),
      );
      if let Some(d) = changes.damage_bounds() {
        damage = Some(damage.map_or(d, |old| old.union(&d)));
      }
      ui.settle_focus();
      ui.update_focus_within();
      ui.update_hover();
      if ui.take_focus_request() && !ui.active {
        entry.window.focus_window();
      }
      ui.report_sizes();
      ui.report_scrolls();
    }
    if let Some(title) = ui.read_lifecycle() {
      entry.window.set_title(&title);
    }
    if let Some(style) = ui.read_style() {
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
    let ring = ui.focus_ring();
    if ring.map(|r| r.0) != ui.ring {
      for r in [ui.ring, ring.map(|r| r.0)].into_iter().flatten() {
        damage = Some(damage.map_or(r, |d| d.union(&r)));
      }
    }
    ui.ring = ring.map(|r| r.0);

    // The caret blinks while the window is in front and the focused
    // element shows one; otherwise it stays on, or off in the background.
    let blinking = ui.active && ui.has_caret();
    match (blinking, blink_rate()) {
      (true, Some(rate)) => {
        if ui.blink_at.is_none() {
          ui.blink_at = Some(Instant::now() + rate);
        }
      },
      _ => {
        ui.caret_on = true;
        ui.blink_at = None;
      },
    }
    // Assistive tools get what changed, while any is listening.
    {
      let UiWindow {
        adapter, access, layout, focus, title, ..
      } = &mut *ui;
      adapter.update_if_active(|| {
        access.update(
          layout,
          focus.as_ref(),
          title,
          info.scale_factor as f32,
          &font_measure(),
        )
      });
    }
    let caret_visible = ui.active && ui.caret_on;
    if caret_visible != ui.painted_caret
      && let Some(r) = ui
        .focus
        .as_ref()
        .and_then(|f| ui.layout.caret_rect(f, &font_measure()))
    {
      // Repaint the caret's pixels, with a pixel to spare for rounding.
      let r = Rect::new(r.x - 1.0, r.y - 1.0, r.width + 2.0, r.height + 2.0);
      damage = Some(damage.map_or(r, |d| d.union(&r)));
    }
    ui.painted_caret = caret_visible;

    // Scrollbars are drawn over the boxes: repaint the old and new tracks
    // when any bar moved, widened or showed (M15).
    let bars = ui.bars_to_paint();
    if bars != ui.painted_bars {
      for (bar, _) in ui.painted_bars.iter().chain(&bars) {
        damage = Some(damage.map_or(bar.track, |d| d.union(&bar.track)));
      }
    }
    ui.painted_bars.clone_from(&bars);

    // The input method is on while the focused element takes text, and is
    // told where the caret is, for its candidate window.
    let wants_ime = ui.wants_ime();
    if wants_ime != ui.ime_allowed {
      entry.window.set_ime_allowed(wants_ime);
      ui.ime_allowed = wants_ime;
      ui.ime_area = None;
    }
    if wants_ime {
      let area = ui
        .focus
        .as_ref()
        .and_then(|f| ui.layout.caret_rect(f, &font_measure()));
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
        boxes: ui.layout.boxes(),
        region,
        scale: info.scale_factor as f32,
        background: ui.style.background,
        focus_ring: ring,
        caret_visible,
        colors: ui.style,
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
        self.shell.dev.ready();
        self.app.frame_presented(
          &mut Self::context(&mut self.shell, event_loop),
          WindowId(id),
        )
      },
      FrameOutcome::Skipped => {},
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
          ui.carry_out(request);
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
    self.shell.serve_host(event_loop);
    // Components whose close guard agreed close their windows.
    for window in self.shell.guarded_to_close() {
      Self::context(&mut self.shell, event_loop).close_window(window);
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
      .flat_map(|ui| [ui.blink_at, ui.bars_hide_at])
      .flatten()
      .chain(self.shell.wake_at.get())
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
    let now = Instant::now();
    // Carets whose time has come turn on or off.
    if let Some(rate) = blink_rate() {
      for entry in self.shell.windows.values_mut() {
        if let Some(ui) = entry.ui.as_mut()
          && ui.blink_at.is_some_and(|at| at <= now)
        {
          ui.caret_on = !ui.caret_on;
          ui.blink_at = Some(now + rate);
          entry.window.request_redraw();
        }
      }
    }
    // Scrollbars that show only while scrolling hide (M15).
    for entry in self.shell.windows.values_mut() {
      if let Some(ui) = entry.ui.as_mut()
        && ui.bars_hide_at.is_some_and(|at| at <= now)
      {
        ui.bars_hide_at = None;
        entry.window.request_redraw();
      }
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
    self
      .app
      .started(&mut Self::context(&mut self.shell, event_loop));
    if self.shell.windows.is_empty() {
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
    match event {
      WindowEvent::RedrawRequested => self.draw(event_loop, id),
      WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
        self.resized(event_loop, id);
        self.save_placement(id);
        self.tell_place(id);
      },
      WindowEvent::Moved(_) => {
        self.save_placement(id);
        self.tell_place(id);
      },
      // A frame asked for while the window was hidden or minimised is
      // skipped, and nothing asks again. So draw once it can be seen.
      WindowEvent::Occluded(false) => {
        if let Some(entry) = self.shell.windows.get(&id) {
          entry.window.request_redraw();
        }
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
      WindowEvent::ThemeChanged(_) => lifecycle::refresh_appearance(),
      WindowEvent::Focused(active) => {
        // Settings may have changed while another window was in front.
        if active {
          lifecycle::refresh_appearance();
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
          ui.active = active;
          ui.wake_caret();
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
          if ui.wheel(dx, dy, steps, modifiers) {
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
          && ui.ime(event)
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
            let used = ui.key(&key);
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
        if let Some(ui) = self
          .shell
          .windows
          .get_mut(&id)
          .and_then(|e| e.ui.as_mut())
        {
          ui.cursor = (f32::NAN, f32::NAN);
          ui.update_hover();
          if ui.bar_drag.is_none() && ui.bar_hot.take().is_some() {
            self.shell.windows[&id].window.request_redraw();
          }
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
  let state_changed = Rc::new(Cell::new(false));
  let flag = Rc::clone(&state_changed);
  dovetail_core::set_on_change(Some(Box::new(move || flag.set(true))));
  // Controls copy and paste through the system clipboard while it runs.
  dovetail_core::set_clipboard(crate::platform::clipboard());
  // Menu bars use the system's own where it has one (macOS).
  dovetail_core::set_native_menus(crate::platform::native_menus());
  // Open and Save As use the system's dialogs, owned by the window in
  // front.
  let dialog_owner = Rc::new(Cell::new(0));
  dovetail_core::set_file_dialogs(crate::platform::file_dialogs(Rc::clone(
    &dialog_owner,
  )));
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
    proxy: event_loop.create_proxy(),
    dev: DevState::from_env(),
    drag: None,
    host,
    dialog_owner,
  };
  // Themes follow the system's light or dark mode, accent colour and
  // contrast (M14).
  lifecycle::refresh_appearance();
  // Tasks wake the event loop while it runs.
  let _tasks = lifecycle::TaskWaker::install(event_loop.create_proxy());
  let mut runner = Runner { app, shell, started: false };
  let result = event_loop
    .run_app(&mut runner)
    .map_err(Error::EventLoop);
  dovetail_core::set_on_change(None);
  dovetail_core::set_clipboard(None);
  dovetail_core::set_native_menus(None);
  dovetail_core::set_window_host(None);
  // Component trees, canvases and surfaces must go before the GPU.
  runner.shell.windows.clear();
  runner.shell.painter = None;
  result
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
