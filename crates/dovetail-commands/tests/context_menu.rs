//! M15 "Done when" item 4: a right-click opens a context menu of commands,
//! with their shortcuts, and greyed-out items cannot be chosen. Through
//! the component tree, layout and key dispatch, with no window.

extern crate dovetail_core as dovetail;

use std::cell::Cell;

use dovetail::{KeyEvent, NamedKey, Role, Signal, Tree, View, use_state};
use dovetail_commands::{Commands, ContextMenu, MenuItem};
use dovetail_controls::Text;
use dovetail_layout::{BoxKey, Content, FixedPitch, Layout};
use dovetail_macros::{component, ui};

thread_local! {
  static STATE: Cell<Option<(Commands, Signal<u32>)>> = const { Cell::new(None) };
}

#[component]
fn App() -> View {
  let copies = use_state(|| 0u32);
  let commands = use_state(move || {
    let c = Commands::new()
      .define("edit.copy", "Copy", Some("Mod+C"), move || {
        copies.update(|n| *n += 1)
      })
      .define("edit.paste", "Paste", Some("Mod+V"), || {});
    c.set_enabled("edit.paste", false);
    c
  })
  .get();
  STATE.set(Some((commands, copies)));
  let items = vec![
    MenuItem::Command("edit.copy".into()),
    MenuItem::Separator,
    MenuItem::Command("edit.paste".into()),
  ];
  ui! {
    ContextMenu {
      commands: commands,
      items: items,
      Text { value: format!("Copies: {}", copies.get()) }
    }
  }
}

struct Window {
  tree: Tree,
  layout: Layout,
}

impl Window {
  fn open() -> Window {
    let mut tree = Tree::new();
    tree.mount(ui! { App {} });
    let mut w = Window { tree, layout: Layout::new() };
    w.refresh();
    w
  }

  fn refresh(&mut self) {
    self.tree.update();
    self
      .layout
      .update(&self.tree, 800.0, 600.0, &FixedPitch::default());
  }

  fn copies(&self) -> u32 {
    STATE.get().unwrap().1.get()
  }

  fn find(
    &self,
    role: Role,
    label: &str,
  ) -> Option<(BoxKey, dovetail::Element)> {
    self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Element(e)
          if e.role == role
            && (label.is_empty() || e.label.as_deref() == Some(label)) =>
        {
          Some((b.key.clone(), (**e).clone()))
        },
        _ => None,
      })
  }

  /// A right-click at (`x`, `y`), as the window passes it on: to the
  /// nearest element with a context menu handler.
  fn right_click(
    &mut self,
    x: f32,
    y: f32,
  ) {
    let key = self
      .layout
      .context_menu_target_at(x, y)
      .expect("a context menu under the point");
    let on_menu = self
      .layout
      .element(&key)
      .and_then(|e| e.on_context_menu.clone())
      .expect("a handler");
    on_menu.call((x, y));
    self.refresh();
  }

  fn menu_key(
    &mut self,
    key: NamedKey,
  ) -> bool {
    let (menu, _) = self.find(Role::Menu, "").expect("an open menu");
    let used = self
      .layout
      .dispatch_key(Some(&menu), &KeyEvent::named(key));
    self.refresh();
    used
  }

  fn has_text(
    &self,
    text: &str,
  ) -> bool {
    self.layout.boxes().iter().any(
      |b| matches!(&b.content, Content::Text { text: t, .. } if &**t == text),
    )
  }
}

#[test]
fn a_right_click_opens_the_menu_where_the_pointer_is() {
  let mut w = Window::open();
  assert!(w.find(Role::Menu, "").is_none());
  w.right_click(5.0, 5.0);
  let (key, menu) = w.find(Role::Menu, "Context menu").expect("open");
  assert!(menu.autofocus, "it takes focus, for the keyboard");
  let rect = w.layout.rect_of(&key).expect("placed");
  assert_eq!((rect.x, rect.y), (5.0, 5.0));
  assert!(w.find(Role::Separator, "").is_some());
}

#[test]
fn items_show_shortcuts_and_greyed_out_items_cannot_be_chosen() {
  let mut w = Window::open();
  w.right_click(5.0, 5.0);
  let commands = STATE.get().unwrap().0;
  let copy_keys = commands
    .info("edit.copy")
    .and_then(|i| i.shortcut)
    .expect("a shortcut")
    .label();
  assert!(w.has_text(&copy_keys), "the shortcut {copy_keys} shows");
  let (_, paste) = w.find(Role::MenuItem, "Paste").expect("paste");
  assert!(paste.disabled && paste.on_press.is_none());
  // Enter on a greyed-out item does nothing: the keys skip it.
  assert!(w.menu_key(NamedKey::ArrowDown));
  assert!(w.menu_key(NamedKey::ArrowDown));
  assert!(w.menu_key(NamedKey::Enter));
  assert_eq!(w.copies(), 1, "Down twice comes round to Copy again");
  assert!(w.find(Role::Menu, "").is_none(), "choosing closes it");
}

#[test]
fn choosing_with_the_pointer_runs_the_command_and_escape_closes() {
  let mut w = Window::open();
  w.right_click(5.0, 5.0);
  let (_, copy) = w.find(Role::MenuItem, "Copy").expect("copy");
  copy.on_press.expect("pressable").call(());
  w.refresh();
  assert_eq!(w.copies(), 1);
  assert!(w.find(Role::Menu, "").is_none());

  w.right_click(5.0, 5.0);
  assert!(w.menu_key(NamedKey::Escape));
  assert!(w.find(Role::Menu, "").is_none());
  assert_eq!(w.copies(), 1);
}
