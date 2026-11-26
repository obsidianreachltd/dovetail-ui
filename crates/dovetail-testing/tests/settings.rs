//! M18 step 4: the window follows the system's settings for double-clicks
//! and the caret blink while it runs, with no restart.

use std::cell::RefCell;
use std::time::Duration;

use dovetail::prelude::*;
use dovetail_core::{
  Element, ElementKind, PointerEvent, PointerHandler, PointerKind, Size, Style,
  SystemSettings, set_system_settings,
};
use dovetail_testing::{TestUi, label};

thread_local! {
  /// The click count of each press on the pad.
  static PRESSES: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

fn presses() -> Vec<u32> {
  PRESSES.with_borrow_mut(std::mem::take)
}

/// A box that records the click count of each press.
fn pad() -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(100.0),
      height: Size::Fixed(100.0),
      ..Style::default()
    })
    .on_pointer(PointerHandler::new(|e: &PointerEvent| {
      if e.kind == PointerKind::Down {
        PRESSES.with_borrow_mut(|p| p.push(e.clicks));
      }
    }))
    .into()
}

/// Two clicks at `a` and `b`, `between` apart.
fn two_clicks(
  ui: &mut TestUi,
  a: (f32, f32),
  b: (f32, f32),
  between: Duration,
) -> Vec<u32> {
  ui.click_at(a);
  ui.advance(between);
  ui.click_at(b);
  // Let the next pair start afresh.
  ui.advance(Duration::from_secs(5));
  presses()
}

#[test]
fn double_clicks_follow_the_system_settings() {
  let mut ui = TestUi::new(pad());
  presses();
  let wait = Duration::from_millis(400);
  let (a, b) = ((20.0, 20.0), (23.0, 20.0));

  // The usual 500 ms and 4 pixels: a double-click.
  assert_eq!(two_clicks(&mut ui, a, b, wait), [1, 2]);

  // A shorter time: the same clicks are two single ones.
  set_system_settings(SystemSettings {
    double_click_time: Duration::from_millis(300),
    ..SystemSettings::default()
  });
  assert_eq!(two_clicks(&mut ui, a, b, wait), [1, 1]);

  // A smaller distance, as Windows gives: 3 pixels apart is too far.
  set_system_settings(SystemSettings {
    double_click_distance: 2.0,
    ..SystemSettings::default()
  });
  assert_eq!(two_clicks(&mut ui, a, b, wait), [1, 1]);
  assert_eq!(two_clicks(&mut ui, a, a, wait), [1, 2]);
}

#[component]
fn Field() -> View {
  let name = use_state(String::new);
  ui! {
    TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
  }
}

#[test]
fn the_caret_blink_follows_the_system_settings() {
  let mut ui = TestUi::new(ui! { Field {} });
  ui.focus(label("Name"));
  let blink = |ui: &mut TestUi| ui.session().caret_blink();
  assert_eq!(blink(&mut ui), Some(true), "it blinks, and is on");
  ui.advance(Duration::from_millis(530));
  assert_eq!(blink(&mut ui), Some(false), "off after 530 ms");

  // Blinking off: the caret stays on.
  set_system_settings(SystemSettings {
    caret_blink: None,
    ..SystemSettings::default()
  });
  ui.update();
  assert_eq!(blink(&mut ui), None);
  assert!(ui.session().caret_visible());

  // A slower blink starts at once.
  set_system_settings(SystemSettings {
    caret_blink: Some(Duration::from_millis(1000)),
    ..SystemSettings::default()
  });
  ui.update();
  assert_eq!(blink(&mut ui), Some(true));
  ui.advance(Duration::from_millis(600));
  assert_eq!(blink(&mut ui), Some(true), "still on after 600 ms");
  ui.advance(Duration::from_millis(400));
  assert_eq!(blink(&mut ui), Some(false), "off after 1000 ms");
}
