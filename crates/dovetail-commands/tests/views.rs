//! M08 "Done when" items 1 and 2: one command, run from a shortcut, a
//! toolbar button, a menu and command search; and disabled everywhere at
//! once. Through the component tree, layout and key dispatch, with no
//! window, the way the desktop crate drives them.

extern crate dovetail_core as dovetail;

use std::cell::Cell;

use dovetail::{
  KeyEvent, Modifiers, NamedKey, Role, Signal, Tree, View, use_state,
};
use dovetail_commands::{
  CommandButton, CommandScope, CommandSearch, Commands, Menu, MenuBar, Toolbar,
};
use dovetail_controls::{Column, Text};
use dovetail_layout::{BoxKey, Content, FixedPitch, Layout};
use dovetail_macros::{component, ui};

thread_local! {
  static STATE: Cell<Option<(Commands, Signal<u32>)>> = const { Cell::new(None) };
}

#[component]
fn App() -> View {
  let count = use_state(|| 0u32);
  let commands = use_state(move || {
    Commands::new().define("list.add", "Add item", Some("Mod+K"), move || {
      count.update(|c| *c += 1)
    })
  })
  .get();
  STATE.set(Some((commands, count)));
  let menus = vec![Menu::new("List").command("list.add").separator()];
  ui! {
    CommandScope {
      commands: commands,
      Column {
        MenuBar { commands: commands, menus: menus }
        Toolbar { CommandButton { commands: commands, id: "list.add" } }
        Text { value: format!("Items: {}", count.get()) }
        CommandSearch { commands: commands }
      }
    }
  }
}

struct Window {
  tree: Tree,
  layout: Layout,
  focus: Option<BoxKey>,
  autofocused: Vec<BoxKey>,
}

impl Window {
  fn open() -> Window {
    let mut tree = Tree::new();
    tree.mount(ui! { App {} });
    let mut w = Window {
      tree,
      layout: Layout::new(),
      focus: None,
      autofocused: Vec::new(),
    };
    w.refresh();
    w
  }

  /// Builds and lays out what changed, then gives focus to an element that
  /// asks for it on appearing, as the desktop crate does.
  fn refresh(&mut self) {
    for _ in 0..3 {
      self.tree.update();
    }
    self
      .layout
      .update(&self.tree, 800.0, 600.0, &FixedPitch::default());
    let wanting: Vec<BoxKey> = self
      .layout
      .boxes()
      .iter()
      .filter(|b| matches!(&b.content, Content::Element(e) if e.autofocus))
      .map(|b| b.key.clone())
      .collect();
    if let Some(new) = wanting
      .iter()
      .find(|k| !self.autofocused.contains(k))
    {
      self.focus = Some(new.clone());
    }
    self.autofocused = wanting;
    if self
      .focus
      .as_ref()
      .is_some_and(|f| !self.layout.can_focus(f))
    {
      self.focus = None;
    }
  }

  fn count(&self) -> u32 {
    STATE.get().unwrap().1.get()
  }

  fn commands(&self) -> Commands {
    STATE.get().unwrap().0
  }

  fn key(
    &mut self,
    event: KeyEvent,
  ) -> bool {
    let used = self
      .layout
      .dispatch_key(self.focus.as_ref(), &event);
    self.refresh();
    used
  }

  fn shortcut(
    &mut self,
    c: char,
    shift: bool,
  ) -> bool {
    let mut m = Modifiers { shift, ..Modifiers::default() };
    if cfg!(target_os = "macos") {
      m.meta = true;
    } else {
      m.control = true;
    }
    let mut e = KeyEvent::character(c).with(m);
    e.text = None;
    self.key(e)
  }

  /// Clicks the middle of the box showing `text`.
  fn click_text(
    &mut self,
    text: &str,
  ) {
    let r = self
      .layout
      .boxes()
      .iter()
      .rev()
      .find(
        |b| matches!(&b.content, Content::Text { text: t, .. } if &**t == text),
      )
      .unwrap_or_else(|| panic!("no text \"{text}\""))
      .rect;
    if let Some(press) = self
      .layout
      .hit_test(r.x + r.width / 2.0, r.y + r.height / 2.0)
    {
      press.call(());
    }
    self.refresh();
  }

  fn shows(
    &self,
    text: &str,
  ) -> bool {
    self.layout.boxes().iter().any(
      |b| matches!(&b.content, Content::Text { text: t, .. } if &**t == text),
    )
  }

  /// The element with this role and name.
  fn element(
    &self,
    role: Role,
    name: &str,
  ) -> Option<std::rc::Rc<dovetail::Element>> {
    self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Element(e)
          if e.role == role && e.label.as_deref() == Some(name) =>
        {
          Some(e.clone())
        },
        _ => None,
      })
  }
}

#[test]
fn one_command_runs_from_every_place_it_appears() {
  let mut w = Window::open();
  assert!(w.shows("Items: 0"));

  // A shortcut, with nothing focused.
  assert!(w.shortcut('k', false));
  assert_eq!(w.count(), 1);

  // The toolbar button, which shows the command's name.
  let button = w.element(Role::Button, "Add item");
  assert!(button.is_none(), "a button is named by its text");
  w.click_text("Add item");
  assert_eq!(w.count(), 2);

  // The menu: open it, then choose the item.
  w.click_text("List");
  assert!(w.element(Role::Menu, "List").is_some(), "the menu is open");
  assert!(
    w.shows("Ctrl+K") || w.shows("⌘K"),
    "with the shortcut shown"
  );
  w.click_text("Add item");
  assert!(w.element(Role::Menu, "List").is_none(), "and closes");
  assert_eq!(w.count(), 3);

  // The menu with the keyboard: F10, then Enter on the first item.
  assert!(w.key(KeyEvent::named(NamedKey::F10)));
  assert!(w.element(Role::Menu, "List").is_some());
  assert!(w.key(KeyEvent::named(NamedKey::Enter)));
  assert_eq!(w.count(), 4);
  assert!(w.element(Role::Menu, "List").is_none());

  // Command search: open, type, Enter.
  assert!(w.shortcut('p', true));
  assert!(
    w.element(Role::Group, "Search commands")
      .is_some()
  );
  assert!(w.focus.is_some(), "the search box has focus");
  for c in "add".chars() {
    assert!(w.key(KeyEvent::character(c)));
  }
  assert!(w.key(KeyEvent::named(NamedKey::Enter)));
  assert_eq!(w.count(), 5);
  assert!(
    w.element(Role::Group, "Search commands")
      .is_none(),
    "and closes"
  );

  // Escape closes command search without running anything.
  w.shortcut('p', true);
  assert!(w.key(KeyEvent::named(NamedKey::Escape)));
  assert!(
    w.element(Role::Group, "Search commands")
      .is_none()
  );
  assert_eq!(w.count(), 5);
}

#[test]
fn a_disabled_command_is_disabled_everywhere() {
  let mut w = Window::open();
  w.commands().set_enabled("list.add", false);
  w.refresh();

  // The toolbar button is greyed out and cannot be pressed.
  let button = w
    .layout
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Element(e) if e.role == Role::Button => Some(e.clone()),
      _ => None,
    })
    .unwrap();
  assert!(button.disabled && button.on_press.is_none());
  w.click_text("Add item");

  // The shortcut does nothing.
  assert!(!w.shortcut('k', false));

  // The menu item is greyed out and cannot be chosen.
  w.click_text("List");
  let item = w.element(Role::MenuItem, "Add item").unwrap();
  assert!(item.disabled && item.on_press.is_none());
  assert!(
    w.key(KeyEvent::named(NamedKey::Enter)),
    "Enter has nothing to choose"
  );
  w.key(KeyEvent::named(NamedKey::Escape));

  // Command search lists it, greyed out, and Enter does not run it.
  w.shortcut('p', true);
  for c in "add".chars() {
    w.key(KeyEvent::character(c));
  }
  let found = w
    .element(Role::ListBoxOption, "Add item")
    .unwrap();
  assert!(found.disabled);
  w.key(KeyEvent::named(NamedKey::Enter));
  assert_eq!(w.count(), 0, "it never ran");

  // Enabled again, it works everywhere again.
  w.key(KeyEvent::named(NamedKey::Escape));
  w.commands().set_enabled("list.add", true);
  w.refresh();
  assert!(w.shortcut('k', false));
  w.click_text("Add item");
  assert_eq!(w.count(), 2);
}
