//! A window host with no windows (M19): `TestUi` keeps each window a
//! component opens as a session of its own, so a test can find the
//! windows a tree has open, look inside them, and close them.

use std::cell::{Cell, RefCell};

use dovetail_core::{
  Callback, ComponentId, HostWindow, WindowHost, WindowLimits, WindowPlace,
  WindowRequest, WindowState,
};
use dovetail_layout::FixedPitch;
use dovetail_session::{Session, Settings};

enum Request {
  Open(HostWindow, Box<WindowRequest>),
  Close(HostWindow),
  Title(HostWindow, String),
  State(HostWindow, WindowState),
  Limits(HostWindow, WindowLimits),
  Focus(HostWindow),
}

/// Requests from components, carried out at the next update.
#[derive(Default)]
pub(crate) struct TestHost {
  next: Cell<u64>,
  requests: RefCell<Vec<Request>>,
}

/// The host that `TestUi` installs: it queues requests in a [`TestHost`].
pub(crate) struct Queue(pub(crate) std::rc::Rc<TestHost>);

impl WindowHost for Queue {
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
      .push(Request::Open(id, Box::new(request)));
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
      .push(Request::Close(window));
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
      .push(Request::Title(window, title.to_string()));
  }

  fn set_state(
    &self,
    window: HostWindow,
    state: WindowState,
  ) {
    self
      .0
      .requests
      .borrow_mut()
      .push(Request::State(window, state));
  }

  fn set_limits(
    &self,
    window: HostWindow,
    limits: WindowLimits,
  ) {
    self
      .0
      .requests
      .borrow_mut()
      .push(Request::Limits(window, limits));
  }

  fn focus(
    &self,
    window: HostWindow,
  ) {
    self
      .0
      .requests
      .borrow_mut()
      .push(Request::Focus(window));
  }
}

/// A window a component opened, as a test sees it (M19).
#[derive(Clone, Debug, PartialEq)]
pub struct TestWindowInfo {
  /// Names it for [`TestUi`](crate::TestUi)'s window functions.
  pub id: HostWindow,
  /// Its title.
  pub title: String,
  /// Where it is and how big its drawing area is.
  pub place: WindowPlace,
  /// Its state.
  pub state: WindowState,
  /// Its size limits, and whether it can be resized.
  pub limits: WindowLimits,
  /// Whether it blocks the window that shows its component.
  pub modal: bool,
}

/// One window, with its own session.
pub(crate) struct Hosted {
  pub(crate) info: TestWindowInfo,
  /// `None` while the window is in front, when `TestUi` holds it.
  pub(crate) session: Option<Session>,
  pub(crate) on_close: Callback<()>,
  pub(crate) on_state: Callback<WindowState>,
  pub(crate) on_place: Callback<WindowPlace>,
  /// The component that opened it, which a modal window's owner shows.
  pub(crate) context: Option<ComponentId>,
}

impl TestHost {
  /// Carries out the queued requests, and adds the windows asked to come
  /// to the front to `focus`. Returns whether there were any.
  pub(crate) fn serve(
    &self,
    windows: &mut Vec<Hosted>,
    focus: &mut Vec<HostWindow>,
  ) -> bool {
    let requests = std::mem::take(&mut *self.requests.borrow_mut());
    let any = !requests.is_empty();
    for request in requests {
      match request {
        Request::Open(id, r) => {
          let mut place = r.place;
          place.size = r.limits.clamp(place.size);
          if r.modal {
            // A modal window comes to the front when it opens.
            focus.push(id);
          }
          let session = Session::new(
            r.title.clone(),
            r.root,
            r.context,
            Settings::from(dovetail_core::system_settings()),
            Box::new(FixedPitch::default()),
          );
          windows.push(Hosted {
            info: TestWindowInfo {
              id,
              title: r.title,
              place,
              state: r.state,
              limits: r.limits,
              modal: r.modal,
            },
            session: Some(session),
            on_close: r.on_close,
            on_state: r.on_state,
            on_place: r.on_place,
            context: r.context,
          });
        },
        Request::Focus(id) => focus.push(id),
        Request::Close(id) => windows.retain(|w| w.info.id != id),
        Request::Title(id, title) => {
          if let Some(w) = windows.iter_mut().find(|w| w.info.id == id) {
            if let Some(s) = &mut w.session {
              s.set_title(title.clone());
            }
            w.info.title = title;
          }
        },
        Request::State(id, state) => {
          if let Some(w) = windows.iter_mut().find(|w| w.info.id == id) {
            w.info.state = state;
          }
        },
        Request::Limits(id, limits) => {
          if let Some(w) = windows.iter_mut().find(|w| w.info.id == id) {
            w.info.limits = limits;
            w.info.place.size = limits.clamp(w.info.place.size);
          }
        },
      }
    }
    any
  }
}
