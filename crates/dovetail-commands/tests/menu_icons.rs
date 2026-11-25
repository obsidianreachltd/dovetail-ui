//! M17: commands' icons reach a system menu bar (as on macOS), drawn at
//! twice their size, as templates when they have one colour (D0024).
//! With a stand-in for the system, so it runs everywhere.

extern crate dovetail_core as dovetail;

use std::cell::RefCell;
use std::rc::Rc;

use dovetail::{
  Callback, MenuItemSpec, MenuModel, NativeMenus, Tree, View, set_native_menus,
  use_state,
};
use dovetail_commands::{Commands, Menu, MenuBar};
use dovetail_image::{Icon, icons};
use dovetail_macros::{component, ui};

/// Keeps the menus it is given.
struct Recorder(Rc<RefCell<Option<MenuModel>>>);

impl NativeMenus for Recorder {
  fn set_menus(
    &self,
    menus: &MenuModel,
    _: Callback<String>,
  ) {
    *self.0.borrow_mut() = Some(menus.clone());
  }
}

fn two_colours() -> Icon {
  Icon::from_svg(
    r##"<svg viewBox="0 0 2 1"><rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1" fill="#00f"/></svg>"##,
  )
  .unwrap()
}

#[component]
fn App() -> View {
  let commands = use_state(|| {
    Commands::new()
      .define("file.save", "Save", Some("Mod+S"), || {})
      .define("file.flag", "Flag", None, || {})
      .define("file.plain", "Plain", None, || {})
      .with_icon("file.save", icons::save())
      .with_icon("file.flag", two_colours())
  })
  .get();
  let menus = vec![
    Menu::new("File")
      .command("file.save")
      .command("file.flag")
      .command("file.plain"),
  ];
  ui! { MenuBar { commands: commands, menus: menus } }
}

#[test]
fn icons_reach_the_system_menu_bar() {
  let seen = Rc::new(RefCell::new(None));
  set_native_menus(Some(Rc::new(Recorder(seen.clone()))));
  let mut tree = Tree::new();
  tree.mount(ui! { App {} });
  tree.update();
  set_native_menus(None);
  let model = seen
    .borrow()
    .clone()
    .expect("menus given to the system");
  let icons: Vec<_> = model.menus[0]
    .items
    .iter()
    .map(|item| match item {
      MenuItemSpec::Command { icon, .. } => icon.clone(),
      MenuItemSpec::Separator => None,
    })
    .collect();
  let save = icons[0].as_ref().expect("Save has an icon");
  assert_eq!((save.size, save.width, save.height), (16.0, 32, 32));
  assert!(save.template, "one colour: the system colours it");
  assert_eq!(save.pixels.len(), 32 * 32 * 4);
  assert!(save.pixels.chunks(4).all(|p| p[..3] == [0, 0, 0]));
  assert!(save.pixels.chunks(4).any(|p| p[3] == 255));
  let flag = icons[1].as_ref().expect("Flag has an icon");
  assert!(!flag.template, "two colours: kept");
  // The red half and the blue half, premultiplied and opaque.
  assert_eq!(
    &flag.pixels[(16 * 32 + 4) * 4..(16 * 32 + 4) * 4 + 4],
    &[255, 0, 0, 255]
  );
  assert_eq!(
    &flag.pixels[(16 * 32 + 28) * 4..(16 * 32 + 28) * 4 + 4],
    &[0, 0, 255, 255]
  );
  assert!(icons[2].is_none());
}
