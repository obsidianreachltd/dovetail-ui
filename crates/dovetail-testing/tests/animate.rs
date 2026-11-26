//! M18 step 6: animated values, moved by the test's clock.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use dovetail::prelude::*;
use dovetail_core::{
  SystemSettings, frame_requested, now, set_system_settings,
};
use dovetail_testing::{FRAME, TestUi};

thread_local! {
  /// The value shown at each build, and when.
  static SHOWN: RefCell<Vec<(Instant, f32)>> = const { RefCell::new(Vec::new()) };
}

fn shown() -> Vec<(Instant, f32)> {
  SHOWN.with_borrow_mut(std::mem::take)
}

const TIME: Duration = Duration::from_millis(200);

/// Moves a number from 0 to the target it is given.
#[component]
fn Slider(
  target: Signal<f32>,
  curve: Curve,
) -> View {
  let value = use_animated(target.get(), Transition::new(TIME, curve));
  SHOWN.with_borrow_mut(|s| s.push((now(), value)));
  ui! { Text { value: format!("{value:.1}") } }
}

#[component]
fn Holder(curve: Curve) -> View {
  let target = use_state(|| 0.0f32);
  TARGET.with(|t| *t.borrow_mut() = Some(target));
  ui! { Slider { target: target, curve: curve } }
}

thread_local! {
  static TARGET: RefCell<Option<Signal<f32>>> = const { RefCell::new(None) };
}

fn target() -> Signal<f32> {
  TARGET.with_borrow(|t| t.expect("mounted"))
}

#[test]
fn each_curve_ends_at_its_time_within_a_frame() {
  for curve in [
    Curve::Linear,
    Curve::EaseIn,
    Curve::EaseOut,
    Curve::EaseInOut,
  ] {
    let mut ui = TestUi::new(ui! { Holder { curve: curve } });
    assert_eq!(shown().last().unwrap().1, 0.0, "the first build jumps");
    assert!(!frame_requested(), "nothing moves yet");

    let start = ui.now();
    target().set(100.0);
    ui.update();
    ui.advance(Duration::from_secs(1));
    let values = shown();
    let (end, last) = *values.last().unwrap();
    assert_eq!(last, 100.0, "{curve:?}");
    let took = end - start;
    assert!(took >= TIME && took < TIME + FRAME, "{curve:?}: {took:?}");
    // Only moving forward, and only reaching the end at the end.
    for pair in values.windows(2) {
      assert!(pair[1].1 >= pair[0].1, "{curve:?}: {values:?}");
    }
    assert!(
      values[..values.len() - 1]
        .iter()
        .all(|v| v.1 < 100.0)
    );
    assert!(!frame_requested(), "{curve:?}: it stops asking");
  }
}

#[test]
fn a_linear_value_is_half_way_at_half_the_time() {
  let mut ui = TestUi::new(ui! { Holder { curve: Curve::Linear } });
  shown();
  let start = ui.now();
  target().set(100.0);
  ui.update();
  ui.advance(TIME / 2);
  let (at, value) = *shown().last().unwrap();
  let expected = (at - start).as_secs_f32() / TIME.as_secs_f32() * 100.0;
  assert!(
    (value - expected).abs() < 0.01,
    "{value} at {:?}",
    at - start
  );
  assert!((40.0..=50.0).contains(&value), "{value}");
}

#[test]
fn a_new_target_part_way_starts_from_the_value_shown() {
  let mut ui = TestUi::new(ui! { Holder { curve: Curve::Linear } });
  let start = ui.now();
  target().set(100.0);
  ui.update();
  ui.advance(TIME / 2);
  // The value at the turn, which may fall between frames.
  let half = (ui.now() - start).as_secs_f32() / TIME.as_secs_f32() * 100.0;

  // Back to 0: it turns back from where it is, with the full time again.
  let turn = ui.now();
  target().set(0.0);
  ui.update();
  let first = shown();
  let turned = first.last().unwrap().1;
  assert!(
    (turned - half).abs() < 0.01,
    "no jump when it turns: {turned}"
  );
  ui.advance(TIME / 2);
  let (at, value) = *shown().last().unwrap();
  let t = (at - turn).as_secs_f32() / TIME.as_secs_f32();
  assert!((value - half * (1.0 - t)).abs() < 0.01, "{value}");
  ui.advance(TIME);
  let (end, last) = *shown().last().unwrap();
  assert_eq!(last, 0.0);
  assert!(end - turn >= TIME && end - turn < TIME + FRAME);
}

#[test]
fn with_less_motion_values_jump() {
  let mut ui = TestUi::new(ui! { Holder { curve: Curve::EaseOut } });
  set_system_settings(SystemSettings {
    reduce_motion: true,
    ..SystemSettings::default()
  });
  ui.update();
  shown();
  target().set(100.0);
  ui.update();
  assert_eq!(shown(), [(ui.now(), 100.0)]);
  assert!(!frame_requested());

  // Less motion set part of the way through: it jumps to the end.
  set_system_settings(SystemSettings::default());
  ui.update();
  target().set(0.0);
  ui.update();
  ui.advance(TIME / 2);
  assert!(frame_requested());
  shown();
  set_system_settings(SystemSettings {
    reduce_motion: true,
    ..SystemSettings::default()
  });
  ui.update();
  assert_eq!(shown().last().unwrap().1, 0.0);
  assert!(!frame_requested());
}

thread_local! {
  static ON: RefCell<Option<Signal<bool>>> = const { RefCell::new(None) };
}

#[component]
fn FadeHolder() -> View {
  let on = use_state(|| false);
  ON.with(|o| *o.borrow_mut() = Some(on));
  ui! { Fade { on: on } }
}

/// A colour that fades.
#[component]
fn Fade(on: Signal<bool>) -> View {
  let color = use_animated(
    if on.get() { Color::WHITE } else { Color::BLACK },
    Transition::new(TIME, Curve::Linear),
  );
  ui! { Text { value: format!("{:.2}", color.r) } }
}

#[test]
fn colours_fade_in_linear_light() {
  let mut ui = TestUi::new(ui! { FadeHolder {} });
  let on = ON.with_borrow(|o| o.expect("mounted"));
  on.set(true);
  ui.update();
  // The sixth frame is at 100 ms.
  ui.advance(Duration::from_millis(101));
  // Half way in light is about 0.74 in sRGB.
  ui.find(dovetail_testing::text("0.74"));
  ui.advance(Duration::from_millis(110));
  ui.find(dovetail_testing::text("1.00"));
}

thread_local! {
  static PRESSED: RefCell<u32> = const { RefCell::new(0) };
}

/// M18 step 7: a box moved by an offset is clicked, focused and described
/// where it is drawn.
#[test]
fn clicks_and_bounds_follow_an_offset() {
  use dovetail_core::{Element, ElementKind, Role, Size, Style};
  let button: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(40.0),
      height: Size::Fixed(20.0),
      offset: (100.0, 50.0),
      ..Style::default()
    })
    .role(Role::Button)
    .label("Moved")
    .on_press(Callback::new(|()| PRESSED.with_borrow_mut(|p| *p += 1)))
    .into();
  let root: View = Element::new(ElementKind::Box)
    .children([button])
    .into();
  let mut ui = TestUi::new(root);
  let found = ui.find(dovetail_testing::label("Moved"));
  let r = found.rect();
  assert_eq!((r.x, r.y, r.width, r.height), (100.0, 50.0, 40.0, 20.0));
  ui.click_at((10.0, 10.0));
  assert_eq!(PRESSED.with_borrow(|p| *p), 0, "not where layout put it");
  ui.click_at((110.0, 60.0));
  assert_eq!(PRESSED.with_borrow(|p| *p), 1, "where it is drawn");
}
