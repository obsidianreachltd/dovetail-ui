//! M17: `Image` loads a file off the UI thread, with a placeholder until it
//! is ready, and `Icon` and `IconButton` give pictures their sizes and
//! names. These run the controls with no window or GPU (`TestUi`, M20),
//! and move its clock to play animations, so nothing waits in real time
//! but decoding.

extern crate dovetail_core as dovetail;

use std::path::Path;
use std::time::Duration;

use dovetail::{Picture, Role, Size, View, use_state};
use dovetail_controls::{
  Button, Column, Icon, IconButton, Image, ImageState, Text,
};
use dovetail_image::icons;
use dovetail_layout::{Content, LayoutBox};
use dovetail_macros::{component, ui};
use dovetail_testing::{Options, TestUi, text};

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

fn open(path: &str) -> TestUi {
  TestUi::with(
    ui! { Pictures { path: path } },
    Options {
      size: (400.0, 600.0),
      ..Options::default()
    },
  )
}

/// The pictures shown, with their boxes.
fn pictures(ui: &TestUi) -> Vec<(Picture, &LayoutBox)> {
  ui.layout()
    .boxes()
    .iter()
    .filter_map(|b| match &b.content {
      Content::Picture { picture, .. } => Some((picture.clone(), b)),
      _ => None,
    })
    .collect()
}

/// Waits, in real time, for the decoding to report `state`.
fn wait_for(
  ui: &mut TestUi,
  state: &str,
) {
  ui.wait_until(state, |ui| {
    ui.find_all(dovetail_testing::role(Role::Label))
      .iter()
      .any(|f| f.text().is_some_and(|t| t.starts_with(state)))
  });
}

#[test]
fn an_image_shows_a_placeholder_then_the_picture() {
  let mut w = open(&fixture("gdi.png"));
  // While loading: placeholders, one named for assistive tools, and the
  // other pictures.
  assert!(w.has(text("loading")));
  let placeholder = w
    .layout()
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
  wait_for(&mut w, "ready");
  let pictures = pictures(&w);
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
  let mut w = open(&fixture("gdi.png"));
  // Once loaded, the placeholders' icons are gone.
  wait_for(&mut w, "ready");
  let icons: Vec<_> = pictures(&w)
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
  let mut w = open(&fixture("make.ps1"));
  wait_for(&mut w, "failed");
  assert!(w.has(text("failed: not a PNG, JPEG or GIF image")));
  let failed = w
    .layout()
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

/// The shared images, decoded, at their first frame.
fn open_shared(bytes: Arc<[u8]>) -> TestUi {
  let mut ui = TestUi::with(
    ui! { Shared { bytes: bytes } },
    Options {
      size: (400.0, 600.0),
      ..Options::default()
    },
  );
  // Decoding is the one thing that takes real time.
  ui.wait_until("the first frame", |ui| colour(ui).is_some());
  ui
}

/// The colour of the first picture shown, once ready.
fn colour(ui: &TestUi) -> Option<[u8; 3]> {
  let (picture, _) = pictures(ui)
    .into_iter()
    .find(|(p, _)| p.label.as_deref() == Some("One"))?;
  let p = picture.source::<Bitmap>()?.to_rgba().pixels;
  Some([p[0], p[1], p[2]])
}

/// How long each frame shows.
const FRAME: Duration = Duration::from_millis(40);

/// The colour now, then after each of `frames` moves of the clock by one
/// frame's time.
fn play(
  ui: &mut TestUi,
  frames: usize,
) -> Vec<[u8; 3]> {
  let mut seen = vec![colour(ui).expect("a frame")];
  for _ in 0..frames {
    ui.advance(FRAME);
    seen.push(colour(ui).expect("a frame"));
  }
  seen
}

const R: [u8; 3] = [255, 0, 0];
const G: [u8; 3] = [0, 255, 0];
const B: [u8; 3] = [0, 0, 255];

#[test]
fn images_of_the_same_bytes_share_one_decoding() {
  let before = image_decodes();
  let w = open_shared(three_colours(Some(1)));
  assert_eq!(
    pictures(&w)
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
  assert_eq!(play(&mut w, 5), [R, G, B, R, G, B], "twice through");
  // Long after, it is still on the last frame.
  w.advance(Duration::from_secs(5));
  assert_eq!(colour(&w), Some(B), "stopped on the last frame");
}

#[test]
fn an_animation_stops_when_paused_and_plays_on() {
  let mut w = open_shared(three_colours(None));
  // Past the end of the first play: it goes on.
  assert_eq!(play(&mut w, 4), [R, G, B, R, G], "plays for ever");
  PLAYING.get().expect("state").set(false);
  w.update();
  w.advance(Duration::from_secs(5));
  assert_eq!(colour(&w), Some(G), "still while paused");
  PLAYING.get().expect("state").set(true);
  w.update();
  assert_eq!(play(&mut w, 3), [G, B, R, G], "plays on");
}
