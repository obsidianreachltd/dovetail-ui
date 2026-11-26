//! Themes in a component tree, without a window (M14 "Done when" items 1
//! and 3): reading the nearest theme, being built again when it changes,
//! overriding part of it for a subtree, and following the system.

extern crate dovetail_core as dovetail;

use std::cell::RefCell;

use dovetail::{
  Color, ComponentId, Signal, SystemAppearance, SystemSettings, Tree, View,
  set_system_appearance, set_system_settings, use_context, use_state,
};
use dovetail_macros::{component, ui};
use dovetail_theme::{Brightness, Theme, ThemeData, ThemeMode, theme};

thread_local! {
  /// What each reader saw, in build order: its name and primary colour.
  static SEEN: RefCell<Vec<(&'static str, Color)>> = const { RefCell::new(Vec::new()) };
  /// The signals the test changes, made by the root.
  static KNOBS: RefCell<Option<(Signal<ThemeData>, Signal<Color>)>> = const { RefCell::new(None) };
}

/// These tests check which theme is chosen, not how it fades in, so the
/// colours change at once (M18).
fn no_motion() {
  set_system_settings(SystemSettings {
    reduce_motion: true,
    ..SystemSettings::default()
  });
}

fn seen() -> Vec<(&'static str, Color)> {
  SEEN.with_borrow_mut(std::mem::take)
}

#[component]
fn Reader(name: &'static str) -> View {
  let t = theme();
  SEEN.with_borrow_mut(|s| s.push((name, t.colors.primary)));
  View::Empty
}

/// A reader inside a theme that overrides the primary colour.
#[component]
fn Override() -> View {
  let (_, accent) = KNOBS.with_borrow(|k| k.unwrap());
  let mut data = theme();
  data.colors.primary = accent.get();
  ui! {
    Theme {
      data: data,
      Reader { name: "inner" }
    }
  }
}

#[component]
fn Root(#[prop(default)] mode: ThemeMode) -> View {
  let data = use_state(ThemeData::light);
  let accent = use_state(|| Color::hex(0xff0000));
  KNOBS.with_borrow_mut(|k| *k = Some((data, accent)));
  ui! {
    Theme {
      data: data.get(),
      dark: Some(ThemeData::dark()),
      mode: mode,
      Reader { name: "outer" }
      Override {}
    }
  }
}

fn knobs() -> (Signal<ThemeData>, Signal<Color>) {
  KNOBS.with_borrow(|k| k.unwrap())
}

#[test]
fn readers_see_the_nearest_theme_and_follow_changes() {
  no_motion();
  set_system_appearance(SystemAppearance::default());
  let mut tree = Tree::new();
  tree.mount(ui! { Root { mode: ThemeMode::Light } });
  let light = ThemeData::light().colors.primary;
  let red = Color::hex(0xff0000);
  assert_eq!(seen(), [("outer", light), ("inner", red)]);

  // A new theme at the root reaches both readers.
  let (data, accent) = knobs();
  let green = ThemeData::from_seed(Color::hex(0x40a02b), Brightness::Light);
  data.set(green.clone());
  tree.update();
  let after = seen();
  assert!(
    after.contains(&("outer", green.colors.primary)),
    "{after:?}"
  );
  assert!(after.contains(&("inner", red)), "the override still wins");

  // Changing only the inner override builds only what is inside it.
  accent.set(Color::hex(0x0000ff));
  tree.update();
  assert_eq!(seen(), [("inner", Color::hex(0x0000ff))]);
}

#[test]
fn without_a_theme_controls_use_the_light_theme() {
  let mut tree = Tree::new();
  tree.mount(ui! { Reader { name: "alone" } });
  assert_eq!(seen(), [("alone", ThemeData::light().colors.primary)]);
}

#[test]
fn the_system_mode_follows_dark_and_high_contrast() {
  no_motion();
  set_system_appearance(SystemAppearance::default());
  let mut tree = Tree::new();
  tree.mount(ui! { Root {} });
  let light = ThemeData::light().colors.primary;
  assert_eq!(seen()[0], ("outer", light));

  set_system_appearance(SystemAppearance { dark: true, ..Default::default() });
  tree.update();
  let dark = ThemeData::dark().colors.primary;
  assert!(seen().contains(&("outer", dark)), "dark follows the system");

  set_system_appearance(SystemAppearance {
    dark: true,
    high_contrast: true,
    ..Default::default()
  });
  tree.update();
  let contrast = ThemeData::dark().high_contrast().colors.primary;
  assert!(seen().contains(&("outer", contrast)));
  set_system_appearance(SystemAppearance::default());
}

/// A value provided in one tree is seen by a tree mounted within it, as a
/// floating window sees its opener's theme.
#[test]
fn a_second_tree_sees_the_values_of_the_component_it_is_within() {
  thread_local! {
    static OUTER: RefCell<Option<ComponentId>> = const { RefCell::new(None) };
    static GOT: RefCell<Option<u32>> = const { RefCell::new(None) };
  }
  #[component]
  fn Provider() -> View {
    let value = use_state(|| 7u32);
    dovetail::provide_context(value);
    OUTER.with_borrow_mut(|o| *o = dovetail::current_component());
    View::Empty
  }
  #[component]
  fn Consumer() -> View {
    let got = use_context::<Signal<u32>>().map(|s| s.get());
    GOT.with_borrow_mut(|g| *g = got);
    View::Empty
  }
  let mut first = Tree::new();
  first.mount(ui! { Provider {} });
  let outer = OUTER.with_borrow(|o| *o);
  let mut second = Tree::new();
  second.mount_within(ui! { Consumer {} }, outer);
  assert_eq!(GOT.with_borrow(|g| *g), Some(7));
  // Without it, nothing is seen.
  let mut third = Tree::new();
  third.mount(ui! { Consumer {} });
  assert_eq!(GOT.with_borrow(|g| *g), None);
}
