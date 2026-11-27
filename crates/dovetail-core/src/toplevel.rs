//! Top-level windows in the component tree (M19, D0029).
//!
//! A [`Window`] opens its window through the [`WindowHost`] when it is
//! first shown, and closes it when it goes from the tree. Its children are
//! shown in the window, and see the values the components around the
//! `Window` provide, such as the theme.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::reactive::{Signal, create_signal};
use crate::saved::SavedWindows;
use crate::screen::{Monitor, SavedMonitor, SavedWindow, fit, monitors};
use crate::tree::{BuildContext, current_component, use_state};
use crate::view::{Callback, Children, Component, View};
use crate::window::{
  HostWindow, WindowHost, WindowLimits, WindowPlace, WindowRequest,
  WindowState, window_host,
};

/// A top-level window, open while this component is shown.
///
/// ```
/// use dovetail_core::{View, Window, WindowProps};
///
/// let props = WindowProps::__builder()
///   .title("Settings")
///   .size((480.0, 360.0))
///   .children(vec![View::text("Hello")]);
/// let window = View::component::<Window>(WindowProps::__build(props));
/// ```
///
/// With `ui!`: `Window { title: "Settings", size: (480.0, 360.0), Text { value: "Hello" } }`.
///
/// - Its title, size limits and `state` follow its properties while it is
///   open. Without `state`, the window keeps the state the user gives it.
/// - When the user changes its state or place, `on_state` and `on_place`
///   are called.
/// - When the user closes it from its title bar, `on_close` is called, and
///   the window stays until this component stops being shown. Without
///   `on_close`, it closes, and opens again only when this component is
///   shown anew.
/// - Given a `name` and [`SavedWindows`], it opens where it was saved,
///   fitted onto the monitors there are, and saves its place and state when
///   they change.
/// - With `modal`, it blocks input to the window that shows it until it
///   closes.
/// - Where no window host is installed, as in a plain [`Tree`](crate::Tree),
///   it shows nothing and opens nothing.
pub struct Window;

/// The properties of [`Window`].
#[derive(Clone, PartialEq)]
pub struct WindowProps {
  /// The title.
  pub title: String,
  /// The drawing area's size when it opens, in logical pixels.
  pub size: (f32, f32),
  /// Where its top-left corner opens, frame included. Without it the
  /// system chooses.
  pub position: Option<(f32, f32)>,
  /// Its size limits, and whether it can be resized.
  pub limits: WindowLimits,
  /// The state it is kept in. Without it, the state it opens in is normal
  /// and the user changes it.
  pub state: Option<WindowState>,
  /// Called when the user closes it.
  pub on_close: Option<Callback<()>>,
  /// Called when its state changes.
  pub on_state: Option<Callback<WindowState>>,
  /// Called when it moves or changes size.
  pub on_place: Option<Callback<WindowPlace>>,
  /// The name its place and state are saved under in `saved`.
  pub name: Option<String>,
  /// Where its place and state are saved.
  pub saved: Option<SavedWindows>,
  /// Blocks input to the window that shows it until it closes.
  pub modal: bool,
  /// What it shows.
  pub children: Children,
}

impl Default for WindowProps {
  fn default() -> Self {
    Self {
      title: String::new(),
      size: (800.0, 600.0),
      position: None,
      limits: WindowLimits::default(),
      state: None,
      on_close: None,
      on_state: None,
      on_place: None,
      name: None,
      saved: None,
      modal: false,
      children: Vec::new(),
    }
  }
}

/// The builder `ui!` uses. Not for direct use: it may change in any
/// release.
#[doc(hidden)]
impl WindowProps {
  pub fn __builder() -> Self {
    Self::default()
  }

  pub fn __build(builder: Self) -> Self {
    builder
  }

  pub fn title(
    mut self,
    value: impl Into<String>,
  ) -> Self {
    self.title = value.into();
    self
  }

  pub fn size(
    mut self,
    value: (f32, f32),
  ) -> Self {
    self.size = value;
    self
  }

  pub fn position(
    mut self,
    value: (f32, f32),
  ) -> Self {
    self.position = Some(value);
    self
  }

  pub fn min_size(
    mut self,
    value: (f32, f32),
  ) -> Self {
    self.limits.min_size = Some(value);
    self
  }

  pub fn max_size(
    mut self,
    value: (f32, f32),
  ) -> Self {
    self.limits.max_size = Some(value);
    self
  }

  pub fn resizable(
    mut self,
    value: bool,
  ) -> Self {
    self.limits.resizable = value;
    self
  }

  pub fn limits(
    mut self,
    value: WindowLimits,
  ) -> Self {
    self.limits = value;
    self
  }

  pub fn state(
    mut self,
    value: WindowState,
  ) -> Self {
    self.state = Some(value);
    self
  }

  pub fn on_close(
    mut self,
    value: impl Fn(()) + 'static,
  ) -> Self {
    self.on_close = Some(Callback::new(value));
    self
  }

  pub fn on_state(
    mut self,
    value: impl Fn(WindowState) + 'static,
  ) -> Self {
    self.on_state = Some(Callback::new(value));
    self
  }

  pub fn on_place(
    mut self,
    value: impl Fn(WindowPlace) + 'static,
  ) -> Self {
    self.on_place = Some(Callback::new(value));
    self
  }

  pub fn name(
    mut self,
    value: impl Into<String>,
  ) -> Self {
    self.name = Some(value.into());
    self
  }

  pub fn saved(
    mut self,
    value: SavedWindows,
  ) -> Self {
    self.saved = Some(value);
    self
  }

  pub fn modal(
    mut self,
    value: bool,
  ) -> Self {
    self.modal = value;
    self
  }

  pub fn children(
    mut self,
    value: Children,
  ) -> Self {
    self.children = value;
    self
  }
}

/// A window that a [`Window`] opened, kept in its state. Dropping it, when
/// the component goes, closes the window.
struct Open {
  host: Rc<dyn WindowHost>,
  window: HostWindow,
  title: String,
  limits: WindowLimits,
  /// The state the window is in, as last asked for or told.
  state: WindowState,
  /// The state property as last seen, so a change to it is applied once.
  asked: Option<WindowState>,
  /// The children shown, which the window's content reads.
  content: Signal<Children>,
  /// A copy of them, to compare without reading the signal.
  children: Children,
  /// The callbacks the window calls, as last given.
  on_close: Option<Callback<()>>,
  on_state: Option<Callback<WindowState>>,
  on_place: Option<Callback<WindowPlace>>,
  /// Where its place and state are saved, and under what name.
  saving: Option<(SavedWindows, String)>,
  /// Where it is, as last told.
  place: WindowPlace,
  closed: bool,
}

/// The monitor a window at `position`, in logical pixels at that
/// monitor's scale, is on: the first whose bounds hold it at its own
/// scale. Without a position, the primary monitor.
fn monitor_of(
  monitors: &[Monitor],
  position: Option<(f32, f32)>,
) -> Option<&Monitor> {
  match position {
    Some((x, y)) => monitors
      .iter()
      .find(|m| m.bounds.contains((x * m.scale, y * m.scale))),
    None => monitors.iter().find(|m| m.primary),
  }
  .or(monitors.first())
}

impl Open {
  /// Saves the window's place, while it is in its normal state.
  fn save_place(
    &mut self,
    place: WindowPlace,
  ) {
    self.place = place;
    if self.state == WindowState::Normal {
      self.save(place, WindowState::Normal);
    }
  }

  fn save(
    &self,
    place: WindowPlace,
    state: WindowState,
  ) {
    let Some((saved, name)) = &self.saving else {
      return;
    };
    let monitors = monitors();
    let monitor = monitor_of(&monitors, place.position);
    saved.set(
      name,
      SavedWindow {
        position: place.position,
        size: place.size,
        scale: monitor.map_or(1.0, |m| m.scale),
        state,
        monitor: monitor
          .map(|m| SavedMonitor { name: m.name.clone(), bounds: m.bounds }),
      },
    );
  }

  /// Saves the window's state. Its normal place stays as it was saved.
  fn save_state(
    &self,
    state: WindowState,
  ) {
    if let Some((saved, name)) = &self.saving {
      match saved.get(name) {
        Some(_) => saved.set_state(name, state),
        // Nothing saved yet: the place it opened at.
        None => self.save(self.place, state),
      }
    }
  }
}

impl Drop for Open {
  fn drop(&mut self) {
    if !self.closed {
      self.host.close(self.window);
    }
  }
}

/// What a window shows: its [`Window`]'s children, as they are now.
struct Content;

#[derive(PartialEq)]
struct ContentProps {
  content: Signal<Children>,
}

impl Component for Content {
  type Props = ContentProps;

  fn build(
    props: &ContentProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    // The window may outlive its `Window` for a moment, until the host
    // closes it.
    props
      .content
      .try_map(|c| View::fragment(c.iter().cloned()))
      .unwrap_or(View::Empty)
  }
}

type Shared = Rc<RefCell<Option<Open>>>;

fn opened(
  props: &WindowProps,
  host: Rc<dyn WindowHost>,
  me: Weak<RefCell<Option<Open>>>,
) -> Open {
  let content = create_signal(props.children.clone());
  let saving = props.saved.clone().zip(props.name.clone());
  // A saved window opens where it was, fitted onto the monitors there are.
  let restored = saving
    .as_ref()
    .and_then(|(saved, name)| saved.get(name))
    .map(|w| fit(&w, &monitors(), props.limits));
  let place = restored.as_ref().map_or(
    WindowPlace {
      position: props.position,
      size: props.size,
    },
    |r| r.0,
  );
  // A saved state wins over the property when it opens; the component is
  // told, so the property follows it from then on.
  let restored_state = restored.map(|r| r.1);
  let state = restored_state.or(props.state).unwrap_or_default();
  if let (Some(saved), Some(tell)) = (restored_state, &props.on_state)
    && props.state != Some(saved)
  {
    tell.call(saved);
  }
  let on = me.clone();
  let on_close = Callback::new(move |()| {
    let Some(shared) = on.upgrade() else {
      return;
    };
    let ask = shared
      .borrow()
      .as_ref()
      .and_then(|o| o.on_close.clone());
    match ask {
      Some(ask) => ask.call(()),
      // With nobody to ask, it closes.
      None => {
        if let Some(open) = shared.borrow_mut().as_mut()
          && !open.closed
        {
          open.closed = true;
          open.host.close(open.window);
        }
      },
    }
  });
  let on = me.clone();
  let on_state = Callback::new(move |state| {
    let Some(shared) = on.upgrade() else {
      return;
    };
    let tell = {
      let mut shared = shared.borrow_mut();
      let Some(open) = shared.as_mut() else {
        return;
      };
      open.state = state;
      open.save_state(state);
      open.on_state.clone()
    };
    if let Some(tell) = tell {
      tell.call(state);
    }
  });
  let on = me;
  let on_place = Callback::new(move |place| {
    let Some(shared) = on.upgrade() else {
      return;
    };
    let tell = shared.borrow_mut().as_mut().and_then(|o| {
      o.save_place(place);
      o.on_place.clone()
    });
    if let Some(tell) = tell {
      tell.call(place);
    }
  });
  let window = host.open(WindowRequest {
    place,
    on_close,
    on_place,
    limits: props.limits,
    state,
    on_state,
    modal: props.modal,
    // The window's components see what this one sees, such as the theme.
    context: current_component(),
    ..WindowRequest::new(
      props.title.clone(),
      View::component::<Content>(ContentProps { content }),
    )
  });
  Open {
    host,
    window,
    title: props.title.clone(),
    limits: props.limits,
    state,
    asked: props.state,
    content,
    children: props.children.clone(),
    on_close: props.on_close.clone(),
    on_state: props.on_state.clone(),
    on_place: props.on_place.clone(),
    saving,
    place,
    closed: false,
  }
}

/// Brings an open window up to date with its properties.
fn follow(
  open: &mut Open,
  props: &WindowProps,
) {
  open.on_close.clone_from(&props.on_close);
  open.on_state.clone_from(&props.on_state);
  open.on_place.clone_from(&props.on_place);
  if open.closed {
    return;
  }
  if open.title != props.title {
    open.title.clone_from(&props.title);
    open.host.set_title(open.window, &props.title);
  }
  if open.limits != props.limits {
    open.limits = props.limits;
    open.host.set_limits(open.window, props.limits);
  }
  if open.asked != props.state {
    open.asked = props.state;
    if let Some(state) = props.state
      && state != open.state
    {
      open.state = state;
      open.host.set_state(open.window, state);
    }
  }
  // The window's content follows the children. The signal is not read
  // here, so this component is not built again when it changes.
  if open.children != props.children {
    open.children.clone_from(&props.children);
    let children = props.children.clone();
    let _ = open.content.try_update(move |c| *c = children);
  }
}

impl Component for Window {
  type Props = WindowProps;

  fn build(
    props: &WindowProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    let shared: Shared = use_state(|| Rc::new(RefCell::new(None))).get();
    let mut slot = shared.borrow_mut();
    match slot.as_mut() {
      Some(open) => follow(open, props),
      None => {
        if let Some(host) = window_host() {
          *slot = Some(opened(props, host, Rc::downgrade(&shared)));
        }
      },
    }
    View::Empty
  }
}
