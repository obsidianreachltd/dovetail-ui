//! How heavy text looks (M06 step 12 follow-up).
//!
//! The canvas blends in linear light. Left alone, that makes the soft
//! edges of dark text on a light background look too light, and of light
//! text on a dark one too heavy: black on white showed 72% of the ink
//! Windows' GDI+ draws, and white on black 146%. The glyph shader changes
//! each pixel's coverage to give, in brightness, the result of blending in
//! sRGB, for the text colour over what the painter knows is behind it.
//!
//! So the ink each case shows, measured as displayed, must equal the
//! glyphs' true coverage, whatever the colours. The text sits in an
//! element with its own background, over a canvas of another colour, so
//! the painter must find the right backdrop.
//!
//! These need a GPU adapter and the system fonts, so they do not run by
//! default: `cargo test -p dovetail-render --test text_weight -- --ignored`.

use dovetail_core::{
  BuildContext, Color, Component, Edges, Element, ElementKind, Size, Style,
  Tree, View,
};
use dovetail_layout::Layout;
use dovetail_render::{Gpu, PaintInput, Painter, font_measure};
use dovetail_text::Fonts;

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

const TEXT: &str = "The quick brown fox jumps over the lazy dog";
const SIZE: f32 = 14.0;

fn to_linear(c: f32) -> f32 {
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}

fn to_srgb(c: f32) -> f32 {
  if c <= 0.003_130_8 {
    c * 12.92
  } else {
    1.055 * c.powf(1.0 / 2.4) - 0.055
  }
}

/// Brightness as displayed: luminance in linear light, back to sRGB.
fn lightness(rgb: [f32; 3]) -> f32 {
  let [r, g, b] = rgb.map(to_linear);
  to_srgb(0.2126 * r + 0.7152 * g + 0.0722 * b)
}

/// The ink the text shows, in pixels' worth as displayed: how far each
/// pixel's brightness is from the background's towards the text's.
fn ink(
  foreground: Color,
  background: Color,
) -> f32 {
  let view: View = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(4.0),
      width: Size::Fill,
      height: Size::Fill,
      background,
      font_size: Some(SIZE),
      text_color: Some(foreground),
      ..Style::default()
    })
    .children([View::text(TEXT)])
    .into();
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(ShowProps { view }));
  let mut layout = Layout::new();
  layout.update(&tree, 400.0, 30.0, &font_measure());
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let mut canvas = painter.canvas(&gpu, 400, 30);
  painter.paint(
    &gpu,
    &mut canvas,
    &PaintInput {
      // Not what the text is on: the element's own background covers it.
      background: Color::hex(0xff00ff),
      ..PaintInput::new(layout.boxes(), 1.0)
    },
  );
  let f = lightness([foreground.r, foreground.g, foreground.b]);
  let b = lightness([background.r, background.g, background.b]);
  painter
    .read_pixels(&gpu, &canvas)
    .iter()
    .map(|p| {
      let l = lightness([p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0));
      (l - b) / (f - b)
    })
    .sum()
}

/// The glyphs' true coverage, in pixels' worth.
fn coverage() -> f32 {
  let fonts = Fonts::system();
  fonts
    .shape(TEXT, SIZE)
    .glyphs
    .iter()
    .map(|g| {
      let b = fonts.rasterize(g.font, g.glyph, SIZE, 0.0);
      b.coverage
        .iter()
        .map(|&c| f32::from(c) / 255.0)
        .sum::<f32>()
    })
    .sum()
}

#[test]
#[ignore = "needs a GPU adapter and the system fonts"]
fn text_shows_the_same_weight_on_any_background() {
  let want = coverage();
  println!("true coverage: {want:.1}");
  for (name, fg, bg) in [
    ("black on white", Color::BLACK, Color::WHITE),
    ("white on black", Color::WHITE, Color::BLACK),
    (
      "white on the accent blue",
      Color::WHITE,
      Color::hex(0x1e66f5),
    ),
    (
      "dark grey on light grey",
      Color::hex(0x4c4f69),
      Color::hex(0xeff1f5),
    ),
    ("blue on white", Color::hex(0x1e66f5), Color::WHITE),
  ] {
    let got = ink(fg, bg);
    println!("{name}: {got:.1} ({:.0}%)", 100.0 * got / want);
    assert!(
      (got / want - 1.0).abs() < 0.04,
      "{name}: {got:.1}, expected about {want:.1}"
    );
  }
}
