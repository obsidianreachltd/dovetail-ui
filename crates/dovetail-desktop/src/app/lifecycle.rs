//! Window titles and close guards set by components, and waking the event
//! loop for tasks (M11, decision D0017).

use dovetail_core::{Callback, CloseGuard};
use winit::event_loop::ActiveEventLoop;

use super::{Shell, WindowId};

/// Reads the system's appearance again and gives it to themes, which
/// change if it did.
pub(super) fn refresh_appearance() {
  dovetail_core::set_system_appearance(crate::platform::appearance());
}

impl Shell {
  /// The close guard of a window, if one of its elements has one.
  fn guard_of(
    &self,
    id: winit::window::WindowId,
  ) -> Option<(CloseGuard, Callback<()>)> {
    self
      .windows
      .get(&id)?
      .ui
      .as_ref()?
      .session
      .close_guard()
  }

  /// Gives a request to close a window to its close guard. True when the
  /// window has one, so it stays open for now.
  pub(super) fn ask_guard(
    &self,
    id: winit::window::WindowId,
  ) -> bool {
    match self.guard_of(id) {
      Some((_, on_request)) => {
        on_request.call(());
        true
      },
      None => false,
    }
  }

  /// The windows whose guards asked to close them now.
  pub(super) fn guarded_to_close(&self) -> Vec<WindowId> {
    let wanted = dovetail_core::take_close_requests();
    if wanted.is_empty() {
      return Vec::new();
    }
    let mut ids: Vec<WindowId> = self
      .windows
      .keys()
      .filter(|id| {
        self
          .guard_of(**id)
          .is_some_and(|(g, _)| wanted.contains(&g))
      })
      .map(|id| WindowId(*id))
      .collect();
    // The first window last, as closing it closes the others.
    ids.sort_by_key(|id| self.primary == Some(id.0));
    ids
  }
}

/// The window's handle on Windows, which owns file dialogs; 0 elsewhere.
pub(super) fn native_handle(window: &winit::window::Window) -> isize {
  use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
  match window.window_handle().map(|h| h.as_raw()) {
    Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
    _ => 0,
  }
}

/// Wakes the event loop when a task has a message, for as long as the
/// returned value lives.
pub(super) struct TaskWaker;

impl TaskWaker {
  pub(super) fn install(
    proxy: winit::event_loop::EventLoopProxy<super::UserEvent>
  ) -> TaskWaker {
    dovetail_core::set_task_waker(Some(std::sync::Arc::new(move || {
      let _ = proxy.send_event(super::UserEvent::Tasks);
    })));
    TaskWaker
  }
}

impl Drop for TaskWaker {
  fn drop(&mut self) {
    dovetail_core::set_task_waker(None);
  }
}

/// Runs the tasks' callbacks. Their changes to state redraw the windows.
pub(super) fn run_tasks(_: &ActiveEventLoop) {
  dovetail_core::run_task_messages();
}
