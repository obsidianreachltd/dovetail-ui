//! M04 "Done when" item 4: Button, Toggle and Text work. These lay the
//! controls out and press them through hit testing, with no window or GPU.

extern crate dovetail_core as dovetail;

use dovetail::{Color, Signal, Tree, View, use_state};
use dovetail_controls::{Button, Column, Text, Toggle};
use dovetail_layout::{Content, FixedPitch, Layout, LayoutBox};
use dovetail_macros::{component, ui};

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

struct Window {
  tree: Tree,
  layout: Layout,
}

impl Window {
  fn open() -> Self {
    let mut tree = Tree::new();
    tree.mount(ui! { Panel {} });
    let mut window = Self { tree, layout: Layout::new() };
    window.refresh();
    window
  }

  fn refresh(&mut self) {
    self.tree.update();
    self
      .layout
      .update(&self.tree, 400.0, 300.0, &FixedPitch::default());
  }

  fn texts(&self) -> Vec<String> {
    self
      .layout
      .boxes()
      .iter()
      .filter_map(|b| match &b.content {
        Content::Text { text, .. } => Some(text.to_string()),
        _ => None,
      })
      .collect()
  }

  /// The box that contains the text `label`, pressed in its middle.
  fn click_text(
    &mut self,
    label: &str,
  ) {
    let target: &LayoutBox = self
      .layout
      .boxes()
      .iter()
      .find(|b| matches!(&b.content, Content::Text { text, .. } if &**text == label))
      .unwrap_or_else(|| panic!("no text {label:?}"));
    let (x, y) = (
      target.rect.x + target.rect.width / 2.0,
      target.rect.y + target.rect.height / 2.0,
    );
    self
      .layout
      .hit_test(x, y)
      .expect("a press handler under the text")
      .call(());
    self.refresh();
  }

  /// The box of the toggle's track: the first box with a 36-pixel width.
  fn click_toggle_track(&mut self) {
    let track = self
      .layout
      .boxes()
      .iter()
      .find(|b| b.rect.width == 36.0)
      .expect("toggle track");
    let (x, y) = (track.rect.x + 5.0, track.rect.y + 5.0);
    self
      .layout
      .hit_test(x, y)
      .expect("toggle handler")
      .call(());
    self.refresh();
  }
}

#[test]
fn text_shows_its_value_with_its_size() {
  let w = Window::open();
  assert_eq!(w.texts(), ["Clicks: 0", "Add one", "Lights", "off"]);
  let clicks = w.layout.boxes().iter().find(|b| matches!(&b.content, Content::Text { text, .. } if &**text == "Clicks: 0")).unwrap();
  let Content::Text { font_size, color, .. } = &clicks.content else {
    unreachable!()
  };
  assert_eq!((*font_size, *color), (20.0, Color::hex(0x4c4f69)));
}

#[test]
fn button_calls_on_click_when_clicked() {
  let mut w = Window::open();
  w.click_text("Add one");
  w.click_text("Add one");
  assert_eq!(w.texts()[0], "Clicks: 2");
}

#[test]
fn toggle_reports_the_new_value_and_shows_it() {
  let mut w = Window::open();
  let off_colour = track_colour(&w);
  w.click_toggle_track();
  assert_eq!(w.texts()[3], "on");
  assert_ne!(track_colour(&w), off_colour, "the track changes colour");
  // The label is part of the toggle, so clicking it toggles too.
  w.click_text("Lights");
  assert_eq!(w.texts()[3], "off");
}

fn track_colour(w: &Window) -> Color {
  let track = w
    .layout
    .boxes()
    .iter()
    .find(|b| b.rect.width == 36.0)
    .expect("track");
  match &track.content {
    Content::Element(e) => e.style.background,
    _ => unreachable!(),
  }
}

#[test]
fn toggle_knob_moves_to_the_end_when_on() {
  let mut w = Window::open();
  let knob_x = |w: &Window| {
    w.layout
      .boxes()
      .iter()
      .find(|b| b.rect.width == 16.0)
      .expect("knob")
      .rect
      .x
  };
  let off = knob_x(&w);
  w.click_toggle_track();
  assert_eq!(
    knob_x(&w) - off,
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
