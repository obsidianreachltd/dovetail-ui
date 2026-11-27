//! Menus handed to the system, where it has a menu bar of its own, such as
//! macOS (M08).
//!
//! The desktop crate installs a [`NativeMenus`] host with
//! [`set_native_menus`] on systems that have one. A menu bar component
//! then gives it a [`MenuModel`] instead of drawing menus in the window.

use std::cell::RefCell;
use std::rc::Rc;

use crate::view::Callback;

/// The menus of a menu bar, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuModel {
  /// The menus, left to right.
  pub menus: Vec<MenuSpec>,
}

/// One menu: its title and items.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuSpec {
  /// The title in the menu bar.
  pub title: String,
  /// The items, top to bottom.
  pub items: Vec<MenuItemSpec>,
}

/// One item in a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItemSpec {
  /// A line between groups of items.
  Separator,
  /// An item that runs a command.
  Command {
    /// The command's id, passed back when the item is chosen.
    id: String,
    /// The text shown.
    label: String,
    /// The item can be chosen now.
    enabled: bool,
    /// The keyboard shortcut shown next to it, if any.
    key: Option<MenuKey>,
    /// The command's icon, if it has one (M17).
    icon: Option<MenuIcon>,
    /// For a command that is on or off, whether it is on, shown with a
    /// tick (M25). `None` for a plain command.
    checked: Option<bool>,
  },
}

/// An icon for a system menu item (M17, D0024), drawn ready for the
/// system: this crate knows no image types.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuIcon {
  /// Its size in points (logical pixels) in the menu.
  pub size: f32,
  /// Width in pixels: more than `size` for sharp displays.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Red, green, blue and alpha bytes for each pixel, row by row, in sRGB,
  /// premultiplied by alpha, as system image types take them.
  pub pixels: Rc<[u8]>,
  /// A one-colour icon, which the system colours as it colours the menu's
  /// text; its colour bytes are black.
  pub template: bool,
}

/// A keyboard shortcut, as a system menu shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuKey {
  /// The key: a lower-case character such as `z`, or a name such as `F5`.
  pub key: String,
  /// With the platform's main modifier: Command on macOS, Ctrl elsewhere.
  pub primary: bool,
  /// With Shift.
  pub shift: bool,
  /// With Alt (Option on macOS).
  pub alt: bool,
}

/// A system menu bar that shows menus for the application.
pub trait NativeMenus {
  /// Shows `menus` in the system menu bar, replacing what was there.
  /// `on_choose` is called with a command's id when its item is chosen.
  fn set_menus(
    &self,
    menus: &MenuModel,
    on_choose: Callback<String>,
  );
}

thread_local! {
  static NATIVE: RefCell<Option<Rc<dyn NativeMenus>>> = const { RefCell::new(None) };
}

/// Installs the system menu bar for this thread, or removes it with `None`.
pub fn set_native_menus(host: Option<Rc<dyn NativeMenus>>) {
  NATIVE.with_borrow_mut(|n| *n = host);
}

/// The system menu bar, when this system has one that Dovetail uses.
pub fn native_menus() -> Option<Rc<dyn NativeMenus>> {
  NATIVE.with_borrow(Clone::clone)
}
