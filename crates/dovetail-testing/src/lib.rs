//! Tests for Dovetail UI applications (M20, decisions D0026 and D0027).
//!
//! A [`TestUi`] runs a component as a window would, with no window or GPU:
//! it builds and lays out, keeps focus, and answers the pointer and the
//! keyboard through the same code as a real window (`dovetail-session`).
//! A test finds elements by what they are, as screen readers do, acts on
//! them, and checks what changed:
//!
//! ```
//! use dovetail::prelude::*;
//! use dovetail_core::Role;
//! use dovetail_testing::{TestUi, role, text};
//!
//! #[component]
//! fn Counter() -> View {
//!   let count = use_state(|| 0);
//!   ui! {
//!     Column {
//!       Text { value: count.map(|n| format!("Count: {n}")) }
//!       Button { text: "Increment", on_click: move |_| count.update(|n| *n += 1) }
//!     }
//!   }
//! }
//!
//! let mut ui = TestUi::new(ui! { Counter {} });
//! ui.click(role(Role::Button).label("Increment"));
//! ui.find(text("Count: 1"));
//! ```
//!
//! What a test does never reaches the system: keys and clicks go to the
//! session, the clipboard is kept in memory, and time stands still until
//! the test moves it with [`TestUi::advance`].

use std::rc::Rc;
use std::time::{Duration, Instant};

use dovetail_core::{
  BuildContext, Component, DragEnd, KeyCode, KeyEvent, Modifiers, NamedKey,
  PointerButton, SystemSettings, TestClock, View,
};
use dovetail_layout::{FixedPitch, Layout, Measure};
use dovetail_session::{AccessTree, Drag, Request, Session, Settings};

mod find;
#[cfg(feature = "pictures")]
mod pictures;
#[cfg(feature = "windows")]
pub mod windows;

pub use find::{Find, FindError, Found, label, role, text};
#[cfg(feature = "pictures")]
pub use pictures::{ODD_PIXELS_PER_THOUSAND, Picture, TOLERANCE};

/// How long [`TestUi::run_tasks`] and [`TestUi::wait_until`] wait, in real
/// time, before they fail.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// How a [`TestUi`] is set up.
pub struct Options {
  /// The window's size, in logical pixels.
  pub size: (f32, f32),
  /// The display scale: physical pixels for each logical pixel. Only
  /// pictures use it.
  pub scale: f32,
  /// The window's title, which names it for assistive tools.
  pub title: String,
  /// What text is measured with. [`FixedPitch`], the default, gives the
  /// same layout on every system; use the system's fonts to test what
  /// people see.
  pub measure: Box<dyn Measure>,
  /// The system's settings the test starts with: less motion, the text
  /// scale, the caret blink, double-clicks and scroll bars (M18). A test
  /// changes them with [`dovetail_core::set_system_settings`], and the
  /// window follows at its next update, as a real one does.
  pub settings: SystemSettings,
}

impl Default for Options {
  /// 800 by 600 at scale 1, titled "test", measured with [`FixedPitch`].
  fn default() -> Self {
    Self {
      size: (800.0, 600.0),
      scale: 1.0,
      title: "test".into(),
      measure: Box::new(FixedPitch::default()),
      settings: SystemSettings::default(),
    }
  }
}

/// Shows a fixed view, so any view can be the root.
struct Show;

#[derive(PartialEq)]
struct ShowProps {
  view: View,
}

impl Component for Show {
  type Props = ShowProps;

  fn build(
    props: &ShowProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    props.view.clone()
  }
}

/// The usual shortcut modifier on this system: Command on macOS, Ctrl
/// elsewhere.
pub fn command() -> Modifiers {
  if cfg!(target_os = "macos") {
    Modifiers { meta: true, ..Modifiers::default() }
  } else {
    Modifiers { control: true, ..Modifiers::default() }
  }
}

/// A component running as a window would, with no window or GPU.
pub struct TestUi {
  session: Session,
  size: (f32, f32),
  scale: f32,
  clock: Rc<TestClock>,
  #[cfg(feature = "pictures")]
  painting: pictures::Cache,
  /// A drag in progress, which a pointer handler started.
  drag: Option<Drag<()>>,
  /// How the last drag ended.
  drag_end: Option<DragEnd>,
  /// When the last frame for animation started, or when a component
  /// first asked for one; `None` while none is asked for.
  last_frame: Option<Instant>,
}

/// How long a frame lasts in [`TestUi::advance`]: 60 frames a second of
/// the test's time.
pub const FRAME: Duration = Duration::from_nanos(16_666_667);

impl TestUi {
  /// Runs `root` in a window 800 by 600 logical pixels, and lays it out.
  pub fn new(root: View) -> Self {
    Self::with(root, Options::default())
  }

  /// Runs `root` as `options` say, and lays it out.
  ///
  /// This installs, for the test's thread, a clipboard in memory, a clock
  /// that stands still until [`TestUi::advance`] moves it, and
  /// `options.settings` as the system's settings.
  pub fn with(
    root: View,
    options: Options,
  ) -> Self {
    dovetail_core::set_clipboard(None);
    let clock = TestClock::install();
    dovetail_core::set_system_settings(options.settings);
    let session = Session::new(
      options.title,
      View::component::<Show>(ShowProps { view: root }),
      None,
      Settings::from(dovetail_core::system_settings()),
      options.measure,
    );
    let mut ui = Self {
      session,
      size: options.size,
      scale: options.scale,
      clock,
      #[cfg(feature = "pictures")]
      painting: pictures::Cache::default(),
      drag: None,
      drag_end: None,
      last_frame: None,
    };
    ui.update();
    ui
  }

  /// Builds what changed and lays it out again. Every action does this
  /// before it returns, so tests call it only after changing state
  /// themselves.
  pub fn update(&mut self) {
    // The window follows the system's settings, as a real one does when
    // they change.
    self
      .session
      .set_settings(Settings::from(dovetail_core::system_settings()));
    // What layout reports can change the tree again; a few rounds settle
    // it, as a window does over a few frames.
    for _ in 0..3 {
      self.session.update(Some(self.size));
      if !self.session.needs_update() {
        break;
      }
    }
    // Frames for animation are counted from the last, or from when a
    // component first asked.
    let now = dovetail_core::now();
    if !dovetail_core::frame_requested() {
      self.last_frame = None;
    } else if self.last_frame.is_none_or(|at| at + FRAME < now) {
      self.last_frame = Some(now);
    }
  }

  /// Gives the window a new size, in logical pixels, and lays it out.
  pub fn resize(
    &mut self,
    width: f32,
    height: f32,
  ) {
    self.size = (width, height);
    self.update();
  }

  /// The display scale: physical pixels for each logical pixel.
  pub fn scale(&self) -> f32 {
    self.scale
  }

  /// The session underneath, for what these functions do not cover.
  pub fn session(&mut self) -> &mut Session {
    &mut self.session
  }

  /// The layout, as last updated.
  pub fn layout(&self) -> &Layout {
    self.session.layout()
  }

  // Finding.

  /// The accessibility tree, as last laid out.
  pub fn access_tree(&self) -> AccessTree {
    self.session.access_tree()
  }

  /// The accessibility tree as text, one element a line, indented under
  /// its parent: for messages, and to see what a test can find.
  pub fn describe(&self) -> String {
    find::dump(&self.access_tree())
  }

  /// Every element that matches `search`, in the tree's order.
  pub fn find_all(
    &self,
    search: Find,
  ) -> Vec<Found> {
    find::search(&self.access_tree(), None, &search)
  }

  /// The one element that matches `search`, or why there is not exactly
  /// one.
  pub fn try_find(
    &self,
    search: Find,
  ) -> Result<Found, FindError> {
    find::one(&self.access_tree(), None, &search)
  }

  /// The one element that matches `search`.
  ///
  /// # Panics
  ///
  /// When nothing or more than one matches. The message names the search,
  /// and lists the matches or what the window has.
  #[track_caller]
  pub fn find(
    &self,
    search: Find,
  ) -> Found {
    self
      .try_find(search)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// The one element inside `within` that matches `search`.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn find_within(
    &self,
    within: &Found,
    search: Find,
  ) -> Found {
    find::one(&self.access_tree(), Some(within.node().id), &search)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// True when anything matches `search`.
  pub fn has(
    &self,
    search: Find,
  ) -> bool {
    !self.find_all(search).is_empty()
  }

  /// The element with keyboard focus, if one has it.
  pub fn focused(&self) -> Option<Found> {
    find::focused(&self.access_tree())
  }

  // The pointer.

  /// Moves the pointer to the middle of the element `search` finds, and
  /// clicks it with the main button.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn click(
    &mut self,
    search: Find,
  ) {
    let at = self.find(search).centre();
    self.click_at(at);
  }

  /// Clicks at a point in the window, in logical pixels.
  pub fn click_at(
    &mut self,
    at: (f32, f32),
  ) {
    self.press_at(at, PointerButton::Primary, Modifiers::default());
    self.release(PointerButton::Primary, Modifiers::default());
  }

  /// Clicks the element `search` finds twice, quickly, in the same place.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn double_click(
    &mut self,
    search: Find,
  ) {
    let at = self.find(search).centre();
    self.click_at(at);
    self.click_at(at);
  }

  /// Clicks the element `search` finds with the second button, which
  /// opens a context menu.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn right_click(
    &mut self,
    search: Find,
  ) {
    let at = self.find(search).centre();
    self.press_at(at, PointerButton::Secondary, Modifiers::default());
    self.release(PointerButton::Secondary, Modifiers::default());
  }

  /// Moves the pointer onto the element `search` finds, with no button
  /// held.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn hover(
    &mut self,
    search: Find,
  ) {
    let at = self.find(search).centre();
    self.move_to(at, Modifiers::default());
  }

  /// Moves the pointer to a point in the window, in logical pixels.
  pub fn move_to(
    &mut self,
    at: (f32, f32),
    modifiers: Modifiers,
  ) {
    self.session.set_cursor(at);
    if self.drag.is_some() {
      self.drag_moved(true);
    } else {
      self.session.pointer_moved(modifiers);
      self.pick_up_drag();
    }
    self.update();
  }

  /// Takes the pointer out of the window.
  pub fn leave(&mut self) {
    if self.drag.is_some() {
      // A drag follows the pointer outside the window.
      self.drag_moved(false);
    } else {
      self.session.pointer_left();
    }
    self.update();
  }

  /// Presses `button` at a point in the window, in logical pixels.
  pub fn press_at(
    &mut self,
    at: (f32, f32),
    button: PointerButton,
    modifiers: Modifiers,
  ) {
    self.session.set_cursor(at);
    if self.drag.is_none() {
      self.session.pointer_moved(modifiers);
    }
    self.session.press(modifiers, button);
    self.pick_up_drag();
    self.update();
  }

  /// Lets go of `button` where the pointer is.
  pub fn release(
    &mut self,
    button: PointerButton,
    modifiers: Modifiers,
  ) {
    if button == PointerButton::Primary
      && let Some(drag) = self.drag.take()
    {
      self.drag_end = Some(drag.release(|()| Some(&self.session)));
    }
    self.session.release(modifiers, button);
    self.update();
  }

  /// True while something is being dragged.
  pub fn dragging(&self) -> bool {
    self.drag.is_some()
  }

  /// How the last drag ended, as its source was told.
  pub fn drag_end(&self) -> Option<DragEnd> {
    self.drag_end
  }

  /// Picks up a drag that a pointer handler started, if one did. Its press
  /// then does not click.
  fn pick_up_drag(&mut self) {
    let Some(drag) = Drag::take_started() else {
      return;
    };
    if let Some(old) = self.drag.take() {
      old.cancel(|()| Some(&self.session));
    }
    self.session.cancel_press();
    self.drag = Some(drag);
    self.drag_end = None;
    self.drag_moved(true);
  }

  /// Tells the drag where the pointer is: in the window, or outside it.
  fn drag_moved(
    &mut self,
    inside: bool,
  ) {
    let Some(mut drag) = self.drag.take() else {
      return;
    };
    let at = inside.then(|| ((), &self.session, self.session.cursor()));
    drag.moved(at, None, |()| Some(&self.session));
    self.drag = Some(drag);
  }

  /// Presses the main button at `from`, moves to `to` in a few steps, and
  /// lets go there: a slider's thumb or a splitter follows, and a drag and
  /// drop that a handler starts ends with a drop at `to`
  /// ([`TestUi::drag_end`] says how).
  pub fn drag(
    &mut self,
    from: (f32, f32),
    to: (f32, f32),
  ) {
    let none = Modifiers::default();
    self.press_at(from, PointerButton::Primary, none);
    const STEPS: u8 = 4;
    for i in 1..=STEPS {
      let t = f32::from(i) / f32::from(STEPS);
      let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
      self.move_to(at, none);
    }
    self.release(PointerButton::Primary, none);
  }

  /// Turns the wheel over the element `search` finds, by (`dx`, `dy`)
  /// logical pixels; positive shows more of what is right and below. True
  /// when something used it.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn scroll(
    &mut self,
    search: Find,
    dx: f32,
    dy: f32,
  ) -> bool {
    let at = self.find(search).centre();
    self.session.set_cursor(at);
    let used = self
      .session
      .wheel(dx, dy, false, Modifiers::default());
    self.update();
    used
  }

  // The keyboard.

  /// Gives keyboard focus to the element `search` finds, as an assistive
  /// tool can.
  ///
  /// # Panics
  ///
  /// As [`TestUi::find`].
  #[track_caller]
  pub fn focus(
    &mut self,
    search: Find,
  ) {
    let found = self.find(search);
    if let Some(key) = found.key() {
      self
        .session
        .carry_out(Request::Focus(key.clone()));
    }
    self.update();
  }

  /// Presses a key, as the keyboard would. True when the window used it.
  pub fn press(
    &mut self,
    event: KeyEvent,
  ) -> bool {
    // Escape gives up a drag.
    if event.is(NamedKey::Escape)
      && let Some(drag) = self.drag.take()
    {
      drag.cancel(|()| Some(&self.session));
      self.drag_end = Some(DragEnd::Cancelled);
      self.update();
      return true;
    }
    let used = self.session.key(&event);
    self.update();
    used
  }

  /// Presses a named key, such as Tab or Enter, with no modifiers.
  pub fn key(
    &mut self,
    key: NamedKey,
  ) -> bool {
    self.press(KeyEvent::named(key))
  }

  /// Types `text` one character at a time; `\n` presses Enter and `\t`
  /// presses Tab.
  pub fn type_text(
    &mut self,
    text: &str,
  ) {
    for c in text.chars() {
      match c {
        '\n' => self.key(NamedKey::Enter),
        '\t' => self.key(NamedKey::Tab),
        c => self.press(KeyEvent::character(c)),
      };
    }
  }

  /// Presses the usual shortcut modifier with `key`: Command+`key` on
  /// macOS, Ctrl+`key` elsewhere, as Ctrl+S saves.
  pub fn shortcut(
    &mut self,
    key: char,
  ) -> bool {
    self.press(KeyEvent {
      key: KeyCode::Character(key.to_string()),
      modifiers: command(),
      text: None,
      repeat: false,
    })
  }

  /// Puts `text` on the clipboard and pastes it with the shortcut.
  pub fn paste(
    &mut self,
    text: &str,
  ) -> bool {
    dovetail_core::set_clipboard_text(text);
    self.shortcut('v')
  }

  /// What is on the clipboard, as a copy or cut left it.
  pub fn clipboard(&self) -> Option<String> {
    dovetail_core::clipboard_text()
  }

  // Time.

  /// The time on the test's clock.
  pub fn now(&self) -> Instant {
    dovetail_core::now()
  }

  /// Moves the clock forward by `time`. On the way, in order:
  ///
  /// - timers fire as they fall due, as a tooltip's wait;
  /// - while a component asks for frames, a frame every [`FRAME`] builds
  ///   it again, as a window at 60 frames a second does;
  /// - the window's own changes happen: the caret blinks, and overlay
  ///   scroll bars hide.
  ///
  /// Then the window lays out again.
  pub fn advance(
    &mut self,
    time: Duration,
  ) {
    let target = dovetail_core::now() + time;
    // Each frame and deadline on the way, in order. Each sets the next one
    // later, so this ends; the limit is a guard.
    for _ in 0..1_000_000 {
      let now = dovetail_core::now();
      let frame = dovetail_core::frame_requested()
        .then(|| self.last_frame.unwrap_or(now) + FRAME);
      let deadline = self.session.next_deadline();
      let Some(at) = frame
        .into_iter()
        .chain(deadline)
        .min()
        .filter(|at| *at <= target)
      else {
        break;
      };
      self
        .clock
        .advance(at.saturating_duration_since(now));
      let now = dovetail_core::now();
      if frame == Some(at) {
        self.last_frame = Some(now);
        dovetail_core::start_frame();
      }
      if deadline.is_some_and(|d| d <= now) {
        self.session.tick(now);
      }
      self.update();
    }
    self
      .clock
      .advance(target.saturating_duration_since(dovetail_core::now()));
    self.update();
  }

  /// Waits, in real time, for the tasks running on other threads to end,
  /// and gives their results to their callbacks. Timers wait for
  /// [`TestUi::advance`] instead.
  ///
  /// # Panics
  ///
  /// When they have not ended after [`PATIENCE`].
  #[track_caller]
  pub fn run_tasks(&mut self) {
    let start = Instant::now();
    loop {
      dovetail_core::run_task_messages();
      self.update();
      let left =
        dovetail_core::tasks_running() - dovetail_core::timers_waiting();
      if left == 0 {
        return;
      }
      assert!(
        start.elapsed() < PATIENCE,
        "{left} tasks still running after {PATIENCE:?}"
      );
      std::thread::sleep(Duration::from_millis(5));
    }
  }

  /// Waits, in real time, until `done` is true, running tasks' callbacks
  /// meanwhile. For work on other threads; move the clock with
  /// [`TestUi::advance`] for timers.
  ///
  /// # Panics
  ///
  /// When `done` is still false after [`PATIENCE`]. The message names
  /// `what`, and shows what the window has.
  #[track_caller]
  pub fn wait_until(
    &mut self,
    what: &str,
    mut done: impl FnMut(&TestUi) -> bool,
  ) {
    let start = Instant::now();
    loop {
      if done(self) {
        return;
      }
      if start.elapsed() > PATIENCE {
        panic!(
          "timed out after {PATIENCE:?} waiting for {what}. The window \
           has:\n{}",
          self.describe()
        );
      }
      std::thread::sleep(Duration::from_millis(5));
      dovetail_core::run_task_messages();
      self.update();
    }
  }
}

impl Drop for TestUi {
  fn drop(&mut self) {
    // The next test on this thread starts with the real time, and the
    // usual settings.
    dovetail_core::set_clock(None);
    dovetail_core::set_system_settings(SystemSettings::default());
  }
}
