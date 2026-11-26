//! M18 step 10: controls that move. Each is stepped through its transition
//! frame by frame with the test's clock, and checked with less motion set,
//! when it jumps.

extern crate dovetail_core as dovetail;

use std::time::Duration;

use dovetail::{
  Animate, Color, Role, Signal, SystemSettings, View, frame_requested,
  set_system_settings, use_state,
};
use dovetail_controls::{CheckBox, Collapsible, Column, Text, Toggle};
use dovetail_layout::{Content, LayoutBox};
use dovetail_macros::{component, ui};
use dovetail_testing::{FRAME, Options, TestUi, role};
use dovetail_theme::{Motion, Theme, ThemeData, ThemeMode};

thread_local! {
  static ON: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}

fn on() -> Signal<bool> {
  ON.get().expect("mounted")
}

#[component]
fn Controls() -> View {
  let on = use_state(|| false);
  ON.set(Some(on));
  ui! {
    Column {
      Toggle { on: on.get(), on_change: move |v| on.set(v), label: "Lights" }
      CheckBox { checked: on.get(), on_change: move |v| on.set(v), label: "Tick" }
      Collapsible {
        title: "More",
        open: on.get(),
        on_toggle: move |v| on.set(v),
        Text { value: "Inside" }
      }
    }
  }
}

fn open(less_motion: bool) -> TestUi {
  TestUi::with(
    ui! { Controls {} },
    Options {
      size: (400.0, 300.0),
      settings: SystemSettings {
        reduce_motion: less_motion,
        ..SystemSettings::default()
      },
      ..Options::default()
    },
  )
}

fn find(
  ui: &TestUi,
  pick: impl Fn(&LayoutBox) -> bool,
) -> Option<LayoutBox> {
  ui.layout()
    .boxes()
    .iter()
    .find(|b| pick(b))
    .cloned()
}

/// Where the toggle's knob is, from the left of its track.
fn knob(ui: &TestUi) -> f32 {
  let track = find(ui, |b| b.rect.width == 36.0).expect("track");
  let knob =
    find(ui, |b| b.rect.width == 16.0 && b.rect.height == 16.0).expect("knob");
  knob.rect.x - track.rect.x - 2.0
}

/// The toggle's track colour.
fn track(ui: &TestUi) -> Color {
  match find(ui, |b| b.rect.width == 36.0).map(|b| b.content) {
    Some(Content::Element(e)) => e.style.background,
    _ => unreachable!(),
  }
}

/// How solid the check box's tick is, or `None` when there is none.
fn tick(ui: &TestUi) -> Option<f32> {
  find(ui, |b| {
    matches!(b.content, Content::Picture { .. }) && b.rect.width == 14.0
  })
  .map(|b| b.opacity)
}

/// How solid the section's content is, or `None` when it is not there.
fn inside(ui: &TestUi) -> Option<f32> {
  find(
    ui,
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == "Inside"),
  )
  .map(|b| b.opacity)
}

/// The values at each frame until nothing moves, at most a second.
fn frames<T>(
  ui: &mut TestUi,
  read: impl Fn(&TestUi) -> T,
) -> Vec<T> {
  let mut seen = vec![read(ui)];
  for _ in 0..60 {
    if !frame_requested() {
      break;
    }
    ui.advance(FRAME);
    seen.push(read(ui));
  }
  seen
}

fn rising(values: &[f32]) -> bool {
  values.windows(2).all(|w| w[1] >= w[0])
}

fn short() -> Duration {
  Motion::default().short
}

fn medium() -> Duration {
  Motion::default().medium
}

/// About how many frames a transition of `time` takes.
fn frames_in(time: Duration) -> usize {
  (time.as_secs_f64() / FRAME.as_secs_f64()).ceil() as usize
}

#[test]
fn the_toggle_knob_slides_and_its_track_fades() {
  let mut ui = open(false);
  let off = track(&ui);
  ui.click(role(Role::Switch).label("Lights"));
  assert_eq!(knob(&ui), 0.0, "it starts where it was");
  let seen = frames(&mut ui, |ui| (knob(ui), track(ui)));
  let xs: Vec<f32> = seen.iter().map(|s| s.0).collect();
  assert!(rising(&xs), "{xs:?}");
  assert_eq!(*xs.last().unwrap(), 16.0);
  // Every frame of the move is shown, and it ends within one more. The
  // section opening at the same time takes longer.
  let n = frames_in(short());
  let end = xs.iter().position(|&x| x == 16.0).unwrap();
  assert!((n..=n + 1).contains(&end), "{end} frames");
  // Half way through, the knob and the colour are on their way.
  let (x, colour) = seen[end / 2];
  assert!(x > 0.0 && x < 16.0, "{x}");
  assert!(colour != off && colour != seen[end].1);
  // The switch says it is on at once.
  assert_eq!(
    ui.find(role(Role::Switch).label("Lights"))
      .is_on(),
    Some(true)
  );
}

#[test]
fn the_check_box_tick_fades_in_and_out() {
  let mut ui = open(false);
  assert_eq!(tick(&ui), None);
  on().set(true);
  ui.update();
  let seen = frames(&mut ui, tick);
  let values: Vec<f32> = seen.iter().map(|t| t.expect("a tick")).collect();
  assert!(rising(&values), "{values:?}");
  assert_eq!(values[0], 0.0);
  assert_eq!(*values.last().unwrap(), 1.0);

  // Unticked, it fades out, then goes.
  on().set(false);
  ui.update();
  assert_eq!(tick(&ui), Some(1.0));
  let seen = frames(&mut ui, tick);
  assert_eq!(*seen.last().unwrap(), None, "{seen:?}");
  assert!(seen[1].is_some_and(|t| t < 1.0));
}

#[test]
fn the_section_fades_in_as_it_opens_and_out_as_it_closes() {
  let mut ui = open(false);
  assert_eq!(inside(&ui), None);
  ui.click(role(Role::Button).label("More"));
  // There at once, for keys and assistive tools, but clear.
  assert_eq!(inside(&ui), Some(0.0));
  let seen = frames(&mut ui, |ui| inside(ui).expect("open"));
  assert!(rising(&seen), "{seen:?}");
  assert_eq!(*seen.last().unwrap(), 1.0);
  let n = frames_in(medium());
  assert!(
    (n..=n + 1).contains(&(seen.len() - 1)),
    "{} frames",
    seen.len() - 1
  );

  ui.click(role(Role::Button).label("More"));
  let seen = frames(&mut ui, inside);
  assert_eq!(*seen.last().unwrap(), None, "gone at the end: {seen:?}");
}

#[test]
fn with_less_motion_every_control_jumps() {
  let mut ui = open(true);
  ui.click(role(Role::Switch).label("Lights"));
  assert!(!frame_requested(), "nothing asks for frames");
  assert_eq!(knob(&ui), 16.0);
  assert_eq!(tick(&ui), Some(1.0));
  assert_eq!(inside(&ui), Some(1.0));
  ui.click(role(Role::Switch).label("Lights"));
  assert!(!frame_requested());
  assert_eq!((knob(&ui), tick(&ui), inside(&ui)), (0.0, None, None));
}

thread_local! {
  static DARK: std::cell::Cell<Option<Signal<ThemeMode>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn Themed() -> View {
  let mode = use_state(|| ThemeMode::Light);
  DARK.set(Some(mode));
  ui! {
    Theme {
      data: ThemeData::light(),
      dark: Some(ThemeData::dark()),
      mode: mode.get(),
      Text { value: "Words" }
    }
  }
}

/// The window's background, which the outermost theme fills.
fn background(ui: &TestUi) -> Color {
  match &ui.layout().boxes()[0].content {
    Content::Element(e) => e.style.background,
    _ => unreachable!(),
  }
}

#[test]
fn the_theme_fades_from_one_to_the_next() {
  let mut ui = TestUi::new(ui! { Themed {} });
  let light = ThemeData::light().colors.surface;
  let dark = ThemeData::dark().colors.surface;
  assert_eq!(background(&ui), light);
  DARK.get().unwrap().set(ThemeMode::Dark);
  ui.update();
  let start = ui.now();
  let seen = frames(&mut ui, background);
  assert_eq!(*seen.last().unwrap(), dark);
  // Half way, in time, it is the colour half way along the curve.
  let half = ui.now() - start;
  assert!(half >= medium() && half < medium() + FRAME, "{half:?}");
  let middle = seen[seen.len() / 2];
  assert!(middle != light && middle != dark);
  let expected_mid = light.animate(&dark, 0.5);
  assert!(
    (middle.r - expected_mid.r).abs() < 0.2,
    "{middle:?} against {expected_mid:?}"
  );

  // With less motion it jumps.
  set_system_settings(SystemSettings {
    reduce_motion: true,
    ..SystemSettings::default()
  });
  DARK.get().unwrap().set(ThemeMode::Light);
  ui.update();
  assert_eq!(background(&ui), light);
  assert!(!frame_requested());
}
