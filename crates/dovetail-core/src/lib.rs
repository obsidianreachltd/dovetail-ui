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
mod appearance;
mod clipboard;
mod dialogs;
mod drag;
mod element;
mod focus;
mod input;
mod menu;
mod picture;
mod reactive;
mod task;
mod tree;
mod view;
mod window;

pub use appearance::{
  SystemAppearance, set_system_appearance, system_appearance,
};
pub use clipboard::{
  Clipboard, MemoryClipboard, clipboard_text, set_clipboard, set_clipboard_text,
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
pub use task::{
  Progress, Task, TaskContext, TaskEnd, run_task_messages, set_task_waker,
  spawn_task, tasks_running,
};
pub use tree::{
  BuildContext, Invalidation, Tree, Update, context_from, current_component,
  provide_context, use_context, use_state,
};
pub use view::{Callback, Children, Component, ComponentView, Key, View};
pub use window::{
  CloseGuard, HostWindow, WindowHost, WindowPlace, WindowRequest,
  set_window_host, take_close_requests, window_host,
};
