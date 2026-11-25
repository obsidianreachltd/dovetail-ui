//! M08 example: a list editor whose commands are defined once and used
//! from the menus, the toolbar, keyboard shortcuts and command search, with
//! undo and redo, and shortcuts that can be changed.
//!
//! - `cargo run -p example-commands` opens it. Try Ctrl+I (Command+I on
//!   macOS) to add an item, Ctrl+Z to undo, F10 for the menus, and
//!   Ctrl+Shift+P to search for a command.
//! - `cargo run -p example-commands -- --self-test` runs one command from
//!   each place, undoes and redoes, locks the list, and changes a shortcut,
//!   by itself.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::commands::{
  CommandButton, CommandScope, CommandSearch, Commands, History, Menu, MenuBar,
  Shortcut, Toolbar,
};
use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, Modifiers, NamedKey, WindowId,
  WindowOptions,
};
use dovetail::layout::{Content, LayoutBox};
use dovetail::prelude::*;
use dovetail::{Edges, Element, ElementKind, Style};

const MUTED: Color = Color::hex(0x6c6f85);

#[component]
fn Editor() -> View {
  let items = use_state(Vec::<String>::new);
  let added = use_state(|| 0u32);
  let locked = use_state(|| false);
  let history = use_state(History::new).get();
  let commands = use_state(move || {
    let add = move || {
      let n = added.get() + 1;
      added.set(n);
      let item = format!("Item {n}");
      history.apply(
        format!("Add {item}"),
        move || items.update(|l| l.push(item.clone())),
        move || {
          items.update(|l| {
            l.pop();
          })
        },
      );
    };
    let remove = move || {
      let Some(last) = items.get().last().cloned() else {
        return;
      };
      history.apply(
        format!("Remove {last}"),
        move || {
          items.update(|l| {
            l.pop();
          })
        },
        move || items.update(|l| l.push(last.clone())),
      );
    };
    let clear = move || {
      let before = items.get();
      history.apply(
        "Clear the list",
        move || items.set(Vec::new()),
        move || items.set(before.clone()),
      );
    };
    let commands = history.add_commands(Commands::new());
    commands
      .define("list.add", "Add item", Some("Mod+I"), add)
      .enabled_when("list.add", move || !locked.get())
      .define(
        "list.remove",
        "Remove last item",
        Some("Mod+Backspace"),
        remove,
      )
      .enabled_when("list.remove", move || {
        !locked.get() && !items.get().is_empty()
      })
      .define(
        "list.clear",
        "Clear the list",
        Some("Mod+Shift+Backspace"),
        clear,
      )
      .enabled_when("list.clear", move || {
        !locked.get() && !items.get().is_empty()
      })
      .define(
        "list.lock",
        "Lock or unlock the list",
        Some("Mod+L"),
        move || locked.update(|l| *l = !*l),
      )
  })
  .get();

  // Changing a shortcut.
  let shortcut_text = use_state(String::new);
  let message = use_state(String::new);
  let set_shortcut = move |_| {
    let text = shortcut_text.get();
    let result = match Shortcut::parse(&text) {
      Err(e) => format!("\"{text}\": {e}."),
      Ok(s) => match commands.set_shortcut("list.add", Some(s.clone())) {
        Ok(()) => format!("Add item is now {s}."),
        Err(conflict) => format!("{conflict}, so it was not changed."),
      },
    };
    message.set(result);
  };

  let menus = vec![
    Menu::new("List")
      .command("list.add")
      .command("list.remove")
      .command("list.clear")
      .separator()
      .command("list.lock"),
    Menu::new("Edit")
      .command("edit.undo")
      .command("edit.redo")
      .separator()
      .command("commands.search"),
  ];
  let list: Vec<View> = items.get().into_iter().map(View::text).collect();
  let count = format!("Items: {}", items.get().len());
  let lock = if locked.get() {
    "The list is locked."
  } else {
    ""
  };

  ui! {
    CommandScope {
      commands: commands,
      Column {
        gap: 0.0,
        MenuBar { commands: commands, menus: menus }
        Toolbar {
          CommandButton { commands: commands, id: "list.add" }
          CommandButton { commands: commands, id: "list.remove" }
          CommandButton { commands: commands, id: "list.clear" }
          CommandButton { commands: commands, id: "edit.undo" }
          CommandButton { commands: commands, id: "edit.redo" }
          Toggle { on: locked.get(), on_change: move |on| locked.set(on), label: "Locked" }
        }
        { padded(ui! {
          Column {
            gap: 10.0,
            Text { value: count }
            Text { value: lock, color: Some(MUTED) }
            Column { gap: 4.0, { View::fragment(list) } }
            Row {
              Text { value: "Shortcut for Add item:" }
              TextField {
                value: shortcut_text.get(),
                on_change: move |v| shortcut_text.set(v),
                width: 160.0,
                label: "Shortcut for Add item",
              }
              Button { text: "Set", on_click: set_shortcut }
            }
            Text { value: message.get(), color: Some(MUTED) }
          }
        }) }
        CommandSearch { commands: commands }
      }
    }
  }
}

fn padded(view: View) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(16.0),
      ..Style::default()
    })
    .children([view])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test();
  }
  match dovetail::run("Commands", ui! { Editor {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("commands: {e}");
      ExitCode::FAILURE
    },
  }
}

// --- Self-test ---------------------------------------------------------------

/// What each step waits to see before doing the next thing.
const STEPS: &[&str] = &[
  "Items: 0",
  "Items: 1",                // after the shortcut
  "Items: 2",                // after the toolbar button
  "Remove last item",        // the List menu is open
  "Items: 3",                // after choosing from the menu
  "Lock or unlock the list", // command search is open
  "Items: 4",                // after command search
  "Items: 2",                // after undoing twice
  "Items: 3",                // after redoing once
  "The list is locked.",
  "Ctrl+Z is already", // giving Add item the Undo shortcut was refused
  "Add item is now",
  "Items: 4", // after the new shortcut
];

struct SelfTest {
  window: Option<WindowId>,
  step: usize,
}

fn find_text<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().rev().find(|b| {
    matches!(&b.content, Content::Text { text, .. } if text.contains(wanted))
  })
}

/// The last box drawn whose text is exactly `wanted`.
fn find_exact<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().rev().find(
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == wanted),
  )
}

fn centre(b: &LayoutBox) -> (f32, f32) {
  (
    b.rect.x + b.rect.width / 2.0,
    b.rect.y + b.rect.height / 2.0,
  )
}

/// A shortcut press: `c` with the main modifier, and Shift if asked.
fn shortcut(
  c: char,
  shift: bool,
) -> KeyEvent {
  let mut m = Modifiers { shift, ..Modifiers::default() };
  if cfg!(target_os = "macos") {
    m.meta = true;
  } else {
    m.control = true;
  }
  let mut e = KeyEvent::character(c).with(m);
  e.text = None;
  e
}

impl Application for SelfTest {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Commands self-test");
    match cx.open_ui_window(options, ui! { Editor {} }) {
      Ok(id) => self.window = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(20));
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: WindowId,
    _: &mut Frame,
  ) {
  }

  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    let Some(expected) = STEPS.get(self.step) else {
      return;
    };
    let boxes = cx.ui_boxes(window).to_vec();
    if find_text(&boxes, expected).is_none() {
      return;
    }
    println!("PASS  the window shows \"{expected}\"");
    let click =
      |cx: &mut Context<'_>, text: &str| match find_exact(&boxes, text) {
        Some(b) => {
          let (x, y) = centre(b);
          cx.click(window, x, y);
          true
        },
        None => {
          println!("FAIL  no \"{text}\" to click");
          false
        },
      };
    let field = boxes
      .iter()
      .find(|b| {
        matches!(&b.content, Content::Element(e)
          if e.label.as_deref() == Some("Shortcut for Add item"))
      })
      .map(centre);
    let typing = |cx: &mut Context<'_>, text: &str| {
      text
        .chars()
        .all(|c| cx.key(window, &KeyEvent::character(c)))
    };
    self.step += 1;
    let ok = match self.step {
      // A shortcut, with nothing focused.
      1 => cx.key(window, &shortcut('i', false)),
      // The toolbar button.
      2 => click(cx, "Add item"),
      // The menu: open List...
      3 => click(cx, "List"),
      // ...and choose Add item from it: the last one drawn, as the open
      // menu is a layer above the toolbar.
      4 => click(cx, "Add item"),
      // Command search: open it...
      5 => cx.key(window, &shortcut('p', true)),
      // ...then type into its box, which has focus, and press Enter.
      6 => {
        typing(cx, "add") && cx.key(window, &KeyEvent::named(NamedKey::Enter))
      },
      // Undo twice with the shortcut.
      7 => {
        cx.key(window, &shortcut('z', false))
          && cx.key(window, &shortcut('z', false))
      },
      // Redo once, from the toolbar.
      8 => click(cx, "Redo"),
      // Lock the list from its switch.
      9 => click(cx, "Locked"),
      // Locked: Add item's button is disabled and its shortcut does nothing.
      10 => {
        let button_disabled = boxes.iter().any(|b| {
          matches!(&b.content, Content::Element(e)
          if e.role == dovetail::Role::Button
            && e.disabled
            && b.children.iter().any(|&c| matches!(
              &boxes[c].content,
              Content::Text { text, .. } if &**text == "Add item"
            )))
        });
        let used = cx.key(window, &shortcut('i', false));
        if used || !button_disabled {
          println!(
            "FAIL  while locked: shortcut used {used}, button disabled {button_disabled}"
          );
          false
        } else {
          println!(
            "PASS  while locked, Add item is disabled and its shortcut does nothing"
          );
          // Unlock with its own shortcut, then give Add item the Undo
          // shortcut, which is refused.
          let (x, y) = field.unwrap_or_default();
          cx.key(window, &shortcut('l', false))
            && {
              cx.click(window, x, y);
              typing(cx, "Mod+Z")
            }
            && click(cx, "Set")
        }
      },
      // Then a free one.
      11 => {
        let (x, y) = field.unwrap_or_default();
        cx.click(window, x, y);
        cx.key(window, &shortcut('a', false))
          && typing(cx, "Mod+J")
          && click(cx, "Set")
      },
      // The new shortcut adds an item; focus is on the Set button.
      12 => cx.key(window, &shortcut('j', false)),
      _ => {
        PASSED.store(true, Ordering::Relaxed);
        println!(
          "PASS  one command ran from a shortcut, the toolbar, a menu and command search; undo, redo, disabling and changing shortcuts work"
        );
        cx.close_window(window);
        true
      },
    };
    if !ok {
      println!("FAIL  at step {}", self.step);
      cx.exit();
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS.get(self.step).copied().unwrap_or("the end");
    println!("FAIL  timed out waiting for \"{waiting}\"");
    cx.exit();
  }
}

static PASSED: AtomicBool = AtomicBool::new(false);

fn self_test() -> ExitCode {
  let result = dovetail::desktop::run(SelfTest { window: None, step: 0 });
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("commands: {e}");
      ExitCode::FAILURE
    },
  }
}
