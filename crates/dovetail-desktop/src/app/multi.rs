//! Drag and drop between windows, windows opened by components, and focus
//! requests (M10, decision D0016). What a drag does over each window is
//! `dovetail_session::Drag` (D0027); this finds the window under the
//! pointer.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dovetail_core::{
  Callback, HostWindow, WindowHost, WindowPlace, WindowRequest,
};
use dovetail_session::{Drag, Session};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::{Shell, UiWindow, WindowEntry, WindowOptions};
use crate::geometry::LogicalSize;

type WinitId = winit::window::WindowId;

/// A window and a point in it, in logical pixels.
type WindowPoint = (WinitId, (f32, f32));

/// A drag in progress, across the windows.
pub(super) type DragState = Drag<WinitId>;

/// A window opened through the window host.
pub(super) struct Hosted {
  pub(super) id: HostWindow,
  pub(super) on_close: Callback<()>,
  pub(super) on_place: Callback<WindowPlace>,
}

enum HostRequest {
  Open(HostWindow, WindowRequest),
  Close(HostWindow),
  Title(HostWindow, String),
}

/// Requests from components, carried out by the event loop.
#[derive(Default)]
pub(super) struct HostQueue {
  next: Cell<u64>,
  requests: RefCell<Vec<HostRequest>>,
}

/// The window host that the desktop installs.
pub(super) struct DesktopHost(pub(super) Rc<HostQueue>);

impl WindowHost for DesktopHost {
  fn open(
    &self,
    request: WindowRequest,
  ) -> HostWindow {
    let id = HostWindow(self.0.next.get() + 1);
    self.0.next.set(id.0);
    self
      .0
      .requests
      .borrow_mut()
      .push(HostRequest::Open(id, request));
    id
  }

  fn close(
    &self,
    window: HostWindow,
  ) {
    self
      .0
      .requests
      .borrow_mut()
      .push(HostRequest::Close(window));
  }

  fn set_title(
    &self,
    window: HostWindow,
    title: &str,
  ) {
    self
      .0
      .requests
      .borrow_mut()
      .push(HostRequest::Title(window, title.to_string()));
  }
}

/// Where a window is and how big its drawing area is.
pub(super) fn place_of(window: &Window) -> WindowPlace {
  let scale = window.scale_factor();
  let size = window.inner_size();
  WindowPlace {
    position: window
      .outer_position()
      .ok()
      .map(|p| ((p.x as f64 / scale) as f32, (p.y as f64 / scale) as f32)),
    size: (
      (size.width as f64 / scale) as f32,
      (size.height as f64 / scale) as f32,
    ),
  }
}

impl Shell {
  fn session(
    &self,
    id: WinitId,
  ) -> Option<&Session> {
    self
      .windows
      .get(&id)
      .and_then(|e| e.ui.as_ref())
      .map(|ui: &UiWindow| &ui.session)
  }

  /// True while something is being dragged.
  pub(super) fn dragging(&self) -> bool {
    self.drag.is_some()
  }

  /// Picks up a drag that a pointer handler in window `id` started, if
  /// one did. The source element gets no more pointer events, and its
  /// press does not click.
  pub(super) fn pick_up_drag(
    &mut self,
    id: WinitId,
  ) {
    let Some(drag) = DragState::take_started() else {
      return;
    };
    self.cancel_drag();
    if let Some(ui) = self
      .windows
      .get_mut(&id)
      .and_then(|e| e.ui.as_mut())
    {
      ui.session.cancel_press();
    }
    self.drag = Some(drag);
    self.drag_moved(id);
  }

  /// The window under the pointer of window `id` and the point in it, and
  /// the pointer on the screen in physical pixels, when known. Floating
  /// windows are looked at before others, as they are usually in front.
  fn locate(
    &self,
    id: WinitId,
  ) -> (Option<WindowPoint>, Option<(f64, f64)>) {
    let Some(source) = self.windows.get(&id) else {
      return (None, None);
    };
    let Some(ui) = source.ui.as_ref() else {
      return (None, None);
    };
    let (x, y) = ui.session.cursor();
    if x.is_nan() || y.is_nan() {
      return (None, None);
    }
    let scale = source.window.scale_factor();
    let screen = source.window.inner_position().ok().map(|o| {
      (
        f64::from(o.x) + f64::from(x) * scale,
        f64::from(o.y) + f64::from(y) * scale,
      )
    });
    let size = source.window.inner_size();
    let inside = x >= 0.0
      && y >= 0.0
      && f64::from(x) * scale < f64::from(size.width)
      && f64::from(y) * scale < f64::from(size.height);
    if inside {
      return (Some((id, (x, y))), screen);
    }
    let Some((sx, sy)) = screen else {
      return (None, None);
    };
    let mut others: Vec<(&WinitId, &WindowEntry)> = self
      .windows
      .iter()
      .filter(|(w, e)| **w != id && e.ui.is_some())
      .filter(|(_, e)| e.window.is_minimized() != Some(true))
      .collect();
    others.sort_by_key(|(_, e)| e.hosted.is_none());
    for (w, e) in others {
      let Ok(o) = e.window.inner_position() else {
        continue;
      };
      let s = e.window.inner_size();
      let (lx, ly) = (sx - f64::from(o.x), sy - f64::from(o.y));
      if lx >= 0.0
        && ly >= 0.0
        && lx < f64::from(s.width)
        && ly < f64::from(s.height)
      {
        let scale = e.window.scale_factor();
        return (
          Some((*w, ((lx / scale) as f32, (ly / scale) as f32))),
          screen,
        );
      }
    }
    (None, screen)
  }

  /// The pointer of window `id` moved during a drag: tells the drop
  /// target under it, from the nearest outwards until one takes it, and
  /// the one it left.
  pub(super) fn drag_moved(
    &mut self,
    id: WinitId,
  ) {
    let Some(mut drag) = self.drag.take() else {
      return;
    };
    let (found, screen) = self.locate(id);
    let scale = self
      .windows
      .get(&id)
      .map_or(1.0, |e| e.window.scale_factor());
    let screen = screen.map(|(x, y)| ((x / scale) as f32, (y / scale) as f32));
    let at = found.and_then(|(w, point)| Some((w, self.session(w)?, point)));
    drag.moved(at, screen, |w| self.session(w));
    self.drag = Some(drag);
  }

  /// The button was let go during a drag: the target under the pointer
  /// gets the drop, and the source is told how the drag ended. False when
  /// nothing was being dragged.
  pub(super) fn drag_released(
    &mut self,
    id: WinitId,
  ) -> bool {
    if self.drag.is_none() {
      return false;
    }
    self.drag_moved(id);
    let Some(drag) = self.drag.take() else {
      return false;
    };
    drag.release(|w| self.session(w));
    true
  }

  /// Gives up the drag in progress, if there is one.
  pub(super) fn cancel_drag(&mut self) {
    if let Some(drag) = self.drag.take() {
      drag.cancel(|w| self.session(w));
    }
  }

  /// Carries out the requests components made through the window host.
  pub(super) fn serve_host(
    &mut self,
    event_loop: &ActiveEventLoop,
  ) {
    loop {
      let requests = std::mem::take(&mut *self.host.requests.borrow_mut());
      if requests.is_empty() {
        return;
      }
      for request in requests {
        match request {
          HostRequest::Open(id, request) => {
            let mut options =
              WindowOptions::new(request.title).with_size(LogicalSize::new(
                f64::from(request.place.size.0),
                f64::from(request.place.size.1),
              ));
            options.position = request.place.position;
            match self.open_ui(
              event_loop,
              options,
              request.root,
              request.context,
            ) {
              Ok(window) => {
                if let Some(entry) = self.windows.get_mut(&window.0) {
                  entry.hosted = Some(Hosted {
                    id,
                    on_close: request.on_close,
                    on_place: request.on_place,
                  });
                }
              },
              Err(e) => eprintln!("dovetail: could not open a window: {e}"),
            }
          },
          HostRequest::Close(id) => {
            let gone: Vec<WinitId> = self
              .windows
              .iter()
              .filter(|(_, e)| e.hosted.as_ref().is_some_and(|h| h.id == id))
              .map(|(w, _)| *w)
              .collect();
            for w in gone {
              if let Some(drag) = &mut self.drag {
                drag.forget_window(w);
              }
              self.windows.remove(&w);
            }
          },
          HostRequest::Title(id, title) => {
            for e in self.windows.values_mut() {
              if e.hosted.as_ref().is_some_and(|h| h.id == id) {
                e.window.set_title(&title);
                if let Some(ui) = &mut e.ui {
                  ui.session.set_title(title.clone());
                }
              }
            }
          },
        }
      }
    }
  }

  /// Closes the windows opened through the host, as when the first window
  /// closes.
  pub(super) fn close_hosted(&mut self) {
    self.windows.retain(|_, e| e.hosted.is_none());
  }
}
