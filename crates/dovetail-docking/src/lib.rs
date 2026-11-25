//! Docking for Dovetail UI (M10): panels in tab stacks, split views that
//! resize, and floating windows, with layouts saved between runs.
//!
//! - [`DockLayout`] is the layout as plain data: splits of tab stacks in
//!   the main window, and floating stacks. It moves panels about, and is
//!   saved as text ([`DockLayout::to_text`], [`DockLayout::from_text`]).
//! - [`Dock`] holds the layout and the panels while the program runs.
//!   [`Dock::keep_in_file`] saves the layout whenever it changes.
//! - [`DockArea`] shows the main window's panels. Floating stacks open in
//!   windows of their own.
//!
//! Using it:
//!
//! - drag a tab onto another stack's tabs to join it, onto the middle of a
//!   panel to join its stack, near a panel's side to dock beside it, or
//!   onto the thin edge round the dock to dock along that side;
//! - drag a tab out of every window, or press Float, to float it, and drag
//!   it back or press Dock to dock it again;
//! - drag the lines between panels to resize them;
//! - F6 and Shift+F6 move between panels, Ctrl+Page Down and Ctrl+Page Up
//!   between tabs, and Left and Right along a tab bar.

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

mod model;
mod text;
mod views;

pub use model::{Axis, DockLayout, Floating, Node, Place, Side, StackAt, Tabs};
pub use text::{ReadError, valid_panel_id};
pub use views::{
  Dock, DockArea, DraggedPanel, FloatingStack, PanelInfo, placeholder,
};
