//! Tests that use real windows (M20, decision D0026).
//!
//! Most tests need no window: use [`TestUi`](crate::TestUi). Use these for
//! what a real window adds: the system's window, the GPU drawing frames,
//! and the desktop's own handling of input.
//!
//! winit allows one event loop for each process, and on macOS it must run
//! on the main thread, which `#[test]` functions do not. So window tests
//! go in a test program of their own, with `harness = false`, whose `main`
//! calls [`run_window_tests`]:
//!
//! ```toml
//! [[test]]
//! name = "windows"
//! harness = false
//! ```
//!
//! ```no_run
//! use dovetail_core::Role;
//! use dovetail_testing::windows::{TestWindows, run_window_tests};
//! use dovetail_testing::role;
//! # fn app() -> dovetail_core::View { unimplemented!() }
//!
//! fn opens(t: &mut TestWindows) {
//!   let w = t.open("App", (400.0, 300.0), app());
//!   t.click(w, role(Role::Button).label("OK"));
//! }
//!
//! fn main() -> std::process::ExitCode {
//!   run_window_tests(&[("opens", opens)])
//! }
//! ```
//!
//! They open windows, so they run only with `--ignored`:
//! `cargo test --test windows -- --ignored`. A word after `--` runs only
//! the tests whose names hold it.
//!
//! Input goes to the window itself, never through the system, so a test
//! cannot type into another program. Each window is brought to the front
//! when it opens. Time is real.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use dovetail_core::{
  KeyCode, KeyEvent, Modifiers, NamedKey, PointerButton, PointerKind, View,
  WindowPlace, WindowState,
};
use dovetail_desktop::{
  Context, LogicalSize, Pumped, WindowId, WindowOptions, WindowStats,
};
use dovetail_session::AccessTree;

use crate::{Find, FindError, Found, PATIENCE, Show, ShowProps, command, find};

/// A window a test opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestWindow(WindowId);

/// Real windows, driven by a test. One serves every test in a program.
pub struct TestWindows {
  pumped: Pumped,
}

/// A window test: it gets the windows, and panics to fail.
pub type WindowTest = fn(&mut TestWindows);

impl TestWindows {
  /// Makes the event loop.
  ///
  /// # Panics
  ///
  /// When the system has no windows to give, or one was made already in
  /// this process.
  pub fn new() -> Self {
    let pumped = Pumped::new()
      .unwrap_or_else(|e| panic!("could not start the windows: {e}"));
    Self { pumped }
  }

  /// Runs `work` with the desktop's [`Context`] on the next turn of the
  /// loop, for what these functions do not cover.
  ///
  /// # Panics
  ///
  /// When the loop has ended.
  pub fn with<R: 'static>(
    &mut self,
    work: impl FnOnce(&mut Context<'_>) -> R + 'static,
  ) -> R {
    self
      .pumped
      .with(work)
      .unwrap_or_else(|e| panic!("the windows stopped: {e}"))
  }

  /// Opens a window titled `title`, `size` logical pixels inside, showing
  /// `root`. It is brought to the front, and this waits for its first
  /// frame.
  ///
  /// # Panics
  ///
  /// When it cannot open, or draws no frame within [`PATIENCE`].
  #[track_caller]
  pub fn open(
    &mut self,
    title: &str,
    size: (f32, f32),
    root: View,
  ) -> TestWindow {
    let options = WindowOptions::new(title)
      .with_size(LogicalSize::new(f64::from(size.0), f64::from(size.1)));
    self.open_with(options, root)
  }

  /// Opens a window with `options`, such as size limits or a state (M19),
  /// showing `root`, as [`TestWindows::open`] does.
  ///
  /// # Panics
  ///
  /// When it cannot open, or draws no frame within [`PATIENCE`].
  #[track_caller]
  pub fn open_with(
    &mut self,
    options: WindowOptions,
    root: View,
  ) -> TestWindow {
    let root = View::component::<Show>(ShowProps { view: root });
    let id = self
      .with(move |cx| {
        let id = cx.open_ui_window(options, root)?;
        // A precaution: one macOS run did not draw windows opened after
        // the last one closed (D0026).
        cx.focus_window(id);
        Ok::<_, dovetail_desktop::Error>(id)
      })
      .unwrap_or_else(|e| panic!("could not open a window: {e}"));
    let w = TestWindow(id);
    self.wait_until("the window's first frame", move |t| {
      t.stats(w).presented > 0
    });
    w
  }

  /// The state a window says it is in (M19).
  ///
  /// # Panics
  ///
  /// When the window is not open.
  #[track_caller]
  pub fn state(
    &mut self,
    w: TestWindow,
  ) -> WindowState {
    self
      .with(move |cx| cx.window_state(w.0))
      .expect("the window is open")
  }

  /// Asks for a window to be put in `state`, and waits until it says it
  /// is (M19).
  ///
  /// # Panics
  ///
  /// When it is not in that state within [`PATIENCE`].
  #[track_caller]
  pub fn set_state(
    &mut self,
    w: TestWindow,
    state: WindowState,
  ) {
    self.with(move |cx| cx.set_window_state(w.0, state));
    self.wait_until(&format!("the window to be {state:?}"), move |t| {
      t.state(w) == state
    });
  }

  /// Where a window is, and the size of its drawing area, in logical
  /// pixels (M19).
  ///
  /// # Panics
  ///
  /// When the window is not open.
  #[track_caller]
  pub fn place(
    &mut self,
    w: TestWindow,
  ) -> WindowPlace {
    self
      .with(move |cx| cx.window_place(w.0))
      .expect("the window is open")
  }

  /// Asks for a window's drawing area to be `size` logical pixels. The
  /// system may refuse or change it: read [`TestWindows::place`].
  pub fn set_size(
    &mut self,
    w: TestWindow,
    size: (f32, f32),
  ) {
    self.with(move |cx| {
      cx.set_window_size(
        w.0,
        LogicalSize::new(f64::from(size.0), f64::from(size.1)),
      );
    });
  }

  /// The one open window titled `title`, such as a window a component
  /// opened, waiting for it to open and draw its first frame (M19).
  ///
  /// # Panics
  ///
  /// When none is open with that title within [`PATIENCE`].
  #[track_caller]
  pub fn window_titled(
    &mut self,
    title: &str,
  ) -> TestWindow {
    let title = title.to_string();
    let mut found = None;
    self.wait_until(&format!("a window titled {title:?}"), |t| {
      let wanted = title.clone();
      found = t.with(move |cx| {
        cx.windows()
          .into_iter()
          .find(|w| cx.window_title(*w).as_deref() == Some(wanted.as_str()))
      });
      found.is_some_and(|w| t.stats(TestWindow(w)).presented > 0)
    });
    TestWindow(found.expect("found"))
  }

  /// Makes `root` the application's root, a tree with no window whose
  /// [`Window`](dovetail_core::Window)s open now, as `run_windows` does
  /// (M19). [`View::Empty`] takes the root away, which closes its windows.
  pub fn set_root(
    &mut self,
    root: View,
  ) {
    self.with(move |cx| cx.set_root(root));
  }

  /// Moves a window's top-left corner to `position`, in logical pixels.
  /// Wayland ignores this (M19).
  pub fn move_to(
    &mut self,
    w: TestWindow,
    position: (f32, f32),
  ) {
    self.with(move |cx| cx.set_window_position(w.0, position));
  }

  /// Asks a window to close as the user would, with its close button:
  /// its components are asked first (M19).
  pub fn request_close(
    &mut self,
    w: TestWindow,
  ) {
    self.with(move |cx| cx.request_close(w.0));
  }

  /// The modal window that blocks a window, if one does (M19).
  pub fn blocked_by(
    &mut self,
    w: TestWindow,
  ) -> Option<TestWindow> {
    self.with(move |cx| cx.blocked_by(w.0).map(TestWindow))
  }

  /// True while a window is open.
  pub fn is_open(
    &mut self,
    w: TestWindow,
  ) -> bool {
    self.with(move |cx| cx.windows().contains(&w.0))
  }

  /// Closes a window.
  pub fn close(
    &mut self,
    w: TestWindow,
  ) {
    self.with(move |cx| cx.close_window(w.0));
  }

  /// The windows open now, including those components opened.
  pub fn windows(&mut self) -> Vec<TestWindow> {
    self.with(|cx| cx.windows().into_iter().map(TestWindow).collect())
  }

  /// Closes every window, so the next test starts with none.
  pub fn close_all(&mut self) {
    for w in self.windows() {
      self.close(w);
    }
  }

  /// What a window has received from the system and the GPU.
  pub fn stats(
    &mut self,
    w: TestWindow,
  ) -> WindowStats {
    self.with(move |cx| cx.window_stats(w.0).unwrap_or_default())
  }

  // Finding.

  /// A window's accessibility tree, as last laid out.
  ///
  /// # Panics
  ///
  /// When the window is not open.
  #[track_caller]
  pub fn access_tree(
    &mut self,
    w: TestWindow,
  ) -> AccessTree {
    self
      .with(move |cx| cx.ui_session(w.0).map(|s| s.access_tree()))
      .unwrap_or_else(|| panic!("the window {w:?} is not open"))
  }

  /// A window's accessibility tree as text, one element a line.
  pub fn describe(
    &mut self,
    w: TestWindow,
  ) -> String {
    find::dump(&self.access_tree(w))
  }

  /// Every element in the window that matches `search`.
  pub fn find_all(
    &mut self,
    w: TestWindow,
    search: Find,
  ) -> Vec<Found> {
    find::search(&self.access_tree(w), None, &search)
  }

  /// The one element in the window that matches `search`, or why there
  /// is not exactly one.
  pub fn try_find(
    &mut self,
    w: TestWindow,
    search: Find,
  ) -> Result<Found, FindError> {
    find::one(&self.access_tree(w), None, &search)
  }

  /// The one element in the window that matches `search`.
  ///
  /// # Panics
  ///
  /// When nothing or more than one matches, as
  /// [`TestUi::find`](crate::TestUi::find) does.
  #[track_caller]
  pub fn find(
    &mut self,
    w: TestWindow,
    search: Find,
  ) -> Found {
    self
      .try_find(w, search)
      .unwrap_or_else(|e| panic!("{e}"))
  }

  /// True when anything in the window matches `search`.
  pub fn has(
    &mut self,
    w: TestWindow,
    search: Find,
  ) -> bool {
    !self.find_all(w, search).is_empty()
  }

  /// The element with keyboard focus in the window, if one has it.
  pub fn focused(
    &mut self,
    w: TestWindow,
  ) -> Option<Found> {
    find::focused(&self.access_tree(w))
  }

  // Input. Each waits for the window to draw the result.

  /// Clicks the middle of the element `search` finds.
  ///
  /// # Panics
  ///
  /// As [`TestWindows::find`], or when the window draws no frame.
  #[track_caller]
  pub fn click(
    &mut self,
    w: TestWindow,
    search: Find,
  ) {
    let at = self.find(w, search).centre();
    self.click_at(w, at);
  }

  /// Clicks a point in the window, in logical pixels.
  #[track_caller]
  pub fn click_at(
    &mut self,
    w: TestWindow,
    (x, y): (f32, f32),
  ) {
    self.act(w, move |cx| cx.click(w.0, x, y));
  }

  /// Clicks the element `search` finds twice, quickly.
  #[track_caller]
  pub fn double_click(
    &mut self,
    w: TestWindow,
    search: Find,
  ) {
    let (x, y) = self.find(w, search).centre();
    self.act(w, move |cx| {
      cx.click(w.0, x, y);
      cx.click(w.0, x, y);
    });
  }

  /// Clicks the element `search` finds with the second button.
  #[track_caller]
  pub fn right_click(
    &mut self,
    w: TestWindow,
    search: Find,
  ) {
    let at = self.find(w, search).centre();
    let none = Modifiers::default();
    self.act(w, move |cx| {
      let second = PointerButton::Secondary;
      cx.pointer_button(w.0, PointerKind::Down, at, none, second);
      cx.pointer_button(w.0, PointerKind::Up, at, none, second);
    });
  }

  /// Moves the pointer onto the element `search` finds.
  #[track_caller]
  pub fn hover(
    &mut self,
    w: TestWindow,
    search: Find,
  ) {
    let at = self.find(w, search).centre();
    let none = Modifiers::default();
    self.act(w, move |cx| {
      cx.pointer_button(
        w.0,
        PointerKind::Move,
        at,
        none,
        PointerButton::Primary,
      )
    });
  }

  /// Takes the pointer out of the window.
  #[track_caller]
  pub fn leave(
    &mut self,
    w: TestWindow,
  ) {
    self.act(w, move |cx| cx.pointer_leave(w.0));
  }

  /// Presses the main button at `from`, moves to `to` in a few steps, and
  /// lets go there.
  #[track_caller]
  pub fn drag(
    &mut self,
    w: TestWindow,
    from: (f32, f32),
    to: (f32, f32),
  ) {
    let none = Modifiers::default();
    self.act(w, move |cx| {
      cx.pointer(w.0, PointerKind::Down, from.0, from.1, none);
      for i in 1..=4u8 {
        let t = f32::from(i) / 4.0;
        let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        cx.pointer(w.0, PointerKind::Move, at.0, at.1, none);
      }
      cx.pointer(w.0, PointerKind::Up, to.0, to.1, none);
    });
  }

  /// Turns the wheel over the element `search` finds, by (`dx`, `dy`)
  /// logical pixels. True when something used it.
  #[track_caller]
  pub fn scroll(
    &mut self,
    w: TestWindow,
    search: Find,
    dx: f32,
    dy: f32,
  ) -> bool {
    let at = self.find(w, search).centre();
    self.act(w, move |cx| cx.wheel(w.0, at, (dx, dy)))
  }

  /// Presses a key in the window. True when the window used it.
  #[track_caller]
  pub fn press(
    &mut self,
    w: TestWindow,
    event: KeyEvent,
  ) -> bool {
    self.act(w, move |cx| cx.key(w.0, &event))
  }

  /// Presses a named key, such as Tab or Enter.
  #[track_caller]
  pub fn key(
    &mut self,
    w: TestWindow,
    key: NamedKey,
  ) -> bool {
    self.press(w, KeyEvent::named(key))
  }

  /// Types `text` one character at a time; `\n` presses Enter and `\t`
  /// presses Tab.
  #[track_caller]
  pub fn type_text(
    &mut self,
    w: TestWindow,
    text: &str,
  ) {
    let keys: Vec<KeyEvent> = text
      .chars()
      .map(|c| match c {
        '\n' => KeyEvent::named(NamedKey::Enter),
        '\t' => KeyEvent::named(NamedKey::Tab),
        c => KeyEvent::character(c),
      })
      .collect();
    self.act(w, move |cx| {
      for key in &keys {
        cx.key(w.0, key);
      }
    });
  }

  /// Presses Command+`key` on macOS, Ctrl+`key` elsewhere.
  #[track_caller]
  pub fn shortcut(
    &mut self,
    w: TestWindow,
    key: char,
  ) -> bool {
    self.press(
      w,
      KeyEvent {
        key: KeyCode::Character(key.to_string()),
        modifiers: command(),
        text: None,
        repeat: false,
      },
    )
  }

  /// Puts `text` on the clipboard, which tests keep in memory, and pastes
  /// it with the shortcut.
  #[track_caller]
  pub fn paste(
    &mut self,
    w: TestWindow,
    text: &str,
  ) -> bool {
    dovetail_core::set_clipboard_text(text);
    self.shortcut(w, 'v')
  }

  /// What is on the clipboard.
  pub fn clipboard(&self) -> Option<String> {
    dovetail_core::clipboard_text()
  }

  // Waiting.

  /// Runs `work`, then waits until the window has drawn a frame after it.
  #[track_caller]
  fn act<R: 'static>(
    &mut self,
    w: TestWindow,
    work: impl FnOnce(&mut Context<'_>) -> R + 'static,
  ) -> R {
    let before = self.stats(w).presented;
    let out = self.with(move |cx| {
      let out = work(cx);
      cx.request_redraw(w.0);
      out
    });
    self.wait_until("the window to draw what changed", move |t| {
      t.stats(w).presented > before
    });
    out
  }

  /// Waits, in real time, until `done` is true, letting the loop go round
  /// meanwhile.
  ///
  /// # Panics
  ///
  /// When `done` is still false after [`PATIENCE`]. The message names
  /// `what`, and says what each window has received from the system and
  /// the GPU, so a window that never draws can be traced (D0026).
  #[track_caller]
  pub fn wait_until(
    &mut self,
    what: &str,
    mut done: impl FnMut(&mut TestWindows) -> bool,
  ) {
    let start = Instant::now();
    loop {
      if done(self) {
        return;
      }
      if start.elapsed() > PATIENCE {
        let report = self.report();
        panic!("timed out after {PATIENCE:?} waiting for {what}.\n{report}");
      }
      if let Err(e) = self.pumped.pump() {
        panic!("the windows stopped while waiting for {what}: {e}");
      }
    }
  }

  /// Waits `time`, in real time, letting the loop go round.
  pub fn pause(
    &mut self,
    time: Duration,
  ) {
    let start = Instant::now();
    self.wait_until("a pause", move |_| start.elapsed() >= time);
  }

  /// One line for each window: what it has received.
  fn report(&mut self) -> String {
    let rows = self.with(|cx| {
      cx.windows()
        .into_iter()
        .map(|id| {
          let s = cx.window_stats(id).unwrap_or_default();
          format!(
            "  {:?}: {} redraw requests, {} frames shown, {} skipped, \
             hidden {:?}, focused {:?}",
            cx.window_title(id).unwrap_or_default(),
            s.redraws,
            s.presented,
            s.skipped,
            s.occluded,
            s.focused
          )
        })
        .collect::<Vec<_>>()
    });
    if rows.is_empty() {
      "No windows are open.".into()
    } else {
      format!("The windows have received:\n{}", rows.join("\n"))
    }
  }
}

impl Default for TestWindows {
  fn default() -> Self {
    Self::new()
  }
}

/// Runs window tests in turn on one event loop, as `main` of a test
/// program with `harness = false`, and prints results as `cargo test`
/// does.
///
/// Without `--ignored` (or `--include-ignored`) it runs none, since they
/// open windows. A word that does not start with `-` runs only the tests
/// whose names hold it. A test fails when it panics or leaves a window
/// open; the next test starts with none open either way.
pub fn run_window_tests(tests: &[(&str, WindowTest)]) -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  if !args
    .iter()
    .any(|a| a == "--ignored" || a == "--include-ignored")
  {
    println!(
      "\nrunning 0 tests (these open windows: run them with -- --ignored)\n"
    );
    return ExitCode::SUCCESS;
  }
  let filter: Vec<&String> = args
    .iter()
    .filter(|a| !a.starts_with('-'))
    .collect();
  let chosen: Vec<&(&str, WindowTest)> = tests
    .iter()
    .filter(|(name, _)| {
      filter.is_empty() || filter.iter().any(|f| name.contains(f.as_str()))
    })
    .collect();
  println!("\nrunning {} tests", chosen.len());
  let start = Instant::now();
  let mut windows = TestWindows::new();
  let mut failed = Vec::new();
  for (name, test) in &chosen {
    let begun = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| test(&mut windows)))
      .map_err(|p| {
        p.downcast_ref::<String>()
          .cloned()
          .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
          .unwrap_or_else(|| "panicked".into())
      });
    let left = catch_unwind(AssertUnwindSafe(|| windows.windows().len()))
      .unwrap_or_default();
    let outcome = outcome.and_then(|()| match left {
      0 => Ok(()),
      n => Err(format!("left {n} windows open")),
    });
    // The next test starts with no windows, whatever this one did.
    let _ = catch_unwind(AssertUnwindSafe(|| windows.close_all()));
    match outcome {
      Ok(()) => println!("test {name} ... ok ({:.0?})", begun.elapsed()),
      Err(e) => {
        println!("test {name} ... FAILED");
        failed.push((*name, e));
      },
    }
  }
  if !failed.is_empty() {
    println!("\nfailures:");
    for (name, e) in &failed {
      println!("\n---- {name} ----\n{e}");
    }
  }
  println!(
    "\ntest result: {}. {} passed; {} failed; finished in {:.2}s\n",
    if failed.is_empty() { "ok" } else { "FAILED" },
    chosen.len() - failed.len(),
    failed.len(),
    start.elapsed().as_secs_f64()
  );
  if failed.is_empty() {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}
