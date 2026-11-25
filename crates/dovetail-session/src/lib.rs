//! One window's component tree, layout, focus and input, with no window
//! (M20, decision D0027).
//!
//! A [`Session`] holds what a window shows and how it answers input: the
//! component tree and its layout, keyboard focus, the element under the
//! pointer, a press in progress, scroll bars and the caret. It has no
//! window, GPU or system behind it. `dovetail-desktop` gives each window
//! a session and turns the system's events into calls on it; tests make
//! one directly, so they run the same code as a real window.
//!
//! What a session would read from the system is given to it instead, in
//! [`Settings`] and the [`Measure`] it lays text out with.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use dovetail_core::{
  AccessAction, Callback, CloseGuard, ComponentId, Element, ImeEvent, KeyEvent,
  Modifiers, NamedKey, PointerButton, PointerEvent, PointerKind, Tree, View,
  Viewport, WheelEvent, WindowStyle,
};
use dovetail_layout::{BoxKey, Content, Layout, Measure, Rect, Scrollbar};

mod access;
mod drag;

pub use access::{
  AccessId, AccessKind, AccessNode, AccessTree, Actions, Numeric, Scroll,
  TextField, Toggle, access_tree, named_by_contents,
};
pub use drag::Drag;

/// How far one notch of the mouse wheel scrolls, in logical pixels.
pub const WHEEL_STEP: f32 = 48.0;

/// How long scrollbars that show only while scrolling stay after it stops.
const BARS_SHOWN_FOR: Duration = Duration::from_millis(1000);
/// How many times an update builds and lays out while what layout reports
/// changes the tree (M16).
const LAYOUT_PASSES: usize = 4;
/// Presses closer together than this in time count as one double click.
const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(500);
/// Presses closer together than this, in logical pixels, count as one
/// double click.
const DOUBLE_CLICK_DISTANCE: f32 = 4.0;
/// Space between an element and its focus ring, in logical pixels.
const FOCUS_RING_GAP: f32 = 1.0;
/// Width of the focus ring, in logical pixels.
const FOCUS_RING_WIDTH: f32 = 2.0;

/// What a session is told about the system, instead of asking it.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
  /// How long the caret stays on, and then off; `None` when it does not
  /// blink.
  pub caret_blink: Option<Duration>,
  /// Whether scroll bars show only while scrolling, as macOS can be set
  /// to. Asked each time, since the user can change it while the program
  /// runs.
  pub overlay_scrollbars: fn() -> bool,
}

impl Default for Settings {
  /// A caret that blinks every 530 ms, and scroll bars that always show.
  fn default() -> Self {
    Self {
      caret_blink: Some(Duration::from_millis(530)),
      overlay_scrollbars: || false,
    }
  }
}

/// What an action from an assistive tool asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
  /// Move keyboard focus to this element.
  Focus(BoxKey),
  /// Press this element.
  Click(BoxKey),
  /// Edit the text of this element.
  Edit(BoxKey, AccessAction),
}

/// What [`Session::update`] found, for the window to act on.
#[derive(Clone, Debug, Default)]
pub struct Update {
  /// The area whose boxes changed, in logical pixels.
  pub damage: Option<Rect>,
  /// An element asked for focus and it moved here, so the window should
  /// come to the front.
  pub focus_requested: bool,
  /// The title the elements ask for, when it changed.
  pub title: Option<String>,
  /// The window style the outermost theme asks for, when it changed.
  pub style: Option<WindowStyle>,
}

/// One window's component tree, layout, focus and input state.
pub struct Session {
  settings: Settings,
  measure: Box<dyn Measure>,
  /// The window's title, which names it for assistive tools.
  title: String,
  tree: Tree,
  layout: Layout,
  /// The pointer, in logical pixels; NaN when it is outside.
  cursor: (f32, f32),
  /// The handler under the pointer when it was pressed.
  pressed: Option<Callback<()>>,
  /// The element with keyboard focus.
  focus: Option<BoxKey>,
  /// Focus came from the keyboard, so the focus ring is shown.
  focus_visible: bool,
  /// The element with a pointer handler that the pointer went down on,
  /// and the click count. It receives moves and the release.
  captured: Option<(BoxKey, u32, PointerButton)>,
  /// When and where the last press was, and its click count, for double
  /// clicks.
  last_click: Option<(Instant, (f32, f32), u32)>,
  /// The blinking caret is in its on half.
  caret_on: bool,
  /// When the caret next turns on or off, while it blinks.
  blink_at: Option<Instant>,
  /// Elements that had focus before, most recent last, to give focus back
  /// to when the focused element goes away.
  focus_history: Vec<BoxKey>,
  /// The elements that ask for focus when they appear, as last laid out.
  autofocused: HashSet<BoxKey>,
  /// The window has the keyboard: it is in front. Carets show only then.
  active: bool,
  /// The sizes last told to elements with a size handler.
  sizes: HashMap<BoxKey, (f32, f32)>,
  /// The viewports last told to elements with a scroll handler (M16).
  viewports: HashMap<BoxKey, Viewport>,
  /// The close guard of an element in the window, as last laid out (M11).
  guard: Option<(CloseGuard, Callback<()>)>,
  /// How the window looks, from its outermost theme (M14).
  style: WindowStyle,
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
  /// When scrollbars that show only while scrolling hide again (M15).
  bars_hide_at: Option<Instant>,
}

impl Session {
  /// Mounts `root`, a component view, inside the component `context`
  /// when given, as a window does when it opens. Nothing is built or laid
  /// out until [`Session::update`].
  ///
  /// # Panics
  ///
  /// If `root` is not a component view.
  pub fn new(
    title: impl Into<String>,
    root: View,
    context: Option<ComponentId>,
    settings: Settings,
    measure: Box<dyn Measure>,
  ) -> Self {
    let mut tree = Tree::new();
    tree.mount_within(root, context);
    Self {
      settings,
      measure,
      title: title.into(),
      tree,
      layout: Layout::new(),
      cursor: (f32::NAN, f32::NAN),
      pressed: None,
      focus: None,
      focus_visible: false,
      captured: None,
      last_click: None,
      caret_on: true,
      blink_at: None,
      focus_history: Vec::new(),
      autofocused: HashSet::new(),
      active: true,
      sizes: HashMap::new(),
      viewports: HashMap::new(),
      guard: None,
      style: WindowStyle::default(),
      hovered: Vec::new(),
      within: Vec::new(),
      bar_hot: None,
      bar_drag: None,
      bars_hide_at: None,
    }
  }

  /// Builds what changed, and lays it out in `size` logical pixels, when
  /// the window shows anything (`None` while it is minimised).
  ///
  /// Sizes and scroll positions told after layout can change what is
  /// built, as a long list makes the rows that came into sight (M16,
  /// D0022): it builds and lays out again, a few times at most. Anything
  /// left waits for the next update.
  pub fn update(
    &mut self,
    size: Option<(f32, f32)>,
  ) -> Update {
    self.tree.update();
    let Some((width, height)) = size else {
      return Update::default();
    };
    let mut update = Update::default();
    for pass in 0..LAYOUT_PASSES {
      if pass > 0 {
        if !self.tree.needs_update() {
          break;
        }
        self.tree.update();
      }
      let changes =
        self
          .layout
          .update(&self.tree, width, height, self.measure.as_ref());
      if let Some(d) = changes.damage_bounds() {
        update.damage = Some(update.damage.map_or(d, |old| old.union(&d)));
      }
      self.settle_focus();
      self.update_focus_within();
      self.update_hover();
      update.focus_requested |= self.take_focus_request();
      self.report_sizes();
      self.report_scrolls();
    }
    update.title = self.read_lifecycle();
    update.style = self.read_style();
    // The caret blinks while the window is in front and the focused
    // element shows one; otherwise it stays on, or off in the background.
    let blinking = self.active && self.has_caret();
    match (blinking, self.settings.caret_blink) {
      (true, Some(rate)) => {
        if self.blink_at.is_none() {
          self.blink_at = Some(dovetail_core::now() + rate);
        }
      },
      _ => {
        self.caret_on = true;
        self.blink_at = None;
      },
    }
    update
  }

  /// True when something changed that the next update must build.
  pub fn needs_update(&self) -> bool {
    self.tree.needs_update()
  }

  /// The layout, as last updated.
  pub fn layout(&self) -> &Layout {
    &self.layout
  }

  /// What the session lays text out with.
  pub fn measure(&self) -> &dyn Measure {
    self.measure.as_ref()
  }

  /// The window's title: the one it opened with, or the last one its
  /// elements asked for.
  pub fn title(&self) -> &str {
    &self.title
  }

  /// Sets the title, as a component that opened the window can.
  pub fn set_title(
    &mut self,
    title: impl Into<String>,
  ) {
    self.title = title.into();
  }

  /// The window style the outermost theme asks for, as last updated.
  pub fn style(&self) -> WindowStyle {
    self.style
  }

  /// The close guard of an element in the window, as last updated (M11).
  pub fn close_guard(&self) -> Option<(CloseGuard, Callback<()>)> {
    self.guard.clone()
  }

  /// The element with keyboard focus.
  pub fn focus(&self) -> Option<&BoxKey> {
    self.focus.as_ref()
  }

  /// The element with keyboard focus, as last laid out.
  pub fn focused_element(&self) -> Option<&Element> {
    self.layout.element(self.focus.as_ref()?)
  }

  /// The pointer, in logical pixels; NaN when it is outside the window.
  pub fn cursor(&self) -> (f32, f32) {
    self.cursor
  }

  /// Moves the pointer without telling anything, as before a press or a
  /// wheel at a new place.
  pub fn set_cursor(
    &mut self,
    at: (f32, f32),
  ) {
    self.cursor = at;
  }

  /// True when the window has the keyboard: it is in front.
  pub fn active(&self) -> bool {
    self.active
  }

  /// The window came to the front or went behind. The caret shows again.
  pub fn set_active(
    &mut self,
    active: bool,
  ) {
    self.active = active;
    self.wake_caret();
  }

  /// Whether the caret is drawn now: in front, and in its on half.
  pub fn caret_visible(&self) -> bool {
    self.active && self.caret_on
  }

  /// While the caret blinks, whether it is in its on half; `None` when it
  /// does not blink.
  pub fn caret_blink(&self) -> Option<bool> {
    self.blink_at.map(|_| self.caret_on)
  }

  /// When the session next changes by itself: the caret blinks, or
  /// scrollbars hide.
  pub fn next_deadline(&self) -> Option<Instant> {
    [self.blink_at, self.bars_hide_at]
      .into_iter()
      .flatten()
      .min()
  }

  /// Carries out what is due by `now`: the caret turns on or off, and
  /// scrollbars hide. True when the window must be drawn again.
  pub fn tick(
    &mut self,
    now: Instant,
  ) -> bool {
    let mut changed = false;
    if let Some(rate) = self.settings.caret_blink
      && self.blink_at.is_some_and(|at| at <= now)
    {
      self.caret_on = !self.caret_on;
      self.blink_at = Some(now + rate);
      changed = true;
    }
    if self.bars_hide_at.is_some_and(|at| at <= now) {
      self.bars_hide_at = None;
      changed = true;
    }
    changed
  }

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
    handler: impl Fn(&Element) -> Option<&Callback<bool>>,
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

  /// A button went down where the pointer is. True when focus or a
  /// scrollbar changed, so the window must be drawn again.
  pub fn press(
    &mut self,
    modifiers: Modifiers,
    button: PointerButton,
  ) -> bool {
    let before = (self.focus.clone(), self.focus_visible);
    self.press_inner(modifiers, button);
    (self.focus.clone(), self.focus_visible) != before || self.bar_hot.is_some()
  }

  fn press_inner(
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

    let now = dovetail_core::now();
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

  /// Forgets the press in progress, as when a drag starts from it: the
  /// pressed element gets no more pointer events, and does not click.
  pub fn cancel_press(&mut self) {
    self.captured = None;
    self.pressed = None;
  }

  /// The wheel over the pointer: an element that handles the wheel gets it
  /// (as a canvas zooms); otherwise what is under the pointer scrolls by
  /// (`dx`, `dy`) logical pixels, sideways with Shift. `steps` is true for
  /// a wheel that moves in notches. True when something used it.
  pub fn wheel(
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
    if (self.settings.overlay_scrollbars)() {
      self.bars_hide_at = Some(dovetail_core::now() + BARS_SHOWN_FOR);
    }
  }

  /// The scrollbars to paint now, each with whether it is wide, as under
  /// the pointer (M15).
  pub fn scrollbars(&self) -> Vec<(Scrollbar, bool)> {
    let overlay = (self.settings.overlay_scrollbars)();
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
      .filter(|b| matches!(&b.content, Content::Element(e) if e.autofocus))
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
  pub fn carry_out(
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
      .focused_element()
      .and_then(|e| e.text_cursor)
      .is_some_and(|c| c.show_caret)
  }

  /// True when the focused element takes text from an input method.
  pub fn wants_ime(&self) -> bool {
    self
      .focused_element()
      .is_some_and(|e| e.on_ime.is_some())
  }

  /// Where the focused element's caret is, in logical pixels.
  pub fn caret_rect(&self) -> Option<Rect> {
    self
      .layout
      .caret_rect(self.focus.as_ref()?, self.measure.as_ref())
  }

  /// Passes input method text to the focused element. True when it took
  /// it.
  pub fn ime(
    &mut self,
    event: ImeEvent,
  ) -> bool {
    self.wake_caret();
    match self
      .focused_element()
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
      text_at: self
        .layout
        .text_at(key, x, y, self.measure.as_ref()),
      modifiers,
      clicks: *clicks,
    });
  }

  /// The pointer moved to where [`Session::set_cursor`] last put it. True
  /// when a scrollbar changed, so the window must be drawn again.
  pub fn pointer_moved(
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

  /// The pointer left the window. True when a scrollbar stopped being
  /// under it, so the window must be drawn again.
  pub fn pointer_left(&mut self) -> bool {
    self.cursor = (f32::NAN, f32::NAN);
    self.update_hover();
    self.bar_drag.is_none() && self.bar_hot.take().is_some()
  }

  /// A button was let go. A click happens when the pointer is released
  /// over the handler it was pressed on.
  pub fn release(
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
  pub fn key(
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
  /// came from the keyboard. Its outer rectangle and corner radius.
  pub fn focus_ring(&self) -> Option<(Rect, f32)> {
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

  /// After layout: tells elements with a size handler their new size.
  fn report_sizes(&mut self) {
    let mut calls = Vec::new();
    let mut seen = HashMap::new();
    for b in self.layout.boxes() {
      if let Content::Element(e) = &b.content
        && let Some(on_size) = &e.on_size
      {
        let size = (b.rect.width, b.rect.height);
        if self.sizes.get(&b.key) != Some(&size) {
          calls.push((on_size.clone(), size));
        }
        seen.insert(b.key.clone(), size);
      }
    }
    self.sizes = seen;
    for (on_size, size) in calls {
      on_size.call(size);
    }
  }

  /// After layout: tells scrolling elements with a scroll handler where
  /// they are scrolled to and how much can be seen, when it changes (M16).
  fn report_scrolls(&mut self) {
    let mut calls = Vec::new();
    let mut seen = HashMap::new();
    for b in self.layout.boxes() {
      if let Content::Element(e) = &b.content
        && let Some(on_scroll) = &e.on_scroll
        && let Some(view) = self.layout.viewport(&b.key)
      {
        if self.viewports.get(&b.key) != Some(&view) {
          calls.push((on_scroll.clone(), view));
        }
        seen.insert(b.key.clone(), view);
      }
    }
    self.viewports = seen;
    for (on_scroll, view) in calls {
      on_scroll.call(view);
    }
  }

  /// After layout: carries out a request to move focus, if this window
  /// shows its target. True when focus moved here.
  fn take_focus_request(&mut self) -> bool {
    let Some(target) = dovetail_core::pending_focus() else {
      return false;
    };
    if !self.layout.has_focus_target(target) {
      return false;
    }
    dovetail_core::focus_done(target);
    match self.layout.focus_for(target) {
      Some(key) => {
        self.set_focus(Some(key));
        self.focus_visible = true;
        true
      },
      None => false,
    }
  }

  /// After layout: reads the title and close guard that the window's
  /// elements ask for. Returns the new title, when it changed.
  fn read_lifecycle(&mut self) -> Option<String> {
    let mut title = None;
    let mut guard = None;
    for b in self.layout.boxes() {
      if let Content::Element(e) = &b.content {
        if title.is_none() {
          title = e.window_title.clone();
        }
        if guard.is_none() {
          guard = e.close_guard.clone();
        }
      }
    }
    self.guard = guard;
    let title = title.filter(|t| *t != self.title)?;
    self.title = title.clone();
    Some(title)
  }

  /// After layout: the window style the outermost theme asks for, when it
  /// changed.
  fn read_style(&mut self) -> Option<WindowStyle> {
    let style = self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Element(e) => e.window_style,
        _ => None,
      })
      .unwrap_or_default();
    (style != self.style).then(|| {
      self.style = style;
      style
    })
  }
}
