//! M25 "Done when" items 1, 3 and 4: commands that are on or off show
//! their state everywhere they appear; a toolbar too narrow for its
//! buttons puts the rest behind a More button; and a tool palette chooses
//! tools by click, letter key, long press, right-click and the keyboard.
//! With `TestUi`: no window or GPU.

extern crate dovetail_core as dovetail;

use std::cell::Cell;

use dovetail::{
  KeyEvent, MenuItemSpec, MenuModel, Modifiers, NamedKey, NativeMenus,
  PointerButton, Role, View, set_native_menus, use_state,
};
use dovetail_commands::{
  CommandButton, CommandGroup, CommandScope, CommandSearch, Commands,
  ContextMenu, LONG_PRESS, Menu, MenuBar, ToolPalette, Toolbar,
};
use dovetail_controls::{Column, Text, TextField};
use dovetail_macros::{component, ui};
use dovetail_testing::{Options, TestUi, role, text};

thread_local! {
  static COMMANDS: Cell<Option<Commands>> = const { Cell::new(None) };
}

/// The commands of a small drawing program.
fn commands() -> Commands {
  let icons = dovetail_image::icons::ALL;
  let icon = |name: &str| {
    icons
      .iter()
      .find(|(n, _)| *n == name)
      .map(|(_, f)| f())
      .expect("an icon")
  };
  Commands::new()
    .define_toggle("view.grid", "Grid", Some("Mod+G"), |_| {})
    .with_icon("view.grid", icon("grid"))
    .define_toggle("view.snap", "Snap to grid", None, |_| {})
    .with_icon("view.snap", icon("magnet"))
    .define_choice("tool.select", "tools", "Select", Some("V"), || {})
    .with_icon("tool.select", icon("pointer"))
    .define_choice("tool.pen", "tools", "Pen", Some("P"), || {})
    .with_icon("tool.pen", icon("pen"))
    .define_choice("tool.brush", "tools", "Brush", None, || {})
    .with_icon("tool.brush", icon("brush"))
    .define_choice("tool.rect", "tools", "Rectangle", Some("U"), || {})
    .with_icon("tool.rect", icon("rectangle"))
    .define_choice("tool.ellipse", "tools", "Ellipse", None, || {})
    .with_icon("tool.ellipse", icon("ellipse"))
    .define_choice("tool.line", "tools", "Line", None, || {})
    .with_icon("tool.line", icon("line"))
    .define("edit.clear", "Clear", None, || {})
}

fn ids(list: &[&str]) -> Vec<String> {
  list.iter().map(|s| s.to_string()).collect()
}

#[component]
fn Drawing() -> View {
  let commands = use_state(commands).get();
  let name = use_state(String::new);
  COMMANDS.set(Some(commands));
  let menus = vec![
    Menu::new("View")
      .command("view.grid")
      .command("view.snap"),
    Menu::new("Tools")
      .command("tool.select")
      .command("tool.pen")
      .separator()
      .command("edit.clear"),
  ];
  ui! {
    CommandScope {
      commands: commands,
      Column {
        MenuBar { commands: commands, menus: menus }
        Toolbar {
          CommandButton { commands: commands, id: "view.grid", icon_only: true }
        }
        ToolPalette {
          commands: commands,
          tools: vec![
            ids(&["tool.select"]),
            ids(&["tool.pen", "tool.brush"]),
            ids(&["tool.rect", "tool.ellipse", "tool.line"]),
          ],
          columns: Some(1),
        }
        TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
        Text { value: format!("name {}", name.get()) }
        ContextMenu {
          commands: commands,
          items: vec![dovetail_commands::MenuItem::Command("view.snap".into())],
          label: "Canvas menu",
          Text { value: "canvas" }
        }
        CommandSearch { commands: commands }
      }
    }
  }
}

fn commands_now() -> Commands {
  COMMANDS.get().expect("built")
}

#[test]
fn a_toggle_shows_a_tick_in_menus_and_a_pressed_button() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  let grid = role(Role::Button).label("Grid");
  assert_eq!(ui.find(grid.clone()).is_on(), Some(false));
  ui.click(grid.clone());
  assert_eq!(commands_now().is_on("view.grid"), Some(true));
  assert_eq!(ui.find(grid.clone()).is_on(), Some(true), "pressed");

  // The menu shows a tick, read as a checked item.
  ui.click(role(Role::MenuItem).label("View"));
  let item = ui.find(role(Role::MenuItemCheckBox).label("Grid"));
  assert_eq!(item.is_on(), Some(true));
  let snap = role(Role::MenuItemCheckBox).label("Snap to grid");
  assert_eq!(ui.find(snap.clone()).is_on(), Some(false));
  ui.click(snap);
  assert_eq!(commands_now().is_on("view.snap"), Some(true));

  // A shortcut turns it over, and every view follows.
  ui.shortcut('g');
  assert_eq!(ui.find(grid).is_on(), Some(false));

  // Command search says the state.
  ui.press(KeyEvent::character('p').with(Modifiers {
    shift: true,
    ..dovetail_testing::command()
  }));
  ui.type_text("grid");
  let found = ui.find(role(Role::ListBoxOption).label("Grid"));
  assert_eq!(found.node().description.as_deref(), Some("Off"));
}

#[test]
fn choices_show_a_dot_and_one_is_on_everywhere() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  ui.click(role(Role::MenuItem).label("Tools"));
  let select = ui.find(role(Role::MenuItemRadio).label("Select"));
  assert_eq!(select.is_on(), Some(true), "the first tool starts on");
  ui.click(role(Role::MenuItemRadio).label("Pen"));
  assert_eq!(commands_now().chosen("tools").as_deref(), Some("tool.pen"));
  // A plain command in the same menu is a plain item.
  ui.click(role(Role::MenuItem).label("Tools"));
  assert!(ui.has(role(Role::MenuItem).label("Clear")));
  ui.key(NamedKey::Escape);
}

#[component]
fn Grouped() -> View {
  let commands = use_state(commands).get();
  COMMANDS.set(Some(commands));
  ui! {
    Column {
      CommandGroup { commands: commands, ids: ids(&["tool.select", "tool.pen"]), label: "Two tools" }
      CommandGroup { commands: commands, ids: ids(&["view.grid", "view.snap"]), icon_only: true, label: "View" }
      CommandGroup { commands: commands, ids: ids(&["edit.clear", "view.grid"]), label: "Mixed" }
    }
  }
}

#[test]
fn groups_made_from_commands_follow_their_state() {
  let mut ui = TestUi::new(ui! { Grouped {} });
  // Choices of one group are a segmented control.
  let group = ui.find(role(Role::RadioGroup).label("Two tools"));
  let pen = ui.find_within(&group, role(Role::RadioButton).label("Pen"));
  ui.click_at(pen.centre());
  assert_eq!(commands_now().chosen("tools").as_deref(), Some("tool.pen"));
  let group = ui.find(role(Role::RadioGroup).label("Two tools"));
  let pen = ui.find_within(&group, role(Role::RadioButton).label("Pen"));
  assert_eq!(pen.is_on(), Some(true));

  // Toggles are toggle buttons, here with icons only, named by command.
  let view = ui.find(role(Role::Group).label("View"));
  let snap = ui.find_within(&view, role(Role::Button).label("Snap to grid"));
  ui.click_at(snap.centre());
  assert_eq!(commands_now().is_on("view.snap"), Some(true));
  let view = ui.find(role(Role::Group).label("View"));
  let snap = ui.find_within(&view, role(Role::Button).label("Snap to grid"));
  assert_eq!(snap.is_on(), Some(true));

  // A mix is plain buttons.
  let mixed = ui.find(role(Role::Group).label("Mixed"));
  let grid = ui.find_within(&mixed, role(Role::Button).label("Grid"));
  assert_eq!(grid.is_on(), None);
  ui.click_at(grid.centre());
  assert_eq!(
    commands_now().is_on("view.grid"),
    Some(true),
    "it still runs"
  );
}

#[test]
fn a_context_menu_shows_the_state_too() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  ui.right_click(text("canvas"));
  let snap = ui.find(role(Role::MenuItemCheckBox).label("Snap to grid"));
  assert_eq!(snap.is_on(), Some(false));
  ui.click(role(Role::MenuItemCheckBox).label("Snap to grid"));
  assert_eq!(commands_now().is_on("view.snap"), Some(true));
}

#[derive(Default)]
struct Host {
  last: std::cell::RefCell<Option<MenuModel>>,
}

impl NativeMenus for Host {
  fn set_menus(
    &self,
    menus: &MenuModel,
    _: dovetail::Callback<String>,
  ) {
    *self.last.borrow_mut() = Some(menus.clone());
  }
}

#[test]
fn the_system_menu_bar_is_told_which_commands_are_on() {
  let host = std::rc::Rc::new(Host::default());
  set_native_menus(Some(host.clone()));
  let ui = TestUi::new(ui! { Drawing {} });
  let checked = |id: &str| -> Option<Option<bool>> {
    host
      .last
      .borrow()
      .as_ref()?
      .menus
      .iter()
      .find_map(|m| {
        m.items.iter().find_map(|i| match i {
          MenuItemSpec::Command { id: c, checked, .. } if c == id => {
            Some(*checked)
          },
          _ => None,
        })
      })
  };
  assert_eq!(checked("view.grid"), Some(Some(false)));
  assert_eq!(checked("tool.select"), Some(Some(true)));
  assert_eq!(checked("edit.clear"), Some(None));
  commands_now().run("view.grid");
  let mut ui = ui;
  ui.update();
  assert_eq!(checked("view.grid"), Some(Some(true)), "it follows");
  set_native_menus(None);
}

#[test]
fn the_palette_chooses_by_click_letter_key_and_keyboard() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  let palette = ui.find(role(Role::Toolbar).label("Tools"));
  let tool = |ui: &TestUi, name: &str| {
    let palette = ui.find(role(Role::Toolbar).label("Tools"));
    ui.find_within(&palette, role(Role::RadioButton).label(name))
  };
  assert_eq!(tool(&ui, "Select").is_on(), Some(true));
  let pen = tool(&ui, "Pen");
  ui.click_at(pen.centre());
  assert_eq!(commands_now().chosen("tools").as_deref(), Some("tool.pen"));
  assert_eq!(tool(&ui, "Pen").is_on(), Some(true));

  // A letter key, from anywhere but a text field.
  ui.focus(role(Role::RadioButton).label("Select").clone());
  ui.type_text("u");
  assert_eq!(commands_now().chosen("tools").as_deref(), Some("tool.rect"));
  // Shift and the group's first letter goes through the group.
  ui.press(
    KeyEvent::character('U')
      .with(Modifiers { shift: true, ..Modifiers::default() }),
  );
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.ellipse")
  );
  assert!(
    tool(&ui, "Ellipse").is_on() == Some(true),
    "the slot shows it"
  );

  // One Tab stop; the arrow keys move between tools, Enter chooses.
  ui.focus(role(Role::RadioButton).label("Ellipse"));
  ui.key(NamedKey::ArrowUp);
  ui.key(NamedKey::ArrowUp);
  assert_eq!(ui.focused().unwrap().label(), Some("Select"));
  ui.key(NamedKey::Enter);
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.select")
  );
  // The group remembers the tool used last.
  assert!(ui.has(role(Role::RadioButton).label("Ellipse")));
  drop(palette);
}

#[test]
fn a_letter_key_in_a_text_field_types_and_does_not_change_the_tool() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  ui.focus(role(Role::TextInput).label("Name"));
  ui.type_text("puv");
  assert!(ui.has(text("name puv")));
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.select"),
    "unchanged"
  );
  // A letter with no text, as while an input method composes, is still
  // the field's.
  let mut quiet = KeyEvent::character('p');
  quiet.text = None;
  ui.press(quiet);
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.select")
  );
}

#[test]
fn a_group_opens_by_long_press_right_click_and_the_keyboard() {
  let mut ui = TestUi::new(ui! { Drawing {} });
  let pen = role(Role::RadioButton).label("Pen");
  let at = ui.find(pen.clone()).centre();
  ui.press_at(at, PointerButton::Primary, Modifiers::default());
  ui.advance(LONG_PRESS / 2);
  assert!(!ui.has(role(Role::Menu).label("Tools in this group")));
  ui.advance(LONG_PRESS);
  assert!(ui.has(role(Role::Menu).label("Tools in this group")));
  ui.release(PointerButton::Primary, Modifiers::default());
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.select"),
    "the release after a long press chooses nothing"
  );
  ui.key(NamedKey::ArrowDown);
  ui.key(NamedKey::Enter);
  assert_eq!(
    commands_now().chosen("tools").as_deref(),
    Some("tool.brush")
  );
  assert!(!ui.has(role(Role::Menu).label("Tools in this group")));
  // The slot now shows the brush.
  assert!(ui.has(role(Role::RadioButton).label("Brush")));
  assert!(!ui.has(pen.clone()));

  // A right-click opens it too.
  ui.right_click(role(Role::RadioButton).label("Brush"));
  ui.click(role(Role::MenuItemRadio).label("Pen"));
  assert_eq!(commands_now().chosen("tools").as_deref(), Some("tool.pen"));

  // And the keyboard: from the Tab stop, Down to the next slot, then
  // Alt+Down, then Escape.
  ui.focus(role(Role::RadioButton).label("Pen"));
  ui.key(NamedKey::ArrowDown);
  assert_eq!(ui.focused().unwrap().label(), Some("Rectangle"));
  ui.press(
    KeyEvent::named(NamedKey::ArrowDown)
      .with(Modifiers { alt: true, ..Modifiers::default() }),
  );
  assert!(ui.has(role(Role::MenuItemRadio).label("Line")));
  ui.key(NamedKey::Escape);
  assert!(!ui.has(role(Role::MenuItemRadio).label("Line")));
}

#[component]
fn Narrow() -> View {
  let commands = use_state(commands).get();
  COMMANDS.set(Some(commands));
  ui! {
    Toolbar {
      overflow: true,
      CommandButton { commands: commands, id: "view.grid" }
      CommandButton { commands: commands, id: "view.snap" }
      CommandButton { commands: commands, id: "tool.select" }
      CommandButton { commands: commands, id: "tool.pen" }
      CommandButton { commands: commands, id: "edit.clear" }
    }
  }
}

#[test]
fn a_narrow_toolbar_puts_the_rest_behind_a_more_button() {
  let mut ui = TestUi::with(
    ui! { Narrow {} },
    Options {
      size: (260.0, 300.0),
      ..Options::default()
    },
  );
  let more = role(Role::Button).label("More tools");
  assert!(ui.has(more.clone()), "{}", ui.describe());
  let shown = ui.find_all(role(Role::Button)).len();
  assert!(shown < 6, "only some fit: {}", ui.describe());
  assert!(!ui.has(role(Role::Button).label("Clear")));
  ui.click(more.clone());
  // Each can still be used from there, and shows its state.
  ui.click(role(Role::Button).label("Clear"));
  let grid_hidden = !ui.has(role(Role::Button).label("Grid"));
  if grid_hidden {
    ui.click(more.clone());
  }
  ui.click(role(Role::Button).label("Snap to grid"));
  assert_eq!(commands_now().is_on("view.snap"), Some(true));

  // Wide enough, the More button goes.
  ui.resize(900.0, 300.0);
  assert!(!ui.has(more), "{}", ui.describe());
  assert!(ui.has(role(Role::Button).label("Clear")));
}
