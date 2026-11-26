//! M04 "Done when" item 4: Button, Toggle and Text work. These run the
//! controls with no window or GPU, and click them through the window's own
//! input (`TestUi`, M20).

extern crate dovetail_core as dovetail;

use std::time::Duration;

use dovetail::{Color, Role, Signal, View, use_state};
use dovetail_controls::{Button, Column, Text, Toggle};
use dovetail_layout::{Content, LayoutBox};
use dovetail_macros::{component, ui};
use dovetail_testing::{Options, TestUi, role, text};

#[component]
fn Panel() -> View {
  let clicks = use_state(|| 0u32);
  let lights = use_state(|| false);
  ui! {
    Column {
      Text { value: format!("Clicks: {}", clicks.get()), size: Some(20.0), color: Some(Color::hex(0x4c4f69)) }
      Button { text: "Add one", on_click: move |_| clicks.update(|c| *c += 1) }
      Toggle { on: lights.get(), on_change: move |on| lights.set(on), label: "Lights" }
      Text { value: if lights.get() { "on" } else { "off" } }
    }
  }
}

fn open() -> TestUi {
  TestUi::with(
    ui! { Panel {} },
    Options {
      size: (400.0, 300.0),
      ..Options::default()
    },
  )
}

/// The first box `pick` chooses.
fn find_box(
  ui: &TestUi,
  pick: impl Fn(&LayoutBox) -> bool,
) -> LayoutBox {
  ui.layout()
    .boxes()
    .iter()
    .find(|b| pick(b))
    .cloned()
    .expect("the box")
}

/// The toggle's track: the box 36 pixels wide.
fn track(ui: &TestUi) -> LayoutBox {
  find_box(ui, |b| b.rect.width == 36.0)
}

#[test]
fn text_shows_its_value_with_its_size() {
  let ui = open();
  let texts: Vec<String> = ui
    .layout()
    .boxes()
    .iter()
    .filter_map(|b| match &b.content {
      Content::Text { text, .. } => Some(text.to_string()),
      _ => None,
    })
    .collect();
  assert_eq!(texts, ["Clicks: 0", "Add one", "Lights", "off"]);
  let clicks = find_box(
    &ui,
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == "Clicks: 0"),
  );
  let Content::Text { font_size, color, .. } = &clicks.content else {
    unreachable!()
  };
  assert_eq!((*font_size, *color), (20.0, Color::hex(0x4c4f69)));
}

#[test]
fn button_calls_on_click_when_clicked() {
  let mut ui = open();
  let add = role(Role::Button).label("Add one");
  ui.click(add.clone());
  ui.click(add);
  ui.find(text("Clicks: 2"));
}

#[test]
fn toggle_reports_the_new_value_and_shows_it() {
  let mut ui = open();
  let lights = role(Role::Switch).label("Lights");
  let colour = |ui: &TestUi| match &track(ui).content {
    Content::Element(e) => e.style.background,
    _ => unreachable!(),
  };
  let off_colour = colour(&ui);
  assert_eq!(ui.find(lights.clone()).is_on(), Some(false));
  let r = track(&ui).rect;
  ui.click_at((r.x + 5.0, r.y + 5.0));
  assert_eq!(ui.find(lights.clone()).is_on(), Some(true));
  ui.find(text("on"));
  // The colour fades (M18).
  ui.advance(Duration::from_millis(500));
  assert_ne!(colour(&ui), off_colour, "the track changes colour");
  // The label is part of the toggle, so clicking anywhere on it toggles.
  ui.click(lights.clone());
  assert_eq!(ui.find(lights).is_on(), Some(false));
  ui.find(text("off"));
}

#[test]
fn toggle_knob_moves_to_the_end_when_on() {
  let mut ui = open();
  let knob_x = |ui: &TestUi| find_box(ui, |b| b.rect.width == 16.0).rect.x;
  let off = knob_x(&ui);
  ui.click(role(Role::Switch).label("Lights"));
  // The knob slides (M18).
  ui.advance(Duration::from_millis(500));
  assert_eq!(
    knob_x(&ui) - off,
    16.0,
    "36 wide, 2 padding each side, 16 knob: moves by 16"
  );
}

#[test]
fn signals_are_shared_with_the_controls_through_closures() {
  // A compile-time check: `Signal` is `Copy`, so one handle can be read in
  // the view and moved into a callback in the same `ui!`.
  fn assert_copy<T: Copy>() {}
  assert_copy::<Signal<u32>>();
}
