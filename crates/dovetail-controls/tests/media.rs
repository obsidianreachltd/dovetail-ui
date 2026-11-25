//! M17: `Image` loads a file off the UI thread, with a placeholder until it
//! is ready, and `Icon` and `IconButton` give pictures their sizes and
//! names. These lay the controls out with no window or GPU.

extern crate dovetail_core as dovetail;

use std::path::Path;
use std::time::{Duration, Instant};

use dovetail::{Picture, Role, Size, Tree, View, run_task_messages, use_state};
use dovetail_controls::{
  Button, Column, Icon, IconButton, Image, ImageState, Text,
};
use dovetail_image::icons;
use dovetail_layout::{Content, FixedPitch, Layout, LayoutBox};
use dovetail_macros::{component, ui};

fn fixture(name: &str) -> String {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../dovetail-image/tests/fixtures")
    .join(name)
    .to_string_lossy()
    .into_owned()
}

#[component]
fn Pictures(#[prop(into)] path: String) -> View {
  let state = use_state(|| "loading".to_string());
  ui! {
    Column {
      Image {
        source: path.as_str(),
        label: "Gradient",
        on_state: move |s: ImageState| state.set(match s {
          ImageState::Ready(_) => "ready".to_string(),
          ImageState::Failed(e) => format!("failed: {e}"),
          ImageState::Loading => "loading".to_string(),
        }),
      }
      Image { source: path.as_str(), label: "", width: Size::Fixed(32.0), height: Size::Fixed(24.0) }
      Icon { icon: icons::save() }
      Icon { icon: icons::save(), size: 24.0, label: "Saved" }
      Button { text: "Open", icon: Some(icons::open()) }
      IconButton { icon: icons::close(), label: "Close" }
      Text { value: state.get() }
    }
  }
}

struct Window {
  tree: Tree,
  layout: Layout,
}

impl Window {
  fn open(path: &str) -> Self {
    let mut tree = Tree::new();
    tree.mount(ui! { Pictures { path: path } });
    let mut w = Self { tree, layout: Layout::new() };
    w.refresh();
    w
  }

  fn refresh(&mut self) {
    self.tree.update();
    self
      .layout
      .update(&self.tree, 400.0, 600.0, &FixedPitch::default());
  }

  fn pictures(&self) -> Vec<(Picture, &LayoutBox)> {
    self
      .layout
      .boxes()
      .iter()
      .filter_map(|b| match &b.content {
        Content::Picture { picture, .. } => Some((picture.clone(), b)),
        _ => None,
      })
      .collect()
  }

  fn has_text(
    &self,
    start: &str,
  ) -> bool {
    self
      .layout
      .boxes()
      .iter()
      .any(|b| matches!(&b.content, Content::Text { text, .. } if text.starts_with(start)))
  }

  /// Waits for the decoding tasks to report.
  fn wait_for(
    &mut self,
    start: &str,
  ) {
    let began = Instant::now();
    while !self.has_text(start) {
      assert!(began.elapsed() < Duration::from_secs(10), "no {start}");
      std::thread::sleep(Duration::from_millis(5));
      run_task_messages();
      self.refresh();
    }
  }
}

#[test]
fn an_image_shows_a_placeholder_then_the_picture() {
  let mut w = Window::open(&fixture("gdi.png"));
  // While loading: placeholders, one named for assistive tools, and the
  // other pictures.
  assert!(w.has_text("loading"));
  let placeholder = w
    .layout
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Element(e) if e.role == Role::Image => {
        Some((e.label.clone(), b.rect))
      },
      _ => None,
    })
    .expect("a placeholder");
  assert_eq!(placeholder.0.as_deref(), Some("Gradient"));
  w.wait_for("ready");
  let pictures = w.pictures();
  let images: Vec<_> = pictures
    .iter()
    .filter(|(p, _)| !p.is_icon())
    .collect();
  assert_eq!(images.len(), 2, "both images loaded");
  // The first at its own size, named; the second in its box, decoration.
  assert_eq!(
    (images[0].1.rect.width, images[0].1.rect.height),
    (64.0, 48.0)
  );
  assert_eq!(images[0].0.label.as_deref(), Some("Gradient"));
  assert_eq!(
    (images[1].1.rect.width, images[1].1.rect.height),
    (32.0, 24.0)
  );
  assert_eq!(images[1].0.label, None);
}

#[test]
fn icons_take_the_text_size_or_their_own() {
  let mut w = Window::open(&fixture("gdi.png"));
  // Once loaded, the placeholders' icons are gone.
  w.wait_for("ready");
  let icons: Vec<_> = w
    .pictures()
    .into_iter()
    .filter(|(p, _)| p.is_icon())
    .collect();
  // Save, save at 24, the button's open, and close.
  assert_eq!(icons.len(), 4);
  assert_eq!(icons[0].1.rect.height, 16.0, "beside 14-pixel text");
  assert_eq!(icons[1].1.rect.height, 24.0);
  assert_eq!(icons[1].0.label.as_deref(), Some("Saved"));
  assert_eq!(icons[2].0.label, None, "a button's icon is decoration");
  assert_eq!(
    icons[3].0.label.as_deref(),
    Some("Close"),
    "an icon-only button's icon names it"
  );
}

#[test]
fn a_broken_file_says_why() {
  let mut w = Window::open(&fixture("make.ps1"));
  w.wait_for("failed");
  assert!(w.has_text("failed: not a PNG, JPEG or GIF image"));
  let failed = w
    .layout
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Element(e) if e.role == Role::Image => e.description.clone(),
      _ => None,
    });
  assert_eq!(failed.as_deref(), Some("not a PNG, JPEG or GIF image"));
}

// --- Sharing and animation (D0024) ------------------------------------------

use std::cell::Cell;
use std::sync::Arc;

use dovetail::Signal;
use dovetail_controls::{ImageSource, image_decodes};
use dovetail_image::{Bitmap, GifFrame, Rgba, encode_gif};

thread_local! {
  static PLAYING: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
}

/// Three frames of one colour each, 40 ms apart, played `plays` times.
fn three_colours(plays: Option<u32>) -> Arc<[u8]> {
  let frame = |c: [u8; 3]| GifFrame {
    image: Rgba {
      width: 4,
      height: 4,
      pixels: [c[0], c[1], c[2], 255].repeat(16),
    },
    left: 0,
    top: 0,
    delay: Duration::from_millis(40),
    disposal: 1,
    interlaced: false,
  };
  let frames = [
    frame([255, 0, 0]),
    frame([0, 255, 0]),
    frame([0, 0, 255]),
  ];
  encode_gif(4, 4, &frames, plays).unwrap().into()
}

#[component]
fn Shared(bytes: Arc<[u8]>) -> View {
  let playing = use_state(|| true);
  PLAYING.set(Some(playing));
  ui! {
    Column {
      Image { source: ImageSource::Bytes(bytes.clone()), label: "One", playing: playing.get() }
      Image { source: ImageSource::Bytes(bytes.clone()), label: "Two", playing: playing.get() }
    }
  }
}

fn open_shared(bytes: Arc<[u8]>) -> Window {
  let mut tree = Tree::new();
  tree.mount(ui! { Shared { bytes: bytes } });
  let mut w = Window { tree, layout: Layout::new() };
  w.refresh();
  w
}

/// The colour of the first picture shown, once ready.
fn colour(w: &Window) -> Option<[u8; 3]> {
  let (picture, _) = w
    .pictures()
    .into_iter()
    .find(|(p, _)| p.label.as_deref() == Some("One"))?;
  let p = picture.source::<Bitmap>()?.to_rgba().pixels;
  Some([p[0], p[1], p[2]])
}

/// The colours shown, each change once, until `enough` of them have been
/// seen or `time` has passed.
///
/// The tests wait for what they expect, not for a fixed time, because CI
/// machines can be slow to decode and to wake for the next frame.
fn watch_until(
  w: &mut Window,
  enough: usize,
  time: Duration,
) -> Vec<[u8; 3]> {
  let start = Instant::now();
  let mut seen: Vec<[u8; 3]> = Vec::new();
  while seen.len() < enough && start.elapsed() < time {
    std::thread::sleep(Duration::from_millis(3));
    run_task_messages();
    w.refresh();
    if let Some(c) = colour(w)
      && seen.last() != Some(&c)
    {
      seen.push(c);
    }
  }
  seen
}

/// The colours shown over `time`, each change once.
fn watch(
  w: &mut Window,
  time: Duration,
) -> Vec<[u8; 3]> {
  watch_until(w, usize::MAX, time)
}

/// Long enough for anything these tests wait for, on a slow machine.
const PATIENCE: Duration = Duration::from_secs(10);

/// Far longer than a frame (40 ms): if nothing changes in this time, the
/// animation is still.
const STILL: Duration = Duration::from_millis(300);

const R: [u8; 3] = [255, 0, 0];
const G: [u8; 3] = [0, 255, 0];
const B: [u8; 3] = [0, 0, 255];

#[test]
fn images_of_the_same_bytes_share_one_decoding() {
  let before = image_decodes();
  let mut w = open_shared(three_colours(Some(1)));
  watch_until(&mut w, 1, PATIENCE);
  assert_eq!(
    w.pictures()
      .iter()
      .filter(|(p, _)| !p.is_icon())
      .count(),
    2
  );
  assert_eq!(image_decodes() - before, 1, "decoded once for both");
}

#[test]
fn an_animation_plays_as_many_times_as_it_says_then_stops() {
  let mut w = open_shared(three_colours(Some(2)));
  let seen = watch_until(&mut w, 6, PATIENCE);
  assert_eq!(seen, [R, G, B, R, G, B], "twice through");
  let after = watch(&mut w, STILL);
  assert_eq!(after, [B], "stopped on the last frame");
}

#[test]
fn an_animation_stops_when_paused_and_plays_on() {
  let mut w = open_shared(three_colours(None));
  // Past the end of the first play: it goes on.
  let seen = watch_until(&mut w, 5, PATIENCE);
  assert_eq!(seen, [R, G, B, R, G], "plays for ever");
  PLAYING.get().expect("state").set(false);
  w.refresh();
  let paused = colour(&w);
  let still = watch(&mut w, STILL);
  assert_eq!(still.len(), 1, "one frame while paused: {still:?}");
  assert_eq!(still.first().copied(), paused);
  PLAYING.get().expect("state").set(true);
  let again = watch_until(&mut w, 3, PATIENCE);
  assert_eq!(again.len(), 3, "plays on: {again:?}");
}
