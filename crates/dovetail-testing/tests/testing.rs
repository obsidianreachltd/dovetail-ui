//! M20's criteria 1 to 4, and what the package does, with no window or
//! GPU.

use std::time::Duration;

use dovetail::controls::Tooltip;
use dovetail::prelude::*;
use dovetail_core::{NamedKey, Role, Signal};
use dovetail_testing::{TestUi, label, role, text};

#[component]
fn Counter() -> View {
  let count = use_state(|| 0);
  ui! {
    Column {
      Text { value: count.map(|n| format!("Count: {n}")) }
      Button { text: "Increment", on_click: move |_| count.update(|n| *n += 1) }
      Button { text: "Reset", on_click: move |_| count.set(0) }
    }
  }
}

/// Criterion 1: find a button by its label, click it, and check the text
/// that changes.
#[test]
fn a_click_on_a_button_found_by_its_label_changes_the_text() {
  let mut ui = TestUi::new(ui! { Counter {} });
  ui.find(text("Count: 0"));
  ui.click(role(Role::Button).label("Increment"));
  ui.click(label("Increment"));
  ui.find(text("Count: 2"));
  ui.click(label("Reset"));
  assert_eq!(
    ui.find(role(Role::Label)).text(),
    Some("Count: 0"),
    "the count went back to 0"
  );
}

/// Criterion 2: no match names the search and shows what is there.
#[test]
fn a_search_with_no_match_says_what_the_window_has() {
  let ui = TestUi::new(ui! { Counter {} });
  let e = ui
    .try_find(role(Role::Button).label("Save"))
    .expect_err("there is no Save button");
  let message = e.to_string();
  assert!(
    message.starts_with(r#"no element matches role Button, label "Save"."#),
    "{message}"
  );
  assert!(message.contains(r#"Button "Increment""#), "{message}");
  assert!(message.contains(r#"Label = "Count: 0""#), "{message}");
}

/// Criterion 2: several matches are listed, with where they are.
#[test]
fn a_search_with_several_matches_lists_them() {
  let ui = TestUi::new(ui! { Counter {} });
  let e = ui
    .try_find(role(Role::Button))
    .expect_err("there are two buttons");
  assert_eq!(e.matches.len(), 2);
  let message = e.to_string();
  assert!(
    message.starts_with("2 elements match role Button, but one was wanted:"),
    "{message}"
  );
  assert!(message.contains(r#"Button "Reset" at ("#), "{message}");
  assert_eq!(ui.find_all(role(Role::Button)).len(), 2);
}

#[component]
fn Form() -> View {
  let name = use_state(String::new);
  let saved = use_state(|| 0);
  ui! {
    Column {
      TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
      Text { value: name.map(|n| format!("Hello, {n}")) }
      Button { text: "Save", on_click: move |_| saved.update(|n| *n += 1) }
      Text { value: saved.map(|n| format!("Saved {n} times")) }
    }
  }
}

/// Criterion 3: typing, shortcuts and pasting, with no key sent to the
/// system.
#[test]
fn typing_shortcuts_and_pasting_go_through_the_window() {
  let mut ui = TestUi::new(ui! { Form {} });
  let name = role(Role::TextInput).label("Name");
  ui.click(name.clone());
  assert!(ui.find(name.clone()).is_focused());
  ui.type_text("Ann");
  assert_eq!(ui.find(name.clone()).value(), Some("Ann"));
  ui.find(text("Hello, Ann"));

  ui.paste(" Lee");
  assert_eq!(ui.find(name.clone()).value(), Some("Ann Lee"));

  // Select all and copy, then type over the selection.
  ui.shortcut('a');
  ui.shortcut('c');
  assert_eq!(ui.clipboard().as_deref(), Some("Ann Lee"));
  ui.type_text("Bo");
  ui.find(text("Hello, Bo"));

  // Tab goes on to the button, and Enter presses it.
  ui.key(NamedKey::Tab);
  assert_eq!(
    ui.focused()
      .and_then(|f| f.label().map(str::to_owned)),
    Some("Save".into())
  );
  ui.key(NamedKey::Enter);
  ui.find(text("Saved 1 times"));
}

#[component]
fn Tipped() -> View {
  ui! {
    Column {
      Tooltip {
        text: "Saves the settings",
        Button { text: "Save" }
      }
    }
  }
}

/// Criterion 4: the clock is moved forward to finish a delay, with no
/// waiting.
#[test]
fn moving_the_clock_shows_a_tooltip_without_waiting() {
  let mut ui = TestUi::new(ui! { Tipped {} });
  let start = std::time::Instant::now();
  ui.hover(label("Save"));
  assert!(!ui.has(role(Role::Tooltip)), "not yet");
  // Time stands still, however long the test takes.
  std::thread::sleep(Duration::from_millis(20));
  ui.update();
  assert!(!ui.has(role(Role::Tooltip)), "still not");
  ui.advance(Duration::from_millis(400));
  assert!(!ui.has(role(Role::Tooltip)), "too soon");
  ui.advance(Duration::from_millis(200));
  ui.find(role(Role::Tooltip).label("Saves the settings"));
  ui.leave();
  assert!(!ui.has(role(Role::Tooltip)), "gone when the pointer leaves");
  assert!(
    start.elapsed() < Duration::from_millis(400),
    "the test did not wait for the tooltip"
  );
}

/// The tree can be printed, to see what a test can find.
#[test]
fn the_window_can_be_described() {
  let ui = TestUi::new(ui! { Counter {} });
  let text = ui.describe();
  assert!(text.starts_with("Window \"test\"\n"), "{text}");
  assert!(text.contains("  Button \"Increment\""), "{text}");
}

/// A search inside an element finds only what it holds.
#[test]
fn a_search_can_look_inside_an_element() {
  let ui = TestUi::new(ui! { Counter {} });
  let reset = ui.find(label("Reset"));
  assert!(ui.try_find(role(Role::Button)).is_err());
  let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    ui.find_within(&reset, text("Increment"));
  }))
  .expect_err("Increment is not inside Reset");
  let message = e
    .downcast_ref::<String>()
    .cloned()
    .unwrap_or_default();
  assert!(message.contains(r#"inside Button "Reset""#), "{message}");
}

/// Drag and drop runs with no window: a source starts a drag, and a
/// target takes the drop.
#[test]
fn a_drag_and_drop_runs_with_no_window() {
  use std::cell::RefCell;
  use std::rc::Rc;

  use dovetail::{Element, ElementKind, Size, Style};
  use dovetail_core::{
    DragData, DragEnd, DropHandler, DropKind, PointerHandler, PointerKind,
    start_drag,
  };

  let log = Rc::new(RefCell::new(Vec::<String>::new()));
  let (l1, l2) = (Rc::clone(&log), Rc::clone(&log));
  let sized = |height: f32| Style {
    width: Size::Fixed(100.0),
    height: Size::Fixed(height),
    ..Style::default()
  };
  let source = Element::new(ElementKind::Box)
    .style(sized(40.0))
    .role(Role::Button)
    .label("Card")
    .on_pointer(PointerHandler::new(move |e| {
      if e.kind == PointerKind::Down {
        let l = Rc::clone(&l1);
        start_drag(
          DragData::new("card"),
          Callback::new(move |end: DragEnd| {
            l.borrow_mut().push(format!("ended {end:?}"))
          }),
        );
      }
    }));
  let target = Element::new(ElementKind::Box)
    .style(sized(40.0))
    .role(Role::Group)
    .label("Bin")
    .on_drop(DropHandler::new(move |e| {
      let what = e.data.get::<&str>().copied().unwrap_or("?");
      if e.kind != DropKind::Over {
        l2.borrow_mut()
          .push(format!("{:?} {what}", e.kind));
      }
      true
    }));
  let mut ui = TestUi::new(
    Element::new(ElementKind::Column)
      .children([source.into(), target.into()])
      .into(),
  );
  let from = ui.find(label("Card")).centre();
  let to = ui.find(label("Bin")).centre();
  ui.drag(from, to);
  assert!(!ui.dragging());
  assert_eq!(ui.drag_end(), Some(DragEnd::Dropped));
  assert_eq!(*log.borrow(), ["Drop card", "ended Dropped"]);

  // Escape gives a drag up, and the target is told it left.
  log.borrow_mut().clear();
  ui.press_at(
    from,
    dovetail_core::PointerButton::Primary,
    Default::default(),
  );
  assert!(ui.dragging());
  ui.move_to(to, Default::default());
  ui.key(NamedKey::Escape);
  assert!(!ui.dragging());
  assert_eq!(*log.borrow(), ["Leave card", "ended Cancelled"]);
}

thread_local! {
  static TIPPED: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn RemovableTip() -> View {
  let shown = use_state(|| true);
  TIPPED.set(Some(shown));
  let tip: View = if shown.get() {
    ui! { Tooltip { text: "Saves the settings", Button { text: "Save" } } }
  } else {
    View::text("Gone")
  };
  ui! { Column { { tip } } }
}

/// B0009: a tooltip removed while its wait runs is not stepped on when the
/// wait ends.
#[test]
fn a_tooltip_removed_while_it_waits_stops_quietly() {
  let mut ui = TestUi::new(ui! { RemovableTip {} });
  ui.hover(label("Save"));
  TIPPED.with(|t| t.get().expect("state").set(false));
  ui.update();
  ui.advance(Duration::from_millis(600));
  ui.find(text("Gone"));
  assert!(!ui.has(role(Role::Tooltip)));
}
