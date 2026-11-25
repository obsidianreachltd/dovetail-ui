//! The self-test: drives the window through its own test functions (no
//! key presses reach the system) and checks, in its pixels and boxes:
//!
//! - the PNG and JPEG pictures at their size, fitted and filled, against
//!   the image they hold, in the light and dark theme;
//! - icons in the text's colour, which changes with the theme;
//! - an icon in a button, the toolbar's buttons, the menus' items, the
//!   tabs, a tree's rows and a dialog's title;
//! - that the window keeps drawing while a 4096 × 4096 image is made,
//!   decoded and uploaded, and shows it at the end;
//! - an icon with two colours keeps its red, while its other parts take
//!   the text's colour, in light and dark;
//! - an animated GIF and an APNG play, on screen, and stop when paused.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dovetail::desktop::{
  Application, Context, Frame, LogicalSize, WindowId, WindowOptions,
};
use dovetail::image::Icon;
use dovetail::layout::{Content, LayoutBox, Rect};
use dovetail::prelude::*;
use dovetail::theme::ThemeData;
use dovetail::{KeyEvent, NamedKey, Picture, Role};

use crate::{STATE, State};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  /// Choose the theme, and wait for the six small pictures.
  Theme(bool),
  Pictures(bool),
  IconColours(bool),
  ButtonsAndToolbar,
  MenuOpen,
  MenuClose,
  TreeTab,
  DialogOpen,
  DialogClose,
  Large,
  Animations,
  Paused,
  PlayAgain,
}

const STEPS: [Step; 17] = [
  Step::Theme(false),
  Step::Pictures(false),
  Step::IconColours(false),
  Step::ButtonsAndToolbar,
  Step::MenuOpen,
  Step::MenuClose,
  Step::TreeTab,
  Step::DialogOpen,
  Step::DialogClose,
  Step::Theme(true),
  Step::Pictures(true),
  Step::IconColours(true),
  Step::Large,
  Step::Animations,
  Step::Paused,
  Step::PlayAgain,
  Step::Theme(false),
];

impl Step {
  fn says(self) -> String {
    let theme = |dark: bool| if dark { "dark" } else { "light" };
    match self {
      Step::Theme(dark) => format!("the {} theme shows, with the pictures loaded", theme(dark)),
      Step::Pictures(dark) => format!(
        "PNG and JPEG pictures show at their size, fitted and filled, pixel by pixel ({})",
        theme(dark)
      ),
      Step::IconColours(dark) => format!(
        "icons are drawn in the {} theme's text colour, and an icon with two colours keeps its red",
        theme(dark)
      ),
      Step::Animations => "the animated GIF and APNG play, on screen".into(),
      Step::Paused => "Pause stops them".into(),
      Step::PlayAgain => "Play starts them again".into(),
      Step::ButtonsAndToolbar => "a button and every toolbar button show their command's icon".into(),
      Step::MenuOpen => "the File menu's items show their command's icons".into(),
      Step::MenuClose => "Escape closes the menu".into(),
      Step::TreeTab => "the tabs show icons, and so do the tree's rows".into(),
      Step::DialogOpen => "a dialog shows an icon by its title".into(),
      Step::DialogClose => "the dialog closes".into(),
      Step::Large => "the window keeps drawing while a 4096 x 4096 image loads, then shows it".into(),
    }
  }
}

fn element(b: &LayoutBox) -> Option<&dovetail::Element> {
  match &b.content {
    Content::Element(e) => Some(e),
    _ => None,
  }
}

/// The text inside box `i`.
fn text_of(
  boxes: &[LayoutBox],
  i: usize,
) -> String {
  let mut out = String::new();
  let mut stack = vec![i];
  while let Some(i) = stack.pop() {
    match &boxes[i].content {
      Content::Text { text, .. } => out.push_str(text),
      Content::Element(_) => stack.extend(boxes[i].children.iter().rev()),
      Content::Picture { .. } => {},
    }
  }
  out
}

/// The pictures inside box `i`, with their boxes.
fn pictures_in(
  boxes: &[LayoutBox],
  i: usize,
) -> Vec<usize> {
  let mut out = Vec::new();
  let mut stack = vec![i];
  while let Some(i) = stack.pop() {
    if matches!(boxes[i].content, Content::Picture { .. }) {
      out.push(i);
    }
    stack.extend(boxes[i].children.iter().copied());
  }
  out
}

/// Every element with `role` whose text or label starts with `text`.
fn all(
  boxes: &[LayoutBox],
  role: Role,
  text: &str,
) -> Vec<usize> {
  (0..boxes.len())
    .filter(|&i| {
      element(&boxes[i]).is_some_and(|e| {
        e.role == role
          && (text_of(boxes, i).starts_with(text)
            || e
              .label
              .as_deref()
              .is_some_and(|l| l.starts_with(text)))
      })
    })
    .collect()
}

/// The picture box named `label`.
fn picture(
  boxes: &[LayoutBox],
  label: &str,
) -> Result<(Picture, Rect), String> {
  boxes
    .iter()
    .find_map(|b| match &b.content {
      Content::Picture { picture, .. }
        if picture.label.as_deref() == Some(label) =>
      {
        Some((picture.clone(), b.rect))
      },
      _ => None,
    })
    .ok_or_else(|| format!("no picture named {label}"))
}

/// The pixel at logical (`x`, `y`).
struct Pixels {
  width: u32,
  scale: f32,
  data: Vec<[u8; 4]>,
}

impl Pixels {
  fn read(
    cx: &Context<'_>,
    main: WindowId,
  ) -> Result<Pixels, String> {
    let (width, _, data) = cx.ui_pixels(main).ok_or("no pixels")?;
    let scale = cx
      .window_info(main)
      .ok_or("no window")?
      .scale_factor as f32;
    Ok(Pixels { width, scale, data })
  }

  fn at(
    &self,
    x: f32,
    y: f32,
  ) -> [u8; 4] {
    let (px, py) = ((x * self.scale) as u32, (y * self.scale) as u32);
    self.data[(py * self.width + px) as usize]
  }
}

/// The source picture's colour at image pixel (`x`, `y`): see
/// `crates/dovetail-image/tests/fixtures/make.ps1`.
fn formula(
  x: f32,
  y: f32,
) -> [f32; 3] {
  [4.0 * x, 5.0 * y, 2.0 * (x + y)]
}

fn near(
  got: [u8; 4],
  want: [f32; 3],
  tolerance: f32,
) -> bool {
  (0..3).all(|c| (f32::from(got[c]) - want[c]).abs() <= tolerance)
}

fn srgb(c: Color) -> [f32; 3] {
  [c.r * 255.0, c.g * 255.0, c.b * 255.0]
}

/// Checks the picture `label` shows the source picture: at its size, the
/// whole picture; fitted and filled, its middle and edges.
fn check_picture(
  boxes: &[LayoutBox],
  pixels: &Pixels,
  label: &str,
  tolerance: f32,
  background: Color,
) -> Result<(), String> {
  let (_, r) = picture(boxes, label)?;
  let fail = |what: &str, got: [u8; 4], want: [f32; 3]| {
    Err(format!("{label}: {what}: {got:?}, want about {want:?}"))
  };
  if label.ends_with("at its size") {
    if (r.width, r.height) != (64.0, 48.0) {
      return Err(format!("{label}: {} × {}", r.width, r.height));
    }
    for iy in (2..48).step_by(5) {
      for ix in (2..64).step_by(5) {
        let (x, y) = (ix as f32 + 0.5, iy as f32 + 0.5);
        let got = pixels.at(r.x + x, r.y + y);
        if !near(got, formula(x, y), tolerance) {
          return fail(&format!("pixel ({ix}, {iy})"), got, formula(x, y));
        }
      }
    }
  } else if label.ends_with("fitted") {
    // 120 wide and 90 high, 15 down.
    let got = pixels.at(r.x + 60.0, r.y + 5.0);
    if !near(got, srgb(background), 3.0) {
      return fail("above the picture", got, srgb(background));
    }
    let got = pixels.at(r.x + 60.0, r.y + 60.0);
    if !near(got, formula(32.0, 24.0), tolerance) {
      return fail("the middle", got, formula(32.0, 24.0));
    }
  } else {
    // Filled: 160 × 120, with 20 cut off each side.
    let got = pixels.at(r.x + 60.0, r.y + 60.0);
    if !near(got, formula(32.0, 24.0), tolerance) {
      return fail("the middle", got, formula(32.0, 24.0));
    }
    let want = formula(8.0 + 2.0 / 2.5, 2.0 / 2.5);
    let got = pixels.at(r.x + 2.0, r.y + 2.0);
    if !near(got, want, tolerance + 4.0) {
      return fail("the top left", got, want);
    }
  }
  Ok(())
}

/// Checks that the icon in box `i` is drawn in `ink` wherever it covers a
/// pixel fully, and returns how many it does. A small icon of thin
/// diagonal lines may cover none.
fn check_icon(
  boxes: &[LayoutBox],
  pixels: &Pixels,
  i: usize,
  ink: Color,
) -> Result<usize, String> {
  let Content::Picture { picture, color, .. } = &boxes[i].content else {
    return Err("not a picture".into());
  };
  if *color != ink {
    return Err(format!("{:?} is in {color:?}, not {ink:?}", picture.label));
  }
  let icon = picture.source::<Icon>().ok_or("not an icon")?;
  let r = boxes[i].rect;
  let s = pixels.scale;
  let (x0, y0) = ((r.x * s).round() as u32, (r.y * s).round() as u32);
  let (w, h) = ((r.width * s).round() as u32, (r.height * s).round() as u32);
  let cover = icon.render(w, h);
  let mut solid = 0;
  for y in 0..h {
    for x in 0..w {
      if cover.alpha[(y * w + x) as usize] == 255 {
        let Some(&got) = pixels
          .data
          .get(((y0 + y) * pixels.width + x0 + x) as usize)
        else {
          return Err("an icon is outside the window".into());
        };
        if !near(got, srgb(ink), 3.0) {
          return Err(format!("{:?} at ({x}, {y}): {got:?}", picture.label));
        }
        solid += 1;
      }
    }
  }
  Ok(solid)
}

/// The colour in the middle of the animation `label`, on screen, and
/// checked against the frame its picture holds.
fn frame_shown(
  boxes: &[LayoutBox],
  pixels: &Pixels,
  label: &str,
) -> Result<[u8; 3], String> {
  let (picture, r) = picture(boxes, label)?;
  let frame = picture
    .source::<dovetail::image::Bitmap>()
    .ok_or("not an image")?
    .to_rgba();
  // The band's colour, at a corner, away from the ball.
  let i = (4 * frame.width as usize + 4) * 4;
  let want = [
    frame.pixels[i],
    frame.pixels[i + 1],
    frame.pixels[i + 2],
  ];
  let got = pixels.at(r.x + 4.5, r.y + 4.5);
  if !near(got, want.map(f32::from), 4.0) {
    return Err(format!("{label} shows {got:?}, its frame {want:?}"));
  }
  Ok(want)
}

/// Checks the icon with two colours: drawn as its colours with the text's
/// colour for its own parts.
fn check_two_colours(
  boxes: &[LayoutBox],
  pixels: &Pixels,
  ink: Color,
) -> Result<(), String> {
  let (picture, r) = picture(boxes, "Unsaved changes")?;
  let icon = picture.source::<Icon>().ok_or("not an icon")?;
  if !icon.is_multicolour() {
    return Err("the icon has one colour".into());
  }
  let s = pixels.scale;
  let (x0, y0) = ((r.x * s).round() as u32, (r.y * s).round() as u32);
  let (w, h) = ((r.width * s).round() as u32, (r.height * s).round() as u32);
  let want = icon.render_rgba(w, h, [ink.r, ink.g, ink.b, ink.a]);
  let (mut red, mut text) = (0, 0);
  for y in 0..h {
    for x in 0..w {
      let i = ((y * w + x) * 4) as usize;
      if want.pixels[i + 3] < 255 {
        continue;
      }
      let c = [
        want.pixels[i],
        want.pixels[i + 1],
        want.pixels[i + 2],
      ];
      let Some(&got) = pixels
        .data
        .get(((y0 + y) * pixels.width + x0 + x) as usize)
      else {
        return Err("an icon is outside the window".into());
      };
      if !near(got, c.map(f32::from), 3.0) {
        return Err(format!(
          "the two-colour icon at ({x}, {y}): {got:?}, want {c:?}"
        ));
      }
      if c == [0xe0, 0x26, 0x2b] {
        red += 1;
      } else if near([c[0], c[1], c[2], 255], srgb(ink), 3.0) {
        text += 1;
      }
    }
  }
  if red < 20 || text < 20 {
    return Err(format!("{red} red and {text} text-coloured pixels"));
  }
  Ok(())
}

/// The window's theme now.
fn theme_of(dark: bool) -> ThemeData {
  if dark {
    ThemeData::dark()
  } else {
    ThemeData::light()
  }
}

struct Test {
  main: Option<WindowId>,
  at: usize,
  acted: bool,
  last: String,
  /// While the large picture loads: when it started, the last frame, and
  /// the longest time between frames.
  large: Option<(Instant, Instant, Duration, u32)>,
  /// The frames seen of each animation.
  colours: std::collections::HashMap<&'static str, Vec<[u8; 3]>>,
  /// While paused: since when the frames shown have not changed.
  paused: Option<(Instant, Vec<[u8; 3]>)>,
}

static PASSED: AtomicBool = AtomicBool::new(false);

impl Test {
  fn state() -> Result<State, String> {
    STATE.get().ok_or_else(|| "no state".to_string())
  }

  fn act(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = Self::state()?;
    match step {
      Step::Theme(dark) => s.dark.set(dark),
      Step::MenuOpen => {
        let boxes = cx.ui_boxes(main);
        let file = *all(boxes, Role::MenuItem, "File")
          .first()
          .ok_or("no File menu")?;
        let r = boxes[file].rect;
        cx.click(main, r.x + r.width / 2.0, r.y + r.height / 2.0);
      },
      Step::MenuClose => {
        cx.key(main, &KeyEvent::named(NamedKey::Escape));
      },
      Step::TreeTab => s.tab.set(1),
      Step::DialogOpen => s.dialog.set(true),
      Step::DialogClose => {
        cx.key(main, &KeyEvent::named(NamedKey::Escape));
        // Back to the gallery, so the card is as tall as before.
        s.tab.set(0);
      },
      Step::Large => {
        let boxes = cx.ui_boxes(main);
        let button = *all(boxes, Role::Button, "Load a large image")
          .first()
          .ok_or("no button")?;
        let r = boxes[button].rect;
        cx.click(main, r.x + r.width / 2.0, r.y + r.height / 2.0);
        let now = Instant::now();
        self.large = Some((now, now, Duration::ZERO, 0));
      },
      Step::Paused => s.playing.set(false),
      Step::PlayAgain => s.playing.set(true),
      Step::Animations => self.colours.clear(),
      Step::Pictures(_) | Step::IconColours(_) | Step::ButtonsAndToolbar => {},
    }
    Ok(())
  }

  fn check(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = Self::state()?;
    let boxes = cx.ui_boxes(main);
    match step {
      Step::Theme(dark) => {
        if s.dark.get() != dark || s.ready.get() < 6 {
          return Err(format!("{} pictures ready", s.ready.get()));
        }
        let t = theme_of(dark);
        let root = boxes
          .iter()
          .find_map(|b| element(b).filter(|e| e.window_style.is_some()));
        if root.is_none_or(|e| e.style.background != t.colors.surface) {
          return Err("the theme's background".into());
        }
      },
      Step::Pictures(dark) => {
        let pixels = Pixels::read(cx, main)?;
        let card = theme_of(dark).colors.surface_container_low;
        for kind in ["PNG", "JPEG"] {
          let tolerance = if kind == "PNG" { 10.0 } else { 16.0 };
          for how in ["at its size", "fitted", "filled"] {
            check_picture(
              boxes,
              &pixels,
              &format!("{kind} {how}"),
              tolerance,
              card,
            )?;
          }
        }
      },
      Step::IconColours(dark) => {
        let pixels = Pixels::read(cx, main)?;
        let ink = theme_of(dark).colors.on_surface;
        // The toolbar's icons and the gallery's.
        let mut solid = 0;
        for i in 0..boxes.len() {
          if let Content::Picture { picture, .. } = &boxes[i].content
            && matches!(
              picture.label.as_deref(),
              Some("New" | "Save" | "Paste" | "search" | "check")
            )
          {
            solid += check_icon(boxes, &pixels, i, ink)?;
          }
        }
        if solid < 20 {
          return Err(format!("only {solid} solid pixels"));
        }
        check_two_colours(boxes, &pixels, ink)?;
      },
      Step::ButtonsAndToolbar => {
        let load = *all(boxes, Role::Button, "Load a large image")
          .first()
          .ok_or("no Load button")?;
        if pictures_in(boxes, load).len() != 1 {
          return Err("the Load button has no icon".into());
        }
        let toolbar = *all(boxes, Role::Toolbar, "")
          .first()
          .ok_or("no toolbar")?;
        let named: Vec<String> = pictures_in(boxes, toolbar)
          .iter()
          .filter_map(|&i| match &boxes[i].content {
            Content::Picture { picture, .. } => {
              picture.label.as_deref().map(str::to_owned)
            },
            _ => None,
          })
          .collect();
        if named.len() != 8 {
          return Err(format!("the toolbar's icons: {named:?}"));
        }
      },
      Step::MenuOpen => {
        for name in ["New", "Open", "Save"] {
          let items = all(boxes, Role::MenuItem, name);
          let item = items.iter().find(|&&i| {
            element(&boxes[i]).is_some_and(|e| e.label.as_deref() == Some(name))
          });
          let item = *item.ok_or_else(|| format!("no {name} item"))?;
          if pictures_in(boxes, item).len() != 1 {
            return Err(format!("the {name} item has no icon"));
          }
        }
      },
      Step::MenuClose => {
        if !all(boxes, Role::Menu, "").is_empty() {
          return Err("the menu is still open".into());
        }
      },
      Step::TreeTab => {
        let tabs = all(boxes, Role::Tab, "");
        if tabs.len() != 2
          || tabs
            .iter()
            .any(|&t| pictures_in(boxes, t).len() != 1)
        {
          return Err("the tabs' icons".into());
        }
        let rows = all(boxes, Role::TreeItem, "Board");
        let row = *rows.first().ok_or("no tree row")?;
        if pictures_in(boxes, row).is_empty() {
          return Err("the tree row has no icon".into());
        }
      },
      Step::DialogOpen => {
        let heading = all(boxes, Role::Heading, "Large files");
        let heading = *heading.first().ok_or("no dialog")?;
        if pictures_in(boxes, heading).len() != 1 {
          return Err("the dialog's title has no icon".into());
        }
      },
      Step::DialogClose => {
        if s.dialog.get() {
          return Err("the dialog is still open".into());
        }
      },
      Step::Animations | Step::PlayAgain => {
        // Each animation shows at least three of its frames, on screen.
        let pixels = Pixels::read(cx, main)?;
        for label in ["Animated GIF", "Animated PNG"] {
          let shown = frame_shown(boxes, &pixels, label)?;
          let seen = self.colours.entry(label).or_default();
          if !seen.contains(&shown) {
            seen.push(shown);
          }
        }
        let fewest = ["Animated GIF", "Animated PNG"]
          .iter()
          .map(|l| self.colours.get(l).map_or(0, Vec::len))
          .min();
        if fewest < Some(3) {
          return Err(format!("frames seen: {:?}", self.colours));
        }
        self.colours.clear();
      },
      Step::Paused => {
        // Still for half a second.
        let pixels = Pixels::read(cx, main)?;
        let now: Vec<[u8; 3]> = ["Animated GIF", "Animated PNG"]
          .iter()
          .map(|l| frame_shown(boxes, &pixels, l))
          .collect::<Result<_, _>>()?;
        match &self.paused {
          Some((since, before)) if *before == now => {
            if since.elapsed() < Duration::from_millis(500) {
              return Err("watching".into());
            }
          },
          _ => {
            self.paused = Some((Instant::now(), now));
            return Err("a frame changed".into());
          },
        }
      },
      Step::Large => {
        let Some((start, last, longest, frames)) = &mut self.large else {
          return Err("not started".into());
        };
        let now = Instant::now();
        *longest = (*longest).max(now - *last);
        *last = now;
        *frames += 1;
        let state = s.large_state.get();
        if state.starts_with("failed") {
          return Err(state);
        }
        if state != "ready" {
          return Err(format!("still {state}"));
        }
        // Ready: once drawn, the middle shows the picture's middle,
        // (128, 128) in red and green.
        let (_, r) = picture(boxes, "Large image")?;
        let pixels = Pixels::read(cx, main)?;
        let got = pixels.at(r.x + r.width / 2.0, r.y + r.height / 2.0);
        if !near(got, [128.0, 128.0, f32::from(got[2])], 10.0) {
          return Err(format!("the large image's middle is {got:?}"));
        }
        let (took, longest, frames) = (start.elapsed(), *longest, *frames);
        println!(
          "      it took {:.1} s, over {frames} frames; the longest time between frames was {} ms",
          took.as_secs_f32(),
          longest.as_millis()
        );
        if longest > Duration::from_millis(250) {
          return Err(format!("a frame took {} ms", longest.as_millis()));
        }
      },
    }
    Ok(())
  }
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Images self-test")
      .with_size(LogicalSize::new(1180.0, 820.0));
    match cx.open_ui_window(options, ui! { crate::App {} }) {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(120));
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
    _: WindowId,
  ) {
    let Some(main) = self.main else {
      return;
    };
    if STATE.get().is_none() {
      return;
    }
    let Some(&step) = STEPS.get(self.at) else {
      return;
    };
    if !self.acted {
      if let Err(e) = self.act(cx, main, step) {
        println!("FAIL  {}: {e}", step.says());
        cx.exit();
        return;
      }
      self.acted = true;
      cx.request_redraw(main);
      return;
    }
    match self.check(cx, main, step) {
      Ok(()) => {
        println!("PASS  {}", step.says());
        self.at += 1;
        self.acted = false;
        if self.at == STEPS.len() {
          PASSED.store(true, Ordering::Relaxed);
          println!("PASS  images and icons show and load as they should");
          cx.close_window(main);
          return;
        }
      },
      Err(e) => self.last = e,
    }
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS
      .get(self.at)
      .map_or_else(String::new, |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    println!("      last: {}", self.last);
    cx.exit();
  }
}

pub fn run() -> ExitCode {
  let test = Test {
    main: None,
    at: 0,
    acted: false,
    last: String::new(),
    large: None,
    colours: Default::default(),
    paused: None,
  };
  let result = dovetail::desktop::run(test);
  // The large picture's file.
  let _ = std::fs::remove_file(
    std::env::temp_dir()
      .join(format!("dovetail-large-{}.png", std::process::id())),
  );
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("images: {e}");
      ExitCode::FAILURE
    },
  }
}
