//! Applications whose windows are all components (M19, D0029).
//!
//! The root is a component tree with no window of its own. The
//! [`Window`](dovetail_core::Window)s it shows are the application's
//! windows, so with [`ExitPolicy::KeepRunning`] it lives on while none is
//! open, as macOS applications do.

use dovetail_core::View;
use dovetail_session::{Session, Settings};

use super::{
  Application, Context, Error, ExitPolicy, Frame, UserEvent, WindowId,
};

/// The size the root's layout is given. It shows no boxes of its own.
pub(super) const ROOT_SIZE: (f32, f32) = (1.0, 1.0);

/// Runs an application whose windows are the [`Window`](dovetail_core::Window)
/// components `root` shows, until it ends: after its last window closes on
/// Windows and Linux, and when it quits on macOS, where it keeps running
/// with no windows open and a click on its Dock icon builds `root` anew.
/// Change this with [`Context::set_exit_policy`] from a component, through
/// [`Application`], or keep it with [`ExitPolicy::default`].
///
/// # Errors
///
/// When the system has no event loop or windows to give, or the GPU cannot
/// be set up.
pub fn run_windows(root: View) -> Result<(), Error> {
  super::run(Root { root, policy: ExitPolicy::default() })
}

/// Runs `root` as [`run_windows`] does, ending as `policy` says.
///
/// # Errors
///
/// As [`run_windows`].
pub fn run_windows_with(
  root: View,
  policy: ExitPolicy,
) -> Result<(), Error> {
  super::run(Root { root, policy })
}

struct Root {
  root: View,
  policy: ExitPolicy,
}

impl Application for Root {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    cx.set_exit_policy(self.policy);
    cx.set_root(self.root.clone());
    // A click on the Dock icon with no window open (macOS).
    let proxy = cx.shell.proxy.clone();
    crate::platform::on_reopen(Box::new(move || {
      let _ = proxy.send_event(UserEvent::Reopen);
    }));
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: WindowId,
    _: &mut Frame,
  ) {
  }

  fn reopen(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    if cx.window_count() == 0 {
      cx.set_root(self.root.clone());
    }
  }
}

impl Context<'_> {
  /// Makes `root` the application's root: a component tree with no window
  /// of its own, whose [`Window`](dovetail_core::Window)s are opened now
  /// (M19). It replaces the root there was, which closes that root's
  /// windows, and [`View::Empty`] takes the root away. For
  /// [`run_windows`], and for tests that run a whole application.
  pub fn set_root(
    &mut self,
    root: View,
  ) {
    // No root: the one there was goes, with its windows.
    if matches!(root, View::Empty) {
      self.shell.root = None;
      self.shell.serve_host(self.event_loop);
      return;
    }
    let settings = Settings::from(dovetail_core::system_settings());
    let mut session = Session::new(
      "",
      root,
      None,
      settings,
      Box::new(dovetail_render::font_measure()),
    );
    session.update(Some(ROOT_SIZE));
    self.shell.root = Some(session);
    self.shell.serve_host(self.event_loop);
  }

  /// Whether the application ends after its last window closes (M19).
  pub fn set_exit_policy(
    &mut self,
    policy: ExitPolicy,
  ) {
    self.shell.exit_policy = policy;
  }

  /// Whether the application ends after its last window closes.
  pub fn exit_policy(&self) -> ExitPolicy {
    self.shell.exit_policy
  }
}
