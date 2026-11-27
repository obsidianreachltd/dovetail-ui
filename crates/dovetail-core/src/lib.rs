//! Component contracts, identity, events and reactive state for Dovetail UI.
//!
//! This crate depends on no other Dovetail crate. Everything here can be
//! used from plain Rust; the `ui!` macro (M03) generates the same calls.
//!
//! - A [`Component`] has typed properties and builds a [`View`].
//! - A [`Tree`] holds component instances. Each has a [`ComponentId`] that is
//!   never reused.
//! - [`BuildContext::use_state`] gives a component a [`Signal`]: reactive
//!   state with a `Copy` handle (decision D0010).
//! - Reading a signal while building subscribes the component; changing it
//!   marks the component, and [`Tree::update`] builds it again.
//! - A [`Callback`] carries a typed event from a child to its parent.
//! - [`View::keyed`] keeps list items' instances and state when the list
//!   changes.
//! - Removing a component drops its state and subscriptions.
//!
//! The component tree and its state belong to the UI thread.

#[doc(hidden)]
pub mod __private;
mod animate;
mod appearance;
mod clipboard;
mod clock;
mod date;
mod dialogs;
mod drag;
mod element;
mod focus;
mod frame;
mod input;
mod menu;
mod picture;
mod reactive;
mod saved;
mod screen;
mod settings;
mod task;
mod toplevel;
mod tree;
mod view;
mod window;

pub use animate::{Animate, Curve, Transition, use_animated};
pub use appearance::{
  SystemAppearance, set_system_appearance, system_appearance,
};
pub use clipboard::{
  Clipboard, MemoryClipboard, clipboard_text, set_clipboard, set_clipboard_text,
};
pub use clock::{Clock, TestClock, now, set_clock};
pub use date::{
  Date, DateSource, MAX_YEAR, MIN_YEAR, MONTH_NAMES, Time, Weekday,
  days_in_month, is_leap_year, set_date_source, set_today, today,
};
pub use dialogs::{
  FileDialog, FileDialogKind, FileDialogs, FileFilter, file_dialogs,
  set_file_dialogs, show_file_dialog,
};
pub use drag::{
  DragData, DragEnd, DropEvent, DropHandler, DropKind, StartedDrag, start_drag,
  take_started_drag,
};
pub use element::{
  Align, Border, CanvasSource, Color, Edges, Element, ElementKind, FontWeight,
  ItemPlace, Justify, Layer, Role, ScrollRequest, Size, SortOrder, Style,
  TextCursor, ValueRange, Viewport, WindowStyle,
};
pub use focus::{FocusTarget, focus_done, pending_focus};
pub use frame::{frame_requested, request_frame, start_frame};
pub use input::{
  AccessAction, ImeEvent, KeyCode, KeyEvent, KeyHandler, Modifiers, NamedKey,
  PointerButton, PointerEvent, PointerHandler, PointerKind, WheelEvent,
};
pub use menu::{
  MenuIcon, MenuItemSpec, MenuKey, MenuModel, MenuSpec, NativeMenus,
  native_menus, set_native_menus,
};
pub use picture::{Fit, ICON_SCALE, Picture, PictureKind, fit_rect};
pub use reactive::{
  ComponentId, RuntimeStats, Signal, StateError, create_signal, runtime_stats,
  set_on_change,
};
pub use saved::{SavedReadError, SavedWindows};
pub use screen::{
  FRAME_ROOM, Monitor, SavedMonitor, SavedWindow, ScreenRect, fit, monitor_at,
  monitors, set_monitors,
};
pub use settings::{
  MAX_TEXT_SCALE, MIN_TEXT_SCALE, SystemSettings, caret_blink,
  double_click_distance, double_click_time, overlay_scrollbars, reduce_motion,
  set_system_settings, system_settings, text_scale,
};
pub use task::{
  Progress, Task, TaskContext, TaskEnd, next_timer, run_task_messages,
  set_task_waker, spawn_task, tasks_running, timers_waiting,
};
pub use toplevel::{Window, WindowProps};
pub use tree::{
  BuildContext, Invalidation, Tree, Update, context_from, current_component,
  provide_context, use_context, use_state,
};
pub use view::{Callback, Children, Component, ComponentView, Key, View};
pub use window::{
  AppWindow, CloseGuard, HostWindow, WindowHost, WindowLimits, WindowPlace,
  WindowRequest, WindowState, app_windows, bring_to_front, set_app_windows,
  set_window_host, take_close_requests, window_host,
};
