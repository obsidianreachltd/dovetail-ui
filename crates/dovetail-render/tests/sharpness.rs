//! M06 "Done when" item 7, and M13 item 4 for Arabic: text is sharp at
//! 100%, 150% and 200% display scale.
//!
//! Sharp means two things here. Glyphs are rasterised at the display's
//! size (not drawn at 100% and stretched), and each glyph is placed on
//! whole pixels (not blurred across two). So, at each scale, every pixel
//! the rasteriser covers fully must come out as solid ink, and every pixel
//! it leaves empty must stay background. Any blur, wrong size or half-pixel
//! shift breaks that.
//!
//! The text starts at an odd logical position, so at 150% its box begins
//! half way through a pixel and the painter has to round.
//!
//! These need a GPU adapter and the system fonts, so they do not run by
//! default: `cargo test -p dovetail-render --test sharpness -- --ignored`.

use dovetail_core::{
  BuildContext, Color, Component, Edges, Element, ElementKind, Style, Tree,
  View,
};
use dovetail_layout::{Content, Layout};
use dovetail_render::{Gpu, PaintInput, Painter, font_measure};
use dovetail_text::Fonts;

/// Shows a fixed view.
struct Show;

#[derive(PartialEq)]
struct ShowProps {
  view: View,
}

impl Component for Show {
  type Props = ShowProps;

  fn build(
    props: &ShowProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    props.view.clone()
  }
}

const TEXT: &str = "Hamburg Sixty 日本";
/// Arabic with marks: "bismi" with its vowel signs, and "salam".
const ARABIC: &str = "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0633}\u{0644}\u{0627}\u{0645}";
const FONT_SIZE: f32 = 13.0;
const LOGICAL: (f32, f32) = (180.0, 40.0);

/// What one scale showed.
struct Result {
  /// Pixels that should be solid ink, and how many were.
  full: (usize, usize),
  /// Pixels that should be empty, and how many were.
  empty: (usize, usize),
  /// How much ink the canvas shows, in whole pixels' worth, measured in
  /// linear light.
  ink: f32,
}

fn check(
  text_shown: &str,
  font_size: f32,
  scale: f32,
) -> Result {
  let fonts = Fonts::system();
  assert!(!fonts.is_empty(), "this test needs the system fonts");

  // Black text on white, starting at an odd logical position.
  let view: View = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges {
        left: 7.0,
        top: 5.0,
        right: 0.0,
        bottom: 0.0,
      },
      font_size: Some(font_size),
      text_color: Some(Color::BLACK),
      ..Style::default()
    })
    .children([View::text(text_shown)])
    .into();
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(ShowProps { view }));
  let mut layout = Layout::new();
  layout.update(&tree, LOGICAL.0, LOGICAL.1, &font_measure());

  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let (width, height) = (
    (LOGICAL.0 * scale).ceil() as u32,
    (LOGICAL.1 * scale).ceil() as u32,
  );
  let mut canvas = painter.canvas(&gpu, width, height);
  painter.paint(
    &gpu,
    &mut canvas,
    &PaintInput {
      background: Color::WHITE,
      ..PaintInput::new(layout.boxes(), scale)
    },
  );
  let pixels = painter.read_pixels(&gpu, &canvas);

  // What the rasteriser covers at this scale, with pens and the baseline
  // on whole pixels.
  let text = layout
    .boxes()
    .iter()
    .find(|b| matches!(b.content, Content::Text { .. }))
    .expect("a text box")
    .rect;
  let size = font_size * scale;
  let line = fonts.shape(text_shown, size);
  // Right-to-left lines line up on the right of their box.
  let start = if line.rtl {
    (text.width * scale - line.width).max(0.0)
  } else {
    0.0
  };
  let baseline = (text.y * scale + line.metrics.ascent).round() as i32;
  let mut expected = vec![0u8; (width * height) as usize];
  for g in &line.glyphs {
    let pen = (text.x * scale + start + g.x).round() as i32;
    // Marks are raised or lowered onto their letter.
    let baseline = (baseline as f32 - g.y).round() as i32;
    let b = fonts.rasterize(g.font, g.glyph, size, 0.0);
    for by in 0..b.height as i32 {
      for bx in 0..b.width as i32 {
        let (x, y) = (pen + b.left + bx, baseline - b.top + by);
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
          continue;
        }
        let i = (y as u32 * width + x as u32) as usize;
        let c = b.coverage[(by as u32 * b.width + bx as u32) as usize];
        expected[i] = expected[i].max(c);
      }
    }
  }

  // The painter blends in linear light, so measure ink there too: an sRGB
  // value back to the share of the pixel the glyph covers.
  let linear = |v: u8| {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 {
      c / 12.92
    } else {
      ((c + 0.055) / 1.055).powf(2.4)
    }
  };
  let ink = pixels.iter().map(|p| 1.0 - linear(p[0])).sum();
  let mut result = Result { full: (0, 0), empty: (0, 0), ink };
  for (i, &c) in expected.iter().enumerate() {
    let ink = pixels[i][0];
    match c {
      255 => {
        result.full.0 += 1;
        if ink <= 2 {
          result.full.1 += 1;
        }
      },
      0 => {
        result.empty.0 += 1;
        if ink >= 253 {
          result.empty.1 += 1;
        }
      },
      _ => {},
    }
  }
  result
}

#[test]
#[ignore = "needs a GPU adapter and the system fonts"]
fn text_is_sharp_at_100_150_and_200_percent() {
  sharp(TEXT, FONT_SIZE);
}

#[test]
#[ignore = "needs a GPU adapter and the system fonts"]
fn arabic_is_sharp_at_100_150_and_200_percent() {
  // Arabic strokes are thin: at 13 pixels no pixel is fully covered.
  sharp(ARABIC, 20.0);
}

fn sharp(
  text: &str,
  font_size: f32,
) {
  let mut inks = Vec::new();
  for scale in [1.0, 1.5, 2.0] {
    let r = check(text, font_size, scale);
    println!(
      "{:>3}%: {} of {} full pixels solid, {} of {} empty pixels clear,        {:.0} pixels of ink",
      (scale * 100.0) as u32,
      r.full.1,
      r.full.0,
      r.empty.1,
      r.empty.0,
      r.ink
    );
    // Enough fully covered pixels for the check to mean something. How
    // many there are depends on the system's fonts: Segoe UI on Windows
    // gives 57 at 100%, and the CI runners for macOS and Linux gave 18.
    assert!(r.full.0 >= 10, "at {scale}: the text has solid pixels");
    assert_eq!(r.full.1, r.full.0, "at {scale}: every full pixel is solid");
    assert_eq!(
      r.empty.1, r.empty.0,
      "at {scale}: no ink outside the glyphs"
    );
    inks.push(r.ink);
  }
  // Drawn at the display's size: the ink grows with the square of the
  // scale, about 2.25 times at 150% and 4 times at 200%.
  for (i, want) in [(1, 2.25), (2, 4.0)] {
    let ratio = inks[i] / inks[0];
    assert!(
      (ratio / want - 1.0).abs() < 0.15,
      "ink at scale {i}: × {ratio}, expected about × {want}"
    );
  }
}
