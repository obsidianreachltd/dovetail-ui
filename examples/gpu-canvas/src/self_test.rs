//! The self-test: selects with a box and with clicks, zooms at a pad and
//! checks it stays under the pointer, and pans with the middle button.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::controls::Camera;
use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, Modifiers, NamedKey, WindowId,
  WindowOptions,
};
use dovetail::layout::{Content, LayoutBox};
use dovetail::prelude::*;
use dovetail::{PointerButton, PointerKind, Role};

use crate::board::{first_camera, selected_text};
use crate::scene::{self, Kind};

/// The canvas's size in the example.
const SIZE: (f32, f32) = (760.0, 470.0);
/// How far the middle-button drag moves, in pixels.
const PAN: (f32, f32) = (60.0, 40.0);
/// Wheel notches towards the screen: three, zooming in.
const ZOOM_STEPS: f32 = -3.0;

struct Test {
  window: Option<WindowId>,
  step: usize,
  /// What each step waits for, worked out as the test goes.
  waits: Vec<String>,
}

fn find_text<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().find(
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == wanted),
  )
}

/// The canvas's top-left corner in the window.
fn canvas_origin(boxes: &[LayoutBox]) -> Option<(f32, f32)> {
  boxes.iter().find_map(|b| match &b.content {
    Content::Element(e) if e.role == Role::Canvas => Some((b.rect.x, b.rect.y)),
    _ => None,
  })
}

fn centre_of(name: &str) -> (f32, f32) {
  let s = scene::small();
  let shape = s
    .shapes
    .iter()
    .find(|s| s.name == name)
    .expect("the shape");
  match shape.kind {
    Kind::Pad { centre, .. } | Kind::Via { centre, .. } => centre,
    Kind::Trace { from, .. } => from,
  }
}

fn command(c: char) -> KeyEvent {
  let m = if cfg!(target_os = "macos") {
    Modifiers { meta: true, ..Modifiers::default() }
  } else {
    Modifiers { control: true, ..Modifiers::default() }
  };
  let mut e = KeyEvent::character(c).with(m);
  e.text = None;
  e
}

/// The cameras the board goes through: at first, after zooming at Pad 9,
/// and after panning.
fn cameras() -> (Camera, Camera, Camera, (f32, f32)) {
  let s = scene::small();
  let first = first_camera(&s, SIZE);
  let pad = first.to_screen(centre_of("Pad 9"));
  let zoomed = first.zoom_at(pad, Camera::wheel_factor(ZOOM_STEPS * 48.0));
  let panned = zoomed.pan(PAN);
  (first, zoomed, panned, pad)
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let s = scene::small();
    let (first, zoomed, _, _) = cameras();
    // A box round the first chip, in world units, and what it catches.
    let chip = s.shapes_in((17.0, 17.0, 33.0, 43.0));
    let mut caught = vec![false; s.shapes.len()];
    for i in chip {
      caught[i] = true;
    }
    let mut two = vec![false; s.shapes.len()];
    for (i, shape) in s.shapes.iter().enumerate() {
      two[i] = shape.name == "Pad 9" || shape.name == "Pad 10";
    }
    self.waits = vec![
      "Selected: none".into(),
      selected_text(&s, &caught),
      "Selected: none".into(),
      "Selected: Pad 9".into(),
      format!("Zoom {:.0}%", zoomed.zoom / first.zoom * 100.0),
      "Selected: none".into(),
      "Selected: Pad 9".into(),
      "Selected: none".into(),
      "Selected: Pad 9".into(),
      selected_text(&s, &two),
      format!("Selected: {} shapes", s.shapes.len()),
    ];
    let options = WindowOptions::new("GPU canvas self-test");
    match cx.open_ui_window(options, ui! { crate::App {} }) {
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
    let Some(expected) = self.waits.get(self.step).cloned() else {
      return;
    };
    let boxes = cx.ui_boxes(window).to_vec();
    if find_text(&boxes, &expected).is_none() {
      return;
    }
    println!("PASS  the window shows \"{expected}\"");
    let Some(origin) = canvas_origin(&boxes) else {
      println!("FAIL  no canvas");
      cx.exit();
      return;
    };
    let window_at = |p: (f32, f32)| (origin.0 + p.0, origin.1 + p.1);
    let (first, zoomed, panned, pad) = cameras();
    let none = Modifiers::default();
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    let click = |cx: &mut Context<'_>, p: (f32, f32), m: Modifiers| {
      cx.pointer(window, PointerKind::Down, p.0, p.1, m);
      cx.pointer(window, PointerKind::Up, p.0, p.1, m);
    };
    self.step += 1;
    match self.step {
      // Drag a box round the first chip.
      1 => {
        let a = window_at(first.to_screen((17.0, 17.0)));
        let b = window_at(first.to_screen((33.0, 43.0)));
        cx.pointer(window, PointerKind::Down, a.0, a.1, none);
        for t in [0.25, 0.5, 0.75, 1.0] {
          let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
          cx.pointer(window, PointerKind::Move, p.0, p.1, none);
        }
        cx.pointer(window, PointerKind::Up, b.0, b.1, none);
      },
      // Escape: nothing selected. The canvas has focus from the drag.
      2 => {
        cx.key(window, &KeyEvent::named(NamedKey::Escape));
      },
      // Click Pad 9.
      3 => click(cx, window_at(pad), none),
      // Zoom in at Pad 9 with the wheel.
      4 => {
        cx.wheel_steps(window, window_at(pad), ZOOM_STEPS, none);
      },
      // Clear, then click the same place: Pad 9 is still there.
      5 => {
        cx.key(window, &KeyEvent::named(NamedKey::Escape));
      },
      6 => click(cx, window_at(pad), none),
      // Pan with the middle button, and clear.
      7 => {
        let a = window_at(pad);
        let b = (a.0 + PAN.0, a.1 + PAN.1);
        let m = PointerButton::Middle;
        cx.pointer_button(window, PointerKind::Down, a, none, m);
        cx.pointer_button(
          window,
          PointerKind::Move,
          ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
          none,
          m,
        );
        cx.pointer_button(window, PointerKind::Move, b, none, m);
        cx.pointer_button(window, PointerKind::Up, b, none, m);
        cx.key(window, &KeyEvent::named(NamedKey::Escape));
      },
      // Pad 9 moved with the drag: click where it is now.
      8 => click(cx, window_at(panned.to_screen(centre_of("Pad 9"))), none),
      // Shift+click adds Pad 10.
      9 => click(cx, window_at(panned.to_screen(centre_of("Pad 10"))), shift),
      // Ctrl+A selects everything.
      10 => {
        cx.key(window, &command('a'));
      },
      _ => {
        let _ = zoomed;
        PASSED.store(true, Ordering::Relaxed);
        println!(
          "PASS  box and click selection, zoom about the pointer, panning and select-all work"
        );
        cx.close_window(window);
      },
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = self
      .waits
      .get(self.step)
      .cloned()
      .unwrap_or_default();
    println!("FAIL  timed out waiting for \"{waiting}\"");
    cx.exit();
  }
}

static PASSED: AtomicBool = AtomicBool::new(false);

pub fn run() -> ExitCode {
  let test = Test { window: None, step: 0, waits: Vec::new() };
  match dovetail::desktop::run(test) {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("gpu-canvas: {e}");
      ExitCode::FAILURE
    },
  }
}
