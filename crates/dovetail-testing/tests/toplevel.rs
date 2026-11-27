//! M19: windows in the component tree, checked with `TestUi`'s headless
//! window host (criterion 3).

use std::cell::Cell;

use dovetail::prelude::*;
use dovetail::{
  Role, Window, WindowPlace, WindowState, provide_context, use_context,
};
use dovetail_testing::{TestUi, role, text};

thread_local! {
  static SHOW: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
  static TITLE: Cell<Option<Signal<String>>> = const { Cell::new(None) };
  static STATE: Cell<Option<Signal<WindowState>>> = const { Cell::new(None) };
  static CLOSES: Cell<u32> = const { Cell::new(0) };
}

fn show() -> Signal<bool> {
  SHOW.get().expect("the app has built")
}

/// A main window with a button, and a settings window shown while `show`
/// is true. Both show one count, kept in the main window's component.
#[component]
fn App() -> View {
  let open = use_state(|| true);
  let count = use_state(|| 0);
  let title = use_state(|| "Settings".to_string());
  SHOW.set(Some(open));
  TITLE.set(Some(title));
  ui! {
    Column {
      Text { value: count.map(|n| format!("Main count: {n}")) }
      if open.get() {
        Window {
          title: title.get(),
          size: (300.0, 200.0),
          min_size: (200.0, 100.0),
          on_close: move |()| {
            CLOSES.set(CLOSES.get() + 1);
            open.set(false);
          },
          Column {
            Text { value: count.map(|n| format!("Window count: {n}")) }
            Button { text: "Add", on_click: move |_| count.update(|n| *n += 1) }
          }
        }
      }
    }
  }
}

#[test]
fn a_window_opens_when_shown_and_closes_when_it_goes() {
  let mut ui = TestUi::new(ui! { App {} });
  let windows = ui.windows();
  assert_eq!(windows.len(), 1, "the settings window is open");
  assert_eq!(windows[0].title, "Settings");
  assert_eq!(windows[0].place.size, (300.0, 200.0));
  assert_eq!(windows[0].limits.min_size, Some((200.0, 100.0)));
  // Its content is in its own window, not the main one.
  assert!(!ui.has(text("Window count: 0")));
  ui.switch_to(ui.window("Settings"));
  ui.find(text("Window count: 0"));
  ui.switch_to_main();

  show().set(false);
  ui.update();
  assert!(ui.windows().is_empty(), "it closed when it went");
  show().set(true);
  ui.update();
  assert_eq!(ui.windows().len(), 1, "and opened again when shown");
}

#[test]
fn a_window_shares_state_with_the_component_that_declares_it() {
  let mut ui = TestUi::new(ui! { App {} });
  ui.switch_to(ui.window("Settings"));
  ui.click(role(Role::Button).label("Add"));
  ui.find(text("Window count: 1"));
  ui.switch_to_main();
  ui.find(text("Main count: 1"));
}

#[test]
fn a_window_title_follows_its_property() {
  let mut ui = TestUi::new(ui! { App {} });
  let w = ui.window("Settings");
  TITLE.get().unwrap().set("Preferences".into());
  ui.update();
  assert_eq!(ui.window("Preferences"), w, "the same window, renamed");
}

#[test]
fn closing_a_window_asks_its_component() {
  CLOSES.set(0);
  let mut ui = TestUi::new(ui! { App {} });
  let w = ui.window("Settings");
  ui.switch_to(w);
  ui.close_window(w);
  assert_eq!(CLOSES.get(), 1);
  assert!(ui.windows().is_empty(), "the component let it go");
  assert_eq!(ui.front_window(), None, "the main window is in front again");
  ui.find(text("Main count: 0"));
}

/// A window that keeps itself open when asked to close.
#[component]
fn Stubborn() -> View {
  ui! { Window { title: "Stays", on_close: move |()| {}, "Here" } }
}

/// A window with no `on_close`.
#[component]
fn Plain() -> View {
  ui! { Window { title: "Goes", "Here" } }
}

#[test]
fn a_window_without_on_close_closes_itself() {
  let mut ui = TestUi::new(ui! { Column { Stubborn {} Plain {} } });
  assert_eq!(ui.windows().len(), 2);
  let stays = ui.window("Stays");
  ui.close_window(stays);
  ui.close_window(ui.window("Goes"));
  let left: Vec<String> = ui
    .windows()
    .into_iter()
    .map(|w| w.title)
    .collect();
  assert_eq!(left, ["Stays"]);
}

/// A window whose state is kept in a signal, and told when the user
/// changes it.
#[component]
fn Stateful() -> View {
  let state = use_state(|| WindowState::Normal);
  STATE.set(Some(state));
  ui! {
    Window {
      title: "Stateful",
      state: state.get(),
      on_state: move |s| state.set(s),
      Text { value: state.map(|s| format!("State: {s:?}")) }
    }
  }
}

#[test]
fn a_window_state_follows_its_property_and_the_user() {
  let mut ui = TestUi::new(ui! { Stateful {} });
  let w = ui.window("Stateful");
  let state = STATE.get().unwrap();
  state.set(WindowState::Maximised);
  ui.update();
  assert_eq!(ui.windows()[0].state, WindowState::Maximised);

  ui.set_window_state(w, WindowState::FullScreen);
  assert_eq!(
    state.get(),
    WindowState::FullScreen,
    "the component was told"
  );
  ui.switch_to(w);
  ui.find(text("State: FullScreen"));
}

#[test]
fn moving_a_window_tells_its_component_and_keeps_its_limits() {
  thread_local! {
    static PLACE: Cell<Option<WindowPlace>> = const { Cell::new(None) };
  }
  #[component]
  fn Placed() -> View {
    ui! {
      Window {
        title: "Placed",
        max_size: (400.0, 300.0),
        on_place: move |p| PLACE.set(Some(p)),
        "Here"
      }
    }
  }
  let mut ui = TestUi::new(ui! { Placed {} });
  let w = ui.window("Placed");
  ui.move_window(
    w,
    WindowPlace {
      position: Some((50.0, 60.0)),
      size: (900.0, 900.0),
    },
  );
  assert_eq!(
    PLACE.get(),
    Some(WindowPlace {
      position: Some((50.0, 60.0)),
      size: (400.0, 300.0)
    })
  );
}

#[derive(Clone, PartialEq)]
struct Accent(&'static str);

#[component]
fn Provider() -> View {
  provide_context(Accent("teal"));
  ui! { Window { title: "Themed", Reader {} } }
}

#[component]
fn Reader() -> View {
  let accent = use_context::<Accent>().map_or("none", |a| a.0);
  ui! { Text { value: format!("Accent: {accent}") } }
}

#[test]
fn a_window_sees_what_its_component_is_given() {
  let mut ui = TestUi::new(ui! { Provider {} });
  ui.switch_to(ui.window("Themed"));
  ui.find(text("Accent: teal"));
}

#[test]
fn a_window_in_a_plain_tree_opens_nothing() {
  assert!(dovetail::window_host().is_none(), "no TestUi is running");
  let mut tree = Tree::new();
  tree.mount(ui! { Plain {} });
  tree.update();
  assert!(dovetail::window_host().is_none());
}

thread_local! {
  static ASKED: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
}

/// A main window with a button that opens a modal settings window.
#[component]
fn WithModal() -> View {
  let asked = use_state(|| false);
  let presses = use_state(|| 0);
  ASKED.set(Some(asked));
  ui! {
    Column {
      Button { text: "Settings", on_click: move |_| asked.set(true) }
      Button { text: "Count", on_click: move |_| presses.update(|n| *n += 1) }
      Text { value: presses.map(|n| format!("Presses: {n}")) }
      if asked.get() {
        Window {
          title: "Settings",
          modal: true,
          on_close: move |()| asked.set(false),
          Button { text: "Done", on_click: move |_| asked.set(false) }
        }
      }
    }
  }
}

#[test]
fn a_modal_window_blocks_its_owner_until_it_closes() {
  let mut ui = TestUi::new(ui! { WithModal {} });
  ui.click(role(Role::Button).label("Settings"));
  let modal = ui.window("Settings");
  assert_eq!(ui.front_window(), Some(modal), "it comes to the front");
  assert_eq!(ui.blocked_by(None), Some(modal));

  // A click on the owner is refused, and brings the modal window back.
  ui.switch_to_main();
  ui.click(role(Role::Button).label("Count"));
  assert_eq!(ui.front_window(), Some(modal));
  ui.switch_to_main();
  ui.find(text("Presses: 0"));
  // So are keys.
  ui.focus(role(Role::Button).label("Count"));
  assert!(!ui.key(dovetail::NamedKey::Enter), "the key was refused");
  ui.find(text("Presses: 0"));

  ui.switch_to(modal);
  ui.click(role(Role::Button).label("Done"));
  assert!(ui.windows().is_empty());
  assert_eq!(ui.blocked_by(None), None, "the owner is free");
  ui.click(role(Role::Button).label("Count"));
  ui.find(text("Presses: 1"));
}

#[test]
fn the_list_of_windows_follows_what_is_open() {
  #[component]
  fn Lister() -> View {
    let names: Vec<String> = dovetail::app_windows()
      .into_iter()
      .map(|w| w.title)
      .collect();
    ui! { Text { value: format!("Open: {}", names.join(", ")) } }
  }
  let mut ui = TestUi::new(ui! { Column { Lister {} App {} } });
  ui.find(text("Open: Settings"));
  show().set(false);
  ui.update();
  ui.find(text("Open: "));
}
